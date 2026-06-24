use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;

use super::process::{
    apply_codex_child_env, binary_from_current_exe_dir, binary_from_repo_ancestors,
};
use super::process_policy::apply_no_visible_terminal_runtime_policy;
use crate::services::dream::{DreamResult, SourceSessionRef};

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

async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, message: &Value) -> Result<()> {
    let encoded = serde_json::to_string(message)?;
    writer.write_all(encoded.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<String>> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return Ok(Some(trimmed.to_string()));
    }
}

#[derive(Debug)]
enum RuntimeMessage {
    Response(Value),
    Notification { method: String, params: Value },
    Eof,
}

fn route_runtime_line(line: &str) -> Option<RuntimeMessage> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("id").is_some() {
        return Some(RuntimeMessage::Response(value));
    }
    let method = value.get("method").and_then(Value::as_str)?.to_string();
    Some(RuntimeMessage::Notification {
        method,
        params: value.get("params").cloned().unwrap_or(Value::Null),
    })
}

#[derive(Debug, Clone)]
pub struct DreamRuntimeConfig {
    pub run_workspace_path: PathBuf,
    pub codex_home: String,
    pub runtime_home: String,
    pub model: String,
    pub api_key: String,
    pub output_language: String,
}

const DREAM_REQUIRED_CAPABILITIES: [&str; 2] = ["dream.phase1", "dream.phase2"];
const DREAM_REQUIRED_PHASES: [&str; 2] = ["phase1", "phase2"];
const DREAM_CAPABILITY_VERSION: u32 = 1;
const DREAM_PROMPT_VERSION: &str = "dream-v1";

fn runtime_capability_available(result: &Value, capability: &str) -> bool {
    match result.get("capabilities") {
        Some(Value::Object(caps)) => caps
            .get(capability)
            .and_then(Value::as_str)
            .is_some_and(|mode| matches!(mode, "normalized" | "raw")),
        // Compatibility for pre-matrix initialize payloads. The current
        // chawork-runtime contract returns a capability mode map.
        Some(Value::Array(caps)) => caps.iter().any(|cap| cap.as_str() == Some(capability)),
        _ => false,
    }
}

fn validate_dream_initialize_result(result: &Value) -> Result<()> {
    let missing = DREAM_REQUIRED_CAPABILITIES
        .iter()
        .copied()
        .filter(|capability| !runtime_capability_available(result, capability))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "chawork-runtime 缺少 Dream capability: {}",
            missing.join(", ")
        );
    }
    let dream = result
        .get("dream")
        .context("chawork-runtime 缺少 dream metadata")?;
    let capability_version = dream
        .get("capabilityVersion")
        .and_then(Value::as_u64)
        .context("chawork-runtime dream metadata 缺少 capabilityVersion")?;
    if capability_version != u64::from(DREAM_CAPABILITY_VERSION) {
        bail!(
            "chawork-runtime dream capabilityVersion 不匹配: expected {}, got {}",
            DREAM_CAPABILITY_VERSION,
            capability_version
        );
    }
    let prompt_version = dream
        .get("promptVersion")
        .and_then(Value::as_str)
        .context("chawork-runtime dream metadata 缺少 promptVersion")?;
    if prompt_version != DREAM_PROMPT_VERSION {
        bail!(
            "chawork-runtime dream promptVersion 不匹配: expected {}, got {}",
            DREAM_PROMPT_VERSION,
            prompt_version
        );
    }
    let supported_phases = dream
        .get("supportedPhases")
        .and_then(Value::as_array)
        .context("chawork-runtime dream metadata 缺少 supportedPhases")?;
    let missing_phases = DREAM_REQUIRED_PHASES
        .iter()
        .copied()
        .filter(|phase| {
            !supported_phases
                .iter()
                .any(|supported| supported.as_str() == Some(*phase))
        })
        .collect::<Vec<_>>();
    if !missing_phases.is_empty() {
        bail!(
            "chawork-runtime dream supportedPhases 缺少: {}",
            missing_phases.join(", ")
        );
    }
    Ok(())
}

