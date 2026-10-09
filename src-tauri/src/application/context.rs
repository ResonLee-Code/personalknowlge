//! 工作区运行时上下文。
//!
//! 保存一个已打开工作区的全部依赖：数据库、文件存储、监听器、事件与设置。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::EventSink;
use crate::core::paths::WorkspacePaths;
use crate::domain::category::Category;
use crate::domain::note::{Note, NoteSummary};
use crate::domain::settings::AppSettings;
use crate::domain::workspace::Workspace;
use crate::infrastructure::db::Db;
use crate::infrastructure::markdown_storage::MarkdownStorage;
use crate::infrastructure::watcher::WatcherHandle;
use crate::repositories::settings_repository::{
    INDEX_ERROR_KEY, INDEX_REBUILT_AT_KEY, INDEX_STATE_KEY,
};
use crate::repositories::{
    CategoryRepository, SettingsRepository, SqliteCategoryRepository, SqliteSettingsRepository,
    tag_repository::tag_names_for_notes,
};

use super::lookup::CategoryLookup;

pub struct WorkspaceContext {
    pub workspace: Workspace,
    pub paths: WorkspacePaths,
    pub db: Arc<Db>,
    pub storage: Arc<MarkdownStorage>,
    pub events: Arc<dyn EventSink>,
    pub settings: RwLock<AppSettings>,
    pub watcher: Mutex<Option<WatcherHandle>>,
    pub index_error: Mutex<Option<String>>,
    pub index_rebuilding: Mutex<bool>,
}

impl WorkspaceContext {
    pub fn new(
        workspace: Workspace,
        db: Arc<Db>,
        paths: WorkspacePaths,
        storage: Arc<MarkdownStorage>,
        settings: AppSettings,
        events: Arc<dyn EventSink>,
    ) -> Arc<Self> {
        Arc::new(Self {
            workspace,
            paths,
            db,
            storage,
            events,
            settings: RwLock::new(settings),
            watcher: Mutex::new(None),
            index_error: Mutex::new(None),
            index_rebuilding: Mutex::new(false),
        })
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace.id
    }

    pub fn settings(&self) -> AppSettings {
        self.settings
            .read()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    pub fn update_settings(&self, mutate: impl FnOnce(&mut AppSettings)) -> Result<AppSettings> {
        let updated = {
            let mut guard = self
                .settings
                .write()
                .map_err(|_| AppError::internal("设置读写锁异常。"))?;
            mutate(&mut guard);
            guard.clone()
        };
        self.db
            .with_conn(|conn| SqliteSettingsRepository.save(conn, &updated))?;
        Ok(updated)
    }

    /// 读取全部分类。
    pub fn categories(&self) -> Result<Vec<Category>> {
        self.db.with_conn(|conn| SqliteCategoryRepository.list(conn, &self.workspace.id))
    }

    pub fn category_lookup(&self) -> Result<CategoryLookup> {
        Ok(CategoryLookup::new(self.categories()?))
    }

    /// 每个分类的直接笔记数量（不含子分类）。
    pub fn category_note_counts(&self) -> Result<HashMap<String, i64>> {
        self.db.with_conn(|conn| {
            let mut counts: HashMap<String, i64> = HashMap::new();
            let mut statement = conn.prepare(
                "SELECT category_id, COUNT(*) FROM notes
                 WHERE workspace_id = ?1 AND status = 'active' AND category_id IS NOT NULL
                 GROUP BY category_id",
            )?;
            let rows = statement.query_map(rusqlite::params![self.workspace.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (category_id, count) = row?;
                counts.insert(category_id, count);
            }
            Ok(counts)
        })
    }

    /// 把笔记行转换为列表摘要（补充分类路径与标签）。
    pub fn to_summaries(&self, notes: Vec<Note>) -> Result<Vec<NoteSummary>> {
        let lookup = self.category_lookup()?;
        let ids: Vec<String> = notes.iter().map(|note| note.id.clone()).collect();
        let tag_map =
            self.db.with_conn(|conn| tag_names_for_notes(conn, &ids))?;

        Ok(notes
            .into_iter()
            .map(|note| NoteSummary {
                category_path: note
                    .category_id
                    .as_ref()
                    .map(|id| lookup.path_string(id))
                    .unwrap_or_default(),
                tags: tag_map.get(&note.id).cloned().unwrap_or_default(),
                id: note.id,
                title: note.title,
                file_path: note.file_path,
                category_id: note.category_id,
                status: note.status,
                favorite: note.favorite,
                word_count: note.word_count,
                created_at: note.created_at,
                updated_at: note.updated_at,
                parent_id: note.parent_id,
                sort_order: note.sort_order,
                pinned: note.pinned,
                snippet: Vec::new(),
            })
            .collect())
    }

    /// 记录索引状态（同时写入 settings 表，便于重启后读取）。
    pub fn set_index_error(&self, message: Option<String>) {
        if let Ok(mut guard) = self.index_error.lock() {
            *guard = message.clone();
        }
        let result = self.db.with_conn(|conn| match message.as_deref() {
            Some(message) => SqliteSettingsRepository.set_raw(conn, INDEX_ERROR_KEY, message),
            None => SqliteSettingsRepository.remove_raw(conn, INDEX_ERROR_KEY),
        });
        if let Err(error) = result {
            tracing::warn!(target: "index", "更新索引错误状态失败：{error}");
        }
    }

    pub fn index_error(&self) -> Option<String> {
        self.index_error
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .or_else(|| {
                self.db
                    .with_conn(|conn| SqliteSettingsRepository.get_raw(conn, INDEX_ERROR_KEY))
                    .ok()
                    .flatten()
            })
    }

    pub fn set_index_rebuilding(&self, value: bool) {
        if let Ok(mut guard) = self.index_rebuilding.lock() {
            *guard = value;
        }
    }

    pub fn is_index_rebuilding(&self) -> bool {
        self.index_rebuilding
            .lock()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    pub fn index_state_value(&self) -> Option<String> {
        self.db
            .with_conn(|conn| SqliteSettingsRepository.get_raw(conn, INDEX_STATE_KEY))
            .ok()
            .flatten()
    }

    pub fn last_rebuilt_at(&self) -> Option<String> {
        self.db
            .with_conn(|conn| SqliteSettingsRepository.get_raw(conn, INDEX_REBUILT_AT_KEY))
            .ok()
            .flatten()
    }

    /// 校验分类 ID 是否属于当前工作区。
    pub fn ensure_category_exists(&self, category_id: &str) -> Result<Category> {
        self.category_lookup()?
            .get(category_id)
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "指定的分类不存在。"))
    }
}
