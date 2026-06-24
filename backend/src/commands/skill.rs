use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::runtime::lifecycle::{
    invalidate_workspace_chat_runtime, MutationWithRuntimeInvalidation, RuntimeInvalidationReason,
};
use crate::services::skill;
use crate::state::AppState;

#[tauri::command]
pub async fn list_skills(
    workspace_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<skill::SkillListView, String> {
    let _ = workspace_id;
    let root_skills_dir = state.root.skills_dir();
    let active_workspace = state.lock_active_workspace();
    let ws_path = active_workspace.as_deref();
    Ok(skill::build_skill_list_view(&root_skills_dir, ws_path))
}

#[tauri::command]
pub async fn set_workspace_skill_selection(
    app: AppHandle,
    workspace_id: String,
    root_skill_id: String,
    enabled: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<skill::SkillSelectionView>, String> {
    let _ = workspace_id;
    let ws = state.require_active_workspace()?;
    let view =
        skill::set_root_skill_enabled(&ws, &root_skill_id, enabled, &state.root.skills_dir())?;
    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &state,
        &app,
        ws,
        RuntimeInvalidationReason::WorkspaceCodexHomeContextChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        view,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn create_workspace_skill_override(
    app: AppHandle,
    workspace_id: String,
    root_skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<skill::SkillSummary>, String> {
    let _ = workspace_id;
    let ws = state.require_active_workspace()?;
    let ws_skills_dir = ws.join("skills");
    let summary = skill::create_override(&state.root.skills_dir(), &ws_skills_dir, &root_skill_id)?;

    let mut selection =
        skill::read_skill_selection(&ws).unwrap_or_else(skill::default_skill_selection);
    selection.workspace_skills.insert(
        root_skill_id.clone(),
        skill::WorkspaceSkillEntry { enabled: true },
    );
    selection.updated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    skill::write_skill_selection(&ws, &selection)?;

    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &state,
        &app,
        ws,
        RuntimeInvalidationReason::WorkspaceCodexHomeContextChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        summary,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn delete_workspace_skill(
    app: AppHandle,
    workspace_id: String,
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    let _ = workspace_id;
    let ws = state.require_active_workspace()?;
    skill::delete_workspace_skill(&ws.join("skills"), &skill_id)?;

    let mut selection =
        skill::read_skill_selection(&ws).unwrap_or_else(skill::default_skill_selection);
    selection.workspace_skills.remove(&skill_id);
    selection.updated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    skill::write_skill_selection(&ws, &selection)?;
    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &state,
        &app,
        ws,
        RuntimeInvalidationReason::WorkspaceCodexHomeContextChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn promote_skill_to_global(
    app: AppHandle,
    proposal_id: String,
    workspace_id: String,
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<skill::SkillPromotionResult>, String> {
    let _ = proposal_id;
    let _ = workspace_id;
    let ws = state.require_active_workspace()?;
    let root_skill =
        skill::promote_to_root(&ws.join("skills"), &state.root.skills_dir(), &skill_id)?;

    let affected = list_known_workspace_paths(&state.known_workspaces_file);
    let result = skill::SkillPromotionResult {
        ok: true,
        root_skill,
        affected_workspaces: affected,
        message: Some(format!("已将技能 {skill_id} 推广到全局目录")),
    };
    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &state,
        &app,
        ws,
        RuntimeInvalidationReason::WorkspaceCodexHomeContextChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        result,
        runtime_invalidation,
    ))
}

fn list_known_workspace_paths(known_file: &std::path::Path) -> Vec<String> {
    crate::services::workspace::list_known(known_file)
        .into_iter()
        .map(|w| w.path)
        .collect()
}
