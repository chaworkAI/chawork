use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_active_at: String,
    pub active_session_id: Option<String>,
    pub domain_pack_id: Option<String>,
    pub index_status: String,
    /// Draft proposals in `proposals/drafts/` (not persisted in workspace.json; filled when listing).
    #[serde(default)]
    pub pending_proposals_count: u32,
    /// Name of the bound employee, if any (not persisted in workspace.json; filled when listing/switching).
    #[serde(default)]
    pub bound_employee_name: Option<String>,
    /// ID of the bound employee, if any (not persisted in workspace.json; filled when listing/switching).
    #[serde(default)]
    pub bound_employee_id: Option<String>,
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn workspace_state_file(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".chawork/state/workspace.json")
}

fn touch_placeholder(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    drop(file);
    if path.extension().and_then(|s| s.to_str()) == Some("json") && fs::metadata(path)?.len() == 0 {
        fs::write(path, b"{}\n")?;
    }
    Ok(())
}

/// Create all standard workspace directories and placeholder files. Idempotent.
pub fn ensure_directories(path: &Path) -> Result<(), String> {
    let pairs = &[
        "raw/recordings",
        "raw/transcripts",
        "raw/uploads",
        "raw/images",
        "raw/notes",
        "wiki/objects",
        "wiki/reports",
        "wiki/concepts",
        "wiki/index.md",
        "wiki/log.md",
        "schema/domain.yaml",
        "schema/AGENTS.md",
        "schema/objects.yaml",
        "schema/workflows.yaml",
        "schema/iteration_log.md",
        "skills",
        "sessions",
        "proposals/drafts",
        "proposals/accepted",
        "proposals/rejected",
        "logs/runtime",
        "logs/import",
        "logs/operations",
        ".chawork/codex-home/config.toml",
        ".chawork/codex-home/skills",
        ".chawork/codex-home/mcp",
        ".chawork/codex-home/logs",
        ".chawork/runtime/env.json",
        ".chawork/runtime/provider.json",
        ".chawork/runtime/runtime-state.json",
        ".chawork/cache",
        ".chawork/state",
        ".qmd",
    ];

    for rel in pairs {
        let full = path.join(rel);
        if rel.ends_with(".md")
            || rel.ends_with(".yaml")
            || rel.ends_with(".toml")
            || rel.ends_with(".json")
        {
            touch_placeholder(&full).map_err(|e| e.to_string())?;
        } else {
            fs::create_dir_all(&full).map_err(|e| e.to_string())?;
        }
    }

    seed_default_domain_yaml_if_empty(path)?;

    Ok(())
}

/// Writes a minimal valid `schema/domain.yaml` when the placeholder file is still empty.
fn seed_default_domain_yaml_if_empty(workspace_path: &Path) -> Result<(), String> {
    let path = workspace_path.join("schema/domain.yaml");
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !raw.trim().is_empty() {
        return Ok(());
    }
    let display = derive_name(workspace_path);
    let id = display
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let id = if id.is_empty() {
        "workspace".to_string()
    } else {
        id
    };
    let body = format!(
        "id: {id}\nname: {display}\n",
        id = id,
        display = display.replace('\n', " ")
    );
    fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(())
}

fn derive_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Workspace")
        .to_string()
}

pub fn read_workspace(workspace_path: &Path) -> Result<WorkspaceState, String> {
    let canonical_path =
        fs::canonicalize(workspace_path).unwrap_or_else(|_| workspace_path.to_path_buf());
    read_workspace_disk(&canonical_path)
}

pub fn current_iso_timestamp() -> String {
    iso_now()
}

fn read_workspace_disk(path: &Path) -> Result<WorkspaceState, String> {
    let fpath = workspace_state_file(path);
    let mut buf = String::new();
    fs::File::open(&fpath)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&buf).map_err(|e| e.to_string())
}

