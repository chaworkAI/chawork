//! Dream run 输入准备服务。
//!
//! 为 Dream workflow 准备输入：发现最近的 sessions、快照 employee prompt
//! 和 transcript，写入独立 run workspace 供后续 runtime 消费。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::employee::{self, EmployeeKind, DREAM_EMPLOYEE_ID};
use super::root_workspace::RootWorkspace;
use super::session;

// ── Dream Config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// `"manual"` | `"daily"`
    #[serde(rename = "type")]
    pub schedule_type: String,
    /// Employee-specific trigger time ("HH:MM"). `None` = use global default.
    #[serde(default)]
    pub time: Option<String>,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            schedule_type: "daily".to_string(),
            time: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionScanConfig {
    /// `"all"` | `"selected"`
    pub scope: String,
    /// Workspace IDs to scan when scope is `"selected"`
    #[serde(default)]
    pub workspace_subset: Vec<String>,
    /// Max sessions to scan per dream run
    #[serde(default = "default_latest_sessions")]
    pub latest_sessions: usize,
}

fn default_latest_sessions() -> usize {
    3
}

impl Default for SessionScanConfig {
    fn default() -> Self {
        Self {
            scope: "all".to_string(),
            workspace_subset: Vec::new(),
            latest_sessions: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamConfig {
    pub enabled: bool,
    #[serde(default)]
    pub schedule: ScheduleConfig,
    #[serde(default)]
    pub session_scan: SessionScanConfig,
}

impl Default for DreamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: ScheduleConfig::default(),
            session_scan: SessionScanConfig::default(),
        }
    }
}

// ── Dream Defaults (global) ──────────────────────────────────────────────

const DEFAULT_DREAM_TIME: &str = "09:00";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamDefaults {
    #[serde(default = "default_dream_time_value")]
    pub default_dream_time: String,
}

fn default_dream_time_value() -> String {
    DEFAULT_DREAM_TIME.to_string()
}

impl Default for DreamDefaults {
    fn default() -> Self {
        Self {
            default_dream_time: DEFAULT_DREAM_TIME.to_string(),
        }
    }
}

fn normalize_dream_defaults(defaults: DreamDefaults) -> DreamDefaults {
    let default_dream_time = if defaults.default_dream_time.trim().is_empty() {
        DEFAULT_DREAM_TIME.to_string()
    } else {
        defaults.default_dream_time
    };

    DreamDefaults { default_dream_time }
}

fn dream_defaults_path(root: &RootWorkspace) -> PathBuf {
    root.config_dir().join("dream-defaults.yaml")
}

pub fn read_dream_defaults(root: &RootWorkspace) -> DreamDefaults {
    let path = dream_defaults_path(root);
    if !path.is_file() {
        return DreamDefaults::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_yaml::from_str(&raw).ok())
        .map(normalize_dream_defaults)
        .unwrap_or_default()
}

pub fn write_dream_defaults(root: &RootWorkspace, defaults: &DreamDefaults) -> Result<(), String> {
    let path = dream_defaults_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 config 目录失败: {e}"))?;
    }
    let normalized = normalize_dream_defaults(defaults.clone());
    let yaml = serde_yaml::to_string(&normalized)
        .map_err(|e| format!("序列化 dream-defaults.yaml 失败: {e}"))?;
    fs::write(&path, yaml).map_err(|e| format!("写入 dream-defaults.yaml 失败: {e}"))?;
    Ok(())
}

/// Get the effective dream trigger time for an employee.
/// Falls back to the global default when the employee config has no override.
pub fn effective_dream_time(root: &RootWorkspace, employee_id: &str) -> String {
    let defaults = read_dream_defaults(root);
    effective_dream_time_with_defaults(root, employee_id, &defaults)
}

/// Like `effective_dream_time` but accepts pre-loaded defaults to avoid
/// repeated disk reads when scanning multiple employees.
pub fn effective_dream_time_with_defaults(
    root: &RootWorkspace,
    employee_id: &str,
    defaults: &DreamDefaults,
) -> String {
    let config = read_dream_config(root, employee_id).unwrap_or_default();
    config
        .schedule
        .time
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| defaults.default_dream_time.clone())
}

fn dream_config_path(root: &RootWorkspace, employee_id: &str) -> PathBuf {
    root.employees_dir().join(employee_id).join("dream.yaml")
}

/// Read the Dream config for an employee.
/// Returns default config if the file doesn't exist yet (graceful init).
/// Returns an error for `__dream__` since it cannot have a dream config.
pub fn read_dream_config(root: &RootWorkspace, employee_id: &str) -> Result<DreamConfig, String> {
    if employee_id == DREAM_EMPLOYEE_ID {
        return Err("__dream__ 不能拥有 Dream 配置".to_string());
    }
    let path = dream_config_path(root, employee_id);
    if !path.is_file() {
        return Ok(DreamConfig::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 dream.yaml 失败: {e}"))?;
    serde_yaml::from_str(&raw).map_err(|e| format!("解析 dream.yaml 失败: {e}"))
}

/// Write the Dream config for an employee.
/// Returns an error for `__dream__` since it cannot have a dream config.
pub fn write_dream_config(
    root: &RootWorkspace,
    employee_id: &str,
    config: &DreamConfig,
) -> Result<(), String> {
    if employee_id == DREAM_EMPLOYEE_ID {
        return Err("__dream__ 不能拥有 Dream 配置".to_string());
    }
    let path = dream_config_path(root, employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 dream.yaml 目录失败: {e}"))?;
    }
    let yaml = serde_yaml::to_string(config).map_err(|e| format!("序列化 dream.yaml 失败: {e}"))?;
    fs::write(&path, yaml).map_err(|e| format!("写入 dream.yaml 失败: {e}"))?;
    Ok(())
}

const DREAM_SCHEDULE_DAILY_MIGRATION_MARKER: &str = "dream-schedule-daily-v1.done";

/// Migrate ordinary employees' `dream.yaml` to daily schedule when still manual
/// or missing an explicit `schedule:` section (legacy minimal files).
pub fn migrate_dream_schedules_to_daily(root: &RootWorkspace) -> Result<Vec<String>, String> {
    let employees = employee::list(root)?;
    let mut updated = Vec::new();

    for entry in employees {
        if entry.id == DREAM_EMPLOYEE_ID || entry.kind != EmployeeKind::Ordinary {
            continue;
        }
        let path = dream_config_path(root, &entry.id);
        if !path.is_file() {
            continue;
        }

        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("读取 dream.yaml 失败 ({}): {e}", path.display()))?;
        let mut config = read_dream_config(root, &entry.id)?;

        let has_schedule_section = raw.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == "schedule:" || trimmed.starts_with("schedule:")
        });
        let is_manual = config.schedule.schedule_type == "manual";

        if !is_manual && has_schedule_section {
            continue;
        }

        config.schedule.schedule_type = "daily".to_string();
        write_dream_config(root, &entry.id, &config)?;
        updated.push(entry.id);
    }

    Ok(updated)
}

/// One-time migration invoked during root workspace init.
pub fn migrate_dream_schedules_to_daily_once(root: &RootWorkspace) -> Result<(), String> {
    let migrations_dir = root.state_dir().join("migrations");
    let marker = migrations_dir.join(DREAM_SCHEDULE_DAILY_MIGRATION_MARKER);
    if marker.is_file() {
        return Ok(());
    }

    let updated = migrate_dream_schedules_to_daily(root)?;

    fs::create_dir_all(&migrations_dir).map_err(|e| format!("创建 migrations 目录失败: {e}"))?;
    let marker_body = if updated.is_empty() {
        "ok\n".to_string()
    } else {
        format!("updated:\n{}\n", updated.join("\n"))
    };
    fs::write(&marker, marker_body).map_err(|e| format!("写入 migration marker 失败: {e}"))?;
    Ok(())
}

// ── Types ──────────────────────────────────────────────────────────────────

