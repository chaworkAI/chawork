//! Builds `CODEX_HOME` (`.chawork/codex-home/`) before Codex runs: config, AGENTS.md, skills.
//!
//! Runtime MCP boot is disabled for release stability. Workspace MCP management
//! files are preserved, but ordinary Chat runtime receives an empty MCP config.

use std::fs;
use std::path::Path;

use serde_json::Value;
use toml::Value as TomlValue;

use crate::services::domain_pack::{self, DomainPack};
use crate::services::employee::{self, BindingStatus};
use crate::services::global_provider;
use crate::services::qmd_index;
use crate::services::root_workspace::RootWorkspace;

const CHAWORK_MODEL_PROVIDER_ID: &str = "chawork_openai_compatible";
const CHAWORK_DEFAULT_SANDBOX: &str = "workspace-write";
const CHAWORK_DEFAULT_APPROVAL_POLICY: &str = "on-request";
const CHAWORK_DEVELOPER_INSTRUCTIONS: &str = r#"You are ChaWork, a local-first workspace assistant.

Primary role:
- Help the user organize, create, update, search, and summarize files in the current ChaWork workspace.
- Treat the current workspace as the writable project root for normal user-requested workspace edits.

Tool policy:
- Use the currently available Codex tools to inspect, search, create, and update workspace files.
- Runtime MCP workspace tools are not registered in ordinary Chat runtime for this release. Do not call `mcp__chawork_workspace__` tools.
- Do not claim the workspace is read-only. Use native Codex filesystem/exec tools within the configured workspace root.
- Keep file operations inside the current workspace unless the user explicitly asks for a global ChaWork action.

Boundaries:
- Only operate inside the current workspace unless the user explicitly asks for a global ChaWork action.
- Do not write ChaWork root skill catalog files directly; global skill promotion must go through ChaWork review/application flow.
"#;

/// Resolved employee context for a bound workspace.
struct EmployeeContext {
    employee_id: String,
    employee_name: String,
    prompt: String,
    /// Absolute path to the employee's prompt.md
    prompt_path: String,
    /// Enabled skills from the employee's skills.json, with their source dirs
    enabled_skills: Vec<EmployeeSkillRef>,
}

struct EmployeeSkillRef {
    id: String,
    /// Absolute path to the skill directory under `employees/<id>/skills/<skill_id>/`
    source_dir: std::path::PathBuf,
}

fn load_employee_context(workspace_path: &Path, root: &RootWorkspace) -> Option<EmployeeContext> {
    let binding = employee::read_workspace_binding(workspace_path)?;
    let employee_id = &binding.employee_id;

    let validation = employee::validate_binding(root, workspace_path).ok()?;
    if validation.status != BindingStatus::Bound {
        eprintln!(
            "[context_builder] workspace 绑定状态异常 ({:?})，回退到无员工模式: {}",
            validation.status, validation.message
        );
        return None;
    }

    let prompt = match employee::read_employee_prompt(root, employee_id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[context_builder] 读取员工 prompt 失败: {e}");
            String::new()
        }
    };

    let prompt_path = root
        .employees_dir()
        .join(employee_id)
        .join("prompt.md")
        .to_string_lossy()
        .into_owned();

    let skills_registry = match employee::read_skills_registry(root, employee_id) {
        Ok(sr) => sr,
        Err(e) => {
            eprintln!("[context_builder] 读取员工 skills.json 失败，使用空技能列表: {e}");
            employee::SkillRegistry {
                version: 1,
                skills: Vec::new(),
            }
        }
    };
    let skills_base = root.employees_dir().join(employee_id).join("skills");
    let enabled_skills: Vec<EmployeeSkillRef> = skills_registry
        .skills
        .iter()
        .filter(|s| s.enabled)
        .map(|s| EmployeeSkillRef {
            id: s.id.clone(),
            source_dir: skills_base.join(&s.id),
        })
        .collect();

    Some(EmployeeContext {
        employee_id: employee_id.clone(),
        employee_name: validation.employee_name.unwrap_or_default(),
        prompt,
        prompt_path,
        enabled_skills,
    })
}

/// Result of preparing CODEX_HOME for a workspace + root configuration.
#[derive(Clone)]
pub struct PreparedCodex {
    pub codex_home: String,
    pub runtime_home: String,
    /// effective provider 的 model 名。空字符串表示未配置。
    pub model: String,
    /// 来自 effective provider 的 API key（root 或 workspace override，根据 mode）。空字符串表示未配置。
    pub api_key: String,
    /// Explicit runtime workspace roots for ordinary Chat threads.
    pub runtime_workspace_roots: Vec<String>,
    /// Runtime contract execution policy for ordinary Chat threads.
    pub approval_policy: String,
    /// Runtime contract sandbox mode for ordinary Chat threads.
    pub sandbox: String,
}

