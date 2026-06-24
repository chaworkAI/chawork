//! Tauri commands for the Proposal/Review system.

use std::sync::Arc;

use tauri::State;

use crate::services::proposal::{self as proposal_svc, ProposalStatus, ProposalType};
use crate::state::AppState;

#[tauri::command]
pub async fn create_proposal(
    app_state: State<'_, Arc<AppState>>,
    title: String,
    description: String,
    proposal_type: ProposalType,
    target_path: Option<String>,
    diff: Option<String>,
    new_content: Option<String>,
    source_session: Option<String>,
    risk: Option<String>,
) -> Result<proposal_svc::Proposal, String> {
    let ws = app_state.require_active_workspace()?;
    proposal_svc::create_proposal(
        &ws,
        &title,
        &description,
        proposal_type,
        target_path.as_deref(),
        diff.as_deref(),
        new_content.as_deref(),
        source_session.as_deref(),
        risk.as_deref(),
    )
}

#[tauri::command]
pub async fn list_proposals(
    app_state: State<'_, Arc<AppState>>,
    status_filter: Option<ProposalStatus>,
) -> Result<Vec<proposal_svc::Proposal>, String> {
    let ws = app_state.require_active_workspace()?;
    proposal_svc::list_proposals(&ws, status_filter)
}

#[tauri::command]
pub async fn get_proposal(
    app_state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<proposal_svc::Proposal, String> {
    let ws = app_state.require_active_workspace()?;
    proposal_svc::get_proposal(&ws, &id)
}

#[tauri::command]
pub async fn apply_proposal(
    app_state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<proposal_svc::Proposal, String> {
    let ws = app_state.require_active_workspace()?;
    proposal_svc::apply_proposal(&ws, &app_state.root, &id)
}

#[tauri::command]
pub async fn reject_proposal(
    app_state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<proposal_svc::Proposal, String> {
    let ws = app_state.require_active_workspace()?;
    proposal_svc::reject_proposal(&ws, &id)
}
