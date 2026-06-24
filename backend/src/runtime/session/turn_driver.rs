use std::process::Stdio;
use std::sync::atomic::Ordering;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tauri::{AppHandle, Runtime as TauriRuntime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, Duration};

use super::connection::{read_line, route_runtime_line, RuntimeConnection, RuntimeMessage};
use super::lifecycle::{mark_status_pending, mark_status_running_after_pending};
use super::pending_requests::{pending_request_owner, runtime_owner_json};
use super::projection::{
    project_runtime_notification, runtime_debug_event, RuntimeProjection, RuntimeProjectionState,
};
use super::{emit, normalize_runtime_thread_id, thread_request_base_params};
use crate::runtime::events::ChaWorkEvent;
use crate::runtime::process::{
    apply_codex_child_env, binary_from_current_exe_dir, binary_from_repo_ancestors, CodexRuntime,
    McpElicitationResponse, PermissionsResponse, RuntimeLocalImage, RuntimeMetadata,
    RuntimeTurnInput, ThreadPersistCtx,
};
use crate::runtime::process_policy::apply_no_visible_terminal_runtime_policy;
use crate::services::session as session_svc;
use crate::state::RuntimeSlotStatus;

/// Resolve the vendored `chawork-runtime` binary. `CHAWORK_RUNTIME_CLI`
/// overrides; otherwise search repo ancestors like the codex resolver.
fn default_chawork_runtime_cli() -> String {
    if let Ok(path) = std::env::var("CHAWORK_RUNTIME_CLI") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let name = if cfg!(windows) {
        "chawork-runtime.exe"
    } else {
        "chawork-runtime"
    };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(found) = binary_from_current_exe_dir(&exe, name) {
            return found.to_string_lossy().into_owned();
        }
        if let Some(parent) = exe.parent() {
            if let Some(found) = binary_from_repo_ancestors(parent, name) {
                return found.to_string_lossy().into_owned();
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = binary_from_repo_ancestors(&cwd, name) {
            return found.to_string_lossy().into_owned();
        }
    }
    String::new()
}

fn required_runtime_capabilities() -> Vec<&'static str> {
    vec![
        "thread.start",
        "thread.resume",
        "thread.compact.start",
        "turn.start.text",
        "turn.start.image",
        "turn.start.local_image",
        "turn.start.skill",
        "turn.start.mention",
        "turn.steer",
        "turn.interrupt",
        "assistant.delta",
        "assistant.done",
        "reasoning.delta",
        "reasoning.done",
        "item.started",
        "tool.call_delta",
        "tool.call_completed",
        "file_change.updated",
        "file_change.delta",
        "file_change.completed",
        "codex.notification.mcp_tool_call_progress",
        "codex.notification.mcp_server_oauth_login_completed",
        "codex.notification.mcp_server_status_updated",
        "plan.updated",
        "plan.delta",
        "plan.done",
        "turn.completed",
        "token_usage.updated",
        "server_request.command_approval",
        "server_request.file_change_approval",
        "server_request.permissions",
        "server_request.user_input",
        "server_request.mcp_elicitation",
    ]
}

