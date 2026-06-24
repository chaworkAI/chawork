use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptAttachment {
    pub kind: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub created_at: String,
    pub last_message_at: String,
    pub message_count: u32,
    pub status: String,
    /// When true, auto title sync from transcript will not overwrite `title`.
    #[serde(default)]
    pub title_locked: bool,
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn sessions_dir(workspace_path: &Path) -> std::path::PathBuf {
    workspace_path.join("sessions")
}

fn session_root(workspace_path: &Path, session_id: &str) -> std::path::PathBuf {
    sessions_dir(workspace_path).join(session_id)
}

pub fn session_attachment_dir(
    workspace_path: &Path,
    session_id: &str,
    message_id: &str,
) -> PathBuf {
    session_root(workspace_path, session_id)
        .join("attachments")
        .join(safe_path_segment(message_id))
}

fn meta_file(workspace_path: &Path, session_id: &str) -> std::path::PathBuf {
    session_root(workspace_path, session_id).join("meta.json")
}

pub fn session_exists(workspace_path: &Path, session_id: &str) -> bool {
    meta_file(workspace_path, session_id).exists()
}

pub fn transcript_path(workspace_path: &Path, session_id: &str) -> std::path::PathBuf {
    session_root(workspace_path, session_id).join("transcript.jsonl")
}

pub fn runtime_path(workspace_path: &Path, session_id: &str) -> std::path::PathBuf {
    session_root(workspace_path, session_id).join("runtime.jsonl")
}

/// Last persisted Codex `thread_id` for this session (JSONL lines with `codex_thread_id`).
pub fn load_runtime_thread_id(
    workspace_path: &Path,
    session_id: &str,
) -> Result<Option<String>, String> {
    let p = runtime_path(workspace_path, session_id);
    if !p.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let mut last: Option<String> = None;
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(t).map_err(|e| e.to_string())?;
        if let Some(s) = v.get("codex_thread_id").and_then(|x| x.as_str()) {
            if !s.is_empty() {
                last = Some(s.to_string());
            }
        }
    }
    Ok(last)
}

/// Append one JSONL record with the current Codex `thread_id`.
pub fn append_runtime_thread_id(
    workspace_path: &Path,
    session_id: &str,
    thread_id: &str,
) -> Result<(), String> {
    let path = runtime_path(workspace_path, session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let line = serde_json::json!({ "codex_thread_id": thread_id });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let s = serde_json::to_string(&line).map_err(|e| e.to_string())?;
    file.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(())
}

fn read_meta(workspace_path: &Path, session_id: &str) -> Result<SessionMeta, String> {
    let p = meta_file(workspace_path, session_id);
    let mut buf = String::new();
    fs::File::open(&p)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&buf).map_err(|e| e.to_string())
}

fn write_meta(workspace_path: &Path, meta: &SessionMeta) -> Result<(), String> {
    let dir = session_root(workspace_path, &meta.id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = meta_file(workspace_path, &meta.id);
    let json = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(&p, json.as_bytes()).map_err(|e| e.to_string())
}

/// Create session directory tree, meta.json, and empty jsonl transcript/runtime files.
pub fn create(workspace_path: &Path, workspace_id: &str) -> Result<SessionMeta, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = iso_now();
    let root = session_root(workspace_path, &session_id);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    let meta = SessionMeta {
        id: session_id.clone(),
        workspace_id: workspace_id.to_string(),
        title: "新会话".to_string(),
        created_at: now.clone(),
        last_message_at: now,
        message_count: 0,
        status: "active".to_string(),
        title_locked: false,
    };

    write_meta(workspace_path, &meta)?;

    for path in [
        &transcript_path(workspace_path, &session_id),
        &runtime_path(workspace_path, &session_id),
    ] {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.as_path())
            .map_err(|e| e.to_string())?;
    }

    Ok(meta)
}

