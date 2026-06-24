//! Employee 管理服务（DESIGN §Employee/Dream 系统）。
//!
//! 提供 employee registry、manifest、skill registry、workspace membership 的
//! 数据模型与 CRUD 操作。Root 初始化时自动创建 `__dream__` 和 `general` 员工。

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use super::root_workspace::RootWorkspace;

// ── Constants ──────────────────────────────────────────────────────────────

pub const DREAM_EMPLOYEE_ID: &str = "__dream__";
pub const GENERAL_EMPLOYEE_ID: &str = "general";
const EMPLOYEE_MANIFEST_FILE: &str = "employee.yaml";
const SKILLS_REGISTRY_FILE: &str = "skills.json";
const PROMPT_FILE: &str = "prompt.md";
const DEFAULT_DREAM_YAML: &str =
    "enabled: false\nschedule:\n  type: daily\nsession_scan:\n  scope: all\n  latest_sessions: 3\n";
const DEFAULT_GENERAL_PROMPT: &str = r#"# 通用员工

你是 ChaWork 的默认通用员工，用于尚未形成专门工作方法的新工作区。

## 工作方式

- 先理解当前 workspace 的资料、会话上下文和用户目标，再给出行动。
- 优先帮助用户整理、创建、更新、搜索和总结当前工作区内的文件。
- 对不确定的信息保持明确边界，必要时说明缺口并提出下一步验证方式。
- 当用户要求修改文件时，保持改动范围清晰，避免无关重构或无关内容变更。
- 把可复用的方法、偏好、约束和决策依据沉淀为未来可以改进本 prompt 的候选经验。
"#;

// ── Data Models ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmployeeKind {
    Ordinary,
    Dream,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmployeeStatus {
    Active,
    Archived,
}

/// A single entry in `state/employee-registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub kind: EmployeeKind,
    pub name: String,
    pub path: String,
    pub status: EmployeeStatus,
}

/// Top-level `state/employee-registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeRegistry {
    pub version: u32,
    pub employees: Vec<RegistryEntry>,
}

impl EmployeeRegistry {
    fn new() -> Self {
        Self {
            version: 1,
            employees: Vec::new(),
        }
    }

    fn find(&self, id: &str) -> Option<&RegistryEntry> {
        self.employees.iter().find(|e| e.id == id)
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut RegistryEntry> {
        self.employees.iter_mut().find(|e| e.id == id)
    }
}

/// `employee.yaml` inside each employee directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub kind: EmployeeKind,
    pub status: EmployeeStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// A single skill entry in `skills.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    pub enabled: bool,
}

/// `skills.json` inside each employee directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRegistry {
    pub version: u32,
    pub skills: Vec<SkillEntry>,
}

impl SkillRegistry {
    fn empty() -> Self {
        Self {
            version: 1,
            skills: Vec::new(),
        }
    }
}

/// A single workspace binding in `workspaces.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMembership {
    pub id: String,
    pub path: String,
    pub name: String,
    pub added_at: String,
}

/// `workspaces.json` inside each employee directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMembershipList {
    pub version: u32,
    pub workspaces: Vec<WorkspaceMembership>,
}

// ── Integrity Check ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityStatus {
    Ok,
    RepairNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub employee_id: String,
    pub status: IntegrityStatus,
    pub issues: Vec<IntegrityIssue>,
}

// ── Workspace Binding ──────────────────────────────────────────────────────

const WORKSPACE_BINDING_FILE: &str = ".chawork/employee.json";
const WORKSPACES_MEMBERSHIP_FILE: &str = "workspaces.json";

/// Content of `.chawork/employee.json` inside a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceBinding {
    pub employee_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    Unbound,
    Bound,
    EmployeeMissing,
    MembershipMissing,
    PathMismatch,
}

/// Validation result returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingValidation {
    pub status: BindingStatus,
    pub employee_id: Option<String>,
    pub employee_name: Option<String>,
    pub message: String,
}

// ── Detail view (returned to frontend) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeDetail {
    pub registry_entry: RegistryEntry,
    pub manifest: Option<EmployeeManifest>,
    pub integrity: IntegrityReport,
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn employee_dir(root: &RootWorkspace, id: &str) -> PathBuf {
    root.employees_dir().join(id)
}

fn manifest_path(root: &RootWorkspace, id: &str) -> PathBuf {
    employee_dir(root, id).join(EMPLOYEE_MANIFEST_FILE)
}

fn skills_path(root: &RootWorkspace, id: &str) -> PathBuf {
    employee_dir(root, id).join(SKILLS_REGISTRY_FILE)
}

// ── Registry I/O ───────────────────────────────────────────────────────────

