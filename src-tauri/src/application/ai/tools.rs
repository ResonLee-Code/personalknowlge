//! AI 只读工具层（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §3、§9.5）。
//!
//! 约定：
//! - 工具**只读**；任何写库都必须回到「建议 + 用户确认」再走常规业务命令；
//! - 所有外发动作（联网）先过 `AiPolicyService` 的闸门，禁止在工具里自行判断；
//! - 工具返回结构化结果，便于决策层校验与审计（不含正文以外的隐式副作用）。

use std::sync::Arc;
use std::time::Instant;

use crate::application::clip_service::{self, ClippedPage};
use crate::core::error::{AppError, ErrorCode, Result};
use crate::domain::search::SearchQuery;

use super::super::context::WorkspaceContext;
use super::super::search_service::SearchService;
use super::policy::AiPolicyService;

/// 单次 `note_read` 返回的正文上限（防止把整篇长文塞进上下文）。
const READ_MAX_CHARS: usize = 4000;

/// 一条本地检索命中（引用定位到笔记 + 片段）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbHit {
    pub note_id: String,
    pub title: String,
    /// 命中片段（正文摘录）
    pub snippet: String,
}

pub struct AiTools {
    ctx: Arc<WorkspaceContext>,
}

impl AiTools {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(Arc::clone(&self.ctx))
    }

    /// `kb_search`：复用既有 FTS5 检索（零额外依赖，永远可用，不需要联网/B 开关）。
    pub fn kb_search(&self, keyword: &str, limit: usize) -> Result<Vec<KbHit>> {
        let trimmed = keyword.trim();
        if trimmed.is_empty() {
            return Err(AppError::invalid_input("检索关键词不能为空。"));
        }
        let query = SearchQuery {
            keyword: trimmed.to_string(),
            page: Some(1),
            page_size: Some(limit.clamp(1, 50) as i64),
            ..Default::default()
        };
        let response = SearchService::new(Arc::clone(&self.ctx)).search(query)?;
        Ok(response
            .hits
            .into_iter()
            .map(|hit| KbHit {
                note_id: hit.note.id,
                title: hit.note.title,
                snippet: hit
                    .snippet
                    .iter()
                    .map(|segment| segment.text.clone())
                    .collect::<String>(),
            })
            .collect())
    }

    /// `note_read`：读一篇笔记的标题与正文（截断到上限）。
    ///
    /// 受 §6「发送原文」开关约束：策略里的默认口径是「只发检索片段」，
    /// 因此这里返回正文供**本地**使用；是否外发由调用方再过一次闸门。
    pub fn note_read(&self, note_id: &str) -> Result<(String, String)> {
        let service = super::super::note_service::NoteService::new(Arc::clone(&self.ctx));
        let detail = service.get(note_id)?;
        let content: String = detail.content.chars().take(READ_MAX_CHARS).collect();
        Ok((detail.note.title, content))
    }

    /// `web_fetch`：抓取网页（**必须**过联网闸门 + 抓取正文开关）。
    ///
    /// `confirmed` 表示调用方已取得用户的单次确认；未确认时这里不会发起任何请求。
    pub fn web_fetch(&self, url: &str, confirmed: bool) -> Result<ClippedPage> {
        let started = Instant::now();
        let policy = self.policy();
        policy.ensure_network_allowed("web_fetch", url, confirmed)?;
        policy.ensure_fetch_body_allowed("web_fetch")?;

        let result = clip_service::clip(url);
        match &result {
            Ok(_) => policy.audit(
                "web_fetch",
                true,
                "ok",
                started.elapsed().as_millis() as u64,
                None,
            ),
            Err(error) => policy.audit(
                "web_fetch",
                true,
                "error",
                started.elapsed().as_millis() as u64,
                Some(error.code.as_str()),
            ),
        }
        result
    }

    /// `web_search`：联网检索（D3）。
    ///
    /// 顺序：总开关 → 联网档位 + 单次确认 → **检索源地址**（在「设置 → AI」配置，
    /// 应用不预置任何第三方服务）→ 发一次请求并把结果解析成候选来源。
    /// 未配置检索源时**不发起任何请求**，如实报错。
    pub fn web_search(
        &self,
        keyword: &str,
        limit: usize,
        confirmed: bool,
    ) -> Result<Vec<super::web_search::WebSearchHit>> {
        let started = Instant::now();
        let service = self.policy();
        let endpoint = service.policy().search_endpoint.clone();
        service.ensure_network_allowed(
            "web_search",
            endpoint.as_deref().unwrap_or(""),
            confirmed,
        )?;
        let Some(endpoint) = endpoint else {
            service.audit("web_search", false, "error", 0, Some("SEARCH_ENDPOINT_MISSING"));
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "还没有配置联网检索源：请在「设置 → AI → 检索源地址」填入一个返回 JSON 的检索地址（含 {query} 占位符）。应用不预置任何第三方检索服务。",
            ));
        };
        let result = super::web_search::search(&endpoint, keyword, limit);
        match &result {
            Ok(_) => service.audit(
                "web_search",
                true,
                "ok",
                started.elapsed().as_millis() as u64,
                None,
            ),
            Err(error) => service.audit(
                "web_search",
                true,
                "error",
                started.elapsed().as_millis() as u64,
                Some(error.code.as_str()),
            ),
        }
        result
    }
}
