use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::State;

use crate::runtime::lifecycle::{
    noop_workspace_runtime_invalidation, MutationWithRuntimeInvalidation, RuntimeInvalidationReason,
};
use crate::services::mcp_tool::{
    self, McpToolPolicyInput, McpToolPolicyView, WorkspaceMcpServer, WorkspaceMcpServerTestResult,
    WorkspaceMcpServerView,
};
use crate::services::workspace as workspace_svc;
use crate::state::AppState;

fn workspace_path_for_workspace_id(
    known_workspaces_file: &Path,
    workspace_id: &str,
) -> Result<PathBuf, String> {
    if workspace_id.trim().is_empty() {
        return Err("workspace_id 不能为空".to_string());
    }
    workspace_svc::list_known(known_workspaces_file)
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .map(|workspace| PathBuf::from(workspace.path))
        .ok_or_else(|| "请求所属工作区不存在".to_string())
}

fn workspace_path_for_mcp_command(state: &AppState, workspace_id: &str) -> Result<PathBuf, String> {
    workspace_path_for_workspace_id(&state.known_workspaces_file, workspace_id)
}

#[tauri::command]
pub async fn list_mcp_tools(
    workspace_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<McpToolPolicyView, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    Ok(mcp_tool::build_policy_view(&state.root.mcp_dir(), &ws))
}

#[tauri::command]
pub async fn set_workspace_mcp_tool_policy(
    workspace_id: String,
    policy: McpToolPolicyInput,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<McpToolPolicyView>, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    let view = mcp_tool::apply_policy_input(&state.root.mcp_dir(), &ws, &policy)?;
    let runtime_invalidation =
        noop_workspace_runtime_invalidation(&ws, RuntimeInvalidationReason::McpContextChanged);
    Ok(MutationWithRuntimeInvalidation::success(
        view,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn list_workspace_mcp_servers(
    workspace_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<WorkspaceMcpServerView, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    Ok(mcp_tool::list_workspace_mcp_servers(&ws))
}

#[tauri::command]
pub async fn upsert_workspace_mcp_server(
    workspace_id: String,
    server: WorkspaceMcpServer,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<WorkspaceMcpServerView>, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    let view = mcp_tool::upsert_workspace_mcp_server(&ws, server)?;
    let runtime_invalidation =
        noop_workspace_runtime_invalidation(&ws, RuntimeInvalidationReason::McpContextChanged);
    Ok(MutationWithRuntimeInvalidation::success(
        view,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn import_workspace_mcp_servers_json(
    workspace_id: String,
    raw_json: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<WorkspaceMcpServerView>, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    let view = mcp_tool::import_mcp_servers_json(&ws, &raw_json)?;
    let runtime_invalidation =
        noop_workspace_runtime_invalidation(&ws, RuntimeInvalidationReason::McpContextChanged);
    Ok(MutationWithRuntimeInvalidation::success(
        view,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn delete_workspace_mcp_server(
    workspace_id: String,
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<WorkspaceMcpServerView>, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    let view = mcp_tool::delete_workspace_mcp_server(&ws, &name)?;
    let runtime_invalidation =
        noop_workspace_runtime_invalidation(&ws, RuntimeInvalidationReason::McpContextChanged);
    Ok(MutationWithRuntimeInvalidation::success(
        view,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn test_workspace_mcp_server(
    workspace_id: String,
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<WorkspaceMcpServerTestResult, String> {
    let ws = workspace_path_for_mcp_command(&state, &workspace_id)?;
    mcp_tool::test_workspace_mcp_server(&ws, &name).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_command_workspace_path_requires_workspace_id() {
        let tmp = tempfile::tempdir().expect("tmp");
        let known = tmp.path().join("known-workspaces.json");
        let ws_path = tmp.path().join("workspace");
        std::fs::create_dir_all(&ws_path).expect("workspace dir");
        let ws = workspace_svc::open_or_create(&ws_path).expect("workspace");
        workspace_svc::add_known(&known, &ws).expect("known");

        let resolved = workspace_path_for_workspace_id(&known, &ws.id).expect("resolve by id");
        assert_eq!(resolved, PathBuf::from(ws.path));
    }

    #[test]
    fn mcp_command_workspace_path_rejects_empty_workspace_id() {
        let err = workspace_path_for_workspace_id(Path::new("/tmp/missing-known.json"), "")
            .expect_err("empty workspace id must fail");
        assert_eq!(err, "workspace_id 不能为空");
    }
}
