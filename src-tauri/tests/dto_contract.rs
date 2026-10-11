//! DTO 契约测试。
//!
//! 前端只会发送界面实际用到的字段，后端必须容忍缺省字段，
//! 否则会出现「invalid args」这类只在实际运行中才暴露的错误。
//! 这里的 JSON 片段与 `docs/API_CONTRACT.md` 中记录的调用保持一致。

use personal_knowledge_os_lib::domain::backup::{BackupCreateInput, ExportMarkdownInput};
use personal_knowledge_os_lib::application::import_service::ImportNotesInput;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, NoteListQuery, NoteSort, NoteStatus, SaveNoteInput,
};
use personal_knowledge_os_lib::domain::research::ResearchRequest;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::domain::settings::UpdateSettingsInput;

#[test]
fn note_list_query_accepts_partial_payload() {
    // 前端刷新列表时只发送排序与分页
    let query: NoteListQuery = serde_json::from_str(r#"{ "sort": "updatedDesc", "pageSize": 300 }"#)
        .expect("列表查询应接受缺省字段");
    assert_eq!(query.page, None);
    assert_eq!(query.page_size, Some(300));
    assert_eq!(query.sort, Some(NoteSort::UpdatedDesc));
    assert!(!query.favorite_only);
    assert!(!query.inbox_only);
    assert_eq!(query.status, None);

    // 空对象也必须可用
    let empty: NoteListQuery = serde_json::from_str("{}").expect("空查询应可用");
    assert_eq!(empty.status, None);
}

#[test]
fn note_list_query_accepts_null_and_status_filters() {
    let query: NoteListQuery = serde_json::from_str(
        r#"{ "categoryId": null, "tagId": null, "status": "deleted", "inboxOnly": true, "keyword": null }"#,
    )
    .expect("分类与标签为空时应可用");
    assert_eq!(query.status, Some(NoteStatus::Deleted));
    assert!(query.inbox_only);
    assert_eq!(query.category_id, None);
    assert_eq!(query.keyword, None);
}

#[test]
fn create_note_input_requires_only_title() {
    let input: CreateNoteInput =
        serde_json::from_str(r#"{ "title": "标题" }"#).expect("只需标题");
    assert_eq!(input.title, "标题");
    assert!(input.category_id.is_none());
    assert!(input.initial_content.is_none());
    assert!(input.tags.is_none());
}

#[test]
fn save_note_input_accepts_null_optionals() {
    let input: SaveNoteInput = serde_json::from_str(
        r#"{ "id": "note_1", "content": "正文", "baseContentHash": null, "title": null, "categoryId": null, "tags": null }"#,
    )
    .expect("保存输入应接受 null 可选字段");
    assert_eq!(input.id, "note_1");
    assert!(input.base_content_hash.is_none());
    assert!(input.tags.is_none());
}

#[test]
fn search_query_accepts_partial_payload() {
    let query: SearchQuery = serde_json::from_str(
        r#"{ "keyword": "全文", "categoryId": null, "favoriteOnly": false, "sort": "relevance", "page": 1, "pageSize": 50 }"#,
    )
    .expect("搜索查询应接受缺省字段");
    assert_eq!(query.keyword, "全文");
    assert_eq!(query.updated_before, None);
    assert_eq!(query.scope_note_id, None);
}

#[test]
fn settings_and_category_inputs_accept_partial_payload() {
    let settings: UpdateSettingsInput =
        serde_json::from_str(r#"{ "autosaveDelayMs": 1200 }"#).expect("设置更新应接受部分字段");
    assert_eq!(settings.autosave_delay_ms, Some(1200));
    assert!(settings.theme.is_none());

    let category: CreateCategoryInput =
        serde_json::from_str(r#"{ "name": "数据库" }"#).expect("分类创建只需名称");
    assert_eq!(category.name, "数据库");
    assert!(category.parent_id.is_none());
    assert!(category.create_directory.is_none());
}

#[test]
fn backup_inputs_accept_partial_payload() {
    let backup: BackupCreateInput =
        serde_json::from_str(r#"{ "targetDir": null, "note": null }"#).expect("备份输入应接受 null");
    assert!(backup.target_dir.is_none());

    let export: ExportMarkdownInput =
        serde_json::from_str(r#"{ "targetDir": "/tmp/export", "includeAttachments": true }"#)
            .expect("导出输入应可解析");
    assert!(export.include_attachments);

    let export_minimal: ExportMarkdownInput =
        serde_json::from_str(r#"{ "targetDir": "/tmp/export" }"#)
            .expect("导出输入应容忍缺省字段");
    assert!(!export_minimal.include_attachments);
}

#[test]
fn import_input_accepts_paths_without_category() {
    // 前端在「导入到 Inbox」时不发送 categoryId
    let input: ImportNotesInput = serde_json::from_str(r#"{ "paths": ["/tmp/a.md", "/tmp/b.txt"] }"#)
        .expect("导入输入应接受缺省分类");
    assert_eq!(input.paths.len(), 2);
    assert!(input.category_id.is_none());

    let with_category: ImportNotesInput =
        serde_json::from_str(r#"{ "paths": ["/tmp/a.md"], "categoryId": "cat_1" }"#)
            .expect("带分类的导入输入应可解析");
    assert_eq!(with_category.category_id.as_deref(), Some("cat_1"));
}

#[test]
fn settings_input_accepts_fractional_pixel_values() {
    // Retina 屏拖拽分栏时 clientX 可能是小数（如 320.5），后端应四舍五入而不是报错
    let input: UpdateSettingsInput = serde_json::from_str(
        r#"{ "sidebarWidth": 320.5, "listWidth": 400.4, "autosaveDelayMs": 1000.0, "editorSplitRatio": 0.512 }"#,
    )
    .expect("小数像素值应被接受");
    assert_eq!(input.sidebar_width, Some(321));
    assert_eq!(input.list_width, Some(400));
    assert_eq!(input.autosave_delay_ms, Some(1000));
    assert_eq!(input.editor_split_ratio, Some(0.512));

    // 整数输入同样可用
    let integers: UpdateSettingsInput =
        serde_json::from_str(r#"{ "sidebarWidth": 248, "editorSplitRatio": 0 }"#)
            .expect("整数应可用");
    assert_eq!(integers.sidebar_width, Some(248));
    assert_eq!(integers.editor_split_ratio, Some(0.0));
}

#[test]
fn research_request_accepts_a_bare_topic() {
    // 研究助手的第一步只有课题是必填，其余（范围/时间窗/写回意图）都可能缺省
    let input: ResearchRequest =
        serde_json::from_str(r#"{ "topic": "Rust 异步运行时调度" }"#).expect("只给课题也要能用");
    assert_eq!(input.topic, "Rust 异步运行时调度");
    assert!(input.scope.is_none());
    assert!(input.time_window_days.is_none());
    assert!(!input.write_back);
    assert!(input.target_category_id.is_none());

    let full: ResearchRequest = serde_json::from_str(
        r#"{ "topic": "Rust 异步运行时调度", "scope": "运行时实现", "timeWindowDays": 365,
             "writeBack": true, "targetCategoryId": "cat_1" }"#,
    )
    .expect("完整输入应可解析");
    assert_eq!(full.time_window_days, Some(365));
    assert!(full.write_back);
    assert_eq!(full.target_category_id.as_deref(), Some("cat_1"));
}

// ---------------------------------------------------------------- 响应结构契约

/// 这些是前端界面真正读取的 DTO：TS 接口里的每个字段都必须能在 Rust 结构里找到，
/// 否则改名之后界面只会渲染 `undefined`（浏览器预览用 mock，看不出来）。
const RESPONSE_SHAPES: &[(&str, &str, &str)] = &[
    ("src-tauri/src/domain/note.rs", "Note", "Note"),
    ("src-tauri/src/domain/note.rs", "NoteSummary", "NoteSummary"),
    ("src-tauri/src/domain/note.rs", "NoteDetail", "NoteDetail"),
    ("src-tauri/src/domain/note.rs", "ReviewDueNote", "ReviewDueNote"),
    ("src-tauri/src/domain/category.rs", "Category", "Category"),
    ("src-tauri/src/domain/category.rs", "CategoryNode", "CategoryNode"),
    ("src-tauri/src/domain/tag.rs", "Tag", "Tag"),
    ("src-tauri/src/domain/search.rs", "SearchResponse", "SearchResponse"),
    ("src-tauri/src/domain/search.rs", "SearchHit", "SearchHit"),
    ("src-tauri/src/domain/search.rs", "IndexStatus", "IndexStatus"),
    ("src-tauri/src/domain/graph.rs", "GraphData", "GraphData"),
    ("src-tauri/src/domain/graph.rs", "GraphNode", "GraphNode"),
    ("src-tauri/src/domain/graph.rs", "GraphEdge", "GraphEdge"),
    ("src-tauri/src/domain/link.rs", "Backlink", "Backlink"),
    ("src-tauri/src/domain/link.rs", "OutgoingLink", "OutgoingLink"),
    ("src-tauri/src/domain/link.rs", "LinkCounts", "LinkCounts"),
    ("src-tauri/src/domain/ai.rs", "AiPolicy", "AiPolicy"),
    ("src-tauri/src/domain/ai.rs", "AiAuditEntry", "AiAuditEntry"),
    ("src-tauri/src/domain/ai.rs", "ResearchBudget", "AiResearchBudget"),
    ("src-tauri/src/domain/research.rs", "ResearchSession", "ResearchSession"),
    ("src-tauri/src/domain/research.rs", "ResearchPlan", "ResearchPlan"),
    ("src-tauri/src/domain/research.rs", "ResearchSource", "ResearchSource"),
    ("src-tauri/src/domain/research.rs", "ResearchClaim", "ResearchClaim"),
    ("src-tauri/src/domain/research.rs", "ResearchReport", "ResearchReport"),
    ("src-tauri/src/domain/research.rs", "ResearchProposal", "ResearchProposal"),
    (
        "src-tauri/src/domain/research.rs",
        "ResearchSessionSummary",
        "ResearchSessionSummary",
    ),
    ("src-tauri/src/domain/settings.rs", "AppSettings", "AppSettings"),
    ("src-tauri/src/domain/workspace.rs", "Workspace", "Workspace"),
    ("src-tauri/src/domain/backup.rs", "BackupInfo", "BackupInfo"),
];

fn repo_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("仓库根目录")
        .join(relative)
}

/// 取结构体字段名（转成 camelCase，与 serde `rename_all = "camelCase"` 对齐）。
fn rust_struct_fields(relative: &str, struct_name: &str) -> std::collections::BTreeSet<String> {
    let source = std::fs::read_to_string(repo_path(relative)).expect("读 Rust 领域文件");
    let marker = format!("pub struct {struct_name} {{");
    let start = source.find(&marker).unwrap_or_else(|| panic!("找不到 {struct_name}"));
    let body_start = start + marker.len();
    let body_end = source[body_start..]
        .find("\n}")
        .map(|offset| body_start + offset)
        .unwrap_or(source.len());
    let body = &source[body_start..body_end];
    let mut fields = std::collections::BTreeSet::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() {
            fields.insert(to_camel(name));
        }
    }
    fields
}

/// 取 TS 接口字段名（`name:` / `name?:` 两种写法）。
fn ts_interface_fields(interface_name: &str) -> std::collections::BTreeSet<String> {
    let source = std::fs::read_to_string(repo_path("src/types/index.ts")).expect("读 TS 类型");
    let marker = format!("export interface {interface_name} {{");
    let start = source.find(&marker).unwrap_or_else(|| panic!("找不到 {interface_name}"));
    let body_start = start + marker.len();
    let body_end = source[body_start..]
        .find("\n}")
        .map(|offset| body_start + offset)
        .unwrap_or(source.len());
    let body = &source[body_start..body_end];
    let mut fields = std::collections::BTreeSet::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("*") {
            continue;
        }
        let cleaned = trimmed.trim_end_matches(';').trim();
        let Some((name, _)) = cleaned.split_once(':') else {
            continue;
        };
        // 可选字段写成 `name?: T`：`?` 要在拆冒号之后再剥掉
        let name = name.trim().trim_end_matches('?').trim();
        if !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            fields.insert(name.to_string());
        }
    }
    fields
}

fn to_camel(name: &str) -> String {
    let mut parts = name.split('_');
    let mut out = parts.next().unwrap_or_default().to_string();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    out
}

#[test]
fn typescript_interfaces_only_reference_existing_rust_fields() {
    let mut problems: Vec<String> = Vec::new();
    let mut total_fields = 0usize;
    for (rust_file, rust_struct, ts_interface) in RESPONSE_SHAPES {
        let rust_fields = rust_struct_fields(rust_file, rust_struct);
        let ts_fields = ts_interface_fields(ts_interface);
        total_fields += rust_fields.len();
        assert!(
            !rust_fields.is_empty() && !ts_fields.is_empty(),
            "{rust_struct} / {ts_interface} 解析为空（结构变了？）"
        );
        for field in ts_fields.difference(&rust_fields) {
            problems.push(format!(
                "{ts_interface}.{field} 在 Rust `{rust_struct}`（{rust_file}）里不存在——界面会拿到 undefined"
            ));
        }
    }
    // 29 个结构目前合计 238 个字段；留一点余量，防止解析失效导致「空集合比空集合」的假绿
    assert!(
        total_fields >= 220,
        "Rust 字段总数异常（{total_fields}），解析可能失效"
    );
    assert!(
        problems.is_empty(),
        "响应结构与前端类型不一致：\n{}",
        problems.join("\n")
    );
}

// ---------------------------------------------------------------- 事件名与错误码契约

#[test]
fn error_codes_match_the_frontend_union() {
    let rust_source = std::fs::read_to_string(repo_path("src-tauri/src/core/error.rs"))
        .expect("读 error.rs");
    // `=> "CODE"` 的映射表就是对外契约
    let mut rust_codes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for line in rust_source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("ErrorCode::") {
            if let Some((_, tail)) = rest.split_once("=>") {
                let code = tail.trim().trim_end_matches(',').trim_matches('"');
                if !code.is_empty() && code.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_') {
                    rust_codes.insert(code.to_string());
                }
            }
        }
    }

    let ts_source = std::fs::read_to_string(repo_path("src/types/index.ts")).expect("读 types");
    let start = ts_source.find("export type ErrorCode =").expect("找不到 ErrorCode 类型");
    let body_end = ts_source[start..].find(';').map(|offset| start + offset).unwrap_or(ts_source.len());
    let body = &ts_source[start..body_end];
    let ts_codes: std::collections::BTreeSet<String> =
        body.split('\'')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect();

    assert!(rust_codes.len() >= 20, "错误码解析异常：{}", rust_codes.len());
    assert!(ts_codes.len() >= 20, "TS 错误码解析异常：{}", ts_codes.len());
    let missing_in_ts: Vec<&String> = rust_codes.difference(&ts_codes).collect();
    let missing_in_rust: Vec<&String> = ts_codes.difference(&rust_codes).collect();
    assert!(
        missing_in_ts.is_empty() && missing_in_rust.is_empty(),
        "错误码不一致：TS 缺少 {missing_in_ts:?}；Rust 缺少 {missing_in_rust:?}"
    );
}

#[test]
fn event_names_match_the_frontend_constants() {
    let rust_source =
        std::fs::read_to_string(repo_path("src-tauri/src/core/events.rs")).expect("读 events.rs");
    let mut rust_events: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for line in rust_source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pub const ") {
            if let Some((_, value)) = rest.split_once('=') {
                let name = value.trim().trim_end_matches(';').trim().trim_matches('"');
                if name.contains("://") {
                    rust_events.insert(name.to_string());
                }
            }
        }
    }

    let ts_source = std::fs::read_to_string(repo_path("src/lib/constants.ts")).expect("读 constants");
    let start = ts_source.find("export const EVENTS").expect("找不到 EVENTS");
    let body_end = ts_source[start..].find("} as const").map(|offset| start + offset).unwrap_or(ts_source.len());
    let ts_events: std::collections::BTreeSet<String> = ts_source[start..body_end]
        .split('\'')
        .skip(1)
        .step_by(2)
        .filter(|value| value.contains("://"))
        .map(str::to_string)
        .collect();

    // 这里只做「解析没坏」的下限断言（新增事件是正常演进），
    // 关键是下面的集合相等：两端只要有一边漏写，实时刷新就会静默失效。
    assert!(
        rust_events.len() >= 3,
        "Rust 事件名解析异常（疑似解析器坏了）：{rust_events:?}"
    );
    assert_eq!(ts_events, rust_events, "事件名不一致（写错就是实时刷新静默失效）");
}
