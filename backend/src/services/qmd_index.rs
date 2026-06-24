//! BM25 knowledge index — thin wrapper around the `xucailiang/qmd` crate
//! (SQLite + FTS5). Stores per-workspace index at `.chawork/qmd/index.sqlite`.
//!
//! Indexed scope per DESIGN §6: `wiki/documents/**/*.md`. Audio/transcripts are
//! deferred to a future release; we only register the `documents` collection.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::path_safety;

const DOCUMENTS_COLLECTION: &str = "documents";

/// Serializes index access per-process. `qmd::Qmd` wraps a SQLite connection
/// (not `Sync`); we own it briefly per call and drop before returning so the
/// frontend can poll status without blocking. A coarse mutex is fine — index
/// operations are workspace-scoped and not on the hot path.
static INDEX_MUTEX: Mutex<()> = Mutex::new(());

fn lock_index() -> std::sync::MutexGuard<'static, ()> {
    INDEX_MUTEX.lock().unwrap_or_else(PoisonError::into_inner)
}

fn index_db_path(workspace_path: &Path) -> PathBuf {
    workspace_path
        .join(".chawork")
        .join("qmd")
        .join("index.sqlite")
}

fn meta_path(workspace_path: &Path) -> PathBuf {
    workspace_path
        .join(".chawork")
        .join("qmd")
        .join("meta.json")
}

fn documents_dir(workspace_path: &Path) -> PathBuf {
    workspace_path.join("wiki").join("documents")
}

/// Stable id for logs / UI (SHA-256 of canonical workspace path, first 16 hex chars).
pub fn workspace_index_name(workspace_path: &Path) -> String {
    let p = workspace_path
        .canonicalize()
        .unwrap_or_else(|_| workspace_path.to_path_buf());
    let mut h = Sha256::new();
    h.update(p.to_string_lossy().as_bytes());
    let full = format!("{:x}", h.finalize());
    full.chars().take(16).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct IndexMetaFile {
    doc_count: u64,
    built_at: String,
    #[serde(default)]
    last_error: Option<String>,
}

fn write_meta(workspace_path: &Path, meta: &IndexMetaFile) -> Result<(), String> {
    let p = meta_path(workspace_path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 .chawork/qmd 失败: {e}"))?;
    }
    let s = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(&p, s).map_err(|e| format!("写入 qmd meta.json 失败: {e}"))
}

fn write_meta_error(workspace_path: &Path, err: &str) {
    let meta = IndexMetaFile {
        doc_count: 0,
        built_at: chrono::Utc::now().to_rfc3339(),
        last_error: Some(err.chars().take(800).collect()),
    };
    let _ = write_meta(workspace_path, &meta);
}

fn open_qmd(workspace_path: &Path) -> Result<qmd::Qmd, String> {
    let db_path = index_db_path(workspace_path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 .chawork/qmd 失败: {e}"))?;
    }
    qmd::Qmd::open(&db_path).map_err(|e| format!("打开 qmd 索引失败: {e}"))
}

/// Ensures the `documents` collection exists in the qmd db and points at
/// `<workspace>/wiki/documents/`. Idempotent.
fn ensure_collection_registered(qm: &qmd::Qmd, workspace_path: &Path) -> Result<(), String> {
    let docs_dir = documents_dir(workspace_path);
    fs::create_dir_all(&docs_dir).map_err(|e| format!("创建 wiki/documents 失败: {e}"))?;
    let coll = qmd::Collection::new(DOCUMENTS_COLLECTION, docs_dir.to_string_lossy().to_string());
    qm.register_collection(&coll)
        .map_err(|e| format!("注册 qmd collection 失败: {e}"))?;
    Ok(())
}

/// Initialize qmd db when missing; idempotent.
pub fn initialize_qmd(workspace_path: &Path) -> Result<String, String> {
    let _g = lock_index();
    let qm = open_qmd(workspace_path)?;
    ensure_collection_registered(&qm, workspace_path)?;
    // Update meta with current doc count without forcing a rescan.
    let doc_count = qm.doc_count().map_err(|e| e.to_string())? as u64;
    let meta = IndexMetaFile {
        doc_count,
        built_at: chrono::Utc::now().to_rfc3339(),
        last_error: None,
    };
    write_meta(workspace_path, &meta)?;
    Ok(format!("qmd index initialized: docs={doc_count}"))
}

/// Full incremental refresh — qmd scans the filesystem, detects new/updated/removed
/// files by content hash, and updates the FTS5 index. Reflects per DESIGN §6.
pub fn refresh_index(workspace_path: &Path) -> Result<String, String> {
    let _g = lock_index();
    let marker = workspace_path.join(".chawork/qmd/building.marker");
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&marker, "1");

    let result = (|| -> Result<String, String> {
        let qm = open_qmd(workspace_path)?;
        ensure_collection_registered(&qm, workspace_path)?;
        let update = qm
            .update(None)
            .map_err(|e| format!("qmd update 失败: {e}"))?;
        let doc_count = qm.doc_count().map_err(|e| e.to_string())? as u64;
        let meta = IndexMetaFile {
            doc_count,
            built_at: chrono::Utc::now().to_rfc3339(),
            last_error: None,
        };
        write_meta(workspace_path, &meta)?;
        Ok(format!(
            "indexed {} new, {} updated, {} unchanged, {} removed across {} collections",
            update.indexed, update.updated, update.unchanged, update.removed, update.collections
        ))
    })();

    let _ = fs::remove_file(&marker);

    if let Err(ref e) = result {
        write_meta_error(workspace_path, e);
    }

    result
}