fn read_registry(root: &RootWorkspace) -> Result<EmployeeRegistry, String> {
    let path = root.employee_registry_path();
    if !path.is_file() {
        return Ok(EmployeeRegistry::new());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 employee registry 失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 employee registry 失败: {e}"))
}

fn write_registry(root: &RootWorkspace, reg: &EmployeeRegistry) -> Result<(), String> {
    let path = root.employee_registry_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 registry 目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(reg)
        .map_err(|e| format!("序列化 employee registry 失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入 employee registry 失败: {e}"))
}

// ── Manifest I/O ───────────────────────────────────────────────────────────

fn read_manifest(root: &RootWorkspace, id: &str) -> Result<Option<EmployeeManifest>, String> {
    let path = manifest_path(root, id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取 employee manifest ({id}) 失败: {e}"))?;
    let m: EmployeeManifest = serde_yaml::from_str(&raw)
        .map_err(|e| format!("解析 employee manifest ({id}) 失败: {e}"))?;
    Ok(Some(m))
}

fn write_manifest(root: &RootWorkspace, id: &str, m: &EmployeeManifest) -> Result<(), String> {
    let path = manifest_path(root, id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 employee 目录失败: {e}"))?;
    }
    let yaml =
        serde_yaml::to_string(m).map_err(|e| format!("序列化 employee manifest 失败: {e}"))?;
    fs::write(&path, yaml).map_err(|e| format!("写入 employee manifest 失败: {e}"))
}

// ── Skills registry I/O ────────────────────────────────────────────────────

fn write_skills_registry(root: &RootWorkspace, id: &str, sr: &SkillRegistry) -> Result<(), String> {
    let path = skills_path(root, id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 skills 目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(sr)
        .map_err(|e| format!("序列化 skills registry 失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入 skills registry 失败: {e}"))
}

// ── Infrastructure init (called from root_workspace::init_or_open) ─────────

/// Ensures built-in employees + registry exist. Idempotent.
pub fn ensure_employee_infrastructure(root: &RootWorkspace) -> Result<(), String> {
    let mut reg = read_registry(root)?;

    // __dream__ 的 workspaces/ 目录用于存放 dream run 子工作区（非 workspace binding）。
    // 普通员工使用 workspaces.json 来记录 workspace membership；__dream__ 不需要。
    let dream_dir = root.dream_employee_dir();
    for sub in [
        dream_dir.clone(),
        dream_dir.join("skills"),
        dream_dir.join("workspaces"),
        dream_dir.join("logs/dream"),
    ] {
        fs::create_dir_all(&sub)
            .map_err(|e| format!("创建 dream 目录 {} 失败: {e}", sub.display()))?;
    }

    // employee.yaml
    if !manifest_path(root, DREAM_EMPLOYEE_ID).is_file() {
        let now = iso_now();
        let manifest = EmployeeManifest {
            id: DREAM_EMPLOYEE_ID.to_string(),
            name: "Dream Workflow".to_string(),
            description: String::new(),
            kind: EmployeeKind::Dream,
            status: EmployeeStatus::Active,
            created_at: now.clone(),
            updated_at: now,
        };
        write_manifest(root, DREAM_EMPLOYEE_ID, &manifest)?;
    }

    // prompt.md
    let prompt_path = dream_dir.join(PROMPT_FILE);
    if !prompt_path.is_file() {
        fs::write(&prompt_path, "").map_err(|e| format!("创建 dream prompt.md 失败: {e}"))?;
    }

    // skills.json
    if !skills_path(root, DREAM_EMPLOYEE_ID).is_file() {
        write_skills_registry(root, DREAM_EMPLOYEE_ID, &SkillRegistry::empty())?;
    }

    // Ensure __dream__ in registry
    if reg.find(DREAM_EMPLOYEE_ID).is_none() {
        reg.employees.push(RegistryEntry {
            id: DREAM_EMPLOYEE_ID.to_string(),
            kind: EmployeeKind::Dream,
            name: "Dream Workflow".to_string(),
            path: format!("employees/{DREAM_EMPLOYEE_ID}"),
            status: EmployeeStatus::Active,
        });
    }

    ensure_general_employee(root, &mut reg)?;

    write_registry(root, &reg)?;
    Ok(())
}

fn ensure_general_employee(root: &RootWorkspace, reg: &mut EmployeeRegistry) -> Result<(), String> {
    let emp_dir = employee_dir(root, GENERAL_EMPLOYEE_ID);
    for sub in [
        emp_dir.clone(),
        emp_dir.join("skills"),
        emp_dir.join("prompt-update-requests/pending"),
        emp_dir.join("prompt-update-requests/approved"),
        emp_dir.join("prompt-update-requests/applied"),
        emp_dir.join("prompt-update-requests/rejected"),
        emp_dir.join("prompt-update-requests/failed"),
        emp_dir.join("logs/dream"),
    ] {
        fs::create_dir_all(&sub)
            .map_err(|e| format!("创建通用员工目录 {} 失败: {e}", sub.display()))?;
    }

    let now = iso_now();
    let default_manifest = EmployeeManifest {
        id: GENERAL_EMPLOYEE_ID.to_string(),
        name: "通用员工".to_string(),
        description: "系统默认普通员工，用于尚未形成专门工作方法的新工作区。".to_string(),
        kind: EmployeeKind::Ordinary,
        status: EmployeeStatus::Active,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    match read_manifest(root, GENERAL_EMPLOYEE_ID) {
        Ok(Some(mut manifest)) => {
            let mut changed = false;
            if manifest.id != GENERAL_EMPLOYEE_ID {
                manifest.id = GENERAL_EMPLOYEE_ID.to_string();
                changed = true;
            }
            if manifest.kind != EmployeeKind::Ordinary {
                manifest.kind = EmployeeKind::Ordinary;
                changed = true;
            }
            if manifest.status != EmployeeStatus::Active {
                manifest.status = EmployeeStatus::Active;
                changed = true;
            }
            if manifest.name.trim().is_empty() {
                manifest.name = default_manifest.name.clone();
                changed = true;
            }
            if changed {
                manifest.updated_at = now.clone();
                write_manifest(root, GENERAL_EMPLOYEE_ID, &manifest)?;
            }
        }
        Ok(None) | Err(_) => write_manifest(root, GENERAL_EMPLOYEE_ID, &default_manifest)?,
    }

    let prompt_path = emp_dir.join(PROMPT_FILE);
    let should_seed_prompt = match fs::read_to_string(&prompt_path) {
        Ok(existing) => existing.trim().is_empty(),
        Err(_) => true,
    };
    if should_seed_prompt {
        fs::write(&prompt_path, DEFAULT_GENERAL_PROMPT)
            .map_err(|e| format!("创建通用员工 prompt.md 失败: {e}"))?;
    }

    if !skills_path(root, GENERAL_EMPLOYEE_ID).is_file() {
        write_skills_registry(root, GENERAL_EMPLOYEE_ID, &SkillRegistry::empty())?;
    }

    let dream_yaml = emp_dir.join("dream.yaml");
    if !dream_yaml.is_file() {
        fs::write(&dream_yaml, DEFAULT_DREAM_YAML)
            .map_err(|e| format!("创建通用员工 dream.yaml 失败: {e}"))?;
    }

    let ws_json = workspaces_membership_path(root, GENERAL_EMPLOYEE_ID);
    if !ws_json.is_file() {
        write_workspace_memberships(
            root,
            GENERAL_EMPLOYEE_ID,
            &WorkspaceMembershipList {
                version: 1,
                workspaces: Vec::new(),
            },
        )?;
    }

    match reg.find_mut(GENERAL_EMPLOYEE_ID) {
        Some(entry) => {
            entry.kind = EmployeeKind::Ordinary;
            entry.status = EmployeeStatus::Active;
            entry.path = format!("employees/{GENERAL_EMPLOYEE_ID}");
            if entry.name.trim().is_empty() {
                entry.name = "通用员工".to_string();
            }
        }
        None => reg.employees.push(RegistryEntry {
            id: GENERAL_EMPLOYEE_ID.to_string(),
            kind: EmployeeKind::Ordinary,
            name: "通用员工".to_string(),
            path: format!("employees/{GENERAL_EMPLOYEE_ID}"),
            status: EmployeeStatus::Active,
        }),
    }

    Ok(())
}

// ── Public API ─────────────────────────────────────────────────────────────

/// List all employees from registry.
pub fn list(root: &RootWorkspace) -> Result<Vec<RegistryEntry>, String> {
    let reg = read_registry(root)?;
    Ok(reg.employees)
}

/// Get employee detail (manifest + integrity check).
pub fn get_detail(root: &RootWorkspace, id: &str) -> Result<EmployeeDetail, String> {
    let reg = read_registry(root)?;
    get_detail_with_registry(root, &reg, id)
}

fn get_detail_with_registry(
    root: &RootWorkspace,
    reg: &EmployeeRegistry,
    id: &str,
) -> Result<EmployeeDetail, String> {
    let entry = reg
        .find(id)
        .ok_or_else(|| format!("员工 {id} 不在 registry 中"))?
        .clone();
    let manifest = read_manifest(root, id)?;
    let integrity = check_integrity_inner(root, reg, id);
    Ok(EmployeeDetail {
        registry_entry: entry,
        manifest,
        integrity,
    })
}

/// Input for creating a new ordinary employee.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateEmployeeInput {
    /// When empty, a UUID v4 is assigned automatically.
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub initial_prompt: String,
    #[serde(default)]
    pub root_skill_ids: Vec<String>,
}

impl CreateEmployeeInput {
    pub fn basic(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            initial_prompt: String::new(),
            root_skill_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedEmployeeInstall {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt_md: String,
    pub root_skill_ids: Vec<String>,
    pub hub_origin: Option<crate::services::hub_state::HubOrigin>,
}

/// Create a new ordinary employee.
pub fn create(root: &RootWorkspace, input: CreateEmployeeInput) -> Result<EmployeeDetail, String> {
    let id = resolve_new_employee_id(&input.id)?;
    let mut reg = read_registry(root)?;
    if reg.find(&id).is_some() {
        return Err(format!("员工 {id} 已存在"));
    }

    let emp_dir = employee_dir(root, &id);
    for sub in [
        emp_dir.clone(),
        emp_dir.join("skills"),
        emp_dir.join("prompt-update-requests/pending"),
        emp_dir.join("prompt-update-requests/approved"),
        emp_dir.join("prompt-update-requests/applied"),
        emp_dir.join("prompt-update-requests/rejected"),
        emp_dir.join("prompt-update-requests/failed"),
        emp_dir.join("logs/dream"),
    ] {
        fs::create_dir_all(&sub)
            .map_err(|e| format!("创建员工目录 {} 失败: {e}", sub.display()))?;
    }

    let now = iso_now();
    let manifest = EmployeeManifest {
        id: id.clone(),
        name: input.name.clone(),
        description: input.description.clone(),
        kind: EmployeeKind::Ordinary,
        status: EmployeeStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    write_manifest(root, &id, &manifest)?;

    // prompt.md
    let prompt = emp_dir.join(PROMPT_FILE);
    if !prompt.is_file() {
        fs::write(&prompt, input.initial_prompt.as_str())
            .map_err(|e| format!("创建 prompt.md 失败: {e}"))?;
    }

    // skills.json — initialize before copying root skills
    if !emp_dir.join("skills.json").is_file() {
        write_skills_registry(root, &id, &SkillRegistry::empty())?;
    }

    for skill_id in &input.root_skill_ids {
        copy_root_skill_to_employee(root, &id, skill_id)?;
    }

    let dream_yaml = emp_dir.join("dream.yaml");
    if !dream_yaml.is_file() {
        fs::write(&dream_yaml, DEFAULT_DREAM_YAML)
            .map_err(|e| format!("创建 dream.yaml 失败: {e}"))?;
    }

    // workspaces.json
    let ws_json = emp_dir.join("workspaces.json");
    if !ws_json.is_file() {
        let wml = WorkspaceMembershipList {
            version: 1,
            workspaces: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&wml)
            .map_err(|e| format!("序列化 workspaces.json 失败: {e}"))?;
        fs::write(&ws_json, json).map_err(|e| format!("写入 workspaces.json 失败: {e}"))?;
    }

    // Registry
    reg.employees.push(RegistryEntry {
        id: id.clone(),
        kind: EmployeeKind::Ordinary,
        name: input.name,
        path: format!("employees/{id}"),
        status: EmployeeStatus::Active,
    });
    write_registry(root, &reg)?;

    get_detail_with_registry(root, &reg, &id)
}

pub fn install_prepared_employee(
    root: &RootWorkspace,
    input: PreparedEmployeeInstall,
) -> Result<EmployeeDetail, String> {
    let id = resolve_new_employee_id(&input.id)?;
    let mut reg = read_registry(root)?;
    if reg.find(&id).is_some() {
        if input.hub_origin.is_none() {
            return Err(format!("员工 {id} 已存在"));
        }
        let emp_dir = employee_dir(root, &id);
        if emp_dir.exists() {
            fs::remove_dir_all(&emp_dir)
                .map_err(|e| format!("替换员工目录 {} 失败: {e}", emp_dir.display()))?;
        }
        reg.employees.retain(|entry| entry.id != id);
    }

    let emp_dir = employee_dir(root, &id);
    for sub in [
        emp_dir.clone(),
        emp_dir.join("skills"),
        emp_dir.join("prompt-update-requests/pending"),
        emp_dir.join("prompt-update-requests/approved"),
        emp_dir.join("prompt-update-requests/applied"),
        emp_dir.join("prompt-update-requests/rejected"),
        emp_dir.join("prompt-update-requests/failed"),
        emp_dir.join("logs/dream"),
    ] {
        fs::create_dir_all(&sub)
            .map_err(|e| format!("创建员工目录 {} 失败: {e}", sub.display()))?;
    }

    let now = iso_now();
    let manifest = EmployeeManifest {
        id: id.clone(),
        name: input.name.clone(),
        description: input.description.clone(),
        kind: EmployeeKind::Ordinary,
        status: EmployeeStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    write_manifest(root, &id, &manifest)?;

    fs::write(emp_dir.join(PROMPT_FILE), input.prompt_md.as_str())
        .map_err(|e| format!("写入员工 prompt.md 失败: {e}"))?;

    write_skills_registry(root, &id, &SkillRegistry::empty())?;

    for skill_id in &input.root_skill_ids {
        copy_root_skill_to_employee(root, &id, skill_id)?;
    }

    let dream_yaml = emp_dir.join("dream.yaml");
    if !dream_yaml.is_file() {
        fs::write(&dream_yaml, DEFAULT_DREAM_YAML)
            .map_err(|e| format!("创建 dream.yaml 失败: {e}"))?;
    }

    let wml = WorkspaceMembershipList {
        version: 1,
        workspaces: Vec::new(),
    };
    write_workspace_memberships(root, &id, &wml)?;

    if let Some(origin) = input.hub_origin {
        crate::services::hub_state::write_origin(&emp_dir, &origin)?;
    }

    reg.employees.push(RegistryEntry {
        id: id.clone(),
        kind: EmployeeKind::Ordinary,
        name: input.name,
        path: format!("employees/{id}"),
        status: EmployeeStatus::Active,
    });
    write_registry(root, &reg)?;

    get_detail_with_registry(root, &reg, &id)
}

/// Input for updating employee metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEmployeeInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<EmployeeStatus>,
}

/// Update metadata (name, description, status) for an existing employee.
pub fn update_metadata(
    root: &RootWorkspace,
    id: &str,
    input: UpdateEmployeeInput,
) -> Result<EmployeeDetail, String> {
    let mut reg = read_registry(root)?;
    let _entry = reg.find(id).ok_or_else(|| format!("员工 {id} 不存在"))?;

    let mut manifest =
        read_manifest(root, id)?.ok_or_else(|| format!("员工 {id} 的 manifest 不存在"))?;

    if let Some(name) = input.name {
        let name = name.trim().to_string();
        if !name.is_empty() {
            manifest.name = name.clone();
            if let Some(entry) = reg.find_mut(id) {
                entry.name = name;
            }
        }
    }
    if let Some(desc) = input.description {
        manifest.description = desc;
    }
    if let Some(status) = input.status {
        manifest.status = status;
        if let Some(entry) = reg.find_mut(id) {
            entry.status = status;
        }
    }
    manifest.updated_at = iso_now();

    write_manifest(root, id, &manifest)?;
    write_registry(root, &reg)?;

    get_detail_with_registry(root, &reg, id)
}

/// Delete an ordinary employee, unbinding all workspaces first.
pub fn delete_employee(root: &RootWorkspace, id: &str) -> Result<Vec<PathBuf>, String> {
    if id == DREAM_EMPLOYEE_ID || id == GENERAL_EMPLOYEE_ID {
        return Err(format!("不能删除系统员工 {id}"));
    }
    let reg = read_registry(root)?;
    let entry = reg
        .find(id)
        .ok_or_else(|| format!("员工 {id} 不存在"))?;
    if entry.kind == EmployeeKind::Dream {
        return Err("不能删除 Dream 员工".to_string());
    }

    let workspace_paths: Vec<PathBuf> = read_workspace_memberships(root, id)
        .map(|memberships| {
            memberships
                .workspaces
                .iter()
                .map(|membership| PathBuf::from(&membership.path))
                .collect()
        })
        .unwrap_or_default();

    for workspace_path in &workspace_paths {
        let _ = unbind_workspace(root, workspace_path);
    }

    let dir = employee_dir(root, id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| format!("删除员工目录 {} 失败: {e}", dir.display()))?;
    }

    let mut reg = read_registry(root)?;
    reg.employees.retain(|employee| employee.id != id);
    write_registry(root, &reg)?;

    Ok(workspace_paths)
}

/// Check integrity of a single employee.
pub fn check_integrity(root: &RootWorkspace, id: &str) -> Result<IntegrityReport, String> {
    let reg = read_registry(root)?;
    Ok(check_integrity_inner(root, &reg, id))
}

fn check_integrity_inner(
    root: &RootWorkspace,
    reg: &EmployeeRegistry,
    id: &str,
) -> IntegrityReport {
    let mut issues = Vec::new();

    // 1) Registry entry exists
    let entry = match reg.find(id) {
        Some(e) => Some(e),
        None => {
            issues.push(IntegrityIssue {
                code: "registry_missing".to_string(),
                message: format!("员工 {id} 不在 registry 中"),
            });
            None
        }
    };

    // 2) Employee directory exists
    let dir = employee_dir(root, id);
    if !dir.is_dir() {
        issues.push(IntegrityIssue {
            code: "dir_missing".to_string(),
            message: format!("员工目录 {} 不存在", dir.display()),
        });
    }

    // 3) Manifest exists
    let mp = manifest_path(root, id);
    let manifest = if mp.is_file() {
        match read_manifest(root, id) {
            Ok(Some(m)) => Some(m),
            Ok(None) => {
                issues.push(IntegrityIssue {
                    code: "manifest_missing".to_string(),
                    message: format!("员工 {id} 的 manifest 文件不存在"),
                });
                None
            }
            Err(e) => {
                issues.push(IntegrityIssue {
                    code: "manifest_invalid".to_string(),
                    message: format!("员工 {id} 的 manifest 解析失败: {e}"),
                });
                None
            }
        }
    } else {
        issues.push(IntegrityIssue {
            code: "manifest_missing".to_string(),
            message: format!("员工 {id} 的 manifest 文件不存在"),
        });
        None
    };

    // 4) Manifest id/kind match registry
    if let (Some(entry), Some(manifest)) = (entry, &manifest) {
        if manifest.id != entry.id {
            issues.push(IntegrityIssue {
                code: "id_mismatch".to_string(),
                message: format!(
                    "manifest.id ({}) 与 registry.id ({}) 不一致",
                    manifest.id, entry.id
                ),
            });
        }
        if manifest.kind != entry.kind {
            issues.push(IntegrityIssue {
                code: "kind_mismatch".to_string(),
                message: format!(
                    "manifest.kind ({:?}) 与 registry.kind ({:?}) 不一致",
                    manifest.kind, entry.kind
                ),
            });
        }
    }

    // 5) Dream employee should NOT have dream.yaml
    if id == DREAM_EMPLOYEE_ID {
        let dream_yaml = dir.join("dream.yaml");
        if dream_yaml.is_file() {
            issues.push(IntegrityIssue {
                code: "dream_has_dream_yaml".to_string(),
                message: "Dream Workflow 不应包含 dream.yaml".to_string(),
            });
        }
    }

    let status = if issues.is_empty() {
        IntegrityStatus::Ok
    } else {
        IntegrityStatus::RepairNeeded
    };

    IntegrityReport {
        employee_id: id.to_string(),
        status,
        issues,
    }
}

fn employee_skills_dir(root: &RootWorkspace, id: &str) -> PathBuf {
    employee_dir(root, id).join("skills")
}

// ── Employee Skill Operations ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EmployeeSkillSummary {
    pub id: String,
    pub source: String,
    pub copied_from: Option<String>,
    pub enabled: bool,
    pub name: String,
    pub description: String,
    pub path: String,
    pub has_skill_md: bool,
}

/// Read employee prompt.md content.
pub fn read_employee_prompt(root: &RootWorkspace, id: &str) -> Result<String, String> {
    let path = employee_dir(root, id).join(PROMPT_FILE);
    if !path.is_file() {
        return Ok(String::new());
    }
    fs::read_to_string(&path).map_err(|e| format!("读取员工 prompt ({id}) 失败: {e}"))
}

/// Write employee prompt.md content.
pub fn write_employee_prompt(root: &RootWorkspace, id: &str, content: &str) -> Result<(), String> {
    let registry = read_registry(root)?;
    if registry.find(id).is_none() {
        return Err(format!("员工不存在: {id}"));
    }

    let path = employee_dir(root, id).join(PROMPT_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建员工 prompt 目录 ({id}) 失败: {e}"))?;
    }
    let tmp_path = path.with_extension("md.tmp");
    fs::write(&tmp_path, content)
        .map_err(|e| format!("写入员工 prompt 临时文件 ({id}) 失败: {e}"))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("替换员工 prompt ({id}) 失败: {e}"))
}

pub fn read_skills_registry(root: &RootWorkspace, id: &str) -> Result<SkillRegistry, String> {
    let path = skills_path(root, id);
    if !path.is_file() {
        return Ok(SkillRegistry::empty());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("读取员工 {id} 的 skills.json 失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析员工 {id} 的 skills.json 失败: {e}"))
}

fn read_skill_frontmatter(skill_md_path: &Path) -> (String, String) {
    let content = match fs::read_to_string(skill_md_path) {
        Ok(c) => c,
        Err(_) => return (String::new(), String::new()),
    };
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (String::new(), String::new());
    }
    let mut lines = trimmed.lines();
    lines.next(); // skip opening ---
    let mut yaml_buf = String::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        if !yaml_buf.is_empty() {
            yaml_buf.push('\n');
        }
        yaml_buf.push_str(line);
    }
    if !closed || yaml_buf.trim().is_empty() {
        return (String::new(), String::new());
    }
    #[derive(Deserialize)]
    struct Fm {
        name: Option<String>,
        description: Option<String>,
    }
    match serde_yaml::from_str::<Fm>(&yaml_buf) {
        Ok(fm) => (
            fm.name.unwrap_or_default(),
            fm.description.unwrap_or_default(),
        ),
        Err(_) => (String::new(), String::new()),
    }
}

fn build_employee_skill_summary(skill_dir: &Path, entry: &SkillEntry) -> EmployeeSkillSummary {
    let skill_md = skill_dir.join("SKILL.md");
    let has_skill_md = skill_md.is_file();
    let (name, description) = if has_skill_md {
        read_skill_frontmatter(&skill_md)
    } else {
        (String::new(), String::new())
    };
    let name = if name.is_empty() {
        entry.id.clone()
    } else {
        name
    };
    EmployeeSkillSummary {
        id: entry.id.clone(),
        source: entry.source.clone(),
        copied_from: entry.copied_from.clone(),
        enabled: entry.enabled,
        name,
        description,
        path: skill_dir.to_string_lossy().into_owned(),
        has_skill_md,
    }
}

pub fn list_employee_skills(
    root: &RootWorkspace,
    id: &str,
) -> Result<Vec<EmployeeSkillSummary>, String> {
    let reg = read_skills_registry(root, id)?;
    let skills_dir = employee_skills_dir(root, id);
    Ok(reg
        .skills
        .iter()
        .map(|entry| {
            let skill_dir = skills_dir.join(&entry.id);
            build_employee_skill_summary(&skill_dir, entry)
        })
        .collect())
}

fn copy_dir_all_employee(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("源目录不存在: {}", src.display()));
    }
    fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {e}"))?;
    for entry in fs::read_dir(src).map_err(|e| format!("读取源目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all_employee(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("复制文件失败: {e}"))?;
        }
    }
    Ok(())
}

