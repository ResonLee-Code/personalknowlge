//! Ask 命令入口（V2.0 M1）。业务与隐私闸门都在 `application::ai::ask`。

use tauri::State;

use crate::application::ai::ask::AskService;
use crate::core::error::Result;
use crate::domain::ai::{AskAnswer, AskNotePreview, AskQuestionInput, AskSaveInput, AskSaveResult};
use crate::state::AppState;

use super::run;

/// 对当前笔记 / 分类 / 标签 / 全库提问，返回带精确原文引用的答案。
#[tauri::command]
pub async fn ask_question(
    state: State<'_, AppState>,
    input: AskQuestionInput,
) -> Result<AskAnswer> {
    let context = state.inner.context()?;
    run(move || AskService::new(context).answer(input)).await
}

/// 预览“把答案存为笔记”的标题、正文与来源；只读，不写文件。
#[tauri::command]
pub async fn ask_answer_note_preview(
    state: State<'_, AppState>,
    input: AskSaveInput,
) -> Result<AskNotePreview> {
    let context = state.inner.context()?;
    run(move || AskService::new(context).preview_answer_note(input)).await
}

/// 用户确认预览后写入普通 Markdown 笔记。
#[tauri::command]
pub async fn ask_answer_save(
    state: State<'_, AppState>,
    input: AskSaveInput,
) -> Result<AskSaveResult> {
    let context = state.inner.context()?;
    run(move || AskService::new(context).save_answer_note(input)).await
}
