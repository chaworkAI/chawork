//! 全局设置 Tauri 命令（DESIGN §10.5 全局设置）。
//!
//! 操作根工作区下的全局 provider 配置和根目录信息。

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::runtime::lifecycle::{
    invalidate_all_chat_runtimes, MutationWithRuntimeInvalidation, RuntimeInvalidationReason,
};
use crate::services::{global_provider, provider as provider_svc, ui_locale};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ProviderConfigInput {
    pub provider: String,
    pub model: String,
    pub openai_base_url: String,
    pub openai_api_key: String,
    pub instructions: String,
}

fn input_to_config(input: ProviderConfigInput) -> provider_svc::ProviderConfig {
    provider_svc::ProviderConfig {
        provider: input.provider,
        model: input.model,
        openai_base_url: input.openai_base_url,
        openai_api_key: input.openai_api_key,
        instructions: input.instructions,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiLocalePayload {
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiPreferencesPayload {
    pub onboarding_tour_completed: bool,
}

fn ui_preferences_path(
    root: &crate::services::root_workspace::RootWorkspace,
) -> std::path::PathBuf {
    root.state_dir().join("ui-preferences.json")
}

fn read_ui_preferences_from_root(
    root: &crate::services::root_workspace::RootWorkspace,
) -> UiPreferencesPayload {
    let path = ui_preferences_path(root);
    if !path.is_file() {
        return UiPreferencesPayload::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<UiPreferencesPayload>(&raw).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_ui_preferences(
    app_state: State<'_, Arc<AppState>>,
) -> Result<UiPreferencesPayload, String> {
    Ok(read_ui_preferences_from_root(&app_state.root))
}

#[tauri::command]
pub fn set_ui_preferences(
    app_state: State<'_, Arc<AppState>>,
    preferences: UiPreferencesPayload,
) -> Result<UiPreferencesPayload, String> {
    let path = ui_preferences_path(&app_state.root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 UI preferences 目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(&preferences)
        .map_err(|e| format!("序列化 UI preferences 失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入 UI preferences 失败: {e}"))?;
    Ok(preferences)
}

#[tauri::command]
pub fn get_ui_locale(app_state: State<'_, Arc<AppState>>) -> Result<UiLocalePayload, String> {
    let locale = ui_locale::read_ui_locale(&app_state.root);
    Ok(UiLocalePayload { locale })
}

#[tauri::command]
pub fn set_ui_locale(
    app_state: State<'_, Arc<AppState>>,
    locale: String,
) -> Result<UiLocalePayload, String> {
    let locale = ui_locale::write_ui_locale(&app_state.root, &locale)?;
    let payload = UiLocalePayload { locale };
    Ok(payload)
}

#[derive(Serialize, Clone)]
pub struct GlobalProviderPayload {
    pub configured: bool,
    pub model: String,
    pub instructions: String,
    pub openai_base_url: String,
    pub openai_api_key: String,
}

#[tauri::command]
pub fn get_global_provider(
    app_state: State<'_, Arc<AppState>>,
) -> Result<GlobalProviderPayload, String> {
    let g = global_provider::read_global(&app_state.root);
    Ok(GlobalProviderPayload {
        configured: g.is_configured(),
        model: g.model,
        instructions: g.instructions,
        openai_base_url: g.openai_base_url,
        openai_api_key: g.openai_api_key,
    })
}

#[tauri::command]
pub async fn set_global_provider_model(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    model: String,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    global_provider::write_global_field(&app_state.root, "model", &model)?;
    let runtime_invalidation =
        invalidate_all_chat_runtimes(&app_state, &app, RuntimeInvalidationReason::ProviderChanged)
            .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn set_global_provider_instructions(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    instructions: String,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    global_provider::write_global_field(&app_state.root, "instructions", &instructions)?;
    let runtime_invalidation =
        invalidate_all_chat_runtimes(&app_state, &app, RuntimeInvalidationReason::ProviderChanged)
            .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn set_global_provider_connection(
    app: AppHandle,
    app_state: State<'_, Arc<AppState>>,
    openai_base_url: String,
    openai_api_key: String,
) -> Result<MutationWithRuntimeInvalidation<()>, String> {
    global_provider::write_global_field(&app_state.root, "openai_base_url", &openai_base_url)?;
    global_provider::write_global_field(&app_state.root, "openai_api_key", &openai_api_key)?;
    let runtime_invalidation =
        invalidate_all_chat_runtimes(&app_state, &app, RuntimeInvalidationReason::ProviderChanged)
            .await;
    Ok(MutationWithRuntimeInvalidation::success(
        (),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub fn is_global_provider_configured(app_state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    Ok(global_provider::read_global(&app_state.root).is_configured())
}

#[derive(Serialize, Clone)]
pub struct RootWorkspaceInfoPayload {
    pub path: String,
    pub codex_home: String,
    pub provider_path: String,
    pub skills_dir: String,
    pub templates_dir: String,
    pub mcp_dir: String,
}

/// Opens the root workspace `runtime/provider.json` in the system file manager.
#[tauri::command]
pub fn reveal_global_provider_config(app_state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let full = app_state.root.provider_path();
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 runtime 目录失败: {e}"))?;
    }
    if !full.is_file() {
        fs::write(&full, b"{\n  \"model\": \"\"\n}\n")
            .map_err(|e| format!("创建 global provider.json 失败: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        let st = std::process::Command::new("open")
            .arg("-R")
            .arg(&full)
            .status()
            .map_err(|e| format!("无法打开访达: {e}"))?;
        if !st.success() {
            return Err("访达未能打开该路径".to_string());
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let p = full.to_string_lossy().replace('/', "\\");
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(format!("/select,{p}"));
        crate::runtime::apply_backend_product_process_policy(
            &mut cmd,
            crate::runtime::SpawnOwner::BackendGuiOpen,
        );
        let st = cmd
            .status()
            .map_err(|e| format!("无法打开资源管理器: {e}"))?;
        if !st.success() && st.code() != Some(1) {
            return Err("资源管理器未能打开该路径".to_string());
        }
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let parent = full
            .parent()
            .ok_or_else(|| "无效路径".to_string())?
            .to_path_buf();
        let st = std::process::Command::new("xdg-open")
            .arg(&parent)
            .status()
            .map_err(|e| format!("无法打开文件管理器: {e}"))?;
        if !st.success() {
            return Err("文件管理器未能打开该路径".to_string());
        }
        Ok(())
    }
}

#[tauri::command]
pub fn get_root_workspace_info(
    app_state: State<'_, Arc<AppState>>,
) -> Result<RootWorkspaceInfoPayload, String> {
    let r = &app_state.root;
    Ok(RootWorkspaceInfoPayload {
        path: r.path().to_string_lossy().into_owned(),
        codex_home: r.codex_home_dir().to_string_lossy().into_owned(),
        provider_path: r.provider_path().to_string_lossy().into_owned(),
        skills_dir: r.skills_dir().to_string_lossy().into_owned(),
        templates_dir: r.templates_dir().to_string_lossy().into_owned(),
        mcp_dir: r.mcp_dir().to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub async fn set_global_provider(
    app: AppHandle,
    config: ProviderConfigInput,
    app_state: State<'_, Arc<AppState>>,
) -> Result<MutationWithRuntimeInvalidation<provider_svc::ProviderConfigView>, String> {
    let path = app_state.root.provider_path();
    let cfg = input_to_config(config);
    provider_svc::write_provider_json(&path, &cfg)?;
    let saved = provider_svc::read_provider_json(&path)?;
    let runtime_invalidation =
        invalidate_all_chat_runtimes(&app_state, &app, RuntimeInvalidationReason::ProviderChanged)
            .await;
    Ok(MutationWithRuntimeInvalidation::success(
        provider_svc::to_view(&saved),
        runtime_invalidation,
    ))
}

#[tauri::command]
pub async fn list_provider_models(
    config: Option<ProviderConfigInput>,
    app_state: State<'_, Arc<AppState>>,
) -> Result<provider_svc::ProviderModelListResult, String> {
    let (base_url, api_key) = if let Some(input) = config {
        let mut cfg = input_to_config(input);
        if cfg.openai_api_key.trim().is_empty() {
            let saved =
                provider_svc::read_provider_json(&app_state.root.provider_path())?.openai_api_key;
            cfg.openai_api_key = saved;
        }
        (cfg.openai_base_url, cfg.openai_api_key)
    } else {
        let cfg = provider_svc::read_provider_json(&app_state.root.provider_path())?;
        (cfg.openai_base_url, cfg.openai_api_key)
    };
    let key_ref = if api_key.trim().is_empty() {
        None
    } else {
        Some(api_key.as_str())
    };
    provider_svc::list_openai_compatible_models(&base_url, key_ref).await
}