pub fn copy_root_skill_to_employee(
    root: &RootWorkspace,
    employee_id: &str,
    skill_id: &str,
) -> Result<EmployeeSkillSummary, String> {
    let root_skill_dir = root.skills_dir().join(skill_id);
    if !root_skill_dir.join("SKILL.md").is_file() {
        return Err(format!("根目录技能 {skill_id} 不存在"));
    }

    let mut sr = read_skills_registry(root, employee_id)?;
    if sr.skills.iter().any(|e| e.id == skill_id) {
        return Err(format!("员工已拥有技能 {skill_id}，如需覆盖请先删除再添加"));
    }

    let dst = employee_skills_dir(root, employee_id).join(skill_id);
    fs::create_dir_all(employee_skills_dir(root, employee_id))
        .map_err(|e| format!("创建员工 skills 目录失败: {e}"))?;
    copy_dir_all_employee(&root_skill_dir, &dst)?;

    let entry = SkillEntry {
        id: skill_id.to_string(),
        source: "root".to_string(),
        copied_from: Some(skill_id.to_string()),
        enabled: true,
    };
    sr.skills.push(entry.clone());
    write_skills_registry(root, employee_id, &sr)?;

    Ok(build_employee_skill_summary(&dst, &entry))
}

pub fn toggle_employee_skill(
    root: &RootWorkspace,
    employee_id: &str,
    skill_id: &str,
    enabled: bool,
) -> Result<SkillRegistry, String> {
    let mut sr = read_skills_registry(root, employee_id)?;
    let entry = sr
        .skills
        .iter_mut()
        .find(|e| e.id == skill_id)
        .ok_or_else(|| format!("员工技能 {skill_id} 不存在"))?;
    entry.enabled = enabled;
    write_skills_registry(root, employee_id, &sr)?;
    Ok(sr)
}

