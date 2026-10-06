//! 索引服务：维护 FTS5 索引、扫描工作区、处理外部文件变化
//! （设计文档 13.3 / 18.4 / 19）。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::hash::sha256_hex_str;
use crate::core::ids::note_id;
use crate::core::text::{count_words, markdown_to_plain_text};
use crate::core::time::now_iso;
use crate::domain::note::{Note, NoteStatus};
use crate::domain::search::{IndexState, IndexStatus};
use crate::infrastructure::fts::{SearchDocument, SearchIndex, SqliteSearchIndex};
use crate::infrastructure::watcher::{FileChangeBatch, FileChangeKind};
use crate::repositories::settings_repository::{
    INDEX_REBUILT_AT_KEY, INDEX_STATE_KEY,
};
use crate::repositories::tag_repository::tags_for_note;
use crate::repositories::{
    NoteRepository, SettingsRepository, SqliteNoteRepository, SqliteSettingsRepository,
};

use super::context::WorkspaceContext;
use super::link_service::LinkService;
use super::relation_service::RelationService;

/// 扫描结果。
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub added: i64,
    pub updated: i64,
    pub missing: i64,
    pub unchanged: i64,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
}

pub struct IndexService {
    ctx: Arc<WorkspaceContext>,
    index: SqliteSearchIndex,
}

