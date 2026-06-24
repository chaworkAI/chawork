use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Domain Pack manifest from schema/domain.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainManifest {
    #[serde(alias = "domain_id")]
    pub id: String,
    #[serde(alias = "domain_name")]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub object_label: Option<String>,
    #[serde(default)]
    pub object_plural_label: Option<String>,
    #[serde(default)]
    pub default_object_type: Option<String>,
    #[serde(default)]
    pub primary_workflows: Vec<String>,
}

/// A template file entry
#[derive(Debug, Clone, Serialize)]
pub struct TemplateEntry {
    pub name: String,     // filename without extension
    pub filename: String, // full filename
    pub content: String,  // file content
}

/// Skill metadata from SKILL.md frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub dir_name: String, // directory name
}

/// UI labels from ui/labels.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiLabels {
    #[serde(flatten)]
    pub labels: serde_json::Value,
}

/// Complete Domain Pack
#[derive(Debug, Clone, Serialize)]
pub struct DomainPack {
    pub manifest: DomainManifest,
    pub agents_md: Option<String>,
    pub objects_schema: Option<serde_json::Value>,
    pub workflows: Option<serde_json::Value>,
    pub templates: Vec<TemplateEntry>,
    pub skills: Vec<SkillMeta>,
    pub labels: Option<UiLabels>,
}

/// Load a Domain Pack from a workspace path. Returns None if schema/domain.yaml doesn't exist.
pub fn load_domain_pack(workspace_path: &Path) -> Result<Option<DomainPack>, String> {
    let domain_yaml = workspace_path.join("schema/domain.yaml");
    if !domain_yaml.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&domain_yaml).map_err(|e| format!("无法读取 domain 配置: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let manifest: DomainManifest =
        serde_yaml::from_str(&raw).map_err(|e| format!("domain.yaml 格式无效: {e}"))?;

    let agents_path = workspace_path.join("schema/AGENTS.md");
    let agents_md = if agents_path.is_file() {
        Some(fs::read_to_string(&agents_path).map_err(|e| format!("无法读取 AGENTS.md: {e}"))?)
    } else {
        None
    };

    let objects_path = workspace_path.join("schema/objects.yaml");
    let objects_schema = if objects_path.is_file() {
        let s =
            fs::read_to_string(&objects_path).map_err(|e| format!("无法读取 objects.yaml: {e}"))?;
        Some(serde_yaml::from_str(&s).map_err(|e| format!("objects.yaml 格式无效: {e}"))?)
    } else {
        None
    };

    let workflows_path = workspace_path.join("schema/workflows.yaml");
    let workflows = if workflows_path.is_file() {
        let s = fs::read_to_string(&workflows_path)
            .map_err(|e| format!("无法读取 workflows.yaml: {e}"))?;
        Some(serde_yaml::from_str(&s).map_err(|e| format!("workflows.yaml 格式无效: {e}"))?)
    } else {
        None
    };

    let mut templates = Vec::new();
    let templates_dir = workspace_path.join("templates");
    if templates_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&templates_dir)
            .map_err(|e| format!("读取 templates 目录失败: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
            .collect();
        entries.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        for path in entries {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let content =
                fs::read_to_string(&path).map_err(|e| format!("无法读取模板 {filename}: {e}"))?;
            templates.push(TemplateEntry {
                name,
                filename,
                content,
            });
        }
    }

    let mut skills = Vec::new();
    let skills_dir = workspace_path.join("skills");
    if skills_dir.is_dir() {
        let mut dirs: Vec<_> = fs::read_dir(&skills_dir)
            .map_err(|e| format!("读取 skills 目录失败: {e}"))?
            .filter_map(|e| e.ok())
            .collect();
        dirs.sort_by_key(|e| e.path());
        for entry in dirs {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            let content =
                fs::read_to_string(&skill_md).map_err(|e| format!("无法读取 SKILL.md: {e}"))?;
            if let Some(mut meta) = parse_skill_frontmatter(&content) {
                meta.dir_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                skills.push(meta);
            }
        }
    }

    let labels_path = workspace_path.join("ui/labels.yaml");
    let labels = if labels_path.is_file() {
        let s = fs::read_to_string(&labels_path)
            .map_err(|e| format!("无法读取 ui/labels.yaml: {e}"))?;
        let v: serde_json::Value =
            serde_yaml::from_str(&s).map_err(|e| format!("ui/labels.yaml 格式无效: {e}"))?;
        Some(UiLabels { labels: v })
    } else {
        None
    };

    Ok(Some(DomainPack {
        manifest,
        agents_md,
        objects_schema,
        workflows,
        templates,
        skills,
        labels,
    }))
}

/// Extract YAML frontmatter from a markdown file (content between first --- pair)
fn parse_skill_frontmatter(content: &str) -> Option<SkillMeta> {
    let t = content.trim_start();
    if !t.starts_with("---") {
        return None;
    }
    let mut lines = t.lines();
    lines.next()?; // opening ---
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
    serde_yaml::from_str::<SkillMeta>(&yaml_buf).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::load_domain_pack;

    #[test]
    fn load_domain_pack_accepts_legacy_domain_id_and_name_fields() {
        let workspace =
            std::env::temp_dir().join(format!("chawork-domain-pack-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(workspace.join("schema")).expect("create schema dir");
        fs::write(
            workspace.join("schema/domain.yaml"),
            "domain_id: generic\ndomain_name: Test Workspace\nschema_version: dv_000001\ncollections: {}\n",
        )
        .expect("write domain yaml");

        let pack = load_domain_pack(&workspace)
            .expect("load domain pack")
            .expect("domain pack should exist");

        assert_eq!(pack.manifest.id, "generic");
        assert_eq!(pack.manifest.name, "Test Workspace");

        fs::remove_dir_all(workspace).expect("cleanup temp workspace");
    }
}
