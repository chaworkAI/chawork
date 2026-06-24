//! 全局 provider 配置（DESIGN §4.3 / §5.1）。
//!
//! - 全局：`<root>/runtime/provider.json`，首次使用必须配置。
//! - Workspace provider credential 不参与 runtime auth；旧文件只作为遗留状态读取。

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{from_str as json_from_str, json, Value};

use crate::services::root_workspace::RootWorkspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderMode {
    #[default]
    InheritGlobal,
}

impl ProviderMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderMode::InheritGlobal => "inherit_global",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalProvider {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub openai_base_url: String,
    #[serde(default)]
    pub openai_api_key: String,
}

impl GlobalProvider {
    pub fn is_configured(&self) -> bool {
        !self.model.trim().is_empty()
            && !self.openai_base_url.trim().is_empty()
            && !self.openai_api_key.trim().is_empty()
    }
}

pub fn read_global(root: &RootWorkspace) -> GlobalProvider {
    let p = root.provider_path();
    if !p.is_file() {
        return GlobalProvider::default();
    }
    let Ok(raw) = fs::read_to_string(&p) else {
        return GlobalProvider::default();
    };
    json_from_str::<GlobalProvider>(&raw).unwrap_or_default()
}

/// 写全局 provider.json 的单个字段。空字符串删除字段。其他字段保留。
pub fn write_global_field(root: &RootWorkspace, field: &str, value: &str) -> Result<(), String> {
    let p = root.provider_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 runtime 目录失败: {e}"))?;
    }
    let mut data: Value = if p.is_file() {
        let raw =
            fs::read_to_string(&p).map_err(|e| format!("读取 global provider.json 失败: {e}"))?;
        json_from_str(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    if !data.is_object() {
        data = json!({});
    }
    let obj = data
        .as_object_mut()
        .ok_or_else(|| "global provider.json 根类型须为对象".to_string())?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        obj.remove(field);
    } else {
        obj.insert(field.to_string(), Value::String(trimmed.to_string()));
    }
    let out = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(&p, out).map_err(|e| format!("写入 global provider.json 失败: {e}"))
}

#[derive(Debug, Clone)]
pub struct EffectiveProvider {
    pub model: String,
    pub instructions: String,
    pub openai_base_url: String,
    pub openai_api_key: String,
    pub origin: ProviderMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderResolveError {
    GlobalNotConfigured,
}

impl ProviderResolveError {
    pub fn kind(self) -> &'static str {
        match self {
            ProviderResolveError::GlobalNotConfigured => "global_not_configured",
        }
    }
}

impl std::fmt::Display for ProviderResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GlobalNotConfigured => {
                write!(
                    f,
                    "还没配置 AI 模型，请先在设置中填写模型、接口地址和 API Key"
                )
            }
        }
    }
}

impl std::error::Error for ProviderResolveError {}

/// 根据当前 workspace 模式解析有效 provider。
pub fn effective_provider(
    root: &RootWorkspace,
    _workspace_path: &Path,
) -> Result<EffectiveProvider, ProviderResolveError> {
    let g = read_global(root);
    if !g.is_configured() {
        return Err(ProviderResolveError::GlobalNotConfigured);
    }
    Ok(EffectiveProvider {
        model: g.model.trim().to_string(),
        instructions: g.instructions.trim().to_string(),
        openai_base_url: g.openai_base_url.trim().to_string(),
        openai_api_key: g.openai_api_key.trim().to_string(),
        origin: ProviderMode::InheritGlobal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root(tmp: &tempfile::TempDir) -> RootWorkspace {
        crate::services::root_workspace::init_or_open(tmp.path()).expect("init root")
    }

    #[test]
    fn global_provider_round_trip_requires_model_base_url_and_key() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(&tmp);
        write_global_field(&root, "model", "gpt-4").unwrap();
        write_global_field(&root, "openai_base_url", "https://api.example.com/v1").unwrap();
        let g = read_global(&root);
        assert_eq!(g.model, "gpt-4");
        assert_eq!(g.openai_base_url, "https://api.example.com/v1");
        assert!(
            !g.is_configured(),
            "model and base URL alone must not enable provider sends"
        );

        write_global_field(&root, "openai_api_key", "sk-test").unwrap();
        let g = read_global(&root);
        assert!(g.is_configured());
    }

    #[test]
    fn unconfigured_global_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(&tmp);
        let ws = tmp.path().join("workspace-a");
        fs::create_dir_all(&ws).unwrap();

        let err = effective_provider(&root, &ws).unwrap_err();
        assert_eq!(err, ProviderResolveError::GlobalNotConfigured);
    }

    #[test]
    fn legacy_workspace_provider_file_does_not_enable_runtime_auth() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(&tmp);
        let ws = tmp.path().join("workspace-a");
        fs::create_dir_all(ws.join(".chawork/runtime")).unwrap();

        fs::write(
            ws.join(".chawork/runtime/provider.json"),
            br#"{"provider_mode":"workspace_override"}"#,
        )
        .unwrap();
        let err = effective_provider(&root, &ws).unwrap_err();
        assert_eq!(err, ProviderResolveError::GlobalNotConfigured);

        write_global_field(&root, "model", "gpt-4").unwrap();
        write_global_field(&root, "openai_base_url", "https://api.example.com/v1").unwrap();
        write_global_field(&root, "openai_api_key", "sk-test").unwrap();
        let eff = effective_provider(&root, &ws).expect("resolve");
        assert_eq!(eff.model, "gpt-4");
        assert_eq!(eff.origin, ProviderMode::InheritGlobal);
    }

    #[test]
    fn inherit_global_uses_global_provider() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(&tmp);
        write_global_field(&root, "model", "gpt-4").unwrap();
        write_global_field(&root, "openai_base_url", "https://api.example.com/v1").unwrap();
        write_global_field(&root, "openai_api_key", "sk-test").unwrap();
        let ws = tmp.path().join("workspace-a");
        fs::create_dir_all(&ws).unwrap();

        let eff = effective_provider(&root, &ws).expect("resolve");
        assert_eq!(eff.model, "gpt-4");
        assert_eq!(eff.origin, ProviderMode::InheritGlobal);
    }

    #[test]
    fn effective_provider_ignores_workspace_override_credentials() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(&tmp);
        write_global_field(&root, "model", "gpt-4").unwrap();
        write_global_field(&root, "openai_base_url", "https://global.example/v1").unwrap();
        write_global_field(&root, "openai_api_key", "global-key").unwrap();
        let ws = tmp.path().join("workspace-a");
        fs::create_dir_all(ws.join(".chawork/runtime")).unwrap();
        fs::write(
            ws.join(".chawork/runtime/provider.json"),
            br#"{
              "provider_mode": "workspace_override",
              "model": "claude-haiku",
              "openai_base_url": "https://workspace.example/v1",
              "openai_api_key": "workspace-key"
            }"#,
        )
        .unwrap();

        let eff = effective_provider(&root, &ws).expect("resolve");
        assert_eq!(eff.model, "gpt-4");
        assert_eq!(eff.openai_base_url, "https://global.example/v1");
        assert_eq!(eff.openai_api_key, "global-key");
        assert_eq!(eff.origin, ProviderMode::InheritGlobal);
    }
}
