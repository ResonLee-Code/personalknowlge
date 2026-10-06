//! 应用层：编排业务流程（设计文档 8.2）。
//!
//! 每个服务都围绕一个已打开的工作区（`WorkspaceContext`）工作。

pub mod backup_service;
pub mod category_service;
pub mod context;
pub mod index_service;
pub mod import_service;
pub mod lookup;
pub mod note_service;
pub mod registry;
pub mod search_service;
pub mod tag_service;
pub mod workspace_service;

pub use backup_service::BackupService;
pub use category_service::CategoryService;
pub use context::WorkspaceContext;
pub use index_service::{IndexService, ScanReport};
pub use import_service::{ImportNotesInput, ImportReport, ImportService};
pub use lookup::CategoryLookup;
pub use note_service::NoteService;
pub use registry::WorkspaceRegistryStore;
pub use search_service::SearchService;
pub use tag_service::TagService;
pub use workspace_service::WorkspaceService;