/// Prepare the CODEX_HOME directory for a workspace.
/// Called before starting a Codex session or when Domain Pack/provider changes.
pub fn prepare_codex_home(
    workspace_path: &Path,
    root: &RootWorkspace,
) -> Result<PreparedCodex, String> {
    let codex_home = workspace_path.join(".chawork").join("codex-home");
    let runtime_home = workspace_path.join(".chawork").join("runtime-home");
    std::fs::create_dir_all(&codex_home).map_err(|e| format!("创建 codex-home 目录失败: {e}"))?;
    std::fs::create_dir_all(&runtime_home)
        .map_err(|e| format!("创建 runtime-home 目录失败: {e}"))?;

    if employee::read_workspace_binding(workspace_path).is_some() {
        let validation = employee::validate_binding(root, workspace_path)?;
        if validation.status != BindingStatus::Bound {
            return Err(format!("无法准备 runtime 上下文：{}", validation.message));
        }
    }

    let domain_pack = domain_pack::load_domain_pack(workspace_path)?;

    let employee_ctx = load_employee_context(workspace_path, root);

    let effective = global_provider::effective_provider(root, workspace_path).ok();

    write_config_toml(&codex_home, domain_pack.as_ref(), effective.as_ref())?;
    write_agents_md(
        &codex_home,
        workspace_path,
        domain_pack.as_ref(),
        employee_ctx.as_ref(),
    )?;
    prepare_skills(
        &codex_home,
        workspace_path,
        domain_pack.as_ref(),
        employee_ctx.as_ref(),
        &root.skills_dir(),
    )?;

    // Best-effort QMD index initialization (non-blocking to startup)
    if let Err(e) = qmd_index::initialize_qmd(workspace_path) {
        eprintln!("[context_builder] QMD 索引初始化警告: {e}");
    }

    // Runtime context summary
    let provider_source = match effective.as_ref() {
        Some(_) => "global",
        None => "none",
    };
    log_context_summary(workspace_path, employee_ctx.as_ref(), provider_source);

    let (model, api_key) = effective
        .as_ref()
        .map(|e| (e.model.clone(), e.openai_api_key.clone()))
        .unwrap_or_default();
    let workspace_root =
        std::fs::canonicalize(workspace_path).unwrap_or_else(|_| workspace_path.to_path_buf());
    Ok(PreparedCodex {
        codex_home: codex_home.to_string_lossy().to_string(),
        runtime_home: runtime_home.to_string_lossy().to_string(),
        model,
        api_key,
        runtime_workspace_roots: vec![workspace_root.to_string_lossy().to_string()],
        approval_policy: CHAWORK_DEFAULT_APPROVAL_POLICY.to_string(),
        sandbox: CHAWORK_DEFAULT_SANDBOX.to_string(),
    })
}

fn workspace_display_name(workspace_path: &Path) -> String {
    let state_path = workspace_path.join(".chawork/state/workspace.json");
    if let Ok(raw) = std::fs::read_to_string(&state_path) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            if let Some(n) = v.get("name").and_then(|x| x.as_str()) {
                let t = n.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
    }
    workspace_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Workspace")
        .to_string()
}

fn apply_provider_config(
    root: &mut toml::map::Map<String, TomlValue>,
    model: Option<String>,
    openai_base_url: &str,
) -> Result<(), String> {
    if let Some(m) = model {
        root.insert("model".to_string(), TomlValue::String(m));
    }

    let base_url = openai_base_url.trim();
    if base_url.is_empty() {
        root.remove("openai_base_url");
        if root
            .get("model_provider")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == CHAWORK_MODEL_PROVIDER_ID)
        {
            root.remove("model_provider");
        }
        if let Some(model_providers) = root
            .get_mut("model_providers")
            .and_then(|v| v.as_table_mut())
        {
            model_providers.remove(CHAWORK_MODEL_PROVIDER_ID);
            if model_providers.is_empty() {
                root.remove("model_providers");
            }
        }
        return Ok(());
    }

    // OpenAI-compatible providers such as DashScope expose HTTP/SSE Responses compatibility,
    // but not Codex's OpenAI Responses WebSocket transport. Use a dedicated provider so the
    // built-in OpenAI provider's websocket capability does not leak onto custom base URLs.
    root.remove("openai_base_url");
    root.insert(
        "model_provider".to_string(),
        TomlValue::String(CHAWORK_MODEL_PROVIDER_ID.to_string()),
    );

    let model_providers = root
        .entry("model_providers".to_string())
        .or_insert_with(|| TomlValue::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| {
            "config.toml 中 model_providers 不是表，无法合并 ChaWork 托管项".to_string()
        })?;

    let mut provider = toml::map::Map::new();
    provider.insert(
        "name".to_string(),
        TomlValue::String("ChaWork OpenAI Compatible".to_string()),
    );
    provider.insert(
        "base_url".to_string(),
        TomlValue::String(base_url.to_string()),
    );
    provider.insert(
        "env_key".to_string(),
        TomlValue::String("OPENAI_API_KEY".to_string()),
    );
    provider.insert(
        "wire_api".to_string(),
        TomlValue::String("responses".to_string()),
    );
    provider.insert("supports_websockets".to_string(), TomlValue::Boolean(false));
    model_providers.insert(
        CHAWORK_MODEL_PROVIDER_ID.to_string(),
        TomlValue::Table(provider),
    );

    Ok(())
}

