use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::runtime::lifecycle::{
    invalidate_employee_chat_runtimes, invalidate_workspace_chat_runtime,
    MutationWithRuntimeInvalidation, RuntimeInvalidationReason,
};
use crate::services::employee as employee_svc;
use crate::services::workspace as workspace_svc;
use crate::state::AppState;

#[tauri::command]
pub fn list_employees(
    app_state: State<'_, Arc<AppState>>,
) -> Result<Vec<employee_svc::RegistryEntry>, String> {
    let _lock = app_state.lock_employee_write();
    employee_svc::ensure_employee_infrastructure(&app_state.root)?;
    employee_svc::list(&app_state.root)
}

#[tauri::command]
pub fn get_employee_detail(
    app_state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<employee_svc::EmployeeDetail, String> {
    employee_svc::get_detail(&app_state.root, &id)
}

#[tauri::command]
pub fn create_employee(
    app_state: State<'_, Arc<AppState>>,
    input: employee_svc::CreateEmployeeInput,
) -> Result<employee_svc::EmployeeDetail, String> {
    let _lock = app_state.lock_employee_write();
    employee_svc::create(&app_state.root, input)
}

#[tauri::command]
pub fn update_employee_metadata(
    app_state: State<'_, Arc<AppState>>,
    id: String,
    input: employee_svc::UpdateEmployeeInput,
) -> Result<employee_svc::EmployeeDetail, String> {
    let _lock = app_state.lock_employee_write();
    employee_svc::update_metadata(&app_state.root, &id, input)
}

#[tauri::command]
pub async fn delete_employee(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    let workspace_paths = {
        let _lock = app_state.lock_employee_write();
        employee_svc::list_workspaces_for_employee(&app_state.root, &id)
            .map(|memberships| {
                memberships
                    .into_iter()
                    .map(|membership| PathBuf::from(membership.path))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let runtime_invalidation = invalidate_employee_chat_runtimes(
        &app_state,
        &app,
        &id,
        RuntimeInvalidationReason::EmployeePromptChanged,
    )
    .await;

    {
        let _lock = app_state.lock_employee_write();
        employee_svc::delete_employee(&app_state.root, &id)?;
    }

    for workspace_path in workspace_paths {
        let _ = invalidate_workspace_chat_runtime(
            &app_state,
            &app,
            workspace_path,
            RuntimeInvalidationReason::WorkspaceBindingChanged,
        )
        .await;
    }

    Ok(MutationWithRuntimeInvalidation::success((), runtime_invalidation))
}

#[tauri::command]
pub fn check_employee_integrity(
    app_state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<employee_svc::IntegrityReport, String> {
    employee_svc::check_integrity(&app_state.root, &id)
}

#[tauri::command]
pub fn read_employee_prompt(
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
) -> Result<String, String> {
    employee_svc::read_employee_prompt(&app_state.root, &employee_id)
}

#[tauri::command]
pub async fn write_employee_prompt(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
    content: String,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    {
        let _lock = app_state.lock_employee_write();
        employee_svc::write_employee_prompt(&app_state.root, &employee_id, &content)?;
    }
    let runtime_invalidation = invalidate_employee_chat_runtimes(
        &app_state,
        &app,
        &employee_id,
        RuntimeInvalidationReason::EmployeePromptChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub fn list_employee_skills(
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
) -> Result<Vec<employee_svc::EmployeeSkillSummary>, String> {
    employee_svc::list_employee_skills(&app_state.root, &employee_id)
}

#[tauri::command]
pub async fn copy_root_skill_to_employee(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
    skill_id: String,
) -> Result<MutationWithRuntimeInvalidation<employee_svc::EmployeeSkillSummary>, String> {
    let summary = {
        let _lock = app_state.lock_employee_write();
        employee_svc::copy_root_skill_to_employee(&app_state.root, &employee_id, &skill_id)?
    };
    let runtime_invalidation = invalidate_employee_chat_runtimes(
        &app_state,
        &app,
        &employee_id,
        RuntimeInvalidationReason::EmployeeSkillsChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        summary,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn toggle_employee_skill(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
    skill_id: String,
    enabled: bool,
) -> Result<MutationWithRuntimeInvalidation<employee_svc::SkillRegistry>, String> {
    let registry = {
        let _lock = app_state.lock_employee_write();
        employee_svc::toggle_employee_skill(&app_state.root, &employee_id, &skill_id, enabled)?
    };
    let runtime_invalidation = invalidate_employee_chat_runtimes(
        &app_state,
        &app,
        &employee_id,
        RuntimeInvalidationReason::EmployeeSkillsChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        registry,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn delete_employee_skill(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
    skill_id: String,
) -> Result<MutationWithRuntimeInvalidation<employee_svc::SkillRegistry>, String> {
    let registry = {
        let _lock = app_state.lock_employee_write();
        employee_svc::delete_employee_skill(&app_state.root, &employee_id, &skill_id)?
    };
    let runtime_invalidation = invalidate_employee_chat_runtimes(
        &app_state,
        &app,
        &employee_id,
        RuntimeInvalidationReason::EmployeeSkillsChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        registry,
        runtime_invalidation,
    ))
}

// ── Workspace Binding Commands ─────────────────────────────────────────────

fn resolve_workspace_path(
    app_state: &AppState,
    workspace_path: Option<String>,
) -> Result<PathBuf, String> {
    match workspace_path {
        Some(p) => Ok(PathBuf::from(p)),
        None => {
            let guard = app_state.lock_active_workspace();
            guard
                .clone()
                .ok_or_else(|| "没有活跃的 workspace，请提供 workspace_path".to_string())
        }
    }
}

#[tauri::command]
pub async fn bind_workspace_to_employee(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    employee_id: String,
    workspace_path: Option<String>,
) -> Result<MutationWithRuntimeInvalidation<employee_svc::BindingValidation>, String> {
    let ws_path = resolve_workspace_path(&app_state, workspace_path)?;
    let ws = workspace_svc::read_workspace(&ws_path)?;
    let validation = {
        let _lock = app_state.lock_employee_write();
        employee_svc::bind_workspace(&app_state.root, &employee_id, &ws_path, &ws.id, &ws.name)?
    };
    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &app_state,
        &app,
        ws_path,
        RuntimeInvalidationReason::WorkspaceBindingChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        validation,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn unbind_workspace_from_employee(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    workspace_path: Option<String>,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    let ws_path = resolve_workspace_path(&app_state, workspace_path)?;
    {
        let _lock = app_state.lock_employee_write();
        employee_svc::unbind_workspace(&app_state.root, &ws_path)?;
    }
    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &app_state,
        &app,
        ws_path,
        RuntimeInvalidationReason::WorkspaceBindingChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub fn validate_workspace_binding(
    app_state: State<Arc<AppState>>,
    workspace_path: Option<String>,
) -> Result<employee_svc::BindingValidation, String> {
    let ws_path = resolve_workspace_path(&app_state, workspace_path)?;
    employee_svc::validate_binding(&app_state.root, &ws_path)
}

#[tauri::command]
pub fn list_workspaces_for_employee(
    app_state: State<Arc<AppState>>,
    employee_id: String,
) -> Result<Vec<employee_svc::WorkspaceMembership>, String> {
    employee_svc::list_workspaces_for_employee(&app_state.root, &employee_id)
}
