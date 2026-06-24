use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChatStreamRequest {
    #[serde(rename = "session_id")]
    pub _session_id: String,
    #[serde(rename = "message")]
    pub _message: String,
}

pub async fn handle_chat_stream(
    State(_state): State<Arc<AppState>>,
    Json(_request): Json<ChatStreamRequest>,
) -> Response {
    (
        StatusCode::GONE,
        Json(json!({
            "error": "http_chat_stream_removed",
            "message": "Chat must use the Tauri send_chat_message command backed by chawork-runtime.",
        })),
    )
        .into_response()
}