fn validate_dream_phase_metadata(
    expected_phase: &str,
    actual_phase: &str,
    capability_version: u32,
    prompt_version: &str,
) -> Result<()> {
    if actual_phase != expected_phase {
        bail!("Dream runtime phase 不匹配: expected {expected_phase}, got {actual_phase}");
    }
    if capability_version != DREAM_CAPABILITY_VERSION {
        bail!(
            "Dream runtime capabilityVersion 不匹配: expected {}, got {}",
            DREAM_CAPABILITY_VERSION,
            capability_version
        );
    }
    if prompt_version != DREAM_PROMPT_VERSION {
        bail!(
            "Dream runtime promptVersion 不匹配: expected {}, got {}",
            DREAM_PROMPT_VERSION,
            prompt_version
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DreamPhase1RuntimeResult {
    pub phase: String,
    pub dream_run_id: String,
    pub target_employee_id: String,
    pub decision: String,
    pub result: DreamResult,
    pub runtime_thread_id: Option<String>,
    pub capability_version: u32,
    pub prompt_version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DreamPhase2RuntimeResult {
    pub phase: String,
    pub dream_run_id: String,
    pub target_employee_id: String,
    pub approved_request_id: String,
    pub target_prompt_path: String,
    pub prompt_candidate: String,
    pub source_sessions: Vec<SourceSessionRef>,
    pub runtime_thread_id: Option<String>,
    pub capability_version: u32,
    pub prompt_version: String,
}

pub struct DreamRuntimeClient {
    config: DreamRuntimeConfig,
    child: AsyncMutex<Option<Child>>,
    cancel_requested: AtomicBool,
}

impl DreamRuntimeClient {
    pub fn new(config: DreamRuntimeConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            child: AsyncMutex::new(None),
            cancel_requested: AtomicBool::new(false),
        })
    }

    pub async fn cancel(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
        }
    }

    pub async fn phase1(
        &self,
        app: &AppHandle,
        target_employee_id: &str,
        dream_run_id: &str,
    ) -> Result<DreamPhase1RuntimeResult> {
        let mut conn = self.spawn_connection().await?;
        self.initialize(&mut conn).await?;
        let result = self
            .send_wait_forwarding(
                &mut conn,
                app,
                "dream/phase1/start",
                Some(json!({
                    "runWorkspacePath": self.config.run_workspace_path,
                    "codexHome": self.config.codex_home,
                    "targetEmployeeId": target_employee_id,
                    "dreamRunId": dream_run_id,
                    "model": self.config.model,
                    "outputLanguage": self.config.output_language,
                })),
            )
            .await?;
        self.shutdown(conn).await;
        let result_value = result
            .get("result")
            .cloned()
            .context("dream phase1 response missing result")?;
        let runtime_result: DreamPhase1RuntimeResult = serde_json::from_value(result_value)?;
        validate_dream_phase_metadata(
            "phase1",
            &runtime_result.phase,
            runtime_result.capability_version,
            &runtime_result.prompt_version,
        )?;
        if runtime_result.target_employee_id != target_employee_id
            || runtime_result.dream_run_id != dream_run_id
            || runtime_result.result.target_employee_id != target_employee_id
            || runtime_result.result.dream_run_id != dream_run_id
        {
            bail!("Dream Phase 1 返回的 target metadata 不匹配");
        }
        Ok(runtime_result)
    }

    pub async fn phase2(
        &self,
        app: &AppHandle,
        approved_request_id: &str,
        approved_update: &DreamResult,
        target_prompt_path: &str,
    ) -> Result<DreamPhase2RuntimeResult> {
        let mut conn = self.spawn_connection().await?;
        self.initialize(&mut conn).await?;
        let result = self
            .send_wait_forwarding(
                &mut conn,
                app,
                "dream/phase2/start",
                Some(json!({
                    "runWorkspacePath": self.config.run_workspace_path,
                    "codexHome": self.config.codex_home,
                    "targetEmployeeId": approved_update.target_employee_id,
                    "dreamRunId": approved_update.dream_run_id,
                    "approvedRequestId": approved_request_id,
                    "approvedUpdate": approved_update,
                    "targetPromptPath": target_prompt_path,
                    "model": self.config.model,
                    "outputLanguage": self.config.output_language,
                })),
            )
            .await?;
        self.shutdown(conn).await;
        let result_value = result
            .get("result")
            .cloned()
            .context("dream phase2 response missing result")?;
        let runtime_result: DreamPhase2RuntimeResult = serde_json::from_value(result_value)?;
        validate_dream_phase_metadata(
            "phase2",
            &runtime_result.phase,
            runtime_result.capability_version,
            &runtime_result.prompt_version,
        )?;
        if runtime_result.approved_request_id != approved_request_id
            || runtime_result.target_employee_id != approved_update.target_employee_id
            || runtime_result.dream_run_id != approved_update.dream_run_id
            || runtime_result.target_prompt_path != target_prompt_path
        {
            bail!("Dream Phase 2 返回的 target metadata 不匹配");
        }
        Ok(runtime_result)
    }

    async fn spawn_connection(&self) -> Result<RuntimeConnection> {
        let binary = default_chawork_runtime_cli();
        if binary.is_empty() {
            bail!(
                "未找到 chawork-runtime 运行时二进制。请构建：cd chawork-runtime/codex-rs && cargo build -p chawork-runtime --bin chawork-runtime"
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
            &self.config.run_workspace_path.to_string_lossy(),
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
                            eprintln!("[dream chawork-runtime stderr] {t}");
                        }
                    }
                }
            }
        });

        let stdin = child.stdin.take().context("chawork-runtime stdin pipe")?;
        let stdout = child.stdout.take().context("chawork-runtime stdout pipe")?;
        let (tx, rx) = mpsc::channel(128);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_line(&mut reader).await {
                    Ok(Some(line)) => {
                        if let Some(message) = route_runtime_line(&line) {
                            if tx.send(message).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) | Err(_) => {
                        let _ = tx.send(RuntimeMessage::Eof).await;
                        break;
                    }
                }
            }
        });

        let mut guard = self.child.lock().await;
        *guard = Some(child);
        Ok(RuntimeConnection {
            stdin,
            events_rx: rx,
            next_request_id: 0,
        })
    }

    async fn initialize(&self, conn: &mut RuntimeConnection) -> Result<()> {
        let response = conn
            .send_rpc_wait(
                "runtime/initialize",
                Some(json!({
                    "contractVersion": 1,
                    "client": { "name": "chawork", "version": env!("CARGO_PKG_VERSION") },
                    "workspacePath": self.config.run_workspace_path,
                    "requiredCapabilities": DREAM_REQUIRED_CAPABILITIES,
                })),
            )
            .await?;
        let result = response
            .get("result")
            .context("runtime initialize missing result")?;
        validate_dream_initialize_result(result)
    }

    async fn send_wait_forwarding(
        &self,
        conn: &mut RuntimeConnection,
        app: &AppHandle,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value> {
        let id = conn.send_rpc(method, params).await?;
        while let Some(message) = conn.events_rx.recv().await {
            if self.cancel_requested.load(Ordering::SeqCst) {
                bail!("Dream run cancelled");
            }
            match message {
                RuntimeMessage::Response(value) => {
                    if value.get("id").and_then(Value::as_i64) == Some(id) {
                        if let Some(err) = value.get("error") {
                            bail!("runtime error for {method}: {err}");
                        }
                        return Ok(value);
                    }
                }
                RuntimeMessage::Notification { method, params } => {
                    if method.starts_with("dream/") {
                        let _ = app.emit(
                            "dream-event",
                            json!({
                                "method": method,
                                "params": params,
                            }),
                        );
                    }
                }
                RuntimeMessage::Eof => break,
            }
        }
        bail!("runtime closed before responding to {method}")
    }

    async fn shutdown(&self, mut conn: RuntimeConnection) {
        let _ = conn.send_rpc("runtime/shutdown", None).await;
        drop(conn.stdin);
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.wait().await;
        }
    }
}

