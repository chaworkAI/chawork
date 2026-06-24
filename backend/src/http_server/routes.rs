use std::sync::Arc;

use axum::{routing::post, Router};

use crate::state::AppState;

use super::handlers;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/chat/stream",
            post(handlers::chat_stream::handle_chat_stream),
        )
        .with_state(state)
}
