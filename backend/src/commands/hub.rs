use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::services::employee::{self, CreateEmployeeInput};
use crate::services::github_client;
use crate::services::hub_client::{
    self, HubConfig, HubEmployee, HubListEmployeesQuery, HubListSkillsQuery, HubManifest, HubSkill,
    PaginatedResponse, ProfessionInfo,
};
use crate::services::hub_install::{
    self, HubEmployeeInstallInput, HubEmployeeInstallResult, HubSkillInstallInput,
    HubSkillInstallResult,
};
use crate::services::hub_state::{
    self, HubDownloadFilter, HubLocalSource, HubLocalState, HubOriginKind,
};
use crate::services::root_workspace::RootWorkspace;
use crate::services::skill;
use crate::state::AppState;

const DEFAULT_HUB_URL: &str = "https://api.chavoai.cn/api/v1";

struct DownloadedSkillBundle {
    bundle: std::path::PathBuf,
    input: HubSkillInstallInput,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubSkillView {
    #[serde(flatten)]
    pub skill: HubSkill,
    #[serde(flatten)]
    pub local: HubLocalState,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubEmployeeDependencySummary {
    pub total: u32,
    pub downloaded: u32,
    pub missing: u32,
    pub update_available: u32,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubEmployeeView {
    #[serde(flatten)]
    pub employee: HubEmployee,
    #[serde(flatten)]
    pub local: HubLocalState,
    pub dependency_summary: HubEmployeeDependencySummary,
}

fn create_temp_dir(prefix: &str) -> Result<std::path::PathBuf, String> {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path)
        .map_err(|e| format!("创建 Hub 临时目录失败 ({}): {e}", path.display()))?;
    Ok(path)
}

fn hub_config(_root: &RootWorkspace) -> HubConfig {
    HubConfig::new(DEFAULT_HUB_URL.to_string())
}

fn origin_by_hub_id(
    root_dir: &Path,
    kind: HubOriginKind,
) -> Result<HashMap<String, hub_state::HubOrigin>, String> {
    let mut map = HashMap::new();
    if !root_dir.is_dir() {
        return Ok(map);
    }
    for entry in fs::read_dir(root_dir).map_err(|e| format!("读取 Hub origin 目录失败: {e}"))?
    {
        let entry = entry.map_err(|e| format!("读取 Hub origin 条目失败: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(origin) = hub_state::read_origin(&path)? {
            if origin.kind == kind {
                map.insert(origin.hub_id.clone(), origin);
            }
        }
    }
    Ok(map)
}

fn dependency_summary(
    root: &RootWorkspace,
    skill_hub_ids: &[String],
) -> Result<HubEmployeeDependencySummary, String> {
    let mut summary = HubEmployeeDependencySummary {
        total: skill_hub_ids.len() as u32,
        downloaded: 0,
        missing: 0,
        update_available: 0,
        conflicts: Vec::new(),
    };
    for skill_hub_id in skill_hub_ids {
        let local_id = hub_install::local_skill_id_from_hub_id(skill_hub_id);
        let skill_dir = root.skills_dir().join(&local_id);
        if !skill_dir.exists() {
            summary.missing += 1;
            continue;
        }
        match hub_state::read_origin(&skill_dir)? {
            Some(origin)
                if origin.kind == HubOriginKind::Skill && origin.hub_id == *skill_hub_id =>
            {
                summary.downloaded += 1;
            }
            Some(origin) => summary
                .conflicts
                .push(format!("{local_id} belongs to {}", origin.hub_id)),
            None => summary
                .conflicts
                .push(format!("{local_id} is a local non-Hub skill")),
        }
    }
    Ok(summary)
}

fn local_state_for_hub_item(
    local_root: &Path,
    kind: HubOriginKind,
    hub_id: &str,
    local_id: &str,
    remote_updated_at: &str,
    origins: &HashMap<String, hub_state::HubOrigin>,
) -> Result<HubLocalState, String> {
    let mut local = hub_state::merge_local_state(remote_updated_at, origins.get(hub_id));
    if local.downloaded {
        return Ok(local);
    }

    let local_dir = local_root.join(local_id);
    if !local_dir.exists() {
        return Ok(local);
    }

    local.local_id = Some(local_id.to_string());
    match hub_state::read_origin(&local_dir)? {
        Some(origin) if origin.kind == kind && origin.hub_id == hub_id => {}
        Some(origin) if origin.kind == kind => {
            local.local_source = Some(HubLocalSource::OtherHub);
            local.local_source_detail = Some(origin.hub_id);
        }
        Some(_) => {
            local.local_source = Some(HubLocalSource::OtherKind);
        }
        None => {
            local.local_source = Some(HubLocalSource::Custom);
        }
    };
    Ok(local)
}

fn skill_matches_list_query(skill: &HubSkill, query: Option<&str>) -> bool {
    let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let query = query.to_lowercase();
    [
        skill.id.as_str(),
        skill.name.as_str(),
        skill.description_zh.as_str(),
        skill.description_en.as_str(),
    ]
    .into_iter()
    .any(|part| part.to_lowercase().contains(&query))
}

fn hub_skill_from_local_origin(
    origin: &hub_state::HubOrigin,
    skills_dir: &Path,
) -> HubSkill {
    let description = skill::read_skill_description(&skills_dir.join(&origin.local_id));
    HubSkill {
        id: origin.hub_id.clone(),
        name: origin.local_id.clone(),
        description_zh: description.clone(),
        description_en: description,
        profession: "general".to_string(),
        content_hash: origin.content_hash.clone().unwrap_or_default(),
        source: serde_json::json!({ "type": "github" }),
        tags: Vec::new(),
        created_at: origin.installed_at.clone(),
        updated_at: origin.hub_updated_at.clone(),
    }
}

fn enrich_skill_description_from_local(skill: &mut HubSkill, skills_dir: &Path, local_id: &str) {
    if !skill.description_zh.is_empty() || !skill.description_en.is_empty() {
        return;
    }
    let description = skill::read_skill_description(&skills_dir.join(local_id));
    if description.is_empty() {
        return;
    }
    skill.description_zh = description.clone();
    skill.description_en = description;
}

async fn list_installed_hub_skills(
    cfg: &HubConfig,
    skills_dir: &Path,
    origins: &HashMap<String, hub_state::HubOrigin>,
    filter: HubDownloadFilter,
    profession: Option<&str>,
    query: Option<&str>,
    page: u32,
    limit: u32,
) -> Result<PaginatedResponse<HubSkillView>, String> {
    let mut matched = Vec::new();
    for (hub_id, origin) in origins {
        let mut skill = if origin.hub_url == DEFAULT_HUB_URL {
            match hub_client::get_skill_detail(cfg, hub_id).await {
                Ok(detail) => detail.skill,
                Err(_) => hub_skill_from_local_origin(origin, skills_dir),
            }
        } else {
            hub_skill_from_local_origin(origin, skills_dir)
        };
        let local_id = hub_install::local_skill_id_from_hub_id(&skill.id);
        enrich_skill_description_from_local(&mut skill, skills_dir, &local_id);
        if let Some(profession) = profession {
            if skill.profession != profession {
                continue;
            }
        }
        if !skill_matches_list_query(&skill, query) {
            continue;
        }
        let local = local_state_for_hub_item(
            skills_dir,
            HubOriginKind::Skill,
            &skill.id,
            &local_id,
            &skill.updated_at,
            origins,
        )?;
        let keep = match filter {
            HubDownloadFilter::Local => local.downloaded && !local.update_available,
            HubDownloadFilter::UpdateAvailable => local.update_available,
            HubDownloadFilter::Custom => local.downloaded && origin.hub_url != DEFAULT_HUB_URL,
            _ => false,
        };
        if keep {
            matched.push(HubSkillView { skill, local });
        }
    }

    matched.sort_by(|left, right| {
        left.skill
            .name
            .cmp(&right.skill.name)
            .then_with(|| left.skill.id.cmp(&right.skill.id))
    });

    let total = matched.len() as u32;
    let page = page.max(1);
    let limit = limit.max(1);
    let start = ((page - 1) * limit) as usize;
    let end = start.saturating_add(limit as usize).min(matched.len());
    let items = matched[start..end].to_vec();

    Ok(PaginatedResponse {
        total,
        page,
        limit,
        items,
    })
}

#[tauri::command]
pub async fn hub_get_manifest(app_state: State<'_, Arc<AppState>>) -> Result<HubManifest, String> {
    hub_client::get_manifest(&hub_config(&app_state.root)).await
}

#[tauri::command]
pub async fn hub_list_professions(
    app_state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProfessionInfo>, String> {
    hub_client::list_professions(&hub_config(&app_state.root)).await
}

#[tauri::command]
pub async fn hub_list_skills(
    app_state: State<'_, Arc<AppState>>,
    query: Option<String>,
    profession: Option<String>,
    filter: Option<HubDownloadFilter>,
    page: Option<u32>,
    limit: Option<u32>,
) -> Result<PaginatedResponse<HubSkillView>, String> {
    let cfg = hub_config(&app_state.root);
    let page = page.unwrap_or(1).max(1);
    let limit = limit.unwrap_or(50).max(1);
    let filter = filter.unwrap_or(HubDownloadFilter::All);
    let origins = origin_by_hub_id(&app_state.root.skills_dir(), HubOriginKind::Skill)?;

    if matches!(
        filter,
        HubDownloadFilter::Local | HubDownloadFilter::UpdateAvailable | HubDownloadFilter::Custom
    ) {
        return list_installed_hub_skills(
            &cfg,
            &app_state.root.skills_dir(),
            &origins,
            filter,
            profession.as_deref(),
            query.as_deref(),
            page,
            limit,
        )
        .await;
    }

    let remote = hub_client::list_skills(
        &cfg,
        HubListSkillsQuery {
            q: query,
            profession,
            filter: None,
            page: Some(page),
            limit: Some(limit),
        },
    )
    .await?;
    let mut items = remote
        .items
        .into_iter()
        .map(|skill| {
            let local_id = hub_install::local_skill_id_from_hub_id(&skill.id);
            let local = local_state_for_hub_item(
                &app_state.root.skills_dir(),
                HubOriginKind::Skill,
                &skill.id,
                &local_id,
                &skill.updated_at,
                &origins,
            )?;
            Ok(HubSkillView { skill, local })
        })
        .collect::<Result<Vec<_>, String>>()?;
    items.retain(|item| match filter {
        HubDownloadFilter::All => true,
        HubDownloadFilter::Remote => !item.local.downloaded && item.local.local_source.is_none(),
        HubDownloadFilter::Local => item.local.downloaded && !item.local.update_available,
        HubDownloadFilter::UpdateAvailable => item.local.update_available,
        HubDownloadFilter::Custom => !item.local.downloaded && item.local.local_source.is_some(),
    });
    Ok(PaginatedResponse {
        total: remote.total,
        page: remote.page,
        limit: remote.limit,
        items,
    })
}

#[tauri::command]
pub async fn hub_get_skill_detail(
    app_state: State<'_, Arc<AppState>>,
    hub_skill_id: String,
) -> Result<hub_client::HubSkillDetail, String> {
    hub_client::get_skill_detail(&hub_config(&app_state.root), &hub_skill_id).await
}

#[tauri::command]
pub async fn hub_install_skill(
    app_state: State<'_, Arc<AppState>>,
    hub_skill_id: String,
) -> Result<HubSkillInstallResult, String> {
    let cfg = hub_config(&app_state.root);
    let detail = hub_client::get_skill_detail(&cfg, &hub_skill_id).await?;
    let tmp = create_temp_dir("chawork-hub-skill")?;
    let result = async {
        let bundle = tmp.join("skill.tar.gz");
        hub_client::download_skill_bundle(&cfg, &hub_skill_id, &bundle).await?;
        let _lock = app_state.lock_employee_write();
        hub_install::install_skill_bundle_from_path(
            &bundle,
            &app_state.root.skills_dir(),
            HubSkillInstallInput {
                hub_url: cfg.base_url.clone(),
                hub_id: detail.skill.id,
                content_hash: detail.skill.content_hash,
                hub_updated_at: detail.skill.updated_at,
            },
        )
    }
    .await;
    let _ = fs::remove_dir_all(&tmp);
    result
}

#[tauri::command]
pub async fn hub_uninstall_skill(
    app_state: State<'_, Arc<AppState>>,
    hub_skill_id: String,
) -> Result<hub_install::HubSkillUninstallResult, String> {
    let _lock = app_state.lock_employee_write();
    hub_install::uninstall_skill_from_root(&app_state.root.skills_dir(), &hub_skill_id)
}

#[tauri::command]
pub async fn hub_list_employees(
    app_state: State<'_, Arc<AppState>>,
    query: Option<String>,
    tags: Option<String>,
    filter: Option<HubDownloadFilter>,
    page: Option<u32>,
    limit: Option<u32>,
) -> Result<PaginatedResponse<HubEmployeeView>, String> {
    let cfg = hub_config(&app_state.root);
    let remote = hub_client::list_employees(
        &cfg,
        HubListEmployeesQuery {
            q: query,
            tags,
            page,
            limit,
        },
    )
    .await?;
    let origins = origin_by_hub_id(&app_state.root.employees_dir(), HubOriginKind::Employee)?;
    let mut items = Vec::new();
    for employee in remote.items {
        let local = local_state_for_hub_item(
            &app_state.root.employees_dir(),
            HubOriginKind::Employee,
            &employee.id,
            &employee.id,
            &employee.updated_at,
            &origins,
        )?;
        let dependency_summary = dependency_summary(&app_state.root, &employee.skill_ids)?;
        items.push(HubEmployeeView {
            employee,
            local,
            dependency_summary,
        });
    }
    let filter = filter.unwrap_or(HubDownloadFilter::All);
    items.retain(|item| match filter {
        HubDownloadFilter::All => true,
        HubDownloadFilter::Remote => !item.local.downloaded && item.local.local_source.is_none(),
        HubDownloadFilter::Local => item.local.downloaded && !item.local.update_available,
        HubDownloadFilter::UpdateAvailable => item.local.update_available,
        HubDownloadFilter::Custom => !item.local.downloaded && item.local.local_source.is_some(),
    });
    Ok(PaginatedResponse {
        total: remote.total,
        page: remote.page,
        limit: remote.limit,
        items,
    })
}

#[tauri::command]
pub async fn hub_get_employee_detail(
    app_state: State<'_, Arc<AppState>>,
    hub_employee_id: String,
) -> Result<hub_client::HubEmployeeDetail, String> {
    hub_client::get_employee_detail(&hub_config(&app_state.root), &hub_employee_id).await
}

#[tauri::command]
pub async fn hub_install_employee(
    app_state: State<'_, Arc<AppState>>,
    hub_employee_id: String,
) -> Result<HubEmployeeInstallResult, String> {
    let cfg = hub_config(&app_state.root);
    let detail = hub_client::get_employee_detail(&cfg, &hub_employee_id).await?;
    let tmp = create_temp_dir("chawork-hub-employee")?;
    let result = async {
        let mut skill_bundles = Vec::new();
        for skill_id in &detail.employee.skill_ids {
            let skill_detail = hub_client::get_skill_detail(&cfg, skill_id).await?;
            let local_id = hub_install::local_skill_id_from_hub_id(skill_id);
            let bundle = tmp.join(format!("skill-{local_id}.tar.gz"));
            hub_client::download_skill_bundle(&cfg, skill_id, &bundle).await?;
            skill_bundles.push(DownloadedSkillBundle {
                bundle,
                input: HubSkillInstallInput {
                    hub_url: cfg.base_url.clone(),
                    hub_id: skill_detail.skill.id,
                    content_hash: skill_detail.skill.content_hash,
                    hub_updated_at: skill_detail.skill.updated_at,
                },
            });
        }

        let bundle = tmp.join("employee.tar.gz");
        hub_client::download_employee_bundle(&cfg, &hub_employee_id, &bundle).await?;
        let _lock = app_state.lock_employee_write();
        for skill_bundle in &skill_bundles {
            hub_install::install_skill_bundle_from_path(
                &skill_bundle.bundle,
                &app_state.root.skills_dir(),
                skill_bundle.input.clone(),
            )?;
        }
        hub_install::install_employee_bundle_from_path(
            &bundle,
            &app_state.root,
            HubEmployeeInstallInput {
                hub_url: cfg.base_url.clone(),
                hub_id: detail.employee.id,
                hub_updated_at: detail.employee.updated_at,
            },
        )
    }
    .await;
    let _ = fs::remove_dir_all(&tmp);
    result
}

#[tauri::command]
pub async fn hub_start_github_import(
    app_state: State<'_, Arc<AppState>>,
    url: String,
    git_ref: Option<String>,
) -> Result<hub_client::HubGithubImportJob, String> {
    hub_client::start_github_import(&hub_config(&app_state.root), &url, git_ref.as_deref()).await
}

#[tauri::command]
pub async fn hub_get_github_import_job(
    app_state: State<'_, Arc<AppState>>,
    job_id: String,
) -> Result<hub_client::HubGithubImportJob, String> {
    hub_client::get_github_import_job(&hub_config(&app_state.root), &job_id).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubGithubImportCompleteResult {
    pub installed_skill_count: u32,
    pub root_skill_ids: Vec<String>,
    pub employee_id: Option<String>,
    pub employee_name: Option<String>,
    pub employee_created: bool,
    pub failed_hub_skill_ids: Vec<String>,
}

fn github_repo_segments(url: &str) -> Result<(&str, &str), String> {
    let trimmed = url.trim().trim_end_matches('/');
    let marker = "github.com/";
    let path = trimmed
        .split(marker)
        .nth(1)
        .ok_or_else(|| "无效的 GitHub 仓库地址".to_string())?;
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if segments.len() < 2 {
        return Err("GitHub 仓库地址缺少 owner/repo".to_string());
    }
    let owner = segments[0];
    let repo = segments[1].trim_end_matches(".git");
    Ok((owner, repo))
}

fn github_repo_name_from_url(url: &str) -> Result<String, String> {
    let (_, repo) = github_repo_segments(url)?;
    Ok(repo.to_string())
}

fn github_owner_name_from_url(url: &str) -> Result<String, String> {
    let (owner, _) = github_repo_segments(url)?;
    Ok(owner.to_string())
}

fn github_owner_repo_slug_from_url(url: &str) -> Result<String, String> {
    let (owner, repo) = github_repo_segments(url)?;
    Ok(hub_install::local_skill_id_from_hub_id(&format!("{owner}-{repo}")))
}

fn github_employee_id_from_repo_url(url: &str) -> Result<String, String> {
    let repo_name = github_repo_name_from_url(url)?;
    Ok(hub_install::local_skill_id_from_hub_id(&repo_name))
}

fn employee_registry_contains(root: &RootWorkspace, id: &str) -> bool {
    employee::list(root)
        .map(|entries| entries.iter().any(|entry| entry.id == id))
        .unwrap_or(false)
}

fn resolve_github_import_employee_target(
    root: &RootWorkspace,
    repo_url: &str,
) -> Result<(String, bool), String> {
    let repo_name = github_repo_name_from_url(repo_url)?;
    let canonical = github_employee_id_from_repo_url(repo_url)?;
    let owner_repo_slug = github_owner_repo_slug_from_url(repo_url)?;

    if employee_registry_contains(root, &canonical) {
        return Ok((canonical, true));
    }
    if repo_name != canonical && employee_registry_contains(root, &repo_name) {
        return Ok((repo_name, true));
    }
    if owner_repo_slug != canonical
        && owner_repo_slug != repo_name
        && employee_registry_contains(root, &owner_repo_slug)
    {
        return Ok((owner_repo_slug, true));
    }
    Ok((canonical, false))
}

fn ensure_github_import_employee(
    root: &RootWorkspace,
    repo_url: &str,
    employee_name: &str,
    prompt: String,
    root_skill_ids: &[String],
) -> Result<String, String> {
    let (employee_id, exists) = resolve_github_import_employee_target(root, repo_url)?;
    if exists {
        employee::update_metadata(
            root,
            &employee_id,
            employee::UpdateEmployeeInput {
                name: Some(employee_name.to_string()),
                description: Some(format!("从 GitHub 仓库 {repo_url} 导入")),
                status: None,
            },
        )?;
        employee::write_employee_prompt(root, &employee_id, &prompt)?;
        for skill_id in root_skill_ids {
            match employee::copy_root_skill_to_employee(root, &employee_id, skill_id) {
                Ok(_) => {}
                Err(message) if message.contains("已拥有技能") => {}
                Err(message) => return Err(message),
            }
        }
        return Ok(employee_id);
    }

    employee::create(
        root,
        CreateEmployeeInput {
            id: employee_id.clone(),
            name: employee_name.to_string(),
            description: format!("从 GitHub 仓库 {repo_url} 导入"),
            initial_prompt: prompt,
            root_skill_ids: Vec::new(),
        },
    )?;
    for skill_id in root_skill_ids {
        match employee::copy_root_skill_to_employee(root, &employee_id, skill_id) {
            Ok(_) => {}
            Err(message) if message.contains("已拥有技能") => {}
            Err(message) => return Err(message),
        }
    }
    Ok(employee_id)
}

async fn install_hub_skill_to_root(
    app_state: &AppState,
    hub_skill_id: &str,
) -> Result<HubSkillInstallResult, String> {
    let cfg = hub_config(&app_state.root);
    let detail = hub_client::get_skill_detail(&cfg, hub_skill_id).await?;
    let tmp = create_temp_dir("chawork-hub-skill")?;
    let result = async {
        let bundle = tmp.join("skill.tar.gz");
        hub_client::download_skill_bundle(&cfg, hub_skill_id, &bundle).await?;
        let _lock = app_state.lock_employee_write();
        hub_install::install_skill_bundle_from_path(
            &bundle,
            &app_state.root.skills_dir(),
            HubSkillInstallInput {
                hub_url: cfg.base_url.clone(),
                hub_id: detail.skill.id,
                content_hash: detail.skill.content_hash,
                hub_updated_at: detail.skill.updated_at,
            },
        )
    }
    .await;
    let _ = fs::remove_dir_all(&tmp);
    result
}

#[tauri::command(rename_all = "camelCase")]
pub async fn hub_complete_github_import(
    app_state: State<'_, Arc<AppState>>,
    repo_url: String,
    hub_skill_ids: Vec<String>,
    create_employee: bool,
    employee_prompt: Option<String>,
) -> Result<HubGithubImportCompleteResult, String> {
    if hub_skill_ids.is_empty() {
        return Err("没有可安装的技能".to_string());
    }

    let should_create_employee = create_employee
        || employee_prompt
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty());

    if should_create_employee
        && employee_prompt
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("同步为员工时必须填写员工定义 prompt".to_string());
    }

    let employee_display_name = github_owner_name_from_url(&repo_url)?;
    let mut root_skill_ids = Vec::new();
    let mut failed_hub_skill_ids = Vec::new();

    for hub_skill_id in &hub_skill_ids {
        match install_hub_skill_to_root(&app_state, hub_skill_id).await {
            Ok(result) => root_skill_ids.push(result.local_id),
            Err(_) => failed_hub_skill_ids.push(hub_skill_id.clone()),
        }
    }

    if root_skill_ids.is_empty() {
        return Err("技能安装到 Root 失败".to_string());
    }

    root_skill_ids.sort();
    root_skill_ids.dedup();

    let (employee_id, employee_name, employee_created) = if should_create_employee {
        let _lock = app_state.lock_employee_write();
        employee::ensure_employee_infrastructure(&app_state.root)?;
        let employee_id = ensure_github_import_employee(
            &app_state.root,
            &repo_url,
            &employee_display_name,
            employee_prompt
                .as_ref()
                .map(|value| value.trim().to_string())
                .unwrap_or_default(),
            &root_skill_ids,
        )?;
        (Some(employee_id.clone()), Some(employee_display_name), true)
    } else {
        (None, None, false)
    };

    Ok(HubGithubImportCompleteResult {
        installed_skill_count: root_skill_ids.len() as u32,
        root_skill_ids,
        employee_id,
        employee_name,
        employee_created,
        failed_hub_skill_ids,
    })
}

// ─── GitHub 直接扫描（不依赖 Hub API）─────────────────────────────

#[tauri::command(rename_all = "camelCase")]
pub async fn github_scan_repo(
    app_state: State<'_, Arc<AppState>>,
    url: String,
    git_ref: Option<String>,
) -> Result<Vec<github_client::GithubSkillPreview>, String> {
    github_client::scan_github_repo(&url, git_ref.as_deref(), Some(&app_state.root.cache_dir()))
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn github_download_all_skills(
    app_state: State<'_, Arc<AppState>>,
    url: String,
    skill_paths: Vec<String>,
    git_ref: Option<String>,
) -> Result<github_client::GithubBulkDownloadResult, String> {
    if skill_paths.is_empty() {
        return Err("没有可安装的技能".to_string());
    }
    let result = github_client::download_and_install_skills(
        &url,
        &skill_paths,
        git_ref.as_deref(),
        &app_state.root.skills_dir(),
        Some(&app_state.root.cache_dir()),
    )
    .await?;
    Ok(result)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubDirectImportResult {
    pub installed_count: u32,
    pub skill_ids: Vec<String>,
    pub failed: Vec<github_client::GithubDownloadResult>,
    pub employee_id: Option<String>,
    pub employee_name: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn github_complete_import(
    app_state: State<'_, Arc<AppState>>,
    url: String,
    skill_paths: Vec<String>,
    git_ref: Option<String>,
    sync_as_employee: bool,
    employee_prompt: Option<String>,
) -> Result<GithubDirectImportResult, String> {
    if skill_paths.is_empty() {
        return Err("没有可安装的技能".to_string());
    }

    if sync_as_employee
        && employee_prompt
            .as_ref()
            .is_none_or(|v| v.trim().is_empty())
    {
        return Err("同步为员工时必须填写员工定义 prompt".to_string());
    }

    let download_result = github_client::download_and_install_skills(
        &url,
        &skill_paths,
        git_ref.as_deref(),
        &app_state.root.skills_dir(),
        Some(&app_state.root.cache_dir()),
    )
    .await?;

    if download_result.skill_ids.is_empty() {
        return Err("没有技能安装成功".to_string());
    }

    let (employee_id, employee_name) = if sync_as_employee {
        let _lock = app_state.lock_employee_write();
        employee::ensure_employee_infrastructure(&app_state.root)?;
        let display_name = github_owner_name_from_url(&url)?;
        let eid = ensure_github_import_employee(
            &app_state.root,
            &url,
            &display_name,
            employee_prompt
                .as_ref()
                .map(|v| v.trim().to_string())
                .unwrap_or_default(),
            &download_result.skill_ids,
        )?;
        (Some(eid), Some(display_name))
    } else {
        (None, None)
    };

    Ok(GithubDirectImportResult {
        installed_count: download_result.installed_count,
        skill_ids: download_result.skill_ids,
        failed: download_result.failed,
        employee_id,
        employee_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::root_workspace;

    #[test]
    fn hub_config_uses_fixed_api_base_url() {
        let tmp = tempfile::tempdir().unwrap();
        let root = root_workspace::init_or_open(tmp.path()).unwrap();

        assert_eq!(hub_config(&root).base_url, "https://api.chavoai.cn/api/v1");
    }

    #[test]
    fn github_repo_name_from_url_parses_owner_repo() {
        assert_eq!(
            github_repo_name_from_url("https://github.com/anthropics/skills").unwrap(),
            "skills"
        );
        assert_eq!(
            github_repo_name_from_url("https://github.com/anthropics/skills/").unwrap(),
            "skills"
        );
    }

    #[test]
    fn hub_skill_from_local_origin_reads_skill_description() {
        let tmp = tempfile::tempdir().unwrap();
        let root = root_workspace::init_or_open(tmp.path()).unwrap();
        let skill_dir = root.skills_dir().join("pdf");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: pdf\ndescription: Parse PDF files\n---\n",
        )
        .unwrap();
        let origin = hub_state::HubOrigin {
            kind: HubOriginKind::Skill,
            hub_url: "github".to_string(),
            hub_id: "github--owner--pdf".to_string(),
            local_id: "pdf".to_string(),
            content_hash: None,
            installed_at: "2026-06-10T00:00:00Z".to_string(),
            hub_updated_at: "2026-06-10T00:00:00Z".to_string(),
            skill_hub_ids: Vec::new(),
        };

        let skill = hub_skill_from_local_origin(&origin, &root.skills_dir());

        assert_eq!(skill.description_zh, "Parse PDF files");
        assert_eq!(skill.description_en, "Parse PDF files");
    }

    #[test]
    fn local_state_reports_custom_local_skill_source() {
        let tmp = tempfile::tempdir().unwrap();
        let root = root_workspace::init_or_open(tmp.path()).unwrap();
        let skill_dir = root.skills_dir().join("pdf");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: pdf\n---\n").unwrap();
        let origins = origin_by_hub_id(&root.skills_dir(), HubOriginKind::Skill).unwrap();

        let state = local_state_for_hub_item(
            &root.skills_dir(),
            HubOriginKind::Skill,
            "repo--skills--pdf",
            "pdf",
            "2026-06-10T00:00:00Z",
            &origins,
        )
        .unwrap();

        assert!(!state.downloaded);
        assert_eq!(state.local_id.as_deref(), Some("pdf"));
        assert_eq!(state.local_source, Some(HubLocalSource::Custom));
    }

    #[test]
    fn github_owner_name_from_url_parses_owner() {
        assert_eq!(
            github_owner_name_from_url("https://github.com/slavingia/skills").unwrap(),
            "slavingia",
        );
    }

    #[test]
    fn github_employee_id_from_repo_url_uses_repo_name() {
        assert_eq!(
            github_employee_id_from_repo_url("https://github.com/slavingia/skills").unwrap(),
            "skills",
        );
    }

    #[test]
    fn github_owner_repo_slug_from_url_uses_owner_repo_slug() {
        assert_eq!(
            github_owner_repo_slug_from_url("https://github.com/slavingia/skills").unwrap(),
            "slavingia-skills",
        );
    }

    #[test]
    fn ensure_github_import_employee_creates_canonical_employee() {
        let tmp = tempfile::tempdir().unwrap();
        let root = root_workspace::init_or_open(tmp.path()).unwrap();
        employee::ensure_employee_infrastructure(&root).unwrap();
        let skill_dir = root.skills_dir().join("doc-coauthoring");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: doc\n---\n").unwrap();

        let employee_id = ensure_github_import_employee(
            &root,
            "https://github.com/slavingia/skills",
            "slavingia",
            "GitHub skills employee prompt.".to_string(),
            &["doc-coauthoring".to_string()],
        )
        .expect("create github import employee");

        assert_eq!(employee_id, "skills");
        assert!(employee_registry_contains(&root, "skills"));
        let detail = employee::get_detail(&root, "skills").expect("employee detail");
        assert_eq!(detail.registry_entry.name, "slavingia");
        let skills = employee::list_employee_skills(&root, "skills").expect("skills");
        assert!(skills.iter().any(|skill| skill.id == "doc-coauthoring"));
    }

    #[test]
    fn ensure_github_import_employee_reuses_legacy_repo_name_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = root_workspace::init_or_open(tmp.path()).unwrap();
        employee::ensure_employee_infrastructure(&root).unwrap();
        employee::create(
            &root,
            CreateEmployeeInput {
                id: "skills".to_string(),
                name: "skills".to_string(),
                description: "legacy import".to_string(),
                initial_prompt: "legacy prompt".to_string(),
                root_skill_ids: Vec::new(),
            },
        )
        .expect("legacy employee");
        let skill_dir = root.skills_dir().join("pdf");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: pdf\n---\n").unwrap();

        let employee_id = ensure_github_import_employee(
            &root,
            "https://github.com/slavingia/skills",
            "slavingia",
            "Updated prompt.".to_string(),
            &["pdf".to_string()],
        )
        .expect("reuse legacy employee");

        assert_eq!(employee_id, "skills");
        assert!(!employee_registry_contains(&root, "slavingia-skills"));
        let detail = employee::get_detail(&root, "skills").expect("employee detail");
        assert_eq!(detail.registry_entry.name, "slavingia");
        let prompt = employee::read_employee_prompt(&root, "skills").expect("prompt");
        assert_eq!(prompt, "Updated prompt.");
        let skills = employee::list_employee_skills(&root, "skills").expect("skills");
        assert!(skills.iter().any(|skill| skill.id == "pdf"));
    }

    #[test]
    fn dependency_summary_treats_dot_skill_id_as_missing_in_empty_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = root_workspace::init_or_open(tmp.path()).unwrap();

        let summary = dependency_summary(&root, &["tanshilong-article-skill--.".to_string()])
            .expect("dependency summary");

        assert_eq!(summary.total, 1);
        assert_eq!(summary.downloaded, 0);
        assert_eq!(summary.missing, 1);
        assert!(summary.conflicts.is_empty());
    }
}
