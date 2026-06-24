//! Proposal Service — manages proposals (drafts, accepted, rejected) in the workspace.
//!
//! Proposals are JSON files stored in `proposals/{drafts,accepted,rejected}/`.
//! Each proposal represents an AI-generated suggestion (schema update, wiki update,
//! skill change, report draft) that requires user review.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalType {
    SchemaUpdate,
    WikiUpdate,
    SkillUpdate,
    TemplateUpdate,
    ReportDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Draft,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    #[serde(default)]
    pub source_session: Option<String>,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default)]
    pub target_path: Option<String>,
    /// New content to write when applying (for non-diff proposals).
    #[serde(default)]
    pub new_content: Option<String>,
    pub created_at: String,
    pub status: ProposalStatus,
    #[serde(default)]
    pub resolved_at: Option<String>,
    /// Optional risk level for UI display.
    #[serde(default)]
    pub risk: Option<String>,
}

/// Create a new proposal in `proposals/drafts/`.
pub fn create_proposal(
    workspace_path: &Path,
    title: &str,
    description: &str,
    proposal_type: ProposalType,
    target_path: Option<&str>,
    diff: Option<&str>,
    new_content: Option<&str>,
    source_session: Option<&str>,
    risk: Option<&str>,
) -> Result<Proposal, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let proposal = Proposal {
        id: id.clone(),
        title: title.to_string(),
        description: description.to_string(),
        proposal_type,
        source_session: source_session.map(String::from),
        diff: diff.map(String::from),
        target_path: target_path.map(String::from),
        new_content: new_content.map(String::from),
        created_at: now,
        status: ProposalStatus::Draft,
        resolved_at: None,
        risk: risk.map(String::from),
    };

    let drafts_dir = workspace_path.join("proposals").join("drafts");
    fs::create_dir_all(&drafts_dir).map_err(|e| format!("创建 proposals/drafts 目录失败: {e}"))?;

    let filepath = drafts_dir.join(format!("{id}.json"));
    let json = serde_json::to_string_pretty(&proposal)
        .map_err(|e| format!("序列化 proposal 失败: {e}"))?;
    fs::write(&filepath, json).map_err(|e| format!("写入 proposal 失败: {e}"))?;

    Ok(proposal)
}

/// List proposals, optionally filtered by status.
pub fn list_proposals(
    workspace_path: &Path,
    status_filter: Option<ProposalStatus>,
) -> Result<Vec<Proposal>, String> {
    let proposals_root = workspace_path.join("proposals");
    let subdirs = match status_filter {
        Some(ProposalStatus::Draft) => vec!["drafts"],
        Some(ProposalStatus::Accepted) => vec!["accepted"],
        Some(ProposalStatus::Rejected) => vec!["rejected"],
        None => vec!["drafts", "accepted", "rejected"],
    };

    let mut result = Vec::new();
    for sub in subdirs {
        let dir = proposals_root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(p) = serde_json::from_str::<Proposal>(&content) {
                        result.push(p);
                    }
                }
            }
        }
    }

    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(result)
}

/// Count JSON files in `proposals/drafts/` (pending user review).
pub fn count_draft_proposals(workspace_path: &Path) -> u32 {
    let dir = workspace_path.join("proposals").join("drafts");
    if !dir.is_dir() {
        return 0;
    }
    fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                .count() as u32
        })
        .unwrap_or(0)
}

/// Get a single proposal by ID.
pub fn get_proposal(workspace_path: &Path, id: &str) -> Result<Proposal, String> {
    let proposals_root = workspace_path.join("proposals");
    for sub in ["drafts", "accepted", "rejected"] {
        let path = proposals_root.join(sub).join(format!("{id}.json"));
        if path.is_file() {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("读取 proposal 失败: {e}"))?;
            return serde_json::from_str(&content).map_err(|e| format!("解析 proposal 失败: {e}"));
        }
    }
    Err(format!("Proposal {id} 未找到"))
}

/// Apply (accept) a proposal: move from drafts → accepted and execute the change.
pub fn apply_proposal(
    workspace_path: &Path,
    root: &crate::services::root_workspace::RootWorkspace,
    id: &str,
) -> Result<Proposal, String> {
    let mut proposal = get_proposal(workspace_path, id)?;
    if proposal.status != ProposalStatus::Draft {
        return Err(format!("Proposal {id} 不在 draft 状态，无法 apply"));
    }

    // Delegate actual file changes + Codex refresh to iteration service
    crate::services::iteration::apply_iteration(workspace_path, root, &proposal)?;

    proposal.status = ProposalStatus::Accepted;
    proposal.resolved_at = Some(Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));

    move_proposal(workspace_path, &proposal, "drafts", "accepted")?;
    Ok(proposal)
}

/// Reject a proposal: move from drafts → rejected.
pub fn reject_proposal(workspace_path: &Path, id: &str) -> Result<Proposal, String> {
    let mut proposal = get_proposal(workspace_path, id)?;
    if proposal.status != ProposalStatus::Draft {
        return Err(format!("Proposal {id} 不在 draft 状态，无法 reject"));
    }

    proposal.status = ProposalStatus::Rejected;
    proposal.resolved_at = Some(Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));

    move_proposal(workspace_path, &proposal, "drafts", "rejected")?;
    Ok(proposal)
}

fn move_proposal(
    workspace_path: &Path,
    proposal: &Proposal,
    from: &str,
    to: &str,
) -> Result<(), String> {
    let proposals_root = workspace_path.join("proposals");
    let filename = format!("{}.json", proposal.id);
    let src = proposals_root.join(from).join(&filename);
    let dst_dir = proposals_root.join(to);

    fs::create_dir_all(&dst_dir).map_err(|e| format!("创建 proposals/{to} 目录失败: {e}"))?;

    let dst = dst_dir.join(&filename);

    let json =
        serde_json::to_string_pretty(proposal).map_err(|e| format!("序列化 proposal 失败: {e}"))?;
    fs::write(&dst, json).map_err(|e| format!("写入 proposal 失败: {e}"))?;

    if src.is_file() {
        let _ = fs::remove_file(&src);
    }

    // Log the action
    log_proposal_action(workspace_path, proposal);

    Ok(())
}

fn log_proposal_action(workspace_path: &Path, proposal: &Proposal) {
    let log_dir = workspace_path.join("logs").join("operations");
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let entry = serde_json::json!({
        "timestamp": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "action": format!("{:?}", proposal.status).to_lowercase(),
        "proposal_id": proposal.id,
        "proposal_type": proposal.proposal_type,
        "title": proposal.title,
        "target_path": proposal.target_path,
    });

    let log_path = log_dir.join("proposals.jsonl");
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{}", entry);
    }
}

/// Helper: find proposal file path by scanning all status directories.
pub fn find_proposal_path(workspace_path: &Path, id: &str) -> Option<PathBuf> {
    let proposals_root = workspace_path.join("proposals");
    for sub in ["drafts", "accepted", "rejected"] {
        let path = proposals_root.join(sub).join(format!("{id}.json"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}
