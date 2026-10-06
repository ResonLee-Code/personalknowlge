//! 双向链接服务（`docs/PLAN_V1.1.md` M1-2）。
//!
//! 职责：
//! - 解析正文中的 `[[…]]` 与相对 Markdown 链接，写入 `note_links`（派生数据，可重建）；
//! - 解析目标：标题（大小写不敏感）→ 工作区相对路径 → 文件名 → 笔记 id；
//! - 与写入路径联动：保存 / 导入 / 外部修改后刷新出链；重命名自动重连；
//!   删除（进回收站）时解绑指向它的链接，恢复后重新绑定；
//! - 提供反链、出链、未解析链接、引用计数与全量重建。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::core::error::Result;
use crate::core::text::{
    count_words, markdown_to_plain_text, parse_markdown_links, parse_wikilinks,
    rewrite_wikilink_target,
};
use crate::domain::link::{
    Backlink, LinkCounts, LinkRebuildReport, LinkType, OutgoingLink, UnresolvedLink,
};
use crate::domain::note::{Note, NoteStatus};
use crate::infrastructure::frontmatter;
use crate::infrastructure::markdown_storage::parent_relative;
use crate::repositories::{
    LinkRepository, NoteLinkRow, NoteRepository, NoteTitleRow, SqliteLinkRepository,
    SqliteNoteRepository,
};

use super::context::WorkspaceContext;
use super::index_service::{file_stem_title, IndexService};

/// 一次解析出的链接（按出现顺序）。
struct PendingLink {
    kind: LinkType,
    target: String,
    anchor: Option<String>,
    context: String,
}

pub struct LinkService {
    ctx: Arc<WorkspaceContext>,
    repo: SqliteLinkRepository,
}