struct RuntimeConnection {
    stdin: ChildStdin,
    events_rx: mpsc::Receiver<RuntimeMessage>,
    next_request_id: i64,
}

impl RuntimeConnection {
    fn next_request_id(&mut self) -> i64 {
        self.next_request_id += 1;
        self.next_request_id
    }

    async fn send_rpc(&mut self, method: &str, params: Option<Value>) -> Result<i64> {
        let id = self.next_request_id();
        let mut message = json!({ "id": id, "method": method });
        if let Some(params) = params {
            message["params"] = params;
        }
        write_message(&mut self.stdin, &message).await?;
        Ok(id)
    }

    async fn send_rpc_wait(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.send_rpc(method, params).await?;
        while let Some(message) = self.events_rx.recv().await {
            match message {
                RuntimeMessage::Response(value) => {
                    if value.get("id").and_then(Value::as_i64) == Some(id) {
                        if let Some(err) = value.get("error") {
                            bail!("runtime error for {method}: {err}");
                        }
                        return Ok(value);
                    }
                }
                RuntimeMessage::Notification { .. } => {}
                RuntimeMessage::Eof => break,
            }
        }
        bail!("runtime closed before responding to {method}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_initialize_result() -> Value {
        json!({
            "capabilities": {
                "dream.phase1": "normalized",
                "dream.phase2": "normalized"
            },
            "dream": {
                "capabilityVersion": 1,
                "promptVersion": "dream-v1",
                "supportedPhases": ["phase1", "phase2"]
            }
        })
    }

    #[test]
    fn dream_initialize_accepts_runtime_capability_map() {
        let result = valid_initialize_result();

        validate_dream_initialize_result(&result).expect("valid dream capability metadata");
    }

    #[test]
    fn dream_initialize_rejects_legacy_single_dream_capability() {
        let result = json!({
            "capabilities": {
                "dream": "normalized"
            },
            "dream": {
                "capabilityVersion": 1,
                "promptVersion": "dream-v1",
                "supportedPhases": ["phase1", "phase2"]
            }
        });

        let err =
            validate_dream_initialize_result(&result).expect_err("legacy key is insufficient");
        assert!(err.to_string().contains("dream.phase1"));
        assert!(err.to_string().contains("dream.phase2"));
    }

    #[test]
    fn dream_initialize_rejects_metadata_version_mismatch() {
        let mut result = valid_initialize_result();
        result["dream"]["capabilityVersion"] = json!(2);

        let err = validate_dream_initialize_result(&result)
            .expect_err("capability version mismatch must be rejected");

        assert!(err.to_string().contains("capabilityVersion"));
    }

    #[test]
    fn dream_initialize_rejects_prompt_version_mismatch() {
        let mut result = valid_initialize_result();
        result["dream"]["promptVersion"] = json!("dream-v2");

        let err = validate_dream_initialize_result(&result)
            .expect_err("prompt version mismatch must be rejected");

        assert!(err.to_string().contains("promptVersion"));
    }

    #[test]
    fn dream_initialize_rejects_missing_supported_phase() {
        let mut result = valid_initialize_result();
        result["dream"]["supportedPhases"] = json!(["phase1"]);

        let err = validate_dream_initialize_result(&result)
            .expect_err("missing phase2 support must be rejected");

        assert!(err.to_string().contains("supportedPhases"));
        assert!(err.to_string().contains("phase2"));
    }

    #[test]
    fn dream_phase_response_rejects_phase_and_version_mismatch() {
        validate_dream_phase_metadata("phase1", "phase1", 1, "dream-v1")
            .expect("matching phase metadata is accepted");

        let phase_err = validate_dream_phase_metadata("phase1", "phase2", 1, "dream-v1")
            .expect_err("phase mismatch must be rejected");
        assert!(phase_err.to_string().contains("phase"));

        let capability_err = validate_dream_phase_metadata("phase1", "phase1", 2, "dream-v1")
            .expect_err("capability version mismatch must be rejected");
        assert!(capability_err.to_string().contains("capabilityVersion"));

        let prompt_err = validate_dream_phase_metadata("phase1", "phase1", 1, "dream-v2")
            .expect_err("prompt version mismatch must be rejected");
        assert!(prompt_err.to_string().contains("promptVersion"));
    }
}
