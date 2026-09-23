//! 分类相关命令（设计文档 14.4）。

use tauri::State;

use crate::application::category_service::{CategoryDeleteReport, CategoryService};
use crate::core::error::Result;
use crate::domain::category::{
    Category, CategoryNode, CreateCategoryInput, DeleteCategoryInput,
};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn category_list(state: State<'_, AppState>) -> Result<Vec<Category>> {
    let context = state.inner.context()?;
    run(move || CategoryService::new(context).list()).await
}

#[tauri::command]
pub async fn category_tree(state: State<'_, AppState>) -> Result<Vec<CategoryNode>> {
    let context = state.inner.context()?;
    run(move || CategoryService::new(context).tree()).await
}

#[tauri::command]
pub async fn category_create(
    state: State<'_, AppState>,
    input: CreateCategoryInput,
) -> Result<Category> {
    let context = state.inner.context()?;
    run(move || CategoryService::new(context).create(input)).await
}

#[tauri::command]
pub async fn category_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Category> {
    let context = state.inner.context()?;
    run(move || CategoryService::new(context).rename(&id, &name)).await
}

#[tauri::command]
pub async fn category_move(
    state: State<'_, AppState>,
    id: String,
    parent_id: Option<String>,
) -> Result<Category> {
    let context = state.inner.context()?;
    run(move || CategoryService::new(context).move_category(&id, parent_id.as_deref())).await
}

#[tauri::command]
pub async fn category_delete(
    state: State<'_, AppState>,
    input: DeleteCategoryInput,
) -> Result<CategoryDeleteReport> {
    let context = state.inner.context()?;
    run(move || CategoryService::new(context).delete(input)).await
}
