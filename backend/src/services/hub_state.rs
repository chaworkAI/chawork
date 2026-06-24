use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const HUB_ORIGIN_FILE: &str = ".hub_origin.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HubOriginKind {
    Skill,
    Employee,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubOrigin {
    pub kind: HubOriginKind,
    pub hub_url: String,
    pub hub_id: String,
    pub local_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    pub hub_updated_at: String,
    pub installed_at: String,
    #[serde(default)]
    pub skill_hub_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HubDownloadFilter {
    All,
    Remote,
    Local,
    UpdateAvailable,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HubLocalSource {
    Hub,
    Custom,
    OtherHub,
    OtherKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HubLocalState {
    pub downloaded: bool,
    pub update_available: bool,
    pub local_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_source: Option<HubLocalSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_source_detail: Option<String>,
    pub installed_at: Option<String>,
    pub local_hub_updated_at: Option<String>,
    pub remote_updated_at: String,
}

impl HubLocalState {
    pub fn new(remote_updated_at: impl Into<String>) -> Self {
        Self {
            downloaded: false,
            update_available: false,
            local_id: None,
            local_source: None,
            local_source_detail: None,
            installed_at: None,
            local_hub_updated_at: None,
            remote_updated_at: remote_updated_at.into(),
        }
    }

    #[cfg(test)]
    fn not_downloaded(self) -> Self {
        self
    }

    #[cfg(test)]
    fn downloaded(mut self, local_id: impl Into<String>) -> Self {
        self.downloaded = true;
        self.local_id = Some(local_id.into());
        self.installed_at = Some("2026-06-05T10:00:00Z".to_string());
        self.local_hub_updated_at = Some(self.remote_updated_at.clone());
        self
    }

    #[cfg(test)]
    fn downloaded_with_update(mut self, local_id: impl Into<String>) -> Self {
        self.downloaded = true;
        self.update_available = true;
        self.local_id = Some(local_id.into());
        self.installed_at = Some("2026-06-05T10:00:00Z".to_string());
        self.local_hub_updated_at = Some("2026-06-05T09:00:00Z".to_string());
        self
    }
}

pub fn origin_path(dir: &Path) -> std::path::PathBuf {
    dir.join(HUB_ORIGIN_FILE)
}

pub fn read_origin(dir: &Path) -> Result<Option<HubOrigin>, String> {
    let path = origin_path(dir);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取 Hub origin 失败 ({}): {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("解析 Hub origin 失败 ({}): {e}", path.display()))
}

pub fn write_origin(dir: &Path, origin: &HubOrigin) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("创建 Hub origin 目录失败: {e}"))?;
    let json =
        serde_json::to_string_pretty(origin).map_err(|e| format!("序列化 Hub origin 失败: {e}"))?;
    fs::write(origin_path(dir), json).map_err(|e| format!("写入 Hub origin 失败: {e}"))
}

pub fn is_update_available(remote_updated_at: &str, origin: Option<&HubOrigin>) -> bool {
    let Some(origin) = origin else {
        return false;
    };
    remote_updated_at > origin.hub_updated_at.as_str()
}

pub fn merge_local_state(remote_updated_at: &str, origin: Option<&HubOrigin>) -> HubLocalState {
    let mut state = HubLocalState::new(remote_updated_at);
    let Some(origin) = origin else {
        return state;
    };
    state.downloaded = true;
    state.update_available = is_update_available(remote_updated_at, Some(origin));
    state.local_id = Some(origin.local_id.clone());
    state.local_source = Some(HubLocalSource::Hub);
    state.installed_at = Some(origin.installed_at.clone());
    state.local_hub_updated_at = Some(origin.hub_updated_at.clone());
    state
}

pub fn filter_by_download_state(
    states: &[HubLocalState],
    filter: HubDownloadFilter,
) -> Vec<HubLocalState> {
    states
        .iter()
        .filter(|state| match filter {
            HubDownloadFilter::All => true,
            HubDownloadFilter::Remote => !state.downloaded && state.local_source.is_none(),
            HubDownloadFilter::Local => state.downloaded && !state.update_available,
            HubDownloadFilter::UpdateAvailable => state.update_available,
            HubDownloadFilter::Custom => !state.downloaded && state.local_source.is_some(),
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_available_when_remote_updated_at_is_newer() {
        let local = HubOrigin {
            kind: HubOriginKind::Skill,
            hub_url: "http://hub/api/v1".into(),
            hub_id: "repo--skills--pdf".into(),
            local_id: "pdf".into(),
            content_hash: Some("old".into()),
            hub_updated_at: "2026-06-05T09:00:00Z".into(),
            installed_at: "2026-06-05T10:00:00Z".into(),
            skill_hub_ids: vec![],
        };

        assert!(is_update_available("2026-06-06T09:00:00Z", Some(&local)));
        assert!(!is_update_available("2026-06-04T09:00:00Z", Some(&local)));
        assert!(!is_update_available("2026-06-06T09:00:00Z", None));
    }

    #[test]
    fn filter_download_state_keeps_expected_items() {
        let items = vec![
            HubLocalState::new("2026-06-05T09:00:00Z").not_downloaded(),
            HubLocalState::new("2026-06-05T09:00:00Z").downloaded("pdf"),
            HubLocalState::new("2026-06-06T09:00:00Z").downloaded_with_update("docx"),
        ];

        assert_eq!(
            filter_by_download_state(&items, HubDownloadFilter::Remote).len(),
            1
        );
        assert_eq!(
            filter_by_download_state(&items, HubDownloadFilter::Local).len(),
            1
        );
        assert_eq!(
            filter_by_download_state(&items, HubDownloadFilter::UpdateAvailable).len(),
            1
        );
        assert_eq!(
            filter_by_download_state(&items, HubDownloadFilter::All).len(),
            3
        );
    }
}