fn managed_skills_config(include_instructions: bool) -> TomlValue {
    let mut skills = toml::map::Map::new();
    skills.insert(
        "include_instructions".to_string(),
        TomlValue::Boolean(include_instructions),
    );
    skills.insert(
        "root_policy".to_string(),
        TomlValue::String("codex_home_only".to_string()),
    );

    let mut bundled = toml::map::Map::new();
    bundled.insert("enabled".to_string(), TomlValue::Boolean(false));
    skills.insert("bundled".to_string(), TomlValue::Table(bundled));

    TomlValue::Table(skills)
}

fn write_config_toml(
    codex_home: &Path,
    _domain_pack: Option<&DomainPack>,
    effective: Option<&global_provider::EffectiveProvider>,
) -> Result<(), String> {
    let cfg_path = codex_home.join("config.toml");

    let mut root: toml::map::Map<String, TomlValue> = if cfg_path.is_file() {
        let raw =
            fs::read_to_string(&cfg_path).map_err(|e| format!("读取 config.toml 失败: {e}"))?;
        match raw.parse::<TomlValue>() {
            Ok(TomlValue::Table(t)) => t,
            Ok(_) => toml::map::Map::new(),
            Err(e) => {
                eprintln!("[context_builder] 解析既有 config.toml 失败，将重建托管段: {e}");
                toml::map::Map::new()
            }
        }
    } else {
        toml::map::Map::new()
    };

    let (model_opt, base_url) = match effective {
        Some(e) => (Some(e.model.clone()), e.openai_base_url.clone()),
        None => (None, String::new()),
    };
    apply_provider_config(&mut root, model_opt, &base_url)?;
    root.insert(
        "sandbox_mode".to_string(),
        TomlValue::String(CHAWORK_DEFAULT_SANDBOX.to_string()),
    );
    root.insert(
        "approval_policy".to_string(),
        TomlValue::String(CHAWORK_DEFAULT_APPROVAL_POLICY.to_string()),
    );
    if !matches!(root.get("features"), Some(TomlValue::Table(_))) {
        root.insert(
            "features".to_string(),
            TomlValue::Table(toml::map::Map::new()),
        );
    }
    let features = root
        .get_mut("features")
        .and_then(TomlValue::as_table_mut)
        .expect("features was normalized to a table");
    features.insert(
        "default_mode_request_user_input".to_string(),
        TomlValue::Boolean(true),
    );
    features.insert("plugins".to_string(), TomlValue::Boolean(false));
    features.insert("plugin_hooks".to_string(), TomlValue::Boolean(false));
    root.insert(
        "skills".to_string(),
        managed_skills_config(/*include_instructions*/ true),
    );
    root.insert(
        "include_permissions_instructions".to_string(),
        TomlValue::Boolean(false),
    );
    root.insert(
        "developer_instructions".to_string(),
        TomlValue::String(CHAWORK_DEVELOPER_INSTRUCTIONS.to_string()),
    );
    root.insert(
        "mcp_servers".to_string(),
        TomlValue::Table(toml::map::Map::new()),
    );

    let out_doc = TomlValue::Table(root);
    let serialized =
        toml::to_string_pretty(&out_doc).map_err(|e| format!("序列化 config.toml 失败: {e}"))?;

    let mut out = String::from(
        "# ChaWork — 启动时合并写入：保留非托管配置，重写 ChaWork 管理的 runtime 启动上下文。\n\
         # 托管段：`model`、ChaWork OpenAI-compatible provider、sandbox、approval、skills 和 features。\n\
         # ChaWork 固定使用 workspace-write sandbox；不要依赖用户全局 Codex 配置。\n\
         # API Key 由 ChaWork 在启动 Runtime 子进程时注入为环境变量 OPENAI_API_KEY（不写进本文件）。\n\
         # Runtime MCP 已关闭；`mcp_servers` 被有意写成空表，workspace MCP 管理配置不进入普通 Chat runtime。\n\n",
    );
    out.push_str(&serialized);

    fs::write(&cfg_path, out).map_err(|e| format!("写入 config.toml 失败: {e}"))?;
    Ok(())
}

fn write_agents_md(
    codex_home: &Path,
    workspace_path: &Path,
    domain_pack: Option<&DomainPack>,
    employee_ctx: Option<&EmployeeContext>,
) -> Result<(), String> {
    let path = codex_home.join("AGENTS.md");
    let ws_name = workspace_display_name(workspace_path);
    let chawork_contract = build_chawork_runtime_contract();

    let tools_section = build_workspace_tools_section(workspace_path);

    let employee_section = build_employee_section(employee_ctx);
    let project_context = build_project_context(domain_pack, employee_ctx.is_some());

    let header = if let Some(ctx) = employee_ctx {
        let domain_line = domain_pack
            .map(|p| format!("\n领域: {}", p.manifest.name))
            .unwrap_or_default();
        format!(
            "# ChaWork 工作助手 — {}\n\n\
             当前工作区: {ws_name}{domain_line}\n\
             当前员工: {} ({})\n",
            ctx.employee_name, ctx.employee_name, ctx.employee_id,
        )
    } else if let Some(pack) = domain_pack {
        format!(
            "# ChaWork 工作助手\n\n\
             当前工作区: {ws_name}\n\
             领域: {}\n",
            pack.manifest.name,
        )
    } else {
        format!("# ChaWork 工作助手\n\n当前工作区: {ws_name}\n")
    };

    let fallback_instructions = if employee_ctx.is_none() && domain_pack.is_none() {
        "\n你是 ChaWork 的工作助手，在本工作区内帮助用户整理资料、记录与检索。\
         若工作区包含 schema 与 Domain Pack，请以其中的约定为准。\n"
            .to_string()
    } else {
        String::new()
    };

    let body = format!(
        "{header}\n\
         ---\n\n\
         {tools_section}\n\
         ---\n\
         {employee_section}\
         {project_context}\
         {fallback_instructions}\
         {chawork_contract}"
    );

    std::fs::write(&path, body).map_err(|e| format!("写入 AGENTS.md 失败: {e}"))?;
    Ok(())
}

