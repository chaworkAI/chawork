use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::commands::runtime::{chat_runtime_from_slot, ensure_workspace_runtime_started_for_path};
use crate::runtime::events::{ChaWorkEvent, ChaWorkEventEnvelope};
use crate::runtime::lifecycle::complete_pending_invalidation_after_turn;
use crate::runtime::{CodexRuntime, RuntimeLocalImage, RuntimeTurnInput, ThreadPersistCtx};
use crate::services::{session as session_svc, workspace as workspace_svc};
use crate::state::AppState;
use crate::state::RuntimeSlotStatus;

#[derive(Clone, Serialize)]
pub struct SwitchSessionResult {
    pub transcript: Vec<serde_json::Value>,
}

#[derive(Clone, Serialize)]
pub struct DeleteSessionResult {
    pub sessions: Vec<session_svc::SessionMeta>,
    pub active_session_id: String,
    pub transcript: Vec<serde_json::Value>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageInput {
    pub content: String,
    pub session_id: String,
    #[serde(default)]
    pub attachments: Vec<ChatAttachmentInput>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentInput {
    pub kind: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub data_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[tauri::command]
pub fn list_sessions(
    app_state: State<Arc<AppState>>,
) -> Result<Vec<session_svc::SessionMeta>, String> {
    let pb = app_state.require_active_workspace()?;
    session_svc::list(pb.as_path())
}

#[tauri::command]
pub fn create_session(app_state: State<Arc<AppState>>) -> Result<session_svc::SessionMeta, String> {
    let pb = app_state.require_active_workspace()?;
    let ws = workspace_svc::read_workspace(pb.as_path())?;
    let meta = session_svc::create(pb.as_path(), &ws.id)?;

    workspace_svc::set_active_session_id(pb.as_path(), Some(&meta.id))?;

    let mut ws = ws;
    ws.active_session_id = Some(meta.id.clone());
    workspace_svc::touch_last_active(&mut ws);
    workspace_svc::persist_workspace(pb.as_path(), &ws)?;

    *app_state.lock_active_session_id() = Some(meta.id.clone());
    workspace_svc::add_known(&app_state.known_workspaces_file, &ws)?;

    Ok(meta)
}

#[tauri::command]
pub async fn switch_session(
    app_state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<SwitchSessionResult, String> {
    let pb = app_state.require_active_workspace()?;
    let sid = session_id.trim();
    if !session_svc::session_exists(pb.as_path(), sid) {
        return Err("会话不存在".to_string());
    }

    workspace_svc::set_active_session_id(pb.as_path(), Some(sid))?;

    let mut ws = workspace_svc::read_workspace(pb.as_path())?;
    ws.active_session_id = Some(sid.to_string());
    workspace_svc::persist_workspace(pb.as_path(), &ws)?;
    workspace_svc::add_known(&app_state.known_workspaces_file, &ws)?;

    *app_state.lock_active_session_id() = Some(sid.to_string());

    let transcript = session_svc::read_transcript(pb.as_path(), sid)?;

    Ok(SwitchSessionResult { transcript })
}

#[tauri::command]
pub fn get_active_session_transcript(
    app_state: State<Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let pb = app_state.require_active_workspace()?;
    let sid = app_state
        .lock_active_session_id()
        .clone()
        .ok_or_else(|| "未选择会话".to_string())?;
    session_svc::read_transcript(pb.as_path(), &sid)
}

#[tauri::command]
pub fn rename_session(
    app_state: State<Arc<AppState>>,
    session_id: String,
    title: String,
) -> Result<session_svc::SessionMeta, String> {
    let pb = app_state.require_active_workspace()?;
    let sid = session_id.trim();
    session_svc::rename_session(pb.as_path(), sid, &title)
}

#[tauri::command]
pub async fn delete_session(
    app_state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<DeleteSessionResult, String> {
    let pb = app_state.require_active_workspace()?;
    let sid = session_id.trim().to_string();
    if !session_svc::session_exists(pb.as_path(), &sid) {
        return Err("会话不存在".to_string());
    }

    let active = app_state
        .lock_active_session_id()
        .clone()
        .unwrap_or_default();
    let deleting_active = active == sid;

    session_svc::delete_session(pb.as_path(), &sid)?;

    let mut sessions = session_svc::list(pb.as_path())?;
    let ws = workspace_svc::read_workspace(pb.as_path())?;

    let next_active = if deleting_active {
        if sessions.is_empty() {
            let meta = session_svc::create(pb.as_path(), &ws.id)?;
            sessions = session_svc::list(pb.as_path())?;
            meta.id
        } else {
            sessions[0].id.clone()
        }
    } else {
        active
    };

    workspace_svc::set_active_session_id(pb.as_path(), Some(&next_active))?;

    let mut ws = ws;
    ws.active_session_id = Some(next_active.clone());
    workspace_svc::touch_last_active(&mut ws);
    workspace_svc::persist_workspace(pb.as_path(), &ws)?;
    workspace_svc::add_known(&app_state.known_workspaces_file, &ws)?;

    *app_state.lock_active_session_id() = Some(next_active.clone());

    let transcript = session_svc::read_transcript(pb.as_path(), &next_active)?;

    Ok(DeleteSessionResult {
        sessions,
        active_session_id: next_active,
        transcript,
    })
}

async fn run_codex_turn_after_user_message(
    app: AppHandle,
    app_state: Arc<AppState>,
    pb: PathBuf,
    workspace_id: String,
    sid: String,
    codex_input: RuntimeTurnInput,
    slot: Arc<crate::state::RuntimeSlot>,
    rt: Arc<CodexRuntime>,
) {
    app_state.turn_cancel.store(false, Ordering::SeqCst);

    let mut st = app_state.codex_status.lock().await;
    *st = "thinking".to_string();
    drop(st);

    let persist = ThreadPersistCtx {
        workspace: pb.clone(),
        workspace_id: workspace_id.clone(),
        session_id: sid.clone(),
    };
    *slot.status.lock().await = RuntimeSlotStatus::Running;
    *slot.last_used_at.lock().await = std::time::Instant::now();

    let result: Result<String, String> = rt
        .start_turn(
            &codex_input,
            &app,
            &app_state.codex_status,
            Some(&slot.status),
            Some(persist),
        )
        .await
        .map_err(|e| e.to_string());
    let turn_ok = result.is_ok();

    match result {
        Ok(assistant_text) => {
            if !assistant_text.trim().is_empty() {
                let ts = workspace_svc::current_iso_timestamp();
                let ae = serde_json::json!({
                    "role": "assistant",
                    "content": assistant_text,
                    "timestamp": ts,
                });
                let _lock = app_state.lock_transcript_write();
                let _ = session_svc::append_transcript(pb.as_path(), &sid, &ae);
                let _ = session_svc::persist_meta_after_assistant_message(pb.as_path(), &sid);
            }
            if let Ok(mut ws) = workspace_svc::read_workspace(pb.as_path()) {
                workspace_svc::touch_last_active(&mut ws);
                let _ = workspace_svc::persist_workspace(pb.as_path(), &ws);
                let _ = workspace_svc::add_known(&app_state.known_workspaces_file, &ws);
            }
        }
        Err(e) => {
            let msg = e;
            let mut st = app_state.codex_status.lock().await;
            *st = "error".to_string();
            emit_owned_event(
                &app,
                &workspace_id,
                &sid,
                &ChaWorkEvent::Error {
                    message: format!("模型调用失败: {msg}"),
                    recoverable: false,
                },
            );
        }
    }

    app_state.turn_cancel.store(false, Ordering::SeqCst);
    let mut st = app_state.codex_status.lock().await;
    *st = "idle".to_string();
    let has_pending_invalidation = slot.pending_invalidation.lock().await.is_some();
    if has_pending_invalidation {
        let _ = complete_pending_invalidation_after_turn(&app_state, &app, &slot).await;
    } else {
        let mut status = slot.status.lock().await;
        complete_runtime_turn_slot(&mut status, turn_ok);
        drop(status);
        *slot.last_used_at.lock().await = std::time::Instant::now();
    }
}

fn emit_owned_event(app: &AppHandle, workspace_id: &str, session_id: &str, event: &ChaWorkEvent) {
    let payload = ChaWorkEventEnvelope {
        workspace_id,
        session_id,
        event,
    };
    let _ = app.emit("codex-event", &payload);
}

fn claim_runtime_turn_slot(
    status: &mut RuntimeSlotStatus,
    has_pending_invalidation: bool,
) -> Result<(), String> {
    ensure_runtime_turn_slot_available(status, has_pending_invalidation)?;
    *status = RuntimeSlotStatus::Running;
    Ok(())
}

fn ensure_runtime_turn_slot_available(
    status: &RuntimeSlotStatus,
    has_pending_invalidation: bool,
) -> Result<(), String> {
    if has_pending_invalidation {
        return Err("当前 workspace runtime context 正在清理，请稍后重试".to_string());
    }
    if matches!(status, RuntimeSlotStatus::Idle | RuntimeSlotStatus::Error) {
        return Ok(());
    }
    Err("当前 workspace runtime 正在运行，请先取消或等待完成".to_string())
}

fn complete_runtime_turn_slot(status: &mut RuntimeSlotStatus, turn_ok: bool) {
    *status = if turn_ok {
        RuntimeSlotStatus::Idle
    } else {
        RuntimeSlotStatus::Error
    };
}

fn cleanup_message_attachments(
    workspace_path: &std::path::Path,
    session_id: &str,
    message_id: &str,
) {
    let _ = std::fs::remove_dir_all(session_svc::session_attachment_dir(
        workspace_path,
        session_id,
        message_id,
    ));
}

/// 追加用户消息到 transcript，运行 Codex 一轮，将助手回复写回 transcript；事件经 `codex-event` 推送。
#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    input: SendChatMessageInput,
) -> Result<(), String> {
    let text = input.content.trim();
    if text.is_empty() && input.attachments.is_empty() {
        return Err("消息不能为空".to_string());
    }

    let pb = app_state.require_active_workspace()?;
    let binding = crate::services::employee::validate_binding(&app_state.root, pb.as_path())?;
    if binding.status != crate::services::employee::BindingStatus::Bound {
        return Err(binding.message);
    }

    let sid = input.session_id.trim().to_string();
    if sid.is_empty() {
        return Err("未选择会话".to_string());
    }
    if !session_svc::session_exists(pb.as_path(), &sid) {
        return Err("会话不存在".to_string());
    }

    let slot = ensure_workspace_runtime_started_for_path(&app_state, pb.clone()).await?;
    let rt = chat_runtime_from_slot(&slot).await?;
    {
        let has_pending_invalidation = slot.pending_invalidation.lock().await.is_some();
        let status = slot.status.lock().await;
        ensure_runtime_turn_slot_available(&status, has_pending_invalidation)?;
    }

    let message_id = uuid::Uuid::new_v4().to_string();
    let mut attachments = Vec::new();
    for attachment in &input.attachments {
        if attachment.kind.trim() != "image" {
            cleanup_message_attachments(pb.as_path(), &sid, &message_id);
            return Err("当前 Chat 只支持图片附件".to_string());
        }
        let copied = if let Some(path) = attachment
            .path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            let source_path = PathBuf::from(path);
            match session_svc::copy_session_image_attachment(
                pb.as_path(),
                &sid,
                &message_id,
                &source_path,
            ) {
                Ok(copied) => copied,
                Err(err) => {
                    cleanup_message_attachments(pb.as_path(), &sid, &message_id);
                    return Err(err);
                }
            }
        } else if let Some(data_url) = attachment
            .data_url
            .as_deref()
            .map(str::trim)
            .filter(|data_url| !data_url.is_empty())
        {
            match session_svc::write_session_image_attachment_from_data_url(
                pb.as_path(),
                &sid,
                &message_id,
                attachment.name.as_deref(),
                data_url,
            ) {
                Ok(copied) => copied,
                Err(err) => {
                    cleanup_message_attachments(pb.as_path(), &sid, &message_id);
                    return Err(err);
                }
            }
        } else {
            cleanup_message_attachments(pb.as_path(), &sid, &message_id);
            return Err("图片附件缺少路径或 dataUrl".to_string());
        };
        attachments.push(copied);
    }
    let timestamp = workspace_svc::current_iso_timestamp();
    let mut entry = serde_json::json!({
        "role": "user",
        "content": text,
        "timestamp": timestamp,
    });
    if !attachments.is_empty() {
        entry["attachments"] =
            serde_json::to_value(&attachments).map_err(|e| format!("序列化图片附件失败: {e}"))?;
    }
    {
        let has_pending_invalidation = slot.pending_invalidation.lock().await.is_some();
        let mut status = slot.status.lock().await;
        if let Err(err) = claim_runtime_turn_slot(&mut status, has_pending_invalidation) {
            cleanup_message_attachments(pb.as_path(), &sid, &message_id);
            return Err(err);
        }
    }
    *slot.last_used_at.lock().await = std::time::Instant::now();

    let persist_result = (|| {
        let _lock = app_state.lock_transcript_write();
        session_svc::append_transcript(pb.as_path(), &sid, &entry)?;
        session_svc::persist_meta_after_user_message(pb.as_path(), &sid, &entry)?;
        Ok::<(), String>(())
    })();
    if let Err(err) = persist_result {
        cleanup_message_attachments(pb.as_path(), &sid, &message_id);
        let mut status = slot.status.lock().await;
        complete_runtime_turn_slot(&mut status, false);
        return Err(err);
    }

    let mut ws = match workspace_svc::read_workspace(pb.as_path()) {
        Ok(ws) => ws,
        Err(err) => {
            cleanup_message_attachments(pb.as_path(), &sid, &message_id);
            let mut status = slot.status.lock().await;
            complete_runtime_turn_slot(&mut status, false);
            return Err(err);
        }
    };
    let workspace_id = ws.id.clone();
    workspace_svc::touch_last_active(&mut ws);
    if let Err(err) = workspace_svc::persist_workspace(pb.as_path(), &ws) {
        cleanup_message_attachments(pb.as_path(), &sid, &message_id);
        let mut status = slot.status.lock().await;
        complete_runtime_turn_slot(&mut status, false);
        return Err(err);
    }
    if let Err(err) = workspace_svc::add_known(&app_state.known_workspaces_file, &ws) {
        cleanup_message_attachments(pb.as_path(), &sid, &message_id);
        let mut status = slot.status.lock().await;
        complete_runtime_turn_slot(&mut status, false);
        return Err(err);
    }

    {
        let mut st = app_state.codex_status.lock().await;
        *st = "thinking".to_string();
    }

    emit_owned_event(
        &app,
        &workspace_id,
        &sid,
        &ChaWorkEvent::Thinking {
            summary: "正在生成回复…".to_string(),
        },
    );

    let codex_input = RuntimeTurnInput {
        text: text.to_string(),
        local_images: attachments
            .iter()
            .map(|attachment| RuntimeLocalImage {
                path: attachment.path.clone(),
            })
            .collect(),
    };

    let state = app_state.inner().clone();
    tauri::async_runtime::spawn(run_codex_turn_after_user_message(
        app,
        state,
        pb,
        workspace_id,
        sid,
        codex_input,
        slot,
        rt,
    ));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_turn_slot_claim_allows_idle_or_error_only() {
        let mut idle = RuntimeSlotStatus::Idle;
        claim_runtime_turn_slot(&mut idle, false).expect("idle runtime can start a turn");
        assert_eq!(idle, RuntimeSlotStatus::Running);

        let mut error = RuntimeSlotStatus::Error;
        claim_runtime_turn_slot(&mut error, false).expect("error runtime can be retried");
        assert_eq!(error, RuntimeSlotStatus::Running);

        for status in [
            RuntimeSlotStatus::Running,
            RuntimeSlotStatus::Pending,
            RuntimeSlotStatus::Cancelling,
        ] {
            let mut status = status;
            let err = claim_runtime_turn_slot(&mut status, false)
                .expect_err("active runtime state must reject a new turn");
            assert!(err.contains("runtime 正在运行"));
            assert_ne!(status, RuntimeSlotStatus::Idle);
        }
    }

    #[test]
    fn runtime_turn_slot_claim_rejects_pending_invalidation() {
        let mut idle = RuntimeSlotStatus::Idle;
        let err = claim_runtime_turn_slot(&mut idle, true)
            .expect_err("pending invalidation must block a new turn");

        assert!(err.contains("runtime context 正在清理"));
        assert_eq!(idle, RuntimeSlotStatus::Idle);
    }

    #[test]
    fn runtime_turn_slot_completion_preserves_error_state() {
        let mut status = RuntimeSlotStatus::Running;
        complete_runtime_turn_slot(&mut status, true);
        assert_eq!(status, RuntimeSlotStatus::Idle);

        let mut status = RuntimeSlotStatus::Running;
        complete_runtime_turn_slot(&mut status, false);
        assert_eq!(status, RuntimeSlotStatus::Error);
    }
}
