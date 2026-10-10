//! 待复核提醒（V1.2.1 阶段 D4 / N1 的 `valid_until` 生命周期）集成测试。
//!
//! 覆盖：已过期优先、临近到期按窗口纳入、窗口外不打扰、没有时效字段的笔记不出现、
//! 回收站里的笔记不算待复核、时效字段无法解析时跳过。

use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::core::time::{date_offset_days, today_date};
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SetSourceInput};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("待复习测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    /// 建一篇笔记；`valid_until` 为 `None` 表示不写时效字段。
    fn note(&self, title: &str, valid_until: Option<&str>) -> String {
        let notes = self.notes();
        let detail = notes
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some("正文".to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记");
        notes
            .set_source(SetSourceInput {
                id: detail.note.id.clone(),
                source_type: Some("official".to_string()),
                source_ref: None,
                source_reliability: Some(4),
                valid_until: valid_until.map(str::to_string),
            })
            .expect("写来源与时效");
        detail.note.id
    }
}

#[test]
fn due_for_review_orders_overdue_first_and_respects_the_window() {
    let fixture = Fixture::new();
    let overdue = fixture.note("过期的规范解读", Some(&date_offset_days(-10)));
    let soon = fixture.note("快到期的手册", Some(&date_offset_days(3)));
    let far = fixture.note("很久以后才到期", Some(&date_offset_days(120)));
    let none = fixture.note("没有时效字段的笔记", None);
    let broken = fixture.note("时效字段写坏了", Some("下个月吧"));

    let due = fixture.notes().due_for_review(14).expect("查询待复核");
    let ids: Vec<&str> = due.iter().map(|item| item.note_id.as_str()).collect();
    assert_eq!(ids, vec![overdue.as_str(), soon.as_str()], "只应包含已过期与 14 天内到期的笔记");
    assert_eq!(due[0].overdue_days, 10, "过期天数按自然日计算");
    assert_eq!(due[1].overdue_days, -3, "还有 3 天到期（负数）");
    assert_eq!(due[0].valid_until, date_offset_days(-10), "原样回显时效字段");
    assert!(due[0].category_path.is_empty(), "未分类 → 路径为空（Inbox）");
    assert!(!ids.contains(&far.as_str()), "窗口外不打扰");
    assert!(!ids.contains(&none.as_str()), "没有时效字段的笔记不算待复核");
    assert!(!ids.contains(&broken.as_str()), "无法解析的时效字段跳过（不误报）");

    // 窗口调大后，远期的也会进来
    let wide = fixture.notes().due_for_review(365).expect("宽窗口");
    assert!(wide.iter().any(|item| item.note_id == far), "窗口 365 天应包含 120 天后的笔记");

    // 窗口调小后，只剩已过期的
    let tight = fixture.notes().due_for_review(0).expect("零窗口");
    assert_eq!(tight.len(), 1);
    assert_eq!(tight[0].note_id, overdue);
    assert_eq!(tight[0].valid_until < today_date(), true, "零窗口只返回已经过期的");
}

#[test]
fn trashed_notes_are_not_due_for_review() {
    let fixture = Fixture::new();
    let notes = fixture.notes();
    let id = fixture.note("会被删掉的过期笔记", Some(&date_offset_days(-5)));
    assert_eq!(notes.due_for_review(7).expect("查询").len(), 1);

    notes.delete(&id).expect("移入回收站");
    assert!(
        notes.due_for_review(7).expect("查询").is_empty(),
        "回收站里的笔记不该再提醒复核"
    );

    notes.restore(&id).expect("从回收站恢复");
    assert_eq!(notes.due_for_review(7).expect("查询").len(), 1, "恢复后重新提醒");
}
