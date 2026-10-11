//! 工作区服务（设计文档 5.1 / FR-W01 ~ FR-W04）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use serde::{Deserialize, Serialize};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::ids::workspace_id;
use crate::core::logging;
use crate::core::paths::{WorkspacePaths, ARCHIVE_DIR, DEFAULT_CATEGORY_DIRS, INBOX_DIR};
use crate::core::time::now_iso;
use crate::domain::note::CreateNoteInput;
use crate::domain::search::IndexStatus;
use crate::domain::settings::{AppSettings, UpdateSettingsInput};
use crate::domain::workspace::{
    ActivityDay,Workspace, WorkspaceStats, WorkspaceSummary};
use crate::infrastructure::db::Db;
use crate::infrastructure::markdown_storage::{MarkdownStorage, RecentWrites};
use crate::infrastructure::watcher::{start_best_watcher, FileChangeBatch, WatchMode};
use crate::repositories::{
    SettingsRepository, SqliteSettingsRepository, SqliteWorkspaceRepository, WorkspaceRepository,
};
use crate::state::AppInner;

use super::category_service::CategoryService;
use super::context::WorkspaceContext;
use super::index_service::{IndexService, ScanReport};
use super::note_service::NoteService;
use super::search_service::SearchService;

/// 打开工作区的返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenResult {
    pub workspace: Workspace,
    pub stats: WorkspaceStats,
    pub settings: AppSettings,
    pub index_status: IndexStatus,
    pub warnings: Vec<String>,
    pub initialized: bool,
    pub scan: ScanReport,
}

pub struct WorkspaceService {
    app: Arc<AppInner>,
}

impl WorkspaceService {
    pub fn new(app: Arc<AppInner>) -> Self {
        Self { app }
    }

