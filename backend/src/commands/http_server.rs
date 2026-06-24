use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_http_server_port(state: State<'_, Arc<AppState>>) -> Result<u16, String> {
    let port = state.http_server_port.load(Ordering::Relaxed);
    if port == 0 {
        return Err("HTTP server not started".to_string());
    }
    Ok(port)
}
