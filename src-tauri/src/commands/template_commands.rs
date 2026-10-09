//! 本地模板命令（V1.1 M2-2 / F8）。

use tauri::State;

use crate::application::template_service::{
    RenderTemplateInput, RenderedTemplate, SaveTemplateInput, TemplateService, TemplateSummary,
};
use crate::core::error::Result;
use crate::state::AppState;

use super::run;

/// 列出 `.knowledge/templates/*.md`。
#[tauri::command]
pub async fn templates_list(state: State<'_, AppState>) -> Result<Vec<TemplateSummary>> {
    let context = state.inner.context()?;
    run(move || TemplateService::new(context).list()).await
}

/// 读取模板并替换占位符（返回渲染后的正文，由前端走常规新建流程写入）。
#[tauri::command]
pub async fn template_render(
    state: State<'_, AppState>,
    input: RenderTemplateInput,
) -> Result<RenderedTemplate> {
    let context = state.inner.context()?;
    run(move || TemplateService::new(context).render(input)).await
}

/// 把当前笔记正文另存为模板（对标语雀「另存为模板」）。
#[tauri::command]
pub async fn template_save(
    state: State<'_, AppState>,
    input: SaveTemplateInput,
) -> Result<TemplateSummary> {
    let context = state.inner.context()?;
    run(move || TemplateService::new(context).save(input)).await
}