    /// 创建工作区（FR-W01）。
    pub fn create(&self, name: &str, root: &str) -> Result<WorkspaceOpenResult> {
        let root_path = WorkspacePaths::validate_root(Path::new(root))?;
        let paths = WorkspacePaths::new(root_path.clone());
        let database = paths.database_path();

        if database.exists() {
            let existing = Db::open(&database)?;
            let has_workspace = existing.with_conn(|conn| {
                let count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))?;
                Ok(count > 0)
            })?;
            if has_workspace {
                return Err(AppError::new(
                    ErrorCode::WorkspaceAlreadyExists,
                    "该目录已经是知识库，请使用「打开知识库」。",
                ));
            }
        }

        let name = if name.trim().is_empty() {
            default_workspace_name(&root_path)
        } else {
            name.trim().to_string()
        };

        self.close()?;
        paths.ensure_layout()?;
        logging::set_log_directory(&paths.log_dir());

        let db = Arc::new(Db::open(&database)?);
        let now = now_iso();
        let workspace = Workspace {
            id: workspace_id(),
            name,
            root_path: root_path.to_string_lossy().to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        db.with_conn(|conn| SqliteWorkspaceRepository.upsert(conn, &workspace))?;

        let context = self.build_context(workspace, db, paths)?;

        // 依据目录约定创建初始分类（设计文档 12.1）
        let category_service = CategoryService::new(Arc::clone(&context));
        for directory in DEFAULT_CATEGORY_DIRS {
            if let Err(error) = category_service.create_for_existing_directory(directory) {
                tracing::debug!(target: "workspace", "跳过默认分类 {directory}: {error}");
            }
        }
        let _ = category_service.create_for_existing_directory(ARCHIVE_DIR);

        self.write_config(&context)?;
        let note_service = NoteService::new(Arc::clone(&context));
        let _ = note_service.create(CreateNoteInput {
            title: "欢迎使用 Personal Knowledge OS".to_string(),
            category_id: None,
            initial_content: Some(welcome_note_body(&context.workspace.name)),
            parent_id: None,
            tags: Some(vec!["入门".to_string()]),
        });
        // 首次创建时放一个示例模板，让「新建笔记 → 模板」这一能力可被发现
        let _ = self.write_starter_template(&context);

        let index_status = IndexService::new(Arc::clone(&context)).rebuild()?;
        self.start_watcher(&context);
        self.app.set_active(Some(Arc::clone(&context)))?;
        self.register(&context.workspace)?;
        self.finish_open(&context, index_status, Vec::new(), true)
    }

    /// 打开已有工作区（FR-W02）。
    pub fn open(&self, root: &str) -> Result<WorkspaceOpenResult> {
        let root_path = WorkspacePaths::validate_root(Path::new(root))?;
        if !root_path.exists() {
            return Err(AppError::new(
                ErrorCode::WorkspaceNotFound,
                "目录不存在，请检查路径。",
            ));
        }
        let paths = WorkspacePaths::new(root_path.clone());
        let database = paths.database_path();
        let mut warnings: Vec<String> = Vec::new();
        let mut initialized = false;

        self.close()?;

        if !database.exists() {
            // 打开一个普通目录：就地初始化，并保留用户已有文件
            paths.ensure_layout()?;
            initialized = true;
        }
        logging::set_log_directory(&paths.log_dir());
        let db = Arc::new(Db::open(&database)?);

        let schema_version = db.schema_version()?;
        if schema_version > crate::infrastructure::db::SCHEMA_VERSION {
            return Err(AppError::new(
                ErrorCode::WorkspaceInvalid,
                "该知识库由更高版本的应用创建，请升级应用后再打开。",
            ));
        }

        let existing = db.with_conn(|conn| {
            SqliteWorkspaceRepository.get_by_root(conn, &root_path.to_string_lossy())
        })?;
        let workspace = match existing {
            Some(workspace) => workspace,
            None => match db.with_conn(|conn| SqliteWorkspaceRepository.first(conn))? {
                // 目录里已经有数据库、但库里没有「这个路径」的记录：这是**同一个知识库**
                // 被整目录拷贝 / 从备份恢复到了新位置（也是文档推荐的搬迁方式）。
                // 必须沿用原来的 id，只把 root_path 指到新位置——否则会新建一个 id，
                // 快照里的行全成孤儿：回收站、收藏这些**只存在数据库里**的状态会静默丢失。
                Some(mut workspace) => {
                    workspace.root_path = root_path.to_string_lossy().to_string();
                    workspace.updated_at = now_iso();
                    db.with_conn(|conn| SqliteWorkspaceRepository.upsert(conn, &workspace))?;
                    tracing::info!(
                        target: "workspace",
                        "沿用已有知识库身份 {}（路径已迁移到新位置）",
                        workspace.id
                    );
                    workspace
                }
                None => {
                    let now = now_iso();
                    let workspace = Workspace {
                        id: workspace_id(),
                        name: default_workspace_name(&root_path),
                        root_path: root_path.to_string_lossy().to_string(),
                        created_at: now.clone(),
                        updated_at: now,
                    };
                    db.with_conn(|conn| SqliteWorkspaceRepository.upsert(conn, &workspace))?;
                    initialized = true;
                    workspace
                }
            },
        };

        let context = self.build_context(workspace, db, paths)?;
        self.adopt_directories(&context, &mut warnings);
        self.write_config(&context)?;

        // 扫描 Markdown 文件，采纳新增与外部修改（设计文档 13.3）
        let index_service = IndexService::new(Arc::clone(&context));
        let scan = match index_service.scan_workspace() {
            Ok(scan) => {
                warnings.extend(scan.warnings.iter().cloned());
                scan
            }
            Err(error) => {
                warnings.push(format!("扫描工作区失败：{}", error.message));
                ScanReport::default()
            }
        };

        let index_status = match index_service.status() {
            Ok(status) if status.indexed_notes < status.total_notes => {
                index_service.rebuild().unwrap_or(status)
            }
            Ok(status) => status,
            Err(_) => index_service.rebuild()?,
        };

        self.start_watcher(&context);
        self.app.set_active(Some(Arc::clone(&context)))?;
        self.register(&context.workspace)?;

        let mut result = self.finish_open(&context, index_status, warnings, initialized)?;
        result.scan = scan;
        Ok(result)
    }

    /// 关闭当前工作区（FR-W04）：释放监听器与数据库连接。
    pub fn close(&self) -> Result<()> {
        if let Some(context) = self.app.active() {
            if let Ok(mut guard) = context.watcher.lock() {
                if let Some(mut watcher) = guard.take() {
                    watcher.stop();
                }
            }
            tracing::info!(target: "workspace", "关闭工作区 {}", context.workspace.name);
        }
        self.app.set_active(None)?;
        Ok(())
    }

    /// 切换工作区（FR-W03）。
    pub fn switch(&self, id: &str) -> Result<WorkspaceOpenResult> {
        let registry = self.app.registry_snapshot();
        let entry = registry
            .workspaces
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound, "找不到该知识库记录。"))?
            .clone();
        self.open(&entry.root_path)
    }

    /// 已知工作区列表。
    pub fn list(&self) -> Vec<WorkspaceSummary> {
        let registry = self.app.registry_snapshot();
        let active = self.app.active();
        let active_id = active.as_ref().map(|context| context.workspace.id.clone());
        let active_stats = active
            .as_ref()
            .and_then(|context| self.stats_of(context).ok())
            .unwrap_or_default();

        registry
            .workspaces
            .into_iter()
            .map(|entry| {
                let is_active = active_id.as_deref() == Some(entry.id.as_str());
                let is_available = PathBuf::from(&entry.root_path).exists();
                WorkspaceSummary {
                    workspace: Workspace {
                        id: entry.id,
                        name: entry.name,
                        root_path: entry.root_path,
                        created_at: entry.last_opened_at.clone(),
                        updated_at: entry.last_opened_at,
                    },
                    stats: if is_active {
                        active_stats.clone()
                    } else {
                        WorkspaceStats::default()
                    },
                    is_active,
                    is_available,
                }
            })
            .collect()
    }

    /// 当前工作区快照。
    pub fn current(&self) -> Result<Option<WorkspaceOpenResult>> {
        let Some(context) = self.app.active() else {
            return Ok(None);
        };
        let index_status = IndexService::new(Arc::clone(&context)).status()?;
        Ok(Some(self.finish_open(&context, index_status, Vec::new(), false)?))
    }

    pub fn stats(&self) -> Result<WorkspaceStats> {
        let context = self.app.context()?;
        self.stats_of(&context)
    }

    /// 更新设置（设计文档 16.7）。
    pub fn update_settings(&self, input: UpdateSettingsInput) -> Result<AppSettings> {
        let context = self.app.context()?;
        let settings = context.update_settings(|settings| apply_settings(settings, &input))?;
        if let Some(theme) = input.theme.clone() {
            let store = self.app.registry_store();
            let registry = store.update_global_settings(|global| {
                global.theme = theme;
            })?;
            self.app.set_registry(registry)?;
        }
        Ok(settings)
    }

    /// 手动扫描工作区（外部编辑后刷新）。
    pub fn scan(&self) -> Result<ScanReport> {
        let context = self.app.context()?;
        IndexService::new(context).scan_workspace()
    }

    /// 手动重建索引（FR-S07）。
    pub fn rebuild_index(&self) -> Result<IndexStatus> {
        let context = self.app.context()?;
        SearchService::new(context).rebuild_index()
    }

    /// 移除注册记录（不删除任何文件）。
    pub fn forget(&self, id: &str) -> Result<()> {
        let store = self.app.registry_store();
        let registry = store.remove(id)?;
        self.app.set_registry(registry)
    }

    fn build_context(
        &self,
        workspace: Workspace,
        db: Arc<Db>,
        paths: WorkspacePaths,
    ) -> Result<Arc<WorkspaceContext>> {
        let settings = db.with_conn(|conn| SqliteSettingsRepository.load(conn))?;
        let recent_writes = Arc::new(RecentWrites::default());
        let storage = Arc::new(MarkdownStorage::new(paths.clone(), recent_writes));
        Ok(WorkspaceContext::new(
            workspace,
            db,
            paths,
            storage,
            settings,
            Arc::clone(&self.app.events),
        ))
    }

    fn finish_open(
        &self,
        context: &Arc<WorkspaceContext>,
        index_status: IndexStatus,
        warnings: Vec<String>,
        initialized: bool,
    ) -> Result<WorkspaceOpenResult> {
        Ok(WorkspaceOpenResult {
            workspace: context.workspace.clone(),
            stats: self.stats_of(context)?,
            settings: context.settings(),
            index_status,
            warnings,
            initialized,
            scan: ScanReport::default(),
        })
    }

    fn stats_of(&self, context: &Arc<WorkspaceContext>) -> Result<WorkspaceStats> {
        let mut stats = context
            .db
            .with_conn(|conn| SqliteWorkspaceRepository.stats(conn, &context.workspace.id))?;
        stats.activity = context
            .db
            .with_conn(|conn| SqliteWorkspaceRepository.activity(conn, &context.workspace.id, 14))?
            .into_iter()
            .map(|(date, count)| ActivityDay { date, count })
            .collect();
        stats.attachment_count = context
            .storage
            .list_attachments()
            .map(|files| files.len() as i64)
            .unwrap_or(0);
        Ok(stats)
    }

    /// 为已存在的顶层目录建立分类记录。
    fn adopt_directories(&self, context: &Arc<WorkspaceContext>, warnings: &mut Vec<String>) {
        let service = CategoryService::new(Arc::clone(context));
        let known: std::collections::HashSet<String> = context
            .categories()
            .unwrap_or_default()
            .into_iter()
            .map(|category| category.dir_name.clone())
            .collect();
        let Ok(entries) = std::fs::read_dir(context.paths.root()) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == crate::core::paths::ATTACHMENTS_DIR {
                continue;
            }
            // Inbox 是「未分类」的虚拟位置，不作为分类出现
            if name == INBOX_DIR {
                continue;
            }
            if known.contains(&name) {
                continue;
            }
            let is_empty = entry
                .path()
                .read_dir()
                .map(|mut dir| dir.next().is_none())
                .unwrap_or(true);
            if is_empty {
                continue;
            }
            if let Err(error) = service.create_for_existing_directory(&name) {
                warnings.push(format!("目录 {name} 未能建立分类：{}", error.message));
            }
        }
    }

    /// 启动文件监听；监听失败不影响应用使用。
    fn start_watcher(&self, context: &Arc<WorkspaceContext>) {
        let weak: Weak<WorkspaceContext> = Arc::downgrade(context);
        let root = context.paths.root().to_path_buf();
        let handle = start_best_watcher(root, move |batch: FileChangeBatch| {
            if let Some(context) = weak.upgrade() {
                let service = IndexService::new(context);
                if let Err(error) = service.apply_file_changes(batch) {
                    tracing::warn!(target: "watcher", "处理文件变化失败：{error}");
                }
            }
        });
        match handle {
            Ok((handle, mode)) => {
                match mode {
                    WatchMode::Native => {
                        tracing::info!(target: "watcher", "已启用原生文件监听")
                    }
                    WatchMode::Polling => {
                        tracing::info!(target: "watcher", "已启用轮询式文件监听（间隔 3 秒）");
                        context.events.emit(
                            event_names::INDEX_STATUS,
                            serde_json::json!({
                                "warning": "文件系统事件不可用，已改用轮询方式同步外部修改（每 3 秒一次）。"
                            }),
                        );
                    }
                }
                if let Ok(mut guard) = context.watcher.lock() {
                    *guard = Some(handle);
                }
            }
            Err(error) => {
                tracing::warn!(target: "watcher", "文件监听启动失败（不影响使用）：{error}");
                context.events.emit(
                    event_names::INDEX_STATUS,
                    serde_json::json!({ "warning": format!("文件监听不可用：{}", error.message) }),
                );
            }
        }
    }

    fn register(&self, workspace: &Workspace) -> Result<()> {
        let store = self.app.registry_store();
        let registry = store.upsert(workspace)?;
        self.app.set_registry(registry)
    }

    /// 首次创建时放一个示例模板，让「新建笔记 → 模板」这一能力可被发现。
    /// 文件已存在时不覆盖，用户可以自由修改或删除。
    fn write_starter_template(&self, context: &Arc<WorkspaceContext>) -> Result<()> {
        let template_path = context.paths.templates_dir().join("会议记录.md");
        if template_path.exists() {
            return Ok(());
        }
        let body = "# {{title}}\n\n- 日期：{{date}}（{{weekday}}）\n- 参与人：\n\n\
                    ## 议题\n\n## 结论\n\n## 待办\n\n- [ ] \n";
        context
            .storage
            .write_atomic_absolute(&template_path, body.as_bytes())
    }

    fn write_config(&self, context: &Arc<WorkspaceContext>) -> Result<()> {
        let path = context.paths.config_path();
        let config = serde_json::json!({
            "app": "Personal Knowledge OS",
            "appVersion": env!("CARGO_PKG_VERSION"),
            "schemaVersion": crate::infrastructure::db::SCHEMA_VERSION,
            "workspaceId": context.workspace.id,
            "workspaceName": context.workspace.name,
            "rootPath": context.workspace.root_path,
            "createdAt": context.workspace.created_at,
            "updatedAt": now_iso(),
            "layout": {
                "inbox": INBOX_DIR,
                "knowledge": crate::core::paths::KNOWLEDGE_DIR,
                "trash": crate::core::paths::TRASH_DIR,
                "attachments": crate::core::paths::ATTACHMENTS_DIR
            }
        });
        let content = serde_json::to_string_pretty(&config)?;
        context
            .storage
            .write_atomic_absolute(&path, content.as_bytes())
    }
}