/// Matches frontend `IndexStatus`: `ready` | `stale` | `building` | `error`.
pub fn infer_index_status_string(workspace_path: &Path) -> String {
    let marker = workspace_path.join(".chawork/qmd/building.marker");
    if marker.is_file() {
        return "building".to_string();
    }
    let mp = meta_path(workspace_path);
    if !mp.is_file() {
        return "stale".to_string();
    }
    let Ok(raw) = fs::read_to_string(&mp) else {
        return "error".to_string();
    };
    let Ok(meta) = serde_json::from_str::<IndexMetaFile>(&raw) else {
        return "error".to_string();
    };
    if meta
        .last_error
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return "error".to_string();
    }
    if meta.doc_count > 0 {
        "ready".to_string()
    } else {
        "stale".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QmdStatus {
    pub index_name: String,
    pub raw_output: String,
    pub is_ready: bool,
    #[serde(default)]
    pub doc_count: u64,
    /// `ready` | `stale` | `building` | `error`
    #[serde(default)]
    pub phase: String,
}

pub fn get_index_status(workspace_path: &Path) -> Result<QmdStatus, String> {
    let name = workspace_index_name(workspace_path);
    let phase = infer_index_status_string(workspace_path);
    let mp = meta_path(workspace_path);
    if !mp.is_file() {
        return Ok(QmdStatus {
            index_name: name,
            raw_output: "尚未构建索引（qmd / SQLite FTS5）".to_string(),
            is_ready: false,
            doc_count: 0,
            phase,
        });
    }
    let raw = fs::read_to_string(&mp).map_err(|e| e.to_string())?;
    let meta: IndexMetaFile = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let ready = meta.doc_count > 0
        && meta
            .last_error
            .as_ref()
            .map_or(true, |s| s.trim().is_empty());
    let summary = format!(
        "qmd (sqlite fts5) | docs={} | built_at={} | err={:?}",
        meta.doc_count, meta.built_at, meta.last_error
    );
    Ok(QmdStatus {
        index_name: name,
        raw_output: summary,
        is_ready: ready,
        doc_count: meta.doc_count,
        phase,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QmdSearchResult {
    pub docid: String,
    pub score: f64,
    pub file: String,
    pub title: String,
    pub snippet: String,
    /// Reserved for future chunk-level extension; always empty in V1.
    #[serde(default)]
    pub start_char: u64,
    #[serde(default)]
    pub end_char: u64,
    #[serde(default)]
    pub breadcrumb: String,
    #[serde(default)]
    pub chunk_index: u64,
}

fn snippet_around_query(body: &str, query: &str, max: usize) -> String {
    let lower = body.to_lowercase();
    let q = query.to_lowercase();
    if let Some(pos) = lower.find(&q) {
        let start = pos.saturating_sub(40);
        let slice = body.chars().skip(start).take(max).collect::<String>();
        return if start > 0 {
            format!("…{slice}")
        } else {
            slice
        };
    }
    body.chars().take(max).collect::<String>()
}

/// BM25 keyword search via qmd / SQLite FTS5.
pub fn search(
    workspace_path: &Path,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<QmdSearchResult>, String> {
    if query.trim().is_empty() {
        return Err("搜索词不能为空".to_string());
    }
    let _g = lock_index();
    let lim = limit.unwrap_or(10).clamp(1, 100);

    let qm = open_qmd(workspace_path)?;
    ensure_collection_registered(&qm, workspace_path)?;
    let hits = qm
        .search(query, lim)
        .map_err(|e| format!("qmd search 失败: {e}"))?;

    let mut out = Vec::with_capacity(hits.len());
    for hit in hits {
        let doc = &hit.doc;
        let file = if doc.collection == DOCUMENTS_COLLECTION {
            format!("wiki/documents/{}", doc.path)
        } else {
            format!("{}/{}", doc.collection, doc.path)
        };
        // For snippet we re-read the file (qmd doesn't expose body via SearchResult).
        // Cheap enough at small N; future enhancement can use qmd's content store.
        let body = fs::read_to_string(workspace_path.join(&file)).unwrap_or_default();
        let snippet = snippet_around_query(&body, query, 220);
        out.push(QmdSearchResult {
            docid: format!("{}/{}", doc.collection, doc.path),
            score: hit.score,
            file,
            title: doc.title.clone(),
            snippet,
            start_char: 0,
            end_char: 0,
            breadcrumb: String::new(),
            chunk_index: 0,
        });
    }
    Ok(out)
}

/// Read full file content by workspace-relative path (validated).
pub fn get_document(workspace_path: &Path, file_path: &str) -> Result<String, String> {
    if file_path.trim().is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let ws = workspace_path
        .canonicalize()
        .map_err(|e| format!("工作区路径无效: {e}"))?;
    let full = path_safety::safe_join_workspace(&ws, file_path)?;
    let full = fs::canonicalize(&full).map_err(|e| format!("路径无效: {e}"))?;
    if !full.starts_with(&ws) {
        return Err("路径不在工作区范围内".to_string());
    }
    if !full.is_file() {
        return Err("不是文件".to_string());
    }
    fs::read_to_string(&full).map_err(|e| e.to_string())
}

pub fn refresh_if_stale(workspace_path: &Path) -> Result<bool, String> {
    let marker = workspace_path.join(".chawork").join("qmd-index-stale");
    if !marker.exists() {
        return Ok(false);
    }
    let _ = fs::remove_file(&marker);
    refresh_index(workspace_path)?;
    Ok(true)
}

/// Removes deprecated index artifacts from older builds so they stop taking
/// disk space and don't confuse status detection. Run on workspace switch.
pub fn cleanup_legacy_artifacts(workspace_path: &Path) {
    let qmd_dir = workspace_path.join(".chawork/qmd");
    for legacy in ["index", "index-v2", "index-v3"] {
        let p = qmd_dir.join(legacy);
        if p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        }
    }
    let chunker = workspace_path.join(".chawork/runtime/chunker.json");
    if chunker.is_file() {
        let _ = fs::remove_file(&chunker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_removes_legacy_index_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork/qmd/index-v2")).unwrap();
        fs::create_dir_all(ws.join(".chawork/qmd/index-v3")).unwrap();
        fs::create_dir_all(ws.join(".chawork/runtime")).unwrap();
        fs::write(ws.join(".chawork/runtime/chunker.json"), b"{}").unwrap();

        cleanup_legacy_artifacts(ws);

        assert!(!ws.join(".chawork/qmd/index-v2").exists());
        assert!(!ws.join(".chawork/qmd/index-v3").exists());
        assert!(!ws.join(".chawork/runtime/chunker.json").exists());
    }

    #[test]
    fn initialize_and_refresh_empty_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("wiki/documents")).unwrap();
        // ensure_directories not needed — initialize_qmd creates what it needs

        let _ = initialize_qmd(ws).expect("init");
        let _ = refresh_index(ws).expect("refresh empty");

        let status = get_index_status(ws).expect("status");
        assert_eq!(status.doc_count, 0);
        // 0 docs → phase reports "stale"
        assert_eq!(status.phase, "stale");
    }

    #[test]
    fn refresh_picks_up_documents_under_wiki_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("wiki/documents")).unwrap();
        fs::write(
            ws.join("wiki/documents/note.md"),
            b"# Hello\n\nworld content for FTS5 search",
        )
        .unwrap();

        refresh_index(ws).expect("refresh");
        let status = get_index_status(ws).expect("status");
        assert!(status.is_ready, "status should be ready after indexing");
        assert!(status.doc_count >= 1);

        let hits = search(ws, "world", Some(5)).expect("search");
        assert!(!hits.is_empty(), "should find at least one hit for 'world'");
        let first = &hits[0];
        assert!(first.file.starts_with("wiki/documents/"));
        assert!(first.snippet.to_lowercase().contains("world"));
    }

    #[test]
    fn lock_index_recovers_from_poison() {
        let _ = std::panic::catch_unwind(|| {
            let _guard = INDEX_MUTEX.lock().unwrap();
            panic!("intentional poison");
        });
        let _guard = lock_index();
    }
}
