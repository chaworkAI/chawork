//! Workspace 配置 Tauri 命令（DESIGN §10.5 Workspace Config）。
//!
//! - effective provider 解析结果（供前端 Composer 决定是否禁用发送）
//! - tool policy（对根 MCP server 工具的启用/禁用）

use std::collections::BTreeMap;

use serde::Serialize;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::runtime::lifecycle::{
    invalidate_workspace_chat_runtime, MutationWithRuntimeInvalidation, RuntimeInvalidationReason,
};
use crate::services::{global_provider, tool_policy};
use crate::state::AppState;

#[derive(Serialize, Clone)]
pub struct EffectiveProviderPayload {
    pub configured: bool,
    /// "inherit_global" | "none"
    pub origin: String,
    pub model: String,
    /// `None` when `configured = true`. Otherwise:
    /// "no_workspace" | "global_not_configured"
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
}

#[tauri::command]
pub fn get_effective_provider(
    app_state: State<'_, Arc<AppState>>,
) -> Result<EffectiveProviderPayload, String> {
    let ws = match app_state.require_active_workspace() {
        Ok(p) => p,
        Err(msg) => {
            return Ok(EffectiveProviderPayload {
                configured: false,
                origin: "none".to_string(),
                model: String::new(),
                error_kind: Some("no_workspace".to_string()),
                error_message: Some(msg),
            });
        }
    };
    match global_provider::effective_provider(&app_state.root, &ws) {
        Ok(p) => Ok(EffectiveProviderPayload {
            configured: true,
            origin: p.origin.as_str().to_string(),
            model: p.model,
            error_kind: None,
            error_message: None,
        }),
        Err(e) => Ok(EffectiveProviderPayload {
            configured: false,
            origin: "none".to_string(),
            model: String::new(),
            error_kind: Some(e.kind().to_string()),
            error_message: Some(e.to_string()),
        }),
    }
}

#[derive(Serialize, Clone)]
pub struct ToolPolicyPayload {
    /// "enabled" | "disabled"
    pub default_action: String,
    pub overrides: BTreeMap<String, String>,
}

fn action_to_string(a: tool_policy::ToolAction) -> String {
    match a {
        tool_policy::ToolAction::Enabled => "enabled".to_string(),
        tool_policy::ToolAction::Disabled => "disabled".to_string(),
    }
}

fn string_to_action(s: &str) -> tool_policy::ToolAction {
    match s {
        "disabled" => tool_policy::ToolAction::Disabled,
        _ => tool_policy::ToolAction::Enabled,
    }
}

#[tauri::command]
pub fn get_tool_policy(app_state: State<'_, Arc<AppState>>) -> Result<ToolPolicyPayload, String> {
    let ws = app_state.require_active_workspace()?;
    let p = tool_policy::load(&ws);
    Ok(ToolPolicyPayload {
        default_action: action_to_string(p.default_action),
        overrides: p
            .overrides
            .iter()
            .map(|(k, v)| (k.clone(), action_to_string(*v)))
            .collect(),
    })
}

#[tauri::command]
pub async fn set_tool_policy(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    default_action: String,
    overrides: BTreeMap<String, String>,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    let ws = app_state.require_active_workspace()?;
    let policy = tool_policy::ToolPolicy {
        default_action: string_to_action(&default_action),
        overrides: overrides
            .into_iter()
            .map(|(k, v)| (k, string_to_action(&v)))
            .collect(),
    };
    tool_policy::save(&ws, &policy)?;
    let runtime_invalidation = invalidate_workspace_chat_runtime(
        &app_state,
        &app,
        ws,
        RuntimeInvalidationReason::McpContextChanged,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}
