//! 仓储层：隔离存储实现（设计文档 8.2）。
//!
//! 约定：仓储方法接收 `&Connection`，由应用服务统一获取连接并在必要时开启事务。
//! 这样既保证「UI 不直接访问数据库」，也避免同一连接被重复加锁。

pub mod category_repository;
pub mod note_repository;
pub mod settings_repository;
pub mod tag_repository;
pub mod workspace_repository;

pub use category_repository::{CategoryRepository, SqliteCategoryRepository};
pub use note_repository::{trash_path_of, NoteRepository, SqliteNoteRepository};
pub use settings_repository::{SettingsRepository, SqliteSettingsRepository};
pub use tag_repository::{SqliteTagRepository, TagRepository};
pub use workspace_repository::{SqliteWorkspaceRepository, WorkspaceRepository};