/// List sessions by scanning `sessions/<id>/meta.json`.
pub fn list(workspace_path: &Path) -> Result<Vec<SessionMeta>, String> {
    let dir = sessions_dir(workspace_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if meta_file(workspace_path, &id).exists() {
            if let Ok(meta) = read_meta(workspace_path, &id) {
                out.push(meta);
            }
        }
    }

    out.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));
    Ok(out)
}

pub fn append_transcript(
    workspace_path: &Path,
    session_id: &str,
    entry: &serde_json::Value,
) -> Result<(), String> {
    let path = transcript_path(workspace_path, session_id);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;

    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    file.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;

    Ok(())
}

fn safe_path_segment(raw: &str) -> String {
    let segment: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if segment.trim_matches('_').is_empty() {
        Uuid::new_v4().to_string()
    } else {
        segment
    }
}

fn image_mime_from_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn image_extension_from_mime(mime: &str) -> Option<&'static str> {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

fn write_session_image_attachment_bytes(
    workspace_path: &Path,
    session_id: &str,
    message_id: &str,
    name: Option<&str>,
    ext: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<TranscriptAttachment, String> {
    if !session_exists(workspace_path, session_id) {
        return Err("会话不存在".to_string());
    }
    if bytes.is_empty() {
        return Err("图片附件不能为空".to_string());
    }

    let target_dir = session_attachment_dir(workspace_path, session_id, message_id);
    fs::create_dir_all(&target_dir).map_err(|e| format!("创建图片附件目录失败: {e}"))?;
    let stem = name
        .and_then(|raw| {
            Path::new(raw)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(safe_path_segment)
        })
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "pasted-image".to_string());
    let filename = format!("{stem}-{}.{}", Uuid::new_v4(), ext);
    let target = target_dir.join(filename);
    fs::write(&target, bytes).map_err(|e| format!("写入图片附件失败: {e}"))?;

    Ok(TranscriptAttachment {
        kind: "image".to_string(),
        path: target.to_string_lossy().into_owned(),
        mime_type: Some(mime_type.to_string()),
    })
}

pub fn copy_session_image_attachment(
    workspace_path: &Path,
    session_id: &str,
    message_id: &str,
    source_path: &Path,
) -> Result<TranscriptAttachment, String> {
    if !session_exists(workspace_path, session_id) {
        return Err("会话不存在".to_string());
    }
    if !source_path.is_absolute() {
        return Err("图片附件路径必须是绝对路径".to_string());
    }
    if !source_path.is_file() {
        return Err(format!("图片附件不存在: {}", source_path.display()));
    }
    let ext = source_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let Some(mime_type) = image_mime_from_extension(&ext) else {
        return Err("仅支持图片附件：png、jpg、jpeg、webp、gif".to_string());
    };

    let bytes = fs::read(source_path).map_err(|e| format!("读取图片附件失败: {e}"))?;
    write_session_image_attachment_bytes(
        workspace_path,
        session_id,
        message_id,
        source_path.file_name().and_then(|s| s.to_str()),
        &ext,
        mime_type,
        &bytes,
    )
}

pub fn write_session_image_attachment_from_data_url(
    workspace_path: &Path,
    session_id: &str,
    message_id: &str,
    name: Option<&str>,
    data_url: &str,
) -> Result<TranscriptAttachment, String> {
    let trimmed = data_url.trim();
    let Some(rest) = trimmed.strip_prefix("data:") else {
        return Err("图片附件 dataUrl 格式无效".to_string());
    };
    let Some((metadata, payload)) = rest.split_once(',') else {
        return Err("图片附件 dataUrl 缺少 base64 数据".to_string());
    };
    let mut parts = metadata.split(';');
    let mime_type = parts.next().unwrap_or_default();
    let is_base64 = parts.any(|part| part.eq_ignore_ascii_case("base64"));
    if !is_base64 {
        return Err("图片附件 dataUrl 必须使用 base64 编码".to_string());
    }
    let Some(ext) = image_extension_from_mime(mime_type) else {
        return Err("仅支持图片附件：png、jpg、jpeg、webp、gif".to_string());
    };
    let bytes = BASE64_STANDARD
        .decode(payload.trim())
        .map_err(|e| format!("图片附件 base64 解码失败: {e}"))?;
    write_session_image_attachment_bytes(
        workspace_path,
        session_id,
        message_id,
        name,
        ext,
        mime_type,
        &bytes,
    )
}