/// Format: `dream-run-<YYYY-MM-DD>-<8-char-hex>`
fn generate_dream_run_id() -> String {
    let date = Utc::now().format("%Y-%m-%d");
    let short = &Uuid::new_v4().to_string().replace('-', "")[..8];
    format!("dream-run-{date}-{short}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamInputManifest {
    pub dream_run_id: String,
    pub target_employee_id: String,
    pub target_prompt_snapshot_path: String,
    pub selected_source_sessions: Vec<RuntimeSelectedSession>,
    pub created_at: String,
    /// `"all"` or `"filtered"`
    pub scan_scope: String,
    pub latest_session_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSelectedSession {
    pub workspace_id: String,
    pub workspace_name: String,
    pub session_id: String,
    pub title: String,
    pub last_message_at: String,
    pub message_count: u32,
    pub snapshot_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedSession {
    pub workspace_id: String,
    pub workspace_name: String,
    pub workspace_path: String,
    pub session_id: String,
    pub title: String,
    pub last_message_at: String,
    pub message_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamPrepareResult {
    pub dream_run_id: String,
    pub run_workspace_path: String,
    pub selected_sessions: Vec<SelectedSession>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DreamPrepareInput {
    pub target_employee_id: String,
    #[serde(default)]
    pub workspace_filter: Option<Vec<String>>,
}

// ── Dream log ──────────────────────────────────────────────────────────────

fn dream_log_path(root: &RootWorkspace) -> PathBuf {
    root.dream_employee_dir().join("logs/dream/dream.log")
}

/// Resolve the on-disk path for a specific dream run workspace.
pub fn dream_run_workspace(root: &RootWorkspace, dream_run_id: &str) -> PathBuf {
    root.dream_employee_dir()
        .join("workspaces")
        .join(dream_run_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamLogEntry {
    pub timestamp: String,
    pub event: String,
    pub message: String,
}

/// Read the most recent N dream log entries (newest first).
pub fn read_dream_log(root: &RootWorkspace, limit: usize) -> Vec<DreamLogEntry> {
    let path = dream_log_path(root);
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut entries: Vec<DreamLogEntry> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }
    entries.reverse();
    entries
}

const MAX_LOG_ENTRIES: usize = 500;

pub fn append_dream_log(root: &RootWorkspace, event: &str, message: &str) {
    let path = dream_log_path(root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let entry = serde_json::json!({
        "timestamp": iso_now(),
        "event": event,
        "message": message,
    });
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let Ok(line) = serde_json::to_string(&entry) else {
        return;
    };
    let _ = file.write_all(line.as_bytes());
    let _ = file.write_all(b"\n");

    truncate_dream_log_if_needed(&path);
}

/// Keep only the most recent `MAX_LOG_ENTRIES` lines in the log file.
fn truncate_dream_log_if_needed(path: &Path) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() <= MAX_LOG_ENTRIES {
        return;
    }
    let keep = &lines[lines.len() - MAX_LOG_ENTRIES..];
    let truncated = keep.join("\n") + "\n";
    let _ = fs::write(path, truncated);
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

// ── Session discovery ──────────────────────────────────────────────────────

/// Discover the most recent sessions across all workspaces bound to `target_employee_id`.
///
/// If `workspace_filter` is provided, only those workspace IDs are scanned.
/// Returns up to `limit` sessions sorted by `last_message_at` descending.
pub fn discover_recent_sessions(
    root: &RootWorkspace,
    target_employee_id: &str,
    workspace_filter: Option<&[String]>,
    limit: usize,
) -> Result<Vec<SelectedSession>, String> {
    let memberships = employee::list_workspaces_for_employee(root, target_employee_id)?;

    let mut all_sessions: Vec<SelectedSession> = Vec::new();

    for membership in &memberships {
        if let Some(filter) = workspace_filter {
            if !filter.iter().any(|f| f == &membership.id) {
                continue;
            }
        }

        let ws_path = Path::new(&membership.path);
        if !ws_path.is_dir() {
            continue;
        }

        let sessions = match session::list(ws_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for meta in sessions {
            all_sessions.push(SelectedSession {
                workspace_id: membership.id.clone(),
                workspace_name: membership.name.clone(),
                workspace_path: membership.path.clone(),
                session_id: meta.id,
                title: meta.title,
                last_message_at: meta.last_message_at,
                message_count: meta.message_count,
            });
        }
    }

    all_sessions.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));
    all_sessions.truncate(limit);
    Ok(all_sessions)
}

// ── Run preparation ────────────────────────────────────────────────────────

/// Prepare a Dream run workspace with all input snapshots.
///
/// Creates `employees/__dream__/workspaces/<dream_run_id>/` containing:
/// - `input-manifest.json`
/// - `snapshots/prompt.md` (target employee prompt copy)
/// - `snapshots/sessions/<ws_id>__<session_id>/meta.json` + `transcript.jsonl`
pub fn prepare_dream_run(
    root: &RootWorkspace,
    input: DreamPrepareInput,
) -> Result<DreamPrepareResult, String> {
    let target_id = input.target_employee_id.trim();

    // Block dreaming about __dream__ itself
    if target_id == DREAM_EMPLOYEE_ID {
        return Err("不能对 __dream__ 自身执行 Dream run".to_string());
    }

    // Validate target employee exists and is ordinary
    let detail = employee::get_detail(root, target_id)?;
    if detail.registry_entry.kind != EmployeeKind::Ordinary {
        return Err(format!(
            "目标员工 {target_id} 的类型不是 Ordinary，无法执行 Dream run"
        ));
    }

    let dream_run_id = generate_dream_run_id();
    let dream_config = read_dream_config(root, target_id).unwrap_or_default();
    let limit = dream_config.session_scan.latest_sessions;
    let scan_scope = if input.workspace_filter.is_some() {
        "filtered"
    } else {
        "all"
    };

    append_dream_log(
        root,
        "run_started",
        &format!("Dream run {dream_run_id} started for employee {target_id}"),
    );

    // Discover sessions
    let selected =
        discover_recent_sessions(root, target_id, input.workspace_filter.as_deref(), limit)?;

    if selected.is_empty() {
        let reason = format!("员工 {target_id} 没有可用的 session，跳过 Dream run");
        append_dream_log(root, "skipped_no_sessions", &reason);
        return Ok(DreamPrepareResult {
            dream_run_id,
            run_workspace_path: String::new(),
            selected_sessions: Vec::new(),
            skipped_reason: Some(reason),
        });
    }

    append_dream_log(
        root,
        "sessions_selected",
        &format!(
            "Selected {} sessions for run {dream_run_id}: [{}]",
            selected.len(),
            selected
                .iter()
                .map(|s| format!("{}:{}", s.workspace_id, s.session_id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );

    // Create run workspace
    let run_dir = root
        .dream_employee_dir()
        .join("workspaces")
        .join(&dream_run_id);
    let snapshots_dir = run_dir.join("snapshots");
    fs::create_dir_all(&snapshots_dir)
        .map_err(|e| format!("创建 Dream run workspace 失败: {e}"))?;

    // Snapshot: target employee prompt.md
    let prompt_content = employee::read_employee_prompt(root, target_id)?;
    let prompt_snapshot = snapshots_dir.join("prompt.md");
    fs::write(&prompt_snapshot, &prompt_content)
        .map_err(|e| format!("写入 prompt 快照失败: {e}"))?;

    // Snapshot: selected sessions
    let sessions_snap_dir = snapshots_dir.join("sessions");
    fs::create_dir_all(&sessions_snap_dir)
        .map_err(|e| format!("创建 sessions 快照目录失败: {e}"))?;

    for sel in &selected {
        let snap_name = format!("{}_{}", sel.workspace_id, sel.session_id);
        let snap_dir = sessions_snap_dir.join(&snap_name);
        fs::create_dir_all(&snap_dir).map_err(|e| format!("创建 session 快照目录失败: {e}"))?;

        let ws_path = Path::new(&sel.workspace_path);

        // Copy meta.json
        let src_meta = session::sessions_dir(ws_path)
            .join(&sel.session_id)
            .join("meta.json");
        if src_meta.is_file() {
            let dst_meta = snap_dir.join("meta.json");
            if let Err(e) = fs::copy(&src_meta, &dst_meta) {
                append_dream_log(
                    root,
                    "snapshot_error",
                    &format!("复制 meta.json 失败 (session {}): {e}", sel.session_id),
                );
            }
        }

        // Copy transcript.jsonl
        let src_transcript = session::transcript_path(ws_path, &sel.session_id);
        if src_transcript.is_file() {
            let dst_transcript = snap_dir.join("transcript.jsonl");
            if let Err(e) = fs::copy(&src_transcript, &dst_transcript) {
                append_dream_log(
                    root,
                    "snapshot_error",
                    &format!(
                        "复制 transcript.jsonl 失败 (session {}): {e}",
                        sel.session_id
                    ),
                );
            }
        }
    }

    // Write runtime-facing input-manifest.json. Do not include source workspace
    // paths; the runtime must only read the snapshots inside this run workspace.
    let runtime_sessions: Vec<RuntimeSelectedSession> = selected
        .iter()
        .map(|sel| RuntimeSelectedSession {
            workspace_id: sel.workspace_id.clone(),
            workspace_name: sel.workspace_name.clone(),
            session_id: sel.session_id.clone(),
            title: sel.title.clone(),
            last_message_at: sel.last_message_at.clone(),
            message_count: sel.message_count,
            snapshot_path: format!("snapshots/sessions/{}_{}", sel.workspace_id, sel.session_id),
        })
        .collect();

    // Write input-manifest.json
    let manifest = DreamInputManifest {
        dream_run_id: dream_run_id.clone(),
        target_employee_id: target_id.to_string(),
        target_prompt_snapshot_path: "snapshots/prompt.md".to_string(),
        selected_source_sessions: runtime_sessions,
        created_at: iso_now(),
        scan_scope: scan_scope.to_string(),
        latest_session_limit: limit,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("序列化 input-manifest.json 失败: {e}"))?;
    fs::write(run_dir.join("input-manifest.json"), manifest_json)
        .map_err(|e| format!("写入 input-manifest.json 失败: {e}"))?;

    Ok(DreamPrepareResult {
        dream_run_id,
        run_workspace_path: run_dir.to_string_lossy().into_owned(),
        selected_sessions: selected,
        skipped_reason: None,
    })
}

// ── Dream Result Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DreamDecision {
    NoUpdate,
    UpdateRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSessionRef {
    pub workspace_id: String,
    pub session_id: String,
    /// ISO 8601 timestamp of the session's last update
    #[serde(default)]
    pub last_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptUpdate {
    pub section: String,
    /// `"add"` | `"modify"` | `"remove"`
    pub action: String,
    pub content: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamResult {
    pub decision: DreamDecision,
    pub target_employee_id: String,
    pub dream_run_id: String,
    pub summary: String,
    pub source_sessions: Vec<SourceSessionRef>,
    /// Present only when decision == UpdateRequired
    pub updates: Option<Vec<PromptUpdate>>,
    pub impact: Option<String>,
    /// "pending" | "approved" | "applying" | "applied" | "rejected" | "failed"
    #[serde(default = "default_status_pending")]
    pub status: String,
    /// e.g. "employees/{target}/prompt.md"
    #[serde(default)]
    pub source_prompt_path: Option<String>,
    /// ISO 8601 creation timestamp
    #[serde(default)]
    pub created_at: Option<String>,
}

fn default_status_pending() -> String {
    "pending".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentDreamResult {
    pub dream_run_id: String,
    pub target_employee_id: String,
    pub decision: DreamDecision,
    pub summary: String,
    pub source_sessions: Vec<SourceSessionRef>,
    pub created_at: String,
    pub parse_failed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<String>,
}

/// Persisted pending update request wrapping the full DreamResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdateRequest {
    pub dream_run_id: String,
    pub target_employee_id: String,
    pub created_at: String,
    pub result: DreamResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

// ── Path helpers ───────────────────────────────────────────────────────────

fn recent_dream_result_path(root: &RootWorkspace, employee_id: &str) -> PathBuf {
    root.employees_dir()
        .join(employee_id)
        .join("recent-dream-result.json")
}

fn pending_request_dir(root: &RootWorkspace, employee_id: &str) -> PathBuf {
    root.employees_dir()
        .join(employee_id)
        .join("prompt-update-requests/pending")
}

fn pending_current_path(root: &RootWorkspace, employee_id: &str) -> PathBuf {
    pending_request_dir(root, employee_id).join("current.json")
}

// ── Result validation ─────────────────────────────────────────────────────

fn validate_dream_result_for_persistence(
    root: &RootWorkspace,
    result: &DreamResult,
) -> Result<(), String> {
    let target_employee_id = result.target_employee_id.trim();
    if target_employee_id.is_empty() {
        return Err("Dream result 缺少 target_employee_id".to_string());
    }
    if target_employee_id == DREAM_EMPLOYEE_ID {
        return Err("__dream__ 不能作为 Dream target".to_string());
    }
    let detail = employee::get_detail(root, target_employee_id)?;
    if detail.registry_entry.kind != EmployeeKind::Ordinary {
        return Err(format!("目标员工 {target_employee_id} 不是 Ordinary"));
    }
    if result.dream_run_id.trim().is_empty() {
        return Err("Dream result 缺少 dream_run_id".to_string());
    }
    if result.summary.trim().is_empty() {
        return Err("Dream result 缺少 summary".to_string());
    }
    if result.source_sessions.is_empty() {
        return Err("Dream result 缺少 source_sessions".to_string());
    }
    for source in &result.source_sessions {
        if source.workspace_id.trim().is_empty() || source.session_id.trim().is_empty() {
            return Err("Dream result 包含无效 source session".to_string());
        }
    }

    if result.decision == DreamDecision::UpdateRequired {
        let updates = result
            .updates
            .as_ref()
            .ok_or_else(|| "decision 为 update_required 但缺少 updates".to_string())?;
        if updates.is_empty() {
            return Err("decision 为 update_required 但 updates 为空".to_string());
        }
        for update in updates {
            if update.action.trim().is_empty() {
                return Err("Dream update 缺少 action".to_string());
            }
            if update.reason.trim().is_empty() {
                return Err("Dream update 缺少 reason".to_string());
            }
            if update.action != "remove" && update.content.trim().is_empty() {
                return Err("Dream update 缺少 content".to_string());
            }
        }
    }

    Ok(())
}

// ── Recent result persistence ─────────────────────────────────────────────

fn save_recent_result(root: &RootWorkspace, recent: &RecentDreamResult) -> Result<(), String> {
    let path = recent_dream_result_path(root, &recent.target_employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建 recent-dream-result 目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(recent)
        .map_err(|e| format!("序列化 recent-dream-result 失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入 recent-dream-result.json 失败: {e}"))?;
    Ok(())
}

/// Read the most recent dream result for an employee.
pub fn read_recent_dream_result(
    root: &RootWorkspace,
    employee_id: &str,
) -> Option<RecentDreamResult> {
    let path = recent_dream_result_path(root, employee_id);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

// ── Pending request persistence ───────────────────────────────────────────

fn save_pending_request(root: &RootWorkspace, result: &DreamResult) -> Result<(), String> {
    let dir = pending_request_dir(root, &result.target_employee_id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 pending 目录失败: {e}"))?;

    let pending = PendingUpdateRequest {
        dream_run_id: result.dream_run_id.clone(),
        target_employee_id: result.target_employee_id.clone(),
        created_at: iso_now(),
        result: result.clone(),
        error_message: None,
    };
    let json = serde_json::to_string_pretty(&pending)
        .map_err(|e| format!("序列化 pending request 失败: {e}"))?;
    let path = pending_current_path(root, &result.target_employee_id);
    fs::write(&path, json).map_err(|e| format!("写入 pending/current.json 失败: {e}"))?;
    Ok(())
}

fn remove_pending_request(root: &RootWorkspace, employee_id: &str) {
    let path = pending_current_path(root, employee_id);
    let _ = fs::remove_file(&path);
}

fn request_status_dir(root: &RootWorkspace, employee_id: &str, status: &str) -> PathBuf {
    root.employees_dir()
        .join(employee_id)
        .join("prompt-update-requests")
        .join(status)
}

const PROMPT_WRITTEN_MARKER: &str = "prompt_written.marker";

fn applying_prompt_written_marker(root: &RootWorkspace, employee_id: &str) -> PathBuf {
    request_status_dir(root, employee_id, "applying").join(PROMPT_WRITTEN_MARKER)
}

fn write_prompt_atomically(prompt_path: &Path, new_prompt: &str) -> Result<(), String> {
    let tmp_path = prompt_path.with_extension("md.tmp");
    fs::write(&tmp_path, new_prompt).map_err(|e| format!("写入临时 prompt 文件失败: {e}"))?;
    fs::rename(&tmp_path, prompt_path).map_err(|e| format!("替换 prompt.md 失败: {e}"))
}

fn move_request_to_status(
    root: &RootWorkspace,
    employee_id: &str,
    from_status: &str,
    to_status: &str,
) -> Result<PendingUpdateRequest, String> {
    let src = request_status_dir(root, employee_id, from_status).join("current.json");
    if !src.is_file() {
        return Err(format!(
            "员工 {employee_id} 没有 {from_status} 状态的 request"
        ));
    }
    let content = fs::read_to_string(&src)
        .map_err(|e| format!("读取 {from_status}/current.json 失败: {e}"))?;
    let mut req: PendingUpdateRequest = serde_json::from_str(&content)
        .map_err(|e| format!("解析 {from_status}/current.json 失败: {e}"))?;
    req.result.status = to_status.to_string();

    let dst_dir = request_status_dir(root, employee_id, to_status);
    fs::create_dir_all(&dst_dir).map_err(|e| format!("创建 {to_status} 目录失败: {e}"))?;
    let updated = serde_json::to_string_pretty(&req)
        .map_err(|e| format!("序列化 {to_status}/current.json 失败: {e}"))?;
    fs::write(dst_dir.join("current.json"), updated)
        .map_err(|e| format!("写入 {to_status}/current.json 失败: {e}"))?;
    let _ = fs::remove_file(&src);

    Ok(req)
}

/// Read the current pending update request for an employee.
pub fn read_pending_request(
    root: &RootWorkspace,
    employee_id: &str,
) -> Option<PendingUpdateRequest> {
    let path = pending_current_path(root, employee_id);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Read the most advanced in-flight review request (applying > approved > pending > failed).
pub fn read_active_review_request(
    root: &RootWorkspace,
    employee_id: &str,
) -> Option<PendingUpdateRequest> {
    for status in ["applying", "approved", "pending", "failed"] {
        let path = request_status_dir(root, employee_id, status).join("current.json");
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if let Ok(mut req) = serde_json::from_str::<PendingUpdateRequest>(&content) {
            req.result.status = status.to_string();
            if status == "failed" {
                req.error_message = read_review_request_error(root, employee_id);
            }
            return Some(req);
        }
    }
    None
}

pub fn read_review_request_error(root: &RootWorkspace, employee_id: &str) -> Option<String> {
    let path = request_status_dir(root, employee_id, "failed").join("error.txt");
    fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Recover stranded review requests for all ordinary employees. Safe at startup when Dream is idle.
pub fn recover_all_stranded_review_requests(root: &RootWorkspace) -> Result<(), String> {
    let employees = employee::list(root)?;
    for entry in employees {
        if entry.kind != EmployeeKind::Ordinary {
            continue;
        }
        recover_stranded_review_requests(root, &entry.id, false)?;
    }
    Ok(())
}

/// Recover review requests stranded after a crash. No-op while Dream Phase 2 is active.
pub fn recover_stranded_review_requests(
    root: &RootWorkspace,
    employee_id: &str,
    dream_phase2_active: bool,
) -> Result<(), String> {
    if dream_phase2_active {
        return Ok(());
    }
    recover_stranded_applying(root, employee_id)?;

    let approved_path = request_status_dir(root, employee_id, "approved").join("current.json");
    if approved_path.is_file() {
        append_dream_log(
            root,
            "recovery",
            &format!("发现 approved 状态的残留请求 (target: {employee_id})，移回 pending 以便重试"),
        );
        move_request_to_status(root, employee_id, "approved", "pending")?;
    }
    Ok(())
}

/// Load the request to execute Dream Phase 2, accepting approved or failed requests for retry.
pub fn take_request_for_phase2(
    root: &RootWorkspace,
    employee_id: &str,
) -> Result<PendingUpdateRequest, String> {
    recover_stranded_applying(root, employee_id)?;
    if let Some(req) = read_request_in_status(root, employee_id, "approved") {
        return Ok(req);
    }
    if read_request_in_status(root, employee_id, "failed").is_some() {
        return move_request_to_status(root, employee_id, "failed", "approved");
    }
    move_request_to_approved(root, employee_id)
}

fn read_request_in_status(
    root: &RootWorkspace,
    employee_id: &str,
    status: &str,
) -> Option<PendingUpdateRequest> {
    let path = request_status_dir(root, employee_id, status).join("current.json");
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Reject the current pending update request.
/// Moves `pending/current.json` → `rejected/current.json`.
/// Does NOT modify employee `prompt.md`.
pub fn reject_pending_request(root: &RootWorkspace, employee_id: &str) -> Result<(), String> {
    let req = move_request_to_status(root, employee_id, "pending", "rejected")?;
    append_dream_log(
        root,
        "request_rejected",
        &format!(
            "Dream run {} 的更新请求已被拒绝 (target: {})",
            req.dream_run_id, employee_id
        ),
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub success: bool,
    pub target_employee_id: String,
    pub dream_run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Recover a request stranded in `applying/` (e.g. from a previous crash).
/// If the prompt was already written, completes `applying→applied`; otherwise moves to `failed/`.
fn recover_stranded_applying(root: &RootWorkspace, employee_id: &str) -> Result<(), String> {
    let applying_path = request_status_dir(root, employee_id, "applying").join("current.json");
    if !applying_path.is_file() {
        return Ok(());
    }

    let marker = applying_prompt_written_marker(root, employee_id);
    if marker.is_file() {
        append_dream_log(
            root,
            "recovery",
            &format!(
                "发现 prompt 已写入但未完成 applying→applied (target: {employee_id})，补全为 applied"
            ),
        );
        move_request_to_status(root, employee_id, "applying", "applied")?;
        let _ = fs::remove_file(&marker);
        return Ok(());
    }

    append_dream_log(
        root,
        "recovery",
        &format!("发现 applying 状态的残留请求 (target: {employee_id})，移入 failed"),
    );
    move_request_to_status(root, employee_id, "applying", "failed")?;
    fs::write(
        request_status_dir(root, employee_id, "failed").join("error.txt"),
        "应用过程中进程异常中断，请求已被标记为失败",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Public wrapper for `move_request_to_status` (used by commands layer for phase 2).
pub fn move_request_to_status_pub(
    root: &RootWorkspace,
    employee_id: &str,
    from_status: &str,
    to_status: &str,
) -> Result<PendingUpdateRequest, String> {
    move_request_to_status(root, employee_id, from_status, to_status)
}

/// Move pending -> approved and return the request (Phase D step 1).
pub fn move_request_to_approved(
    root: &RootWorkspace,
    employee_id: &str,
) -> Result<PendingUpdateRequest, String> {
    recover_stranded_applying(root, employee_id)?;

    let req = move_request_to_status(root, employee_id, "pending", "approved")?;
    append_dream_log(
        root,
        "request_approved",
        &format!(
            "Dream run {} 的更新请求已被批准 (target: {})",
            req.dream_run_id, employee_id
        ),
    );
    Ok(req)
}

/// Apply Phase 2 prompt output and atomically complete the review request (`applying→applied`).
/// Writes `prompt_written.marker` before the status move so recovery can finish if the move fails.
pub fn apply_prompt_and_complete_request(
    root: &RootWorkspace,
    employee_id: &str,
    dream_run_id: &str,
    new_prompt: &str,
) -> Result<ApplyResult, String> {
    validate_runtime_prompt_candidate(root, employee_id, new_prompt)?;

    let prompt_path = root.employees_dir().join(employee_id).join("prompt.md");
    write_prompt_atomically(&prompt_path, new_prompt)?;

    let marker = applying_prompt_written_marker(root, employee_id);
    fs::write(&marker, dream_run_id)
        .map_err(|e| format!("写入 prompt_written.marker 失败: {e}"))?;

    match move_request_to_status(root, employee_id, "applying", "applied") {
        Ok(_req) => {
            let _ = fs::remove_file(&marker);
            Ok(ApplyResult {
                success: true,
                target_employee_id: employee_id.to_string(),
                dream_run_id: dream_run_id.to_string(),
                error: None,
            })
        }
        Err(e) => Err(format!("prompt 已写入但无法迁移到 applied: {e}")),
    }
}

/// Apply a full prompt text (output by Phase 2 runtime) to the target employee's `prompt.md`.
/// Atomic write via temp file + rename. Prefer `apply_prompt_and_complete_request` for Phase 2.
pub fn apply_prompt_from_runtime(
    root: &RootWorkspace,
    employee_id: &str,
    dream_run_id: &str,
    new_prompt: &str,
) -> Result<ApplyResult, String> {
    validate_runtime_prompt_candidate(root, employee_id, new_prompt)?;

    let prompt_path = root.employees_dir().join(employee_id).join("prompt.md");
    write_prompt_atomically(&prompt_path, new_prompt)?;

    Ok(ApplyResult {
        success: true,
        target_employee_id: employee_id.to_string(),
        dream_run_id: dream_run_id.to_string(),
        error: None,
    })
}

fn validate_runtime_prompt_candidate(
    root: &RootWorkspace,
    employee_id: &str,
    new_prompt: &str,
) -> Result<(), String> {
    if employee_id == DREAM_EMPLOYEE_ID {
        return Err("不能写入 __dream__ 的 prompt.md".to_string());
    }
    let detail = employee::get_detail(root, employee_id)?;
    if detail.registry_entry.kind != EmployeeKind::Ordinary {
        return Err(format!("目标员工 {employee_id} 不是 Ordinary"));
    }
    let prompt_path = root.employees_dir().join(employee_id).join("prompt.md");
    let current_prompt = fs::read_to_string(&prompt_path).unwrap_or_default();
    let candidate = new_prompt.trim();
    if candidate.is_empty() {
        return Err("promptCandidate 为空".to_string());
    }
    if candidate.starts_with('{') || candidate.starts_with('[') {
        return Err("promptCandidate 不能是 JSON object / array".to_string());
    }
    if candidate.starts_with("```") && candidate.ends_with("```") {
        return Err("promptCandidate 不能整体包在 markdown code fence 中".to_string());
    }
    if !candidate
        .lines()
        .any(|line| line.trim_start().starts_with('#'))
    {
        return Err("promptCandidate markdown 必须包含至少一个 heading".to_string());
    }
    let minimum = current_prompt.trim().len().saturating_div(2).max(40);
    if candidate.len() < minimum {
        return Err("promptCandidate 长度低于安全阈值".to_string());
    }
    Ok(())
}

// ── Result processing ─────────────────────────────────────────────────────

/// Process a validated Dream result: save recent result & handle pending requests.
pub fn process_dream_result(root: &RootWorkspace, result: &DreamResult) -> Result<(), String> {
    validate_dream_result_for_persistence(root, result)?;

    let recent = RecentDreamResult {
        dream_run_id: result.dream_run_id.clone(),
        target_employee_id: result.target_employee_id.clone(),
        decision: result.decision.clone(),
        summary: result.summary.clone(),
        source_sessions: result.source_sessions.clone(),
        created_at: iso_now(),
        parse_failed: false,
        raw_output: None,
    };
    save_recent_result(root, &recent)?;

    match result.decision {
        DreamDecision::NoUpdate => {
            remove_pending_request(root, &result.target_employee_id);
            append_dream_log(
                root,
                "run_completed",
                &format!(
                    "Dream run {} 完成: no_update (target: {})",
                    result.dream_run_id, result.target_employee_id
                ),
            );
        }
        DreamDecision::UpdateRequired => {
            save_pending_request(root, result)?;
            append_dream_log(
                root,
                "run_completed",
                &format!(
                    "Dream run {} 完成: update_required, {} 条更新 (target: {})",
                    result.dream_run_id,
                    result.updates.as_ref().map_or(0, |u| u.len()),
                    result.target_employee_id
                ),
            );
        }
    }

    Ok(())
}

// ── Scheduling helpers ────────────────────────────────────────────────────

use chrono::NaiveTime;

/// Whether the employee is configured for daily auto-scheduling.
pub fn should_run_dream(root: &RootWorkspace, employee_id: &str) -> bool {
    let config = match read_dream_config(root, employee_id) {
        Ok(c) => c,
        Err(_) => return false,
    };
    config.enabled && config.schedule.schedule_type == "daily"
}

/// Parse an "HH:MM" string into `NaiveTime`.
fn parse_hm(hm: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(hm, "%H:%M").ok()
}

/// Check if the trigger time has been reached today but no dream result
/// was recorded for today yet (i.e. a run is due or missed).
pub fn has_missed_dream(root: &RootWorkspace, employee_id: &str) -> bool {
    let defaults = read_dream_defaults(root);
    has_missed_dream_with_defaults(root, employee_id, &defaults)
}

fn has_missed_dream_with_defaults(
    root: &RootWorkspace,
    employee_id: &str,
    defaults: &DreamDefaults,
) -> bool {
    let trigger = effective_dream_time_with_defaults(root, employee_id, defaults);
    let trigger_time = match parse_hm(&trigger) {
        Some(t) => t,
        None => return false,
    };

    let now = chrono::Local::now();
    if now.time() < trigger_time {
        return false;
    }

    let today_str = now.format("%Y-%m-%d").to_string();

    match read_recent_dream_result(root, employee_id) {
        Some(result) => {
            // Use the created_at ISO timestamp to determine if a run was already
            // recorded today — much more reliable than pattern-matching on run IDs.
            !result.created_at.starts_with(&today_str)
        }
        None => true,
    }
}

/// Scan all employees and return IDs of those due for a scheduled dream run.
/// Reads global defaults once and reuses for all employees.
pub fn scan_due_employees(root: &RootWorkspace) -> Vec<String> {
    let entries = match employee::list(root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let defaults = read_dream_defaults(root);

    entries
        .into_iter()
        .filter(|e| {
            e.id != DREAM_EMPLOYEE_ID
                && should_run_dream(root, &e.id)
                && has_missed_dream_with_defaults(root, &e.id, &defaults)
                && !has_active_review_request(root, &e.id)
        })
        .map(|e| e.id)
        .collect()
}

/// Check if an employee has any pending (unapproved) dream update request.
pub fn has_pending_request(root: &RootWorkspace, employee_id: &str) -> bool {
    pending_current_path(root, employee_id).is_file()
}

/// Check if an employee has an approved or applying request in flight.
pub fn has_inflight_review_request(root: &RootWorkspace, employee_id: &str) -> bool {
    request_status_dir(root, employee_id, "approved")
        .join("current.json")
        .is_file()
        || request_status_dir(root, employee_id, "applying")
            .join("current.json")
            .is_file()
}

/// Check if an employee has a failed Dream prompt update request.
pub fn has_failed_review_request(root: &RootWorkspace, employee_id: &str) -> bool {
    request_status_dir(root, employee_id, "failed")
        .join("current.json")
        .is_file()
}

/// Whether the employee has any review request that still needs product attention.
pub fn has_active_review_request(root: &RootWorkspace, employee_id: &str) -> bool {
    has_pending_request(root, employee_id)
        || has_inflight_review_request(root, employee_id)
        || has_failed_review_request(root, employee_id)
}

/// List ordinary employee ids that currently have an active Dream review request.
pub fn list_employees_with_pending_requests(root: &RootWorkspace) -> Result<Vec<String>, String> {
    let employees = employee::list(root)?;
    Ok(employees
        .into_iter()
        .filter(|entry| {
            entry.kind == EmployeeKind::Ordinary && has_active_review_request(root, &entry.id)
        })
        .map(|entry| entry.id)
        .collect())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{employee as emp, root_workspace, session};

    fn setup() -> (tempfile::TempDir, RootWorkspace) {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        (tmp, root)
    }

    fn create_employee(root: &RootWorkspace, id: &str) {
        emp::create(root, emp::CreateEmployeeInput::basic(id, id)).expect("create employee");
    }

    fn create_workspace_with_sessions(
        tmp: &tempfile::TempDir,
        root: &RootWorkspace,
        employee_id: &str,
        ws_name: &str,
        session_count: usize,
    ) -> PathBuf {
        let ws_path = tmp.path().join(ws_name);
        fs::create_dir_all(ws_path.join(".chawork/state")).expect("create ws dirs");

        let ws_id = Uuid::new_v4().to_string();
        let ws = crate::services::workspace::WorkspaceState {
            id: ws_id.clone(),
            name: ws_name.to_string(),
            path: ws_path.to_string_lossy().into_owned(),
            created_at: iso_now(),
            last_active_at: iso_now(),
            active_session_id: None,
            domain_pack_id: None,
            index_status: "stale".to_string(),
            pending_proposals_count: 0,
            bound_employee_name: None,
            bound_employee_id: None,
        };
        let json = serde_json::to_string_pretty(&ws).expect("serialize ws");
        fs::write(ws_path.join(".chawork/state/workspace.json"), json).expect("write ws");

        emp::bind_workspace(root, employee_id, &ws_path, &ws_id, ws_name).expect("bind workspace");

        for _ in 0..session_count {
            session::create(&ws_path, &ws_id).expect("create session");
        }

        ws_path
    }

    #[test]
    fn migrates_manual_and_minimal_dream_yaml_to_daily() {
        let (_tmp, root) = setup();
        create_employee(&root, "alice");
        create_employee(&root, "bob");

        let alice_path = dream_config_path(&root, "alice");
        fs::write(&alice_path, "enabled: false\nschedule:\n  type: manual\n").unwrap();

        let bob_path = dream_config_path(&root, "bob");
        fs::write(&bob_path, "enabled: true\n").unwrap();

        let updated = migrate_dream_schedules_to_daily(&root).expect("migrate");
        assert_eq!(updated.len(), 2);

        let alice = read_dream_config(&root, "alice").expect("read alice");
        assert_eq!(alice.schedule.schedule_type, "daily");

        let bob = read_dream_config(&root, "bob").expect("read bob");
        assert_eq!(bob.schedule.schedule_type, "daily");
        let bob_raw = fs::read_to_string(bob_path).unwrap();
        assert!(bob_raw.contains("type: daily"));
    }

    #[test]
    fn dream_run_id_format() {
        let id = generate_dream_run_id();
        assert!(id.starts_with("dream-run-"));
        let parts: Vec<&str> = id.split('-').collect();
        // dream-run-YYYY-MM-DD-hexhex
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "dream");
        assert_eq!(parts[1], "run");
        assert_eq!(parts[5].len(), 8);
    }

    #[test]
    fn dream_about_self_rejected() {
        let (_tmp, root) = setup();
        let err = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: DREAM_EMPLOYEE_ID.to_string(),
                workspace_filter: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("__dream__"));
    }

    #[test]
    fn dream_about_nonexistent_employee_rejected() {
        let (_tmp, root) = setup();
        let err = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "ghost".to_string(),
                workspace_filter: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("ghost"));
    }

    #[test]
    fn dream_run_skips_when_no_sessions() {
        let (_tmp, root) = setup();
        create_employee(&root, "empty-emp");

        let result = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "empty-emp".to_string(),
                workspace_filter: None,
            },
        )
        .expect("prepare");

        assert!(result.skipped_reason.is_some());
        assert!(result.selected_sessions.is_empty());
        assert!(result.run_workspace_path.is_empty());
    }

    #[test]
    fn dream_run_creates_full_workspace() {
        let (tmp, root) = setup();
        create_employee(&root, "target-emp");

        // Write a non-empty prompt
        let prompt_path = root.employees_dir().join("target-emp/prompt.md");
        fs::write(&prompt_path, "You are an IP screener.").expect("write prompt");

        let ws_path = create_workspace_with_sessions(&tmp, &root, "target-emp", "ws-dream-test", 2);

        let result = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "target-emp".to_string(),
                workspace_filter: None,
            },
        )
        .expect("prepare");

        assert!(result.skipped_reason.is_none());
        assert_eq!(result.selected_sessions.len(), 2);
        assert!(!result.run_workspace_path.is_empty());

        let run_dir = PathBuf::from(&result.run_workspace_path);
        assert!(run_dir.join("input-manifest.json").is_file());
        assert!(run_dir.join("snapshots/prompt.md").is_file());

        let snapped_prompt =
            fs::read_to_string(run_dir.join("snapshots/prompt.md")).expect("read snap prompt");
        assert_eq!(snapped_prompt, "You are an IP screener.");

        // Each session should have a snapshot directory
        for sel in &result.selected_sessions {
            let snap_name = format!("{}_{}", sel.workspace_id, sel.session_id);
            let snap_dir = run_dir.join("snapshots/sessions").join(&snap_name);
            assert!(snap_dir.is_dir(), "snapshot dir should exist: {snap_name}");
            assert!(snap_dir.join("meta.json").is_file());
            assert!(snap_dir.join("transcript.jsonl").is_file());
        }

        // Verify manifest content
        let manifest_raw =
            fs::read_to_string(run_dir.join("input-manifest.json")).expect("read manifest");
        let manifest: DreamInputManifest =
            serde_json::from_str(&manifest_raw).expect("parse manifest");
        assert_eq!(manifest.dream_run_id, result.dream_run_id);
        assert_eq!(manifest.target_employee_id, "target-emp");
        assert_eq!(manifest.scan_scope, "all");
        assert_eq!(manifest.latest_session_limit, 3);
        assert!(
            !manifest_raw.contains("workspace_path"),
            "runtime-facing manifest must not expose source workspace paths"
        );
        assert!(
            !manifest_raw.contains(&ws_path.to_string_lossy().to_string()),
            "runtime-facing manifest must not expose absolute source workspace paths"
        );

        // Dream log should exist
        let log_path = dream_log_path(&root);
        assert!(log_path.is_file());
        let log_content = fs::read_to_string(&log_path).expect("read log");
        assert!(log_content.contains("run_started"));
        assert!(log_content.contains("sessions_selected"));

        // Verify the workspace path is under __dream__
        assert!(result.run_workspace_path.contains("__dream__/workspaces/"));
    }

    #[test]
    fn dream_run_for_general_uses_seeded_prompt_snapshot() {
        let (tmp, root) = setup();
        create_workspace_with_sessions(&tmp, &root, "general", "general-dream-test", 1);

        let result = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "general".to_string(),
                workspace_filter: None,
            },
        )
        .expect("prepare general dream run");

        let run_dir = PathBuf::from(&result.run_workspace_path);
        let snapped_prompt =
            fs::read_to_string(run_dir.join("snapshots/prompt.md")).expect("read snap prompt");
        assert!(
            snapped_prompt.trim().contains("默认通用员工"),
            "general dream prompt snapshot should be seeded"
        );
    }

    #[test]
    fn dream_run_limits_to_3_sessions() {
        let (tmp, root) = setup();
        create_employee(&root, "many-sess-emp");

        create_workspace_with_sessions(&tmp, &root, "many-sess-emp", "ws-many", 5);

        let result = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "many-sess-emp".to_string(),
                workspace_filter: None,
            },
        )
        .expect("prepare");

        assert!(result.skipped_reason.is_none());
        assert_eq!(result.selected_sessions.len(), 3);
    }

    #[test]
    fn dream_run_with_workspace_filter() {
        let (tmp, root) = setup();
        create_employee(&root, "filter-emp");

        let ws1 = create_workspace_with_sessions(&tmp, &root, "filter-emp", "ws-a", 2);
        let _ws2 = create_workspace_with_sessions(&tmp, &root, "filter-emp", "ws-b", 2);

        // Get ws1 id from workspace.json
        let ws1_state_raw =
            fs::read_to_string(ws1.join(".chawork/state/workspace.json")).expect("read ws state");
        let ws1_state: serde_json::Value =
            serde_json::from_str(&ws1_state_raw).expect("parse ws state");
        let ws1_id = ws1_state["id"].as_str().unwrap().to_string();

        let result = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "filter-emp".to_string(),
                workspace_filter: Some(vec![ws1_id.clone()]),
            },
        )
        .expect("prepare");

        assert!(result.skipped_reason.is_none());
        assert_eq!(result.selected_sessions.len(), 2);
        assert!(result
            .selected_sessions
            .iter()
            .all(|s| s.workspace_id == ws1_id));

        // Verify manifest says "filtered"
        let run_dir = PathBuf::from(&result.run_workspace_path);
        let manifest_raw =
            fs::read_to_string(run_dir.join("input-manifest.json")).expect("read manifest");
        let manifest: DreamInputManifest =
            serde_json::from_str(&manifest_raw).expect("parse manifest");
        assert_eq!(manifest.scan_scope, "filtered");
    }

    #[test]
    fn discover_sessions_aggregates_across_workspaces() {
        let (tmp, root) = setup();
        create_employee(&root, "multi-ws-emp");

        create_workspace_with_sessions(&tmp, &root, "multi-ws-emp", "ws-x", 2);
        create_workspace_with_sessions(&tmp, &root, "multi-ws-emp", "ws-y", 2);

        let sessions = discover_recent_sessions(&root, "multi-ws-emp", None, 10).expect("discover");

        assert_eq!(sessions.len(), 4);
        // Should be sorted by last_message_at desc
        for window in sessions.windows(2) {
            assert!(window[0].last_message_at >= window[1].last_message_at);
        }
    }

    // ── Structured Dream result tests ────────────────────────────────────

    fn source_sessions() -> Vec<SourceSessionRef> {
        vec![SourceSessionRef {
            workspace_id: "ws1".to_string(),
            session_id: "s1".to_string(),
            last_updated_at: None,
        }]
    }

    fn default_update() -> PromptUpdate {
        PromptUpdate {
            section: "communication".to_string(),
            action: "add".to_string(),
            content: "Always greet politely".to_string(),
            reason: "Observed pattern in sessions".to_string(),
        }
    }

    fn no_update_result(run_id: &str, emp_id: &str) -> DreamResult {
        DreamResult {
            decision: DreamDecision::NoUpdate,
            target_employee_id: emp_id.to_string(),
            dream_run_id: run_id.to_string(),
            summary: "No changes needed".to_string(),
            source_sessions: source_sessions(),
            updates: None,
            impact: None,
            status: default_status_pending(),
            source_prompt_path: None,
            created_at: None,
        }
    }

    fn update_required_result(run_id: &str, emp_id: &str) -> DreamResult {
        update_required_with_updates(run_id, emp_id, vec![default_update()])
    }

    fn update_required_with_updates(
        run_id: &str,
        emp_id: &str,
        updates: Vec<PromptUpdate>,
    ) -> DreamResult {
        DreamResult {
            decision: DreamDecision::UpdateRequired,
            target_employee_id: emp_id.to_string(),
            dream_run_id: run_id.to_string(),
            summary: "Found improvements".to_string(),
            source_sessions: source_sessions(),
            updates: Some(updates),
            impact: Some("Minor style update".to_string()),
            status: default_status_pending(),
            source_prompt_path: Some(format!("employees/{emp_id}/prompt.md")),
            created_at: None,
        }
    }

    fn process_result(root: &RootWorkspace, result: DreamResult) -> RecentDreamResult {
        process_dream_result(root, &result).expect("process structured Dream result");
        read_recent_dream_result(root, &result.target_employee_id).expect("recent result")
    }

    fn runtime_prompt_candidate(body: &str) -> String {
        format!(
            "# Updated Employee Prompt\n\n## Operating Rules\n\n{body}\n\n## Runtime Evidence\n\nGenerated by Dream Phase 2 runtime candidate.\n"
        )
    }

    fn approve_with_runtime_candidate(
        root: &RootWorkspace,
        employee_id: &str,
        prompt_candidate: &str,
    ) -> Result<ApplyResult, String> {
        let req = move_request_to_approved(root, employee_id)?;
        move_request_to_status(root, employee_id, "approved", "applying")?;
        match apply_prompt_and_complete_request(
            root,
            employee_id,
            &req.dream_run_id,
            prompt_candidate,
        ) {
            Ok(result) => Ok(result),
            Err(err) => {
                move_request_to_status(root, employee_id, "applying", "failed")?;
                fs::write(
                    request_status_dir(root, employee_id, "failed").join("error.txt"),
                    &err,
                )
                .ok();
                Err(err)
            }
        }
    }

    #[test]
    fn process_no_update_saves_recent_cleans_pending() {
        let (_tmp, root) = setup();
        create_employee(&root, "noupd-emp");

        let pending_dir = pending_request_dir(&root, "noupd-emp");
        fs::create_dir_all(&pending_dir).unwrap();
        fs::write(pending_dir.join("current.json"), "{}").unwrap();
        assert!(pending_current_path(&root, "noupd-emp").is_file());

        let recent = process_result(&root, no_update_result("run-nu", "noupd-emp"));

        assert!(!recent.parse_failed);
        assert_eq!(recent.decision, DreamDecision::NoUpdate);
        assert_eq!(recent.dream_run_id, "run-nu");
        assert!(!pending_current_path(&root, "noupd-emp").is_file());
    }

    #[test]
    fn process_update_required_saves_pending() {
        let (_tmp, root) = setup();
        create_employee(&root, "upd-emp");

        let recent = process_result(&root, update_required_result("run-ur", "upd-emp"));

        assert!(!recent.parse_failed);
        assert_eq!(recent.decision, DreamDecision::UpdateRequired);
        let pending_path = pending_current_path(&root, "upd-emp");
        assert!(pending_path.is_file());
        let pending_raw = fs::read_to_string(&pending_path).unwrap();
        let pending: PendingUpdateRequest = serde_json::from_str(&pending_raw).unwrap();
        assert_eq!(pending.dream_run_id, "run-ur");
        assert_eq!(pending.result.updates.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn process_result_rejects_invalid_structured_payloads() {
        let (_tmp, root) = setup();
        create_employee(&root, "invalid-emp");

        let mut missing_summary = no_update_result("run-a", "invalid-emp");
        missing_summary.summary.clear();
        assert!(process_dream_result(&root, &missing_summary)
            .unwrap_err()
            .contains("summary"));

        let mut missing_sources = no_update_result("run-b", "invalid-emp");
        missing_sources.source_sessions.clear();
        assert!(process_dream_result(&root, &missing_sources)
            .unwrap_err()
            .contains("source_sessions"));

        let mut missing_updates = update_required_result("run-c", "invalid-emp");
        missing_updates.updates = Some(Vec::new());
        assert!(process_dream_result(&root, &missing_updates)
            .unwrap_err()
            .contains("updates"));

        assert!(!pending_current_path(&root, "invalid-emp").is_file());
    }

    #[test]
    fn new_update_replaces_old_pending() {
        let (_tmp, root) = setup();
        create_employee(&root, "replace-emp");

        process_result(&root, update_required_result("run-old", "replace-emp"));
        let p1: PendingUpdateRequest = serde_json::from_str(
            &fs::read_to_string(pending_current_path(&root, "replace-emp")).unwrap(),
        )
        .unwrap();
        assert_eq!(p1.dream_run_id, "run-old");

        process_result(&root, update_required_result("run-new", "replace-emp"));
        let p2: PendingUpdateRequest = serde_json::from_str(
            &fs::read_to_string(pending_current_path(&root, "replace-emp")).unwrap(),
        )
        .unwrap();
        assert_eq!(p2.dream_run_id, "run-new");
    }

    #[test]
    fn no_update_after_update_clears_pending() {
        let (_tmp, root) = setup();
        create_employee(&root, "clear-emp");

        process_result(&root, update_required_result("run-upd", "clear-emp"));
        assert!(pending_current_path(&root, "clear-emp").is_file());

        process_result(&root, no_update_result("run-clr", "clear-emp"));
        assert!(!pending_current_path(&root, "clear-emp").is_file());
    }

    #[test]
    fn read_recent_returns_none_for_missing() {
        let (_tmp, root) = setup();
        assert!(read_recent_dream_result(&root, "nonexistent").is_none());
    }

    #[test]
    fn first_round_does_not_write_employee_prompt() {
        let (_tmp, root) = setup();
        create_employee(&root, "safe-emp");

        let prompt_before = emp::read_employee_prompt(&root, "safe-emp").unwrap();
        process_result(&root, update_required_result("run-safe", "safe-emp"));
        let prompt_after = emp::read_employee_prompt(&root, "safe-emp").unwrap();

        assert_eq!(
            prompt_before, prompt_after,
            "Phase 1 must not modify employee prompt"
        );
    }

    // ── Request lifecycle tests ──────────────────────────────────────────

    #[test]
    fn reject_clears_pending_and_preserves_prompt() {
        let (_tmp, root) = setup();
        create_employee(&root, "rej-emp");

        let prompt_path = root.employees_dir().join("rej-emp/prompt.md");
        fs::write(&prompt_path, "Original prompt").unwrap();

        process_result(&root, update_required_result("run-rej", "rej-emp"));
        assert!(pending_current_path(&root, "rej-emp").is_file());

        reject_pending_request(&root, "rej-emp").unwrap();

        assert!(!pending_current_path(&root, "rej-emp").is_file());
        let rejected_path = request_status_dir(&root, "rej-emp", "rejected").join("current.json");
        assert!(rejected_path.is_file());
        let prompt_after = fs::read_to_string(&prompt_path).unwrap();
        assert_eq!(prompt_after, "Original prompt");
    }

    #[test]
    fn phase2_candidate_apply_writes_prompt_and_moves_to_applied() {
        let (_tmp, root) = setup();
        create_employee(&root, "app-emp");

        let prompt_path = root.employees_dir().join("app-emp/prompt.md");
        fs::write(&prompt_path, "Base prompt").unwrap();
        process_result(&root, update_required_result("run-app", "app-emp"));

        let candidate = runtime_prompt_candidate("Base prompt\n\nAlways greet politely.");
        let result = approve_with_runtime_candidate(&root, "app-emp", &candidate).unwrap();
        assert!(result.success);
        assert_eq!(result.dream_run_id, "run-app");
        assert!(result.error.is_none());

        assert!(!pending_current_path(&root, "app-emp").is_file());
        let applied_path = request_status_dir(&root, "app-emp", "applied").join("current.json");
        assert!(applied_path.is_file());
        assert_eq!(fs::read_to_string(&prompt_path).unwrap(), candidate);
    }

    #[test]
    fn phase2_candidate_apply_does_not_preview_long_unicode_prompt() {
        let (_tmp, root) = setup();
        create_employee(&root, "unicode-emp");

        let prompt_path = root.employees_dir().join("unicode-emp/prompt.md");
        fs::write(&prompt_path, "Base prompt").unwrap();
        process_result(&root, update_required_result("run-unicode", "unicode-emp"));

        let candidate = runtime_prompt_candidate(&"这是一段中文提示词。".repeat(80));
        let result = approve_with_runtime_candidate(&root, "unicode-emp", &candidate).unwrap();

        assert!(result.success);
        let serialized = serde_json::to_value(&result).expect("serialize result");
        assert!(serialized.get("new_prompt_preview").is_none());
        assert_eq!(fs::read_to_string(&prompt_path).unwrap(), candidate);
        assert!(request_status_dir(&root, "unicode-emp", "applied")
            .join("current.json")
            .is_file());
    }

    #[test]
    fn move_request_to_approved_when_no_pending_returns_error() {
        let (_tmp, root) = setup();
        create_employee(&root, "nopend-emp");
        let err = move_request_to_approved(&root, "nopend-emp").unwrap_err();
        assert!(err.contains("pending"));
    }

    #[test]
    fn reject_when_no_pending_returns_error() {
        let (_tmp, root) = setup();
        create_employee(&root, "norej-emp");
        let err = reject_pending_request(&root, "norej-emp").unwrap_err();
        assert!(err.contains("pending"));
    }

    #[test]
    fn read_pending_request_works() {
        let (_tmp, root) = setup();
        create_employee(&root, "readpend-emp");

        assert!(read_pending_request(&root, "readpend-emp").is_none());

        process_result(&root, update_required_result("run-rp", "readpend-emp"));

        let pending = read_pending_request(&root, "readpend-emp").expect("should have pending");
        assert_eq!(pending.dream_run_id, "run-rp");
        assert_eq!(pending.target_employee_id, "readpend-emp");
    }

    #[test]
    fn list_pending_request_employee_ids() {
        let (_tmp, root) = setup();
        create_employee(&root, "badge-a");
        create_employee(&root, "badge-b");

        assert!(super::list_employees_with_pending_requests(&root)
            .expect("list")
            .is_empty());

        process_result(&root, update_required_result("run-badge", "badge-a"));

        let pending = super::list_employees_with_pending_requests(&root).expect("list");
        assert_eq!(pending, vec!["badge-a".to_string()]);
    }

    #[test]
    fn new_phase2_candidate_after_old_applied_works() {
        let (_tmp, root) = setup();
        create_employee(&root, "multi-emp");

        let prompt_path = root.employees_dir().join("multi-emp/prompt.md");
        fs::write(&prompt_path, "V1").unwrap();

        process_result(&root, update_required_result("run-v2", "multi-emp"));
        let candidate_v2 = runtime_prompt_candidate("V1\n\nAlways greet politely.");
        let r1 = approve_with_runtime_candidate(&root, "multi-emp", &candidate_v2).unwrap();
        assert!(r1.success);
        assert_eq!(fs::read_to_string(&prompt_path).unwrap(), candidate_v2);

        process_result(
            &root,
            update_required_with_updates(
                "run-v3",
                "multi-emp",
                vec![PromptUpdate {
                    section: "Tone".to_string(),
                    action: "add".to_string(),
                    content: "Be friendly.".to_string(),
                    reason: "User feedback".to_string(),
                }],
            ),
        );
        let candidate_v3 = runtime_prompt_candidate("V1\n\nAlways greet politely.\n\nBe friendly.");
        let r2 = approve_with_runtime_candidate(&root, "multi-emp", &candidate_v3).unwrap();
        assert!(r2.success);
        assert_eq!(fs::read_to_string(&prompt_path).unwrap(), candidate_v3);
    }

    // ── Recovery tests ─────────────────────────────────────────────────

    #[test]
    fn read_active_review_request_syncs_status_from_directory() {
        let (_tmp, root) = setup();
        create_employee(&root, "status-sync-emp");
        process_result(&root, update_required_result("run-sync", "status-sync-emp"));

        move_request_to_status(&root, "status-sync-emp", "pending", "approved").unwrap();

        let req = read_active_review_request(&root, "status-sync-emp").expect("active request");
        assert_eq!(req.result.status, "approved");

        let approved_json = fs::read_to_string(
            request_status_dir(&root, "status-sync-emp", "approved").join("current.json"),
        )
        .unwrap();
        assert!(approved_json.contains("\"status\": \"approved\""));
    }

    #[test]
    fn move_request_to_status_updates_result_status_field() {
        let (_tmp, root) = setup();
        create_employee(&root, "status-emp");
        process_result(&root, update_required_result("run-st", "status-emp"));

        let req = move_request_to_status(&root, "status-emp", "pending", "approved").unwrap();
        assert_eq!(req.result.status, "approved");

        let approved_json = fs::read_to_string(
            request_status_dir(&root, "status-emp", "approved").join("current.json"),
        )
        .unwrap();
        assert!(approved_json.contains("\"status\": \"approved\""));

        move_request_to_status(&root, "status-emp", "approved", "failed").unwrap();
        let failed_json = fs::read_to_string(
            request_status_dir(&root, "status-emp", "failed").join("current.json"),
        )
        .unwrap();
        assert!(failed_json.contains("\"status\": \"failed\""));
        assert!(!request_status_dir(&root, "status-emp", "approved")
            .join("current.json")
            .is_file());
    }

    #[test]
    fn stranded_applying_recovered_on_next_approve() {
        let (_tmp, root) = setup();
        create_employee(&root, "strand-emp");

        let applying_dir = request_status_dir(&root, "strand-emp", "applying");
        fs::create_dir_all(&applying_dir).unwrap();
        fs::write(applying_dir.join("current.json"), r#"{"dream_run_id":"old","target_employee_id":"strand-emp","created_at":"t","result":{"decision":"update_required","target_employee_id":"strand-emp","dream_run_id":"old","summary":"s","source_sessions":[{"workspace_id":"w","session_id":"s"}],"updates":[{"section":"x","action":"add","content":"c","reason":"r"}]}}"#).unwrap();

        process_result(&root, update_required_result("run-new", "strand-emp"));
        let candidate = runtime_prompt_candidate("Recovered from stranded applying request.");
        let result = approve_with_runtime_candidate(&root, "strand-emp", &candidate).unwrap();
        assert!(result.success);
        assert!(
            !applying_dir.join("current.json").is_file(),
            "applying should be cleaned up"
        );
    }

    #[test]
    fn apply_prompt_from_runtime_rejects_invalid_candidates_without_writing() {
        let (_tmp, root) = setup();
        create_employee(&root, "runtime-emp");
        let prompt_path = root.employees_dir().join("runtime-emp/prompt.md");
        let original = "# Runtime Employee\n\nKeep this prompt intact.\n";
        fs::write(&prompt_path, original).expect("write prompt");

        for bad in [
            "",
            "{\"prompt\":\"# Wrapped\"}",
            "```markdown\n# Wrapped\n```",
            "# X",
        ] {
            let err = apply_prompt_from_runtime(&root, "runtime-emp", "run-1", bad)
                .expect_err("candidate should be rejected");
            assert!(
                err.contains("promptCandidate") || err.contains("markdown"),
                "unexpected error: {err}"
            );
            assert_eq!(
                fs::read_to_string(&prompt_path).expect("read prompt"),
                original,
                "invalid candidate must not modify prompt.md"
            );
        }
    }

    #[test]
    fn apply_prompt_from_runtime_rejects_dream_employee() {
        let (_tmp, root) = setup();
        let err = apply_prompt_from_runtime(
            &root,
            DREAM_EMPLOYEE_ID,
            "run-1",
            "# Dream\n\nThis should not be written.\n",
        )
        .expect_err("__dream__ must be rejected");
        assert!(err.contains("__dream__"));
    }

    /// MVP §3.4 / §9: end-to-end employee → bind → dream → approve flow (backend).
    #[test]
    fn mvp_e2e_employee_dream_happy_path() {
        let (tmp, root) = setup();
        assert!(
            emp::list(&root)
                .expect("list")
                .iter()
                .any(|e| e.id == DREAM_EMPLOYEE_ID),
            "root init must include __dream__"
        );

        create_employee(&root, "e2e-emp");
        let prompt_path = root.employees_dir().join("e2e-emp/prompt.md");
        fs::write(&prompt_path, "E2E baseline prompt v1").expect("write prompt");

        let ws_path = create_workspace_with_sessions(&tmp, &root, "e2e-emp", "e2e-ws", 3);
        let validation = emp::validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(validation.status, emp::BindingStatus::Bound);

        let prepare = prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: "e2e-emp".to_string(),
                workspace_filter: None,
            },
        )
        .expect("prepare dream");
        assert_eq!(prepare.selected_sessions.len(), 3);

        process_result(&root, no_update_result("e2e-run-a", "e2e-emp"));
        assert!(!pending_current_path(&root, "e2e-emp").is_file());

        process_result(&root, update_required_result("e2e-run-b", "e2e-emp"));
        assert!(pending_current_path(&root, "e2e-emp").is_file());

        reject_pending_request(&root, "e2e-emp").expect("reject");
        assert_eq!(
            fs::read_to_string(&prompt_path).expect("read prompt"),
            "E2E baseline prompt v1"
        );

        process_result(&root, update_required_result("e2e-run-c", "e2e-emp"));
        let candidate =
            runtime_prompt_candidate("E2E baseline prompt v1\n\nAlways greet politely.");
        let apply = approve_with_runtime_candidate(&root, "e2e-emp", &candidate).expect("approve");
        assert!(apply.success);
        let prompt_v2 = fs::read_to_string(&prompt_path).expect("read prompt v2");
        assert!(prompt_v2.contains("E2E baseline prompt v1"));
        assert!(prompt_v2.contains("Always greet politely"));
        assert_ne!(prompt_v2, "E2E baseline prompt v1");

        let applied_path = request_status_dir(&root, "e2e-emp", "applied").join("current.json");
        assert!(applied_path.is_file());

        let unbound =
            emp::validate_binding(&root, &tmp.path().join("lonely-ws")).expect("validate");
        assert_eq!(unbound.status, emp::BindingStatus::Unbound);

        assert!(!should_run_dream(&root, DREAM_EMPLOYEE_ID));
        let due = scan_due_employees(&root);
        assert!(!due.contains(&DREAM_EMPLOYEE_ID.to_string()));
    }

    #[test]
    fn scan_due_never_includes_dream_workflow() {
        let (_tmp, root) = setup();
        assert!(!scan_due_employees(&root).contains(&DREAM_EMPLOYEE_ID.to_string()));
    }

    #[test]
    fn dream_defaults_default_to_nine_am() {
        let (_tmp, root) = setup();

        let defaults = read_dream_defaults(&root);

        assert_eq!(defaults.default_dream_time, "09:00");
    }

    #[test]
    fn dream_defaults_preserve_default_time() {
        let (_tmp, root) = setup();
        write_dream_defaults(
            &root,
            &DreamDefaults {
                default_dream_time: "10:30".to_string(),
            },
        )
        .expect("write defaults");

        let defaults = read_dream_defaults(&root);

        assert_eq!(defaults.default_dream_time, "10:30");
    }

    #[test]
    fn dream_defaults_normalize_empty_default_time() {
        let (_tmp, root) = setup();
        write_dream_defaults(
            &root,
            &DreamDefaults {
                default_dream_time: "".to_string(),
            },
        )
        .expect("write defaults");

        let defaults = read_dream_defaults(&root);

        assert_eq!(defaults.default_dream_time, "09:00");
    }
}