fn build_employee_section(employee_ctx: Option<&EmployeeContext>) -> String {
    let Some(ctx) = employee_ctx else {
        return String::new();
    };
    let prompt = ctx.prompt.trim();
    if prompt.is_empty() {
        return format!(
            "\n## 员工身份\n\n\
             你是 **{}**（ID: `{}`）。该员工尚未配置 prompt，请按通用工作助手模式运行。\n",
            ctx.employee_name, ctx.employee_id,
        );
    }
    format!(
        "\n## 员工身份与指令\n\n\
         你是 **{}**（ID: `{}`）。以下是你的核心指令，优先级高于通用 ChaWork 助手说明：\n\n\
         {prompt}\n",
        ctx.employee_name, ctx.employee_id,
    )
}

fn build_chawork_runtime_contract() -> String {
    let mut out = String::from(
        "\n\n## ChaWork Runtime Contract\n\n\
         - 你运行在 ChaWork 工作助手中，不是普通代码编辑器助手。\n\
         - 当前 workspace 是用户授权的工作目录。\n\
         - 不要要求用户自己运行 `echo ... > file` 之类命令来完成普通文件创建或修改。\n",
    );
    out.push_str(
        "- Runtime MCP 当前关闭；不要调用 `mcp__chawork_workspace__` namespace，也不要声称 `search_knowledge` / `write_file` MCP 工具可用。\n\
         - 可以使用当前可用的 Codex 原生文件、搜索或命令工具在 workspace 边界内完成任务。\n",
    );
    out.push('\n');
    out
}

/// Build project-level context from Domain Pack schema (AGENTS.md, objects, templates).
/// When employee is bound, workflows.yaml is NOT injected (it's not an employee definition source).
fn build_project_context(domain_pack: Option<&DomainPack>, has_employee: bool) -> String {
    let Some(pack) = domain_pack else {
        return String::new();
    };

    let mut ctx = String::new();

    if let Some(ref agents_md) = pack.agents_md {
        ctx.push_str("\n\n## 项目说明 (schema/AGENTS.md)\n\n");
        ctx.push_str(agents_md);
        ctx.push('\n');
    }

    if let Some(ref objects) = pack.objects_schema {
        if let Ok(yaml) = serde_yaml::to_string(objects) {
            ctx.push_str("\n\n## 对象定义 (objects.yaml)\n\n```yaml\n");
            ctx.push_str(&yaml);
            ctx.push_str("```\n");
        }
    }

    // workflows.yaml: only inject when NO employee is bound (legacy fallback).
    // With an employee bound, workflows.yaml should NOT be treated as employee definition.
    if !has_employee {
        if let Some(ref workflows) = pack.workflows {
            if let Ok(yaml) = serde_yaml::to_string(workflows) {
                ctx.push_str("\n\n## 工作流 (workflows.yaml)\n\n```yaml\n");
                ctx.push_str(&yaml);
                ctx.push_str("```\n");
            }
        }
    }

    if !pack.templates.is_empty() {
        ctx.push_str("\n\n## 可用模板\n\n");
        for t in &pack.templates {
            ctx.push_str(&format!("- `templates/{}`\n", t.filename));
        }
    }

    ctx
}

/// Builds the workspace inventory section injected into every AGENTS.md.
fn build_workspace_tools_section(workspace_path: &Path) -> String {
    let mut out = String::from(
        "## Workspace Tools\n\n\
         Runtime MCP workspace tools are disabled for this release. Do not call `mcp__chawork_workspace__` tools.\n\n\
         Use native Codex file/search/exec tools within the current workspace root. For knowledge lookup, inspect `wiki/`, `raw/`, `templates/`, and other workspace files directly.\n",
    );
    append_workspace_documents_inventory(workspace_path, &mut out);
    out
}

fn append_workspace_documents_inventory(workspace_path: &Path, out: &mut String) {
    let docs_dir = workspace_path.join("wiki").join("documents");
    if let Ok(entries) = std::fs::read_dir(&docs_dir) {
        let mut files: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        if !files.is_empty() {
            out.push_str("\n## 当前 wiki/documents/ 文件清单\n\n");
            let total = files.len();
            for f in files.iter().take(30) {
                out.push_str(&format!("- `wiki/documents/{f}`\n"));
            }
            if total > 30 {
                out.push_str(&format!("- …其余 {} 个文件未列出。\n", total - 30));
            }
        } else {
            out.push_str("\n（`wiki/documents/` 为空，用户尚未导入任何资料。）\n");
        }
    }
}

