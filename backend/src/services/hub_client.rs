use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub total: u32,
    pub page: u32,
    pub limit: u32,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionInfo {
    pub name: String,
    pub skill_count: u32,
    pub employee_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubManifest {
    pub total_skills: u32,
    pub total_employees: u32,
    pub professions: Vec<ProfessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSkill {
    pub id: String,
    pub name: String,
    pub description_zh: String,
    pub description_en: String,
    pub profession: String,
    pub content_hash: String,
    pub source: serde_json::Value,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSkillDetail {
    #[serde(flatten)]
    pub skill: HubSkill,
    pub skill_md: String,
    #[serde(default)]
    pub referenced_by_employees: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubEmployee {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub prompt_preview: String,
    pub skill_ids: Vec<String>,
    pub skill_count: u32,
    pub tags: Vec<String>,
    pub source: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubEmployeeSkillRef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description_zh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubEmployeeDetail {
    #[serde(flatten)]
    pub employee: HubEmployee,
    pub prompt_md: String,
    #[serde(default)]
    pub skills: Vec<HubEmployeeSkillRef>,
}

#[derive(Debug, Clone)]
pub struct HubConfig {
    pub base_url: String,
    pub json_timeout_secs: u64,
    pub bundle_timeout_secs: u64,
}

impl HubConfig {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            json_timeout_secs: 10,
            bundle_timeout_secs: 30,
        }
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}

#[derive(Debug, Clone, Default)]
pub struct HubListSkillsQuery {
    pub q: Option<String>,
    pub profession: Option<String>,
    pub filter: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct HubListEmployeesQuery {
    pub q: Option<String>,
    pub tags: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

fn client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("创建 Hub HTTP client 失败: {e}"))
}

async fn post_json<T, B>(cfg: &HubConfig, path: &str, body: &B) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
    B: Serialize,
{
    let url = cfg.endpoint(path);
    let timeout_secs = if path.contains("import/github") {
        120
    } else {
        cfg.json_timeout_secs
    };
    let response = client(timeout_secs)?
        .post(&url)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("请求 Hub 失败 ({url}): {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let body_preview = body.chars().take(240).collect::<String>();
        return Err(format!(
            "Hub 请求失败 ({url}): HTTP {status} {body_preview}"
        ));
    }
    response
        .json::<T>()
        .await
        .map_err(|e| format!("解析 Hub 响应失败 ({url}): {e}"))
}

async fn get_json<T: for<'de> Deserialize<'de>>(cfg: &HubConfig, path: &str) -> Result<T, String> {
    let url = cfg.endpoint(path);
    let response = client(cfg.json_timeout_secs)?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求 Hub 失败 ({url}): {e}"))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let body_preview = body.chars().take(240).collect::<String>();
        if content_type.contains("text/html")
            || body_preview.trim_start().starts_with("<!DOCTYPE html")
        {
            return Err(format!(
                "Hub API 未返回 JSON ({url}): HTTP {status}. 当前地址返回的是网页，请检查 Hub API 地址是否已部署到 /api/v1。"
            ));
        }
        return Err(format!(
            "Hub 请求失败 ({url}): HTTP {status} {body_preview}"
        ));
    }
    if !content_type.is_empty() && !content_type.contains("json") {
        return Err(format!(
            "Hub API 未返回 JSON ({url}): Content-Type {content_type}. 请检查 Hub API 地址。"
        ));
    }
    response
        .json::<T>()
        .await
        .map_err(|e| format!("解析 Hub 响应失败 ({url}): {e}"))
}

fn append_query(base: String, pairs: Vec<(&str, Option<String>)>) -> String {
    let mut url = reqwest::Url::parse(&base).expect("hub endpoint should be an absolute URL");
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            if let Some(value) = value {
                if !value.is_empty() {
                    query.append_pair(key, &value);
                }
            }
        }
    }
    url.to_string()
}

fn encode_path_segment(raw: &str) -> String {
    let mut encoded = String::new();
    for b in raw.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char)
            }
            _ => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    encoded
}

pub async fn get_manifest(cfg: &HubConfig) -> Result<HubManifest, String> {
    get_json(cfg, "/manifest").await
}

