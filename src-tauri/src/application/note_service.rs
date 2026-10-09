//! 笔记服务：创建、读取、保存、重命名、移动、删除与恢复
//! （设计文档 5.2 / 13）。

use std::sync::Arc;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::hash::sha256_hex_str;
use crate::core::ids::note_id;
use crate::core::paths::{WorkspacePaths, INBOX_DIR};
use crate::core::text::{count_words, markdown_to_plain_text};
use crate::core::time::{now_file_timestamp, now_iso};
use crate::domain::note::{
    ReorderNotesInput,
    SetParentInput,
    SetSourceInput,
    CreateNoteInput, Note, NoteDetail, NoteListQuery, NoteListResult, NoteStatus, NoteSummary,
    SaveNoteInput, SaveNoteResult,
};
use crate::infrastructure::frontmatter::{self, Frontmatter};
use crate::infrastructure::markdown_storage::{file_name_of, join_relative, parent_relative};
use crate::repositories::tag_repository::tags_for_note;
use crate::repositories::{NoteRepository, SqliteNoteRepository, trash_path_of};

use super::context::WorkspaceContext;
use super::history_service::{HistoryContent, HistoryEntry, HistoryService};
use super::index_service::IndexService;
use super::link_service::LinkService;
use super::relation_service::RelationService;
use super::tag_service::TagService;

pub struct NoteService {
    ctx: Arc<WorkspaceContext>,
}