fn prepare_skills(
    codex_home: &Path,
    workspace_path: &Path,
    domain_pack: Option<&DomainPack>,
    employee_ctx: Option<&EmployeeContext>,
    root_skills_dir: &Path,
) -> Result<(), String> {
    let skills_root = codex_home.join("skills");

    // Clean previous skills
    if skills_root.is_dir() {
        let _ = std::fs::remove_dir_all(&skills_root);
    }
    std::fs::create_dir_all(&skills_root).map_err(|e| format!("创建 skills 目录失败: {e}"))?;

    if let Some(ctx) = employee_ctx {
        // Employee-bound mode: sync ENABLED employee skills only.
        // Do NOT sync workspace-local skills/ or Domain Pack skills.
        for skill_ref in &ctx.enabled_skills {
            let skill_md = skill_ref.source_dir.join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            let dest_dir = skills_root.join(&skill_ref.id);
            copy_skill_dir(&skill_ref.source_dir, &dest_dir)
                .map_err(|e| format!("同步员工技能 {} 失败: {e}", skill_ref.id))?;
        }
    } else {
        // Unbound legacy: domain pack skills from workspace skills/ tree.
        if let Some(pack) = domain_pack {
            for skill in &pack.skills {
                let dest_dir = skills_root.join(&skill.dir_name);
                if dest_dir.is_dir() {
                    let _ = std::fs::remove_dir_all(&dest_dir);
                }
            }

            for skill in &pack.skills {
                let src = workspace_path
                    .join("skills")
                    .join(&skill.dir_name)
                    .join("SKILL.md");
                if !src.is_file() {
                    continue;
                }
                let dest_dir = skills_root.join(&skill.dir_name);
                std::fs::create_dir_all(&dest_dir)
                    .map_err(|e| format!("创建技能目录 {} 失败: {e}", dest_dir.display()))?;
                std::fs::copy(&src, dest_dir.join("SKILL.md"))
                    .map_err(|e| format!("复制技能 {} 失败: {e}", skill.dir_name))?;
            }
        }

        // Enabled root skills from .chawork/skills.json.
        if let Some(selection) = crate::services::skill::read_skill_selection(workspace_path) {
            for (id, entry) in &selection.root_skills {
                if !entry.enabled {
                    continue;
                }
                let src = root_skills_dir.join(id).join("SKILL.md");
                if !src.is_file() {
                    continue;
                }
                let dest_dir = skills_root.join(id);
                std::fs::create_dir_all(&dest_dir)
                    .map_err(|e| format!("创建技能目录 {} 失败: {e}", dest_dir.display()))?;
                std::fs::copy(&src, dest_dir.join("SKILL.md"))
                    .map_err(|e| format!("复制 root 技能 {id} 失败: {e}"))?;
            }
        }
    }

    Ok(())
}

fn copy_skill_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("技能源目录不存在: {}", src.display()));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {e}"))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("读取源目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_skill_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("复制文件失败: {e}"))?;
        }
    }
    Ok(())
}

fn log_context_summary(
    workspace_path: &Path,
    employee_ctx: Option<&EmployeeContext>,
    provider_source: &str,
) {
    match employee_ctx {
        Some(ctx) => {
            eprintln!(
                "[context_builder] runtime context: employee_id={}, prompt={}, skills_count={}, workspace={}, provider={}",
                ctx.employee_id,
                ctx.prompt_path,
                ctx.enabled_skills.len(),
                workspace_path.display(),
                provider_source,
            );
        }
        None => {
            eprintln!(
                "[context_builder] runtime context: employee=none (unbound), workspace={}, provider={}",
                workspace_path.display(),
                provider_source,
            );
        }
    }
}

// ── Dream Runtime context builder ─────────────────────────────────────────

/// Result of preparing CODEX_HOME for a Dream run.
#[derive(Clone)]
pub struct PreparedDreamCodex {
    pub codex_home: String,
    pub runtime_home: String,
    pub model: String,
    pub api_key: String,
}

/// Prepare a minimal CODEX_HOME for Dream runtime execution.
///
/// Unlike `prepare_codex_home` this omits domain-pack, workspace MCP, and wiki —
/// the Dream runtime only needs the `__dream__` employee's prompt/skills,
/// target employee context, and the provider configuration.
pub fn prepare_dream_codex_home(
    run_workspace: &Path,
    root: &RootWorkspace,
    _target_employee_id: &str,
    _dream_run_id: &str,
    _phase: u8,
) -> Result<PreparedDreamCodex, String> {
    let codex_home = run_workspace.join(".chawork").join("codex-home");
    let runtime_home = run_workspace.join(".chawork").join("runtime-home");
    fs::create_dir_all(&codex_home).map_err(|e| format!("创建 dream codex-home 失败: {e}"))?;
    fs::create_dir_all(&runtime_home).map_err(|e| format!("创建 dream runtime-home 失败: {e}"))?;

    let global = global_provider::read_global(root);

    write_dream_config_toml(&codex_home, run_workspace, &global)?;

    Ok(PreparedDreamCodex {
        codex_home: codex_home.to_string_lossy().to_string(),
        runtime_home: runtime_home.to_string_lossy().to_string(),
        model: global.model.clone(),
        api_key: global.openai_api_key.clone(),
    })
}

