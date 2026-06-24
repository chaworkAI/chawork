//! Tauri commands for the Wiki Knowledge Build import pipeline.
//!
//! `import_file` is fire-and-poll: it creates a task synchronously, returns
//! the task id immediately, and spawns the pipeline in the background. The
//! frontend polls `get_import_task` / `list_import_tasks` for status.
//!
//! Per-workspace serialization: each workspace gets its own async mutex
//! (stored in `AppState::import_queues`) so multiple submissions for the
//! same workspace queue up rather than racing for `wiki/` writes and qmd
//! index locks.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tokio::sync::Mutex as AsyncMutex;

use crate::services::import as import_svc;
use crate::state::AppState;

fn active_workspace(state: &AppState) -> Result<PathBuf, String> {
    state.require_active_workspace()
}

fn workspace_queue(app_state: &AppState, workspace: &std::path::Path) -> Arc<AsyncMutex<()>> {
    let mut map = app_state.lock_import_queues();
    map.entry(workspace.to_path_buf())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Submit an import and get back a task id immediately. The pipeline runs in
/// the background; poll `get_import_task(task_id)` for progress.
#[tauri::command]
pub async fn import_file(
    app_state: State<'_, Arc<AppState>>,
    source_path: String,
) -> Result<String, String> {
    let ws = active_workspace(&app_state)?;
    let src = PathBuf::from(&source_path);
    if !src.is_file() {
        return Err(format!("文件不存在: {source_path}"));
    }

    // Create task synchronously so we can return the id straight away.
    let task = {
        let ws_local = ws.clone();
        let src_local = src.clone();
        tokio::task::spawn_blocking(move || import_svc::create_task(&ws_local, &src_local))
            .await
            .map_err(|e| format!("task join error: {e}"))??
    };
    let task_id = task.manifest.id.clone();

    // Background pipeline: acquire the workspace queue, then run.
    let app_state_arc: Arc<AppState> = (*app_state).clone();
    let ws_clone = ws.clone();
    let task_id_clone = task_id.clone();
    tauri::async_runtime::spawn(async move {
        let queue = workspace_queue(&app_state_arc, &ws_clone);
        let _guard = queue.lock().await;
        let ws_for_blocking = ws_clone.clone();
        let task_for_blocking = task_id_clone.clone();
        let _ = tokio::task::spawn_blocking(move || {
            import_svc::run_pipeline(&ws_for_blocking, &task_for_blocking)
        })
        .await;
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn get_import_task(
    app_state: State<'_, Arc<AppState>>,
    task_id: String,
) -> Result<import_svc::ImportTask, String> {
    let ws = active_workspace(&app_state)?;
    tokio::task::spawn_blocking(move || import_svc::get_task(&ws, &task_id))
        .await
        .map_err(|e| format!("task join error: {e}"))?
}

#[tauri::command]
pub async fn list_import_tasks(
    app_state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<import_svc::ImportTask>, String> {
    let ws = active_workspace(&app_state)?;
    let n = limit.unwrap_or(50);
    Ok(
        tokio::task::spawn_blocking(move || import_svc::list_tasks(&ws, n))
            .await
            .map_err(|e| format!("task join error: {e}"))?,
    )
}

/// Legacy: flat ImportRecord feed from `logs/import/imports.jsonl`.
#[tauri::command]
pub async fn list_imports(
    app_state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<import_svc::ImportRecord>, String> {
    let ws = active_workspace(&app_state)?;
    let n = limit.unwrap_or(20);
    Ok(import_svc::list_imports(&ws, n))
}
