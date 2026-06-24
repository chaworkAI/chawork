//! Workspace 对根工作区 MCP server 的工具启用/禁用策略（DESIGN §4.3）。
//!
//! 保存在 `<workspace>/.chawork/mcp-tools.json`。
//! 由 Runtime context builder 在启动 Codex 子进程前应用：
//! 优先在 config 层不暴露已关闭的工具，而非调用时再拒绝。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolAction {
    Enabled,
    Disabled,
}

impl Default for ToolAction {
    fn default() -> Self {
        ToolAction::Enabled
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPolicy {
    #[serde(default)]
    pub default_action: ToolAction,
    #[serde(default)]
    pub overrides: BTreeMap<String, ToolAction>,
}

impl ToolPolicy {
    pub fn is_enabled(&self, tool: &str) -> bool {
        self.overrides
            .get(tool)
            .copied()
            .unwrap_or(self.default_action)
            == ToolAction::Enabled
    }
}

fn policy_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(".chawork").join("mcp-tools.json")
}

pub fn load(workspace_path: &Path) -> ToolPolicy {
    let p = policy_path(workspace_path);
    if !p.is_file() {
        return ToolPolicy::default();
    }
    let Ok(raw) = fs::read_to_string(&p) else {
        return ToolPolicy::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(workspace_path: &Path, policy: &ToolPolicy) -> Result<(), String> {
    let p = policy_path(workspace_path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(policy).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_action_applies_when_no_override() {
        let p = ToolPolicy::default();
        assert!(p.is_enabled("anything"));
    }

    #[test]
    fn override_takes_precedence_over_default() {
        let mut p = ToolPolicy {
            default_action: ToolAction::Disabled,
            ..Default::default()
        };
        p.overrides
            .insert("write_file".to_string(), ToolAction::Enabled);
        assert!(p.is_enabled("write_file"));
        assert!(!p.is_enabled("read_file"));
    }

    #[test]
    fn round_trips_through_disk() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork")).unwrap();

        let mut p = ToolPolicy::default();
        p.overrides
            .insert("search_text".to_string(), ToolAction::Disabled);
        save(ws, &p).expect("save policy");

        let loaded = load(ws);
        assert_eq!(loaded.default_action, ToolAction::Enabled);
        assert_eq!(
            loaded.overrides.get("search_text").copied(),
            Some(ToolAction::Disabled)
        );
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let tmp = tempfile::tempdir().expect("tmp");
        let p = load(tmp.path());
        assert!(p.is_enabled("anything"));
    }
}
