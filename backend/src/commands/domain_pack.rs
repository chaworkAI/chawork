use std::sync::Arc;

use tauri::State;

use crate::services::domain_pack::{self, DomainPack};
use crate::state::AppState;

#[tauri::command]
pub async fn get_domain_pack(
    app_state: State<'_, Arc<AppState>>,
) -> Result<Option<DomainPack>, String> {
    let path = app_state.lock_active_workspace().clone();
    let Some(pb) = path else {
        return Ok(None);
    };
    domain_pack::load_domain_pack(pb.as_path())
}
