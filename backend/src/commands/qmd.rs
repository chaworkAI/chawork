//! Tauri commands for QMD knowledge-base operations.

use std::sync::Arc;

use tauri::State;

use crate::services::{qmd_index, workspace as workspace_svc};
use crate::state::AppState;

#[tauri::command]
pub async fn qmd_initialize(app_state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let ws = app_state.require_active_workspace()?;
    tokio::task::spawn_blocking(move || {
        let r = qmd_index::initialize_qmd(&ws);
        let _ = workspace_svc::sync_workspace_index_status(&ws);
        r
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}

#[tauri::command]
pub async fn qmd_refresh(app_state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let ws = app_state.require_active_workspace()?;
    tokio::task::spawn_blocking(move || {
        let r = qmd_index::refresh_index(&ws);
        let _ = workspace_svc::sync_workspace_index_status(&ws);
        r
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}

#[tauri::command]
pub async fn qmd_status(
    app_state: State<'_, Arc<AppState>>,
) -> Result<qmd_index::QmdStatus, String> {
    let ws = app_state.require_active_workspace()?;
    tokio::task::spawn_blocking(move || qmd_index::get_index_status(&ws))
        .await
        .map_err(|e| format!("task join error: {e}"))?
}

#[tauri::command]
pub async fn qmd_search(
    app_state: State<'_, Arc<AppState>>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<qmd_index::QmdSearchResult>, String> {
    let ws = app_state.require_active_workspace()?;
    tokio::task::spawn_blocking(move || qmd_index::search(&ws, &query, limit))
        .await
        .map_err(|e| format!("task join error: {e}"))?
}

#[tauri::command]
pub async fn qmd_get_document(
    app_state: State<'_, Arc<AppState>>,
    file_path: String,
) -> Result<String, String> {
    let ws = app_state.require_active_workspace()?;
    tokio::task::spawn_blocking(move || qmd_index::get_document(&ws, &file_path))
        .await
        .map_err(|e| format!("task join error: {e}"))?
}

/// Check the stale marker and refresh the index if needed. Returns true if a refresh was triggered.
#[tauri::command]
pub async fn qmd_refresh_if_stale(app_state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let ws = app_state.require_active_workspace()?;
    tokio::task::spawn_blocking(move || {
        let r = qmd_index::refresh_if_stale(&ws);
        let _ = workspace_svc::sync_workspace_index_status(&ws);
        r
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}
