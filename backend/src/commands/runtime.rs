use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::runtime::process::RuntimeMetadata;
use crate::runtime::{CodexRuntime, RuntimeConfig};
use crate::services::{
    context_builder, mcp_tool, session as session_svc, skill, workspace as workspace_svc,
};
use crate::state::{AppState, RuntimeSlot, RuntimeSlotStatus};

#[derive(Serialize)]
pub struct RefreshResult {
    pub ok: bool,
    pub dirty: bool,
    pub restart_required: bool,
    pub runtime_status: String,
    pub can_restart_now: bool,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct RuntimeMetadataPayload {
    pub runtime_status: String,
    pub thread_id: Option<String>,
    pub metadata: Option<RuntimeMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeRefreshState {
    restart_required: bool,
    runtime_status: String,
    can_restart_now: bool,
}

fn runtime_refresh_state(runtime_present: bool, status: RuntimeSlotStatus) -> RuntimeRefreshState {
    if !runtime_present {
        return RuntimeRefreshState {
            restart_required: false,
            runtime_status: "uninitialized".to_string(),
            can_restart_now: false,
        };
    }
    let can_restart_now = matches!(status, RuntimeSlotStatus::Idle | RuntimeSlotStatus::Error);
    RuntimeRefreshState {
        restart_required: true,
        runtime_status: status.as_str().to_string(),
        can_restart_now,
    }
}

fn runtime_status_string(runtime_present: bool, status: RuntimeSlotStatus) -> String {
    if runtime_present {
        status.as_str().to_string()
    } else {
        "uninitialized".to_string()
    }
}

fn mark_runtime_cancel_requested(status: &mut RuntimeSlotStatus, runtime_present: bool) -> bool {
    if !runtime_present {
        return false;
    }
    if matches!(
        status,
        RuntimeSlotStatus::Running | RuntimeSlotStatus::Pending
    ) {
        *status = RuntimeSlotStatus::Cancelling;
        return true;
    }
    false
}

fn workspace_key(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

pub(crate) async fn runtime_slot_for_path(
    app_state: &AppState,
    pb: PathBuf,
) -> Result<Arc<RuntimeSlot>, String> {
    let key = workspace_key(&pb);
    let mut pool = app_state.runtime_pool.lock().await;
    Ok(pool
        .entry(key.clone())
        .or_insert_with(|| Arc::new(RuntimeSlot::new(key, pb)))
        .clone())
}

pub(crate) async fn active_runtime_slot(app_state: &AppState) -> Result<Arc<RuntimeSlot>, String> {
    let pb = app_state.require_active_workspace()?;
    runtime_slot_for_path(app_state, pb).await
}

async fn start_workspace_runtime_inner(
    app_state: &AppState,
    workspace_path: Option<PathBuf>,
    force_replace: bool,
) -> Result<(), String> {
    recycle_idle_runtime_slots(app_state).await;
    let slot = match workspace_path {
        Some(pb) => runtime_slot_for_path(app_state, pb).await?,
        None => active_runtime_slot(app_state).await?,
    };
    let pb = slot.workspace_path.clone();
    let workspace_str = pb.to_string_lossy().into_owned();

    if force_replace {
        let status = slot.status.lock().await.clone();
        if !matches!(status, RuntimeSlotStatus::Idle | RuntimeSlotStatus::Error) {
            return Err("当前 workspace runtime 正在运行，请先取消或等待完成后再重启".to_string());
        }
    }

    let status_snapshot = slot.status.lock().await.clone();
    let has_pending_invalidation = slot.pending_invalidation.lock().await.is_some();
    if has_pending_invalidation {
        return Err("当前 workspace runtime context 正在清理，请稍后重试".to_string());
    }
    let replaced = {
        let mut proc_guard = slot.runtime.lock().await;
        if !force_replace && proc_guard.is_some() && status_snapshot != RuntimeSlotStatus::Error {
            return Ok(());
        }
        proc_guard.take()
    };

    if let Some(existing) = replaced {
        existing
            .shutdown_session()
            .await
            .map_err(|e| e.to_string())?;
    }

    let prepared = context_builder::prepare_codex_home(&pb, &app_state.root)?;
    let cfg = RuntimeConfig {
        workspace_path: workspace_str,
        codex_home: prepared.codex_home,
        runtime_home: prepared.runtime_home,
        model: prepared.model,
        api_key: prepared.api_key,
        runtime_workspace_roots: prepared.runtime_workspace_roots,
        approval_policy: prepared.approval_policy,
        sandbox: prepared.sandbox,
    };
    let runtime = Arc::new(CodexRuntime::new(cfg));

    {
        let mut st = app_state.codex_status.lock().await;
        *st = "idle".to_string();
    }

    {
        let mut proc_guard = slot.runtime.lock().await;
        *proc_guard = Some(runtime);
    }
    *slot.status.lock().await = RuntimeSlotStatus::Idle;
    *slot.pending_invalidation.lock().await = None;
    *slot.config_dirty.lock().await = false;
    *slot.last_used_at.lock().await = std::time::Instant::now();
    Ok(())
}

pub async fn recycle_idle_runtime_slots(app_state: &AppState) {
    let slots: Vec<Arc<RuntimeSlot>> = app_state
        .runtime_pool
        .lock()
        .await
        .values()
        .cloned()
        .collect();
    for slot in slots {
        let status = slot.status.lock().await.clone();
        if status != RuntimeSlotStatus::Idle {
            continue;
        }
        let timeout = *slot.idle_timeout.lock().await;
        let idle_for = slot.last_used_at.lock().await.elapsed();
        if idle_for < timeout {
            continue;
        }
        if let Some(runtime) = slot.runtime.lock().await.take() {
            let _ = runtime.shutdown_session().await;
        }
    }
}

pub async fn ensure_workspace_runtime_started_for_path(
    app_state: &AppState,
    pb: PathBuf,
) -> Result<Arc<RuntimeSlot>, String> {
    start_workspace_runtime_inner(app_state, Some(pb.clone()), false).await?;
    runtime_slot_for_path(app_state, pb).await
}

pub(crate) async fn chat_runtime_from_slot(
    slot: &Arc<RuntimeSlot>,
) -> Result<Arc<CodexRuntime>, String> {
    let runtime = slot.runtime.lock().await.clone();
    runtime.ok_or_else(|| "Codex Runtime 未启动".to_string())
}

async fn runtime_slot_for_workspace_id(
    app_state: &AppState,
    workspace_id: &str,
) -> Result<Arc<RuntimeSlot>, String> {
    let ws = workspace_svc::list_known(&app_state.known_workspaces_file)
        .into_iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| "请求所属工作区不存在".to_string())?;
    let pb = PathBuf::from(ws.path);
    runtime_slot_for_path(app_state, pb).await
}

fn workspace_path_for_workspace_id(
    app_state: &AppState,
    workspace_id: &str,
) -> Result<PathBuf, String> {
    if workspace_id.trim().is_empty() {
        return app_state.require_active_workspace();
    }
    let ws = workspace_svc::list_known(&app_state.known_workspaces_file)
        .into_iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| "请求所属工作区不存在".to_string())?;
    Ok(PathBuf::from(ws.path))
}

async fn chat_runtime_for_owner(
    app_state: &AppState,
    workspace_id: &str,
    session_id: Option<&str>,
) -> Result<Arc<CodexRuntime>, String> {
    let slot = runtime_slot_for_workspace_id(app_state, workspace_id).await?;
    if let Some(sid) = session_id {
        if !session_svc::session_exists(slot.workspace_path.as_path(), sid) {
            return Err("请求所属会话不存在".to_string());
        }
    }
    chat_runtime_from_slot(&slot).await
}

#[tauri::command]
pub async fn get_runtime_status(app_state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let slot = active_runtime_slot(&app_state).await?;
    let rt = slot.runtime.lock().await.clone();
    let status = slot.status.lock().await.clone();
    Ok(runtime_status_string(rt.is_some(), status))
}

#[tauri::command]
pub async fn get_runtime_metadata(
    app_state: State<'_, Arc<AppState>>,
) -> Result<RuntimeMetadataPayload, String> {
    let slot = active_runtime_slot(&app_state).await?;
    let rt = slot.runtime.lock().await.clone();
    let status = slot.status.lock().await.as_str().to_string();
    match rt {
        None => Ok(RuntimeMetadataPayload {
            runtime_status: "uninitialized".to_string(),
            thread_id: None,
            metadata: None,
        }),
        Some(runtime) => Ok(RuntimeMetadataPayload {
            runtime_status: status,
            thread_id: runtime.thread_id().await,
            metadata: runtime.runtime_metadata().await,
        }),
    }
}

#[tauri::command]
pub async fn cancel_current_turn(
    app_state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<(), String> {
    let slot = runtime_slot_for_workspace_id(&app_state, &workspace_id).await?;
    let rt = slot.runtime.lock().await.clone();
    let should_cancel = {
        let mut status = slot.status.lock().await;
        mark_runtime_cancel_requested(&mut status, rt.is_some())
    };
    if should_cancel {
        let Some(rt) = rt else {
            return Ok(());
        };
        rt.cancel_current_turn().await;
    }

    Ok(())
}

#[tauri::command]
pub async fn respond_runtime_approval(
    app_state: State<'_, Arc<AppState>>,
    workspace_id: String,
    session_id: Option<String>,
    approval_id: String,
    decision: String,
) -> Result<(), String> {
    match decision.as_str() {
        "accept" | "acceptForSession" | "decline" | "cancel" => {}
        _ => return Err("不支持的审批决定".to_string()),
    }

    let rt = chat_runtime_for_owner(&app_state, &workspace_id, session_id.as_deref()).await?;
    rt.validate_pending_request_owner(&approval_id, &workspace_id, session_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    rt.send_approval_decision(approval_id.clone(), decision)
        .await
        .map_err(|e| format!("发送审批结果失败: {e}"))?;
    rt.clear_pending_request(&approval_id).await;
    Ok(())
}

#[tauri::command]
pub async fn respond_runtime_permissions(
    app_state: State<'_, Arc<AppState>>,
    workspace_id: String,
    session_id: Option<String>,
    request_id: String,
    granted: bool,
    permissions: serde_json::Value,
    scope: serde_json::Value,
    strict_auto_review: Option<bool>,
) -> Result<(), String> {
    let rt = chat_runtime_for_owner(&app_state, &workspace_id, session_id.as_deref()).await?;
    rt.validate_pending_request_owner(&request_id, &workspace_id, session_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    rt.send_permissions_response(crate::runtime::process::PermissionsResponse {
        request_id: request_id.clone(),
        granted,
        permissions,
        scope,
        strict_auto_review,
    })
    .await
    .map_err(|e| format!("发送 permissions 决定失败: {e}"))?;
    rt.clear_pending_request(&request_id).await;
    Ok(())
}

#[tauri::command]
pub async fn respond_runtime_mcp_elicitation(
    app_state: State<'_, Arc<AppState>>,
    workspace_id: String,
    session_id: Option<String>,
    request_id: String,
    action: String,
    content: Option<serde_json::Value>,
    meta: Option<serde_json::Value>,
) -> Result<(), String> {
    match action.as_str() {
        "accept" | "decline" | "cancel" => {}
        _ => return Err("不支持的 elicitation 动作".to_string()),
    }
    let rt = chat_runtime_for_owner(&app_state, &workspace_id, session_id.as_deref()).await?;
    rt.validate_pending_request_owner(&request_id, &workspace_id, session_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    rt.send_mcp_elicitation_response(crate::runtime::process::McpElicitationResponse {
        request_id: request_id.clone(),
        action,
        content,
        meta,
    })
    .await
    .map_err(|e| format!("发送 elicitation 结果失败: {e}"))?;
    rt.clear_pending_request(&request_id).await;
    Ok(())
}

#[tauri::command]
pub async fn respond_runtime_user_input(
    app_state: State<'_, Arc<AppState>>,
    workspace_id: String,
    session_id: Option<String>,
    request_id: String,
    answers: serde_json::Value,
) -> Result<(), String> {
    let rt = chat_runtime_for_owner(&app_state, &workspace_id, session_id.as_deref()).await?;
    rt.validate_pending_request_owner(&request_id, &workspace_id, session_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    rt.send_user_input_answers(request_id.clone(), answers)
        .await
        .map_err(|e| format!("发送用户输入结果失败: {e}"))?;
    rt.clear_pending_request(&request_id).await;
    Ok(())
}

#[tauri::command]
pub async fn start_workspace_runtime(
    app_state: State<'_, Arc<AppState>>,
    workspace_id: Option<String>,
) -> Result<(), String> {
    let workspace_path = match workspace_id.as_deref() {
        Some(id) if !id.trim().is_empty() => Some(workspace_path_for_workspace_id(&app_state, id)?),
        _ => None,
    };
    start_workspace_runtime_inner(&app_state, workspace_path.clone(), true).await?;
    let slot = match workspace_path {
        Some(pb) => runtime_slot_for_path(&app_state, pb).await?,
        None => active_runtime_slot(&app_state).await?,
    };
    let runtime = chat_runtime_from_slot(&slot).await?;
    runtime
        .ensure_session_started()
        .await
        .map_err(|e| format!("启动 workspace runtime 失败: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn refresh_runtime_context(
    workspace_id: String,
    session_id: Option<String>,
    app_state: State<'_, Arc<AppState>>,
) -> Result<RefreshResult, String> {
    let _ = session_id;
    let ws = workspace_path_for_workspace_id(&app_state, &workspace_id)?;
    refresh_runtime_context_for_workspace_path(&app_state, ws).await
}

pub(crate) async fn refresh_runtime_context_for_workspace_path(
    app_state: &AppState,
    ws: PathBuf,
) -> Result<RefreshResult, String> {
    let _selection = skill::read_skill_selection(&ws);
    if let Some(policy) = mcp_tool::read_tool_policy(&ws) {
        mcp_tool::sync_legacy_tool_policy(&app_state.root.mcp_dir(), &ws, &policy)?;
    }

    context_builder::prepare_codex_home(&ws, &app_state.root)?;

    let view = skill::build_skill_list_view(&app_state.root.skills_dir(), Some(&ws));
    let dirty = view
        .root_catalog
        .iter()
        .chain(view.workspace_local.iter())
        .any(|s| s.runtime_status == "dirty");

    let slot = runtime_slot_for_path(app_state, ws).await?;
    let runtime_present = slot.runtime.lock().await.is_some();
    let status = slot.status.lock().await.clone();
    let refresh_state = runtime_refresh_state(runtime_present, status);
    let message = if !refresh_state.restart_required {
        "Runtime context 已刷新；下次发送消息时会直接加载新配置".to_string()
    } else if refresh_state.can_restart_now {
        "Runtime context 诊断已刷新；已有 runtime 不会热替换，产品配置变更由 backend lifecycle invalidation 接管".to_string()
    } else {
        "Runtime context 诊断已刷新；当前 workspace runtime 正在运行，产品配置变更会在本轮结束后由 backend 清理"
            .to_string()
    };

    Ok(RefreshResult {
        ok: true,
        dirty,
        restart_required: refresh_state.restart_required,
        runtime_status: refresh_state.runtime_status,
        can_restart_now: refresh_state.can_restart_now,
        message: Some(message),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicU16;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use crate::runtime::lifecycle::{
        RuntimeInvalidationMark, RuntimeInvalidationReason, RuntimeInvalidationScope,
    };
    use crate::services::root_workspace;

    fn test_runtime(workspace_path: &std::path::Path) -> Arc<CodexRuntime> {
        Arc::new(CodexRuntime::new(RuntimeConfig {
            workspace_path: workspace_path.to_string_lossy().into_owned(),
            codex_home: workspace_path
                .join(".codex-home")
                .to_string_lossy()
                .into_owned(),
            runtime_home: workspace_path
                .join(".runtime-home")
                .to_string_lossy()
                .into_owned(),
            model: String::new(),
            api_key: String::new(),
            runtime_workspace_roots: vec![workspace_path.to_string_lossy().into_owned()],
            approval_policy: "on-request".to_string(),
            sandbox: "workspace-write".to_string(),
        }))
    }

    fn test_app_state(
        root: std::sync::Arc<crate::services::root_workspace::RootWorkspace>,
    ) -> AppState {
        AppState {
            known_workspaces_file: root.known_workspaces_path(),
            root,
            active_workspace_path: Mutex::new(None),
            active_session_id: Mutex::new(None),
            runtime_pool: tokio::sync::Mutex::new(HashMap::new()),
            codex_status: tokio::sync::Mutex::new("idle".to_string()),
            turn_cancel: Arc::new(AtomicBool::new(false)),
            transcript_write_lock: Mutex::new(()),
            employee_write_lock: Mutex::new(()),
            http_server_port: AtomicU16::new(0),
            import_queues: Mutex::new(HashMap::new()),
            dream_runtime: tokio::sync::Mutex::new(None),
            dream_status: tokio::sync::Mutex::new("idle".to_string()),
        }
    }

    async fn expired_slot_with_status(
        app_state: &AppState,
        workspace_path: PathBuf,
        status: RuntimeSlotStatus,
    ) -> Arc<RuntimeSlot> {
        let slot = runtime_slot_for_path(app_state, workspace_path.clone())
            .await
            .expect("slot");
        *slot.runtime.lock().await = Some(test_runtime(&workspace_path));
        *slot.status.lock().await = status;
        *slot.idle_timeout.lock().await = Duration::ZERO;
        *slot.last_used_at.lock().await = Instant::now() - Duration::from_secs(60);
        slot
    }

    #[test]
    fn runtime_refresh_state_marks_existing_idle_runtime_restartable() {
        let state = runtime_refresh_state(true, RuntimeSlotStatus::Idle);

        assert!(state.restart_required);
        assert!(state.can_restart_now);
        assert_eq!(state.runtime_status, "idle");
    }

    #[test]
    fn runtime_refresh_state_marks_running_runtime_manual_later() {
        let state = runtime_refresh_state(true, RuntimeSlotStatus::Running);

        assert!(state.restart_required);
        assert!(!state.can_restart_now);
        assert_eq!(state.runtime_status, "running");
    }

    #[test]
    fn runtime_refresh_state_only_allows_immediate_restart_when_idle_or_error() {
        for status in [
            RuntimeSlotStatus::Idle,
            RuntimeSlotStatus::Running,
            RuntimeSlotStatus::Pending,
            RuntimeSlotStatus::Cancelling,
            RuntimeSlotStatus::Error,
        ] {
            let state = runtime_refresh_state(true, status.clone());

            assert!(state.restart_required);
            assert_eq!(state.runtime_status, status.as_str());
            assert_eq!(
                state.can_restart_now,
                matches!(status, RuntimeSlotStatus::Idle | RuntimeSlotStatus::Error),
                "restart availability must match lifecycle state for {status:?}"
            );
        }
    }

    #[test]
    fn runtime_refresh_state_does_not_require_restart_without_runtime() {
        let state = runtime_refresh_state(false, RuntimeSlotStatus::Idle);

        assert!(!state.restart_required);
        assert!(!state.can_restart_now);
        assert_eq!(state.runtime_status, "uninitialized");
    }

    #[test]
    fn runtime_status_string_is_single_structured_status() {
        assert_eq!(
            runtime_status_string(false, RuntimeSlotStatus::Idle),
            "uninitialized"
        );

        for status in [
            RuntimeSlotStatus::Idle,
            RuntimeSlotStatus::Running,
            RuntimeSlotStatus::Pending,
            RuntimeSlotStatus::Cancelling,
            RuntimeSlotStatus::Error,
        ] {
            let value = runtime_status_string(true, status.clone());
            assert_eq!(value, status.as_str());
            assert!(
                !value.contains('|') && !value.contains("thread="),
                "runtime status command must not expose compound string protocol"
            );
        }
    }

    #[test]
    fn runtime_cancel_request_marks_only_active_slots() {
        for mut status in [RuntimeSlotStatus::Running, RuntimeSlotStatus::Pending] {
            assert!(
                mark_runtime_cancel_requested(&mut status, true),
                "active {status:?} slot must forward cancel"
            );
            assert_eq!(status, RuntimeSlotStatus::Cancelling);
        }
    }

    #[test]
    fn runtime_cancel_request_is_noop_for_inactive_or_missing_runtime() {
        for mut status in [
            RuntimeSlotStatus::Idle,
            RuntimeSlotStatus::Error,
            RuntimeSlotStatus::Cancelling,
        ] {
            let original = status.clone();
            assert!(
                !mark_runtime_cancel_requested(&mut status, true),
                "inactive {original:?} slot must not forward cancel"
            );
            assert_eq!(status, original);
        }

        for mut status in [
            RuntimeSlotStatus::Idle,
            RuntimeSlotStatus::Running,
            RuntimeSlotStatus::Pending,
            RuntimeSlotStatus::Cancelling,
            RuntimeSlotStatus::Error,
        ] {
            let original = status.clone();
            assert!(
                !mark_runtime_cancel_requested(&mut status, false),
                "slot without runtime must not enter cancelling"
            );
            assert_eq!(status, original);
        }
    }

    #[tokio::test]
    async fn idle_recycle_drops_only_expired_idle_runtime() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("init root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-idle");
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let slot = expired_slot_with_status(&app_state, workspace, RuntimeSlotStatus::Idle).await;

        recycle_idle_runtime_slots(&app_state).await;

        assert!(
            slot.runtime.lock().await.is_none(),
            "expired idle runtime must be recycled"
        );
        assert_eq!(
            *slot.status.lock().await,
            RuntimeSlotStatus::Idle,
            "recycling an idle runtime must not invent an active state"
        );
    }

    #[tokio::test]
    async fn idle_recycle_preserves_active_or_blocked_runtime_slots() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("init root"));
        let app_state = test_app_state(root);

        for status in [
            RuntimeSlotStatus::Running,
            RuntimeSlotStatus::Pending,
            RuntimeSlotStatus::Cancelling,
            RuntimeSlotStatus::Error,
        ] {
            let workspace = tmp.path().join(format!("workspace-{}", status.as_str()));
            std::fs::create_dir_all(&workspace).expect("workspace dir");
            expired_slot_with_status(&app_state, workspace, status).await;
        }

        recycle_idle_runtime_slots(&app_state).await;

        let slots: Vec<Arc<RuntimeSlot>> = app_state
            .runtime_pool
            .lock()
            .await
            .values()
            .cloned()
            .collect();
        for slot in slots {
            let status = slot.status.lock().await.clone();
            assert!(
                slot.runtime.lock().await.is_some(),
                "expired {status:?} runtime must not be recycled by idle timeout"
            );
        }
    }

    #[tokio::test]
    async fn refresh_context_is_diagnostic_and_does_not_mark_running_slot_dirty() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("init root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-refresh-running");
        std::fs::create_dir_all(workspace.join(".chawork")).expect("workspace dir");
        let slot =
            expired_slot_with_status(&app_state, workspace.clone(), RuntimeSlotStatus::Running)
                .await;

        let result = refresh_runtime_context_for_workspace_path(&app_state, workspace)
            .await
            .expect("refresh context");

        assert!(result.ok);
        assert!(result.restart_required);
        assert!(!result.can_restart_now);
        assert_eq!(result.runtime_status, RuntimeSlotStatus::Running.as_str());
        assert!(
            !*slot.config_dirty.lock().await,
            "diagnostic refresh must not mark product lifecycle dirty"
        );
        assert!(
            slot.runtime.lock().await.is_some(),
            "refreshing a running runtime must not replace or clear the runtime"
        );
    }

    #[tokio::test]
    async fn refresh_context_without_runtime_loads_next_turn_without_restart() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("init root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-refresh-unstarted");
        std::fs::create_dir_all(workspace.join(".chawork")).expect("workspace dir");

        let result = refresh_runtime_context_for_workspace_path(&app_state, workspace.clone())
            .await
            .expect("refresh context");
        let slot = runtime_slot_for_path(&app_state, workspace)
            .await
            .expect("slot");

        assert!(result.ok);
        assert!(!result.restart_required);
        assert!(!result.can_restart_now);
        assert_eq!(result.runtime_status, "uninitialized");
        assert!(
            !*slot.config_dirty.lock().await,
            "unstarted runtime refresh must not force a restart"
        );
        assert!(slot.runtime.lock().await.is_none());
    }

    #[tokio::test]
    async fn start_runtime_rejects_pending_cleanup_even_after_runtime_detach() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("init root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-pending-cleanup");
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let slot = runtime_slot_for_path(&app_state, workspace.clone())
            .await
            .expect("slot");
        *slot.status.lock().await = RuntimeSlotStatus::Running;
        *slot.runtime.lock().await = None;
        *slot.pending_invalidation.lock().await = Some(RuntimeInvalidationMark {
            reason: RuntimeInvalidationReason::ProviderChanged,
            scope: RuntimeInvalidationScope::Global,
            scope_identity: "global".to_string(),
            invalidation_id: "runtime_invalidation_test".to_string(),
            created_at: "2026-06-14T00:00:00Z".to_string(),
            user_message_key: None,
            affected_workspaces: Vec::new(),
            message: None,
        });

        let err = start_workspace_runtime_inner(&app_state, Some(workspace), false)
            .await
            .expect_err("pending cleanup must reject new runtime start");

        assert!(err.contains("正在清理"));
        assert!(slot.runtime.lock().await.is_none());
        assert!(slot.pending_invalidation.lock().await.is_some());
    }

    #[test]
    fn interactive_runtime_does_not_depend_on_codex_exec_binary_or_jsonl_parser() {
        let runtime_commands = include_str!("runtime.rs");
        for forbidden in [
            concat!("default_", "codex_cli"),
            concat!("resolve_", "codex_executable"),
            concat!("codex_", "binary"),
        ] {
            assert!(
                !runtime_commands.contains(forbidden),
                "interactive runtime startup must not depend on Codex exec binary: {forbidden}"
            );
        }

        let process = include_str!("../runtime/process.rs");
        for forbidden in [
            concat!("exec_", "turn("),
            concat!("exec_", "turn_on("),
            concat!("codex ", "exec"),
            concat!("CODEX_", "JSON_FLAG"),
            concat!("Thread", "Event"),
            concat!("Thread", "ItemDetails"),
            concat!("CHAWORK_", "APPROVAL_DIR"),
        ] {
            assert!(
                !process.contains(forbidden),
                "interactive backend runtime must not retain Codex exec JSONL path: {forbidden}"
            );
        }
    }
}
