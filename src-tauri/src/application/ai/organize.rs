//! Organize P0：摘要 / 标签 / 关系 / 重复建议与持久化批处理。
//!
//! 生成建议时只写 SQLite；接受建议后才走常规笔记 / 标签 / 关系写入路径。
//! 接受前保存完整文件文本，撤销时按原字节写回，而不是重新序列化 frontmatter。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use serde_json::json;

use crate::application::context::WorkspaceContext;
use crate::application::note_service::NoteService;
use crate::application::relation_service::{AddRelationInput, RelationService};
use crate::application::tag_service::TagService;
use crate::core::chunk::chunk_markdown;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events;
use crate::core::hash::sha256_hex_str;
use crate::core::ids::ulid;
use crate::core::text::{first_sentence, markdown_to_plain_text, truncate_chars};
use crate::core::time::now_iso;
use crate::domain::note::{NoteListQuery, SaveNoteInput};
use crate::domain::organize::{
    AiJob, AiJobItemStatus, AiJobProgress, AiJobStatus, AiSuggestion, AiSuggestionPayload,
    DuplicateCandidate, OrganizeBatchFailure, OrganizeBatchReport, SuggestedRelation, SuggestedTag,
    SuggestionEvidence, SuggestionKind, SuggestionStatus,
};
use crate::repositories::ai_organize_repository::{
    AiOrganizeRepository, SqliteAiOrganizeRepository, SuggestionRow,
};

use super::embeddings::{embedding_provider, MAX_INPUT_CHARS};
use super::model_gateway::generation_candidates;
use super::policy::AiPolicyService;
use super::research::polarity;
use super::traits::{GenerationRequest, RetrievedChunk};
use super::vector_index::VectorIndexService;

const RULE_MODEL: &str = "rule.organize";
const PROMPT_VERSION: &str = "organize-v1";
const MAX_EVIDENCE: usize = 4;
const MAX_EVIDENCE_CHARS: usize = 700;
const MAX_TAG_SUGGESTIONS: usize = 5;
const MAX_RELATION_SUGGESTIONS: usize = 4;
const MAX_DUPLICATE_CANDIDATES: usize = 5;

struct RefinePayloadInput<'a> {
    note_id: &'a str,
    title: &'a str,
    body: &'a str,
    fallback: AiSuggestionPayload,
    evidence: &'a [SuggestionEvidence],
    kind: SuggestionKind,
    confirm_outbound: bool,
}

pub struct OrganizeService {
    ctx: Arc<WorkspaceContext>,
}