impl IndexService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx, index: SqliteSearchIndex }
    }

    fn workspace_id(&self) -> &str {
        &self.ctx.workspace.id
    }

    /// 读取一篇笔记的标签名。
    fn tag_names(&self, note_id: &str) -> Result<Vec<String>> {
        let tags = self.ctx.db.with_conn(|conn| tags_for_note(conn, note_id))?;
        Ok(tags.into_iter().map(|tag| tag.name).collect())
    }

    /// 写入索引。
    pub fn index_document(
        &self,
        note_id: &str,
        title: &str,
        body: &str,
        tags: &[String],
        category_path: &str,
    ) -> Result<()> {
        let plain_text = markdown_to_plain_text(body);
        let document = SearchDocument::build(note_id, title, &plain_text, tags, category_path);
        self.ctx
            .db
            .with_conn(|conn| self.index.upsert(conn, &document))
            .map_err(|error| self.index_failure(error))
    }

    /// 按笔记当前状态重建索引条目。
    pub fn reindex_note(&self, note: &Note) -> Result<()> {
        let category_path = match &note.category_id {
            Some(category_id) => self.ctx.category_lookup()?.path_string(category_id),
            None => String::new(),
        };
        let tags = self.tag_names(&note.id)?;
        let body = self.read_body(&note.file_path).unwrap_or_default();
        self.index_document(&note.id, &note.title, &body, &tags, &category_path)?;
        self.mark_indexed(&note.id)?;
        Ok(())
    }

    pub fn remove_note(&self, note_id: &str) -> Result<()> {
        self.ctx
            .db
            .with_conn(|conn| self.index.remove(conn, note_id))
            .map_err(|error| self.index_failure(error))
    }

    fn read_body(&self, relative: &str) -> Result<String> {
        let stored = self.ctx.storage.read_note(relative)?;
        Ok(stored.body)
    }

    fn mark_indexed(&self, note_id: &str) -> Result<()> {
        let now = now_iso();
        self.ctx.db.with_conn(|conn| {
            conn.execute(
                "UPDATE notes SET last_indexed_at = ?2 WHERE id = ?1",
                rusqlite::params![note_id, now],
            )?;
            Ok(())
        })
    }

    fn index_failure(&self, error: AppError) -> AppError {
        self.ctx.set_index_error(Some(error.message.clone()));
        AppError::new(ErrorCode::SearchIndexError, error.message)
    }

    /// 标记索引落后于数据。
    pub fn mark_stale(&self, reason: Option<String>) -> Result<()> {
        let state = IndexState::Stale.as_str().to_string();
        self.ctx.db.with_conn(|conn| {
            SqliteSettingsRepository.set_raw(conn, INDEX_STATE_KEY, &state)
        })?;
        if let Some(reason) = &reason {
            tracing::warn!(target: "index", "索引标记为过期：{reason}");
        }
        self.emit_status();
        Ok(())
    }

    /// 当前索引状态。
    pub fn status(&self) -> Result<IndexStatus> {
        let indexed_notes = self
            .ctx
            .db
            .with_conn(|conn| self.index.count(conn))
            .unwrap_or(0);
        let total_notes = self
            .ctx
            .db
            .with_conn(|conn| {
                SqliteNoteRepository.count_by_status(conn, self.workspace_id(), NoteStatus::Active)
            })
            .unwrap_or(0);

        let state = if self.ctx.is_index_rebuilding() {
            IndexState::Rebuilding
        } else if let Some(error) = self.ctx.index_error() {
            if indexed_notes == 0 && total_notes > 0 {
                let _ = error;
                IndexState::Failed
            } else {
                IndexState::Stale
            }
        } else if indexed_notes < total_notes {
            IndexState::Stale
        } else {
            IndexState::Ready
        };

        Ok(IndexStatus {
            state,
            indexed_notes,
            total_notes,
            last_rebuilt_at: self.ctx.last_rebuilt_at(),
            last_error: self.ctx.index_error(),
        })
    }

    fn emit_status(&self) {
        if let Ok(status) = self.status() {
            if let Ok(payload) = serde_json::to_value(&status) {
                self.ctx
                    .events
                    .emit(event_names::INDEX_STATUS, payload);
            }
        }
    }

    /// 全量重建索引（设计文档 FR-S07）。
    pub fn rebuild(&self) -> Result<IndexStatus> {
        let started = Instant::now();
        self.ctx.set_index_rebuilding(true);
        self.emit_status();
        tracing::info!(target: "index", "开始重建索引");

        let result = (|| -> Result<i64> {
            self.ctx
                .db
                .with_conn(|conn| self.index.clear(conn))
                .map_err(|error| self.index_failure(error))?;

            let notes = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.list_by_status(conn, self.workspace_id(), NoteStatus::Active)
            })?;
            let lookup = self.ctx.category_lookup()?;
            let mut indexed = 0i64;

            for note in notes {
                let category_path = note
                    .category_id
                    .as_ref()
                    .map(|id| lookup.path_string(id))
                    .unwrap_or_default();
                let tags = self.tag_names(&note.id).unwrap_or_default();
                let body = self.read_body(&note.file_path).unwrap_or_default();
                let plain_text = markdown_to_plain_text(&body);
                let document =
                    SearchDocument::build(&note.id, &note.title, &plain_text, &tags, &category_path);
                self.ctx
                    .db
                    .with_conn(|conn| self.index.upsert(conn, &document))
                    .map_err(|error| self.index_failure(error))?;
                self.mark_indexed(&note.id)?;
                indexed += 1;
            }

            self.ctx
                .db
                .with_conn(|conn| self.index.optimize(conn))
                .map_err(|error| self.index_failure(error))?;
            Ok(indexed)
        })();

        self.ctx.set_index_rebuilding(false);

        match result {
            Ok(indexed) => {
                let now = now_iso();
                self.ctx.db.with_conn(|conn| {
                    SqliteSettingsRepository.set_raw(conn, INDEX_STATE_KEY, IndexState::Ready.as_str())?;
                    SqliteSettingsRepository.set_raw(conn, INDEX_REBUILT_AT_KEY, &now)
                })?;
                self.ctx.set_index_error(None);
                tracing::info!(
                    target: "index",
                    indexed,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "索引重建完成"
                );
                self.emit_status();
                self.status()
            }
            Err(error) => {
                self.ctx.set_index_error(Some(error.message.clone()));
                let _ = self.mark_stale(Some(error.message.clone()));
                self.emit_status();
                Err(error)
            }
        }
    }

    /// 扫描工作区：采纳新增文件、同步外部修改、标记缺失文件
    /// （设计文档 FR-W02 / 13.3）。
    pub fn scan_workspace(&self) -> Result<ScanReport> {
        self.scan_scope(None)
    }

    /// 只扫描某个目录（用于事件粒度只到目录的情况）。
    pub fn scan_directory(&self, directory: &str) -> Result<ScanReport> {
        self.scan_scope(Some(directory))
    }

    fn scan_scope(&self, scope: Option<&str>) -> Result<ScanReport> {
        let started = Instant::now();
        let mut report = ScanReport::default();
        let prefix = scope
            .map(|directory| format!("{}/", directory.trim_matches('/')))
            .filter(|prefix| prefix != "/");
        let in_scope = |relative: &str| match &prefix {
            Some(prefix) => relative.starts_with(prefix.as_str()),
            None => true,
        };

        let files: Vec<String> = self
            .ctx
            .storage
            .list_markdown_files()?
            .into_iter()
            .filter(|relative| in_scope(relative))
            .collect();
        let disk: HashSet<String> = files.iter().cloned().collect();
        let lookup = self.ctx.category_lookup()?;
        let existing = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, self.workspace_id(), NoteStatus::Active)
        })?;
        let mut by_path: HashMap<String, Note> = existing
            .into_iter()
            .map(|note| (note.file_path.clone(), note))
            .collect();

        for relative in files {
            match by_path.remove(&relative) {
                Some(note) => {
                    if self.sync_note_from_disk(&note, &mut report)? {
                        report.updated += 1;
                    } else {
                        report.unchanged += 1;
                    }
                }
                None => {
                    self.adopt_file(&relative, &lookup, &mut report)?;
                }
            }
        }

        // 剩余记录对应的文件已消失
        for note in by_path.values() {
            if disk.contains(&note.file_path) {
                continue;
            }
            // 快照可能已过期（并发写入/移动），落库前再做一次新鲜检查
            if self.ctx.storage.exists(&note.file_path) {
                continue;
            }
            self.mark_missing(&note.id)?;
            report.missing += 1;
        }

        report.duration_ms = started.elapsed().as_millis() as u64;
        if report.added > 0 || report.updated > 0 || report.missing > 0 {
            self.ctx.events.emit(
                event_names::NOTES_CHANGED,
                serde_json::json!({
                    "added": report.added,
                    "updated": report.updated,
                    "missing": report.missing,
                }),
            );
        }
        Ok(report)
    }

    /// 处理文件监听器上报的批量变化。
    pub fn apply_file_changes(&self, batch: FileChangeBatch) -> Result<ScanReport> {
        let started = Instant::now();
        let mut report = ScanReport::default();
        let lookup = self.ctx.category_lookup()?;

        for change in batch.changes {
            let relative = change.path.clone();
            let existing = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.get_by_path(conn, self.workspace_id(), &relative)
            })?;

            match change.kind {
                FileChangeKind::Removed => {
                    if let Some(note) = existing {
                        if note.status != NoteStatus::Deleted {
                            self.mark_missing(&note.id)?;
                            report.missing += 1;
                        }
                    }
                }
                FileChangeKind::Created | FileChangeKind::Modified => {
                    if !self.ctx.storage.exists(&relative) {
                        continue;
                    }
                    let stored = match self.ctx.storage.read_note(&relative) {
                        Ok(stored) => stored,
                        Err(error) => {
                            report
                                .warnings
                                .push(format!("读取 {relative} 失败：{}", error.message));
                            continue;
                        }
                    };
                    // 忽略应用自身写入触发的回声事件
                    if self
                        .ctx
                        .storage
                        .recent_writes()
                        .is_self_write(&relative, &stored.content_hash)
                    {
                        report.unchanged += 1;
                        continue;
                    }
                    match existing {
                        Some(note) => {
                            if note.content_hash.as_deref() == Some(stored.content_hash.as_str())
                                && note.status == NoteStatus::Active
                            {
                                report.unchanged += 1;
                                continue;
                            }
                            self.apply_external_edit(&note, &stored, &mut report)?;
                            report.updated += 1;
                        }
                        None => {
                            self.adopt_file(&relative, &lookup, &mut report)?;
                        }
                    }
                }
            }
        }

        if batch.attachments_changed {
            report.warnings.push("附件目录发生变化。".to_string());
        }

        // 事件只精确到目录时，按目录重新比对（macOS FSEvents 常见）
        for directory in &batch.directories {
            match self.scan_directory(directory) {
                Ok(scan) => {
                    report.added += scan.added;
                    report.updated += scan.updated;
                    report.missing += scan.missing;
                    report.unchanged += scan.unchanged;
                    report.warnings.extend(scan.warnings);
                }
                Err(error) => report.warnings.push(format!(
                    "重新扫描目录 {directory} 失败：{}",
                    error.message
                )),
            }
        }

        report.duration_ms = started.elapsed().as_millis() as u64;
        if report.added > 0 || report.updated > 0 || report.missing > 0 {
            if let Ok(payload) = serde_json::to_value(&report) {
                self.ctx.events.emit(event_names::FILES_CHANGED, payload);
            }
        }
        Ok(report)
    }

    /// 采纳磁盘上的新文件。
    fn adopt_file(
        &self,
        relative: &str,
        lookup: &super::lookup::CategoryLookup,
        report: &mut ScanReport,
    ) -> Result<()> {
        let stored = match self.ctx.storage.read_note(relative) {
            Ok(stored) => stored,
            Err(error) => {
                report
                    .warnings
                    .push(format!("跳过无法读取的文件 {relative}：{}", error.message));
                return Ok(());
            }
        };
        if let Some(error) = &stored.parse_error {
            report
                .warnings
                .push(format!("{relative}：{error}"));
        }

        let now = now_iso();
        let title = stored
            .frontmatter
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| file_stem_title(relative));

        // 复用 frontmatter 中的 id；若冲突则重新生成，避免覆盖已有笔记
        let id = match stored.frontmatter.id.clone() {
            Some(id) if !id.trim().is_empty() && !self.id_exists(&id)? => id,
            _ => note_id(),
        };

        let category_id = directory_category_id(lookup, relative);
        let created_at = stored
            .frontmatter
            .created_at
            .clone()
            .or_else(|| stored.modified_at.clone())
            .unwrap_or_else(|| now.clone());
        let updated_at = stored
            .frontmatter
            .updated_at
            .clone()
            .or_else(|| stored.modified_at.clone())
            .unwrap_or_else(|| now.clone());

        let note = Note {
            id: id.clone(),
            workspace_id: self.workspace_id().to_string(),
            title: title.clone(),
            file_path: relative.to_string(),
            category_id: category_id.clone(),
            status: NoteStatus::Active,
            content_hash: Some(stored.content_hash.clone()),
            word_count: count_words(&markdown_to_plain_text(&stored.body)),
            favorite: false,
            created_at,
            updated_at,
            last_indexed_at: None,
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
        };
        // 幂等采纳：文件监听回调与显式扫描可能并发（两者都是「先读磁盘快照、再写库」），
        // 因此插入前在同一事务内再确认一次路径是否已被别的扫描采纳；已存在则直接跳过，
        // 而不是抛出 UNIQUE(workspace_id, file_path) 让整轮扫描中止。
        let adopted = self.ctx.db.with_transaction(|tx| {
            if SqliteNoteRepository.get_by_path(tx, self.workspace_id(), relative)?.is_some() {
                return Ok(false);
            }
            SqliteNoteRepository.insert(tx, &note)?;
            Ok(true)
        })?;
        if !adopted {
            tracing::debug!(target: "index", "路径已被其它扫描采纳，跳过：{relative}");
            report.unchanged += 1;
            return Ok(());
        }

        for tag in &stored.frontmatter.tags {
            let _ = super::tag_service::TagService::new(Arc::clone(&self.ctx))
                .ensure_tag(&id, tag);
        }
        if let Err(error) = RelationService::new(Arc::clone(&self.ctx)).refresh_note(&note) {
            tracing::debug!(target: "relations", "采纳文件时刷新关系失败：{}", error.message);
        }

        let category_path = category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let tags = self.tag_names(&id).unwrap_or_default();
        if let Err(error) =
            self.index_document(&id, &title, &stored.body, &tags, &category_path)
        {
            report.warnings.push(format!("{relative} 建立索引失败：{}", error.message));
            self.mark_stale(Some(error.message.clone()))?;
        } else {
            self.mark_indexed(&id)?;
        }
        report.added += 1;
        // 采纳外部文件后，指向该标题的未解析链接应自动连接
        if let Err(error) = LinkService::new(Arc::clone(&self.ctx)).on_note_available(&note, &stored.body)
        {
            report
                .warnings
                .push(format!("{relative} 建立链接失败：{}", error.message));
        }
        Ok(())
    }

    /// 同步已知笔记的内容变化（工作区打开时使用）。
    fn sync_note_from_disk(&self, note: &Note, report: &mut ScanReport) -> Result<bool> {
        let stored = match self.ctx.storage.read_note(&note.file_path) {
            Ok(stored) => stored,
            Err(error) => {
                report
                    .warnings
                    .push(format!("读取 {} 失败：{}", note.file_path, error.message));
                return Ok(false);
            }
        };
        if note.content_hash.as_deref() == Some(stored.content_hash.as_str()) {
            return Ok(false);
        }
        if self
            .ctx
            .storage
            .recent_writes()
            .is_self_write(&note.file_path, &stored.content_hash)
        {
            return Ok(false);
        }
        self.apply_external_edit(note, &stored, report)?;
        Ok(true)
    }

    /// 应用外部编辑：更新元数据与索引，不覆盖用户文件。
    fn apply_external_edit(
        &self,
        note: &Note,
        stored: &crate::infrastructure::markdown_storage::StoredNote,
        report: &mut ScanReport,
    ) -> Result<()> {
        let title = stored
            .frontmatter
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| note.title.clone());
        let updated_at = stored
            .frontmatter
            .updated_at
            .clone()
            .or_else(|| stored.modified_at.clone())
            .unwrap_or_else(now_iso);
        let word_count = count_words(&markdown_to_plain_text(&stored.body));

        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.update_identity(
                conn,
                &note.id,
                &title,
                &note.file_path,
                note.category_id.as_deref(),
                &updated_at,
            )?;
            SqliteNoteRepository.update_content_meta(
                conn,
                &note.id,
                &updated_at,
                &stored.content_hash,
                word_count,
                None,
            )?;
            if note.status == NoteStatus::Missing {
                SqliteNoteRepository.set_status(conn, &note.id, NoteStatus::Active, None)?;
            }
            // 来源与时效也以文件为准：外部编辑器改了 frontmatter 后同步到库里
            SqliteNoteRepository.set_source(
                conn,
                &note.id,
                stored.frontmatter.source_type.as_deref(),
                stored.frontmatter.source_ref.as_deref(),
                stored.frontmatter.source_reliability,
                stored.frontmatter.valid_until.as_deref(),
            )?;
            Ok(())
        })?;

        for tag in &stored.frontmatter.tags {
            let _ = super::tag_service::TagService::new(Arc::clone(&self.ctx))
                .ensure_tag(&note.id, tag);
        }

        let category_path = match &note.category_id {
            Some(category_id) => self.ctx.category_lookup()?.path_string(category_id),
            None => String::new(),
        };
        let tags = self.tag_names(&note.id).unwrap_or_default();
        if let Err(error) = RelationService::new(Arc::clone(&self.ctx)).refresh_note(note) {
            tracing::warn!(target: "relations", "同步外部修改的关系失败：{}", error.message);
        }
        if let Err(error) = self.index_document(&note.id, &title, &stored.body, &tags, &category_path)
        {
            report
                .warnings
                .push(format!("{} 更新索引失败：{}", note.file_path, error.message));
            self.mark_stale(Some(error.message.clone()))?;
        } else {
            self.mark_indexed(&note.id)?;
        }
        if let Err(error) =
            LinkService::new(Arc::clone(&self.ctx)).refresh_note_links(note, &stored.body)
        {
            report
                .warnings
                .push(format!("{} 更新链接失败：{}", note.file_path, error.message));
        }
        Ok(())
    }

    fn mark_missing(&self, note_id: &str) -> Result<()> {
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.set_status(conn, note_id, NoteStatus::Missing, None)
        })?;
        self.remove_note(note_id)?;
        let _ = LinkService::new(Arc::clone(&self.ctx)).on_note_unavailable(note_id);
        Ok(())
    }

    fn id_exists(&self, note_id: &str) -> Result<bool> {
        Ok(self
            .ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, note_id))?
            .is_some())
    }
}

