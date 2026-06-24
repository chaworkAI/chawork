use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use chrono::{SecondsFormat, Utc};
use serde::Deserialize;

use crate::services::employee::{self, PreparedEmployeeInstall};
use crate::services::hub_state::{self, HubOrigin, HubOriginKind};
use crate::services::root_workspace::RootWorkspace;

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn local_skill_id_from_hub_id(hub_id: &str) -> String {
    let candidate = hub_id
        .rsplit("--")
        .next()
        .filter(|s| is_safe_local_id_segment(s))
        .unwrap_or(hub_id);
    sanitize_local_id(candidate)
}

fn is_safe_local_id_segment(segment: &str) -> bool {
    let segment = segment.trim();
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.contains('/')
        && !segment.contains('\\')
}

fn sanitize_local_id(raw: &str) -> String {
    let mut out = raw
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            ch if ch.is_control() => '-',
            ch => ch,
        })
        .collect::<String>();
    if !is_safe_local_id_segment(&out) {
        out = "hub-skill".to_string();
    }
    out
}

pub fn ensure_safe_archive_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() {
        return Err(format!(
            "path traversal in bundle entry: {}",
            path.display()
        ));
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "path traversal in bundle entry: {}",
                    path.display()
                ));
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }
    Ok(())
}

fn list_tar_entries(bundle_path: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("tar")
        .arg("-tzf")
        .arg(bundle_path)
        .output()
        .map_err(|e| format!("读取 Hub bundle 目录失败: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "读取 Hub bundle 目录失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("解析 Hub bundle 目录输出失败: {e}"))?;
    Ok(stdout
        .lines()
        .map(|line| PathBuf::from(line.trim_start_matches("./")))
        .collect())
}

pub fn extract_bundle_safely(bundle_path: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| format!("清理 bundle 临时目录失败: {e}"))?;
    }
    fs::create_dir_all(dest).map_err(|e| format!("创建 bundle 临时目录失败: {e}"))?;

    for entry in list_tar_entries(bundle_path)? {
        ensure_safe_archive_path(&entry)?;
    }

    let output = Command::new("tar")
        .arg("-xzf")
        .arg(bundle_path)
        .arg("-C")
        .arg(dest)
        .output()
        .map_err(|e| format!("解压 Hub bundle 失败: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "解压 Hub bundle 失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

pub fn ensure_skill_install_allowed(
    root_skills_dir: &Path,
    hub_id: &str,
    local_id: &str,
) -> Result<(), String> {
    let _ = (root_skills_dir, hub_id, local_id);
    Ok(())
}

fn remove_existing_path(path: &Path, label: &str) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("{label}: {e}"))?;
    } else if path.exists() {
        fs::remove_file(path).map_err(|e| format!("{label}: {e}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct HubSkillInstallInput {
    pub hub_url: String,
    pub hub_id: String,
    pub content_hash: String,
    pub hub_updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HubSkillInstallResult {
    pub hub_id: String,
    pub local_id: String,
    pub path: String,
    pub installed_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HubSkillUninstallResult {
    pub hub_id: String,
    pub local_id: String,
}

#[derive(Debug, Clone)]
pub struct HubEmployeeInstallInput {
    pub hub_url: String,
    pub hub_id: String,
    pub hub_updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HubEmployeeInstallResult {
    pub hub_id: String,
    pub local_id: String,
    pub path: String,
    pub installed_at: String,
    pub root_skill_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SkillMeta {
    #[serde(default)]
    content_hash: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EmployeeBundleJson {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    skill_ids: Vec<String>,
    #[serde(default)]
    updated_at: String,
}

fn read_employee_skill_ids(
    bundle_root: &Path,
    employee: &EmployeeBundleJson,
) -> Result<Vec<String>, String> {
    let skills_path = bundle_root.join("skills.json");
    if !skills_path.is_file() {
        return Ok(employee.skill_ids.clone());
    }
    let raw = fs::read_to_string(&skills_path)
        .map_err(|e| format!("读取 Hub employee skills.json 失败: {e}"))?;
    serde_json::from_str::<Vec<String>>(&raw)
        .map_err(|e| format!("解析 Hub employee skills.json 失败: {e}"))
}

fn find_single_bundle_root(extracted: &Path) -> Result<PathBuf, String> {
    let mut dirs = fs::read_dir(extracted)
        .map_err(|e| format!("读取解压目录失败: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    if dirs.len() == 1 {
        return Ok(dirs.remove(0));
    }
    if extracted.join("SKILL.md").is_file() {
        return Ok(extracted.to_path_buf());
    }
    Err("Hub skill bundle 缺少唯一技能目录".to_string())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {e}"))?;
    for entry in fs::read_dir(src).map_err(|e| format!("读取源目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("复制文件失败: {e}"))?;
        }
    }
    Ok(())
}

pub fn install_skill_bundle_from_path(
    bundle_path: &Path,
    root_skills_dir: &Path,
    input: HubSkillInstallInput,
) -> Result<HubSkillInstallResult, String> {
    let local_id = local_skill_id_from_hub_id(&input.hub_id);
    ensure_skill_install_allowed(root_skills_dir, &input.hub_id, &local_id)?;

    let temp_root = root_skills_dir.join(format!(".{}.hub-install", local_id));
    let extract_dir = temp_root.join("extract");
    let stage_dir = temp_root.join("stage").join(&local_id);
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root).map_err(|e| format!("清理安装临时目录失败: {e}"))?;
    }
    fs::create_dir_all(&temp_root).map_err(|e| format!("创建安装临时目录失败: {e}"))?;

    let result = (|| {
        extract_bundle_safely(bundle_path, &extract_dir)?;
        let bundle_root = find_single_bundle_root(&extract_dir)?;
        if !bundle_root.join("SKILL.md").is_file() {
            return Err("Hub skill bundle 缺少 SKILL.md".to_string());
        }

        copy_dir_all(&bundle_root, &stage_dir)?;

        if let Ok(raw) = fs::read_to_string(stage_dir.join("skill.meta.json")) {
            let meta: SkillMeta = serde_json::from_str(&raw)
                .map_err(|e| format!("解析 skill.meta.json 失败: {e}"))?;
            if let Some(hash) = meta.content_hash {
                if !hash.is_empty() && hash != input.content_hash {
                    return Err("Hub skill content_hash 与 metadata 不一致".to_string());
                }
            }
            if let Some(updated_at) = meta.updated_at {
                if !updated_at.is_empty() && updated_at != input.hub_updated_at {
                    return Err("Hub skill updated_at 与 metadata 不一致".to_string());
                }
            }
        }

        let installed_at = iso_now();
        hub_state::write_origin(
            &stage_dir,
            &HubOrigin {
                kind: HubOriginKind::Skill,
                hub_url: input.hub_url,
                hub_id: input.hub_id.clone(),
                local_id: local_id.clone(),
                content_hash: Some(input.content_hash),
                hub_updated_at: input.hub_updated_at,
                installed_at: installed_at.clone(),
                skill_hub_ids: Vec::new(),
            },
        )?;

        fs::create_dir_all(root_skills_dir)
            .map_err(|e| format!("创建 Root skills 目录失败: {e}"))?;
        let dest = root_skills_dir.join(&local_id);
        remove_existing_path(&dest, "替换 Root skill 失败")?;
        fs::rename(&stage_dir, &dest).map_err(|e| format!("写入 Root skill 失败: {e}"))?;

        Ok(HubSkillInstallResult {
            hub_id: input.hub_id,
            local_id,
            path: dest.to_string_lossy().into_owned(),
            installed_at,
        })
    })();

    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    result
}

pub fn uninstall_skill_from_root(
    root_skills_dir: &Path,
    hub_id: &str,
) -> Result<HubSkillUninstallResult, String> {
    let local_id = local_skill_id_from_hub_id(hub_id);
    let skill_dir = root_skills_dir.join(&local_id);
    if skill_dir.is_dir() {
        fs::remove_dir_all(&skill_dir).map_err(|e| format!("删除 Root skill 失败: {e}"))?;
    }
    Ok(HubSkillUninstallResult {
        hub_id: hub_id.to_string(),
        local_id,
    })
}

pub fn install_employee_bundle_from_path(
    bundle_path: &Path,
    root: &RootWorkspace,
    input: HubEmployeeInstallInput,
) -> Result<HubEmployeeInstallResult, String> {
    let local_id = input.hub_id.clone();
    let temp_root = root
        .employees_dir()
        .join(format!(".{}.hub-employee-install", local_id));
    let extract_dir = temp_root.join("extract");
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root).map_err(|e| format!("清理员工安装临时目录失败: {e}"))?;
    }
    fs::create_dir_all(&temp_root).map_err(|e| format!("创建员工安装临时目录失败: {e}"))?;

    let result = (|| {
        extract_bundle_safely(bundle_path, &extract_dir)?;
        let bundle_root = find_single_bundle_root(&extract_dir)?;

        let employee_json_path = bundle_root.join("employee.json");
        let prompt_path = bundle_root.join("prompt.md");
        if !employee_json_path.is_file() {
            return Err("Hub employee bundle 缺少 employee.json".to_string());
        }
        if !prompt_path.is_file() {
            return Err("Hub employee bundle 缺少 prompt.md".to_string());
        }

        let employee_raw = fs::read_to_string(&employee_json_path)
            .map_err(|e| format!("读取 employee.json 失败: {e}"))?;
        let employee: EmployeeBundleJson = serde_json::from_str(&employee_raw)
            .map_err(|e| format!("解析 employee.json 失败: {e}"))?;
        let skill_hub_ids = read_employee_skill_ids(&bundle_root, &employee)?;

        let mut root_skill_ids = Vec::new();
        for skill_hub_id in &skill_hub_ids {
            let root_skill_id = local_skill_id_from_hub_id(skill_hub_id);
            ensure_skill_install_allowed(&root.skills_dir(), skill_hub_id, &root_skill_id)?;
            let root_skill_dir = root.skills_dir().join(&root_skill_id);
            if !root_skill_dir.join("SKILL.md").is_file() {
                return Err(format!(
                    "Hub employee 依赖技能未下载到 Root: {skill_hub_id}"
                ));
            }
            let Some(origin) = hub_state::read_origin(&root_skill_dir)? else {
                return Err(format!(
                    "local_custom_source: Root skill {root_skill_id} already exists"
                ));
            };
            if origin.kind != HubOriginKind::Skill || origin.hub_id != *skill_hub_id {
                return Err(format!(
                    "hub_source_mismatch: Root skill {root_skill_id} belongs to {}",
                    origin.hub_id
                ));
            }
            root_skill_ids.push(root_skill_id);
        }

        let prompt_md =
            fs::read_to_string(&prompt_path).map_err(|e| format!("读取 prompt.md 失败: {e}"))?;
        let installed_at = iso_now();
        let detail = employee::install_prepared_employee(
            root,
            PreparedEmployeeInstall {
                id: employee.id.clone(),
                name: employee.name.clone(),
                description: employee.description.clone(),
                prompt_md,
                root_skill_ids: root_skill_ids.clone(),
                hub_origin: Some(HubOrigin {
                    kind: HubOriginKind::Employee,
                    hub_url: input.hub_url,
                    hub_id: input.hub_id.clone(),
                    local_id: employee.id,
                    content_hash: None,
                    hub_updated_at: if input.hub_updated_at.is_empty() {
                        employee.updated_at
                    } else {
                        input.hub_updated_at
                    },
                    installed_at: installed_at.clone(),
                    skill_hub_ids,
                }),
            },
        )?;

        Ok(HubEmployeeInstallResult {
            hub_id: input.hub_id,
            local_id: detail.registry_entry.id.clone(),
            path: detail.registry_entry.path,
            installed_at,
            root_skill_ids,
        })
    })();

    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    fn write_tar_bundle_with_entry(bundle_path: &Path, entry_path: &str, bytes: &[u8]) {
        let src = bundle_path.parent().unwrap().join("bundle-src");
        fs::create_dir_all(src.join(Path::new(entry_path).parent().unwrap_or(Path::new(""))))
            .unwrap();
        fs::write(src.join(entry_path), bytes).unwrap();
        let status = std::process::Command::new("tar")
            .arg("-czf")
            .arg(bundle_path)
            .arg("-C")
            .arg(&src)
            .arg(".")
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn write_tar_bundle_from_dir(bundle_path: &Path, src: &Path) {
        let status = std::process::Command::new("tar")
            .arg("-czf")
            .arg(bundle_path)
            .arg("-C")
            .arg(src)
            .arg(".")
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn resolve_local_skill_id_uses_last_segment() {
        assert_eq!(
            local_skill_id_from_hub_id("anthropics-skills--skills--pdf"),
            "pdf"
        );
        assert_eq!(local_skill_id_from_hub_id("pdf"), "pdf");
    }

    #[test]
    fn resolve_local_skill_id_ignores_dot_last_segment() {
        assert_eq!(
            local_skill_id_from_hub_id("tanshilong-article-skill--."),
            "tanshilong-article-skill--."
        );
    }

    #[test]
    fn rejects_path_traversal_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = tmp.path().join("evil.tar.gz");
        fs::write(&bundle, b"not actually used by traversal validation").unwrap();

        let err = ensure_safe_archive_path(Path::new("../evil/SKILL.md")).unwrap_err();

        assert!(err.contains("path traversal"));
    }

    #[test]
    fn extracts_valid_bundle_into_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = tmp.path().join("skill.tar.gz");
        write_tar_bundle_with_entry(
            &bundle,
            "repo--skills--pdf/SKILL.md",
            b"---\nname: pdf\n---",
        );

        let out = tmp.path().join("out");
        extract_bundle_safely(&bundle, &out).unwrap();

        assert!(out.join("repo--skills--pdf/SKILL.md").is_file());
    }

    #[test]
    fn install_skill_bundle_overwrites_custom_local_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let root_skills = tmp.path().join("skills");
        fs::create_dir_all(root_skills.join("pdf")).unwrap();
        fs::write(root_skills.join("pdf/SKILL.md"), "---\nname: local\n---").unwrap();
        let bundle = tmp.path().join("skill.tar.gz");
        write_tar_bundle_with_entry(
            &bundle,
            "repo--skills--pdf/SKILL.md",
            b"---\nname: remote\n---",
        );

        let result = install_skill_bundle_from_path(
            &bundle,
            &root_skills,
            HubSkillInstallInput {
                hub_url: "http://hub/api/v1".into(),
                hub_id: "repo--skills--pdf".into(),
                content_hash: "hash".into(),
                hub_updated_at: "2026-06-10T00:00:00Z".into(),
            },
        )
        .expect("install skill");

        assert_eq!(result.local_id, "pdf");
        assert!(fs::read_to_string(root_skills.join("pdf/SKILL.md"))
            .unwrap()
            .contains("remote"));
        assert!(root_skills.join("pdf/.hub_origin.json").is_file());
    }

    #[test]
    fn install_employee_bundle_copies_root_dependency_snapshots() {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::services::root_workspace::init_or_open(tmp.path()).unwrap();
        let root_skill_dir = root.skills_dir().join("content-marketer");
        fs::create_dir_all(&root_skill_dir).unwrap();
        fs::write(
            root_skill_dir.join("SKILL.md"),
            "---\nname: content-marketer\ndescription: content skill\n---\n",
        )
        .unwrap();
        crate::services::hub_state::write_origin(
            &root_skill_dir,
            &crate::services::hub_state::HubOrigin {
                kind: crate::services::hub_state::HubOriginKind::Skill,
                hub_url: "http://hub/api/v1".into(),
                hub_id: "repo--skills--content-marketer".into(),
                local_id: "content-marketer".into(),
                content_hash: Some("hash".into()),
                hub_updated_at: "2026-06-05T09:44:58Z".into(),
                installed_at: "2026-06-10T10:00:00Z".into(),
                skill_hub_ids: vec![],
            },
        )
        .unwrap();

        let src = tmp.path().join("employee-src/content-marketer");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("employee.json"),
            r#"{
              "id": "content-marketer",
              "name": "内容营销师",
              "description": "内容营销策略专家",
              "kind": "ordinary",
              "prompt_preview": "你是一名内容营销策略专家",
              "skill_ids": ["repo--skills--content-marketer"],
              "skill_count": 1,
              "tags": ["内容营销"],
              "source": {"type": "official"},
              "created_at": "2026-06-05T09:53:10Z",
              "updated_at": "2026-06-05T09:53:10Z"
            }"#,
        )
        .unwrap();
        fs::write(src.join("prompt.md"), "你是一名内容营销策略专家。").unwrap();
        fs::write(
            src.join("skills.json"),
            r#"["repo--skills--content-marketer"]"#,
        )
        .unwrap();
        let bundle = tmp.path().join("employee.tar.gz");
        write_tar_bundle_from_dir(&bundle, tmp.path().join("employee-src").as_path());

        let result = install_employee_bundle_from_path(
            &bundle,
            &root,
            HubEmployeeInstallInput {
                hub_url: "http://hub/api/v1".into(),
                hub_id: "content-marketer".into(),
                hub_updated_at: "2026-06-05T09:53:10Z".into(),
            },
        )
        .expect("install employee");

        assert_eq!(result.local_id, "content-marketer");
        assert!(root
            .employees_dir()
            .join("content-marketer/skills")
            .join("content-marketer/SKILL.md")
            .is_file());
    }

    #[test]
    fn install_employee_bundle_rejects_custom_dependency_before_registry_write() {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::services::root_workspace::init_or_open(tmp.path()).unwrap();
        let root_skill_dir = root.skills_dir().join("content-marketer");
        fs::create_dir_all(&root_skill_dir).unwrap();
        fs::write(root_skill_dir.join("SKILL.md"), "---\nname: local\n---\n").unwrap();

        let src = tmp.path().join("employee-src/content-marketer");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("employee.json"),
            r#"{
              "id": "content-marketer",
              "name": "内容营销师",
              "description": "内容营销策略专家",
              "kind": "ordinary",
              "prompt_preview": "",
              "skill_ids": ["repo--skills--content-marketer"],
              "skill_count": 1,
              "tags": [],
              "source": {"type": "official"},
              "created_at": "2026-06-05T09:53:10Z",
              "updated_at": "2026-06-05T09:53:10Z"
            }"#,
        )
        .unwrap();
        fs::write(src.join("prompt.md"), "prompt").unwrap();
        fs::write(
            src.join("skills.json"),
            r#"["repo--skills--content-marketer"]"#,
        )
        .unwrap();
        let bundle = tmp.path().join("employee.tar.gz");
        write_tar_bundle_from_dir(&bundle, tmp.path().join("employee-src").as_path());

        let err = install_employee_bundle_from_path(
            &bundle,
            &root,
            HubEmployeeInstallInput {
                hub_url: "http://hub/api/v1".into(),
                hub_id: "content-marketer".into(),
                hub_updated_at: "2026-06-05T09:53:10Z".into(),
            },
        )
        .unwrap_err();

        assert!(err.contains("local_custom_source"));
        assert!(crate::services::employee::get_detail(&root, "content-marketer").is_err());
    }
}