impl LinkService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx, repo: SqliteLinkRepository }
    }

    fn workspace_id(&self) -> &str {
        &self.ctx.workspace.id
    }

    /// 创建或恢复笔记后调用：刷新其出链，并把指向该标题的未解析链接绑定过来。
    pub fn on_note_available(&self, note: &Note, body: &str) -> Result<usize> {
        let count = self.refresh_note_links(note, body)?;
        self.ctx.db.with_conn(|conn| {
            self.repo.rebind_target(
                conn,
                &self.ctx.workspace.id,
                &note.title,
                &note.id,
            )
        })?;
        Ok(count)
    }

    /// 保存 / 导入 / 外部修改后刷新某篇笔记的出链。
    pub fn refresh_note_links(&self, note: &Note, body: &str) -> Result<usize> {
        let index = self.title_index()?;
        let rows = self.collect_rows(note, body, &index)?;
        let count = rows.len();
        self.ctx.db.with_transaction(|tx| {
            self.repo.delete_for_source(tx, &note.id)?;
            for row in &rows {
                self.repo.insert(tx, self.workspace_id(), &note.id, row)?;
            }
            Ok(())
        })?;
        Ok(count)
    }

    /// 笔记移入回收站或文件缺失：清空其出链，并解绑指向它的链接。
    pub fn on_note_unavailable(&self, note_id: &str) -> Result<()> {
        self.ctx.db.with_transaction(|tx| {
            self.repo.delete_for_source(tx, note_id)?;
            self.repo.unbind_target(tx, note_id)?;
            Ok(())
        })?;
        Ok(())
    }

    /// 重命名笔记：更新链接表，并改写来源笔记正文里的 `[[旧标题…]]`。
    pub fn on_note_renamed(
        &self,
        note_id: &str,
        old_title: &str,
        new_title: &str,
    ) -> Result<usize> {
        if old_title == new_title {
            return Ok(0);
        }
        // 1) 链接表：指向该笔记的记录改为新标题
        self.ctx
            .db
            .with_conn(|conn| self.repo.rename_target(conn, note_id, new_title))?;

        // 2) 来源笔记正文：[[旧标题…]] → [[新标题…]]
        let rows = self
            .ctx
            .db
            .with_conn(|conn| self.repo.list_for_target(conn, note_id))?;
        let mut rewritten = 0usize;
        let mut seen: Vec<String> = Vec::new();

        for row in rows {
            if row.source_note_id == note_id || seen.contains(&row.source_note_id) {
                continue;
            }
            seen.push(row.source_note_id.clone());

            let Some(source) = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.get(conn, &row.source_note_id)
            })? else {
                continue;
            };
            if source.status != NoteStatus::Active {
                continue;
            }
            let stored = match self.ctx.storage.read_note(&source.file_path) {
                Ok(stored) => stored,
                Err(error) => {
                    tracing::warn!(
                        target: "links",
                        "重写链接时读取 {} 失败：{}",
                        source.file_path,
                        error.message
                    );
                    continue;
                }
            };
            let (updated_body, count) = rewrite_wikilink_target(&stored.body, old_title, new_title);
            if count == 0 {
                continue;
            }
            let markdown = frontmatter::serialize(&stored.frontmatter, &updated_body);
            let hash = self.ctx.storage.write_atomic(&source.file_path, &markdown)?;
            rewritten += count;

            // 元数据 + 全文索引 + 链接同步
            self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.update_content_meta(
                    conn,
                    &source.id,
                    &crate::core::time::now_iso(),
                    &hash,
                    count_words(&markdown_to_plain_text(&updated_body)),
                    None,
                )
            })?;
            if let Ok(updated_note) = self.note(&source.id) {
                let _ = IndexService::new(Arc::clone(&self.ctx)).reindex_note(&updated_note);
                let _ = self.refresh_note_links(&updated_note, &updated_body);
            }
        }
        Ok(rewritten)
    }

    /// 全量重建（删除链接表后按正文重新解析）。
    pub fn rebuild(&self) -> Result<LinkRebuildReport> {
        let started = Instant::now();
        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, self.workspace_id(), NoteStatus::Active)
        })?;
        self.ctx
            .db
            .with_conn(|conn| self.repo.clear_workspace(conn, self.workspace_id()))?;

        let mut links = 0i64;
        let mut resolved = 0i64;
        for note in &notes {
            let body = match self.ctx.storage.read_note(&note.file_path) {
                Ok(stored) => stored.body,
                Err(error) => {
                    tracing::warn!(
                        target: "links",
                        "重建链接时跳过 {}：{}",
                        note.file_path,
                        error.message
                    );
                    continue;
                }
            };
            let count = self.refresh_note_links(note, &body)? as i64;
            links += count;
            let (_, _, unresolved) = self
                .ctx
                .db
                .with_conn(|conn| self.repo.counts(conn, &note.id))?;
            resolved += count - unresolved;
        }

        let report = LinkRebuildReport {
            scanned_notes: notes.len() as i64,
            links,
            resolved,
            unresolved: links - resolved,
            duration_ms: started.elapsed().as_millis() as u64,
        };
        tracing::info!(
            target: "links",
            notes = report.scanned_notes,
            links = report.links,
            unresolved = report.unresolved,
            elapsed_ms = report.duration_ms,
            "链接重建完成"
        );
        Ok(report)
    }

    /// 反向链接：谁引用了我。
    pub fn backlinks(&self, note_id: &str) -> Result<Vec<Backlink>> {
        let rows = self
            .ctx
            .db
            .with_conn(|conn| self.repo.list_for_target(conn, note_id))?;
        let mut backlinks = Vec::new();
        let mut seen: Vec<(String, String)> = Vec::new();

        for row in rows {
            if row.link_type == LinkType::Url {
                continue;
            }
            let key = (row.source_note_id.clone(), row.context.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);

            let Some(source) = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.get(conn, &row.source_note_id)
            })? else {
                continue;
            };
            if source.status != NoteStatus::Active {
                continue;
            }
            backlinks.push(Backlink {
                note_id: source.id.clone(),
                title: source.title.clone(),
                file_path: source.file_path.clone(),
                context: row.context.clone(),
                anchor: row.target_anchor.clone(),
                updated_at: source.updated_at.clone(),
            });
        }
        backlinks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(backlinks)
    }

    /// 出链：我引用了谁（含未解析与外链）。
    pub fn outgoing(&self, note_id: &str) -> Result<Vec<OutgoingLink>> {
        let rows = self
            .ctx
            .db
            .with_conn(|conn| self.repo.list_for_source(conn, note_id))?;
        Ok(rows
            .into_iter()
            .map(|row| OutgoingLink {
                target_exists: row.link_type == LinkType::Url || row.target_note_id.is_some(),
                target_title: row.target_title,
                target_note_id: row.target_note_id,
                anchor: row.target_anchor,
                link_type: row.link_type,
                context: row.context,
            })
            .collect())
    }

    /// 全库未解析链接。
    pub fn unresolved(&self) -> Result<Vec<UnresolvedLink>> {
        let rows = self
            .ctx
            .db
            .with_conn(|conn| self.repo.list_unresolved(conn, self.workspace_id()))?;
        let mut unresolved = Vec::new();
        for row in rows {
            let Some(source) = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.get(conn, &row.source_note_id)
            })? else {
                continue;
            };
            if source.status != NoteStatus::Active {
                continue;
            }
            unresolved.push(UnresolvedLink {
                source_note_id: source.id.clone(),
                source_title: source.title.clone(),
                target_title: row.target_title.clone(),
                context: row.context.clone(),
            });
        }
        Ok(unresolved)
    }

    /// 引用计数（反链 / 出链 / 未解析）。
    pub fn counts(&self, note_id: &str) -> Result<LinkCounts> {
        let (backlinks, outgoing, unresolved) = self
            .ctx
            .db
            .with_conn(|conn| self.repo.counts(conn, note_id))?;
        Ok(LinkCounts { backlinks, outgoing, unresolved })
    }

    // ------------------------------------------------------------ 内部实现

    fn note(&self, note_id: &str) -> Result<Note> {
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, note_id))?
            .ok_or_else(|| {
                crate::core::error::AppError::new(
                    crate::core::error::ErrorCode::NoteNotFound,
                    "笔记不存在。",
                )
            })
    }

    /// 把正文解析成链接行（目标已解析）。
    fn collect_rows(
        &self,
        note: &Note,
        body: &str,
        index: &TitleIndex,
    ) -> Result<Vec<NoteLinkRow>> {
        let source_directory = parent_relative(&note.file_path);
        let mut pending: Vec<(usize, PendingLink)> = Vec::new();

        for item in parse_wikilinks(body) {
            pending.push((
                item.offset,
                PendingLink {
                    kind: LinkType::Wikilink,
                    target: item.target,
                    anchor: item.anchor,
                    context: item.context,
                },
            ));
        }

        for item in parse_markdown_links(body) {
            let target = sanitize_markdown_target(&item.target);
            if target.is_empty() {
                continue;
            }
            if item.is_external {
                pending.push((
                    item.offset,
                    PendingLink {
                        kind: LinkType::Url,
                        target,
                        anchor: None,
                        context: item.context,
                    },
                ));
            } else if is_markdown_path(&target) {
                pending.push((
                    item.offset,
                    PendingLink {
                        kind: LinkType::Markdown,
                        target: resolve_relative_path(&source_directory, &target),
                        anchor: None,
                        context: item.context,
                    },
                ));
            }
        }

        pending.sort_by_key(|(offset, _)| *offset);

        Ok(pending
            .into_iter()
            .enumerate()
            .map(|(position, (_, link))| {
                let resolved = match link.kind {
                    LinkType::Url => None,
                    _ => resolve_target(index, note.category_id.as_deref(), link.kind, &link.target),
                };
                NoteLinkRow {
                    id: 0,
                    source_note_id: note.id.clone(),
                    target_note_id: resolved,
                    target_title: link.target,
                    target_anchor: link.anchor,
                    link_type: link.kind,
                    context: link.context,
                    position: position as i64,
                }
            })
            .collect())
    }

    /// 加载标题索引（用于把链接目标解析到具体笔记）。
    fn title_index(&self) -> Result<TitleIndex> {
        let rows = self
            .ctx
            .db
            .with_conn(|conn| self.repo.list_titles(conn, self.workspace_id()))?;
        Ok(TitleIndex::new(rows))
    }
}