/// 从文件路径推断标题：去掉已知文本扩展名与 `YYYYMMDD-HHmmss-` 前缀。
pub fn file_stem_title(path: &str) -> String {
    use std::sync::OnceLock;

    static TIMESTAMP_PREFIX: OnceLock<regex::Regex> = OnceLock::new();
    const TEXT_EXTENSIONS: [&str; 5] = ["md", "markdown", "mdown", "txt", "text"];

    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);

    // 只去掉已知的文本扩展名，避免误伤标题中的点号
    let stem = match name.rsplit_once('.') {
        Some((base, extension))
            if !base.is_empty()
                && TEXT_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) =>
        {
            base
        }
        _ => name,
    };

    let stamp = TIMESTAMP_PREFIX
        .get_or_init(|| regex::Regex::new(r"^\d{8}-\d{6}-").expect("时间戳前缀正则必须合法"));
    let cleaned = stamp.replace(stem, "");
    let title = cleaned.trim();

    if title.is_empty() {
        stem.to_string()
    } else {
        title.to_string()
    }
}

/// 文件所在目录对应的分类。
fn directory_category_id(
    lookup: &super::lookup::CategoryLookup,
    relative: &str,
) -> Option<String> {
    let directory = match relative.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => return None,
    };
    if directory == crate::core::paths::INBOX_DIR {
        return None;
    }
    lookup.category_by_directory(directory)
}