impl CodexRuntime {
    pub async fn ensure_session_started(&self) -> Result<()> {
        let mut guard = self.session_connection.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let binary = default_chawork_runtime_cli();
        if binary.is_empty() {
            bail!(
                "未找到 chawork-runtime 运行时二进制。请构建：\n  \
                 cd chawork-runtime/codex-rs && cargo build -p chawork-runtime --bin chawork-runtime\n\
                 或设置 CHAWORK_RUNTIME_CLI=/abs/path/to/chawork-runtime。"
            );
        }

        let mut cmd = Command::new(&binary);
        cmd.arg("--protocol=jsonrpc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        apply_codex_child_env(
            &mut cmd,
            &self.config.codex_home,
            &self.config.runtime_home,
            &self.config.workspace_path,
            &self.config.api_key,
        );
        apply_no_visible_terminal_runtime_policy(&mut cmd);

        let mut child = cmd.spawn().with_context(|| format!("spawn {binary}"))?;

        let stderr = child.stderr.take().context("chawork-runtime stderr pipe")?;
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let t = line.trim_end();
                        if !t.is_empty() {
                            eprintln!("[chawork-runtime stderr] {t}");
                        }
                    }
                }
            }
        });

        let stdin = child.stdin.take().context("chawork-runtime stdin pipe")?;
        let stdout = child.stdout.take().context("chawork-runtime stdout pipe")?;
        let (events_tx, events_rx) = mpsc::channel(128);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_line(&mut reader).await {
                    Ok(Some(line)) => {
                        if let Some(message) = route_runtime_line(&line) {
                            if events_tx.send(message).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        let _ = events_tx.send(RuntimeMessage::Eof).await;
                        break;
                    }
                    Err(_) => {
                        let _ = events_tx.send(RuntimeMessage::Eof).await;
                        break;
                    }
                }
            }
        });

        let mut conn = RuntimeConnection::new(child, stdin, events_rx);
        let initialize_response = conn
            .send_rpc_wait(
                "runtime/initialize",
                Some(json!({
                    "contractVersion": 1,
                    "client": { "name": "chawork", "version": env!("CARGO_PKG_VERSION") },
                    "workspacePath": self.config.workspace_path,
                    "requiredCapabilities": required_runtime_capabilities(),
                })),
            )
            .await?;
        let initialize_result = initialize_response
            .get("result")
            .cloned()
            .unwrap_or(Value::Null);
        self.set_runtime_metadata(RuntimeMetadata::from_initialize_result(&initialize_result))
            .await;
        conn.initialized = true;
        *guard = Some(conn);
        Ok(())
    }

    pub async fn shutdown_session(&self) -> Result<()> {
        self.clear_all_pending_requests().await;
        let mut guard = self.session_connection.lock().await;
        let Some(mut conn) = guard.take() else {
            return Ok(());
        };
        let _ = conn.send_rpc("runtime/shutdown", None).await;
        drop(conn.stdin);
        let _ = conn.child.wait().await;
        self.clear_all_pending_requests().await;
        Ok(())
    }

    /// Run one turn through `chawork-runtime`.
    pub async fn start_turn(
        &self,
        input: &RuntimeTurnInput,
        app: &AppHandle,
        codex_status: &AsyncMutex<String>,
        slot_status: Option<&AsyncMutex<RuntimeSlotStatus>>,
        persist: Option<ThreadPersistCtx>,
    ) -> Result<String> {
        self.cancel_requested.store(false, Ordering::SeqCst);
        self.ensure_session_started().await?;

        let result = {
            let mut guard = self.session_connection.lock().await;
            let conn = guard
                .as_mut()
                .context("runtime connection missing after initialization")?;
            self.drive_turn(input, app, conn, slot_status, persist.as_ref())
                .await
        };

        if result.is_err() {
            let mut guard = self.session_connection.lock().await;
            if let Some(mut conn) = guard.take() {
                let _ = conn.child.kill().await;
            }
        }
        *codex_status.lock().await = "idle".to_string();
        result
    }
    async fn send_interrupt_if_cancelled(
        &self,
        conn: &mut RuntimeConnection,
        thread_id: &str,
        turn_id: &str,
        interrupt_sent: &mut bool,
    ) -> Result<bool> {
        if !self.cancel_requested.load(Ordering::SeqCst) || *interrupt_sent {
            return Ok(false);
        }
        if !turn_id.is_empty() && !thread_id.is_empty() {
            conn.send_rpc(
                "turn/interrupt",
                Some(json!({
                    "threadId": thread_id,
                    "turnId": turn_id,
                })),
            )
            .await?;
            *interrupt_sent = true;
        }
        Ok(true)
    }

    async fn recv_approval_or_cancel(
        &self,
        conn: &mut RuntimeConnection,
        thread_id: &str,
        turn_id: &str,
        interrupt_sent: &mut bool,
    ) -> Result<Option<(String, String)>> {
        let mut rx = self.approval_rx.lock().await;
        loop {
            tokio::select! {
                value = rx.recv() => return Ok(value),
                _ = sleep(Duration::from_millis(100)) => {
                    if self.send_interrupt_if_cancelled(conn, thread_id, turn_id, interrupt_sent).await? {
                        return Ok(None);
                    }
                }
            }
        }
    }

    async fn recv_permissions_or_cancel(
        &self,
        conn: &mut RuntimeConnection,
        thread_id: &str,
        turn_id: &str,
        interrupt_sent: &mut bool,
    ) -> Result<Option<PermissionsResponse>> {
        let mut rx = self.permissions_rx.lock().await;
        loop {
            tokio::select! {
                value = rx.recv() => return Ok(value),
                _ = sleep(Duration::from_millis(100)) => {
                    if self.send_interrupt_if_cancelled(conn, thread_id, turn_id, interrupt_sent).await? {
                        return Ok(None);
                    }
                }
            }
        }
    }

    async fn recv_elicitation_or_cancel(
        &self,
        conn: &mut RuntimeConnection,
        thread_id: &str,
        turn_id: &str,
        interrupt_sent: &mut bool,
    ) -> Result<Option<McpElicitationResponse>> {
        let mut rx = self.elicitation_rx.lock().await;
        loop {
            tokio::select! {
                value = rx.recv() => return Ok(value),
                _ = sleep(Duration::from_millis(100)) => {
                    if self.send_interrupt_if_cancelled(conn, thread_id, turn_id, interrupt_sent).await? {
                        return Ok(None);
                    }
                }
            }
        }
    }

    async fn recv_user_input_or_cancel(
        &self,
        conn: &mut RuntimeConnection,
        thread_id: &str,
        turn_id: &str,
        interrupt_sent: &mut bool,
    ) -> Result<Option<(String, Value)>> {
        let mut rx = self.user_input_rx.lock().await;
        loop {
            tokio::select! {
                value = rx.recv() => return Ok(value),
                _ = sleep(Duration::from_millis(100)) => {
                    if self.send_interrupt_if_cancelled(conn, thread_id, turn_id, interrupt_sent).await? {
                        return Ok(None);
                    }
                }
            }
        }
    }

    async fn register_pending_owner_for_request(
        &self,
        persist: Option<&ThreadPersistCtx>,
        thread_id: &str,
        turn_id: &str,
        request_id: &str,
    ) -> Result<()> {
        let owner = pending_request_owner(persist, thread_id, turn_id, request_id)
            .ok_or_else(|| anyhow::anyhow!("runtime pending request owner is incomplete"))?;
        self.register_pending_request(owner).await;
        Ok(())
    }

    fn emit_buffered_notifications<R: TauriRuntime>(
        conn: &mut RuntimeConnection,
        app: &AppHandle<R>,
        persist: Option<&ThreadPersistCtx>,
        projection_state: &mut RuntimeProjectionState,
    ) -> Result<()> {
        for (method, params) in conn.take_pending_notifications() {
            match project_runtime_notification(&method, &params, projection_state) {
                RuntimeProjection::Events(events) => {
                    for event in events {
                        emit(app, &event, persist)?;
                    }
                }
                RuntimeProjection::RuntimeError {
                    message,
                    recoverable,
                } => {
                    emit(
                        app,
                        &ChaWorkEvent::Error {
                            message,
                            recoverable,
                        },
                        persist,
                    )?;
                }
                RuntimeProjection::TurnFailed { message } => {
                    emit(
                        app,
                        &ChaWorkEvent::Error {
                            message,
                            recoverable: false,
                        },
                        persist,
                    )?;
                }
                RuntimeProjection::Ignored
                | RuntimeProjection::TurnCompleted { .. }
                | RuntimeProjection::TurnInterrupted
                | RuntimeProjection::BlockingRequest
                | RuntimeProjection::RawServerRequest => {}
            }
        }
        Ok(())
    }

    pub(super) async fn drive_turn<R: TauriRuntime>(
        &self,
        input: &RuntimeTurnInput,
        app: &AppHandle<R>,
        conn: &mut RuntimeConnection,
        slot_status: Option<&AsyncMutex<RuntimeSlotStatus>>,
        persist: Option<&ThreadPersistCtx>,
    ) -> Result<String> {
        if input.is_empty() {
            bail!("turn input must contain text or at least one image");
        }
        let mut projection_state = RuntimeProjectionState::for_input(input.has_image_input());
        // 1. thread/start or thread/resume
        let session_id = persist
            .map(|ctx| ctx.session_id.clone())
            .unwrap_or_default();
        let thread_params = thread_request_base_params(self, &session_id);
        let session_thread_id = persist
            .map(|ctx| {
                session_svc::load_runtime_thread_id(ctx.workspace.as_path(), &ctx.session_id)
                    .map_err(|e| anyhow::anyhow!("读取 session runtime thread 映射失败: {e}"))
            })
            .transpose()?
            .flatten();
        let persisted_thread_id = normalize_runtime_thread_id(session_thread_id);

        let thread_id = if let Some(active) = conn.active_thread_id.clone() {
            if persisted_thread_id.as_deref() == Some(active.as_str()) {
                active
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let thread_id = if !thread_id.is_empty() {
            thread_id
        } else {
            let (method, request_params) = match persisted_thread_id {
                Some(tid) => {
                    let mut p = thread_params;
                    p["runtimeThreadId"] = json!(tid);
                    ("thread/resume", p)
                }
                None => ("thread/start", thread_params),
            };
            let thread_resp = conn.send_rpc_wait(method, Some(request_params)).await?;
            thread_resp["result"]["threadId"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };

        if !thread_id.is_empty() {
            conn.active_thread_id = Some(thread_id.clone());
            *self.thread_id.lock().await = Some(thread_id.clone());
            if let Some(ctx) = persist {
                let _ = session_svc::append_runtime_thread_id(
                    ctx.workspace.as_path(),
                    &ctx.session_id,
                    &thread_id,
                );
            }
        }
        Self::emit_buffered_notifications(conn, app, persist, &mut projection_state)?;

        // 2. turn/start
        let turn_resp = conn
            .send_rpc_wait(
                "turn/start",
                Some(json!({
                    "threadId": thread_id,
                    "input": runtime_turn_input_json(input),
                })),
            )
            .await
            .map_err(|err| {
                anyhow::anyhow!(
                    "{}",
                    super::projection::user_facing_turn_error_for_input(
                        &format!("{err:#}"),
                        input.has_image_input()
                    )
                )
            })?;
        let turn_id = turn_resp["result"]["turnId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        conn.active_turn_id = if turn_id.is_empty() {
            None
        } else {
            Some(turn_id.clone())
        };
        Self::emit_buffered_notifications(conn, app, persist, &mut projection_state)?;

        // 3. pump notifications to terminal
        // True once we've already sent a turn/interrupt RPC for this turn so we
        // don't spam Codex if cancel_requested stays set while we wait for the
        // runtime's turn/interrupted notification.
        let mut interrupt_sent = false;
        let mut runtime_closed = false;
        let mut fatal_runtime = false;
        let mut terminal_error: Option<String> = None;
        loop {
            if self
                .send_interrupt_if_cancelled(conn, &thread_id, &turn_id, &mut interrupt_sent)
                .await?
            {
                // Don't break — keep reading; the runtime will emit
                // turn/interrupted (or turn/completed{Interrupted}) which
                // closes the loop below.
            }
            let Some(message) = conn.events_rx.recv().await else {
                runtime_closed = true;
                break;
            };
            let (method, params) = match message {
                RuntimeMessage::Notification { method, params } => (method, params),
                RuntimeMessage::Response(_) => continue,
                RuntimeMessage::Eof => {
                    runtime_closed = true;
                    break;
                }
            };
            match project_runtime_notification(&method, &params, &mut projection_state) {
                RuntimeProjection::Events(events) => {
                    for event in events {
                        emit(app, &event, persist)?;
                    }
                }
                RuntimeProjection::TurnCompleted { usage } => {
                    if let Some(event) = projection_state.synthetic_assistant_done(None) {
                        emit(app, &event, persist)?;
                    }
                    emit(app, &ChaWorkEvent::TurnComplete { usage }, persist)?;
                    if let Some(ctx) = persist {
                        self.clear_pending_requests_for_session(&ctx.session_id)
                            .await;
                    }
                    break;
                }
                RuntimeProjection::TurnInterrupted => {
                    emit(app, &ChaWorkEvent::Cancelled, persist)?;
                    if let Some(ctx) = persist {
                        self.clear_pending_requests_for_session(&ctx.session_id)
                            .await;
                    }
                    break;
                }
                RuntimeProjection::TurnFailed { message } => {
                    terminal_error = Some(message.clone());
                    if let Some(event) =
                        projection_state.synthetic_assistant_done(Some(message.as_str()))
                    {
                        emit(app, &event, persist)?;
                    }
                    emit(
                        app,
                        &ChaWorkEvent::Error {
                            message,
                            recoverable: false,
                        },
                        persist,
                    )?;
                    if let Some(ctx) = persist {
                        self.clear_pending_requests_for_session(&ctx.session_id)
                            .await;
                    }
                    break;
                }
                RuntimeProjection::RuntimeError {
                    message,
                    recoverable,
                } => {
                    emit(
                        app,
                        &ChaWorkEvent::Error {
                            message,
                            recoverable,
                        },
                        persist,
                    )?;
                    if !recoverable {
                        fatal_runtime = true;
                        break;
                    }
                }
                RuntimeProjection::Ignored => {}
                RuntimeProjection::BlockingRequest => match method.as_str() {
                    "approval/requested" => {
                        let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                        let kind = params["kind"].as_str().unwrap_or("command").to_string();
                        if let Err(err) = self
                            .register_pending_owner_for_request(
                                persist,
                                &thread_id,
                                &turn_id,
                                &request_id,
                            )
                            .await
                        {
                            emit(
                                app,
                                &ChaWorkEvent::Error {
                                    message: err.to_string(),
                                    recoverable: false,
                                },
                                persist,
                            )?;
                            fatal_runtime = true;
                            break;
                        }
                        mark_status_pending(slot_status).await;
                        emit(
                            app,
                            &ChaWorkEvent::ApprovalRequest {
                                id: request_id.clone(),
                                method: format!("approval/{kind}"),
                                title: params["title"].as_str().unwrap_or("审批").to_string(),
                                description: params["description"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                                risk: params["risk"].as_str().unwrap_or("medium").to_string(),
                                params: params["params"].clone(),
                            },
                            persist,
                        )?;
                        if kind == "permissions" {
                            match self
                                .recv_permissions_or_cancel(
                                    conn,
                                    &thread_id,
                                    &turn_id,
                                    &mut interrupt_sent,
                                )
                                .await?
                            {
                                Some(resp) if resp.granted => {
                                    let mut respond_params = json!({
                                        "kind": "permissions",
                                        "owner": runtime_owner_json(persist, &thread_id, &turn_id, &resp.request_id),
                                        "requestId": resp.request_id,
                                        "permissions": resp.permissions,
                                        "scope": resp.scope,
                                    });
                                    if let Some(strict) = resp.strict_auto_review {
                                        respond_params["strictAutoReview"] = json!(strict);
                                    }
                                    conn.send_rpc("serverRequest/respond", Some(respond_params))
                                        .await?;
                                }
                                // Denied grant or closed channel both reject the
                                // Codex request — never silently grant.
                                other => {
                                    let reason = match other {
                                        Some(_) => "user denied permission escalation",
                                        None => "permissions channel closed",
                                    };
                                    conn.send_rpc(
                                    "serverRequest/reject",
                                    Some(json!({
                                        "owner": runtime_owner_json(persist, &thread_id, &turn_id, &request_id),
                                        "requestId": request_id,
                                        "reason": reason,
                                    })),
                                )
                                .await?;
                                }
                            }
                        } else {
                            match self
                                .recv_approval_or_cancel(
                                    conn,
                                    &thread_id,
                                    &turn_id,
                                    &mut interrupt_sent,
                                )
                                .await?
                            {
                                Some((decision_id, decision)) => {
                                    conn.send_rpc(
                                    "serverRequest/respond",
                                    Some(json!({
                                        "owner": runtime_owner_json(persist, &thread_id, &turn_id, &decision_id),
                                        "requestId": decision_id,
                                        "kind": kind,
                                        "decision": decision,
                                    })),
                                )
                                .await?;
                                }
                                None => {
                                    conn.send_rpc(
                                    "serverRequest/reject",
                                    Some(json!({
                                        "owner": runtime_owner_json(persist, &thread_id, &turn_id, &request_id),
                                        "requestId": request_id,
                                        "reason": "approval channel closed",
                                    })),
                                )
                                .await?;
                                }
                            }
                        }
                        mark_status_running_after_pending(slot_status).await;
                    }
                    "mcp_elicitation/requested" => {
                        let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                        let server_name = params["serverName"].as_str().unwrap_or("").to_string();
                        let mode = params["mode"].as_str().unwrap_or("form").to_string();
                        let message = params["message"].as_str().unwrap_or("").to_string();
                        if let Err(err) = self
                            .register_pending_owner_for_request(
                                persist,
                                &thread_id,
                                &turn_id,
                                &request_id,
                            )
                            .await
                        {
                            emit(
                                app,
                                &ChaWorkEvent::Error {
                                    message: err.to_string(),
                                    recoverable: false,
                                },
                                persist,
                            )?;
                            fatal_runtime = true;
                            break;
                        }
                        mark_status_pending(slot_status).await;
                        emit(
                            app,
                            &ChaWorkEvent::McpElicitationRequest {
                                id: request_id.clone(),
                                server_name,
                                mode,
                                message,
                                params: params.clone(),
                            },
                            persist,
                        )?;
                        match self
                            .recv_elicitation_or_cancel(
                                conn,
                                &thread_id,
                                &turn_id,
                                &mut interrupt_sent,
                            )
                            .await?
                        {
                            Some(resp) => {
                                let mut respond_params = json!({
                                    "kind": "mcp_elicitation",
                                    "owner": runtime_owner_json(persist, &thread_id, &turn_id, &resp.request_id),
                                    "requestId": resp.request_id,
                                    "action": resp.action,
                                });
                                if let Some(content) = resp.content {
                                    respond_params["content"] = content;
                                }
                                if let Some(meta) = resp.meta {
                                    respond_params["_meta"] = meta;
                                }
                                conn.send_rpc("serverRequest/respond", Some(respond_params))
                                    .await?;
                            }
                            None => {
                                conn.send_rpc(
                                "serverRequest/reject",
                                Some(json!({
                                    "owner": runtime_owner_json(persist, &thread_id, &turn_id, &request_id),
                                    "requestId": request_id,
                                    "reason": "elicitation channel closed",
                                })),
                            )
                            .await?;
                            }
                        }
                        mark_status_running_after_pending(slot_status).await;
                    }
                    "user_input/requested" => {
                        let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                        let questions = params["questions"].clone();
                        let count = questions.as_array().map(Vec::len).unwrap_or_default();
                        let description = if count > 0 {
                            format!("Codex 请求你回答 {count} 个问题后继续执行")
                        } else {
                            "Codex 请求用户输入后继续执行".to_string()
                        };
                        if let Err(err) = self
                            .register_pending_owner_for_request(
                                persist,
                                &thread_id,
                                &turn_id,
                                &request_id,
                            )
                            .await
                        {
                            emit(
                                app,
                                &ChaWorkEvent::Error {
                                    message: err.to_string(),
                                    recoverable: false,
                                },
                                persist,
                            )?;
                            fatal_runtime = true;
                            break;
                        }
                        mark_status_pending(slot_status).await;
                        emit(
                            app,
                            &ChaWorkEvent::UserInputRequest {
                                id: request_id.clone(),
                                method: "user_input/requested".to_string(),
                                title: "需要用户输入".to_string(),
                                description,
                                questions,
                                params: params.clone(),
                            },
                            persist,
                        )?;
                        match self
                            .recv_user_input_or_cancel(
                                conn,
                                &thread_id,
                                &turn_id,
                                &mut interrupt_sent,
                            )
                            .await?
                        {
                            Some((answer_id, answers)) => {
                                conn.send_rpc(
                                "serverRequest/respond",
                                Some(json!({
                                    "kind": "user_input",
                                    "owner": runtime_owner_json(persist, &thread_id, &turn_id, &answer_id),
                                    "requestId": answer_id,
                                    "answers": answers,
                                })),
                            )
                            .await?;
                            }
                            None => {
                                conn.send_rpc(
                                "serverRequest/reject",
                                Some(json!({
                                    "owner": runtime_owner_json(persist, &thread_id, &turn_id, &request_id),
                                    "requestId": request_id,
                                    "reason": "user-input channel closed",
                                })),
                            )
                            .await?;
                            }
                        }
                        mark_status_running_after_pending(slot_status).await;
                    }
                    other => bail!("blocking projection returned unexpected method {other}"),
                },
                RuntimeProjection::RawServerRequest => {
                    let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                    let event = runtime_debug_event(&method, params.clone());
                    emit(app, &event, persist)?;
                    if request_id.trim().is_empty() {
                        emit(
                            app,
                            &ChaWorkEvent::Error {
                                message: "raw Codex ServerRequest missing requestId".to_string(),
                                recoverable: false,
                            },
                            persist,
                        )?;
                        fatal_runtime = true;
                        break;
                    }
                    conn.send_rpc(
                        "serverRequest/reject",
                        Some(json!({
                            "owner": runtime_owner_json(persist, &thread_id, &turn_id, &request_id),
                            "requestId": request_id,
                            "reason": "raw Codex ServerRequest is not handled by ChaWork app",
                        })),
                    )
                    .await?;
                }
            }
        }

        conn.active_turn_id = None;
        if runtime_closed {
            if let Some(ctx) = persist {
                self.clear_pending_requests_for_session(&ctx.session_id)
                    .await;
            }
            bail!("runtime connection closed");
        }
        if fatal_runtime {
            conn.initialized = false;
            if let Some(ctx) = persist {
                self.clear_pending_requests_for_session(&ctx.session_id)
                    .await;
            }
            bail!("runtime reported fatal error");
        }

        Ok(projection_state.into_final_assistant_text(terminal_error))
    }
}

fn runtime_turn_input_json(input: &RuntimeTurnInput) -> Vec<Value> {
    let mut items = Vec::new();
    let text = input.text.trim();
    if !text.is_empty() {
        items.push(json!({ "type": "text", "text": text }));
    }
    for RuntimeLocalImage { path } in &input.local_images {
        items.push(json!({
            "type": "local_image",
            "path": path,
            "detail": "high",
        }));
    }
    items
}

#[cfg(test)]
#[path = "turn_driver_tests.rs"]
mod tests;