fn write_workspace_disk(path: &Path, state: &WorkspaceState) -> Result<(), String> {
    ensure_directories(path)?;
    let fpath = workspace_state_file(path);
    if let Some(parent) = fpath.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut disk_state = state.clone();
    disk_state.bound_employee_name = None;
    disk_state.bound_employee_id = None;
    let json = serde_json::to_string_pretty(&disk_state).map_err(|e| e.to_string())?;
    fs::write(&fpath, json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn touch_last_active(ws: &mut WorkspaceState) {
    ws.last_active_at = iso_now();
}

/// Open existing workspace or bootstrap a new one at `path`. Updates `last_active_at` when opening existing.
pub fn open_or_create(path: &Path) -> Result<WorkspaceState, String> {
    ensure_directories(path)?;
    let fpath = workspace_state_file(path);
    let canonical_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical_path.to_string_lossy().into_owned();

    if fpath.exists() {
        let mut ws = read_workspace_disk(&canonical_path)?;
        touch_last_active(&mut ws);
        write_workspace_disk(&canonical_path, &ws)?;
        return Ok(ws);
    }

    let now = iso_now();
    let id = Uuid::new_v4().to_string();
    let name = derive_name(&canonical_path);
    let ws = WorkspaceState {
        id,
        name,
        path: path_str,
        created_at: now.clone(),
        last_active_at: now,
        active_session_id: None,
        domain_pack_id: None,
        index_status: "stale".to_string(),
        pending_proposals_count: 0,
        bound_employee_name: None,
        bound_employee_id: None,
    };
    write_workspace_disk(&canonical_path, &ws)?;
    Ok(ws)
}

pub fn set_active_session_id(
    workspace_path: &Path,
    active_session_id: Option<&str>,
) -> Result<(), String> {
    let canonical_path =
        fs::canonicalize(workspace_path).unwrap_or_else(|_| workspace_path.to_path_buf());
    let mut ws = read_workspace_disk(&canonical_path)?;
    ws.active_session_id = active_session_id.map(String::from);
    touch_last_active(&mut ws);
    write_workspace_disk(&canonical_path, &ws)?;
    Ok(())
}

pub fn list_known(path: &Path) -> Vec<WorkspaceState> {
    if !path.exists() {
        return Vec::new();
    }
    let mut buf = String::new();
    if fs::File::open(path)
        .and_then(|mut f| f.read_to_string(&mut buf))
        .is_err()
    {
        return Vec::new();
    }

    serde_json::from_str::<Vec<WorkspaceState>>(&buf).unwrap_or_else(|_| Vec::new())
}

/// Like [`list_known`] but fills `pending_proposals_count` and live `index_status` from disk (no write-back).
pub fn list_known_with_draft_counts(store_path: &Path) -> Vec<WorkspaceState> {
    let mut list = list_known(store_path);
    for ws in &mut list {
        let pb = Path::new(&ws.path);
        ws.pending_proposals_count = crate::services::proposal::count_draft_proposals(pb);
        if let Ok(canon) = fs::canonicalize(pb) {
            ws.index_status = crate::services::qmd_index::infer_index_status_string(&canon);
        }
    }
    list
}

/// Refresh `index_status` on disk from embedded QMD meta / building marker.
pub fn sync_workspace_index_status(workspace_path: &Path) -> Result<(), String> {
    let mut ws = read_workspace(workspace_path)?;
    ws.index_status = crate::services::qmd_index::infer_index_status_string(workspace_path);
    persist_workspace(workspace_path, &ws)
}

fn save_known_list(path: &Path, list: &[WorkspaceState]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    fs::write(path, json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn add_known(store_path: &Path, ws: &WorkspaceState) -> Result<(), String> {
    let mut list = list_known(store_path);
    let ws_key = normalize_path_key(&ws.path);
    let pos = list
        .iter()
        .position(|w| normalize_path_key(&w.path) == ws_key);
    match pos {
        Some(i) => list[i] = ws.clone(),
        None => list.push(ws.clone()),
    }
    save_known_list(store_path, &list)?;
    Ok(())
}

fn normalize_path_key(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    if normalized.starts_with("//?/") {
        normalized = normalized[4..].to_string();
    }
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        let mut chars = normalized.chars();
        if let Some(drive) = chars.next() {
            normalized = format!("{}{}", drive.to_ascii_lowercase(), chars.as_str());
        }
    }
    normalized.trim_end_matches('/').to_string()
}

pub fn persist_workspace(workspace_path: &Path, ws: &WorkspaceState) -> Result<(), String> {
    let canonical_path =
        fs::canonicalize(workspace_path).unwrap_or_else(|_| workspace_path.to_path_buf());
    write_workspace_disk(&canonical_path, ws)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_key_treats_extended_windows_paths_as_equal() {
        assert_eq!(
            normalize_path_key(r"\\?\D:\测试222"),
            normalize_path_key(r"D:\测试222")
        );
    }

    #[test]
    fn ensure_directories_seeds_json_placeholders_as_valid_objects() {
        let tmp = tempfile::tempdir().unwrap();
        ensure_directories(tmp.path()).unwrap();

        for rel in [
            ".chawork/runtime/env.json",
            ".chawork/runtime/provider.json",
            ".chawork/runtime/runtime-state.json",
        ] {
            let raw = fs::read_to_string(tmp.path().join(rel)).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(parsed, serde_json::json!({}));
        }
    }
}