impl OrganizeService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 生成一条建议。只写建议表，不修改 Markdown。
    pub fn generate(&self, note_id: &str, kind: SuggestionKind) -> Result<AiSuggestion> {
        self.generate_with_confirmation(note_id, kind, false)
    }

    pub fn generate_with_confirmation(
        &self,
        note_id: &str,
        kind: SuggestionKind,
        confirm_outbound: bool,
    ) -> Result<AiSuggestion> {
        let started = Instant::now();
        let policy = AiPolicyService::new(Arc::clone(&self.ctx));
        policy.ensure_enabled("organize_generate")?;
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let detail = notes.get(note_id)?;
        let raw = self.ctx.storage.read_raw(&detail.note.file_path)?;
        let source_hash = sha256_hex_str(&raw);
        let evidence = self.evidence_for(note_id, &detail.note.title, &detail.content)?;
        let (payload, extra_evidence, vector_outbound_attempted) = self.build_payload(
            note_id,
            &detail.note.title,
            &detail.content,
            kind,
            policy.outbound_confirmed(confirm_outbound),
        )?;
        let mut all_evidence = evidence;
        for item in extra_evidence {
            if !all_evidence.iter().any(|existing| {
                existing.note_id == item.note_id
                    && existing.char_start == item.char_start
                    && existing.char_end == item.char_end
            }) {
                all_evidence.push(item);
            }
        }
        let (payload, model, model_outbound_attempted) = self.refine_payload(RefinePayloadInput {
            note_id,
            title: &detail.note.title,
            body: &detail.content,
            fallback: payload,
            evidence: &all_evidence,
            kind,
            confirm_outbound,
        });
        let outbound_attempted = vector_outbound_attempted || model_outbound_attempted;
        let row = SuggestionRow {
            id: ulid(),
            workspace_id: self.ctx.workspace.id.clone(),
            note_id: note_id.to_string(),
            note_title: detail.note.title.clone(),
            kind: kind.as_str().to_string(),
            status: SuggestionStatus::Pending.as_str().to_string(),
            model,
            prompt_version: PROMPT_VERSION.to_string(),
            evidence: serde_json::to_value(&all_evidence)?,
            payload: serde_json::to_value(&payload)?,
            source_hash,
            before_raw: None,
            applied_hash: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            applied_at: None,
            undone_at: None,
            error: None,
        };
        self.ctx
            .db
            .with_transaction(|tx| SqliteAiOrganizeRepository.replace_pending(tx, &row))?;
        policy.audit(
            "organize_generate",
            outbound_attempted,
            "ok",
            started.elapsed().as_millis() as u64,
            Some(kind.as_str()),
        );
        let saved = self
            .ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.get_suggestion(conn, &row.id))?
            .ok_or_else(|| AppError::internal("建议写入后无法读回。"))?;
        suggestion_view(saved)
    }

    fn refine_payload(&self, input: RefinePayloadInput<'_>) -> (AiSuggestionPayload, String, bool) {
        let RefinePayloadInput {
            note_id,
            title,
            body,
            fallback,
            evidence,
            kind,
            confirm_outbound,
        } = input;
        if !matches!(kind, SuggestionKind::Summary | SuggestionKind::Tags) {
            return (fallback, RULE_MODEL.to_string(), false);
        }
        let mut warnings = Vec::new();
        let candidates = generation_candidates(
            &self.ctx,
            "organize_generate",
            confirm_outbound,
            &mut warnings,
        );
        if candidates.is_empty() {
            return (fallback, RULE_MODEL.to_string(), false);
        }
        let outbound_attempted = candidates.iter().any(|(_, tier, _, _)| tier == "online");
        let instructions = if kind == SuggestionKind::Summary {
            "为给定笔记生成**原文摘要建议**。只输出严格 JSON，不要 Markdown 代码围栏：\
             {\"summary\":\"不超过240字的摘要\",\"bullets\":[\"最多4个要点\"]}。\
             不得加入原文没有的事实；材料不足就直说。"
        } else {
            "为给定笔记建议标签。只输出严格 JSON，不要 Markdown 代码围栏：\
             {\"tags\":[{\"tag\":\"标签\",\"reason\":\"一句话依据\"}]}。\
             最多5个标签，优先复用已有标签；不得创建过泛标签。"
        };
        let request = GenerationRequest {
            prompt: format!(
                "笔记标题：{title}\n笔记正文：\n{}",
                truncate_chars(body, 6000)
            ),
            instructions: Some(instructions.to_string()),
            evidence: evidence
                .iter()
                .map(|item| RetrievedChunk {
                    note_id: note_id.to_string(),
                    chunk_index: 0,
                    heading_path: item
                        .heading
                        .split(" > ")
                        .filter(|part| !part.is_empty())
                        .map(str::to_string)
                        .collect(),
                    snippet: item.snippet.clone(),
                    char_start: item.char_start,
                    char_end: item.char_end,
                    score: 1.0,
                    origin: super::traits::RetrievalOrigin::Bm25,
                })
                .collect(),
            max_tokens: Some(700),
            allow_remote: false,
        };
        let existing_tags = TagService::new(Arc::clone(&self.ctx))
            .list()
            .unwrap_or_default()
            .into_iter()
            .map(|tag| tag.name.to_lowercase())
            .collect::<HashSet<_>>();
        for (provider, _tier, is_local, model) in candidates {
            let mut outgoing = request.clone();
            outgoing.allow_remote = !is_local;
            let Ok(response) = model.complete(&outgoing) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(response.text.trim()) else {
                continue;
            };
            if kind == SuggestionKind::Summary {
                let Some(summary) = value.get("summary").and_then(|item| item.as_str()) else {
                    continue;
                };
                if summary.trim().is_empty() {
                    continue;
                }
                let bullets = value
                    .get("bullets")
                    .and_then(|item| item.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str())
                            .map(|item| truncate_chars(item, 180))
                            .take(4)
                            .collect()
                    })
                    .unwrap_or_default();
                return (
                    AiSuggestionPayload::Summary {
                        summary: truncate_chars(summary.trim(), 240),
                        bullets,
                    },
                    provider,
                    outbound_attempted,
                );
            }
            let Some(items) = value.get("tags").and_then(|item| item.as_array()) else {
                continue;
            };
            let tags = items
                .iter()
                .filter_map(|item| {
                    let tag = item.get("tag")?.as_str()?.trim();
                    if tag.is_empty() {
                        return None;
                    }
                    let reason = item
                        .get("reason")
                        .and_then(|value| value.as_str())
                        .unwrap_or("模型认为与正文相关");
                    Some(SuggestedTag {
                        tag: truncate_chars(tag, 24),
                        reason: truncate_chars(reason, 160),
                        existing: existing_tags.contains(&tag.to_lowercase()),
                    })
                })
                .take(MAX_TAG_SUGGESTIONS)
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                return (
                    AiSuggestionPayload::Tags { tags },
                    provider,
                    outbound_attempted,
                );
            }
        }
        (fallback, RULE_MODEL.to_string(), outbound_attempted)
    }

    pub fn list_suggestions(
        &self,
        status: Option<SuggestionStatus>,
        kind: Option<SuggestionKind>,
        limit: usize,
    ) -> Result<Vec<AiSuggestion>> {
        let rows = self.ctx.db.with_conn(|conn| {
            SqliteAiOrganizeRepository.list_suggestions(
                conn,
                &self.ctx.workspace.id,
                status.map(SuggestionStatus::as_str),
                kind.map(SuggestionKind::as_str),
                limit.clamp(1, 500),
            )
        })?;
        rows.into_iter().map(suggestion_view).collect()
    }

    pub fn reject(&self, id: &str) -> Result<AiSuggestion> {
        let changed = self
            .ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.reject_suggestion(conn, id))?;
        if !changed {
            return Err(AppError::invalid_input("建议不存在或已不是待确认状态。"));
        }
        let rejected = self.get(id)?;
        AiPolicyService::new(Arc::clone(&self.ctx)).audit(
            "organize_reject",
            false,
            "ok",
            0,
            Some(rejected.kind.as_str()),
        );
        Ok(rejected)
    }

    /// 批量接受：逐条应用并如实报告部分失败，不把整批失败伪装成成功。
    pub fn accept_many(&self, ids: &[String]) -> OrganizeBatchReport {
        let mut succeeded = 0usize;
        let mut failures = Vec::new();
        for id in ids {
            match self.accept(id) {
                Ok(_) => succeeded += 1,
                Err(error) => failures.push(OrganizeBatchFailure {
                    suggestion_id: id.clone(),
                    message: error.message,
                }),
            }
        }
        OrganizeBatchReport {
            requested: ids.len(),
            succeeded,
            failures,
        }
    }

    /// 接受建议：先检查来源哈希，再走常规业务写入；最后登记可撤销快照。
    pub fn accept(&self, id: &str) -> Result<AiSuggestion> {
        let row = self
            .ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.get_suggestion(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "建议不存在。"))?;
        if row.status != SuggestionStatus::Pending.as_str() {
            return Err(AppError::invalid_input("这条建议已经处理过。"));
        }
        let kind = SuggestionKind::parse(&row.kind)
            .ok_or_else(|| AppError::invalid_input("未知建议类型。"))?;
        let payload: AiSuggestionPayload = serde_json::from_value(row.payload.clone())?;
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let detail = notes.get(&row.note_id)?;
        let before_raw = self.ctx.storage.read_raw(&detail.note.file_path)?;
        if sha256_hex_str(&before_raw) != row.source_hash {
            return Err(AppError::new(
                ErrorCode::NoteConflict,
                "笔记在建议生成后被修改；请拒绝旧建议并重新生成。",
            )
            .with_details(json!({
                "suggestionId": id,
                "noteId": row.note_id,
                "expectedHash": row.source_hash,
                "currentHash": sha256_hex_str(&before_raw),
            })));
        }

        if let Err(error) = self.apply_payload(&detail, &payload) {
            // 关系建议可能包含多条写入；任何一步失败都要回到接受前快照，
            // 不能留下“建议未接受，但文件已部分修改”的状态。
            let _ = notes.restore_raw_bytes(&row.note_id, &before_raw);
            return Err(error);
        }
        let after_raw = match self.ctx.storage.read_raw(&detail.note.file_path) {
            Ok(raw) => raw,
            Err(error) => {
                let _ = notes.restore_raw_bytes(&row.note_id, &before_raw);
                return Err(error);
            }
        };
        let applied_hash = sha256_hex_str(&after_raw);
        let accepted = self.ctx.db.with_conn(|conn| {
            SqliteAiOrganizeRepository.accept_suggestion(conn, id, &before_raw, &applied_hash)
        });
        match accepted {
            Ok(true) => {}
            Ok(false) => {
                let _ = notes.restore_raw_bytes(&row.note_id, &before_raw);
                return Err(AppError::invalid_input("建议状态已变化，本次应用已回滚。"));
            }
            Err(error) => {
                let _ = notes.restore_raw_bytes(&row.note_id, &before_raw);
                return Err(error);
            }
        }
        AiPolicyService::new(Arc::clone(&self.ctx)).audit(
            "organize_accept",
            false,
            "ok",
            0,
            Some(kind.as_str()),
        );
        self.get(id)
    }

    /// 一键撤销：按接受前完整文本原字节恢复。
    pub fn undo(&self, id: &str) -> Result<AiSuggestion> {
        let row = self
            .ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.get_suggestion(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "建议不存在。"))?;
        if row.status != SuggestionStatus::Accepted.as_str() || row.undone_at.is_some() {
            return Err(AppError::invalid_input("这条建议当前不可撤销。"));
        }
        let before_raw = row
            .before_raw
            .as_deref()
            .ok_or_else(|| AppError::internal("建议缺少撤销快照。"))?;
        let applied_hash = row.applied_hash.as_deref().ok_or_else(|| {
            AppError::internal("建议缺少接受后的文件哈希，无法安全撤销；请重新生成建议。")
        })?;
        let notes = NoteService::new(Arc::clone(&self.ctx));
        let detail = notes.get(&row.note_id)?;
        let current_raw = self.ctx.storage.read_raw(&detail.note.file_path)?;
        let current_hash = sha256_hex_str(&current_raw);
        if current_hash != applied_hash {
            return Err(AppError::new(
                ErrorCode::NoteConflict,
                "笔记在接受建议后又被修改；为避免覆盖新内容，旧建议不能撤销。",
            )
            .with_details(json!({
                "suggestionId": id,
                "noteId": row.note_id,
                "expectedHash": applied_hash,
                "currentHash": current_hash,
            })));
        }
        notes.restore_raw_bytes(&row.note_id, before_raw)?;
        let undone = self
            .ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.undo_suggestion(conn, id));
        match undone {
            Ok(Some((snapshot, _))) if snapshot.as_str() == before_raw => {}
            Ok(Some(_)) => {
                let _ = notes.restore_raw_bytes(&row.note_id, &current_raw);
                return Err(AppError::internal(
                    "撤销快照在读取期间发生变化；本次已回滚。",
                ));
            }
            Ok(None) => {
                let _ = notes.restore_raw_bytes(&row.note_id, &current_raw);
                return Err(AppError::invalid_input("建议已被撤销或状态已变化。"));
            }
            Err(error) => {
                let _ = notes.restore_raw_bytes(&row.note_id, &current_raw);
                return Err(error);
            }
        };
        AiPolicyService::new(Arc::clone(&self.ctx)).audit(
            "organize_undo",
            false,
            "ok",
            0,
            Some(&row.kind),
        );
        self.get(id)
    }

    fn get(&self, id: &str) -> Result<AiSuggestion> {
        let row = self
            .ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.get_suggestion(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "建议不存在。"))?;
        suggestion_view(row)
    }

    fn apply_payload(
        &self,
        detail: &crate::domain::note::NoteDetail,
        payload: &AiSuggestionPayload,
    ) -> Result<()> {
        match payload {
            AiSuggestionPayload::Summary { summary, bullets } => {
                let mut section = format!("\n\n## 摘要\n\n{}", summary.trim());
                if !bullets.is_empty() {
                    section.push_str("\n\n");
                    for bullet in bullets {
                        section.push_str(&format!("- {}\n", bullet.trim()));
                    }
                }
                if detail.content.contains(summary.trim()) {
                    return Ok(());
                }
                NoteService::new(Arc::clone(&self.ctx)).save(SaveNoteInput {
                    id: detail.note.id.clone(),
                    content: format!("{}{}", detail.content.trim_end(), section),
                    base_content_hash: detail.note.content_hash.clone(),
                    title: None,
                    category_id: None,
                    tags: None,
                })?;
                Ok(())
            }
            AiSuggestionPayload::Tags { tags } => {
                let current = detail
                    .tags
                    .iter()
                    .map(|tag| tag.name.clone())
                    .collect::<Vec<_>>();
                let mut next = current.clone();
                let mut seen: HashSet<String> =
                    current.iter().map(|name| name.to_lowercase()).collect();
                for tag in tags {
                    if seen.insert(tag.tag.to_lowercase()) {
                        next.push(tag.tag.clone());
                    }
                }
                NoteService::new(Arc::clone(&self.ctx)).set_tags(&detail.note.id, &next)?;
                Ok(())
            }
            AiSuggestionPayload::Relations { relations } => {
                let mut wiki_links = Vec::new();
                for relation in relations {
                    if relation.relation_type.eq_ignore_ascii_case("WIKILINK") {
                        wiki_links.push(relation);
                        continue;
                    }
                    RelationService::new(Arc::clone(&self.ctx)).add(AddRelationInput {
                        source_id: detail.note.id.clone(),
                        target_id: relation.target_note_id.clone(),
                        relation_type: relation.relation_type.clone(),
                        note: Some(relation.reason.clone()),
                    })?;
                }
                if !wiki_links.is_empty() {
                    let notes = NoteService::new(Arc::clone(&self.ctx));
                    let current = notes.get(&detail.note.id)?;
                    let stored = self.ctx.storage.read_note(&current.note.file_path)?;
                    let mut additions = Vec::new();
                    for relation in wiki_links {
                        let link = format!(
                            "[[{}]]",
                            crate::core::text::escape_wikilink_target(&relation.target_title)
                        );
                        if stored.body.contains(&link) {
                            continue;
                        }
                        additions.push(format!("- {link} — {}", relation.reason));
                    }
                    if !additions.is_empty() {
                        notes.save(SaveNoteInput {
                            id: detail.note.id.clone(),
                            content: format!(
                                "{}\n\n## 相关笔记\n\n{}",
                                stored.body.trim_end(),
                                additions.join("\n")
                            ),
                            base_content_hash: Some(stored.content_hash.clone()),
                            title: None,
                            category_id: None,
                            tags: None,
                        })?;
                    }
                }
                Ok(())
            }
            // 重复建议用于比较与人工判断，接受本身不改文件。
            AiSuggestionPayload::Duplicate { .. } => Ok(()),
        }
    }

    fn evidence_for(
        &self,
        note_id: &str,
        title: &str,
        body: &str,
    ) -> Result<Vec<SuggestionEvidence>> {
        let chars: Vec<char> = body.chars().collect();
        Ok(chunk_markdown(body)
            .into_iter()
            .take(MAX_EVIDENCE)
            .map(|chunk| {
                let start = chunk.char_start.min(chars.len());
                let end = chunk.char_end.min(chars.len());
                SuggestionEvidence {
                    note_id: note_id.to_string(),
                    title: title.to_string(),
                    heading: chunk.heading,
                    snippet: truncate_chars(
                        chars[start..end].iter().collect::<String>().trim(),
                        MAX_EVIDENCE_CHARS,
                    ),
                    char_start: start as i64,
                    char_end: end as i64,
                }
            })
            .collect())
    }

    fn build_payload(
        &self,
        note_id: &str,
        title: &str,
        body: &str,
        kind: SuggestionKind,
        allow_vector_outbound: bool,
    ) -> Result<(AiSuggestionPayload, Vec<SuggestionEvidence>, bool)> {
        match kind {
            SuggestionKind::Summary => {
                let plain = markdown_to_plain_text(body)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let summary = truncate_chars(&first_sentence(&plain), 240);
                let bullets = chunk_markdown(body)
                    .into_iter()
                    .filter_map(|chunk| {
                        chunk
                            .heading
                            .split(" > ")
                            .last()
                            .map(str::trim)
                            .filter(|heading| !heading.is_empty())
                            .map(str::to_string)
                    })
                    .take(4)
                    .collect::<Vec<_>>();
                Ok((
                    AiSuggestionPayload::Summary { summary, bullets },
                    Vec::new(),
                    false,
                ))
            }
            SuggestionKind::Tags => {
                let existing = TagService::new(Arc::clone(&self.ctx)).list()?;
                let assigned: HashSet<String> = NoteService::new(Arc::clone(&self.ctx))
                    .get(note_id)?
                    .tags
                    .into_iter()
                    .map(|tag| tag.name.to_lowercase())
                    .collect();
                let mut tags: Vec<SuggestedTag> = Vec::new();
                let add = |name: &str,
                           reason: String,
                           existing_flag: bool,
                           tags: &mut Vec<SuggestedTag>| {
                    let tag = name.trim().trim_matches(|ch: char| {
                        ch.is_whitespace() || "#,，。；;:：()（）[]【】".contains(ch)
                    });
                    if tag.chars().count() < 2
                        || tag.chars().count() > 24
                        || assigned.contains(&tag.to_lowercase())
                        || tags.iter().any(|item| item.tag.eq_ignore_ascii_case(tag))
                    {
                        return;
                    }
                    tags.push(SuggestedTag {
                        tag: tag.to_string(),
                        reason,
                        existing: existing_flag,
                    });
                };
                let lower = format!("{}\n{}", title, body).to_lowercase();
                for tag in &existing {
                    if lower.contains(&tag.name.to_lowercase()) {
                        add(
                            &tag.name,
                            "正文直接提到，复用已有标签".to_string(),
                            true,
                            &mut tags,
                        );
                    }
                    if tags.len() >= MAX_TAG_SUGGESTIONS {
                        break;
                    }
                }
                for heading in chunk_markdown(body).into_iter().flat_map(|chunk| {
                    chunk
                        .heading
                        .split(" > ")
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                }) {
                    add(&heading, "来自标题层级".to_string(), false, &mut tags);
                    if tags.len() >= MAX_TAG_SUGGESTIONS {
                        break;
                    }
                }
                Ok((AiSuggestionPayload::Tags { tags }, Vec::new(), false))
            }
            SuggestionKind::Relations => {
                let title_bigrams = char_bigrams(title);
                let body_bigrams = char_bigrams(body);
                let notes = NoteService::new(Arc::clone(&self.ctx)).list(NoteListQuery {
                    page: Some(1),
                    page_size: Some(300),
                    ..Default::default()
                })?;
                let mut scored = Vec::new();
                for summary in notes.items.into_iter().filter(|note| note.id != note_id) {
                    let Ok(target) = NoteService::new(Arc::clone(&self.ctx)).get(&summary.id)
                    else {
                        continue;
                    };
                    let title_score = jaccard(&title_bigrams, &char_bigrams(&summary.title));
                    let body_score = jaccard(&body_bigrams, &char_bigrams(&target.content));
                    let score = 0.6 * title_score + 0.4 * body_score;
                    if score < 0.04 {
                        continue;
                    }
                    scored.push((score, summary, title_score, body_score));
                }
                scored.sort_by(|left, right| {
                    right
                        .0
                        .partial_cmp(&left.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let relations = scored
                    .into_iter()
                    .take(MAX_RELATION_SUGGESTIONS)
                    .enumerate()
                    .map(
                        |(index, (_, summary, title_score, body_score))| SuggestedRelation {
                            target_note_id: summary.id,
                            target_title: summary.title,
                            relation_type: if index < 2 { "RELATED_TO" } else { "WIKILINK" }
                                .to_string(),
                            reason: format!(
                                "标题相似度 {:.0}%，正文相似度 {:.0}%",
                                title_score * 100.0,
                                body_score * 100.0
                            ),
                        },
                    )
                    .collect();
                Ok((
                    AiSuggestionPayload::Relations { relations },
                    Vec::new(),
                    false,
                ))
            }
            SuggestionKind::Duplicate => {
                let current_chars = char_bigrams(body);
                let current_polarity = polarity(&format!("{title}\n{body}"));
                let (semantic_scores, vector_outbound_attempted) = if allow_vector_outbound {
                    if let Some(provider) = embedding_provider(&self.ctx) {
                        let hits = VectorIndexService::new(Arc::clone(&self.ctx))
                            .search(
                                provider.as_ref(),
                                &truncate_chars(body, MAX_INPUT_CHARS),
                                50,
                            )
                            .ok();
                        let scores = hits.map(|hits| {
                            let mut scores: HashMap<String, f32> = HashMap::new();
                            for hit in hits {
                                scores
                                    .entry(hit.note_id)
                                    .and_modify(|score| *score = score.max(hit.score))
                                    .or_insert(hit.score);
                            }
                            scores
                        });
                        (scores, true)
                    } else {
                        (None, false)
                    }
                } else {
                    (None, false)
                };
                let notes = NoteService::new(Arc::clone(&self.ctx)).list(NoteListQuery {
                    page: Some(1),
                    page_size: Some(300),
                    ..Default::default()
                })?;
                let mut candidates = Vec::new();
                for summary in notes.items.into_iter().filter(|note| note.id != note_id) {
                    let title_score = jaccard(&char_bigrams(title), &char_bigrams(&summary.title));
                    if title_score < 0.45 {
                        continue;
                    }
                    let Ok(candidate) = NoteService::new(Arc::clone(&self.ctx)).get(&summary.id)
                    else {
                        continue;
                    };
                    let candidate_polarity =
                        polarity(&format!("{}\n{}", candidate.note.title, candidate.content));
                    if (current_polarity > 0 && candidate_polarity < 0)
                        || (current_polarity < 0 && candidate_polarity > 0)
                    {
                        continue;
                    }
                    let body_score = jaccard(&current_chars, &char_bigrams(&candidate.content));
                    let vector_score = semantic_scores
                        .as_ref()
                        .and_then(|scores| scores.get(&summary.id).copied());
                    let votes = usize::from(title_score >= 0.45)
                        + usize::from(body_score >= 0.45)
                        + usize::from(vector_score.is_some_and(|score| score >= 0.72));
                    if votes < 2 {
                        continue;
                    }
                    let score = [Some(title_score), Some(body_score), vector_score]
                        .into_iter()
                        .flatten()
                        .fold(0.0f32, f32::max);
                    let mut reasons = vec![
                        format!("标题相似度 {:.0}%", title_score * 100.0),
                        format!("正文相似度 {:.0}%", body_score * 100.0),
                    ];
                    reasons.push(match vector_score {
                        Some(value) => format!("语义相似度 {:.0}%", value * 100.0),
                        None => "语义向量不可用（本项未投票）".to_string(),
                    });
                    candidates.push(DuplicateCandidate {
                        note_id: summary.id,
                        title: summary.title,
                        score,
                        reasons,
                    });
                }
                candidates.sort_by(|left, right| {
                    right
                        .score
                        .partial_cmp(&left.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                candidates.truncate(MAX_DUPLICATE_CANDIDATES);
                Ok((
                    AiSuggestionPayload::Duplicate { candidates },
                    Vec::new(),
                    vector_outbound_attempted,
                ))
            }
        }
    }

    pub fn create_job(&self, kind: SuggestionKind, note_ids: &[String]) -> Result<AiJob> {
        self.create_job_with_confirmation(kind, note_ids, false)
    }

    pub fn create_job_with_confirmation(
        &self,
        kind: SuggestionKind,
        note_ids: &[String],
        confirm_outbound: bool,
    ) -> Result<AiJob> {
        if note_ids.is_empty() {
            return Err(AppError::invalid_input("批量任务至少需要一篇笔记。"));
        }
        let mut unique = Vec::new();
        let mut seen = HashSet::new();
        for note_id in note_ids {
            if seen.insert(note_id.clone()) {
                unique.push(note_id.clone());
            }
        }
        let job = AiJob {
            id: ulid(),
            workspace_id: self.ctx.workspace.id.clone(),
            kind: format!("organize.{}", kind.as_str()),
            status: AiJobStatus::Queued,
            total: unique.len(),
            completed: 0,
            failed: 0,
            payload: json!({
                "suggestionKind": kind.as_str(),
                "confirmOutbound": confirm_outbound
            }),
            error: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            started_at: None,
            finished_at: None,
        };
        self.ctx
            .db
            .with_transaction(|tx| SqliteAiOrganizeRepository.create_job(tx, &job, &unique))?;
        self.get_job(&job.id)
    }

    pub fn get_job(&self, id: &str) -> Result<AiJob> {
        self.ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.get_job(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "批量任务不存在。"))
    }

    pub fn list_jobs(&self, limit: usize) -> Result<Vec<AiJob>> {
        self.ctx.db.with_conn(|conn| {
            SqliteAiOrganizeRepository.list_jobs(conn, &self.ctx.workspace.id, limit.clamp(1, 100))
        })
    }

    pub fn run_job(&self, id: &str) -> Result<AiJob> {
        let job = self.get_job(id)?;
        if !matches!(job.status, AiJobStatus::Queued | AiJobStatus::Running) {
            return Ok(job);
        }
        let kind = SuggestionKind::parse(job.kind.trim_start_matches("organize."))
            .ok_or_else(|| AppError::invalid_input("未知批量任务类型。"))?;
        let confirm_outbound = job
            .payload
            .get("confirmOutbound")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        self.ctx.db.with_conn(|conn| {
            SqliteAiOrganizeRepository.set_job_status(conn, id, AiJobStatus::Running, None)
        })?;
        loop {
            let current = self.get_job(id)?;
            if !matches!(current.status, AiJobStatus::Running) {
                return Ok(current);
            }
            let Some(item) = self
                .ctx
                .db
                .with_conn(|conn| SqliteAiOrganizeRepository.next_pending_item(conn, id))?
            else {
                break;
            };
            let result = self.generate_with_confirmation(&item.note_id, kind, confirm_outbound);
            let item_status = if result.is_ok() {
                AiJobItemStatus::Completed
            } else {
                AiJobItemStatus::Failed
            };
            let error = result.as_ref().err().map(|error| error.message.clone());
            self.ctx.db.with_conn(|conn| {
                SqliteAiOrganizeRepository.finish_job_item(
                    conn,
                    id,
                    item.item_index,
                    item_status,
                    error.as_deref(),
                )?;
                SqliteAiOrganizeRepository.refresh_job_counts(conn, id)
            })?;
            let progress_job = self.get_job(id)?;
            self.emit_progress(AiJobProgress {
                job_id: id.to_string(),
                status: progress_job.status,
                total: progress_job.total,
                completed: progress_job.completed,
                failed: progress_job.failed,
                current_note_id: Some(item.note_id),
            });
        }
        let current = self.get_job(id)?;
        if current.status == AiJobStatus::Running {
            let final_status = if current.completed == 0 && current.failed > 0 {
                AiJobStatus::Failed
            } else {
                AiJobStatus::Completed
            };
            self.ctx.db.with_conn(|conn| {
                SqliteAiOrganizeRepository.set_job_status(
                    conn,
                    id,
                    final_status,
                    (final_status == AiJobStatus::Failed).then_some("所有任务项均失败"),
                )
            })?;
        }
        self.get_job(id)
    }

    pub fn pause_job(&self, id: &str) -> Result<AiJob> {
        self.set_job_status_if_active(id, AiJobStatus::Paused)
    }

    pub fn cancel_job(&self, id: &str) -> Result<AiJob> {
        self.set_job_status_if_active(id, AiJobStatus::Canceled)
    }

    pub fn resume_job(&self, id: &str) -> Result<AiJob> {
        let current = self.get_job(id)?;
        if !matches!(current.status, AiJobStatus::Paused | AiJobStatus::Failed) {
            return Err(AppError::invalid_input("只有暂停或失败的任务可以继续。"));
        }
        self.ctx.db.with_conn(|conn| {
            SqliteAiOrganizeRepository.set_job_status(conn, id, AiJobStatus::Running, None)
        })?;
        self.run_job(id)
    }

    pub fn retry_job(&self, id: &str) -> Result<AiJob> {
        let current = self.get_job(id)?;
        if matches!(current.status, AiJobStatus::Running) {
            return Err(AppError::invalid_input("任务正在运行。"));
        }
        self.ctx.db.with_transaction(|tx| {
            SqliteAiOrganizeRepository.requeue_failed_items(tx, id)?;
            SqliteAiOrganizeRepository.set_job_status(tx, id, AiJobStatus::Queued, None)
        })?;
        self.run_job(id)
    }

    pub fn recover_interrupted(&self) -> Result<usize> {
        self.ctx.db.with_conn(|conn| {
            SqliteAiOrganizeRepository.recover_interrupted(conn, &self.ctx.workspace.id)
        })
    }

    fn set_job_status_if_active(&self, id: &str, status: AiJobStatus) -> Result<AiJob> {
        let current = self.get_job(id)?;
        if !matches!(
            current.status,
            AiJobStatus::Queued | AiJobStatus::Running | AiJobStatus::Paused
        ) {
            return Err(AppError::invalid_input("任务已经结束。"));
        }
        self.ctx
            .db
            .with_conn(|conn| SqliteAiOrganizeRepository.set_job_status(conn, id, status, None))?;
        self.get_job(id)
    }

    fn emit_progress(&self, progress: AiJobProgress) {
        if let Ok(value) = serde_json::to_value(progress) {
            self.ctx.events.emit(events::ORGANIZE_PROGRESS, value);
        }
    }
}

fn suggestion_view(row: SuggestionRow) -> Result<AiSuggestion> {
    let kind = SuggestionKind::parse(&row.kind)
        .ok_or_else(|| AppError::invalid_input("未知建议类型。"))?;
    let status = SuggestionStatus::parse(&row.status)
        .ok_or_else(|| AppError::invalid_input("未知建议状态。"))?;
    let evidence: Vec<SuggestionEvidence> = serde_json::from_value(row.evidence)?;
    let payload: AiSuggestionPayload = serde_json::from_value(row.payload)?;
    let undo_available = status == SuggestionStatus::Accepted && row.undone_at.is_none();
    Ok(AiSuggestion {
        id: row.id,
        note_id: row.note_id,
        note_title: row.note_title,
        kind,
        status,
        model: row.model,
        prompt_version: row.prompt_version,
        evidence,
        payload,
        source_hash: row.source_hash,
        created_at: row.created_at,
        updated_at: row.updated_at,
        applied_at: row.applied_at,
        undone_at: row.undone_at,
        error: row.error,
        undo_available,
    })
}

fn char_bigrams(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.chars().filter(|ch| ch.is_alphanumeric()).collect();
    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

fn jaccard(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    intersection / union
}