pub fn delete_employee_skill(
    root: &RootWorkspace,
    employee_id: &str,
    skill_id: &str,
) -> Result<SkillRegistry, String> {
    let mut sr = read_skills_registry(root, employee_id)?;
    let before_len = sr.skills.len();
    sr.skills.retain(|e| e.id != skill_id);
    if sr.skills.len() == before_len {
        return Err(format!("员工技能 {skill_id} 不存在"));
    }

    let skill_dir = employee_skills_dir(root, employee_id).join(skill_id);
    if skill_dir.is_dir() {
        fs::remove_dir_all(&skill_dir).map_err(|e| format!("删除技能目录失败: {e}"))?;
    }

    write_skills_registry(root, employee_id, &sr)?;
    Ok(sr)
}

// ── Workspace Binding I/O ──────────────────────────────────────────────────

fn workspace_binding_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(WORKSPACE_BINDING_FILE)
}

fn workspaces_membership_path(root: &RootWorkspace, employee_id: &str) -> PathBuf {
    employee_dir(root, employee_id).join(WORKSPACES_MEMBERSHIP_FILE)
}

pub fn read_workspace_binding(workspace_path: &Path) -> Option<WorkspaceBinding> {
    let path = workspace_binding_path(workspace_path);
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Quick lookup: returns the bound employee id if the workspace is bound, or None.
pub fn bound_employee_id(workspace_path: &Path) -> Option<String> {
    read_workspace_binding(workspace_path).map(|b| b.employee_id)
}

/// Quick lookup: returns the bound employee's display name if the workspace is bound, or None.
/// Performs only file reads (no full validation). Suitable for workspace listing.
pub fn bound_employee_name(root: &RootWorkspace, workspace_path: &Path) -> Option<String> {
    let binding = read_workspace_binding(workspace_path)?;
    let manifest = read_manifest(root, &binding.employee_id).ok()??;
    Some(manifest.name)
}

fn write_workspace_binding(
    workspace_path: &Path,
    binding: &WorkspaceBinding,
) -> Result<(), String> {
    let path = workspace_binding_path(workspace_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 .chawork 目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(binding)
        .map_err(|e| format!("序列化 workspace binding 失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入 workspace binding 失败: {e}"))
}

fn remove_workspace_binding(workspace_path: &Path) -> Result<(), String> {
    let path = workspace_binding_path(workspace_path);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("删除 workspace binding 失败: {e}"))?;
    }
    Ok(())
}

