pub mod handlers;
pub mod routes;

use std::net::TcpListener;
use std::sync::Arc;

use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

pub async fn start_http_server(
    state: Arc<AppState>,
) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    // tokio 拒绝接管 blocking socket（github.com/tokio-rs/tokio/issues/7172）
    listener.set_nonblocking(true)?;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::create_router(state).layer(cors);
    let tcp_listener = tokio::net::TcpListener::from_std(listener)?;

    tokio::spawn(async move {
        axum::serve(tcp_listener, app).await.ok();
    });

    Ok(port)
}
