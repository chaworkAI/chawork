//! ChaWork runtime adapter.
//!
//! This spawns the `chawork-runtime --protocol=jsonrpc` binary (the
//! ChaWork-owned runtime wrapper), drives the v1 contract (initialize ->
//! thread/start -> turn/start), and translates runtime notifications into the
//! [`ChaWorkEvent`] values consumed by the UI.
//!
//! Interactive Chat always uses this runtime session contract. Approvals, user
//! input, MCP elicitation, permissions, tool/file events, reasoning events, and
//! turn lifecycle events are carried through the v1 contract.

mod connection;
mod lifecycle;
mod pending_requests;
mod projection;
mod turn_driver;

use anyhow::Result;
use serde_json::json;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime as TauriRuntime};

pub(crate) use connection::RuntimeConnection;

use super::events::{ChaWorkEvent, ChaWorkEventEnvelope};
use super::process::CodexRuntime;
use super::process::ThreadPersistCtx;

fn normalize_runtime_thread_id(session_thread_id: Option<String>) -> Option<String> {
    session_thread_id.filter(|id| !id.trim().is_empty())
}

fn thread_request_base_params(runtime: &CodexRuntime, session_id: &str) -> Value {
    let mut params = json!({
        "workspacePath": runtime.config.workspace_path,
        "sessionId": session_id,
        "codexHome": runtime.config.codex_home,
        "enableCodexApiKeyEnv": false,
    });
    if !runtime.config.model.trim().is_empty() {
        params["model"] = json!(runtime.config.model.trim());
    }
    if !runtime.config.runtime_workspace_roots.is_empty() {
        params["runtimeWorkspaceRoots"] = json!(runtime.config.runtime_workspace_roots);
    }
    if !runtime.config.approval_policy.trim().is_empty() {
        params["approvalPolicy"] = json!(runtime.config.approval_policy.trim());
    }
    if !runtime.config.sandbox.trim().is_empty() {
        params["sandbox"] = json!(runtime.config.sandbox.trim());
    }
    params
}

fn emit<R: TauriRuntime>(
    app: &AppHandle<R>,
    event: &ChaWorkEvent,
    persist: Option<&ThreadPersistCtx>,
) -> Result<()> {
    if let Some(ctx) = persist {
        let payload = ChaWorkEventEnvelope {
            workspace_id: &ctx.workspace_id,
            session_id: &ctx.session_id,
            event,
        };
        return app
            .emit("codex-event", &payload)
            .map_err(|e| anyhow::anyhow!("emit codex-event: {e}"));
    }
    app.emit("codex-event", event)
        .map_err(|e| anyhow::anyhow!("emit codex-event: {e}"))
}