impl NoteService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    fn index(&self) -> IndexService {
        IndexService::new(Arc::clone(&self.ctx))
    }

    fn tags(&self) -> TagService {
        TagService::new(Arc::clone(&self.ctx))
    }

    fn history(&self) -> HistoryService {
        HistoryService::new(Arc::clone(&self.ctx))
    }

    /// 版本历史：列表 / 读取 / 还原 / 删除（对标语雀「查看历史版本」）。
    pub fn history_list(&self, note_id: &str) -> Result<Vec<HistoryEntry>> {
        self.note_or_err(note_id)?;
        self.history().list(note_id)
    }

    pub fn history_read(&self, note_id: &str, entry_id: &str) -> Result<HistoryContent> {
        self.note_or_err(note_id)?;
        self.history().read(note_id, entry_id)
    }

    pub fn history_remove(&self, note_id: &str, entry_id: &str) -> Result<usize> {
        self.note_or_err(note_id)?;
        self.history().remove(note_id, entry_id)?;
        Ok(self.history().list(note_id)?.len())
    }

    /// 还原到某个历史版本：先把当前内容强制留档，再走常规保存路径写入旧正文。
    ///
    /// 只还原**正文**，标题 / 标签 / 分类 / 父文档等元数据保持当前值——
    /// 避免「回滚内容」意外把改过的标题也退回去。
    pub fn history_restore(&self, note_id: &str, entry_id: &str) -> Result<NoteDetail> {
        let note = self.note_or_err(note_id)?;
        let entry = self.history().read(note_id, entry_id)?;
        if let Ok(current_raw) = self.ctx.storage.read_raw(&note.file_path) {
            let _ = self.history().capture_forced(&note.id, &current_raw);
        }
        let content_hash = self.ctx.storage.read_note(&note.file_path)?.content_hash;
        self.save(SaveNoteInput {
            id: note.id.clone(),
            content: entry.body,
            base_content_hash: Some(content_hash),
            title: None,
            category_id: None,
            tags: None,
        })?;
        self.get(&note.id)
    }

    fn workspace_id(&self) -> &str {
        &self.ctx.workspace.id
    }

    /// 读取笔记行，不存在时返回统一错误。
    fn note_or_err(&self, id: &str) -> Result<Note> {
        let note = self
            .ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, id))?;
        note.ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "笔记不存在或已被删除。"))
    }

    /// 组装完整笔记详情。
    pub fn detail(&self, note: Note) -> Result<NoteDetail> {
        let stored = match self.ctx.storage.read_note(&note.file_path) {
            Ok(stored) => Some(stored),
            Err(_) => None,
        };
        let content = stored
            .as_ref()
            .map(|stored| stored.body.clone())
            .unwrap_or_default();
        let lookup = self.ctx.category_lookup()?;
        let category = note
            .category_id
            .as_ref()
            .and_then(|id| lookup.get(id).cloned());
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let tags = self
            .ctx
            .db
            .with_conn(|conn| tags_for_note(conn, &note.id))?;

        Ok(NoteDetail { note, content, category, category_path, tags })
    }

    /// 目录：分类目录缺失时回退到 Inbox。
    fn directory_for_category(&self, category_id: Option<&str>) -> Result<String> {
        match category_id {
            Some(category_id) => {
                let lookup = self.ctx.category_lookup()?;
                if lookup.get(category_id).is_none() {
                    return Err(AppError::new(
                        ErrorCode::CategoryNotFound,
                        "指定的分类不存在。",
                    ));
                }
                Ok(lookup.directory_of(category_id))
            }
            None => Ok(INBOX_DIR.to_string()),
        }
    }

    /// 创建笔记（FR-N01）。
    pub fn create(&self, input: CreateNoteInput) -> Result<NoteDetail> {
        let title = normalize_title(&input.title);
        let tags = self.tags().normalize_names(input.tags.unwrap_or_default());
        // 子文档：父文档可以用 id 指定（前端），也允许直接写标题
        let parent = match input.parent_id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(reference) => self.resolve_parent(reference, None)?,
            None => None,
        };
        // 未显式指定分类时继承父文档的分类（子文档默认与父文档放在一起）
        let category_id = input
            .category_id
            .clone()
            .or_else(|| parent.as_ref().and_then(|note| note.category_id.clone()));
        let directory = self.directory_for_category(category_id.as_deref())?;
        let lookup = self.ctx.category_lookup()?;
        let category_path = category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();

        let now = now_iso();
        let id = note_id();
        let body = input.initial_content.unwrap_or_default();
        let frontmatter = Frontmatter {
            id: Some(id.clone()),
            title: Some(title.clone()),
            note_type: Some("note".to_string()),
            status: Some("active".to_string()),
            category: Some(category_path.clone()).filter(|value| !value.is_empty()),
            tags: tags.clone(),
            created_at: Some(now.clone()),
            updated_at: Some(now.clone()),
            source_type: None,
            source_ref: None,
            source_reliability: None,
            valid_until: None,
            relations: Vec::new(),
            parent: parent.as_ref().map(|note| note.id.clone()),
            order: None,
            pinned: false,
            extra: Vec::new(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &body);
        let file_name = crate::core::text::note_file_name(&title, &now_file_timestamp());
        let relative = self
            .ctx
            .storage
            .create_note_file(&directory, &file_name, &markdown)?;

        let plain_text = markdown_to_plain_text(&body);
        let note = Note {
            id: id.clone(),
            workspace_id: self.workspace_id().to_string(),
            title,
            file_path: relative,
            category_id,
            status: NoteStatus::Active,
            content_hash: Some(sha256_hex_str(&markdown)),
            word_count: count_words(&plain_text),
            favorite: false,
            created_at: now.clone(),
            updated_at: now,
            last_indexed_at: None,
            file_size: None,
            file_mtime: None,
            parent_id: parent.as_ref().map(|note| note.id.clone()),
            sort_order: 0,
            pinned: false,
            source_type: None,
            source_ref: None,
            source_reliability: None,
            valid_until: None,
        };
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.insert(conn, &note))?;

        if !tags.is_empty() {
            self.tags().set_note_tags(&id, &tags)?;
        }
        self.reindex(&id)?;
        self.refresh_links(&note, &body);
        self.emit_change("created", &id);
        self.detail(note)
    }

    pub fn get(&self, id: &str) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        self.detail(note)
    }

    /// 保存笔记（FR-N03 / 13.1 / 19.3）。
    pub fn save(&self, input: SaveNoteInput) -> Result<SaveNoteResult> {
        let note = self.note_or_err(&input.id)?;
        if note.status == NoteStatus::Deleted {
            return Err(AppError::new(
                ErrorCode::NoteNotFound,
                "笔记位于回收站中，请先恢复后再编辑。",
            ));
        }

        let existing = self.ctx.storage.read_note(&note.file_path)?;
        if let Some(base) = input.base_content_hash.as_deref() {
            if !base.is_empty() && base != existing.content_hash {
                return Err(AppError::new(
                    ErrorCode::NoteConflict,
                    "文件已被其他程序修改，请选择重新加载或覆盖保存。",
                )
                .with_details(serde_json::json!({
                    "noteId": note.id,
                    "filePath": note.file_path,
                    "currentHash": existing.content_hash,
                    "expectedHash": base,
                    "currentContent": existing.body,
                })));
            }
        }

        let title = input
            .title
            .as_deref()
            .map(normalize_title)
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| note.title.clone());
        let category_id = input.category_id.clone().or_else(|| note.category_id.clone());
        let lookup = self.ctx.category_lookup()?;
        let category_path = category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();

        let replace_tags = input.tags.is_some();
        let tags = match input.tags {
            Some(tags) => self.tags().normalize_names(tags),
            None => self
                .ctx
                .db
                .with_conn(|conn| tags_for_note(conn, &note.id))?
                .into_iter()
                .map(|tag| tag.name)
                .collect(),
        };

        let now = now_iso();
        let created_at = existing
            .frontmatter
            .created_at
            .clone()
            .unwrap_or_else(|| note.created_at.clone());
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(title.clone()),
            note_type: existing
                .frontmatter
                .note_type
                .clone()
                .or_else(|| Some("note".to_string())),
            status: Some("active".to_string()),
            category: Some(category_path.clone()).filter(|value| !value.is_empty()),
            tags: tags.clone(),
            created_at: Some(created_at),
            updated_at: Some(now.clone()),
            source_type: existing.frontmatter.source_type.clone(),
            source_ref: existing.frontmatter.source_ref.clone(),
            source_reliability: existing.frontmatter.source_reliability,
            valid_until: existing.frontmatter.valid_until.clone(),
            parent: existing.frontmatter.parent.clone(),
            order: existing.frontmatter.order,
            pinned: existing.frontmatter.pinned,
            relations: existing.frontmatter.relations.clone(),
            extra: existing.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &input.content);
        // 覆盖写入之前先给「写入前的内容」留一份历史快照（对标语雀「查看历史版本」）。
        // 只在正文真的变了时留档：自动保存会被重复触发，否则会把「当前内容」也记成历史，
        // 用户点还原就等于什么都没发生。
        if existing.body != input.content {
            if let Ok(previous_raw) = self.ctx.storage.read_raw(&note.file_path) {
                if let Err(error) = self.history().capture(&note.id, &previous_raw) {
                    // 历史是附加价值：留档失败不应阻断保存，只记录警告
                    tracing::warn!(target: "history", "写入历史快照失败：{}", error.message);
                }
            }
        }
        self.refresh_file_stamp(&note.id, &note.file_path);
        let _ = self.refresh_parent(&note);
        let content_hash = self
            .ctx
            .storage
            .write_atomic(&note.file_path, &markdown)?;

        let word_count = count_words(&markdown_to_plain_text(&input.content));
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.update_identity(
                conn,
                &note.id,
                &title,
                &note.file_path,
                category_id.as_deref(),
                &now,
            )?;
            SqliteNoteRepository.update_content_meta(
                conn,
                &note.id,
                &now,
                &content_hash,
                word_count,
                None,
            )
        })?;

        if !tags.is_empty() || replace_tags {
            self.tags().set_note_tags(&note.id, &tags)?;
        }

        let index_warning = match self.reindex(&note.id) {
            Ok(()) => None,
            Err(error) => Some(error.message),
        };

        let updated = self.note_or_err(&note.id)?;
        self.refresh_outgoing_links(&updated, &input.content);
        self.emit_change("updated", &note.id);
        Ok(SaveNoteResult {
            note: updated,
            content_hash,
            saved_at: now,
            index_updated: index_warning.is_none(),
            index_warning,
        })
    }

    /// 重命名（FR-N06）：可选择同步重命名文件。
    pub fn rename(&self, id: &str, new_title: &str, rename_file: bool) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        let title = normalize_title(new_title);
        if title.is_empty() {
            return Err(AppError::invalid_input("标题不能为空。"));
        }
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let mut file_path = note.file_path.clone();
        let now = now_iso();

        if rename_file {
            let directory = parent_relative(&note.file_path);
            let stem = file_stem_with_prefix(&note.file_path, &title);
            let new_relative = if directory.is_empty() {
                format!("{stem}.md")
            } else {
                join_relative(&directory, &format!("{stem}.md"))
            };
            if new_relative != file_path {
                file_path = self
                    .ctx
                    .storage
                    .move_note(&file_path, &directory, Some(&stem))?;
            }
        }

        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let tags: Vec<String> = self
            .ctx
            .db
            .with_conn(|conn| tags_for_note(conn, &note.id))?
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: Some("active".to_string()),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags,
            created_at: stored
                .frontmatter
                .created_at
                .clone()
                .or_else(|| Some(note.created_at.clone())),
            updated_at: Some(now.clone()),
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            relations: stored.frontmatter.relations.clone(),
            parent: stored.frontmatter.parent.clone(),
            order: stored.frontmatter.order,
            pinned: stored.frontmatter.pinned,
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&file_path, &markdown)?;

        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.update_identity(
                conn,
                &note.id,
                &title,
                &file_path,
                note.category_id.as_deref(),
                &now,
            )
        })?;
        self.reindex(&note.id)?;
        if let Ok(updated) = self.note_or_err(&note.id) {
            self.refresh_outgoing_links(&updated, &stored.body);
        }
        // 指向旧标题的链接自动重连（含改写来源笔记正文）
        if let Err(error) = RelationService::new(Arc::clone(&self.ctx))
            .on_note_renamed(&note.id, &note.title, &title)
        {
            tracing::warn!(target: "relations", "重命名时更新关系失败：{}", error.message);
        }
        if let Err(error) = self.links().on_note_renamed(&note.id, &note.title, &title) {
            tracing::warn!(target: "links", "重连双向链接失败：{}", error.message);
        }
        self.emit_change("renamed", &note.id);
        self.detail(self.note_or_err(&note.id)?)
    }

    /// 移动笔记到分类（FR-N07）。
    pub fn move_to_category(&self, id: &str, category_id: Option<&str>) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        if note.category_id.as_deref() == category_id {
            return self.detail(note);
        }
        let target_directory = self.directory_for_category(category_id)?;
        let current_directory = parent_relative(&note.file_path);
        let now = now_iso();

        let file_path = if current_directory != target_directory {
            self.ctx
                .storage
                .move_note(&note.file_path, &target_directory, None)?
        } else {
            note.file_path.clone()
        };

        // 同步 frontmatter 中的分类路径
        let stored = self.ctx.storage.read_note(&file_path)?;
        let lookup = self.ctx.category_lookup()?;
        let category_path = category_id
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let tags: Vec<String> = self
            .ctx
            .db
            .with_conn(|conn| tags_for_note(conn, &note.id))?
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(note.title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: Some("active".to_string()),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags,
            created_at: stored
                .frontmatter
                .created_at
                .clone()
                .or_else(|| Some(note.created_at.clone())),
            updated_at: Some(now.clone()),
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            relations: stored.frontmatter.relations.clone(),
            parent: stored.frontmatter.parent.clone(),
            order: stored.frontmatter.order,
            pinned: stored.frontmatter.pinned,
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&file_path, &markdown)?;

        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.update_identity(
                conn,
                &note.id,
                &note.title,
                &file_path,
                category_id,
                &now,
            )
        })?;
        self.reindex(&note.id)?;
        if let Ok(updated) = self.note_or_err(&note.id) {
            self.refresh_outgoing_links(&updated, &stored.body);
        }
        self.emit_change("moved", &note.id);
        self.detail(self.note_or_err(&note.id)?)
    }

    /// 复制笔记。
    pub fn duplicate(&self, id: &str) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let tags: Vec<String> = self
            .ctx
            .db
            .with_conn(|conn| tags_for_note(conn, &note.id))?
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        self.create(CreateNoteInput {
            title: format!("{} 副本", note.title),
            category_id: note.category_id.clone(),
            initial_content: Some(stored.body),
            // 副本与原文同层（保持子文档归属），顺序由后续拖拽调整
            parent_id: note.parent_id.clone(),
            tags: Some(tags),
        })
    }

    /// 删除到回收站（FR-N05）。
    pub fn delete(&self, id: &str) -> Result<()> {
        let note = self.note_or_err(id)?;
        if note.status == NoteStatus::Deleted {
            return Ok(());
        }
        let _ = RelationService::new(Arc::clone(&self.ctx)).on_note_unavailable(&note.id);
        let trash_path = if self.ctx.storage.exists(&note.file_path) {
            Some(self.ctx.storage.move_to_trash(&note.file_path)?)
        } else {
            None
        };
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.set_status(conn, &note.id, NoteStatus::Deleted, trash_path.as_deref())
        })?;
        let _ = self.index().remove_note(&note.id);
        let _ = self.links().on_note_unavailable(&note.id);
        self.emit_change("deleted", &note.id);
        Ok(())
    }

    /// 从回收站恢复。
    pub fn restore(&self, id: &str) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        if note.status != NoteStatus::Deleted {
            return self.detail(note);
        }
        let trash_path = self
            .ctx
            .db
            .with_conn(|conn| trash_path_of(conn, &note.id))?
            .ok_or_else(|| {
                AppError::new(ErrorCode::NoteNotFound, "回收站中找不到对应文件，无法恢复。")
            })?;
        let restored = self
            .ctx
            .storage
            .restore_from_trash(&trash_path, &note.file_path)?;
        let now = now_iso();
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.set_status(conn, &note.id, NoteStatus::Active, None)?;
            SqliteNoteRepository.update_identity(
                conn,
                &note.id,
                &note.title,
                &restored,
                note.category_id.as_deref(),
                &now,
            )
        })?;
        self.reindex(&note.id)?;
        if let Ok(stored) = self.ctx.storage.read_note(&restored) {
            if let Ok(updated) = self.note_or_err(&note.id) {
                self.refresh_links(&updated, &stored.body);
            }
        }
        self.emit_change("restored", &note.id);
        self.detail(self.note_or_err(&note.id)?)
    }

    /// 永久删除。
    pub fn purge(&self, id: &str) -> Result<()> {
        let note = self.note_or_err(id)?;
        if let Some(trash_path) = self
            .ctx
            .db
            .with_conn(|conn| trash_path_of(conn, &note.id))?
        {
            self.ctx.storage.delete_from_trash(&trash_path)?;
        } else if self.ctx.storage.exists(&note.file_path) {
            self.ctx.storage.delete_from_trash(&note.file_path)?;
        }
        let _ = self.index().remove_note(&note.id);
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.delete(conn, &note.id))?;
        // 永久删除时一并清掉版本历史，避免 .knowledge/history 里留下孤儿目录
        if let Err(error) = self.history().purge(&note.id) {
            tracing::warn!(target: "history", "清理历史目录失败：{}", error.message);
        }
        self.emit_change("purged", &note.id);
        Ok(())
    }

    /// 收藏状态。
    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.set_favorite(conn, &note.id, favorite))?;
        self.detail(self.note_or_err(&note.id)?)
    }

    /// 设置标签。
    /// 设置父文档（子文档层级）：写回 frontmatter 的 `parent` 并更新派生列。
    pub fn set_parent(&self, input: SetParentInput) -> Result<NoteDetail> {
        let note = self.note_or_err(&input.id)?;
        let parent = match input.parent_id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(reference) => Some(self.resolve_parent(reference, Some(&note.id))?.ok_or_else(|| {
                AppError::new(ErrorCode::NoteNotFound, "父文档不存在或已被删除。")
            })?),
            None => None,
        };
        // 防环：不能把自己或自己的后代设为父文档
        if let Some(parent) = &parent {
            let mut cursor = Some(parent.id.clone());
            let mut hops = 0;
            while let Some(current) = cursor {
                if current == note.id {
                    return Err(AppError::invalid_input("不能把文档设置到它自己的子层级下。"));
                }
                hops += 1;
                if hops > 64 {
                    break;
                }
                cursor = self.note(&current)?.and_then(|item| item.parent_id);
            }
        }

        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(note.title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: stored.frontmatter.status.clone(),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags: stored.frontmatter.tags.clone(),
            created_at: stored.frontmatter.created_at.clone(),
            updated_at: Some(now_iso()),
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            relations: stored.frontmatter.relations.clone(),
            parent: parent.as_ref().map(|item| item.id.clone()),
            order: stored.frontmatter.order,
            pinned: stored.frontmatter.pinned,
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&note.file_path, &markdown)?;
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.set_parent(conn, &note.id, parent.as_ref().map(|item| item.id.as_str()))
        })?;
        self.refresh_file_stamp(&note.id, &note.file_path);
        self.get(&note.id)
    }

    /// 置顶 / 取消置顶（对标语雀「置顶文档」）：写回 frontmatter 的 `pinned`。
    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(note.title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: stored.frontmatter.status.clone(),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags: stored.frontmatter.tags.clone(),
            created_at: stored.frontmatter.created_at.clone(),
            // 置顶不是内容改动，保留原 updated_at
            updated_at: stored
                .frontmatter
                .updated_at
                .clone()
                .or_else(|| Some(note.updated_at.clone())),
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            relations: stored.frontmatter.relations.clone(),
            parent: stored.frontmatter.parent.clone(),
            order: stored.frontmatter.order,
            pinned,
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&note.file_path, &markdown)?;
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.set_pinned(conn, &note.id, pinned))?;
        self.refresh_file_stamp(&note.id, &note.file_path);
        self.emit_change("modified", &note.id);
        self.get(&note.id)
    }

    /// 手动排序：`ids` 是同一层兄弟笔记的新顺序（靠前者在上）。
    ///
    /// 位次从 1 开始写入 frontmatter 的 `order`，再投影到 `notes.sort_order`；
    /// 与父文档一样以文件为准，数据库整列删掉也能从 Markdown 重建。
    ///
    /// 为了让「自定义顺序」这一层是确定的，会把**同层全部笔记**一起归一化：
    /// 传入的 ids 按给定顺序排在前，其余同层笔记保持原相对顺序排在后面。
    pub fn reorder(&self, input: ReorderNotesInput) -> Result<usize> {
        let mut ids: Vec<String> = Vec::new();
        for id in &input.ids {
            let trimmed = id.trim();
            if !trimmed.is_empty() && !ids.iter().any(|item| item == trimmed) {
                ids.push(trimmed.to_string());
            }
        }
        if ids.is_empty() {
            return Err(AppError::invalid_input("排序列表为空。"));
        }

        // 以第一个存在的笔记为锚点确定层级（一次拖拽只会涉及同一层）
        let anchor = ids
            .iter()
            .find_map(|id| self.note(id).ok().flatten())
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "排序目标不存在或已被删除。"))?;
        let parent_id = anchor.parent_id.clone();

        // 同层归一化：传入顺序在前，其余同层笔记按原顺序追加
        let siblings = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.active_ids_under_parent(conn, self.workspace_id(), parent_id.as_deref())
        })?;
        let mut ordered: Vec<String> = ids
            .iter()
            .filter(|id| siblings.iter().any(|sibling| sibling == *id))
            .cloned()
            .collect();
        for sibling in siblings {
            if !ordered.contains(&sibling) {
                ordered.push(sibling);
            }
        }

        let mut updated = 0usize;
        for (index, id) in ordered.iter().enumerate() {
            let Some(note) = self.note(id)? else {
                continue; // 已被删除的笔记跳过，不影响其余顺序
            };
            let order = (index + 1) as i64;
            if note.sort_order == order {
                continue;
            }
            let stored = self.ctx.storage.read_note(&note.file_path)?;
            let lookup = self.ctx.category_lookup()?;
            let category_path = note
                .category_id
                .as_ref()
                .map(|id| lookup.path_string(id))
                .unwrap_or_default();
            let frontmatter = Frontmatter {
                id: Some(note.id.clone()),
                title: Some(note.title.clone()),
                note_type: stored.frontmatter.note_type.clone(),
                status: stored.frontmatter.status.clone(),
                category: Some(category_path).filter(|value| !value.is_empty()),
                tags: stored.frontmatter.tags.clone(),
                created_at: stored.frontmatter.created_at.clone(),
                // 排序不是内容改动，保留原 updated_at，避免「拖一下就变成最近修改」
                updated_at: stored
                    .frontmatter
                    .updated_at
                    .clone()
                    .or_else(|| Some(note.updated_at.clone())),
                source_type: stored.frontmatter.source_type.clone(),
                source_ref: stored.frontmatter.source_ref.clone(),
                source_reliability: stored.frontmatter.source_reliability,
                valid_until: stored.frontmatter.valid_until.clone(),
                relations: stored.frontmatter.relations.clone(),
                parent: stored.frontmatter.parent.clone(),
                order: Some(order),
                pinned: stored.frontmatter.pinned,
                extra: stored.frontmatter.extra.clone(),
            };
            let markdown = frontmatter::serialize(&frontmatter, &stored.body);
            self.ctx.storage.write_atomic(&note.file_path, &markdown)?;
            self.ctx
                .db
                .with_conn(|conn| SqliteNoteRepository.set_sort_order(conn, &note.id, order))?;
            self.refresh_file_stamp(&note.id, &note.file_path);
            updated += 1;
        }
        if updated > 0 {
            self.emit_change("modified", &ordered[0]);
        }
        Ok(updated)
    }

    /// 解析父文档引用：优先按 id，其次按标题；`for_child` 用于排除自身。
    fn resolve_parent(&self, reference: &str, for_child: Option<&str>) -> Result<Option<Note>> {
        if let Some(note) = self.note(reference)? {
            if Some(note.id.as_str()) != for_child {
                return Ok(Some(note));
            }
        }
        let found = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.find_active_id_by_title(conn, self.workspace_id(), reference)
        })?;
        match found {
            Some(id) if Some(id.as_str()) != for_child => self.note(&id),
            _ => Ok(None),
        }
    }

    /// 外部编辑后同步父文档（frontmatter 里的 `parent` 可能是 id 或标题）。
    pub fn refresh_parent(&self, note: &Note) -> Result<()> {
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let resolved = match stored.frontmatter.parent.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(reference) => self.resolve_parent(reference, Some(&note.id))?,
            None => None,
        };
        let parent_id = resolved.map(|item| item.id);
        if parent_id == note.parent_id {
            return Ok(());
        }
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.set_parent(conn, &note.id, parent_id.as_deref()))?;
        Ok(())
    }

    fn note(&self, id: &str) -> Result<Option<Note>> {
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, id))
    }

    /// 更新来源与时效（N1）。来源属于知识资产，写进 frontmatter，因此必须落盘。
    pub fn set_source(&self, input: SetSourceInput) -> Result<NoteDetail> {
        let note = self.note_or_err(&input.id)?;
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let now = now_iso();
        let reliability = input
            .source_reliability
            .filter(|level| (1..=5).contains(level));
        let trim = |value: Option<String>| {
            value
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
        };
        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(note.title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: stored.frontmatter.status.clone().or(Some("active".to_string())),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags: stored.frontmatter.tags.clone(),
            created_at: stored
                .frontmatter
                .created_at
                .clone()
                .or_else(|| Some(note.created_at.clone())),
            updated_at: Some(now.clone()),
            source_type: trim(input.source_type),
            source_ref: trim(input.source_ref),
            source_reliability: reliability,
            valid_until: trim(input.valid_until),
            relations: stored.frontmatter.relations.clone(),
            parent: stored.frontmatter.parent.clone(),
            order: stored.frontmatter.order,
            pinned: stored.frontmatter.pinned,
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&note.file_path, &markdown)?;
        self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.set_source(
                conn,
                &note.id,
                frontmatter.source_type.as_deref(),
                frontmatter.source_ref.as_deref(),
                frontmatter.source_reliability,
                frontmatter.valid_until.as_deref(),
            )
        })?;
        self.get(&note.id)
    }

    pub fn set_tags(&self, id: &str, tags: &[String]) -> Result<NoteDetail> {
        let note = self.note_or_err(id)?;
        let names = self.tags().normalize_names(tags.to_vec());
        self.tags().set_note_tags(&note.id, &names)?;
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|id| lookup.path_string(id))
            .unwrap_or_default();
        let frontmatter = Frontmatter {
            id: Some(note.id.clone()),
            title: Some(note.title.clone()),
            note_type: stored.frontmatter.note_type.clone(),
            status: Some("active".to_string()),
            category: Some(category_path).filter(|value| !value.is_empty()),
            tags: names.clone(),
            created_at: stored
                .frontmatter
                .created_at
                .clone()
                .or_else(|| Some(note.created_at.clone())),
            updated_at: Some(now_iso()),
            source_type: stored.frontmatter.source_type.clone(),
            source_ref: stored.frontmatter.source_ref.clone(),
            source_reliability: stored.frontmatter.source_reliability,
            valid_until: stored.frontmatter.valid_until.clone(),
            relations: stored.frontmatter.relations.clone(),
            parent: stored.frontmatter.parent.clone(),
            order: stored.frontmatter.order,
            pinned: stored.frontmatter.pinned,
            extra: stored.frontmatter.extra.clone(),
        };
        let markdown = frontmatter::serialize(&frontmatter, &stored.body);
        self.ctx.storage.write_atomic(&note.file_path, &markdown)?;
        self.reindex(&note.id)?;
        if let Ok(updated) = self.note_or_err(&note.id) {
            self.refresh_outgoing_links(&updated, &stored.body);
        }
        self.emit_change("tags", &note.id);
        self.detail(self.note_or_err(&note.id)?)
    }

    /// 列表查询。
    pub fn list(&self, query: NoteListQuery) -> Result<NoteListResult> {
        let (notes, total) = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list(conn, self.workspace_id(), &query)
        })?;
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(200).clamp(1, 500);
        let items = self.ctx.to_summaries(notes)?;
        Ok(NoteListResult { items, total, page, page_size })
    }

    /// 最近编辑（FR-N08）。
    pub fn recent(&self, limit: i64) -> Result<Vec<NoteSummary>> {
        let query = NoteListQuery {
            page: Some(1),
            page_size: Some(limit.clamp(1, 100)),
            ..Default::default()
        };
        Ok(self.list(query)?.items)
    }

    /// 回收站列表。
    pub fn list_deleted(&self) -> Result<Vec<NoteSummary>> {
        let query = NoteListQuery {
            status: Some(NoteStatus::Deleted),
            page_size: Some(500),
            ..Default::default()
        };
        Ok(self.list(query)?.items)
    }

    /// 缺失文件列表（外部删除或移动后需要人工处理）。
    pub fn list_missing(&self) -> Result<Vec<NoteSummary>> {
        let query = NoteListQuery {
            status: Some(NoteStatus::Missing),
            page_size: Some(500),
            ..Default::default()
        };
        Ok(self.list(query)?.items)
    }


    /// 写入文件后刷新「mtime + size」快照，使后续扫描可以直接短路。
    fn refresh_file_stamp(&self, note_id: &str, file_path: &str) {
        if let Ok((size, mtime)) = self.ctx.storage.file_stamp(file_path) {
            let _ = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.set_file_stamp(
                    conn,
                    note_id,
                    Some(size as i64),
                    mtime.as_deref(),
                )
            });
        }
    }

    fn reindex(&self, note_id: &str) -> Result<()> {
        let note = self.note_or_err(note_id)?;
        self.index().reindex_note(&note)
    }

    fn links(&self) -> LinkService {
        LinkService::new(Arc::clone(&self.ctx))
    }

    /// 刷新出链，并把指向该标题的未解析链接绑定到这篇笔记（创建/恢复后调用）。
    fn refresh_links(&self, note: &Note, body: &str) {
        if let Err(error) = self.links().on_note_available(note, body) {
            tracing::warn!(target: "links", "更新双向链接失败：{}", error.message);
        }
    }

    /// 只刷新这篇笔记的出链（正文变化时调用）。
    fn refresh_outgoing_links(&self, note: &Note, body: &str) {
        if let Err(error) = self.links().refresh_note_links(note, body) {
            tracing::warn!(target: "links", "更新出链失败：{}", error.message);
        }
    }

    fn emit_change(&self, kind: &str, note_id: &str) {
        self.ctx.events.emit(
            event_names::NOTES_CHANGED,
            serde_json::json!({ "kind": kind, "noteId": note_id }),
        );
    }
}

/// 标题规范化。
pub fn normalize_title(title: &str) -> String {
    let trimmed = title.split_whitespace().collect::<Vec<_>>().join(" ");
    crate::core::text::truncate_chars(trimmed.trim(), 200)
}

/// 重命名文件时保留已有的时间戳前缀。
fn file_stem_with_prefix(relative: &str, title: &str) -> String {
    let current = file_name_of(relative);
    let stem = current
        .trim_end_matches(".md")
        .trim_end_matches(".markdown");
    let safe_title = crate::core::text::sanitize_file_name(title);
    match stem.find('-') {
        Some(index) if index == 15 => {
            let prefix = &stem[..index];
            format!("{prefix}-{safe_title}")
        }
        _ => crate::core::text::note_file_name(title, &now_file_timestamp()).replace(".md", ""),
    }
}

/// 校验相对路径属于工作区（供命令层复用）。
pub fn ensure_inside_workspace(paths: &WorkspacePaths, relative: &str) -> Result<()> {
    paths.resolve_relative(relative).map(|_| ())
}
