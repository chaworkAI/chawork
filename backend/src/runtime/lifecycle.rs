use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime as TauriRuntime};
use uuid::Uuid;

use crate::runtime::CodexRuntime;
use crate::services::employee;
use crate::services::workspace as workspace_svc;
use crate::state::{AppState, RuntimeSlot, RuntimeSlotStatus};

pub const RUNTIME_LIFECYCLE_INVALIDATED_EVENT: &str = "runtime-lifecycle/invalidated";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInvalidationReason {
    ProviderChanged,
    EmployeePromptChanged,
    EmployeeSkillsChanged,
    DreamPromptApplied,
    WorkspaceBindingChanged,
    WorkspaceCodexHomeContextChanged,
    McpContextChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInvalidationScope {
    Global,
    Employee,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInvalidationPhase {
    Marked,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInvalidationMode {
    Noop,
    Immediate,
    Deferred,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInvalidationUserMessageKey {
    SettingsSavedNoActiveTurn,
    SettingsSavedActiveTaskUsesPrevious,
    ProviderSettingsSavedActiveTaskUsesPrevious,
    EmployeeSettingsSavedActiveTaskUsesPrevious,
    DreamPromptAppliedLaterMessages,
    WorkspaceBindingSavedLaterMessages,
    SettingsSavedCleanupWarning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInvalidationAffectedWorkspace {
    pub workspace_id: String,
    pub workspace_path: String,
    pub previous_status: String,
    pub mode: RuntimeInvalidationMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInvalidationResult {
    pub ok: bool,
    pub invalidation_id: String,
    pub phase: RuntimeInvalidationPhase,
    pub reason: RuntimeInvalidationReason,
    pub scope: RuntimeInvalidationScope,
    pub scope_identity: String,
    pub user_message_key: Option<RuntimeInvalidationUserMessageKey>,
    pub invalidated_now_count: usize,
    pub deferred_count: usize,
    pub affected_workspaces: Vec<RuntimeInvalidationAffectedWorkspace>,
    pub termination_warnings: Vec<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationResult<T>
where
    T: Serialize,
{
    pub ok: bool,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationWithRuntimeInvalidation<T>
where
    T: Serialize,
{
    pub mutation: MutationResult<T>,
    pub runtime_invalidation: RuntimeInvalidationResult,
}

impl<T> MutationWithRuntimeInvalidation<T>
where
    T: Serialize,
{
    pub fn success(payload: T, runtime_invalidation: RuntimeInvalidationResult) -> Self {
        Self {
            mutation: MutationResult { ok: true, payload },
            runtime_invalidation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInvalidationMark {
    pub reason: RuntimeInvalidationReason,
    pub scope: RuntimeInvalidationScope,
    pub scope_identity: String,
    pub invalidation_id: String,
    pub created_at: String,
    pub user_message_key: Option<RuntimeInvalidationUserMessageKey>,
    pub affected_workspaces: Vec<RuntimeInvalidationAffectedWorkspace>,
    pub message: Option<String>,
}

struct SlotInvalidationOutcome {
    affected: RuntimeInvalidationAffectedWorkspace,
    runtime_to_shutdown: Option<Arc<CodexRuntime>>,
}

fn new_invalidation_id() -> String {
    format!("runtime_invalidation_{}", Uuid::new_v4())
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn canonical_path_string(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn workspace_key(path: &Path) -> String {
    canonical_path_string(path)
}

async fn runtime_slot_for_path(app_state: &AppState, path: PathBuf) -> Arc<RuntimeSlot> {
    let key = workspace_key(&path);
    let mut pool = app_state.runtime_pool.lock().await;
    pool.entry(key.clone())
        .or_insert_with(|| Arc::new(RuntimeSlot::new(key, path)))
        .clone()
}

fn workspace_identity(app_state: &AppState, slot: &RuntimeSlot) -> (String, String) {
    let path = canonical_path_string(&slot.workspace_path);
    let workspace_id = workspace_svc::read_workspace(&slot.workspace_path)
        .map(|workspace| workspace.id)
        .or_else(|_| {
            workspace_svc::list_known(&app_state.known_workspaces_file)
                .into_iter()
                .find(|workspace| canonical_path_string(Path::new(&workspace.path)) == path)
                .map(|workspace| workspace.id)
                .ok_or_else(|| "workspace not known".to_string())
        })
        .unwrap_or_else(|_| slot.workspace_key.clone());
    (workspace_id, path)
}

fn previous_status(runtime_present: bool, status: &RuntimeSlotStatus) -> String {
    if runtime_present {
        status.as_str().to_string()
    } else {
        "uninitialized".to_string()
    }
}

fn user_message_key(
    phase: RuntimeInvalidationPhase,
    reason: RuntimeInvalidationReason,
    deferred_count: usize,
    affected_count: usize,
    termination_warnings: &[String],
) -> Option<RuntimeInvalidationUserMessageKey> {
    if phase == RuntimeInvalidationPhase::Completed {
        return None;
    }
    if !termination_warnings.is_empty() {
        return Some(RuntimeInvalidationUserMessageKey::SettingsSavedCleanupWarning);
    }
    if affected_count == 0 {
        return None;
    }
    match reason {
        RuntimeInvalidationReason::ProviderChanged if deferred_count > 0 => {
            Some(RuntimeInvalidationUserMessageKey::ProviderSettingsSavedActiveTaskUsesPrevious)
        }
        RuntimeInvalidationReason::EmployeePromptChanged
        | RuntimeInvalidationReason::EmployeeSkillsChanged
            if deferred_count > 0 =>
        {
            Some(RuntimeInvalidationUserMessageKey::EmployeeSettingsSavedActiveTaskUsesPrevious)
        }
        RuntimeInvalidationReason::DreamPromptApplied => {
            Some(RuntimeInvalidationUserMessageKey::DreamPromptAppliedLaterMessages)
        }
        RuntimeInvalidationReason::WorkspaceBindingChanged => {
            Some(RuntimeInvalidationUserMessageKey::WorkspaceBindingSavedLaterMessages)
        }
        _ if deferred_count > 0 => {
            Some(RuntimeInvalidationUserMessageKey::SettingsSavedActiveTaskUsesPrevious)
        }
        _ => Some(RuntimeInvalidationUserMessageKey::SettingsSavedNoActiveTurn),
    }
}

async fn invalidate_slot(
    app_state: &AppState,
    slot: &Arc<RuntimeSlot>,
    mark: &RuntimeInvalidationMark,
) -> SlotInvalidationOutcome {
    let status = slot.status.lock().await.clone();
    let runtime_present = slot.runtime.lock().await.is_some();
    let (workspace_id, workspace_path) = workspace_identity(app_state, slot);
    let previous_status = previous_status(runtime_present, &status);

    if !runtime_present {
        {
            let mut status_guard = slot.status.lock().await;
            *status_guard = RuntimeSlotStatus::Idle;
        }
        {
            let mut pending = slot.pending_invalidation.lock().await;
            *pending = None;
        }
        return SlotInvalidationOutcome {
            affected: RuntimeInvalidationAffectedWorkspace {
                workspace_id,
                workspace_path,
                previous_status,
                mode: RuntimeInvalidationMode::Noop,
            },
            runtime_to_shutdown: None,
        };
    }

    if matches!(status, RuntimeSlotStatus::Idle | RuntimeSlotStatus::Error) {
        let runtime_to_shutdown = {
            let mut runtime = slot.runtime.lock().await;
            runtime.take()
        };
        {
            let mut pending = slot.pending_invalidation.lock().await;
            *pending = None;
        }
        {
            let mut status_guard = slot.status.lock().await;
            *status_guard = RuntimeSlotStatus::Idle;
        }
        *slot.last_used_at.lock().await = Instant::now();
        return SlotInvalidationOutcome {
            affected: RuntimeInvalidationAffectedWorkspace {
                workspace_id,
                workspace_path,
                previous_status,
                mode: RuntimeInvalidationMode::Immediate,
            },
            runtime_to_shutdown,
        };
    }

    {
        let mut pending = slot.pending_invalidation.lock().await;
        let mut slot_mark = mark.clone();
        slot_mark.affected_workspaces = vec![RuntimeInvalidationAffectedWorkspace {
            workspace_id: workspace_id.clone(),
            workspace_path: workspace_path.clone(),
            previous_status: previous_status.clone(),
            mode: RuntimeInvalidationMode::Deferred,
        }];
        *pending = Some(slot_mark);
    }

    SlotInvalidationOutcome {
        affected: RuntimeInvalidationAffectedWorkspace {
            workspace_id,
            workspace_path,
            previous_status,
            mode: RuntimeInvalidationMode::Deferred,
        },
        runtime_to_shutdown: None,
    }
}

async fn shutdown_detached_runtime(
    runtime: Option<Arc<CodexRuntime>>,
    workspace_path: &str,
) -> Option<String> {
    let runtime = runtime?;
    runtime.shutdown_session().await.err().map(|err| {
        format!("旧 runtime 已从 slot 脱离，但 graceful shutdown 失败 ({workspace_path}): {err}")
    })
}

async fn build_marked_result<R: TauriRuntime>(
    app_state: &AppState,
    app: &AppHandle<R>,
    slots: Vec<Arc<RuntimeSlot>>,
    reason: RuntimeInvalidationReason,
    scope: RuntimeInvalidationScope,
    scope_identity: String,
) -> RuntimeInvalidationResult {
    let invalidation_id = new_invalidation_id();
    let mark = RuntimeInvalidationMark {
        reason,
        scope,
        scope_identity: scope_identity.clone(),
        invalidation_id: invalidation_id.clone(),
        created_at: iso_now(),
        user_message_key: None,
        affected_workspaces: Vec::new(),
        message: None,
    };

    let mut affected_workspaces = Vec::new();
    let mut runtimes_to_shutdown = Vec::new();

    for slot in slots {
        let outcome = invalidate_slot(app_state, &slot, &mark).await;
        if matches!(outcome.affected.mode, RuntimeInvalidationMode::Immediate) {
            runtimes_to_shutdown.push((
                outcome.runtime_to_shutdown,
                outcome.affected.workspace_path.clone(),
            ));
        }
        affected_workspaces.push(outcome.affected);
    }

    let mut termination_warnings = Vec::new();
    for (runtime, workspace_path) in runtimes_to_shutdown {
        if let Some(warning) = shutdown_detached_runtime(runtime, &workspace_path).await {
            termination_warnings.push(warning);
        }
    }

    let invalidated_now_count = affected_workspaces
        .iter()
        .filter(|workspace| workspace.mode == RuntimeInvalidationMode::Immediate)
        .count();
    let deferred_count = affected_workspaces
        .iter()
        .filter(|workspace| workspace.mode == RuntimeInvalidationMode::Deferred)
        .count();
    let user_message_key = user_message_key(
        RuntimeInvalidationPhase::Marked,
        reason,
        deferred_count,
        invalidated_now_count + deferred_count,
        &termination_warnings,
    );

    let slots_for_pending_update = app_state
        .runtime_pool
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for slot in slots_for_pending_update {
        let mut pending = slot.pending_invalidation.lock().await;
        if let Some(mark) = pending.as_mut() {
            if mark.invalidation_id == invalidation_id {
                mark.user_message_key = user_message_key;
            }
        }
    }

    let result = RuntimeInvalidationResult {
        ok: true,
        invalidation_id,
        phase: RuntimeInvalidationPhase::Marked,
        reason,
        scope,
        scope_identity,
        user_message_key,
        invalidated_now_count,
        deferred_count,
        affected_workspaces,
        termination_warnings,
        message: None,
        error: None,
    };

    if !result.affected_workspaces.is_empty() {
        let _ = app.emit(RUNTIME_LIFECYCLE_INVALIDATED_EVENT, &result);
    }

    result
}

pub async fn invalidate_all_chat_runtimes<R: TauriRuntime>(
    app_state: &AppState,
    app: &AppHandle<R>,
    reason: RuntimeInvalidationReason,
) -> RuntimeInvalidationResult {
    let slots = app_state
        .runtime_pool
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    build_marked_result(
        app_state,
        app,
        slots,
        reason,
        RuntimeInvalidationScope::Global,
        "global".to_string(),
    )
    .await
}

pub async fn invalidate_workspace_chat_runtime<R: TauriRuntime>(
    app_state: &AppState,
    app: &AppHandle<R>,
    workspace_path: PathBuf,
    reason: RuntimeInvalidationReason,
) -> RuntimeInvalidationResult {
    let slot = runtime_slot_for_path(app_state, workspace_path.clone()).await;
    build_marked_result(
        app_state,
        app,
        vec![slot],
        reason,
        RuntimeInvalidationScope::Workspace,
        canonical_path_string(&workspace_path),
    )
    .await
}

pub fn noop_workspace_runtime_invalidation(
    workspace_path: &Path,
    reason: RuntimeInvalidationReason,
) -> RuntimeInvalidationResult {
    RuntimeInvalidationResult {
        ok: true,
        invalidation_id: new_invalidation_id(),
        phase: RuntimeInvalidationPhase::Marked,
        reason,
        scope: RuntimeInvalidationScope::Workspace,
        scope_identity: canonical_path_string(workspace_path),
        user_message_key: None,
        invalidated_now_count: 0,
        deferred_count: 0,
        affected_workspaces: Vec::new(),
        termination_warnings: Vec::new(),
        message: None,
        error: None,
    }
}

pub async fn invalidate_employee_chat_runtimes<R: TauriRuntime>(
    app_state: &AppState,
    app: &AppHandle<R>,
    employee_id: &str,
    reason: RuntimeInvalidationReason,
) -> RuntimeInvalidationResult {
    let workspaces = match employee::list_workspaces_for_employee(&app_state.root, employee_id) {
        Ok(workspaces) => workspaces,
        Err(err) => {
            return RuntimeInvalidationResult {
                ok: false,
                invalidation_id: new_invalidation_id(),
                phase: RuntimeInvalidationPhase::Marked,
                reason,
                scope: RuntimeInvalidationScope::Employee,
                scope_identity: employee_id.to_string(),
                user_message_key: Some(
                    RuntimeInvalidationUserMessageKey::SettingsSavedCleanupWarning,
                ),
                invalidated_now_count: 0,
                deferred_count: 0,
                affected_workspaces: Vec::new(),
                termination_warnings: Vec::new(),
                message: None,
                error: Some(err),
            };
        }
    };
    let mut slots = Vec::new();
    for workspace in workspaces {
        slots.push(runtime_slot_for_path(app_state, PathBuf::from(workspace.path)).await);
    }
    build_marked_result(
        app_state,
        app,
        slots,
        reason,
        RuntimeInvalidationScope::Employee,
        employee_id.to_string(),
    )
    .await
}

pub async fn complete_pending_invalidation_after_turn<R: TauriRuntime>(
    _app_state: &AppState,
    app: &AppHandle<R>,
    slot: &Arc<RuntimeSlot>,
) -> RuntimeInvalidationResult {
    let mark = {
        let pending = slot.pending_invalidation.lock().await;
        pending.clone()
    };
    let Some(mark) = mark else {
        return RuntimeInvalidationResult {
            ok: true,
            invalidation_id: new_invalidation_id(),
            phase: RuntimeInvalidationPhase::Completed,
            reason: RuntimeInvalidationReason::WorkspaceCodexHomeContextChanged,
            scope: RuntimeInvalidationScope::Workspace,
            scope_identity: canonical_path_string(&slot.workspace_path),
            user_message_key: None,
            invalidated_now_count: 0,
            deferred_count: 0,
            affected_workspaces: Vec::new(),
            termination_warnings: Vec::new(),
            message: None,
            error: None,
        };
    };

    let runtime_to_shutdown = {
        let mut runtime = slot.runtime.lock().await;
        runtime.take()
    };

    let mut affected_workspaces = mark.affected_workspaces.clone();
    for workspace in &mut affected_workspaces {
        workspace.mode = RuntimeInvalidationMode::Completed;
    }
    let mut termination_warnings = Vec::new();
    if let Some(warning) = shutdown_detached_runtime(
        runtime_to_shutdown,
        affected_workspaces
            .first()
            .map(|workspace| workspace.workspace_path.as_str())
            .unwrap_or_else(|| slot.workspace_path.to_str().unwrap_or("unknown")),
    )
    .await
    {
        termination_warnings.push(warning);
    }

    {
        let mut pending = slot.pending_invalidation.lock().await;
        if pending
            .as_ref()
            .map(|current| current.invalidation_id.as_str())
            == Some(mark.invalidation_id.as_str())
        {
            *pending = None;
        }
    }
    {
        let mut status = slot.status.lock().await;
        *status = RuntimeSlotStatus::Idle;
    }
    *slot.last_used_at.lock().await = Instant::now();

    let result = RuntimeInvalidationResult {
        ok: true,
        invalidation_id: mark.invalidation_id,
        phase: RuntimeInvalidationPhase::Completed,
        reason: mark.reason,
        scope: mark.scope,
        scope_identity: mark.scope_identity,
        user_message_key: None,
        invalidated_now_count: affected_workspaces.len(),
        deferred_count: 0,
        affected_workspaces,
        termination_warnings,
        message: None,
        error: None,
    };
    let _ = app.emit(RUNTIME_LIFECYCLE_INVALIDATED_EVENT, &result);
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU16};
    use std::sync::{Arc, Mutex};

    use crate::runtime::process::RuntimeConfig;
    use crate::runtime::CodexRuntime;
    use crate::services::employee;
    use crate::services::root_workspace;
    use crate::services::workspace;
    use crate::state::{AppState, RuntimeSlotStatus};

    use super::{
        complete_pending_invalidation_after_turn, invalidate_all_chat_runtimes,
        invalidate_employee_chat_runtimes, invalidate_workspace_chat_runtime,
        RuntimeInvalidationMode, RuntimeInvalidationPhase, RuntimeInvalidationReason,
        RuntimeInvalidationScope, RuntimeInvalidationUserMessageKey,
    };

    fn test_app_state(root: Arc<crate::services::root_workspace::RootWorkspace>) -> AppState {
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

    async fn slot_with_status(
        app_state: &AppState,
        workspace_path: PathBuf,
        status: RuntimeSlotStatus,
    ) -> Arc<crate::state::RuntimeSlot> {
        let slot =
            crate::commands::runtime::runtime_slot_for_path(app_state, workspace_path.clone())
                .await
                .expect("slot");
        *slot.runtime.lock().await = Some(test_runtime(&workspace_path));
        *slot.status.lock().await = status;
        slot
    }

    #[tokio::test]
    async fn immediate_invalidation_detaches_idle_runtime() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-idle");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let slot = slot_with_status(&app_state, workspace.clone(), RuntimeSlotStatus::Idle).await;
        let app = tauri::test::mock_app();

        let result = invalidate_workspace_chat_runtime(
            &app_state,
            app.handle(),
            workspace.clone(),
            RuntimeInvalidationReason::WorkspaceBindingChanged,
        )
        .await;

        assert!(result.ok);
        assert_eq!(result.phase, RuntimeInvalidationPhase::Marked);
        assert_eq!(result.scope, RuntimeInvalidationScope::Workspace);
        assert_eq!(result.invalidated_now_count, 1);
        assert_eq!(result.deferred_count, 0);
        assert_eq!(
            result.affected_workspaces[0].mode,
            RuntimeInvalidationMode::Immediate
        );
        assert_eq!(
            result.user_message_key,
            Some(RuntimeInvalidationUserMessageKey::WorkspaceBindingSavedLaterMessages)
        );
        assert!(slot.runtime.lock().await.is_none());
        assert_eq!(*slot.status.lock().await, RuntimeSlotStatus::Idle);
        assert!(slot.pending_invalidation.lock().await.is_none());
    }

    #[tokio::test]
    async fn active_invalidation_defers_until_terminal_cleanup() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-running");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let slot =
            slot_with_status(&app_state, workspace.clone(), RuntimeSlotStatus::Running).await;
        let app = tauri::test::mock_app();

        let marked = invalidate_all_chat_runtimes(
            &app_state,
            app.handle(),
            RuntimeInvalidationReason::ProviderChanged,
        )
        .await;

        assert!(marked.ok);
        assert_eq!(marked.phase, RuntimeInvalidationPhase::Marked);
        assert_eq!(marked.scope, RuntimeInvalidationScope::Global);
        assert_eq!(marked.invalidated_now_count, 0);
        assert_eq!(marked.deferred_count, 1);
        assert_eq!(
            marked.affected_workspaces[0].mode,
            RuntimeInvalidationMode::Deferred
        );
        assert_eq!(
            marked.user_message_key,
            Some(RuntimeInvalidationUserMessageKey::ProviderSettingsSavedActiveTaskUsesPrevious)
        );
        assert!(slot.runtime.lock().await.is_some());
        assert_eq!(*slot.status.lock().await, RuntimeSlotStatus::Running);
        assert_eq!(
            slot.pending_invalidation
                .lock()
                .await
                .as_ref()
                .map(|mark| mark.invalidation_id.clone()),
            Some(marked.invalidation_id.clone())
        );

        let completed =
            complete_pending_invalidation_after_turn(&app_state, app.handle(), &slot).await;

        assert!(completed.ok);
        assert_eq!(completed.phase, RuntimeInvalidationPhase::Completed);
        assert_eq!(completed.invalidation_id, marked.invalidation_id);
        assert_eq!(
            completed.affected_workspaces[0].mode,
            RuntimeInvalidationMode::Completed
        );
        assert!(slot.runtime.lock().await.is_none());
        assert_eq!(*slot.status.lock().await, RuntimeSlotStatus::Idle);
        assert!(slot.pending_invalidation.lock().await.is_none());
    }

    #[tokio::test]
    async fn noop_invalidation_clears_stale_pending_mark() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("root"));
        let app_state = test_app_state(root);
        let workspace = tmp.path().join("workspace-stale-pending");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let slot = crate::commands::runtime::runtime_slot_for_path(&app_state, workspace.clone())
            .await
            .expect("slot");
        *slot.pending_invalidation.lock().await = Some(super::RuntimeInvalidationMark {
            reason: RuntimeInvalidationReason::EmployeeSkillsChanged,
            scope: RuntimeInvalidationScope::Employee,
            scope_identity: employee::GENERAL_EMPLOYEE_ID.to_string(),
            invalidation_id: "runtime_invalidation_stale".to_string(),
            created_at: "2026-06-15T00:00:00Z".to_string(),
            user_message_key: None,
            affected_workspaces: Vec::new(),
            message: None,
        });
        let app = tauri::test::mock_app();

        let result = invalidate_workspace_chat_runtime(
            &app_state,
            app.handle(),
            workspace,
            RuntimeInvalidationReason::WorkspaceCodexHomeContextChanged,
        )
        .await;

        assert!(result.ok);
        assert_eq!(result.invalidated_now_count, 0);
        assert_eq!(result.deferred_count, 0);
        assert_eq!(
            result.affected_workspaces[0].mode,
            RuntimeInvalidationMode::Noop
        );
        assert_eq!(*slot.status.lock().await, RuntimeSlotStatus::Idle);
        assert!(slot.pending_invalidation.lock().await.is_none());
    }

    #[tokio::test]
    async fn employee_invalidation_only_affects_bound_workspace_slots() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = Arc::new(root_workspace::init_or_open(tmp.path()).expect("root"));
        let app_state = test_app_state(Arc::clone(&root));
        let bound_path = tmp.path().join("workspace-bound");
        let unrelated_path = tmp.path().join("workspace-unrelated");
        let bound_ws = workspace::open_or_create(&bound_path).expect("bound workspace");
        let unrelated_ws = workspace::open_or_create(&unrelated_path).expect("unrelated workspace");
        workspace::add_known(&root.known_workspaces_path(), &bound_ws).expect("known bound");
        workspace::add_known(&root.known_workspaces_path(), &unrelated_ws)
            .expect("known unrelated");
        employee::bind_workspace(
            &root,
            employee::GENERAL_EMPLOYEE_ID,
            &bound_path,
            &bound_ws.id,
            &bound_ws.name,
        )
        .expect("bind workspace");
        let bound_slot =
            slot_with_status(&app_state, bound_path.clone(), RuntimeSlotStatus::Idle).await;
        let unrelated_slot =
            slot_with_status(&app_state, unrelated_path.clone(), RuntimeSlotStatus::Idle).await;
        let app = tauri::test::mock_app();

        let result = invalidate_employee_chat_runtimes(
            &app_state,
            app.handle(),
            employee::GENERAL_EMPLOYEE_ID,
            RuntimeInvalidationReason::EmployeePromptChanged,
        )
        .await;

        assert!(result.ok);
        assert_eq!(result.scope, RuntimeInvalidationScope::Employee);
        assert_eq!(result.scope_identity, employee::GENERAL_EMPLOYEE_ID);
        assert_eq!(result.invalidated_now_count, 1);
        assert_eq!(result.affected_workspaces.len(), 1);
        assert_eq!(result.affected_workspaces[0].workspace_id, bound_ws.id);
        assert!(bound_slot.runtime.lock().await.is_none());
        assert!(
            unrelated_slot.runtime.lock().await.is_some(),
            "unrelated workspace runtime must not be invalidated"
        );
    }
}
