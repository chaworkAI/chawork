use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub effective_mode: String,
    pub path: String,
    pub description: String,
    pub version: Option<String>,
    pub checksum: String,
    pub root_checksum: Option<String>,
    pub source: Option<String>,
    pub updated_at: String,
    pub enabled: bool,
    pub executor_for_write: String,
    pub runtime_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillListView {
    pub root_catalog: Vec<SkillSummary>,
    pub workspace_selection: Vec<SkillSummary>,
    pub workspace_local: Vec<SkillSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSelectionFile {
    pub version: u32,
    pub root_skills: HashMap<String, SkillSelectionEntry>,
    pub workspace_skills: HashMap<String, WorkspaceSkillEntry>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSelectionEntry {
    pub enabled: bool,
    pub mode: String,
    pub root_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSkillEntry {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillSelectionView {
    pub root_skills: HashMap<String, SkillSelectionEntry>,
    pub workspace_skills: HashMap<String, WorkspaceSkillEntry>,
    pub dirty: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillPromotionResult {
    pub ok: bool,
    pub root_skill: SkillSummary,
    pub affected_workspaces: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn selection_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(".chawork").join("skills.json")
}

pub fn compute_checksum(path: &Path) -> String {
    let Ok(raw) = fs::read(path) else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    hasher.update(&raw);
    format!("{:x}", hasher.finalize())
}

fn parse_skill_frontmatter(content: &str) -> Option<SkillFrontmatter> {
    let t = content.trim_start();
    if !t.starts_with("---") {
        return None;
    }
    let mut lines = t.lines();
    lines.next()?;
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
        return None;
    }
    serde_yaml::from_str(&yaml_buf).ok()
}

/// Read the `description` field from SKILL.md frontmatter.
pub fn read_skill_description(skill_dir: &Path) -> String {
    let skill_md = skill_dir.join("SKILL.md");
    let content = match fs::read_to_string(&skill_md) {
        Ok(content) => content,
        Err(_) => return String::new(),
    };
    parse_skill_frontmatter(&content)
        .and_then(|meta| meta.description)
        .unwrap_or_default()
}

fn skill_mtime(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| {
            let dur = t.duration_since(std::time::UNIX_EPOCH).ok()?;
            chrono::DateTime::<Utc>::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
        })
        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, true))
        .unwrap_or_else(iso_now)
}

fn read_skill_summary(
    skill_dir: &Path,
    scope: &str,
    effective_mode: &str,
    enabled: bool,
    root_checksum: Option<String>,
    source: Option<String>,
) -> Option<SkillSummary> {
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.is_file() {
        return None;
    }
    let id = skill_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return None;
    }
    let content = fs::read_to_string(&skill_md).ok()?;
    let meta = parse_skill_frontmatter(&content);
    let name = meta
        .as_ref()
        .and_then(|m| m.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.clone());
    let description = meta
        .as_ref()
        .and_then(|m| m.description.clone())
        .unwrap_or_default();
    let version = meta.and_then(|m| m.version);
    let checksum = compute_checksum(&skill_md);
    let executor = if scope == "root" {
        "chawork-app"
    } else {
        "workspace-tools"
    };
    Some(SkillSummary {
        id: id.clone(),
        name,
        scope: scope.to_string(),
        effective_mode: effective_mode.to_string(),
        path: skill_dir.to_string_lossy().into_owned(),
        description,
        version,
        checksum,
        root_checksum,
        source,
        updated_at: skill_mtime(&skill_md),
        enabled,
        executor_for_write: executor.to_string(),
        runtime_status: "synced".to_string(),
    })
}