/// Minimal config.toml for Dream runs — no MCP server, workspace-write sandbox.
fn write_dream_config_toml(
    codex_home: &Path,
    _run_workspace: &Path,
    global: &global_provider::GlobalProvider,
) -> Result<(), String> {
    let cfg_path = codex_home.join("config.toml");

    let mut root_map: toml::map::Map<String, TomlValue> = toml::map::Map::new();

    let model_opt = if global.model.trim().is_empty() {
        None
    } else {
        Some(global.model.clone())
    };
    apply_provider_config(&mut root_map, model_opt, &global.openai_base_url)?;

    root_map.insert(
        "sandbox_mode".to_string(),
        TomlValue::String("workspace-write".to_string()),
    );
    root_map.insert(
        "approval_policy".to_string(),
        TomlValue::String("never".to_string()),
    );
    root_map.insert(
        "include_permissions_instructions".to_string(),
        TomlValue::Boolean(false),
    );
    root_map.insert(
        "skills".to_string(),
        managed_skills_config(/*include_instructions*/ false),
    );
    let mut features = toml::map::Map::new();
    features.insert("plugins".to_string(), TomlValue::Boolean(false));
    features.insert("plugin_hooks".to_string(), TomlValue::Boolean(false));
    root_map.insert("features".to_string(), TomlValue::Table(features));

    let out_doc = TomlValue::Table(root_map);
    let serialized = toml::to_string_pretty(&out_doc)
        .map_err(|e| format!("序列化 dream config.toml 失败: {e}"))?;
    fs::write(&cfg_path, serialized).map_err(|e| format!("写入 dream config.toml 失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_config_uses_http_responses_provider_for_custom_base_url() {
        let mut root = toml::map::Map::new();

        apply_provider_config(
            &mut root,
            Some("qwen3.6-plus".to_string()),
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        )
        .expect("apply provider config");

        assert_eq!(
            root.get("model").and_then(|v| v.as_str()),
            Some("qwen3.6-plus")
        );
        assert_eq!(
            root.get("model_provider").and_then(|v| v.as_str()),
            Some(CHAWORK_MODEL_PROVIDER_ID)
        );
        assert!(
            root.get("openai_base_url").is_none(),
            "custom base URLs should not use the built-in OpenAI provider"
        );

        let provider = root
            .get("model_providers")
            .and_then(|v| v.as_table())
            .and_then(|providers| providers.get(CHAWORK_MODEL_PROVIDER_ID))
            .and_then(|v| v.as_table())
            .expect("managed provider table");

        assert_eq!(
            provider.get("base_url").and_then(|v| v.as_str()),
            Some("https://dashscope.aliyuncs.com/compatible-mode/v1")
        );
        assert_eq!(
            provider.get("env_key").and_then(|v| v.as_str()),
            Some("OPENAI_API_KEY")
        );
        assert_eq!(
            provider.get("wire_api").and_then(|v| v.as_str()),
            Some("responses")
        );
        assert_eq!(
            provider
                .get("supports_websockets")
                .and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn prepare_dream_codex_home_uses_valid_noninteractive_approval_policy() {
        use crate::services::root_workspace;

        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        let run_workspace = tmp.path().join("dream-run");
        std::fs::create_dir_all(&run_workspace).expect("run workspace");

        let prepared =
            prepare_dream_codex_home(&run_workspace, &root, "dream-emp", "dream-run-test", 1)
                .expect("prepare dream codex home");

        let config_path = std::path::Path::new(&prepared.codex_home).join("config.toml");
        let raw = std::fs::read_to_string(config_path).expect("read dream config");
        let parsed: TomlValue = raw.parse().expect("parse dream config toml");

        assert_eq!(
            parsed.get("approval_policy").and_then(|v| v.as_str()),
            Some("never")
        );
        assert!(
            std::path::Path::new(&prepared.runtime_home).is_dir(),
            "dream runtime home should be created"
        );
        let skills = parsed
            .get("skills")
            .and_then(|v| v.as_table())
            .expect("dream skills table");
        assert_eq!(
            skills.get("include_instructions").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            skills.get("root_policy").and_then(|v| v.as_str()),
            Some("codex_home_only")
        );
        assert_eq!(
            skills
                .get("bundled")
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool()),
            Some(false)
        );
        let features = parsed
            .get("features")
            .and_then(|v| v.as_table())
            .expect("dream features table");
        assert_eq!(
            features.get("plugins").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            features.get("plugin_hooks").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn prepare_codex_home_writes_managed_skill_isolation_config() {
        use crate::services::root_workspace;

        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        let ws_path = tmp.path().join("skill-isolation-ws");
        std::fs::create_dir_all(ws_path.join(".chawork")).expect("ws dirs");

        let prepared = prepare_codex_home(&ws_path, &root).expect("prepare codex home");
        assert!(
            std::path::Path::new(&prepared.runtime_home).is_dir(),
            "workspace runtime home should be created"
        );
        assert_eq!(
            prepared.runtime_home,
            ws_path
                .join(".chawork/runtime-home")
                .to_string_lossy()
                .to_string()
        );

        let raw =
            std::fs::read_to_string(std::path::Path::new(&prepared.codex_home).join("config.toml"))
                .expect("read config");
        let parsed: TomlValue = raw.parse().expect("parse config toml");
        let skills = parsed
            .get("skills")
            .and_then(|v| v.as_table())
            .expect("skills table");
        assert_eq!(
            skills.get("include_instructions").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            skills.get("root_policy").and_then(|v| v.as_str()),
            Some("codex_home_only")
        );
        assert_eq!(
            skills
                .get("bundled")
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(
            skills.get("config").is_none(),
            "managed config must not preserve skills.config overrides"
        );
        let features = parsed
            .get("features")
            .and_then(|v| v.as_table())
            .expect("features table");
        assert_eq!(
            features.get("plugins").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            features.get("plugin_hooks").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn prepare_codex_home_omits_runtime_mcp_by_default() {
        use crate::services::mcp_tool;
        use crate::services::root_workspace;

        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        let ws_path = tmp.path().join("mcp-ws");
        std::fs::create_dir_all(ws_path.join(".chawork")).expect("ws dirs");

        mcp_tool::import_mcp_servers_json(
            &ws_path,
            r#"{
              "mcpServers": {
                "xsct-bench": {
                  "type": "streamable_http",
                  "url": "https://mcp.api-inference.modelscope.net/6460f2d4ed8347/mcp",
                  "headers": {
                    "Authorization": "Bearer ms-inline-token"
                  }
                }
              }
            }"#,
        )
        .expect("import mcp server");

        let codex_home = ws_path.join(".chawork/codex-home");
        std::fs::create_dir_all(&codex_home).expect("codex-home");
        std::fs::write(
            codex_home.join("config.toml"),
            r#"[mcp_servers.chawork_workspace]
command = "/tmp/old-chawork-mcp-server"

[mcp_servers.old_custom]
url = "https://example.com/old"
"#,
        )
        .expect("write old config");

        let prepared = prepare_codex_home(&ws_path, &root).expect("prepare codex home");
        assert_eq!(prepared.approval_policy, CHAWORK_DEFAULT_APPROVAL_POLICY);
        assert_eq!(prepared.sandbox, CHAWORK_DEFAULT_SANDBOX);
        assert_eq!(prepared.runtime_workspace_roots.len(), 1);
        assert_eq!(
            prepared.runtime_workspace_roots[0],
            std::fs::canonicalize(&ws_path)
                .expect("canonical workspace")
                .to_string_lossy()
        );
        let raw =
            std::fs::read_to_string(std::path::Path::new(&prepared.codex_home).join("config.toml"))
                .expect("read config");
        let parsed: TomlValue = raw.parse().expect("parse config toml");
        assert!(
            parsed
                .get("mcp_servers")
                .and_then(|v| v.as_table())
                .is_some_and(|table| table.is_empty()),
            "runtime MCP config should be written as an explicit empty table"
        );

        std::fs::write(
            ws_path.join(".chawork/mcp-servers.json"),
            r#"{
              "version": 1,
              "updated_at": "2026-06-04T00:00:00Z",
              "servers": {
                "weather": {
                  "name": "weather",
                  "type": "streamable_http",
                  "url": "https://example.com/mcp",
                  "enabled": true,
                  "headers": {},
                  "tools": [
                    { "name": "get_weather", "description": "获取未来7天天气" }
                  ],
                  "last_tested_at": "2026-06-04T00:00:00Z"
                }
              }
            }"#,
        )
        .expect("write cached mcp tools");
        let prepared = prepare_codex_home(&ws_path, &root).expect("prepare codex home again");
        let raw =
            std::fs::read_to_string(std::path::Path::new(&prepared.codex_home).join("config.toml"))
                .expect("read config again");
        let parsed: TomlValue = raw.parse().expect("parse config toml again");
        assert!(
            parsed
                .get("mcp_servers")
                .and_then(|v| v.as_table())
                .is_some_and(|table| table.is_empty()),
            "custom MCP servers should stay out of runtime config while MCP is disabled"
        );
        let agents =
            std::fs::read_to_string(std::path::Path::new(&prepared.codex_home).join("AGENTS.md"))
                .expect("read AGENTS.md");
        assert!(agents.contains("Runtime MCP 当前关闭"));
        assert!(agents.contains("Runtime MCP workspace tools are disabled"));
        assert!(!agents.contains("工作区自定义 MCP 工具"));
        assert!(!agents.contains("Tool `get_weather`"));
    }

    /// MVP §3.4: bound workspace runtime context uses employee prompt (not legacy workflows).
    #[test]
    fn prepare_codex_home_injects_bound_employee_prompt() {
        use crate::services::employee::{self, CreateEmployeeInput};
        use crate::services::root_workspace;
        use crate::services::workspace;

        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");

        employee::create(
            &root,
            CreateEmployeeInput {
                id: "ctx-emp".to_string(),
                name: "Context Emp".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create employee");

        let prompt_marker = "CTX_E2E_EMPLOYEE_PROMPT_MARKER";
        let prompt_path = root.employees_dir().join("ctx-emp/prompt.md");
        std::fs::write(&prompt_path, prompt_marker).expect("write employee prompt");

        let ws_path = tmp.path().join("ctx-ws");
        std::fs::create_dir_all(ws_path.join(".chawork/state")).expect("ws dirs");
        let ws_state = workspace::WorkspaceState {
            id: uuid::Uuid::new_v4().to_string(),
            name: "ctx-ws".to_string(),
            path: ws_path.to_string_lossy().into_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_active_at: chrono::Utc::now().to_rfc3339(),
            active_session_id: None,
            domain_pack_id: None,
            index_status: "stale".to_string(),
            pending_proposals_count: 0,
            bound_employee_name: None,
            bound_employee_id: None,
        };
        std::fs::write(
            ws_path.join(".chawork/state/workspace.json"),
            serde_json::to_string_pretty(&ws_state).expect("json"),
        )
        .expect("write ws json");

        std::fs::create_dir_all(ws_path.join("schema")).expect("schema dir");
        std::fs::write(ws_path.join("schema/workflows.yaml"), "legacy: true\n")
            .expect("legacy workflows");

        employee::bind_workspace(&root, "ctx-emp", &ws_path, &ws_state.id, &ws_state.name)
            .expect("bind");

        let prepared = prepare_codex_home(&ws_path, &root).expect("prepare codex home");
        let agents =
            std::fs::read_to_string(std::path::Path::new(&prepared.codex_home).join("AGENTS.md"))
                .expect("read AGENTS.md");
        assert!(agents.contains(prompt_marker));
        assert!(agents.contains("ctx-emp"));
        assert!(
            !agents.contains("工作流 (workflows.yaml)"),
            "bound workspace must not inject workflows.yaml as employee definition"
        );
    }

    #[test]
    fn prepare_codex_home_rejects_invalid_binding() {
        use crate::services::employee::{self, CreateEmployeeInput};
        use crate::services::root_workspace;

        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        employee::create(
            &root,
            CreateEmployeeInput {
                id: "bad-bind-emp".to_string(),
                name: "Bad Bind".to_string(),
                description: String::new(),
                initial_prompt: String::new(),
                root_skill_ids: vec![],
            },
        )
        .expect("create employee");

        let ws_path = tmp.path().join("bad-bind-ws");
        std::fs::create_dir_all(ws_path.join(".chawork")).expect("ws dirs");
        std::fs::write(
            ws_path.join(".chawork/employee.json"),
            r#"{"employee_id":"bad-bind-emp"}"#,
        )
        .expect("write binding");

        match prepare_codex_home(&ws_path, &root) {
            Err(msg) => assert!(msg.contains("无法准备 runtime 上下文")),
            Ok(_) => panic!("expected invalid binding to fail prepare_codex_home"),
        }
    }

    #[test]
    fn prepare_codex_home_syncs_employee_skills() {
        use crate::services::employee::{self, CreateEmployeeInput};
        use crate::services::root_workspace;
        use crate::services::workspace;

        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");

        let skill_dir = root.skills_dir().join("sync-skill");
        std::fs::create_dir_all(&skill_dir).expect("skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Sync Skill\ndescription: sync test\n---\n",
        )
        .expect("write skill");

        employee::create(
            &root,
            CreateEmployeeInput {
                id: "sync-emp".to_string(),
                name: "Sync Emp".to_string(),
                description: String::new(),
                initial_prompt: "sync prompt".to_string(),
                root_skill_ids: vec!["sync-skill".to_string()],
            },
        )
        .expect("create employee");

        let ws_path = tmp.path().join("sync-ws");
        std::fs::create_dir_all(ws_path.join(".chawork/state")).expect("ws dirs");
        let ws_state = workspace::WorkspaceState {
            id: uuid::Uuid::new_v4().to_string(),
            name: "sync-ws".to_string(),
            path: ws_path.to_string_lossy().into_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_active_at: chrono::Utc::now().to_rfc3339(),
            active_session_id: None,
            domain_pack_id: None,
            index_status: "stale".to_string(),
            pending_proposals_count: 0,
            bound_employee_name: None,
            bound_employee_id: None,
        };
        std::fs::write(
            ws_path.join(".chawork/state/workspace.json"),
            serde_json::to_string_pretty(&ws_state).expect("json"),
        )
        .expect("write ws json");

        employee::bind_workspace(&root, "sync-emp", &ws_path, &ws_state.id, &ws_state.name)
            .expect("bind");

        let prepared = prepare_codex_home(&ws_path, &root).expect("prepare");
        let skill_md =
            std::path::Path::new(&prepared.codex_home).join("skills/sync-skill/SKILL.md");
        assert!(
            skill_md.is_file(),
            "employee skill should sync into codex-home"
        );
    }
}