pub fn read_workspace_memberships(
    root: &RootWorkspace,
    employee_id: &str,
) -> Result<WorkspaceMembershipList, String> {
    let path = workspaces_membership_path(root, employee_id);
    if !path.is_file() {
        return Ok(WorkspaceMembershipList {
            version: 1,
            workspaces: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取员工 {employee_id} 的 workspaces.json 失败: {e}"))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("解析员工 {employee_id} 的 workspaces.json 失败: {e}"))
}

fn write_workspace_memberships(
    root: &RootWorkspace,
    employee_id: &str,
    wml: &WorkspaceMembershipList,
) -> Result<(), String> {
    let path = workspaces_membership_path(root, employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 workspaces.json 目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(wml)
        .map_err(|e| format!("序列化 workspaces.json 失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入 workspaces.json 失败: {e}"))
}

fn canonical_path_string(p: &Path) -> String {
    fs::canonicalize(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn read_workspace_id(workspace_path: &Path) -> Option<String> {
    let state_path = workspace_path.join(".chawork/state/workspace.json");
    let raw = fs::read_to_string(&state_path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("id")?.as_str().map(|s| s.to_string())
}

// ── Workspace Binding Public API ───────────────────────────────────────────

/// Validate binding state for a workspace.
pub fn validate_binding(
    root: &RootWorkspace,
    workspace_path: &Path,
) -> Result<BindingValidation, String> {
    let binding = match read_workspace_binding(workspace_path) {
        Some(b) => b,
        None => {
            return Ok(BindingValidation {
                status: BindingStatus::Unbound,
                employee_id: None,
                employee_name: None,
                message: "该 workspace 尚未绑定到任何员工".to_string(),
            });
        }
    };

    let employee_id = &binding.employee_id;
    let reg = read_registry(root)?;

    let entry = match reg.find(employee_id) {
        Some(e) => e,
        None => {
            return Ok(BindingValidation {
                status: BindingStatus::EmployeeMissing,
                employee_id: Some(employee_id.clone()),
                employee_name: None,
                message: format!("绑定目标员工 {employee_id} 不存在（可能已被删除）"),
            });
        }
    };

    let wml = read_workspace_memberships(root, employee_id)?;
    let ws_canonical = canonical_path_string(workspace_path);

    let membership_by_path = wml
        .workspaces
        .iter()
        .find(|m| canonical_path_string(Path::new(&m.path)) == ws_canonical);

    if membership_by_path.is_some() {
        return Ok(BindingValidation {
            status: BindingStatus::Bound,
            employee_id: Some(employee_id.clone()),
            employee_name: Some(entry.name.clone()),
            message: format!("已绑定到员工 {}", entry.name),
        });
    }

    if let Some(ws_id) = read_workspace_id(workspace_path) {
        if let Some(m) = wml.workspaces.iter().find(|m| m.id == ws_id) {
            return Ok(BindingValidation {
                status: BindingStatus::PathMismatch,
                employee_id: Some(employee_id.clone()),
                employee_name: Some(entry.name.clone()),
                message: format!(
                    "工作区路径已变更：员工记录为 {}，当前为 {}。请重新绑定。",
                    m.path, ws_canonical
                ),
            });
        }
    }

    Ok(BindingValidation {
        status: BindingStatus::MembershipMissing,
        employee_id: Some(employee_id.clone()),
        employee_name: Some(entry.name.clone()),
        message: format!(
            "workspace 声明绑定到 {employee_id}，但该员工的 workspaces.json 中无此 workspace"
        ),
    })
}

/// Bind a workspace to an employee. Writes `.chawork/employee.json` and updates `workspaces.json`.
pub fn bind_workspace(
    root: &RootWorkspace,
    employee_id: &str,
    workspace_path: &Path,
    workspace_id: &str,
    workspace_name: &str,
) -> Result<BindingValidation, String> {
    let reg = read_registry(root)?;
    let entry = reg
        .find(employee_id)
        .ok_or_else(|| format!("员工 {employee_id} 不存在"))?;

    if entry.kind == EmployeeKind::Dream {
        return Err("不能将 workspace 绑定到 Dream 员工".to_string());
    }

    if let Some(existing) = read_workspace_binding(workspace_path) {
        if existing.employee_id != employee_id {
            return Err(format!(
                "该 workspace 已绑定到员工 {}，请先解绑",
                existing.employee_id
            ));
        }
    }

    write_workspace_binding(
        workspace_path,
        &WorkspaceBinding {
            employee_id: employee_id.to_string(),
        },
    )?;

    let mut wml = read_workspace_memberships(root, employee_id)?;
    let ws_canonical = canonical_path_string(workspace_path);

    let existing_idx = wml
        .workspaces
        .iter()
        .position(|m| canonical_path_string(Path::new(&m.path)) == ws_canonical);

    match existing_idx {
        Some(i) => {
            wml.workspaces[i].name = workspace_name.to_string();
            wml.workspaces[i].path = ws_canonical;
        }
        None => {
            wml.workspaces.push(WorkspaceMembership {
                id: workspace_id.to_string(),
                path: ws_canonical,
                name: workspace_name.to_string(),
                added_at: iso_now(),
            });
        }
    }

    write_workspace_memberships(root, employee_id, &wml)?;

    Ok(BindingValidation {
        status: BindingStatus::Bound,
        employee_id: Some(employee_id.to_string()),
        employee_name: Some(entry.name.clone()),
        message: format!("已绑定到员工 {}", entry.name),
    })
}

/// Unbind a workspace from its employee. Clears `.chawork/employee.json` and removes from `workspaces.json`.
pub fn unbind_workspace(root: &RootWorkspace, workspace_path: &Path) -> Result<(), String> {
    let binding = match read_workspace_binding(workspace_path) {
        Some(b) => b,
        None => return Ok(()),
    };

    remove_workspace_binding(workspace_path)?;

    let employee_id = &binding.employee_id;
    let mut wml = read_workspace_memberships(root, employee_id)?;
    let ws_canonical = canonical_path_string(workspace_path);

    wml.workspaces
        .retain(|m| canonical_path_string(Path::new(&m.path)) != ws_canonical);

    write_workspace_memberships(root, employee_id, &wml)?;

    Ok(())
}

/// List all workspace memberships for an employee.
/// Reconciles `workspaces.json` against known workspaces and on-disk `.chawork/employee.json` bindings.
pub fn list_workspaces_for_employee(
    root: &RootWorkspace,
    employee_id: &str,
) -> Result<Vec<WorkspaceMembership>, String> {
    let reg = read_registry(root)?;
    reg.find(employee_id)
        .ok_or_else(|| format!("员工 {employee_id} 不存在"))?;

    let mut wml = read_workspace_memberships(root, employee_id)?;
    let known = crate::services::workspace::list_known(&root.known_workspaces_path());

    let mut merged = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();

    let mut push_membership = |membership: WorkspaceMembership| {
        let key = canonical_path_string(Path::new(&membership.path));
        if seen_keys.insert(key) {
            merged.push(membership);
        }
    };

    for ws in known {
        let ws_path = Path::new(&ws.path);
        let Some(binding) = read_workspace_binding(ws_path) else {
            continue;
        };
        if binding.employee_id != employee_id {
            continue;
        }
        let ws_canonical = canonical_path_string(ws_path);
        let existing = wml
            .workspaces
            .iter()
            .find(|m| canonical_path_string(Path::new(&m.path)) == ws_canonical);
        push_membership(WorkspaceMembership {
            id: ws.id.clone(),
            path: ws_canonical,
            name: ws.name.clone(),
            added_at: existing.map(|m| m.added_at.clone()).unwrap_or_else(iso_now),
        });
    }

    for membership in &wml.workspaces {
        let ws_path = Path::new(&membership.path);
        let Some(binding) = read_workspace_binding(ws_path) else {
            continue;
        };
        if binding.employee_id != employee_id {
            continue;
        }
        push_membership(WorkspaceMembership {
            id: membership.id.clone(),
            path: canonical_path_string(ws_path),
            name: membership.name.clone(),
            added_at: membership.added_at.clone(),
        });
    }

    wml.workspaces = merged.clone();
    write_workspace_memberships(root, employee_id, &wml)?;

    Ok(merged)
}

fn resolve_new_employee_id(provided: &str) -> Result<String, String> {
    let id = if provided.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        provided.trim().to_string()
    };
    if id == DREAM_EMPLOYEE_ID || id == GENERAL_EMPLOYEE_ID {
        return Err(format!("不能创建保留 ID {id}"));
    }
    if !is_valid_employee_id(&id) {
        return Err("员工 ID 只允许小写字母、数字和连字符（kebab-case）".to_string());
    }
    Ok(id)
}

fn is_valid_employee_id(id: &str) -> bool {
    if id.is_empty() || id.starts_with('-') || id.ends_with('-') {
        return false;
    }
    id.chars().all(|c| {
        c == '-'
            || c.is_ascii_digit()
            || (c.is_ascii_lowercase())
            || (c.is_alphanumeric() && !c.is_ascii())
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::root_workspace;

    fn setup() -> (tempfile::TempDir, RootWorkspace) {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        (tmp, root)
    }

    #[test]
    fn init_creates_builtin_employees() {
        let (_tmp, root) = setup();

        assert!(root.dream_employee_dir().is_dir());
        assert!(manifest_path(&root, DREAM_EMPLOYEE_ID).is_file());
        assert!(skills_path(&root, DREAM_EMPLOYEE_ID).is_file());
        assert!(root.dream_employee_dir().join("prompt.md").is_file());
        assert!(root.dream_employee_dir().join("skills").is_dir());
        assert!(root.dream_employee_dir().join("workspaces").is_dir());
        assert!(root.dream_employee_dir().join("logs/dream").is_dir());
        assert!(root.employee_registry_path().is_file());

        let reg = read_registry(&root).expect("read registry");
        assert!(reg.find(DREAM_EMPLOYEE_ID).is_some());
        let entry = reg.find(DREAM_EMPLOYEE_ID).unwrap();
        assert_eq!(entry.kind, EmployeeKind::Dream);
        assert_eq!(entry.status, EmployeeStatus::Active);

        assert!(employee_dir(&root, GENERAL_EMPLOYEE_ID).is_dir());
        assert!(manifest_path(&root, GENERAL_EMPLOYEE_ID).is_file());
        assert!(skills_path(&root, GENERAL_EMPLOYEE_ID).is_file());
        assert!(employee_dir(&root, GENERAL_EMPLOYEE_ID)
            .join("prompt.md")
            .is_file());
        let general_prompt = read_employee_prompt(&root, GENERAL_EMPLOYEE_ID).expect("read prompt");
        assert!(
            general_prompt.contains("默认通用员工"),
            "general prompt should be seeded"
        );
        assert!(employee_dir(&root, GENERAL_EMPLOYEE_ID)
            .join("dream.yaml")
            .is_file());
        assert!(employee_dir(&root, GENERAL_EMPLOYEE_ID)
            .join("workspaces.json")
            .is_file());

        let general = reg.find(GENERAL_EMPLOYEE_ID).expect("general in registry");
        assert_eq!(general.kind, EmployeeKind::Ordinary);
        assert_eq!(general.status, EmployeeStatus::Active);
    }

    #[test]
    fn init_repairs_empty_general_prompt_without_overwriting_custom_prompt() {
        let (_tmp, root) = setup();
        let prompt_path = employee_dir(&root, GENERAL_EMPLOYEE_ID).join("prompt.md");

        fs::write(&prompt_path, " \n\t").expect("blank prompt");
        ensure_employee_infrastructure(&root).expect("repair blank prompt");
        let repaired = read_employee_prompt(&root, GENERAL_EMPLOYEE_ID).expect("read repaired");
        assert!(repaired.contains("默认通用员工"));

        fs::write(&prompt_path, "Custom prompt").expect("custom prompt");
        ensure_employee_infrastructure(&root).expect("preserve custom prompt");
        let preserved = read_employee_prompt(&root, GENERAL_EMPLOYEE_ID).expect("read preserved");
        assert_eq!(preserved, "Custom prompt");
    }

    #[test]
    fn init_is_idempotent() {
        let (_tmp, root) = setup();

        let reg1 = read_registry(&root).expect("read registry");
        let dream_count_1 = reg1
            .employees
            .iter()
            .filter(|e| e.id == DREAM_EMPLOYEE_ID)
            .count();
        assert_eq!(dream_count_1, 1);
        let general_count_1 = reg1
            .employees
            .iter()
            .filter(|e| e.id == GENERAL_EMPLOYEE_ID)
            .count();
        assert_eq!(general_count_1, 1);

        // Run init again
        ensure_employee_infrastructure(&root).expect("second init");

        let reg2 = read_registry(&root).expect("read registry");
        let dream_count_2 = reg2
            .employees
            .iter()
            .filter(|e| e.id == DREAM_EMPLOYEE_ID)
            .count();
        assert_eq!(dream_count_2, 1);
        let general_count_2 = reg2
            .employees
            .iter()
            .filter(|e| e.id == GENERAL_EMPLOYEE_ID)
            .count();
        assert_eq!(general_count_2, 1);
    }

    #[test]
    fn init_repairs_missing_general_registry_entry() {
        let (_tmp, root) = setup();
        let mut reg = read_registry(&root).expect("read registry");
        reg.employees.retain(|e| e.id != GENERAL_EMPLOYEE_ID);
        write_registry(&root, &reg).expect("write registry");

        ensure_employee_infrastructure(&root).expect("repair init");

        let repaired = read_registry(&root).expect("read repaired registry");
        let general = repaired
            .find(GENERAL_EMPLOYEE_ID)
            .expect("general repaired in registry");
        assert_eq!(general.kind, EmployeeKind::Ordinary);
        assert_eq!(general.status, EmployeeStatus::Active);
    }

    #[test]
    fn init_repairs_missing_general_manifest() {
        let (_tmp, root) = setup();
        fs::remove_file(manifest_path(&root, GENERAL_EMPLOYEE_ID)).expect("remove manifest");

        ensure_employee_infrastructure(&root).expect("repair init");

        let manifest = read_manifest(&root, GENERAL_EMPLOYEE_ID)
            .expect("read manifest")
            .expect("manifest exists");
        assert_eq!(manifest.id, GENERAL_EMPLOYEE_ID);
        assert_eq!(manifest.kind, EmployeeKind::Ordinary);
        assert_eq!(manifest.status, EmployeeStatus::Active);
    }

    #[test]
    fn general_employee_can_bind_workspace() {
        let (tmp, root) = setup();
        let ws_path = tmp.path().join("general-ws");
        fs::create_dir_all(&ws_path).expect("create workspace");

        let validation = bind_workspace(
            &root,
            GENERAL_EMPLOYEE_ID,
            &ws_path,
            "general-ws",
            "General Workspace",
        )
        .expect("bind general");

        assert_eq!(validation.status, BindingStatus::Bound);
        assert_eq!(validation.employee_id.as_deref(), Some(GENERAL_EMPLOYEE_ID));
    }

    #[test]
    fn create_ordinary_employee() {
        let (_tmp, root) = setup();

        let detail = create(
            &root,
            CreateEmployeeInput {
                id: "ip-screening".to_string(),
                name: "IP 筛选".to_string(),
                description: "筛选、评估和跟进潜在 IP".to_string(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create employee");

        assert_eq!(detail.registry_entry.id, "ip-screening");
        assert_eq!(detail.registry_entry.kind, EmployeeKind::Ordinary);

        let dir = employee_dir(&root, "ip-screening");
        assert!(dir.is_dir());
        assert!(dir.join("employee.yaml").is_file());
        assert!(dir.join("prompt.md").is_file());
        assert!(dir.join("skills.json").is_file());
        assert!(dir.join("dream.yaml").is_file());
        assert!(dir.join("workspaces.json").is_file());
        assert!(dir.join("skills").is_dir());
        assert!(dir.join("prompt-update-requests/pending").is_dir());
        assert!(dir.join("prompt-update-requests/approved").is_dir());
        assert!(dir.join("prompt-update-requests/applied").is_dir());
        assert!(dir.join("prompt-update-requests/rejected").is_dir());
        assert!(dir.join("prompt-update-requests/failed").is_dir());
        assert!(dir.join("logs/dream").is_dir());

        let manifest = read_manifest(&root, "ip-screening")
            .expect("read manifest")
            .expect("manifest exists");
        assert_eq!(manifest.id, "ip-screening");
        assert_eq!(manifest.kind, EmployeeKind::Ordinary);
    }

    #[test]
    fn create_duplicate_employee_fails() {
        let (_tmp, root) = setup();

        create(
            &root,
            CreateEmployeeInput {
                id: "test-emp".to_string(),
                name: "Test".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("first create");

        let err = create(
            &root,
            CreateEmployeeInput {
                id: "test-emp".to_string(),
                name: "Test 2".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .unwrap_err();
        assert!(err.contains("已存在"));
    }

    #[test]
    fn create_dream_id_rejected() {
        let (_tmp, root) = setup();
        let err = create(
            &root,
            CreateEmployeeInput {
                id: DREAM_EMPLOYEE_ID.to_string(),
                name: "Nope".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .unwrap_err();
        assert!(err.contains("保留 ID"));
    }

    #[test]
    fn invalid_employee_id_rejected() {
        let (_tmp, root) = setup();
        for bad_id in &[
            "Hello",
            "has space",
            "-leading",
            "trailing-",
            "UPPER",
            "under_score",
        ] {
            let err = create(
                &root,
                CreateEmployeeInput {
                    id: bad_id.to_string(),
                    name: "Bad".to_string(),
                    description: String::new(),
                    initial_prompt: String::new(),
                    root_skill_ids: vec![],
                },
            )
            .unwrap_err();
            assert!(
                err.contains("kebab-case"),
                "expected kebab-case error for {bad_id}, got: {err}"
            );
        }
    }

    #[test]
    fn list_employees_uses_registry() {
        let (_tmp, root) = setup();

        let entries = list(&root).expect("list");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.id == DREAM_EMPLOYEE_ID));
        assert!(entries.iter().any(|e| e.id == GENERAL_EMPLOYEE_ID));

        create(
            &root,
            CreateEmployeeInput {
                id: "emp-a".to_string(),
                name: "A".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create a");

        let entries = list(&root).expect("list");
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.id == "emp-a"));
    }

    #[test]
    fn update_metadata_works() {
        let (_tmp, root) = setup();

        create(
            &root,
            CreateEmployeeInput {
                id: "updatable".to_string(),
                name: "Before".to_string(),
                description: "old".to_string(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create");

        let detail = update_metadata(
            &root,
            "updatable",
            UpdateEmployeeInput {
                name: Some("After".to_string()),
                description: Some("new desc".to_string()),
                status: None,
            },
        )
        .expect("update");

        assert_eq!(detail.registry_entry.name, "After");
        let m = detail.manifest.expect("manifest");
        assert_eq!(m.name, "After");
        assert_eq!(m.description, "new desc");
    }

    #[test]
    fn integrity_ok_for_healthy_employee() {
        let (_tmp, root) = setup();

        create(
            &root,
            CreateEmployeeInput {
                id: "healthy".to_string(),
                name: "Healthy".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create");

        let report = check_integrity(&root, "healthy").expect("check");
        assert!(matches!(report.status, IntegrityStatus::Ok));
        assert!(report.issues.is_empty());
    }

    #[test]
    fn integrity_detects_missing_manifest() {
        let (_tmp, root) = setup();

        create(
            &root,
            CreateEmployeeInput {
                id: "broken".to_string(),
                name: "Broken".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create");

        // Delete manifest
        fs::remove_file(manifest_path(&root, "broken")).expect("rm manifest");

        let report = check_integrity(&root, "broken").expect("check");
        assert!(matches!(report.status, IntegrityStatus::RepairNeeded));
        assert!(report.issues.iter().any(|i| i.code == "manifest_missing"));
    }

    #[test]
    fn integrity_detects_dream_with_dream_yaml() {
        let (_tmp, root) = setup();

        // Create a dream.yaml in __dream__ (shouldn't exist)
        let bad_file = root.dream_employee_dir().join("dream.yaml");
        fs::write(&bad_file, "bad").expect("write");

        let report = check_integrity(&root, DREAM_EMPLOYEE_ID).expect("check");
        assert!(matches!(report.status, IntegrityStatus::RepairNeeded));
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "dream_has_dream_yaml"));
    }

    #[test]
    fn integrity_detects_kind_mismatch() {
        let (_tmp, root) = setup();

        create(
            &root,
            CreateEmployeeInput {
                id: "mismatch".to_string(),
                name: "Mismatch".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create");

        // Tamper manifest kind
        let mut m = read_manifest(&root, "mismatch")
            .expect("read")
            .expect("exists");
        m.kind = EmployeeKind::Dream;
        write_manifest(&root, "mismatch", &m).expect("write");

        let report = check_integrity(&root, "mismatch").expect("check");
        assert!(matches!(report.status, IntegrityStatus::RepairNeeded));
        assert!(report.issues.iter().any(|i| i.code == "kind_mismatch"));
    }

    #[test]
    fn get_detail_returns_full_info() {
        let (_tmp, root) = setup();

        create(
            &root,
            CreateEmployeeInput {
                id: "detail-test".to_string(),
                name: "Detail".to_string(),
                description: "desc".to_string(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create");

        let detail = get_detail(&root, "detail-test").expect("detail");
        assert_eq!(detail.registry_entry.id, "detail-test");
        let m = detail.manifest.expect("manifest present");
        assert_eq!(m.description, "desc");
        assert!(matches!(detail.integrity.status, IntegrityStatus::Ok));
    }

    #[test]
    fn valid_employee_ids() {
        assert!(is_valid_employee_id("ip-screening"));
        assert!(is_valid_employee_id("a"));
        assert!(is_valid_employee_id("abc-123"));
        assert!(is_valid_employee_id("罗伯特"));
        assert!(is_valid_employee_id("黄一鸣-笔记"));
        assert!(!is_valid_employee_id(""));
        assert!(!is_valid_employee_id("-bad"));
        assert!(!is_valid_employee_id("bad-"));
        assert!(!is_valid_employee_id("HAS_UPPER"));
        assert!(!is_valid_employee_id("has space"));
    }

    // ── Employee Skill Tests ──────────────────────────────────────────

    fn create_root_skill(root: &RootWorkspace, skill_id: &str, name: &str, desc: &str) {
        let dir = root.skills_dir().join(skill_id);
        fs::create_dir_all(&dir).expect("create root skill dir");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n"),
        )
        .expect("write SKILL.md");
        fs::write(dir.join("extra.txt"), "bonus content").expect("write extra");
    }

    fn setup_employee(root: &RootWorkspace, id: &str) {
        create(root, CreateEmployeeInput::basic(id, id)).expect("create employee");
    }

    #[test]
    fn install_prepared_employee_registers_manifest_prompt_skills_and_origin() {
        let (_tmp, root) = setup();
        create_root_skill(
            &root,
            "content-marketer",
            "Content Marketer",
            "content skill",
        );

        let detail = install_prepared_employee(
            &root,
            PreparedEmployeeInstall {
                id: "content-marketer-employee".into(),
                name: "内容营销师".into(),
                description: "内容营销策略专家".into(),
                prompt_md: "你是一名内容营销策略专家。".into(),
                root_skill_ids: vec!["content-marketer".into()],
                hub_origin: Some(crate::services::hub_state::HubOrigin {
                    kind: crate::services::hub_state::HubOriginKind::Employee,
                    hub_url: "http://hub/api/v1".into(),
                    hub_id: "content-marketer".into(),
                    local_id: "content-marketer-employee".into(),
                    content_hash: None,
                    hub_updated_at: "2026-06-05T09:53:10Z".into(),
                    installed_at: "2026-06-10T10:00:00Z".into(),
                    skill_hub_ids: vec!["repo--skills--content-marketer".into()],
                }),
            },
        )
        .expect("install prepared employee");

        assert_eq!(detail.registry_entry.id, "content-marketer-employee");
        assert_eq!(detail.registry_entry.name, "内容营销师");
        assert_eq!(
            read_employee_prompt(&root, "content-marketer-employee").expect("read prompt"),
            "你是一名内容营销策略专家。"
        );
        assert!(employee_skills_dir(&root, "content-marketer-employee")
            .join("content-marketer/SKILL.md")
            .is_file());
        assert!(employee_dir(&root, "content-marketer-employee")
            .join(".hub_origin.json")
            .is_file());

        let skills = read_skills_registry(&root, "content-marketer-employee").expect("read skills");
        assert_eq!(skills.skills.len(), 1);
        assert_eq!(skills.skills[0].id, "content-marketer");
        assert_eq!(
            skills.skills[0].copied_from.as_deref(),
            Some("content-marketer")
        );
    }

    #[test]
    fn install_prepared_employee_overwrites_existing_employee_without_touching_root_skill() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "writer", "Writer", "root skill");
        create(&root, CreateEmployeeInput::basic("writer", "旧员工")).expect("create employee");
        copy_root_skill_to_employee(&root, "writer", "writer").expect("copy skill");

        let detail = install_prepared_employee(
            &root,
            PreparedEmployeeInstall {
                id: "writer".into(),
                name: "远程写作员工".into(),
                description: "remote employee".into(),
                prompt_md: "new prompt".into(),
                root_skill_ids: vec!["writer".into()],
                hub_origin: Some(crate::services::hub_state::HubOrigin {
                    kind: crate::services::hub_state::HubOriginKind::Employee,
                    hub_url: "http://hub/api/v1".into(),
                    hub_id: "writer".into(),
                    local_id: "writer".into(),
                    content_hash: None,
                    hub_updated_at: "2026-06-10T00:00:00Z".into(),
                    installed_at: "2026-06-10T10:00:00Z".into(),
                    skill_hub_ids: vec!["repo--skills--writer".into()],
                }),
            },
        )
        .expect("overwrite employee");

        assert_eq!(detail.registry_entry.name, "远程写作员工");
        assert_eq!(read_employee_prompt(&root, "writer").unwrap(), "new prompt");
        assert!(root.skills_dir().join("writer/SKILL.md").is_file());
        assert!(employee_skills_dir(&root, "writer")
            .join("writer/SKILL.md")
            .is_file());
    }

    #[test]
    fn copy_root_skill_to_employee_works() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "alpha", "Alpha", "alpha skill");
        setup_employee(&root, "emp-a");

        let summary = copy_root_skill_to_employee(&root, "emp-a", "alpha").expect("copy");
        assert_eq!(summary.id, "alpha");
        assert_eq!(summary.source, "root");
        assert_eq!(summary.copied_from.as_deref(), Some("alpha"));
        assert!(summary.enabled);
        assert_eq!(summary.name, "Alpha");
        assert!(summary.has_skill_md);

        let skill_dir = employee_skills_dir(&root, "emp-a").join("alpha");
        assert!(skill_dir.join("SKILL.md").is_file());
        assert!(skill_dir.join("extra.txt").is_file());

        let sr = read_skills_registry(&root, "emp-a").expect("read sr");
        assert_eq!(sr.skills.len(), 1);
        assert_eq!(sr.skills[0].id, "alpha");
    }

    #[test]
    fn copy_duplicate_skill_rejected() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "beta", "Beta", "beta skill");
        setup_employee(&root, "emp-b");

        copy_root_skill_to_employee(&root, "emp-b", "beta").expect("first copy");
        let err = copy_root_skill_to_employee(&root, "emp-b", "beta").unwrap_err();
        assert!(err.contains("已拥有技能"));
    }

    #[test]
    fn copy_nonexistent_root_skill_rejected() {
        let (_tmp, root) = setup();
        setup_employee(&root, "emp-c");

        let err = copy_root_skill_to_employee(&root, "emp-c", "nonexistent").unwrap_err();
        assert!(err.contains("不存在"));
    }

    #[test]
    fn list_employee_skills_returns_summaries() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "s1", "Skill 1", "first");
        create_root_skill(&root, "s2", "Skill 2", "second");
        setup_employee(&root, "emp-d");

        let empty = list_employee_skills(&root, "emp-d").expect("list empty");
        assert!(empty.is_empty());

        copy_root_skill_to_employee(&root, "emp-d", "s1").expect("copy s1");
        copy_root_skill_to_employee(&root, "emp-d", "s2").expect("copy s2");

        let skills = list_employee_skills(&root, "emp-d").expect("list");
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].id, "s1");
        assert_eq!(skills[1].id, "s2");
    }

    #[test]
    fn toggle_employee_skill_works() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "tog", "Toggle", "toggleable");
        setup_employee(&root, "emp-e");
        copy_root_skill_to_employee(&root, "emp-e", "tog").expect("copy");

        let sr = toggle_employee_skill(&root, "emp-e", "tog", false).expect("disable");
        assert!(!sr.skills[0].enabled);

        let sr = toggle_employee_skill(&root, "emp-e", "tog", true).expect("enable");
        assert!(sr.skills[0].enabled);
    }

    #[test]
    fn toggle_nonexistent_skill_rejected() {
        let (_tmp, root) = setup();
        setup_employee(&root, "emp-f");

        let err = toggle_employee_skill(&root, "emp-f", "nope", true).unwrap_err();
        assert!(err.contains("不存在"));
    }

    #[test]
    fn delete_employee_skill_works() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "del", "Deletable", "to be deleted");
        setup_employee(&root, "emp-g");
        copy_root_skill_to_employee(&root, "emp-g", "del").expect("copy");

        let skill_dir = employee_skills_dir(&root, "emp-g").join("del");
        assert!(skill_dir.is_dir());

        let sr = delete_employee_skill(&root, "emp-g", "del").expect("delete");
        assert!(sr.skills.is_empty());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn delete_nonexistent_skill_rejected() {
        let (_tmp, root) = setup();
        setup_employee(&root, "emp-h");

        let err = delete_employee_skill(&root, "emp-h", "ghost").unwrap_err();
        assert!(err.contains("不存在"));
    }

    #[test]
    fn delete_employee_removes_registry_and_directory() {
        let (_tmp, root) = setup();
        setup_employee(&root, "emp-del");
        assert!(employee_dir(&root, "emp-del").is_dir());

        delete_employee(&root, "emp-del").expect("delete employee");

        assert!(list(&root).expect("list").iter().all(|entry| entry.id != "emp-del"));
        assert!(!employee_dir(&root, "emp-del").exists());
    }

    #[test]
    fn delete_employee_rejects_system_employee() {
        let (_tmp, root) = setup();
        ensure_employee_infrastructure(&root).expect("ensure");

        let err = delete_employee(&root, GENERAL_EMPLOYEE_ID).unwrap_err();
        assert!(err.contains("不能删除系统员工"));
    }

    #[test]
    fn copied_skill_decoupled_from_root() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "decoupled", "Original", "original desc");
        setup_employee(&root, "emp-i");
        copy_root_skill_to_employee(&root, "emp-i", "decoupled").expect("copy");

        let root_skill_md = root.skills_dir().join("decoupled/SKILL.md");
        fs::write(
            &root_skill_md,
            "---\nname: Changed\ndescription: changed desc\n---\n\n# Changed\n",
        )
        .expect("update root");

        let skills = list_employee_skills(&root, "emp-i").expect("list");
        assert_eq!(skills[0].name, "Original");
        assert_eq!(skills[0].description, "original desc");
    }

    #[test]
    fn read_skills_registry_returns_empty_when_missing() {
        let (_tmp, root) = setup();
        setup_employee(&root, "emp-j");
        fs::remove_file(skills_path(&root, "emp-j")).ok();

        let sr = read_skills_registry(&root, "emp-j").expect("read missing");
        assert!(sr.skills.is_empty());
        assert_eq!(sr.version, 1);
    }

    // ── Workspace Binding Tests ────────────────────────────────────────

    fn create_fake_workspace(tmp: &tempfile::TempDir, name: &str) -> PathBuf {
        let ws_path = tmp.path().join(name);
        fs::create_dir_all(ws_path.join(".chawork/state")).expect("create ws dirs");
        let ws = crate::services::workspace::WorkspaceState {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
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
        ws_path
    }

    #[test]
    fn validate_unbound_workspace() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-unbound");

        let v = validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(v.status, BindingStatus::Unbound);
        assert!(v.employee_id.is_none());
        assert!(v.employee_name.is_none());
    }

    #[test]
    fn bind_and_validate_workspace() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-bind");
        setup_employee(&root, "emp-bind");

        let v = bind_workspace(&root, "emp-bind", &ws_path, "ws-id-1", "WS Bind").expect("bind");
        assert_eq!(v.status, BindingStatus::Bound);
        assert_eq!(v.employee_id.as_deref(), Some("emp-bind"));

        // Validate from scratch
        let v2 = validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(v2.status, BindingStatus::Bound);
        assert_eq!(v2.employee_name.as_deref(), Some("emp-bind"));

        // Check employee workspaces.json
        let wml = read_workspace_memberships(&root, "emp-bind").expect("read wml");
        assert_eq!(wml.workspaces.len(), 1);
        assert_eq!(wml.workspaces[0].name, "WS Bind");
    }

    #[test]
    fn bind_to_nonexistent_employee_fails() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-noemp");

        let err = bind_workspace(&root, "ghost", &ws_path, "id", "name").unwrap_err();
        assert!(err.contains("不存在"));
    }

    #[test]
    fn bind_to_dream_employee_fails() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-dream");

        let err = bind_workspace(&root, DREAM_EMPLOYEE_ID, &ws_path, "id", "name").unwrap_err();
        assert!(err.contains("Dream"));
    }

    #[test]
    fn bind_already_bound_to_different_employee_fails() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-conflict");
        setup_employee(&root, "emp-first");
        setup_employee(&root, "emp-second");

        bind_workspace(&root, "emp-first", &ws_path, "id", "name").expect("bind first");

        let err = bind_workspace(&root, "emp-second", &ws_path, "id", "name").unwrap_err();
        assert!(err.contains("已绑定"));
    }

    #[test]
    fn rebind_same_employee_is_idempotent() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-rebind");
        setup_employee(&root, "emp-rebind");

        bind_workspace(&root, "emp-rebind", &ws_path, "id", "Name 1").expect("bind 1");
        bind_workspace(&root, "emp-rebind", &ws_path, "id", "Name 2").expect("bind 2");

        let wml = read_workspace_memberships(&root, "emp-rebind").expect("read wml");
        assert_eq!(wml.workspaces.len(), 1);
        assert_eq!(wml.workspaces[0].name, "Name 2");
    }

    #[test]
    fn unbind_workspace_clears_both_sides() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-unbind");
        setup_employee(&root, "emp-unbind");

        bind_workspace(&root, "emp-unbind", &ws_path, "id", "name").expect("bind");
        unbind_workspace(&root, &ws_path).expect("unbind");

        let v = validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(v.status, BindingStatus::Unbound);

        let wml = read_workspace_memberships(&root, "emp-unbind").expect("read wml");
        assert!(wml.workspaces.is_empty());
    }

    #[test]
    fn unbind_unbound_workspace_is_noop() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-noop");

        unbind_workspace(&root, &ws_path).expect("unbind noop");
    }

    #[test]
    fn validate_detects_employee_missing() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-emp-missing");

        write_workspace_binding(
            &ws_path,
            &WorkspaceBinding {
                employee_id: "ghost-emp".to_string(),
            },
        )
        .expect("write binding");

        let v = validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(v.status, BindingStatus::EmployeeMissing);
        assert_eq!(v.employee_id.as_deref(), Some("ghost-emp"));
    }

    #[test]
    fn validate_detects_membership_missing() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-mem-missing");
        setup_employee(&root, "emp-mem");

        write_workspace_binding(
            &ws_path,
            &WorkspaceBinding {
                employee_id: "emp-mem".to_string(),
            },
        )
        .expect("write binding");

        let v = validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(v.status, BindingStatus::MembershipMissing);
    }

    #[test]
    fn validate_detects_path_mismatch() {
        let (tmp, root) = setup();
        let ws_path = create_fake_workspace(&tmp, "ws-path-mismatch");
        setup_employee(&root, "emp-path");

        let ws_id = read_workspace_id(&ws_path).expect("workspace id");
        bind_workspace(&root, "emp-path", &ws_path, &ws_id, "ws-path-mismatch").expect("bind");

        let mut wml = read_workspace_memberships(&root, "emp-path").expect("read wml");
        wml.workspaces[0].path = tmp.path().join("stale-path").to_string_lossy().into_owned();
        write_workspace_memberships(&root, "emp-path", &wml).expect("write wml");

        let v = validate_binding(&root, &ws_path).expect("validate");
        assert_eq!(v.status, BindingStatus::PathMismatch);
    }

    #[test]
    fn create_employee_assigns_uuid_when_id_empty() {
        let (_tmp, root) = setup();

        let detail = create(
            &root,
            CreateEmployeeInput {
                id: String::new(),
                name: "Auto ID Emp".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create");

        let id = &detail.registry_entry.id;
        assert!(
            uuid::Uuid::parse_str(id).is_ok(),
            "expected UUID id, got {id}"
        );
        assert!(employee_dir(&root, id).is_dir());
    }

    #[test]
    fn create_employee_with_initial_prompt_and_skills() {
        let (_tmp, root) = setup();
        create_root_skill(&root, "boot-skill", "Boot", "bootstrap skill");

        let detail = create(
            &root,
            CreateEmployeeInput {
                id: "boot-emp".to_string(),
                name: "Boot Emp".to_string(),
                description: String::new(),
                initial_prompt: "Initial prompt body".to_string(),
                root_skill_ids: vec!["boot-skill".to_string()],
            },
        )
        .expect("create");

        assert_eq!(detail.registry_entry.id, "boot-emp");
        let prompt = fs::read_to_string(employee_dir(&root, "boot-emp").join(PROMPT_FILE))
            .expect("read prompt");
        assert_eq!(prompt, "Initial prompt body");

        let skills = list_employee_skills(&root, "boot-emp").expect("list skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "boot-skill");
    }

    #[test]
    fn list_workspaces_for_employee_returns_memberships() {
        let (tmp, root) = setup();
        let ws1 = create_fake_workspace(&tmp, "ws-list-1");
        let ws2 = create_fake_workspace(&tmp, "ws-list-2");
        setup_employee(&root, "emp-list");

        bind_workspace(&root, "emp-list", &ws1, "id-1", "WS 1").expect("bind 1");
        bind_workspace(&root, "emp-list", &ws2, "id-2", "WS 2").expect("bind 2");

        let workspaces = list_workspaces_for_employee(&root, "emp-list").expect("list");
        assert_eq!(workspaces.len(), 2);
    }

    #[test]
    fn list_workspaces_for_nonexistent_employee_fails() {
        let (_tmp, root) = setup();

        let err = list_workspaces_for_employee(&root, "nope").unwrap_err();
        assert!(err.contains("不存在"));
    }

    #[test]
    fn bind_multiple_workspaces_to_one_employee() {
        let (tmp, root) = setup();
        let ws1 = create_fake_workspace(&tmp, "ws-multi-1");
        let ws2 = create_fake_workspace(&tmp, "ws-multi-2");
        let ws3 = create_fake_workspace(&tmp, "ws-multi-3");
        setup_employee(&root, "emp-multi");

        for (ws, id, name) in [
            (&ws1, "id-1", "WS 1"),
            (&ws2, "id-2", "WS 2"),
            (&ws3, "id-3", "WS 3"),
        ] {
            bind_workspace(&root, "emp-multi", ws, id, name).expect("bind");
        }

        let wml = read_workspace_memberships(&root, "emp-multi").expect("read wml");
        assert_eq!(wml.workspaces.len(), 3);

        // Unbind one
        unbind_workspace(&root, &ws2).expect("unbind ws2");
        let wml = read_workspace_memberships(&root, "emp-multi").expect("read wml");
        assert_eq!(wml.workspaces.len(), 2);
        assert!(wml.workspaces.iter().all(|m| m.name != "WS 2"));
    }
}