fn scan_skill_dirs(skills_dir: &Path) -> Vec<PathBuf> {
    if !skills_dir.is_dir() {
        return Vec::new();
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(skills_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("SKILL.md").is_file())
        .collect();
    dirs.sort();
    dirs
}

pub fn list_root_skills(root_skills_dir: &Path) -> Vec<SkillSummary> {
    scan_skill_dirs(root_skills_dir)
        .into_iter()
        .filter_map(|dir| {
            read_skill_summary(
                &dir,
                "root",
                "unselected_root",
                false,
                None,
                Some("root_catalog".to_string()),
            )
        })
        .collect()
}

pub fn list_workspace_skills(workspace_skills_dir: &Path) -> Vec<SkillSummary> {
    scan_skill_dirs(workspace_skills_dir)
        .into_iter()
        .filter_map(|dir| {
            read_skill_summary(
                &dir,
                "workspace",
                "workspace_local",
                true,
                None,
                Some("workspace_local".to_string()),
            )
        })
        .collect()
}

pub fn default_skill_selection() -> SkillSelectionFile {
    SkillSelectionFile {
        version: 1,
        root_skills: HashMap::new(),
        workspace_skills: HashMap::new(),
        updated_at: iso_now(),
    }
}

/// True when workspace has no skill selection file or no enabled root skills.
/// Bound workspaces use employee skills instead — never prompt legacy skill setup.
pub fn workspace_needs_skill_setup(
    workspace_path: &Path,
    root: &crate::services::root_workspace::RootWorkspace,
) -> bool {
    if let Ok(binding) = crate::services::employee::validate_binding(root, workspace_path) {
        if binding.status == crate::services::employee::BindingStatus::Bound {
            return false;
        }
    }
    match read_skill_selection(workspace_path) {
        None => true,
        Some(sel) => !sel.root_skills.values().any(|e| e.enabled),
    }
}

pub fn read_skill_selection(workspace_path: &Path) -> Option<SkillSelectionFile> {
    let p = selection_path(workspace_path);
    if !p.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&p).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn write_skill_selection(
    workspace_path: &Path,
    selection: &SkillSelectionFile,
) -> Result<(), String> {
    let p = selection_path(workspace_path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(selection).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| e.to_string())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("源目录不存在: {}", src.display()));
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn create_override(
    root_skills_dir: &Path,
    workspace_skills_dir: &Path,
    skill_name: &str,
) -> Result<SkillSummary, String> {
    let src = root_skills_dir.join(skill_name);
    if !src.join("SKILL.md").is_file() {
        return Err(format!("根目录技能不存在: {skill_name}"));
    }
    fs::create_dir_all(workspace_skills_dir).map_err(|e| e.to_string())?;
    let dest = workspace_skills_dir.join(skill_name);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    copy_dir_all(&src, &dest)?;
    let root_checksum = compute_checksum(&src.join("SKILL.md"));
    read_skill_summary(
        &dest,
        "workspace",
        "workspace_override",
        true,
        Some(root_checksum),
        Some("workspace_override".to_string()),
    )
    .ok_or_else(|| "创建 override 后无法读取技能".to_string())
}

pub fn delete_workspace_skill(workspace_skills_dir: &Path, skill_name: &str) -> Result<(), String> {
    let dest = workspace_skills_dir.join(skill_name);
    if !dest.is_dir() {
        return Err(format!("工作区技能不存在: {skill_name}"));
    }
    fs::remove_dir_all(&dest).map_err(|e| e.to_string())
}

pub fn promote_to_root(
    workspace_skills_dir: &Path,
    root_skills_dir: &Path,
    skill_name: &str,
) -> Result<SkillSummary, String> {
    let src = workspace_skills_dir.join(skill_name);
    if !src.join("SKILL.md").is_file() {
        return Err(format!("工作区技能不存在: {skill_name}"));
    }
    fs::create_dir_all(root_skills_dir).map_err(|e| e.to_string())?;
    let dest = root_skills_dir.join(skill_name);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    copy_dir_all(&src, &dest)?;
    read_skill_summary(
        &dest,
        "root",
        "selected_root",
        true,
        None,
        Some("manual".to_string()),
    )
    .ok_or_else(|| "推广后无法读取根技能".to_string())
}

/// Merge root catalog, workspace skills, and selection into a list view.
pub fn build_skill_list_view(
    root_skills_dir: &Path,
    workspace_path: Option<&Path>,
) -> SkillListView {
    let root_catalog_raw = list_root_skills(root_skills_dir);
    let Some(ws_path) = workspace_path else {
        return SkillListView {
            root_catalog: root_catalog_raw,
            workspace_selection: vec![],
            workspace_local: vec![],
        };
    };

    let ws_skills_dir = ws_path.join("skills");
    let ws_skills = list_workspace_skills(&ws_skills_dir);
    let selection = read_skill_selection(ws_path).unwrap_or_else(default_skill_selection);

    let root_ids: HashMap<String, String> = root_catalog_raw
        .iter()
        .map(|s| (s.id.clone(), s.checksum.clone()))
        .collect();
    let ws_by_id: HashMap<String, SkillSummary> = ws_skills
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    let mut root_catalog = Vec::new();
    let mut workspace_selection = Vec::new();

    for mut skill in root_catalog_raw {
        let root_cs = skill.checksum.clone();
        if let Some(ws) = ws_by_id.get(&skill.id) {
            skill.effective_mode = "workspace_override".to_string();
            skill.scope = "workspace".to_string();
            skill.path = ws.path.clone();
            skill.checksum = ws.checksum.clone();
            skill.root_checksum = Some(root_cs);
            skill.source = Some("workspace_override".to_string());
            skill.enabled = selection
                .workspace_skills
                .get(&skill.id)
                .map(|e| e.enabled)
                .unwrap_or(true);
        } else if let Some(entry) = selection.root_skills.get(&skill.id) {
            if entry.enabled {
                skill.effective_mode = "selected_root".to_string();
                skill.enabled = true;
                workspace_selection.push(skill.clone());
                if entry.root_checksum != skill.checksum {
                    skill.runtime_status = "dirty".to_string();
                }
            } else {
                skill.effective_mode = "unselected_root".to_string();
                skill.enabled = false;
            }
        } else {
            skill.effective_mode = "unselected_root".to_string();
            skill.enabled = false;
        }
        root_catalog.push(skill);
    }

    let mut workspace_local = Vec::new();
    for ws in ws_skills {
        if root_ids.contains_key(&ws.id) {
            continue;
        }
        let enabled = selection
            .workspace_skills
            .get(&ws.id)
            .map(|e| e.enabled)
            .unwrap_or(true);
        let mut local = ws;
        local.enabled = enabled;
        local.effective_mode = "workspace_local".to_string();
        workspace_local.push(local);
    }

    SkillListView {
        root_catalog,
        workspace_selection,
        workspace_local,
    }
}

pub fn selection_view_from_file(workspace_path: &Path, dirty: bool) -> SkillSelectionView {
    let selection = read_skill_selection(workspace_path).unwrap_or_else(default_skill_selection);
    SkillSelectionView {
        root_skills: selection.root_skills,
        workspace_skills: selection.workspace_skills,
        dirty,
        updated_at: Some(selection.updated_at),
    }
}

pub fn set_root_skill_enabled(
    workspace_path: &Path,
    root_skill_id: &str,
    enabled: bool,
    root_skills_dir: &Path,
) -> Result<SkillSelectionView, String> {
    let skill_md = root_skills_dir.join(root_skill_id).join("SKILL.md");
    if !skill_md.is_file() {
        return Err(format!("根技能不存在: {root_skill_id}"));
    }
    let checksum = compute_checksum(&skill_md);
    let mut selection =
        read_skill_selection(workspace_path).unwrap_or_else(default_skill_selection);
    selection.root_skills.insert(
        root_skill_id.to_string(),
        SkillSelectionEntry {
            enabled,
            mode: "follow_root".to_string(),
            root_checksum: checksum,
        },
    );
    selection.updated_at = iso_now();
    write_skill_selection(workspace_path, &selection)?;
    Ok(selection_view_from_file(workspace_path, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_skill(dir: &Path, name: &str, desc: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n"),
        )
        .unwrap();
    }

    #[test]
    fn compute_checksum_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("SKILL.md");
        fs::write(&p, "hello").unwrap();
        let a = compute_checksum(&p);
        let b = compute_checksum(&p);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn list_and_select_root_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root/skills");
        write_skill(&root.join("alpha"), "Alpha", "test skill");

        let ws = tmp.path().join("ws");
        fs::create_dir_all(ws.join(".chawork")).unwrap();

        let listed = list_root_skills(&root);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "alpha");

        set_root_skill_enabled(&ws, "alpha", true, &root).unwrap();
        let view = build_skill_list_view(&root, Some(&ws));
        assert_eq!(view.workspace_selection.len(), 1);
        assert_eq!(view.root_catalog[0].effective_mode, "selected_root");
    }

    #[test]
    fn create_override_copies_root_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root/skills");
        let ws = tmp.path().join("ws/skills");
        write_skill(&root.join("beta"), "Beta", "root");

        let summary = create_override(&root, &ws, "beta").unwrap();
        assert_eq!(summary.effective_mode, "workspace_override");
        assert!(ws.join("beta/SKILL.md").is_file());
    }
}
