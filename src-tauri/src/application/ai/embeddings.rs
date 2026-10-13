//! 嵌入（向量化）Provider（D3 / N7 第二步）。
//!
//! 抽成 trait 有两个原因：
//! 1. **可测**：向量通道的端到端测试不该依赖真实网络，用确定性的假实现就能跑通
//!    "构建索引 → 查询 → 融合"整条链路；
//! 2. **可换**：在线嵌入是"谁提供向量"的实现细节，检索层只认 `EmbeddingProvider`。
//!
//! 纪律（与在线模型同一套）：**Key 只出现在请求头**，错误信息只带主机名与状态码；
//! 出错一律向上返回 `Err`，由调用方降级（通道不可用 → 只用 BM25）。

use std::time::Duration;

use crate::core::error::{AppError, ErrorCode, Result};

/// 单次请求最多带多少条文本（各家限制不同，保守取小；实现内部会按需拆分）。
pub const MAX_BATCH: usize = 16;

/// 单条文本的字符上限：控制成本，也避免超出端点限制。
pub const MAX_INPUT_CHARS: usize = 2000;

/// 向量化 Provider。
pub trait EmbeddingProvider: Send + Sync {
    /// 稳定标识：入库时写进 `chunk_embeddings.model`（换实现/换模型必须能区分）。
    fn id(&self) -> String;
    /// 批量向量化（顺序与输入一一对应）。
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// 在线嵌入：OpenAI 兼容的 `POST {base}/embeddings`。
pub struct RemoteEmbeddingProvider {
    base_url: String,
    model: String,
    api_key: String,
    /// 出站代理（为空 = 跟随环境变量 / 系统代理）
    proxy: Option<String>,
}

impl RemoteEmbeddingProvider {
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            model: model.trim().to_string(),
            api_key: api_key.trim().to_string(),
            proxy: None,
        }
    }

    /// 指定出站代理。
    pub fn with_proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy.filter(|value| !value.trim().is_empty());
        self
    }

    /// 只用于错误信息的主机名（**绝不回显 Key 或完整 URL**）。
    fn host_label(&self) -> String {
        super::host::host_of(&self.base_url).unwrap_or_else(|| "（未知主机）".to_string())
    }

    /// 模型标识（入库的 `chunk_embeddings.model` 列）。
    ///
    /// **自检/状态检查要跟它同源**，否则会出现"明明建好了却说不可用"
    /// （按用户填的模型名去数，而库里存的是带 provider 前缀的标识）。
    pub fn provider_id(model: &str) -> String {
        format!("remote.embeddings:{}", model.trim())
    }
}

impl EmbeddingProvider for RemoteEmbeddingProvider {
    fn id(&self) -> String {
        Self::provider_id(&self.model)
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        for batch in texts.chunks(MAX_BATCH) {
            let agent =
                crate::core::net::agent_for(&self.base_url, Duration::from_secs(60), self.proxy.as_deref());
            let body = serde_json::json!({ "model": self.model, "input": batch });
            let response = agent
                .post(format!("{}/embeddings", self.base_url))
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("Bearer {}", self.api_key))
                .send(body.to_string())
                .map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidInput,
                        format!(
                            "嵌入端点调用失败：{error}（主机 {}，模型 `{}`）",
                            self.host_label(),
                            self.model
                        ),
                    )
                })?;
            let status = response.status().as_u16();
            if status >= 400 {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "嵌入端点返回 HTTP {status}（主机 {}，模型 `{}`）：检查 API Key、嵌入模型名与账户额度",
                        self.host_label(),
                        self.model
                    ),
                ));
            }
            let text = response.into_body().read_to_string().map_err(|error| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!("读取嵌入响应失败（主机 {}）：{error}", self.host_label()),
                )
            })?;
            let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!("嵌入响应不是合法 JSON（主机 {}）：{error}", self.host_label()),
                )
            })?;
            let data = value
                .get("data")
                .and_then(|entry| entry.as_array())
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::InvalidInput,
                        format!("嵌入响应缺少 data 数组（主机 {}）", self.host_label()),
                    )
                })?;
            // 端点通常带 `index`：按它排序，保证与输入一一对应
            let mut items: Vec<(i64, Vec<f32>)> = Vec::with_capacity(data.len());
            for (position, item) in data.iter().enumerate() {
                let index = item
                    .get("index")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(position as i64);
                let vector: Vec<f32> = item
                    .get("embedding")
                    .and_then(|entry| entry.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(serde_json::Value::as_f64)
                            .map(|value| value as f32)
                            .collect()
                    })
                    .unwrap_or_default();
                if vector.is_empty() {
                    return Err(AppError::new(
                        ErrorCode::InvalidInput,
                        format!("嵌入响应里有空向量（主机 {}）", self.host_label()),
                    ));
                }
                items.push((index, vector));
            }
            items.sort_by_key(|(index, _)| *index);
            if items.len() != batch.len() {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "嵌入响应条数对不上（发出 {} 条、回来 {} 条，主机 {}）",
                        batch.len(),
                        items.len(),
                        self.host_label()
                    ),
                ));
            }
            vectors.extend(items.into_iter().map(|(_, vector)| vector));
        }
        Ok(vectors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 错误信息里绝不能出现 Key —— 它会进界面、进日志、进问题反馈。
    #[test]
    fn error_messages_never_leak_the_api_key() {
        let provider = RemoteEmbeddingProvider::new("http://127.0.0.1:9/v1", "m", "sk-secret-value");
        let error = provider
            .embed(&["测试".to_string()])
            .expect_err("端口没人听，必须失败");
        assert!(!error.message.contains("sk-secret-value"), "{}", error.message);
        assert!(error.message.contains("主机"), "错误要能定位主机：{}", error.message);
    }
}