/// 标题 / 路径 / 文件名索引。
struct TitleIndex {
    by_title: HashMap<String, Vec<NoteTitleRow>>,
    by_path: HashMap<String, String>,
    by_file_name: HashMap<String, Vec<String>>,
}

impl TitleIndex {
    fn new(rows: Vec<NoteTitleRow>) -> Self {
        let mut by_title: HashMap<String, Vec<NoteTitleRow>> = HashMap::new();
        let mut by_path = HashMap::new();
        let mut by_file_name: HashMap<String, Vec<String>> = HashMap::new();

        for row in rows {
            by_title
                .entry(normalize_key(&row.title))
                .or_default()
                .push(row.clone());
            by_path.insert(normalize_path(&row.file_path), row.id.clone());

            let file_name = row
                .file_path
                .rsplit('/')
                .next()
                .unwrap_or(&row.file_path)
                .to_string();
            by_file_name
                .entry(normalize_key(&file_name))
                .or_default()
                .push(row.id.clone());
            by_file_name
                .entry(normalize_key(&file_stem_title(&file_name)))
                .or_default()
                .push(row.id.clone());
            by_file_name
                .entry(normalize_key(&row.id))
                .or_default()
                .push(row.id.clone());
        }

        Self { by_title, by_path, by_file_name }
    }
}

