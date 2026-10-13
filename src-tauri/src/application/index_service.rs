//! 索引服务：维护 FTS5 索引、扫描工作区、处理外部文件变化
//! （设计文档 13.3 / 18.4 / 19）。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::paths::TRASH_DIR;
use crate::core::hash::sha256_hex_str;
use crate::core::ids::note_id;
use crate::core::chunk::chunk_markdown;
use crate::core::text::{count_words, markdown_to_plain_text, tag_key};
use crate::core::time::now_iso;
use crate::domain::note::{Note, NoteStatus};
use crate::domain::tag::Tag;
use crate::domain::search::{IndexState, IndexStatus};
use crate::infrastructure::fts::{SearchDocument, SearchIndex, SqliteSearchIndex};
use crate::infrastructure::watcher::{FileChangeBatch, FileChangeKind};
use crate::repositories::settings_repository::{
    INDEX_REBUILT_AT_KEY, INDEX_STATE_KEY,
};
use crate::repositories::tag_repository::{tags_for_note, SqliteTagRepository, TagRepository};
use crate::repositories::chunk_repository::{
    ChunkRepository, SqliteChunkRepository,
};
use crate::repositories::{
    trash_path_of, NoteRepository, SettingsRepository, SqliteNoteRepository,
    SqliteSettingsRepository,
};