/// If the last non-empty JSONL line matches `expected` on `role` / `content` / `timestamp`, remove it.
/// Used when a user turn was cancelled or did not complete so the transcript should not retain that user line.
pub fn pop_last_transcript_if_matches(
    workspace_path: &Path,
    session_id: &str,
    expected: &serde_json::Value,
) -> Result<bool, String> {
    let path = transcript_path(workspace_path, session_id);
    if !path.is_file() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    if lines.is_empty() {
        return Ok(false);
    }
    let Some(last_line) = lines.last() else {
        return Ok(false);
    };
    let last: serde_json::Value = serde_json::from_str(last_line).map_err(|e| e.to_string())?;
    if !transcript_entries_match_for_rollback(&last, expected) {
        return Ok(false);
    }
    lines.pop();
    let new_body = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };
    fs::write(&path, new_body.as_bytes()).map_err(|e| e.to_string())?;
    Ok(true)
}

fn transcript_entries_match_for_rollback(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    a.get("role") == b.get("role")
        && a.get("content") == b.get("content")
        && a.get("timestamp") == b.get("timestamp")
}

/// Recompute `message_count`, `last_message_at`, and `title` from `transcript.jsonl` (after rollback).
pub fn sync_meta_from_transcript(workspace_path: &Path, session_id: &str) -> Result<(), String> {
    let mut meta = read_meta(workspace_path, session_id)?;
    let entries = read_transcript(workspace_path, session_id)?;
    meta.message_count = entries.len() as u32;
    meta.last_message_at = entries
        .last()
        .and_then(|e| e.get("timestamp").and_then(|t| t.as_str()))
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| meta.created_at.clone());
    let title = entries
        .iter()
        .find_map(|e| {
            if e.get("role").and_then(|r| r.as_str()) == Some("user") {
                e.get("content")
                    .and_then(|c| c.as_str())
                    .map(|t| truncate_title(t, 20))
            } else {
                None
            }
        })
        .unwrap_or_else(|| "新会话".to_string());
    if !meta.title_locked {
        meta.title = title;
    }
    write_meta(workspace_path, &meta)?;
    Ok(())
}

const MAX_SESSION_TITLE_CHARS: usize = 80;

/// Rename a session; locks title so transcript sync will not overwrite it.
pub fn rename_session(
    workspace_path: &Path,
    session_id: &str,
    title: &str,
) -> Result<SessionMeta, String> {
    if !session_exists(workspace_path, session_id) {
        return Err("会话不存在".to_string());
    }
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("会话名称不能为空".to_string());
    }
    let mut meta = read_meta(workspace_path, session_id)?;
    meta.title = trimmed.chars().take(MAX_SESSION_TITLE_CHARS).collect();
    meta.title_locked = true;
    write_meta(workspace_path, &meta)?;
    Ok(meta)
}

/// Delete session directory and all transcript/runtime data.
pub fn delete_session(workspace_path: &Path, session_id: &str) -> Result<(), String> {
    if !session_exists(workspace_path, session_id) {
        return Err("会话不存在".to_string());
    }
    let root = session_root(workspace_path, session_id);
    fs::remove_dir_all(&root).map_err(|e| format!("删除会话失败: {e}"))?;
    Ok(())
}

pub fn read_transcript(
    workspace_path: &Path,
    session_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let path = transcript_path(workspace_path, session_id);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let f = fs::File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(f);
    let mut lines = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        lines.push(serde_json::from_str(&line).map_err(|e| e.to_string())?);
    }

    Ok(lines)
}

pub fn truncate_title(content: &str, max_chars: usize) -> String {
    let t = content.trim();
    if t.is_empty() {
        return "新会话".to_string();
    }
    t.chars().take(max_chars).collect()
}