/// 解析链接目标：标题 → 路径 → 文件名 → id。
fn resolve_target(
    index: &TitleIndex,
    source_category: Option<&str>,
    kind: LinkType,
    target: &str,
) -> Option<String> {
    if kind == LinkType::Markdown {
        if let Some(id) = index.by_path.get(&normalize_path(target)) {
            return Some(id.clone());
        }
    }

    if let Some(candidates) = index.by_title.get(&normalize_key(target)) {
        if let Some(id) = pick_candidate(candidates, source_category) {
            return Some(id);
        }
    }

    if let Some(id) = index.by_path.get(&normalize_path(target)) {
        return Some(id.clone());
    }

    index
        .by_file_name
        .get(&normalize_key(target))
        .and_then(|ids| ids.first().cloned())
        .or_else(|| {
            let stem = normalize_key(&file_stem_title(target));
            index
                .by_file_name
                .get(&stem)
                .and_then(|ids| ids.first().cloned())
        })
}

/// 同名冲突时：同分类优先，其次最近更新。
fn pick_candidate(candidates: &[NoteTitleRow], source_category: Option<&str>) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let same_category = candidates.iter().find(|row| {
        row.category_id.as_deref().is_some() && row.category_id.as_deref() == source_category
    });
    if let Some(row) = same_category {
        return Some(row.id.clone());
    }
    candidates
        .iter()
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
        .map(|row| row.id.clone())
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_path(value: &str) -> String {
    value.replace('\\', "/").trim_start_matches("./").to_lowercase()
}

fn is_markdown_path(target: &str) -> bool {
    let lower = target.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// 去掉 `[文字](路径 "标题")` 中的可选标题与尖括号。
fn sanitize_markdown_target(target: &str) -> String {
    let trimmed = target.trim();
    let trimmed = trimmed.trim_start_matches('<');
    let trimmed = trimmed.trim_end_matches('>');
    trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// 把链接里的相对路径按来源笔记所在目录展开。
fn resolve_relative_path(source_directory: &str, target: &str) -> String {
    let target = normalize_path(target);
    if target.starts_with('/') || !target.contains("../") && !target.starts_with("../") {
        return target;
    }
    let mut stack: Vec<&str> = if source_directory.is_empty() {
        Vec::new()
    } else {
        source_directory.split('/').collect()
    };
    for part in target.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    stack.join("/")
}
