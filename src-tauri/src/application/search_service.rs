//! 搜索服务（设计文档 18）。
//!
//! 流程：关键词 -> 安全 MATCH 表达式 -> FTS5 检索 -> 过滤 -> 排序
//! -> 生成摘要与高亮 -> 返回结果。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::core::error::Result;
use crate::core::text::{build_fts_match, build_snippet, highlight_terms, markdown_to_plain_text};
use crate::domain::note::{Note, NoteListQuery, NoteSummary};
use crate::domain::search::{IndexStatus, SearchHit, SearchQuery, SearchResponse, SearchSort};
use crate::infrastructure::fts::{FtsRequest, SearchIndex, SqliteSearchIndex};
use crate::repositories::{NoteRepository, SqliteNoteRepository};

use super::context::WorkspaceContext;
use super::index_service::IndexService;
use super::note_service::NoteService;

const SNIPPET_CHARS: usize = 180;

pub struct SearchService {
    ctx: Arc<WorkspaceContext>,
    index: SqliteSearchIndex,
}

impl SearchService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx, index: SqliteSearchIndex }
    }

    /// 执行搜索。
    pub fn search(&self, query: SearchQuery) -> Result<SearchResponse> {
        let started = Instant::now();
        let keyword = query.keyword.trim().to_string();
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
        let offset = (page - 1) * page_size;
        let sort = query.sort.unwrap_or_default();
        let status = self.status()?;
        let index_stale = status.state != crate::domain::search::IndexState::Ready;

        let Some(match_expression) = build_fts_match(&keyword) else {
            // 空关键词：回退为「按时间浏览」
            let list_query = NoteListQuery {
                category_id: query.category_id.clone(),
                tag_id: query.tag_id.clone(),
                favorite_only: query.favorite_only,
                page: Some(page),
                page_size: Some(page_size),
                ..Default::default()
            };
            let result = NoteService::new(Arc::clone(&self.ctx)).list(list_query)?;
            let hits = result
                .items
                .into_iter()
                .map(|note| SearchHit {
                    note,
                    score: 0.0,
                    title_matched: false,
                    snippet: Vec::new(),
                    matched_terms: Vec::new(),
                })
                .collect();
            return Ok(SearchResponse {
                keyword,
                hits,
                total: result.total,
                page,
                page_size,
                took_ms: started.elapsed().as_millis() as u64,
                index_stale,
            });
        };

        let request = FtsRequest {
            match_expression: &match_expression,
            category_id: query.category_id.as_deref(),
            tag_id: query.tag_id.as_deref(),
            favorite_only: query.favorite_only,
            updated_after: query.updated_after.as_deref(),
            updated_before: query.updated_before.as_deref(),
            scope_note_id: query.scope_note_id.as_deref(),
            limit: page_size,
            offset,
            by_relevance: sort == SearchSort::Relevance,
            sort_by_updated_desc: sort != SearchSort::UpdatedDesc,
        };

        let (fts_hits, total) = self.ctx.db.with_conn(|conn| {
            let hits = self.index.search(conn, &request)?;
            let total = self.index.count_matches(conn, &request)?;
            Ok((hits, total))
        })?;

        if fts_hits.is_empty() {
            return Ok(SearchResponse {
                keyword,
                hits: Vec::new(),
                total,
                page,
                page_size,
                took_ms: started.elapsed().as_millis() as u64,
                index_stale,
            });
        }

        let ids: Vec<String> = fts_hits.iter().map(|hit| hit.note_id.clone()).collect();
        let notes = self.ctx.db.with_conn(|conn| notes_by_ids(conn, &ids))?;
        let mut by_id: HashMap<String, Note> =
            notes.into_iter().map(|note| (note.id.clone(), note)).collect();
        let summaries = self.ctx.to_summaries(
            ids.iter()
                .filter_map(|id| by_id.remove(id))
                .collect::<Vec<Note>>(),
        )?;
        let mut summary_by_id: HashMap<String, NoteSummary> =
            summaries.into_iter().map(|summary| (summary.id.clone(), summary)).collect();

        let terms = highlight_terms(&keyword);
        let mut hits: Vec<SearchHit> = Vec::with_capacity(fts_hits.len());
        for fts_hit in fts_hits {
            let Some(summary) = summary_by_id.remove(&fts_hit.note_id) else {
                continue;
            };
            let title_matched = matches_any(&summary.title, &terms);
            let snippet = self
                .snippet_for(&summary, &keyword)
                .unwrap_or_default();
            hits.push(SearchHit {
                note: summary,
                score: fts_hit.score,
                title_matched,
                snippet,
                matched_terms: terms.clone(),
            });
        }

        // 标题命中优先，其次按相关度（设计文档 18.2）
        hits.sort_by(|a, b| {
            b.title_matched
                .cmp(&a.title_matched)
                .then(a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
                .then(b.note.updated_at.cmp(&a.note.updated_at))
        });
        if sort == SearchSort::TitleAsc {
            hits.sort_by(|a, b| a.note.title.cmp(&b.note.title));
        }

        Ok(SearchResponse {
            keyword,
            hits,
            total,
            page,
            page_size,
            took_ms: started.elapsed().as_millis() as u64,
            index_stale,
        })
    }

    /// 快速搜索，用于全局搜索面板预览。
    pub fn quick_search(&self, keyword: &str, limit: i64) -> Result<Vec<NoteSummary>> {
        let response = self.search(SearchQuery {
            keyword: keyword.to_string(),
            page: Some(1),
            page_size: Some(limit.clamp(1, 30)),
            ..Default::default()
        })?;
        Ok(response.hits.into_iter().map(|hit| hit.note).collect())
    }

    pub fn status(&self) -> Result<IndexStatus> {
        IndexService::new(Arc::clone(&self.ctx)).status()
    }

    pub fn rebuild_index(&self) -> Result<IndexStatus> {
        IndexService::new(Arc::clone(&self.ctx)).rebuild()
    }

    /// 生成摘要：优先使用文件正文，文件不可读时退化为空摘要。
    fn snippet_for(
        &self,
        summary: &NoteSummary,
        keyword: &str,
    ) -> Option<Vec<crate::core::text::TextSegment>> {
        let stored = self.ctx.storage.read_note(&summary.file_path).ok()?;
        let plain = markdown_to_plain_text(&stored.body);
        Some(build_snippet(&plain, keyword, SNIPPET_CHARS))
    }
}

