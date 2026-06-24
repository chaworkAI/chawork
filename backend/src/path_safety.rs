//! Workspace-relative path joining with `..` traversal rejected (shared by MCP server and proposal apply).

use std::path::{Component, Path, PathBuf};

/// Join `relative` onto `workspace_root` without allowing escape above `workspace_root`.
pub fn safe_join_workspace(workspace_root: &Path, relative: &str) -> Result<PathBuf, String> {
    let root = workspace_root;
    let mut cur = root.to_path_buf();
    let rel = Path::new(relative);
    for c in rel.components() {
        match c {
            Component::Normal(s) => cur.push(s),
            Component::ParentDir => {
                cur.pop();
                if !cur.starts_with(root) {
                    return Err("路径不在工作区范围内".to_string());
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {
                return Err("路径不在工作区范围内".to_string());
            }
        }
    }
    Ok(cur)
}