use super::context::WorkspaceContext;
use super::link_service::LinkService;
use super::relation_service::RelationService;
use super::note_service::NoteService;

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
        // 分块索引（N5）：与 FTS 同一事务写入，保证「命中能定位」与「能搜到」同步生效
        let chunks = chunk_markdown(body);
        self.ctx
            .db
            .with_conn(|conn| {
                self.index.upsert(conn, &document)?;
                SqliteChunkRepository.replace(conn, note_id, &chunks)
            })
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
            .with_conn(|conn| {
                self.index.remove(conn, note_id)?;
                SqliteChunkRepository.delete(conn, note_id)
            })
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
        // 分块索引（N5）是索引的一部分：**有笔记却没分块**说明这批派生数据缺了
        // （典型场景：从更早的版本升级上来），
        // 这时同样报「落后」，让界面提示重建、并让打开工作区时的自动重建把它补上。
        let chunk_rows = self
            .ctx
            .db
            .with_conn(|conn| SqliteChunkRepository.count(conn))
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
        } else if indexed_notes < total_notes || (indexed_notes > 0 && chunk_rows == 0) {
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

    /// 分块索引是否缺失（升级上来、或有人手工删过派生库）。
    ///
    /// 用来决定**要不要在打开工作区时自动重建**——只补 FTS 而不补分块的话，
    /// 检索是能用的，但"跳回原文精确区间"会静默失效。
    pub fn chunks_missing(&self) -> bool {
        let chunks = self
            .ctx
            .db
            .with_conn(|conn| SqliteChunkRepository.count(conn))
            .unwrap_or(0);
        if chunks > 0 {
            return false;
        }
        let total_notes = self
            .ctx
            .db
            .with_conn(|conn| {
                SqliteNoteRepository.count_by_status(conn, self.workspace_id(), NoteStatus::Active)
            })
            .unwrap_or(0);
        total_notes > 0
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
                .with_conn(|conn| {
                    self.index.clear(conn)?;
                    // 分块与 FTS 同生共死：重建时一起清空，再从文件重算
                    SqliteChunkRepository.clear(conn)
                })
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
                let chunks = chunk_markdown(&body);
                self.ctx
                    .db
                    .with_conn(|conn| {
                        self.index.upsert(conn, &document)?;
                        SqliteChunkRepository.replace(conn, &note.id, &chunks)
                    })
                    .map_err(|error| self.index_failure(error))?;
                self.mark_indexed(&note.id)?;
                indexed += 1;
            }

            self.ctx
                .db
                .with_conn(|conn| self.index.optimize(conn))
                .map_err(|error| self.index_failure(error))?;
            // 重建索引 ⇒ 派生数据整体刷新：决策缓存也一起清掉（键虽内容寻址，
            // 但清掉能避免历史条目长期堆积；下次判定重算一遍即可）
            let _ = self
                .ctx
                .db
                .with_conn(|conn| crate::application::ai::decision_cache::DecisionCache.clear(conn));
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

    /// 批处理结束后统一重建派生数据（仅大批量路径需要）。
    fn rebuild_derived_after_bulk(&self) {
        if let Err(error) = LinkService::new(Arc::clone(&self.ctx)).rebuild() {
            tracing::warn!(target: "links", "批处理后重建链接失败：{}", error.message);
        }
        if let Err(error) = RelationService::new(Arc::clone(&self.ctx)).rebuild() {
            tracing::warn!(target: "relations", "批处理后重建关系失败：{}", error.message);
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
        // 候选里必须包含**已标记缺失**的笔记：文件被移走又放回来时，
        // 扫描要能把它们恢复成正常笔记（否则文件明明回来了，笔记却永远停在「缺失」）。
        let existing = self.ctx.db.with_conn(|conn| {
            let mut notes =
                SqliteNoteRepository.list_by_status(conn, self.workspace_id(), NoteStatus::Active)?;
            notes.extend(SqliteNoteRepository.list_by_status(
                conn,
                self.workspace_id(),
                NoteStatus::Missing,
            )?);
            Ok::<Vec<Note>, crate::core::error::AppError>(notes)
        })?;
        let mut by_path: HashMap<String, Note> = existing
            .into_iter()
            .map(|note| (note.file_path.clone(), note))
            .collect();
        // 关键：按「本轮真正需要采纳的数量」判断是否批量，而不是磁盘上的文件总数。
        // 轮询式监听每 3 秒都会看到全部文件；若按总数判断，每轮都会触发一次全量重建，
        // 反而把首次采纳拖慢到 400s+（实测）。
        const BULK_ADOPT_THRESHOLD: usize = 50;
        let defer_derived = files
            .iter()
            .filter(|relative| !by_path.contains_key(relative.as_str()))
            .count()
            > BULK_ADOPT_THRESHOLD;

        let mut to_adopt: Vec<String> = Vec::new();
        // 单文件路径采纳时收集「frontmatter 声明了父文档」的笔记，扫描末尾统一解析
        // （父文档可能排在子文档后面才被采纳）。
        let mut declared_parents: Vec<(String, String)> = Vec::new();
        for relative in files {
            match by_path.remove(&relative) {
                Some(note) => {
                    if note.status == NoteStatus::Missing {
                        // 文件回来了：走「外部编辑」路径恢复（内含 缺失 → 正常 + 重建索引/链接/关系）
                        match self.ctx.storage.read_note(&note.file_path) {
                            Ok(stored) => {
                                self.apply_external_edit(&note, &stored, &mut report)?;
                                report.updated += 1;
                            }
                            Err(error) => report.warnings.push(format!(
                                "{} 恢复失败：{}",
                                note.file_path, error.message
                            )),
                        }
                        continue;
                    }
                    if self.sync_note_from_disk(&note, &mut report)? {
                        report.updated += 1;
                    } else {
                        report.unchanged += 1;
                    }
                }
                None => {
                    if defer_derived {
                        // 批量导入：先收集，循环结束后按批事务写入
                        to_adopt.push(relative);
                    } else {
                        self.adopt_file(
                            &relative,
                            &lookup,
                            &mut report,
                            false,
                            &mut declared_parents,
                        )?;
                    }
                }
            }
        }
        if !to_adopt.is_empty() {
            self.adopt_files_batched(&to_adopt, &lookup, &mut report)?;
        }
        let resolved_parents = self.resolve_declared_parents(&declared_parents)?;
        if resolved_parents > 0 {
            tracing::debug!(target: "index", "本轮解析了 {resolved_parents} 篇子文档的父文档");
        }
        // 本轮有新增 / 更新：父文档可能就是这一轮才出现的，补一次「待解析声明」的重试
        if report.added > 0 || report.updated > 0 {
            let retried = self.retry_unresolved_declarations()?;
            if retried > 0 {
                tracing::debug!(target: "index", "补解析了 {retried} 篇先前悬空的子文档层级");
            }
        }

        // 剩余记录对应的文件已消失
        for note in by_path.values() {
            if disk.contains(&note.file_path) {
                continue;
            }
            if note.status == NoteStatus::Missing {
                // 上一轮已经标记过：不重复计数、不重复发事件
                continue;
            }
            // 快照可能已过期（并发写入/移动），落库前再做一次新鲜检查
            if self.ctx.storage.exists(&note.file_path) {
                continue;
            }
            self.mark_missing(&note.id)?;
            report.missing += 1;
        }

        // 采纳过新文件：统一重建一次派生数据（链接 + 类型化关系）。
        // 两者都是 O(N) 的全量重建，比逐篇重建标题索引便宜两个数量级。
        if defer_derived && report.added > 0 {
            self.rebuild_derived_after_bulk();
        }

        // `.trash/` 里的 Markdown 也要登记回库：删库重建（文档推荐的恢复手段）或换机器之后，
        // 回收站不能凭空消失——文件还在，只是此前没有任何一条记录指向它们。
        if let Err(error) = self.adopt_trash_notes(&mut report) {
            report
                .warnings
                .push(format!("重建回收站记录失败：{}", error.message));
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
        // 轮询式监听可能一次上报成百上千个新增文件（例如整目录拷入），
        // 此时逐篇刷新派生数据同样会退化为 O(N²)，改为整批结束后重建一次。
        const BULK_ADOPT_THRESHOLD: usize = 50;
        let defer_derived = batch.changes.len() > BULK_ADOPT_THRESHOLD;
        let mut to_adopt: Vec<String> = Vec::new();

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
                            // 新增文件统一收集，循环结束后按批事务写入（单篇也只是一个很小的批次）
                            to_adopt.push(relative);
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

        if !to_adopt.is_empty() {
            self.adopt_files_batched(&to_adopt, &lookup, &mut report)?;
        }
        // 父文档可能是这一轮才被外部新增的：补一次「待解析声明」的重试，
        // 让先到的子文档在同一轮就归位（不必等它下次被编辑）。
        if report.added > 0 || report.updated > 0 {
            let retried = self.retry_unresolved_declarations()?;
            if retried > 0 {
                tracing::debug!(target: "index", "补解析了 {retried} 篇先前悬空的子文档层级");
            }
        }
        if defer_derived && report.added > 0 {
            self.rebuild_derived_after_bulk();
        }

        report.duration_ms = started.elapsed().as_millis() as u64;
        if report.added > 0 || report.updated > 0 || report.missing > 0 {
            if let Ok(payload) = serde_json::to_value(&report) {
                self.ctx.events.emit(event_names::FILES_CHANGED, payload);
            }
        }
        Ok(report)
    }

    /// 批量采纳：文件读取在事务外完成，数据库写入按批提交。
    ///
    /// 单篇采纳原本要 4–6 次独立事务（记录 / 标签 / FTS / 标记已索引），
    /// 1 万篇即 5 万+ 次提交，实测首次导入要 6.5 分钟；按批（200 篇/事务）后
    /// 提交次数降到 1/1000 量级，同时大幅减少 WAL 检查点开销。
    fn adopt_files_batched(
        &self,
        files: &[String],
        lookup: &super::lookup::CategoryLookup,
        report: &mut ScanReport,
    ) -> Result<()> {
        const ADOPT_BATCH_SIZE: usize = 200;

        // 1) 事务外：读取 + 解析 + 组装 Note（IO 密集，不占用数据库事务）
        struct Prepared {
            note: Note,
            plain_text: String,
            /// 正文原文（Markdown，不含 frontmatter）：分块索引要从它算字符区间
            source: String,
            tags: Vec<String>,
            category_path: String,
            /// frontmatter 里声明的父文档（id 或标题），批次结束后统一解析
            declared_parent: Option<String>,
        }
        let workspace_id = self.workspace_id().to_string();
        let mut prepared: Vec<Prepared> = Vec::with_capacity(files.len());
        for relative in files {
            let stored = match self.ctx.storage.read_note(relative) {
                Ok(stored) => stored,
                Err(error) => {
                    report
                        .warnings
                        .push(format!("跳过无法读取的文件 {relative}：{}", error.message));
                    continue;
                }
            };
            if let Some(error) = &stored.parse_error {
                report.warnings.push(format!("{relative}：{error}"));
            }
            let title = stored
                .frontmatter
                .title
                .clone()
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| file_stem_title(relative));
            let category_id = directory_category_id(lookup, relative);
            let now = now_iso();
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
            let plain_text = markdown_to_plain_text(&stored.body);
            let stamp = self.ctx.storage.file_stamp(relative).ok();
            let category_path = category_id
                .as_ref()
                .map(|id| lookup.path_string(id))
                .unwrap_or_default();
            let declared_parent = stored
                .frontmatter
                .parent
                .clone()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            prepared.push(Prepared {
                note: Note {
                    // id 在事务内确定（需要检查 frontmatter 里的 id 是否已被占用）
                    id: stored
                        .frontmatter
                        .id
                        .clone()
                        .filter(|id| !id.trim().is_empty())
                        .unwrap_or_else(note_id),
                    workspace_id: workspace_id.clone(),
                    title,
                    file_path: relative.to_string(),
                    category_id,
                    status: NoteStatus::Active,
                    content_hash: Some(stored.content_hash.clone()),
                    word_count: count_words(&plain_text),
                    favorite: stored.frontmatter.favorite,
                    created_at,
                    updated_at,
                    last_indexed_at: None,
                    source_type: stored.frontmatter.source_type.clone(),
                    source_ref: stored.frontmatter.source_ref.clone(),
                    source_reliability: stored.frontmatter.source_reliability,
                    valid_until: stored.frontmatter.valid_until.clone(),
                    file_size: stamp.as_ref().map(|(size, _)| *size as i64),
                    file_mtime: stamp.and_then(|(_, mtime)| mtime),
                    parent_id: None,
                    declared_parent: declared_parent.clone(),
                    // frontmatter 的 order 是文件里的顺序声明，扫描时投影到派生列
                    sort_order: stored.frontmatter.order.unwrap_or(0).max(0),
                    pinned: stored.frontmatter.pinned,
                },
                plain_text,
                source: stored.body.clone(),
                tags: stored.frontmatter.tags.clone(),
                category_path,
                declared_parent,
            });
        }

        // 2) 分批事务写入
        let workspace_key = workspace_id.clone();
        for chunk in prepared.chunks(ADOPT_BATCH_SIZE) {
            self.ctx.db.with_transaction(|tx| {
                for item in chunk {
                    // 路径已被其它扫描采纳 → 跳过（幂等）
                    if SqliteNoteRepository.get_by_path(tx, &workspace_key, &item.note.file_path)?
                        .is_some()
                    {
                        continue;
                    }
                    let mut note = item.note.clone();
                    // id 冲突（frontmatter 里带的是别篇的 id）时重新生成
                    if SqliteNoteRepository.get(tx, &note.id)?.is_some() {
                        note.id = note_id();
                    }
                    SqliteNoteRepository.insert(tx, &note)?;

                    // 标签：复用仓储的 conn 接口，避免在事务内再开事务
                    let mut tag_names: Vec<String> = Vec::with_capacity(item.tags.len());
                    for name in &item.tags {
                        let key = tag_key(name);
                        if key.is_empty() {
                            continue;
                        }
                        let tag = match SqliteTagRepository.find_by_key(tx, &workspace_key, &key)? {
                            Some(tag) => tag,
                            None => {
                                let tag = Tag {
                                    id: crate::core::ids::tag_id(),
                                    workspace_id: workspace_key.clone(),
                                    name: name.trim().to_string(),
                                    created_at: now_iso(),
                                    note_count: 0,
                                };
                                SqliteTagRepository.insert(tx, &tag)?;
                                tag
                            }
                        };
                        tx.execute(
                            "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                            rusqlite::params![note.id, tag.id],
                        )?;
                        tag_names.push(tag.name);
                    }

                    // FTS 索引（同一事务）
                    let document = SearchDocument::build(
                        &note.id,
                        &note.title,
                        &item.plain_text,
                        &tag_names,
                        &item.category_path,
                    );
                    SqliteSearchIndex.upsert(tx, &document)?;
                    // 分块索引（N5）：与 FTS 同一批次写入
                    SqliteChunkRepository.replace(tx, &note.id, &chunk_markdown(&item.source))?;
                    tx.execute(
                        "UPDATE notes SET last_indexed_at = ?2 WHERE id = ?1",
                        rusqlite::params![note.id, now_iso()],
                    )?;
                    report.added += 1;
                }
                Ok(())
            })?;
        }
        // 3) 解析子文档层级：放在所有批次之后统一处理，保证跨批次的父子引用也能解析。
        let pending: Vec<(String, String)> = prepared
            .iter()
            .filter_map(|item| item.declared_parent.clone().map(|parent| (item.note.id.clone(), parent)))
            .collect();
        self.resolve_declared_parents(&pending)?;
        Ok(())
    }

    /// 统一解析 frontmatter 里声明的父文档（`parent` 可能是**笔记 id**，也可能是**标题**）。
    ///
    /// 必须等「本轮所有文件都入库」之后再跑：单文件路径是按文件名顺序采纳的，
    /// 子文档可能先于父文档被读到（例如删库后重扫、或一次拖入多篇）。
    ///
    /// 解析不到时（父文档在这一轮里还不存在）把**声明原文留在库里**，
    /// 由 [`Self::retry_unresolved_declarations`] 在后续轮次重试——
    /// 否则子文档会一直停在顶层，直到它下次被外部编辑。
    fn resolve_declared_parents(&self, pending: &[(String, String)]) -> Result<usize> {
        if pending.is_empty() {
            return Ok(0);
        }
        let workspace_id = self.workspace_id().to_string();
        let titles: HashMap<String, String> = self
            .ctx
            .db
            .with_conn(|conn| {
                SqliteNoteRepository.list_by_status(conn, &workspace_id, NoteStatus::Active)
            })?
            .into_iter()
            .map(|note| (note.title.to_lowercase(), note.id))
            .collect();
        let resolved = self.ctx.db.with_transaction(|tx| {
            let mut resolved = 0usize;
            for (note_id, declared) in pending {
                let reference = declared.trim();
                if reference.is_empty() {
                    continue;
                }
                let parent_id = match SqliteNoteRepository.get(tx, reference)? {
                    Some(_) => Some(reference.to_string()),
                    None => titles.get(&reference.to_lowercase()).cloned(),
                };
                match parent_id {
                    Some(parent_id) if parent_id != *note_id => {
                        SqliteNoteRepository.set_parent(
                            tx,
                            note_id,
                            Some(&parent_id),
                            Some(reference),
                        )?;
                        resolved += 1;
                    }
                    // 自指或解析不到：只保留声明原文，留给后续轮次重试
                    _ => SqliteNoteRepository.set_declared_parent(tx, note_id, Some(reference))?,
                }
            }
            Ok(resolved)
        })?;
        Ok(resolved)
    }

    /// 重试解析「声明了父文档、但还没解析成功」的子文档。
    ///
    /// 场景：子文档先被采纳（父文档当时还不在工作区），父文档在**后一轮**扫描才出现。
    /// 声明的原文存在 `notes.declared_parent`，所以这里只需一次小查询即可重试，
    /// 不必为了找它们去读全库文件。
    fn retry_unresolved_declarations(&self) -> Result<usize> {
        let unresolved = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_unresolved_declared_parents(conn, self.workspace_id())
        })?;
        if unresolved.is_empty() {
            return Ok(0);
        }
        let pending: Vec<(String, String)> = unresolved
            .into_iter()
            .map(|note| (note.id, note.declared_parent.unwrap_or_default()))
            .collect();
        self.resolve_declared_parents(&pending)
    }

    /// 把 `.trash/` 下**尚未登记**的 Markdown 采纳成「已删除」笔记（可恢复）。
    ///
    /// 背景：回收站文件是「原文件被移动到 `.trash/<原相对路径>`」，
    /// 它们同样是 Markdown、同样带 frontmatter。删库重建 / 换机器后，
    /// 如果不登记这些文件，「回收站」会空掉——文件明明还在，用户却看不到、也恢复不了。
    ///
    /// 规则：
    /// - 只处理 Markdown；按 `trash_path`（或 id）已登记过的直接跳过，保持幂等；
    /// - `file_path` 取「去掉 `.trash/` 前缀」的原路径（`restore_from_trash` 依赖这个约定）；
    /// - id 已被占用（例如这篇笔记又回到活跃列表）时跳过，绝不覆盖已有记录。
    fn adopt_trash_notes(&self, report: &mut ScanReport) -> Result<usize> {
        let root = self.ctx.paths.trash_dir();
        if !root.exists() {
            return Ok(0);
        }
        let mut adopted = 0usize;
        for entry in walkdir::WalkDir::new(&root).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(relative) = self.ctx.paths.to_relative(entry.path()) else {
                continue;
            };
            if !relative.starts_with(&format!("{TRASH_DIR}/")) {
                continue;
            }
            if !(relative.ends_with(".md") || relative.ends_with(".markdown")) {
                continue;
            }
            let stored = match self.ctx.storage.read_note(&relative) {
                Ok(stored) => stored,
                Err(error) => {
                    report
                        .warnings
                        .push(format!("跳过无法读取的回收站文件 {relative}：{}", error.message));
                    continue;
                }
            };
            let id = stored
                .frontmatter
                .id
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(note_id);
            if self.ctx.db.with_conn(|conn| {
                Ok(SqliteNoteRepository.get(conn, &id)?.is_some()
                    || trash_path_of(conn, &id)?.is_some())
            })? {
                continue;
            }
            let title = stored
                .frontmatter
                .title
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| file_stem_title(&relative));
            let created_at = stored
                .frontmatter
                .created_at
                .clone()
                .or_else(|| stored.modified_at.clone())
                .unwrap_or_else(now_iso);
            let updated_at = stored
                .frontmatter
                .updated_at
                .clone()
                .unwrap_or_else(|| created_at.clone());
            let original_path = relative
                .trim_start_matches(&format!("{TRASH_DIR}/"))
                .to_string();
            let note = Note {
                id: id.clone(),
                workspace_id: self.workspace_id().to_string(),
                title,
                file_path: original_path,
                category_id: None,
                status: NoteStatus::Deleted,
                content_hash: Some(stored.content_hash.clone()),
                word_count: count_words(&markdown_to_plain_text(&stored.body)),
                favorite: stored.frontmatter.favorite,
                created_at,
                updated_at,
                last_indexed_at: None,
                source_type: stored.frontmatter.source_type.clone(),
                source_ref: stored.frontmatter.source_ref.clone(),
                source_reliability: stored.frontmatter.source_reliability,
                valid_until: stored.frontmatter.valid_until.clone(),
                file_size: None,
                file_mtime: None,
                parent_id: None,
                declared_parent: None,
                sort_order: 0,
                pinned: stored.frontmatter.pinned,
            };
            self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.insert(conn, &note)?;
                SqliteNoteRepository.set_status(conn, &note.id, NoteStatus::Deleted, Some(&relative))
            })?;
            adopted += 1;
        }
        if adopted > 0 {
            report.updated += adopted as i64;
            tracing::info!(target: "index", "从 .trash/ 重建了 {adopted} 条回收站记录");
        }
        Ok(adopted)
    }

    /// 采纳磁盘上的新文件。
    fn adopt_file(
        &self,
        relative: &str,
        lookup: &super::lookup::CategoryLookup,
        report: &mut ScanReport,
        defer_derived: bool,
        declared_parents: &mut Vec<(String, String)>,
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
            favorite: stored.frontmatter.favorite,
            created_at,
            updated_at,
            last_indexed_at: None,
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            file_size: None,
            file_mtime: None,
            parent_id: None,
            declared_parent: stored
                .frontmatter
                .parent
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            sort_order: stored.frontmatter.order.unwrap_or(0).max(0),
            pinned: stored.frontmatter.pinned,
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
        if adopted {
            if let Ok((size, mtime)) = self.ctx.storage.file_stamp(relative) {
                self.ctx.db.with_conn(|conn| {
                    SqliteNoteRepository.set_file_stamp(
                        conn,
                        &note.id,
                        Some(size as i64),
                        mtime.as_deref(),
                    )
                })?;
            }
        }
        if !adopted {
            tracing::debug!(target: "index", "路径已被其它扫描采纳，跳过：{relative}");
            report.unchanged += 1;
            return Ok(());
        }

        for tag in &stored.frontmatter.tags {
            let _ = super::tag_service::TagService::new(Arc::clone(&self.ctx))
                .ensure_tag(&id, tag);
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
        // 小批量（监听回调、零散新增）：逐篇刷新，改动立即生效；
        // 大批量（导入整目录）：跳过逐篇刷新（每篇都会重建标题索引 → O(N²)，10k 实测 306s），
        // 由 scan_scope 在整轮结束后统一重建一次（O(N)）。
        if !defer_derived {
            if let Err(error) =
                LinkService::new(Arc::clone(&self.ctx)).on_note_available(&note, &stored.body)
            {
                report
                    .warnings
                    .push(format!("{relative} 建立链接失败：{}", error.message));
            }
            if let Err(error) = RelationService::new(Arc::clone(&self.ctx)).refresh_note(&note) {
                tracing::debug!(target: "relations", "采纳文件时刷新关系失败：{}", error.message);
            }
            // 子文档层级：frontmatter 的 `parent` 可能是 id 或标题。
            // 单文件路径（监听回调、零散新增、小库重扫）先记下来，
            // 等整轮扫描结束再统一解析——父文档可能排在子文档后面才被采纳。
            let declares_parent = stored
                .frontmatter
                .parent
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            if declares_parent {
                declared_parents.push((note.id.clone(), stored.frontmatter.parent.clone().unwrap_or_default()));
            }
        }
        Ok(())
    }

    /// 同步已知笔记的内容变化（工作区打开时使用）。
    fn sync_note_from_disk(&self, note: &Note, report: &mut ScanReport) -> Result<bool> {
        // 1) mtime + size 短路：完全没动过的文件不必读取全文、更不必计算 sha256。
        //    1 万篇规模下这一步是扫描的主要成本（轮询式监听每 3 秒会扫一遍全库）。
        let stamp = self.ctx.storage.file_stamp(&note.file_path)?;
        let unchanged_stamp = note.file_size == Some(stamp.0 as i64)
            && note.file_mtime.as_deref() == stamp.1.as_deref();
        if unchanged_stamp {
            return Ok(false);
        }

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
            // 内容没变（可能只是被 touch 过或我们自己写入导致的 mtime 变化）：刷新快照即可
            self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.set_file_stamp(
                    conn,
                    &note.id,
                    Some(stamp.0 as i64),
                    stamp.1.as_deref(),
                )
            })?;
            return Ok(false);
        }
        if self
            .ctx
            .storage
            .recent_writes()
            .is_self_write(&note.file_path, &stored.content_hash)
        {
            self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.set_file_stamp(
                    conn,
                    &note.id,
                    Some(stamp.0 as i64),
                    stamp.1.as_deref(),
                )
            })?;
            return Ok(false);
        }
        self.apply_external_edit(note, &stored, report)?;
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.set_file_stamp(
                conn,
                &note.id,
                Some(stamp.0 as i64),
                stamp.1.as_deref(),
            )
        })?;
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
            // 手动排序与置顶同样是「文件为准」的派生列：
            // 用户用外部编辑器改 `order:` / `pinned:` 后要能生效（此前漏了这两个投影）
            SqliteNoteRepository.set_sort_order(
                conn,
                &note.id,
                stored.frontmatter.order.unwrap_or(0).max(0),
            )?;
            SqliteNoteRepository.set_pinned(conn, &note.id, stored.frontmatter.pinned)?;
            // 收藏同样是「文件为准」的派生列：外部编辑器改 `favorite:` 后要能生效
            SqliteNoteRepository.set_favorite(conn, &note.id, stored.frontmatter.favorite)?;
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
        if let Err(error) = NoteService::new(Arc::clone(&self.ctx)).refresh_parent(note) {
            tracing::debug!(target: "notes", "同步父文档失败：{}", error.message);
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
    // 除文本外，也去掉 Office 导入的扩展名（.docx / .csv），标题不保留后缀
    const TEXT_EXTENSIONS: [&str; 7] = ["md", "markdown", "mdown", "txt", "text", "docx", "csv"];

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
