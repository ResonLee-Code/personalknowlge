//! 应用层：编排业务流程（设计文档 8.2）。
//!
//! 每个服务都围绕一个已打开的工作区（`WorkspaceContext`）工作。

pub mod ai;
pub mod attachment_service;
pub mod backup_service;
pub mod batch_service;
pub mod category_service;
pub mod context;
pub mod export_service;
pub mod graph_service;
pub mod history_service;
pub mod index_service;
pub mod import_service;
pub mod import_transform;
pub mod lookup;
pub mod link_service;
pub mod note_service;
pub mod registry;
pub mod relation_service;
pub mod search_service;
pub mod tag_service;
pub mod template_service;
pub mod workspace_service;

pub use backup_service::BackupService;
pub use batch_service::{BatchDeleteInput, BatchMoveInput, BatchReport, BatchService, BatchTagInput};
pub use category_service::CategoryService;
pub use context::WorkspaceContext;
pub use export_service::{ExportFormat, ExportNoteInput, ExportNoteResult, ExportService};
pub use graph_service::GraphService;
pub use index_service::{IndexService, ScanReport};
pub use import_service::{ImportNotesInput, ImportReport, ImportService};
pub use lookup::CategoryLookup;
pub use link_service::LinkService;
pub use note_service::NoteService;
pub use registry::WorkspaceRegistryStore;
pub use relation_service::{AddRelationInput, RelationService, RelationView};
pub use search_service::SearchService;
pub use tag_service::TagService;
pub use template_service::{
    RenderTemplateInput, RenderedTemplate, TemplateService, TemplateSummary,
};
pub use workspace_service::WorkspaceService;
pub use attachment_service::{AttachmentService, SaveAttachmentInput, SavedAttachment};
