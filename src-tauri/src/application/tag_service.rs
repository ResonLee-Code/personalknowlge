//! 标签服务（设计文档 5.4）。

use std::collections::HashSet;
use std::sync::Arc;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::ids::tag_id;
use crate::core::text::{normalize_tag_name, tag_key};
use crate::core::time::now_iso;
use crate::domain::tag::Tag;
use crate::repositories::{NoteRepository, SqliteNoteRepository, SqliteTagRepository, TagRepository};

use super::context::WorkspaceContext;
use super::index_service::IndexService;

pub struct TagService {
    ctx: Arc<WorkspaceContext>,
}

impl TagService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    fn workspace_id(&self) -> &str {
        &self.ctx.workspace.id
    }

    /// 规范化标签名：去空白、按大小写不敏感去重（FR-T01）。
    pub fn normalize_names(&self, names: Vec<String>) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for name in names {
            let normalized = normalize_tag_name(&name);
            if normalized.is_empty() {
                continue;
            }
            if seen.insert(tag_key(&normalized)) {
                out.push(normalized);
            }
        }
        out
    }

    pub fn list(&self) -> Result<Vec<Tag>> {
        self.ctx
            .db
            .with_conn(|conn| SqliteTagRepository.list(conn, self.workspace_id()))
    }

    /// 读取或创建标签。
    pub fn get_or_create(&self, name: &str) -> Result<Tag> {
        let normalized = normalize_tag_name(name);
        if normalized.is_empty() {
            return Err(AppError::invalid_input("标签名不能为空。"));
        }
        let key = tag_key(&normalized);
        let existing = self
            .ctx
            .db
            .with_conn(|conn| SqliteTagRepository.find_by_key(conn, self.workspace_id(), &key))?;
        if let Some(tag) = existing {
            return Ok(tag);
        }
        let tag = Tag {
            id: tag_id(),
            workspace_id: self.workspace_id().to_string(),
            name: normalized,
            created_at: now_iso(),
            note_count: 0,
        };
        self.ctx
            .db
            .with_conn(|conn| SqliteTagRepository.insert(conn, &tag))?;
        Ok(tag)
    }

    /// 为笔记附加一个标签（不存在时创建）。
    pub fn ensure_tag(&self, note_id: &str, name: &str) -> Result<Tag> {
        let tag = self.get_or_create(name)?;
        self.ctx.db.with_conn(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                rusqlite::params![note_id, tag.id],
            )?;
            Ok(())
        })?;
        Ok(tag)
    }

    /// 覆盖设置笔记标签（FR-T03）。
    pub fn set_note_tags(&self, note_id: &str, names: &[String]) -> Result<Vec<Tag>> {
        let mut tags = Vec::new();
        for name in names {
            tags.push(self.get_or_create(name)?);
        }
        let ids: Vec<String> = tags.iter().map(|tag| tag.id.clone()).collect();
        self.ctx
            .db
            .with_conn(|conn| SqliteTagRepository.set_note_tags(conn, note_id, &ids))?;
        Ok(tags)
    }

    pub fn rename(&self, id: &str, new_name: &str) -> Result<Tag> {
        let normalized = normalize_tag_name(new_name);
        if normalized.is_empty() {
            return Err(AppError::invalid_input("标签名不能为空。"));
        }
        let conflict = self.ctx.db.with_conn(|conn| {
            SqliteTagRepository.find_by_key(conn, self.workspace_id(), &tag_key(&normalized))
        })?;
        if let Some(existing) = conflict {
            if existing.id != id {
                return Err(AppError::new(
                    ErrorCode::CategoryNameConflict,
                    "同名标签已存在。",
                ));
            }
        }
        self.ctx.db.with_conn(|conn| {
            conn.execute(
                "UPDATE tags SET name = ?2 WHERE id = ?1",
                rusqlite::params![id, normalized],
            )?;
            Ok(())
        })?;
        let tag = self
            .ctx
            .db
            .with_conn(|conn| SqliteTagRepository.get(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::TagNotFound, "标签不存在。"))?;

        // 标签参与索引关键词，需要重建受影响笔记的索引
        let index = IndexService::new(Arc::clone(&self.ctx));
        let note_ids = self.ctx.db.with_conn(|conn| {
            let mut statement = conn.prepare("SELECT note_id FROM note_tags WHERE tag_id = ?1")?;
            let rows = statement.query_map(rusqlite::params![id], |row| row.get::<_, String>(0))?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row?);
            }
            Ok(ids)
        })?;
        for note_id in note_ids {
            if let Ok(note) = self
                .ctx
                .db
                .with_conn(|conn| SqliteNoteRepository.get(conn, &note_id))
            {
                if let Some(note) = note {
                    let _ = index.reindex_note(&note);
                }
            }
        }
        Ok(tag)
    }

    /// 删除标签（FR-T02：不删除笔记正文）。
    pub fn delete(&self, id: &str) -> Result<()> {
        let affected = self.ctx.db.with_conn(|conn| {
            let mut statement = conn.prepare("SELECT note_id FROM note_tags WHERE tag_id = ?1")?;
            let rows = statement.query_map(rusqlite::params![id], |row| row.get::<_, String>(0))?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row?);
            }
            Ok(ids)
        })?;
        self.ctx
            .db
            .with_conn(|conn| SqliteTagRepository.delete(conn, id))?;

        let index = IndexService::new(Arc::clone(&self.ctx));
        for note_id in affected {
            if let Ok(Some(note)) = self
                .ctx
                .db
                .with_conn(|conn| SqliteNoteRepository.get(conn, &note_id))
            {
                let _ = index.reindex_note(&note);
            }
        }
        Ok(())
    }
}
