//! 模板生成 Provider —— **无模型、无网络**的 `GenerativeModel` 实现（D1 假 Provider）。
//!
//! 存在的理由：研究助手链路（立项 → 收集 → 验证 → 汇总）必须在没有 LLM 时也能跑通，
//! 且产出必须**明显可辨**为模板文本，避免被误当成模型结论（§9.6 的降级路径）。
//! 输出是确定性的：同样的输入得到同样的文本，便于测试与复核。

use std::time::Instant;

use crate::core::error::Result;
use crate::core::text::first_sentence;

use super::traits::{GenerationRequest, GenerationResponse, GenerativeModel};

/// 模板实现：把证据片段按固定格式拼成草稿，不做任何语义推断。
#[derive(Debug, Default, Clone)]
pub struct TemplateGenerativeModel;

impl TemplateGenerativeModel {
    pub fn new() -> Self {
        Self
    }
}

impl GenerativeModel for TemplateGenerativeModel {
    fn id(&self) -> &'static str {
        "template.draft"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn complete(&self, request: &GenerationRequest) -> Result<GenerationResponse> {
        let started = Instant::now();
        let mut lines = Vec::new();
        if request.evidence.is_empty() {
            lines.push("（模板草稿）本次没有可用材料，无法给出任何推测内容。".to_string());
        } else {
            lines.push(format!(
                "（模板草稿·未接模型）以下是按材料顺序罗列的待确认方向，共 {} 条，**不构成结论**：",
                request.evidence.len()
            ));
            for (index, chunk) in request.evidence.iter().enumerate() {
                let heading = if chunk.heading_path.is_empty() {
                    String::new()
                } else {
                    format!("{} · ", chunk.heading_path.join(" / "))
                };
                lines.push(format!(
                    "{}. {}{} —— {}",
                    index + 1,
                    heading,
                    first_sentence(&chunk.snippet),
                    chunk.note_id
                ));
            }
            lines.push(
                "下一步需要接入模型（或人工复核）才能把这些方向写成结论。".to_string(),
            );
        }
        Ok(GenerationResponse {
            text: lines.join("\n"),
            model: self.id().to_string(),
            duration_ms: started.elapsed().as_millis() as u64,
            usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::{RetrievalOrigin, RetrievedChunk};

    fn chunk(note_id: &str, snippet: &str) -> RetrievedChunk {
        RetrievedChunk {
            note_id: note_id.to_string(),
            chunk_index: 0,
            heading_path: vec!["索引".to_string()],
            snippet: snippet.to_string(),
            char_start: 0,
            char_end: snippet.chars().count() as i64,
            score: 1.0,
            origin: RetrievalOrigin::Bm25,
        }
    }

    #[test]
    fn template_model_is_deterministic_and_clearly_labelled() {
        let model = TemplateGenerativeModel::new();
        let request = GenerationRequest {
            prompt: "生成推测段".into(),
            evidence: vec![chunk("note-1", "第一句。第二句。"), chunk("note-2", "只有一句")],
            max_tokens: None,
            allow_remote: false,
        };
        let first = model.complete(&request).expect("模板生成不应失败");
        let second = model.complete(&request).expect("模板生成不应失败");
        assert_eq!(first.text, second.text, "同输入必须同输出");
        assert_eq!(first.model, "template.draft");
        assert!(first.usage.is_none(), "模板没有 token 用量");
        assert!(first.text.contains("未接模型"));
        assert!(first.text.contains("不构成结论"));
        assert!(model.is_local() && !model.supports_streaming());
    }

    #[test]
    fn empty_evidence_is_reported_honestly() {
        let model = TemplateGenerativeModel::new();
        let response = model
            .complete(&GenerationRequest {
                prompt: "生成".into(),
                evidence: vec![],
                max_tokens: None,
                allow_remote: false,
            })
            .expect("空证据也必须返回");
        assert!(response.text.contains("没有可用材料"));
    }
}
