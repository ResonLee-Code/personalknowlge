//! 分类服务（设计文档 5.3）。

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::ids::category_id;
use crate::core::paths::{category_name_to_dir_name, INBOX_DIR};
use crate::core::time::now_iso;
use crate::domain::category::{
    Category, CategoryDeleteStrategy, CategoryNode, CreateCategoryInput, DeleteCategoryInput,
};
use crate::repositories::{
    CategoryRepository, NoteRepository, SqliteCategoryRepository, SqliteNoteRepository,
};

use super::context::WorkspaceContext;
use super::index_service::IndexService;
use super::note_service::NoteService;

/// 删除分类的结果说明。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDeleteReport {
    pub deleted_categories: i64,
    pub moved_notes: i64,
    pub unassigned_notes: i64,
    pub warnings: Vec<String>,
}

pub struct CategoryService {
    ctx: Arc<WorkspaceContext>,
}

impl CategoryService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    fn workspace_id(&self) -> &str {
        &self.ctx.workspace.id
    }

    pub fn list(&self) -> Result<Vec<Category>> {
        self.ctx.categories()
    }

    /// 分类树（含笔记数量）。
    pub fn tree(&self) -> Result<Vec<CategoryNode>> {
        let lookup = self.ctx.category_lookup()?;
        let counts = self.ctx.category_note_counts()?;
        Ok(lookup.tree(&counts))
    }

    /// 创建分类（FR-C01）。
    pub fn create(&self, input: CreateCategoryInput) -> Result<Category> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::invalid_input("分类名称不能为空。"));
        }
        if name.contains('/') || name.contains('\\') {
            return Err(AppError::invalid_input("分类名称不能包含斜杠。"));
        }
        if let Some(parent_id) = input.parent_id.as_deref() {
            if self
                .ctx
                .db
                .with_conn(|conn| SqliteCategoryRepository.get(conn, parent_id))?
                .is_none()
            {
                return Err(AppError::new(ErrorCode::CategoryNotFound, "父分类不存在。"));
            }
        }

        let duplicated = self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.name_exists(
                conn,
                self.workspace_id(),
                input.parent_id.as_deref(),
                &name,
                None,
            )
        })?;
        if duplicated {
            return Err(AppError::new(
                ErrorCode::CategoryNameConflict,
                "同级下已存在同名分类。",
            ));
        }

        let sort_order = self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.next_sort_order(
                conn,
                self.workspace_id(),
                input.parent_id.as_deref(),
            )
        })?;
        let create_directory = input.create_directory.unwrap_or(true);
        let dir_name = if create_directory {
            category_name_to_dir_name(&name, sort_order)
        } else {
            String::new()
        };

        let category = Category {
            id: category_id(),
            workspace_id: self.workspace_id().to_string(),
            parent_id: input.parent_id.clone(),
            name,
            sort_order,
            dir_name: dir_name.clone(),
            created_at: now_iso(),
            note_count: 0,
        };

        if create_directory {
            let lookup = self.ctx.category_lookup()?;
            let parent_directory = match input.parent_id.as_deref() {
                Some(parent) => lookup.directory_of(parent),
                None => String::new(),
            };
            let relative = if parent_directory.is_empty() {
                dir_name.clone()
            } else {
                format!("{parent_directory}/{dir_name}")
            };
            let path = self.ctx.paths.resolve_relative(&relative)?;
            std::fs::create_dir_all(path)?;
        }

        self.ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.insert(conn, &category))?;
        Ok(category)
    }

    /// 重命名分类（FR-C02）：同步重命名目录。
    pub fn rename(&self, id: &str, new_name: &str) -> Result<Category> {
        let category = self
            .ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.get(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "分类不存在。"))?;
        let name = new_name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::invalid_input("分类名称不能为空。"));
        }
        if name.contains('/') || name.contains('\\') {
            return Err(AppError::invalid_input("分类名称不能包含斜杠。"));
        }
        let duplicated = self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.name_exists(
                conn,
                self.workspace_id(),
                category.parent_id.as_deref(),
                &name,
                Some(id),
            )
        })?;
        if duplicated {
            return Err(AppError::new(
                ErrorCode::CategoryNameConflict,
                "同级下已存在同名分类。",
            ));
        }

        let lookup = self.ctx.category_lookup()?;
        let old_directory = lookup.directory_of(id);
        let new_dir_name = if category.dir_name.trim().is_empty() {
            String::new()
        } else {
            category_name_to_dir_name(&name, category.sort_order)
        };

        self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.update(
                conn,
                id,
                &name,
                &new_dir_name,
                category.parent_id.as_deref(),
                category.sort_order,
            )
        })?;

        if !old_directory.is_empty() && !new_dir_name.is_empty() {
            let updated = self.ctx.category_lookup()?;
            let new_directory = updated.directory_of(id);
            if new_directory != old_directory {
                self.rename_directory(&old_directory, &new_directory)?;
            }
        }

        let category = self
            .ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.get(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "分类不存在。"))?;
        Ok(category)
    }

    /// 移动分类到新的父节点。
    pub fn move_category(&self, id: &str, new_parent_id: Option<&str>) -> Result<Category> {
        let category = self
            .ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.get(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "分类不存在。"))?;
        if Some(id) == new_parent_id {
            return Err(AppError::new(
                ErrorCode::CategoryMoveInvalid,
                "不能把分类移动到自身之下。",
            ));
        }
        if let Some(parent) = new_parent_id {
            let descendants = self
                .ctx
                .db
                .with_conn(|conn| SqliteCategoryRepository.descendants(conn, id))?;
            if descendants.iter().any(|child| child == parent) {
                return Err(AppError::new(
                    ErrorCode::CategoryMoveInvalid,
                    "不能把分类移动到其子分类之下。",
                ));
            }
            if self
                .ctx
                .db
                .with_conn(|conn| SqliteCategoryRepository.get(conn, parent))?
                .is_none()
            {
                return Err(AppError::new(ErrorCode::CategoryNotFound, "目标父分类不存在。"));
            }
        }

        let lookup = self.ctx.category_lookup()?;
        let old_directory = lookup.directory_of(id);
        let sort_order = self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.next_sort_order(conn, self.workspace_id(), new_parent_id)
        })?;
        let dir_name = if category.dir_name.trim().is_empty() {
            String::new()
        } else {
            category_name_to_dir_name(&category.name, sort_order)
        };

        self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.update(
                conn,
                id,
                &category.name,
                &dir_name,
                new_parent_id,
                sort_order,
            )
        })?;

        if !old_directory.is_empty() && !dir_name.is_empty() {
            let updated = self.ctx.category_lookup()?;
            let new_directory = updated.directory_of(id);
            if new_directory != old_directory {
                self.rename_directory(&old_directory, &new_directory)?;
            }
        }

        self.ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.get(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "分类不存在。"))
    }

    /// 删除分类（FR-C03）：先处理子树与笔记，再移除目录。
    pub fn delete(&self, input: DeleteCategoryInput) -> Result<CategoryDeleteReport> {
        let category = self
            .ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.get(conn, &input.id))?
            .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "分类不存在。"))?;
        let mut ids = self
            .ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.descendants(conn, &input.id))?;
        ids.push(input.id.clone());

        let lookup = self.ctx.category_lookup()?;
        let mut report = CategoryDeleteReport {
            deleted_categories: ids.len() as i64,
            moved_notes: 0,
            unassigned_notes: 0,
            warnings: Vec::new(),
        };
        let note_service = NoteService::new(Arc::clone(&self.ctx));

        // 先处理笔记，再删除分类记录，避免外键置空导致信息丢失
        let notes = self.ctx.db.with_conn(|conn| {
            let mut statement = conn.prepare(
                "SELECT id, category_id FROM notes
                 WHERE workspace_id = ?1 AND category_id IS NOT NULL AND status <> 'deleted'",
            )?;
            let rows = statement.query_map(rusqlite::params![self.workspace_id()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut list = Vec::new();
            for row in rows {
                list.push(row?);
            }
            Ok(list)
        })?;

        let target_category: Option<String> = match input.strategy {
            CategoryDeleteStrategy::MoveToInbox => None,
            CategoryDeleteStrategy::MoveToParent => category.parent_id.clone(),
            CategoryDeleteStrategy::Unassign => None,
        };
        let notes_in_subtree: Vec<(String, String)> = notes
            .into_iter()
            .filter(|(_, category_id)| ids.contains(category_id))
            .collect();

        for (note, _) in &notes_in_subtree {
            let result = match input.strategy {
                CategoryDeleteStrategy::MoveToInbox
                | CategoryDeleteStrategy::MoveToParent => {
                    note_service.move_to_category(note, target_category.as_deref())
                        .map(|_| ())
                }
                CategoryDeleteStrategy::Unassign => self.unassign_note(note),
            };
            match result {
                Ok(()) => {
                    if input.strategy == CategoryDeleteStrategy::Unassign {
                        report.unassigned_notes += 1;
                    } else {
                        report.moved_notes += 1;
                    }
                }
                Err(error) => report
                    .warnings
                    .push(format!("处理笔记 {note} 失败：{}", error.message)),
            }
        }

        for id in ids.iter().rev() {
            self.ctx
                .db
                .with_conn(|conn| SqliteCategoryRepository.delete(conn, id))?;
        }

        // 清理空目录（Unassign 策略下目录可能仍包含笔记，忽略失败）
        for id in &ids {
            let directory = lookup.directory_of(id);
            if directory.is_empty() {
                continue;
            }
            if let Ok(path) = self.ctx.paths.resolve_relative(&directory) {
                if path.exists() {
                    let _ = std::fs::remove_dir(&path);
                }
            }
        }
        Ok(report)
    }

    /// 供工作区初始化使用：为已存在的目录创建分类。
    pub fn create_for_existing_directory(&self, directory_name: &str) -> Result<Category> {
        let name = crate::core::paths::dir_name_to_category_name(directory_name);
        let duplicated = self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.name_exists(conn, self.workspace_id(), None, &name, None)
        })?;
        if duplicated {
            return Err(AppError::new(
                ErrorCode::CategoryNameConflict,
                "同名分类已存在。",
            ));
        }
        let sort_order = self.ctx.db.with_conn(|conn| {
            SqliteCategoryRepository.next_sort_order(conn, self.workspace_id(), None)
        })?;
        let category = Category {
            id: category_id(),
            workspace_id: self.workspace_id().to_string(),
            parent_id: None,
            name: name.clone(),
            sort_order,
            dir_name: directory_name.to_string(),
            created_at: now_iso(),
            note_count: 0,
        };
        self.ctx
            .db
            .with_conn(|conn| SqliteCategoryRepository.insert(conn, &category))?;
        Ok(category)
    }

    /// 统计每个分类的笔记数量（含子分类）。
    pub fn note_counts(&self) -> Result<HashMap<String, i64>> {
        self.ctx.category_note_counts()
    }

    fn unassign_note(&self, note_id: &str) -> Result<()> {
        let note = self
            .ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, note_id))?
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "笔记不存在。"))?;
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.set_category(conn, note_id, None))?;
        IndexService::new(Arc::clone(&self.ctx)).reindex_note(&note)?;
        Ok(())
    }

    /// 目录整体改名，并同步数据库中的文件路径。
    fn rename_directory(&self, old_directory: &str, new_directory: &str) -> Result<()> {
        let old_path = self.ctx.paths.resolve_relative(old_directory)?;
        let new_path = self.ctx.paths.resolve_relative(new_directory)?;
        if !old_path.exists() {
            std::fs::create_dir_all(&new_path)?;
            return Ok(());
        }
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if new_path.exists() {
            // 目标已存在：逐个移动文件
            self.move_directory_contents(&old_path, &new_path)?;
            let _ = std::fs::remove_dir(&old_path);
        } else {
            std::fs::rename(&old_path, &new_path)?;
        }

        let notes = self.ctx.db.with_conn(|conn| {
            let mut statement = conn.prepare(
                "SELECT id, file_path FROM notes WHERE workspace_id = ?1 AND status <> 'deleted'",
            )?;
            let rows = statement.query_map(rusqlite::params![self.workspace_id()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut list = Vec::new();
            for row in rows {
                list.push(row?);
            }
            Ok(list)
        })?;

        let prefix = format!("{old_directory}/");
        for (note_id, file_path) in notes {
            if let Some(suffix) = file_path.strip_prefix(&prefix) {
                let updated = format!("{new_directory}/{suffix}");
                self.ctx.db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE notes SET file_path = ?2 WHERE id = ?1",
                        rusqlite::params![note_id, updated],
                    )?;
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    fn move_directory_contents(
        &self,
        from: &std::path::Path,
        to: &std::path::Path,
    ) -> Result<()> {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                std::fs::create_dir_all(&target)?;
                self.move_directory_contents(&entry.path(), &target)?;
                let _ = std::fs::remove_dir(entry.path());
            } else if target.exists() {
                let stem = target
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                let extension = target
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_string())
                    .unwrap_or_default();
                let unique = target.with_file_name(format!(
                    "{stem}-{}.{}",
                    crate::core::time::now_file_timestamp(),
                    extension
                ));
                std::fs::rename(entry.path(), unique)?;
            } else {
                std::fs::rename(entry.path(), target)?;
            }
        }
        Ok(())
    }

    /// Inbox 目录（未分类笔记位置）。
    pub fn inbox_directory(&self) -> &'static str {
        INBOX_DIR
    }
}