fn matches_any(text: &str, terms: &[String]) -> bool {
    let lower = text.to_lowercase();
    terms.iter().any(|term| lower.contains(&term.to_lowercase()))
}

/// 批量读取笔记（保持传入顺序由调用方处理）。
fn notes_by_ids(conn: &rusqlite::Connection, ids: &[String]) -> Result<Vec<Note>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT id, workspace_id, title, file_path, category_id, status, content_hash,
                word_count, favorite, created_at, updated_at, last_indexed_at,
                source_type, source_ref, source_reliability, valid_until, file_size, file_mtime,
                parent_id, declared_parent, sort_order, pinned
         FROM notes WHERE id IN ({placeholders})"
    );
    let values: Vec<rusqlite::types::Value> =
        ids.iter().cloned().map(rusqlite::types::Value::Text).collect();
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        let status: String = row.get("status")?;
        Ok(Note {
            id: row.get("id")?,
            workspace_id: row.get("workspace_id")?,
            title: row.get("title")?,
            file_path: row.get("file_path")?,
            category_id: row.get("category_id")?,
            status: crate::domain::note::NoteStatus::from_str(&status),
            content_hash: row.get("content_hash")?,
            word_count: row.get("word_count")?,
            favorite: row.get::<_, i64>("favorite")? != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            last_indexed_at: row.get("last_indexed_at")?,
            source_type: row.get("source_type")?,
            source_ref: row.get("source_ref")?,
            source_reliability: row.get("source_reliability")?,
            valid_until: row.get("valid_until")?,
            file_size: row.get("file_size")?,
            file_mtime: row.get("file_mtime")?,
            parent_id: row.get("parent_id")?,
            declared_parent: row.get("declared_parent")?,
            sort_order: row.get("sort_order")?,
            pinned: row.get::<_, i64>("pinned")? != 0,
        })
    })?;
    let mut notes = Vec::new();
    for row in rows {
        notes.push(row?);
    }
    Ok(notes)
}

/// 统计索引进度（供前端显示）。
pub fn progress_label(status: &IndexStatus) -> String {
    format!("{}/{}", status.indexed_notes, status.total_notes)
}

/// 供命令层使用：按 ID 读取笔记（含缺失文件）。
pub fn note_ids_of(notes: &[Note]) -> Vec<String> {
    notes.iter().map(|note| note.id.clone()).collect()
}

/// 占位，确保 `SqliteNoteRepository` 被引用（列表查询经过服务层）。
#[allow(dead_code)]
fn _assert_repo_used() {
    fn assert_impl<T: NoteRepository>() {}
    assert_impl::<SqliteNoteRepository>();
}