/// 合并设置更新（缺省字段保持不变）。
pub fn apply_settings(settings: &mut AppSettings, input: &UpdateSettingsInput) {
    if let Some(value) = input.autosave_enabled {
        settings.autosave_enabled = value;
    }
    if let Some(value) = input.autosave_delay_ms {
        settings.autosave_delay_ms = value.clamp(200, 10_000);
    }
    if let Some(value) = &input.editor_theme {
        // 只接受已知值：脏数据写进设置后界面会拿着它去查表，查不到就静默退化
        if ["light", "dark"].contains(&value.as_str()) {
            settings.editor_theme = value.clone();
        }
    }
    if let Some(value) = input.font_size {
        settings.font_size = value.clamp(10, 32);
    }
    if let Some(value) = &input.default_new_location {
        if ["inbox", "currentCategory", "lastCategory"].contains(&value.as_str()) {
            settings.default_new_location = value.clone();
        }
    }
    if let Some(value) = &input.backup_dir {
        settings.backup_dir = Some(value.clone());
    }
    if let Some(value) = &input.editor_mode {
        // 与 list_sort 同口径：未知值忽略（保留上一个有效值），不让编辑器落到未定义分支
        if ["edit", "split", "preview"].contains(&value.as_str()) {
            settings.editor_mode = value.clone();
        }
    }
    if let Some(value) = input.editor_split_ratio {
        settings.editor_split_ratio = value.clamp(0.3, 0.7);
    }
    if let Some(value) = input.sidebar_width {
        settings.sidebar_width = value.clamp(160, 480);
    }
    if let Some(value) = input.list_width {
        settings.list_width = value.clamp(220, 640);
    }
    if let Some(value) = &input.list_sort {
        // 只接受已知排序，避免把脏数据写进设置
        const ALLOWED: [&str; 5] = ["updatedDesc", "updatedAsc", "createdDesc", "titleAsc", "manual"];
        if ALLOWED.contains(&value.as_str()) {
            settings.list_sort = value.clone();
        }
    }
    if let Some(value) = &input.last_opened_note_id {
        settings.last_opened_note_id = Some(value.clone());
    }
    if let Some(value) = &input.last_category_id {
        settings.last_category_id = Some(value.clone());
    }
}

/// 用目录名作为默认工作区名称。
pub fn default_workspace_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "我的知识库".to_string())
}

/// 首次创建时写入的欢迎内容。
pub fn welcome_note_body(name: &str) -> String {
    format!(
        "# 欢迎使用 {name}\n\n\
         这是一个本地优先的个人知识库，所有笔记都以 Markdown 文件保存在这个目录中。\n\n\
         ## 常用操作\n\n\
         - `Cmd/Ctrl + N` 新建笔记\n\
         - `Cmd/Ctrl + K` 全局搜索\n\
         - `Cmd/Ctrl + S` 立即保存（默认已开启自动保存）\n\
         - 左侧分类树用于组织内容，未分类的笔记放在 `{inbox}` 目录\n\n\
         ## 文件位置\n\n\
         - 笔记正文：工作区目录下的 `.md` 文件\n\
         - 元数据与索引：`.knowledge/database.sqlite`\n\
         - 回收站：`.trash/`\n\n\
         你可以随时使用其他编辑器修改这些文件，应用会自动重新解析并更新索引。\n",
        name = name,
        inbox = INBOX_DIR
    )
}