/// 内容哈希变化检测的辅助（供测试使用）。
pub fn content_changed(stored_hash: Option<&str>, current: &str) -> bool {
    match stored_hash {
        Some(hash) => hash != sha256_hex_str(current),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::file_stem_title;

    #[test]
    fn strips_timestamp_prefix_and_text_extension() {
        assert_eq!(
            file_stem_title("00_Inbox/20260924-010009-欢迎使用-Personal-Knowledge-OS.md"),
            "欢迎使用-Personal-Knowledge-OS"
        );
        assert_eq!(file_stem_title("20260901-100000-纯文本文档.txt"), "纯文本文档");
        assert_eq!(file_stem_title("20260901-100000-笔记.markdown"), "笔记");
    }

    #[test]
    fn keeps_meaningful_dots_in_names() {
        // 只去掉已知的文本扩展名，文件名中的其它点号保留
        assert_eq!(file_stem_title("2026.09.23 会议记录.md"), "2026.09.23 会议记录");
        assert_eq!(file_stem_title("v1.2 设计稿"), "v1.2 设计稿");
    }

    #[test]
    fn handles_paths_without_extension_or_title() {
        assert_eq!(file_stem_title("note"), "note");
        assert_eq!(file_stem_title("/tmp/20260901-100000-meeting.md"), "meeting");
        // 前缀后为空时回退为去掉扩展名的原文件名
        assert_eq!(file_stem_title("20260901-100000-.md"), "20260901-100000-");
    }
}