fn is_image_only_user_message(body: &serde_json::Value) -> bool {
    body.get("attachments")
        .and_then(|v| v.as_array())
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("kind").and_then(|v| v.as_str()) == Some("image"))
        })
}

pub fn persist_meta_after_user_message(
    workspace_path: &Path,
    session_id: &str,
    body: &serde_json::Value,
) -> Result<(), String> {
    let mut meta = read_meta(workspace_path, session_id)?;
    meta.message_count += 1;
    let now = iso_now();
    meta.last_message_at = now;
    if meta.title == "新会话" {
        if let Some(txt) = body.get("content").and_then(|v| v.as_str()) {
            let title = truncate_title(txt, 20);
            meta.title = if title == "新会话" && is_image_only_user_message(body) {
                "图片消息".to_string()
            } else {
                title
            };
        }
    }
    write_meta(workspace_path, &meta)?;
    Ok(())
}

pub fn persist_meta_after_assistant_message(
    workspace_path: &Path,
    session_id: &str,
) -> Result<(), String> {
    let mut meta = read_meta(workspace_path, session_id)?;
    meta.message_count += 1;
    meta.last_message_at = iso_now();
    write_meta(workspace_path, &meta)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_image_attachment_lands_inside_session_assets() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let session = create(&workspace, "workspace-a").expect("session");
        let source = tmp.path().join("source image.PNG");
        fs::write(&source, b"png bytes").expect("source image");

        let attachment =
            copy_session_image_attachment(&workspace, &session.id, "message-1", &source)
                .expect("copy attachment");

        let copied = Path::new(&attachment.path);
        assert!(copied.is_absolute());
        assert!(copied.is_file());
        assert!(copied.starts_with(session_root(&workspace, &session.id)));
        assert!(copied
            .parent()
            .is_some_and(|parent| parent.ends_with("attachments/message-1")));
        assert_eq!(attachment.kind, "image");
        assert_eq!(attachment.mime_type.as_deref(), Some("image/png"));
        assert_eq!(fs::read(copied).expect("copied bytes"), b"png bytes");
    }

    #[test]
    fn copy_image_attachment_rejects_non_image_extension() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let session = create(&workspace, "workspace-a").expect("session");
        let source = tmp.path().join("notes.txt");
        fs::write(&source, b"text").expect("source file");

        let err = copy_session_image_attachment(&workspace, &session.id, "message-1", &source)
            .expect_err("non-image attachment must be rejected");

        assert!(err.contains("仅支持图片附件"));
    }

    #[test]
    fn data_url_image_attachment_lands_inside_session_assets() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let session = create(&workspace, "workspace-a").expect("session");
        let data_url = "data:image/png;base64,aGVsbG8=";

        let attachment = write_session_image_attachment_from_data_url(
            &workspace,
            &session.id,
            "message-1",
            Some("clipboard.png"),
            data_url,
        )
        .expect("write pasted attachment");

        let copied = Path::new(&attachment.path);
        assert!(copied.is_absolute());
        assert!(copied.is_file());
        assert!(copied.starts_with(session_root(&workspace, &session.id)));
        assert_eq!(attachment.kind, "image");
        assert_eq!(attachment.mime_type.as_deref(), Some("image/png"));
        assert_eq!(fs::read(copied).expect("copied bytes"), b"hello");
    }

    #[test]
    fn image_only_message_gets_stable_session_title() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let session = create(&workspace, "workspace-a").expect("session");
        let body = serde_json::json!({
            "role": "user",
            "content": "",
            "timestamp": iso_now(),
            "attachments": [{ "kind": "image", "path": "/tmp/a.png", "mime_type": "image/png" }],
        });

        persist_meta_after_user_message(&workspace, &session.id, &body).expect("persist meta");
        let meta = read_meta(&workspace, &session.id).expect("read meta");

        assert_eq!(meta.title, "图片消息");
    }
}
