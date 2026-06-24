use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;

/// Candidate vendored binaries next to the ChaWork repo
/// (`chawork-runtime/codex-rs/target/{release,debug}/<name>`).
pub(crate) fn binary_from_repo_ancestors(start: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..12 {
        let base = dir.join("chawork-runtime").join("codex-rs").join("target");
        for profile in ["release", "debug"] {
            let candidate = base.join(profile).join(name);
            if candidate.is_file()
                && std::fs::metadata(&candidate)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false)
            {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Candidate binaries installed next to the Tauri executable.
pub(crate) fn binary_from_current_exe_dir(exe: &Path, name: &str) -> Option<PathBuf> {
    let parent = exe.parent()?;
    for candidate in [
        parent.join(name),
        parent.join("resources").join(name),
        parent.join("Resource").join(name),
    ] {
        if candidate.is_file()
            && std::fs::metadata(&candidate)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn apply_codex_child_env(
    cmd: &mut Command,
    codex_home: &str,
    runtime_home: &str,
    workspace_path: &str,
    api_key: &str,
) {
    cmd.env_clear();
    const KEYS: &[&str] = &[
        "PATH",
        "USER",
        "LOGNAME",
        "SHELL",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "LC_MESSAGES",
        "USERNAME",
        "TMP",
        "TEMP",
        "SystemRoot",
    ];
    for k in KEYS {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                cmd.env(k, v);
            }
        }
    }
    cmd.env("CODEX_HOME", codex_home);
    cmd.env("HOME", runtime_home);
    #[cfg(windows)]
    {
        let runtime_home_path = Path::new(runtime_home);
        cmd.env("USERPROFILE", runtime_home);
        if let Some(std::path::Component::Prefix(prefix)) = runtime_home_path.components().next() {
            let drive = prefix.as_os_str().to_string_lossy().to_string();
            cmd.env("HOMEDRIVE", &drive);
            let home_path = runtime_home
                .strip_prefix(&drive)
                .filter(|value| !value.is_empty())
                .unwrap_or("\\");
            cmd.env("HOMEPATH", home_path);
        }
    }
    cmd.env("CHAWORK_WORKSPACE", workspace_path);
    // Propagate process mode so chawork-runtime / Codex apply the same
    // no-visible-terminal policy on their own child processes.
    cmd.env(
        super::process_policy::CHAWORK_RUNTIME_PROCESS_MODE_ENV,
        super::process_policy::PROCESS_MODE_NO_VISIBLE_TERMINAL,
    );

    for key in ["RUST_LOG", "RUST_BACKTRACE"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                cmd.env(key, v);
            }
        }
    }

    let trimmed = api_key.trim();
    if !trimmed.is_empty() {
        cmd.env("OPENAI_API_KEY", trimmed);
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeMetadata {
    #[serde(rename = "releaseUnitId")]
    pub release_unit_id: Option<String>,
    #[serde(rename = "runtimeVersion")]
    pub runtime_version: Option<String>,
    #[serde(rename = "codexVersion")]
    pub codex_version: Option<String>,
    #[serde(rename = "capabilityMatrixVersion")]
    pub capability_matrix_version: Option<String>,
    #[serde(default)]
    pub capabilities: Value,
    #[serde(rename = "unsupportedCapabilities", default)]
    pub unsupported_capabilities: Vec<String>,
    #[serde(default)]
    pub dream: Option<Value>,
}

impl RuntimeMetadata {
    pub fn from_initialize_result(result: &Value) -> Self {
        Self {
            release_unit_id: result
                .get("releaseUnitId")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            runtime_version: result
                .get("runtimeVersion")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            codex_version: result
                .get("codexVersion")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            capability_matrix_version: result
                .get("capabilityMatrixVersion")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            capabilities: result
                .get("capabilities")
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default())),
            unsupported_capabilities: result
                .get("unsupportedCapabilities")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            dream: result.get("dream").cloned(),
        }
    }
}

pub struct RuntimeConfig {
    pub workspace_path: String,
    pub codex_home: String,
    pub runtime_home: String,
    /// Effective provider model. Empty means the runtime should use Codex config defaults.
    pub model: String,
    /// Effective provider API key. Empty means no API key env is injected.
    pub api_key: String,
    /// Explicit runtime workspace roots for ordinary Chat threads.
    pub runtime_workspace_roots: Vec<String>,
    /// Runtime contract approval policy for ordinary Chat threads.
    pub approval_policy: String,
    /// Runtime contract sandbox mode for ordinary Chat threads.
    pub sandbox: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeLocalImage {
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeTurnInput {
    pub text: String,
    pub local_images: Vec<RuntimeLocalImage>,
}

impl RuntimeTurnInput {
    pub(crate) fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.local_images.is_empty()
    }

    pub(crate) fn has_image_input(&self) -> bool {
        !self.local_images.is_empty()
    }
}

/// Runtime thread persistence owner for a workspace session.
#[derive(Clone)]
pub struct ThreadPersistCtx {
    pub workspace: PathBuf,
    pub workspace_id: String,
    pub session_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRequestOwner {
    pub workspace_id: String,
    pub session_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub request_id: String,
}

impl PendingRequestOwner {
    pub(crate) fn is_complete(&self) -> bool {
        !self.workspace_id.trim().is_empty()
            && !self.session_id.trim().is_empty()
            && !self.thread_id.trim().is_empty()
            && !self.turn_id.trim().is_empty()
            && !self.request_id.trim().is_empty()
    }
}

pub struct CodexRuntime {
    pub(crate) config: RuntimeConfig,
    pub(crate) thread_id: Arc<AsyncMutex<Option<String>>>,
    /// Persistent stdio JSON-RPC connection for interactive ChaWork runtime turns.
    pub(crate) session_connection: Arc<AsyncMutex<Option<super::session::RuntimeConnection>>>,
    pub(crate) runtime_metadata: Arc<AsyncMutex<Option<RuntimeMetadata>>>,
    pub(crate) cancel_requested: Arc<AtomicBool>,
    pub(crate) approval_tx: mpsc::Sender<(String, String)>,
    pub(crate) approval_rx: Arc<AsyncMutex<mpsc::Receiver<(String, String)>>>,
    pub(crate) user_input_tx: mpsc::Sender<(String, serde_json::Value)>,
    pub(crate) user_input_rx: Arc<AsyncMutex<mpsc::Receiver<(String, serde_json::Value)>>>,
    pub(crate) elicitation_tx: mpsc::Sender<McpElicitationResponse>,
    pub(crate) elicitation_rx: Arc<AsyncMutex<mpsc::Receiver<McpElicitationResponse>>>,
    pub(crate) permissions_tx: mpsc::Sender<PermissionsResponse>,
    pub(crate) permissions_rx: Arc<AsyncMutex<mpsc::Receiver<PermissionsResponse>>>,
    pub(crate) pending_request_owners: Arc<AsyncMutex<HashMap<String, PendingRequestOwner>>>,
}

#[derive(Clone, Debug)]
pub struct McpElicitationResponse {
    pub request_id: String,
    pub action: String,
    pub content: Option<serde_json::Value>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct PermissionsResponse {
    pub request_id: String,
    pub granted: bool,
    pub permissions: serde_json::Value,
    pub scope: serde_json::Value,
    pub strict_auto_review: Option<bool>,
}

impl Clone for CodexRuntime {
    fn clone(&self) -> Self {
        Self {
            config: RuntimeConfig {
                workspace_path: self.config.workspace_path.clone(),
                codex_home: self.config.codex_home.clone(),
                runtime_home: self.config.runtime_home.clone(),
                model: self.config.model.clone(),
                api_key: self.config.api_key.clone(),
                runtime_workspace_roots: self.config.runtime_workspace_roots.clone(),
                approval_policy: self.config.approval_policy.clone(),
                sandbox: self.config.sandbox.clone(),
            },
            thread_id: Arc::clone(&self.thread_id),
            session_connection: Arc::clone(&self.session_connection),
            runtime_metadata: Arc::clone(&self.runtime_metadata),
            cancel_requested: Arc::clone(&self.cancel_requested),
            approval_tx: self.approval_tx.clone(),
            approval_rx: Arc::clone(&self.approval_rx),
            user_input_tx: self.user_input_tx.clone(),
            user_input_rx: Arc::clone(&self.user_input_rx),
            elicitation_tx: self.elicitation_tx.clone(),
            elicitation_rx: Arc::clone(&self.elicitation_rx),
            permissions_tx: self.permissions_tx.clone(),
            permissions_rx: Arc::clone(&self.permissions_rx),
            pending_request_owners: Arc::clone(&self.pending_request_owners),
        }
    }
}

impl CodexRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        let (approval_tx, approval_rx) = mpsc::channel(8);
        let (user_input_tx, user_input_rx) = mpsc::channel(8);
        let (elicitation_tx, elicitation_rx) = mpsc::channel(8);
        let (permissions_tx, permissions_rx) = mpsc::channel(8);
        Self {
            config,
            thread_id: Arc::new(AsyncMutex::new(None)),
            session_connection: Arc::new(AsyncMutex::new(None)),
            runtime_metadata: Arc::new(AsyncMutex::new(None)),
            cancel_requested: Arc::new(AtomicBool::new(false)),
            approval_tx,
            approval_rx: Arc::new(AsyncMutex::new(approval_rx)),
            user_input_tx,
            user_input_rx: Arc::new(AsyncMutex::new(user_input_rx)),
            elicitation_tx,
            elicitation_rx: Arc::new(AsyncMutex::new(elicitation_rx)),
            permissions_tx,
            permissions_rx: Arc::new(AsyncMutex::new(permissions_rx)),
            pending_request_owners: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    pub async fn register_pending_request(&self, owner: PendingRequestOwner) {
        if !owner.is_complete() {
            return;
        }
        self.pending_request_owners
            .lock()
            .await
            .insert(owner.request_id.clone(), owner);
    }

    pub async fn clear_pending_request(&self, request_id: &str) {
        if request_id.trim().is_empty() {
            return;
        }
        self.pending_request_owners.lock().await.remove(request_id);
    }

    pub async fn clear_pending_requests_for_session(&self, session_id: &str) {
        if session_id.trim().is_empty() {
            return;
        }
        self.pending_request_owners
            .lock()
            .await
            .retain(|_, owner| owner.session_id != session_id);
    }

    pub async fn clear_all_pending_requests(&self) {
        self.pending_request_owners.lock().await.clear();
    }

    pub async fn set_runtime_metadata(&self, metadata: RuntimeMetadata) {
        *self.runtime_metadata.lock().await = Some(metadata);
    }

    pub async fn runtime_metadata(&self) -> Option<RuntimeMetadata> {
        self.runtime_metadata.lock().await.clone()
    }

    pub async fn validate_pending_request_owner(
        &self,
        request_id: &str,
        workspace_id: &str,
        session_id: Option<&str>,
    ) -> Result<PendingRequestOwner> {
        let session_id = session_id.unwrap_or_default();
        if request_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || session_id.trim().is_empty()
        {
            anyhow::bail!("请求 owner 不完整");
        }
        let owner = {
            let pending = self.pending_request_owners.lock().await;
            pending.get(request_id).cloned()
        };
        let Some(owner) = owner else {
            anyhow::bail!("请求不存在或已结束");
        };
        if !owner.is_complete() || owner.request_id != request_id {
            anyhow::bail!("请求 owner 不完整");
        }
        if owner.workspace_id != workspace_id {
            anyhow::bail!("请求不属于该工作区");
        }
        if owner.session_id != session_id {
            anyhow::bail!("请求不属于该会话");
        }
        let current_thread_id = self.thread_id().await.unwrap_or_default();
        if current_thread_id.trim().is_empty() {
            anyhow::bail!("当前 runtime thread owner 不完整");
        }
        if current_thread_id != owner.thread_id {
            anyhow::bail!("请求不属于当前 runtime thread");
        }
        Ok(owner)
    }

    pub async fn send_approval_decision(&self, id: String, decision: String) -> Result<()> {
        self.approval_tx
            .send((id, decision))
            .await
            .map_err(|e| anyhow::anyhow!("approval channel closed: {e}"))
    }

    pub async fn recv_approval_decision(&self) -> Option<(String, String)> {
        self.approval_rx.lock().await.recv().await
    }

    pub async fn send_user_input_answers(
        &self,
        id: String,
        answers: serde_json::Value,
    ) -> Result<()> {
        self.user_input_tx
            .send((id, answers))
            .await
            .map_err(|e| anyhow::anyhow!("user-input channel closed: {e}"))
    }

    pub async fn recv_user_input_answers(&self) -> Option<(String, serde_json::Value)> {
        self.user_input_rx.lock().await.recv().await
    }

    pub async fn send_mcp_elicitation_response(
        &self,
        response: McpElicitationResponse,
    ) -> Result<()> {
        self.elicitation_tx
            .send(response)
            .await
            .map_err(|e| anyhow::anyhow!("elicitation channel closed: {e}"))
    }

    pub async fn recv_mcp_elicitation_response(&self) -> Option<McpElicitationResponse> {
        self.elicitation_rx.lock().await.recv().await
    }

    pub async fn send_permissions_response(&self, response: PermissionsResponse) -> Result<()> {
        self.permissions_tx
            .send(response)
            .await
            .map_err(|e| anyhow::anyhow!("permissions channel closed: {e}"))
    }

    pub async fn recv_permissions_response(&self) -> Option<PermissionsResponse> {
        self.permissions_rx.lock().await.recv().await
    }

    pub async fn replace_thread_id(&self, id: Option<String>) {
        *self.thread_id.lock().await = id;
    }

    pub async fn thread_id(&self) -> Option<String> {
        self.thread_id.lock().await.clone()
    }

    /// Signal cancellation for the in-flight interactive turn.
    pub async fn cancel_current_turn(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_runtime() -> CodexRuntime {
        CodexRuntime::new(RuntimeConfig {
            workspace_path: "/tmp/chawork-test".to_string(),
            codex_home: "/tmp/chawork-test-codex-home".to_string(),
            runtime_home: "/tmp/chawork-test-runtime-home".to_string(),
            model: String::new(),
            api_key: String::new(),
            runtime_workspace_roots: vec!["/tmp/chawork-test".to_string()],
            approval_policy: "on-request".to_string(),
            sandbox: "workspace-write".to_string(),
        })
    }

    fn test_owner(request_id: &str) -> PendingRequestOwner {
        PendingRequestOwner {
            workspace_id: "workspace_1".to_string(),
            session_id: "session_1".to_string(),
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            request_id: request_id.to_string(),
        }
    }

    #[tokio::test]
    async fn pending_request_validation_rejects_missing_session_owner() {
        let runtime = test_runtime();
        runtime
            .replace_thread_id(Some("thread_1".to_string()))
            .await;
        runtime
            .register_pending_request(test_owner("approval_1"))
            .await;

        let err = runtime
            .validate_pending_request_owner("approval_1", "workspace_1", None)
            .await
            .expect_err("missing session owner must be rejected");

        assert!(err.to_string().contains("owner"));
    }

    #[tokio::test]
    async fn pending_request_validation_rejects_empty_or_wrong_session_owner() {
        let runtime = test_runtime();
        runtime
            .replace_thread_id(Some("thread_1".to_string()))
            .await;
        runtime
            .register_pending_request(test_owner("approval_1"))
            .await;

        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some(""))
            .await
            .expect_err("empty session owner must be rejected");
        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some("session_2"))
            .await
            .expect_err("wrong session owner must be rejected");
        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some("session_1"))
            .await
            .expect("matching session owner is accepted");
    }

    #[tokio::test]
    async fn pending_request_validation_rejects_workspace_or_thread_mismatch() {
        let runtime = test_runtime();
        runtime
            .replace_thread_id(Some("thread_1".to_string()))
            .await;
        runtime
            .register_pending_request(test_owner("approval_1"))
            .await;

        runtime
            .validate_pending_request_owner("approval_1", "workspace_2", Some("session_1"))
            .await
            .expect_err("wrong workspace owner must be rejected");

        runtime
            .replace_thread_id(Some("thread_2".to_string()))
            .await;
        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some("session_1"))
            .await
            .expect_err("stale thread owner must be rejected");
    }

    #[tokio::test]
    async fn pending_request_validation_rejects_missing_current_thread_owner() {
        let runtime = test_runtime();
        runtime
            .register_pending_request(test_owner("approval_1"))
            .await;

        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some("session_1"))
            .await
            .expect_err("missing current runtime thread must be rejected");
    }

    #[tokio::test]
    async fn incomplete_pending_request_owner_is_not_registered() {
        let runtime = test_runtime();
        runtime
            .replace_thread_id(Some("thread_1".to_string()))
            .await;
        let mut owner = test_owner("approval_1");
        owner.turn_id.clear();

        runtime.register_pending_request(owner).await;

        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some("session_1"))
            .await
            .expect_err("incomplete owner must not be registered");
    }

    #[tokio::test]
    async fn pending_requests_are_cleared_on_runtime_shutdown_cleanup() {
        let runtime = test_runtime();
        runtime
            .replace_thread_id(Some("thread_1".to_string()))
            .await;
        runtime
            .register_pending_request(test_owner("approval_1"))
            .await;
        runtime
            .register_pending_request(test_owner("input_1"))
            .await;

        runtime.clear_all_pending_requests().await;

        runtime
            .validate_pending_request_owner("approval_1", "workspace_1", Some("session_1"))
            .await
            .expect_err("approval request must be gone after cleanup");
        runtime
            .validate_pending_request_owner("input_1", "workspace_1", Some("session_1"))
            .await
            .expect_err("input request must be gone after cleanup");
    }

    #[tokio::test]
    async fn codex_child_env_injects_only_openai_api_key() {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(r#"printf 'OPENAI=%s\nCODEX=%s\n' "$OPENAI_API_KEY" "$CODEX_API_KEY""#);
        apply_codex_child_env(
            &mut cmd,
            "/tmp/codex-home",
            "/tmp/runtime-home",
            "/tmp/workspace",
            "secret-key",
        );

        let output = cmd.output().await.expect("child env output");
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");

        assert!(stdout.contains("OPENAI=secret-key"));
        assert!(stdout.contains("CODEX=\n"));
    }

    #[tokio::test]
    async fn codex_child_env_uses_fake_home() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let codex_home = tmp.path().join("codex-home");
        let runtime_home = tmp.path().join("runtime-home");
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&runtime_home).expect("runtime home");
        std::fs::create_dir_all(&workspace).expect("workspace");

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(r#"printf 'HOME=%s\nUSERPROFILE=%s\nCODEX_HOME=%s\nCHAWORK_WORKSPACE=%s\n' "$HOME" "$USERPROFILE" "$CODEX_HOME" "$CHAWORK_WORKSPACE""#);
        apply_codex_child_env(
            &mut cmd,
            &codex_home.to_string_lossy(),
            &runtime_home.to_string_lossy(),
            &workspace.to_string_lossy(),
            "",
        );

        let output = cmd.output().await.expect("child env output");
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");

        assert!(stdout.contains(&format!("HOME={}\n", runtime_home.display())));
        assert!(stdout.contains(&format!("CODEX_HOME={}\n", codex_home.display())));
        assert!(stdout.contains(&format!("CHAWORK_WORKSPACE={}\n", workspace.display())));
        if cfg!(windows) {
            assert!(stdout.contains(&format!("USERPROFILE={}\n", runtime_home.display())));
        }
    }

    #[test]
    fn binary_from_current_exe_dir_finds_installed_sidecar() {
        let tmp = tempfile::tempdir().expect("tmp");
        let app_exe = tmp.path().join(if cfg!(windows) {
            "chawork.exe"
        } else {
            "chawork"
        });
        let sidecar = tmp.path().join(if cfg!(windows) {
            "chawork-runtime.exe"
        } else {
            "chawork-runtime"
        });
        std::fs::write(&app_exe, b"app").expect("app exe");
        std::fs::write(&sidecar, b"sidecar").expect("sidecar");

        let found =
            binary_from_current_exe_dir(&app_exe, sidecar.file_name().unwrap().to_str().unwrap());

        assert_eq!(found.as_deref(), Some(sidecar.as_path()));
    }
}
