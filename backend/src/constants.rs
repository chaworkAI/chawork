//! Stable strings for IPC / UI contract (avoid locale-specific substring checks).

/// Returned by `open_workspace_dialog` when the user closes the picker without choosing a folder.
pub const DIALOG_CANCELLED: &str = "__CHAWORK_DIALOG_CANCELLED__";
