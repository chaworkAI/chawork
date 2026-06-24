//! Global UI locale service.
//!
//! The UI locale is the single language source for interface labels and Dream
//! natural-language output. Dream defaults must not duplicate this value.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::root_workspace::RootWorkspace;

const DEFAULT_UI_LOCALE: &str = "zh-CN";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UiLocaleFile {
    locale: String,
}

fn ui_locale_path(root: &RootWorkspace) -> PathBuf {
    root.config_dir().join("ui-locale.yaml")
}

pub fn normalize_ui_locale(locale: &str) -> String {
    match locale.trim() {
        "en-US" | "en" => "en-US".to_string(),
        _ => DEFAULT_UI_LOCALE.to_string(),
    }
}

pub fn read_ui_locale(root: &RootWorkspace) -> String {
    let path = ui_locale_path(root);
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_yaml::from_str::<UiLocaleFile>(&raw).ok())
        .map(|payload| normalize_ui_locale(&payload.locale))
        .unwrap_or_else(|| DEFAULT_UI_LOCALE.to_string())
}

pub fn write_ui_locale(root: &RootWorkspace, locale: &str) -> Result<String, String> {
    let locale = normalize_ui_locale(locale);
    let payload = UiLocaleFile {
        locale: locale.clone(),
    };
    let path = ui_locale_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 config 目录失败: {e}"))?;
    }
    let yaml =
        serde_yaml::to_string(&payload).map_err(|e| format!("序列化 ui-locale.yaml 失败: {e}"))?;
    fs::write(&path, yaml).map_err(|e| format!("写入 ui-locale.yaml 失败: {e}"))?;
    Ok(locale)
}