pub async fn list_professions(cfg: &HubConfig) -> Result<Vec<ProfessionInfo>, String> {
    get_json(cfg, "/professions").await
}

pub async fn list_skills(
    cfg: &HubConfig,
    query: HubListSkillsQuery,
) -> Result<PaginatedResponse<HubSkill>, String> {
    let url = append_query(
        cfg.endpoint("/skills"),
        vec![
            ("q", query.q),
            ("profession", query.profession),
            ("filter", query.filter),
            ("page", query.page.map(|v| v.to_string())),
            ("limit", query.limit.map(|v| v.to_string())),
        ],
    );
    get_json(cfg, url.strip_prefix(&cfg.base_url).unwrap_or(&url)).await
}

pub async fn get_skill_detail(cfg: &HubConfig, id: &str) -> Result<HubSkillDetail, String> {
    get_json(cfg, &format!("/skills/{}", encode_path_segment(id))).await
}

pub async fn list_employees(
    cfg: &HubConfig,
    query: HubListEmployeesQuery,
) -> Result<PaginatedResponse<HubEmployee>, String> {
    let url = append_query(
        cfg.endpoint("/employees"),
        vec![
            ("q", query.q),
            ("tags", query.tags),
            ("page", query.page.map(|v| v.to_string())),
            ("limit", query.limit.map(|v| v.to_string())),
        ],
    );
    get_json(cfg, url.strip_prefix(&cfg.base_url).unwrap_or(&url)).await
}

pub async fn get_employee_detail(cfg: &HubConfig, id: &str) -> Result<HubEmployeeDetail, String> {
    get_json(cfg, &format!("/employees/{}", encode_path_segment(id))).await
}

async fn download_bundle(cfg: &HubConfig, path: &str, dest: &Path) -> Result<(), String> {
    let url = cfg.endpoint(path);
    let response = client(cfg.bundle_timeout_secs)?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载 Hub bundle 失败 ({url}): {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "下载 Hub bundle 失败 ({url}): HTTP {status} {body}"
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取 Hub bundle 响应失败 ({url}): {e}"))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建 Hub bundle 目录失败 ({}): {e}", parent.display()))?;
    }
    std::fs::write(dest, bytes).map_err(|e| format!("写入 Hub bundle 失败: {e}"))
}

pub async fn download_skill_bundle(cfg: &HubConfig, id: &str, dest: &Path) -> Result<(), String> {
    download_bundle(
        cfg,
        &format!("/skills/{}/bundle", encode_path_segment(id)),
        dest,
    )
    .await
}

pub async fn download_employee_bundle(
    cfg: &HubConfig,
    id: &str,
    dest: &Path,
) -> Result<(), String> {
    download_bundle(
        cfg,
        &format!("/employees/{}/bundle", encode_path_segment(id)),
        dest,
    )
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubGithubImportSkillPreview {
    pub id: String,
    pub name: String,
    pub profession: String,
    #[serde(default)]
    pub description_zh: String,
    #[serde(default)]
    pub description_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubGithubImportJob {
    #[serde(rename = "id", alias = "job_id", default)]
    pub id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub imported: u32,
    #[serde(default)]
    pub skills: Vec<HubGithubImportSkillPreview>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct HubGithubImportStartBody<'a> {
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#ref: Option<&'a str>,
}

pub async fn start_github_import(
    cfg: &HubConfig,
    url: &str,
    git_ref: Option<&str>,
) -> Result<HubGithubImportJob, String> {
    post_json(
        cfg,
        "/skills/import/github",
        &HubGithubImportStartBody {
            url,
            r#ref: git_ref,
        },
    )
    .await
}

pub async fn get_github_import_job(
    cfg: &HubConfig,
    job_id: &str,
) -> Result<HubGithubImportJob, String> {
    get_json(
        cfg,
        &format!("/skills/import/jobs/{}", encode_path_segment(job_id)),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_config_trims_trailing_slash() {
        let cfg = HubConfig::new("http://localhost:3100/api/v1/".into());
        assert_eq!(
            cfg.endpoint("/skills"),
            "http://localhost:3100/api/v1/skills"
        );
        assert_eq!(
            cfg.endpoint("employees"),
            "http://localhost:3100/api/v1/employees"
        );
    }
}
