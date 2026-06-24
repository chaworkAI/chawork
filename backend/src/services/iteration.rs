//! Iteration Service — applies accepted proposals to workspace files
//! (schema, skills, templates) and logs changes to `schema/iteration_log.md`.
//!
//! After applying changes, refreshes the Codex environment so the next turn
//! picks up updated Domain Pack rules.

use std::fs;
use std::path::Path;

use chrono::Utc;

use crate::path_safety;
use crate::services::context_builder;
use crate::services::proposal::{Proposal, ProposalType};
use crate::services::root_workspace::RootWorkspace;

/// Apply a proposal's changes to the workspace, then refresh Codex context.
pub fn apply_iteration(
    workspace_path: &Path,
    root: &RootWorkspace,
    proposal: &Proposal,
) -> Result<(), String> {
    match proposal.proposal_type {
        ProposalType::SchemaUpdate => apply_file_update(workspace_path, proposal)?,
        ProposalType::SkillUpdate => apply_file_update(workspace_path, proposal)?,
        ProposalType::TemplateUpdate => apply_file_update(workspace_path, proposal)?,
        ProposalType::WikiUpdate => apply_file_update(workspace_path, proposal)?,
        ProposalType::ReportDraft => apply_file_update(workspace_path, proposal)?,
    }

    log_iteration(workspace_path, proposal)?;

    // Refresh Codex environment to pick up new rules
    if matches!(
        proposal.proposal_type,
        ProposalType::SchemaUpdate | ProposalType::SkillUpdate | ProposalType::TemplateUpdate
    ) {
        if let Err(e) = context_builder::prepare_codex_home(workspace_path, root) {
            eprintln!("[iteration] Codex 环境刷新警告: {e}");
        }
    }

    Ok(())
}

fn apply_file_update(workspace_path: &Path, proposal: &Proposal) -> Result<(), String> {
    let Some(ref target) = proposal.target_path else {
        return Ok(());
    };

    let ws_root =
        std::fs::canonicalize(workspace_path).map_err(|e| format!("工作区路径无效: {e}"))?;
    let joined = path_safety::safe_join_workspace(&ws_root, target)?;
    let full_path = if joined.exists() {
        fs::canonicalize(&joined).map_err(|e| format!("路径无效: {e}"))?
    } else {
        let parent = joined.parent().ok_or_else(|| "无效目标路径".to_string())?;
        fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {e}"))?;
        let canon_parent = fs::canonicalize(parent).map_err(|e| format!("路径无效: {e}"))?;
        if !canon_parent.starts_with(&ws_root) {
            return Err("路径不在工作区范围内".to_string());
        }
        let name = joined
            .file_name()
            .ok_or_else(|| "无效目标路径".to_string())?;
        canon_parent.join(name)
    };

    if !full_path.starts_with(&ws_root) {
        return Err("路径不在工作区范围内".to_string());
    }

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {e}"))?;
    }

    if let Some(ref content) = proposal.new_content {
        fs::write(&full_path, content).map_err(|e| format!("写入文件失败: {e}"))?;
        return Ok(());
    }

    // If diff is provided but no new_content, log it
    if proposal.diff.is_some() {
        eprintln!(
            "[iteration] Proposal {} has diff but no new_content; skip file write for {}",
            proposal.id, target
        );
    }

    Ok(())
}

fn log_iteration(workspace_path: &Path, proposal: &Proposal) -> Result<(), String> {
    let log_path = workspace_path.join("schema").join("iteration_log.md");

    // Ensure schema/ exists
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 schema 目录失败: {e}"))?;
    }

    let timestamp = Utc::now().format("%Y-%m-%d %H:%M").to_string();
    let type_label = match proposal.proposal_type {
        ProposalType::SchemaUpdate => "Schema 更新",
        ProposalType::SkillUpdate => "Skill 更新",
        ProposalType::TemplateUpdate => "模板更新",
        ProposalType::WikiUpdate => "Wiki 更新",
        ProposalType::ReportDraft => "报告草稿",
    };

    let entry = format!(
        "\n## [{timestamp}] {type_label}: {title}\n\n{desc}\n\n目标: {target}\n\n---\n",
        title = proposal.title,
        desc = proposal.description,
        target = proposal.target_path.as_deref().unwrap_or("(无)"),
    );

    let mut existing = if log_path.is_file() {
        fs::read_to_string(&log_path).unwrap_or_default()
    } else {
        "# 迭代日志\n\n记录 Domain Pack 的变更历史。\n".to_string()
    };

    existing.push_str(&entry);
    fs::write(&log_path, existing).map_err(|e| format!("写入迭代日志失败: {e}"))?;

    Ok(())
}
