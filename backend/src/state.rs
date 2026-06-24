use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::Mutex as AsyncMutex;

use crate::runtime::dream_session::DreamRuntimeClient;
use crate::runtime::lifecycle::RuntimeInvalidationMark;
use crate::runtime::CodexRuntime;
use crate::services::root_workspace::RootWorkspace;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeSlotStatus {
    Idle,
    Running,
    Pending,
    Cancelling,
    Error,
}

impl RuntimeSlotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Pending => "pending",
            Self::Cancelling => "cancelling",
            Self::Error => "error",
        }
    }
}

pub struct RuntimeSlot {
    pub workspace_key: String,
    pub workspace_path: PathBuf,
    pub runtime: AsyncMutex<Option<Arc<CodexRuntime>>>,
    pub status: AsyncMutex<RuntimeSlotStatus>,
    pub pending_invalidation: AsyncMutex<Option<RuntimeInvalidationMark>>,
    pub config_dirty: AsyncMutex<bool>,
    pub last_used_at: AsyncMutex<Instant>,
    pub idle_timeout: AsyncMutex<Duration>,
}

impl RuntimeSlot {
    pub fn new(workspace_key: String, workspace_path: PathBuf) -> Self {
        Self {
            workspace_key,
            workspace_path,
            runtime: AsyncMutex::new(None),
            status: AsyncMutex::new(RuntimeSlotStatus::Idle),
            pending_invalidation: AsyncMutex::new(None),
            config_dirty: AsyncMutex::new(false),
            last_used_at: AsyncMutex::new(Instant::now()),
            idle_timeout: AsyncMutex::new(Duration::from_secs(10 * 60)),
        }
    }
}

pub struct AppState {
    pub root: Arc<RootWorkspace>,
    pub active_workspace_path: Mutex<Option<PathBuf>>,
    pub active_session_id: Mutex<Option<String>>,
    pub known_workspaces_file: PathBuf,
    pub runtime_pool: AsyncMutex<HashMap<String, Arc<RuntimeSlot>>>,
    pub codex_status: AsyncMutex<String>,
    /// Set when an active turn is cancelled; checked by native SSE and Codex exec loops.
    pub turn_cancel: Arc<AtomicBool>,
    /// Serializes transcript JSONL appends so concurrent turns cannot interleave lines.
    pub transcript_write_lock: Mutex<()>,
    /// Serializes Employee registry/membership file writes to prevent concurrent race conditions.
    pub employee_write_lock: Mutex<()>,
    pub http_server_port: AtomicU16,
    /// One async mutex per workspace, lazily inserted. Held by background import
    /// tasks so that concurrent submissions for the same workspace serialize
    /// instead of competing for `wiki/` writes and the qmd index lock.
    pub import_queues: Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>,
    /// Independent runtime slot for Dream workflow execution (separate from chat runtime).
    pub dream_runtime: AsyncMutex<Option<Arc<DreamRuntimeClient>>>,
    pub dream_status: AsyncMutex<String>,
}

impl AppState {
    /// Serializes employee/dream file writes. Recovers from poisoned locks because the
    /// guarded value is unit-typed and carries no inconsistent state after a panic.
    pub fn lock_employee_write(&self) -> MutexGuard<'_, ()> {
        self.employee_write_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn lock_active_workspace(&self) -> MutexGuard<'_, Option<PathBuf>> {
        self.active_workspace_path
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn require_active_workspace(&self) -> Result<PathBuf, String> {
        self.lock_active_workspace()
            .clone()
            .ok_or_else(|| "未选择当前工作区".to_string())
    }

    pub fn lock_active_session_id(&self) -> MutexGuard<'_, Option<String>> {
        self.active_session_id
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn lock_transcript_write(&self) -> MutexGuard<'_, ()> {
        self.transcript_write_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn lock_import_queues(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<AsyncMutex<()>>>> {
        self.import_queues
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU16};
    use std::sync::Arc;

    fn test_app_state() -> AppState {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root =
            Arc::new(crate::services::root_workspace::init_or_open(tmp.path()).expect("init root"));
        AppState {
            root,
            active_workspace_path: Mutex::new(None),
            active_session_id: Mutex::new(None),
            known_workspaces_file: tmp.path().join("known.json"),
            runtime_pool: AsyncMutex::new(HashMap::new()),
            codex_status: AsyncMutex::new("idle".to_string()),
            turn_cancel: Arc::new(AtomicBool::new(false)),
            transcript_write_lock: Mutex::new(()),
            employee_write_lock: Mutex::new(()),
            http_server_port: AtomicU16::new(0),
            import_queues: Mutex::new(HashMap::new()),
            dream_runtime: AsyncMutex::new(None),
            dream_status: AsyncMutex::new("idle".to_string()),
        }
    }

    fn poison_mutex<T: Send + 'static>(mutex: &Mutex<T>) {
        let _ = std::panic::catch_unwind(|| {
            let _guard = mutex.lock().unwrap();
            panic!("intentional poison");
        });
    }

    #[test]
    fn lock_active_workspace_recovers_from_poison() {
        let state = test_app_state();
        *state.lock_active_workspace() = Some(PathBuf::from("/tmp/ws"));
        poison_mutex(&state.active_workspace_path);
        let guard = state.lock_active_workspace();
        assert_eq!(guard.as_deref(), Some(PathBuf::from("/tmp/ws").as_path()));
    }

    #[test]
    fn lock_active_session_id_recovers_from_poison() {
        let state = test_app_state();
        *state.lock_active_session_id() = Some("session-1".to_string());
        poison_mutex(&state.active_session_id);
        let guard = state.lock_active_session_id();
        assert_eq!(guard.as_deref(), Some("session-1"));
    }

    #[test]
    fn lock_employee_write_recovers_from_poison() {
        let state = test_app_state();
        poison_mutex(&state.employee_write_lock);
        let _guard = state.lock_employee_write();
    }
}
