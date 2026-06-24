use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::constants::DIALOG_CANCELLED;
use crate::services::{session as session_svc, skill as skill_svc, workspace as workspace_svc};
use crate::state::AppState;

#[derive(Clone, Serialize)]
pub struct SwitchWorkspaceResult {
    pub workspace: workspace_svc::WorkspaceState,
    pub sessions: Vec<session_svc::SessionMeta>,
    pub needs_skill_setup: bool,
}

#[tauri::command]
pub fn list_workspaces(
    app_state: State<Arc<AppState>>,
) -> Result<Vec<workspace_svc::WorkspaceState>, String> {
    let path = app_state.known_workspaces_file.clone();
    let mut list = workspace_svc::list_known_with_draft_counts(&path);
    for ws in &mut list {
        ws.bound_employee_id = crate::services::employee::bound_employee_id(Path::new(&ws.path));
        ws.bound_employee_name = ws.bound_employee_id.as_ref().and_then(|_| {
            crate::services::employee::bound_employee_name(&app_state.root, Path::new(&ws.path))
        });
    }
    Ok(list)
}

#[tauri::command]
pub fn create_workspace(
    app_state: State<Arc<AppState>>,
    name: String,
    path: String,
) -> Result<workspace_svc::WorkspaceState, String> {
    let pb = PathBuf::from(path.trim());
    std::fs::create_dir_all(&pb).map_err(|e| e.to_string())?;
    let mut ws = workspace_svc::open_or_create(pb.as_path())?;
    ws.name = name.trim().to_string();
    workspace_svc::persist_workspace(pb.as_path(), &ws)?;
    let _ = workspace_svc::sync_workspace_index_status(pb.as_path());
    workspace_svc::add_known(&app_state.known_workspaces_file, &ws)?;
    Ok(ws)
}

fn resolve_active_sessions(
    app_state: &AppState,
    workspace_path: &Path,
    workspace: &mut workspace_svc::WorkspaceState,
) -> Result<Vec<session_svc::SessionMeta>, String> {
    if let Some(ref sid) = workspace.active_session_id {
        let meta_path = workspace_path.join("sessions").join(sid).join("meta.json");
        if !meta_path.exists() {
            workspace.active_session_id = None;
            workspace_svc::persist_workspace(workspace_path, workspace)?;
            let mut gid = app_state.lock_active_session_id();
            *gid = None;
        }
    }

    session_svc::list(workspace_path)
}

fn prepare_workspace_from_path(
    app_state: &AppState,
    path: String,
) -> Result<(PathBuf, workspace_svc::WorkspaceState, bool), String> {
    let pb = PathBuf::from(path.trim());
    std::fs::metadata(&pb).map_err(|e| e.to_string())?;
    let pb = std::fs::canonicalize(&pb).unwrap_or(pb);
    crate::services::qmd_index::cleanup_legacy_artifacts(&pb);
    let ws = workspace_svc::open_or_create(pb.as_path())?;
    let _ = workspace_svc::sync_workspace_index_status(pb.as_path());
    workspace_svc::add_known(&app_state.known_workspaces_file, &ws)?;
    let needs_skill_setup = skill_svc::workspace_needs_skill_setup(pb.as_path(), &app_state.root);
    Ok((pb, ws, needs_skill_setup))
}

fn fill_workspace_binding_fields(
    root: &crate::services::root_workspace::RootWorkspace,
    ws: &mut workspace_svc::WorkspaceState,
    workspace_path: &Path,
) {
    ws.bound_employee_id = crate::services::employee::bound_employee_id(workspace_path);
    ws.bound_employee_name = ws
        .bound_employee_id
        .as_ref()
        .and_then(|_| crate::services::employee::bound_employee_name(root, workspace_path));
}

#[tauri::command]
pub fn register_workspace(
    app_state: State<Arc<AppState>>,
    path: String,
) -> Result<SwitchWorkspaceResult, String> {
    let (pb, mut ws, needs_skill_setup) = prepare_workspace_from_path(&app_state, path)?;
    fill_workspace_binding_fields(&app_state.root, &mut ws, pb.as_path());
    Ok(SwitchWorkspaceResult {
        workspace: ws,
        sessions: Vec::new(),
        needs_skill_setup,
    })
}

#[tauri::command]
pub async fn switch_workspace(
    app_state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<SwitchWorkspaceResult, String> {
    let (pb, mut ws, needs_skill_setup) = prepare_workspace_from_path(&app_state, path)?;
    workspace_svc::touch_last_active(&mut ws);
    workspace_svc::persist_workspace(pb.as_path(), &ws)?;

    let sessions = resolve_active_sessions(&app_state, pb.as_path(), &mut ws)?;
    workspace_svc::add_known(&app_state.known_workspaces_file, &ws)?;

    let mut gp = app_state.lock_active_workspace();
    *gp = Some(pb.clone());
    let mut gid = app_state.lock_active_session_id();
    *gid = ws.active_session_id.clone();

    fill_workspace_binding_fields(&app_state.root, &mut ws, pb.as_path());

    Ok(SwitchWorkspaceResult {
        workspace: ws,
        sessions,
        needs_skill_setup,
    })
}

#[tauri::command]
pub async fn open_workspace_dialog(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    activate: Option<bool>,
) -> Result<SwitchWorkspaceResult, String> {
    let picked = app
        .dialog()
        .file()
        .blocking_pick_folder()
        .ok_or_else(|| DIALOG_CANCELLED.to_string())?;
    let picked = picked
        .into_path()
        .map_err(|e| format!("无效的文件夹路径: {e:?}"))?;
    let path_str = picked.to_string_lossy().into_owned();
    if activate.unwrap_or(true) {
        switch_workspace(app_state, path_str).await
    } else {
        register_workspace(app_state, path_str)
    }
}
