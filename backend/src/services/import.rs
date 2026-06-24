//! Import Service — async, task-tracked Markdown ingestion pipeline
//! (DESIGN §4 / §5 / §6 post-revision).
//!
//! The Tauri command for `import_file` creates a task synchronously and
//! returns its id immediately; the pipeline runs in a background tokio
//! task, advancing through the state machine and persisting progress to
//! `.chawork/imports/{task_id}/manifest.json` + `result.json`. A flat
//! JSONL log at `logs/import/imports.jsonl` keeps a chronological view
//! for diagnostics.
//!
//! State machine (DESIGN §4):
//!
//! ```text
//! queued
//!   -> saving_source
//!   -> converting_to_markdown
//!   -> writing_wiki
//!   -> refreshing_index
//!   -> completed
//! ```
//!
//! Failure terminals (no rollback of work already done):
//!
//! ```text
//! failed_save                  // raw copy failed; nothing else attempted
//! failed_convert               // raw saved but parse_file failed
//! failed_write                 // parsed but wiki write failed
//! completed_with_index_error   // wiki written but qmd refresh failed
//! cancelled                    // explicit cancel (V1: not yet exposed)
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::services::parser::{self, SourceType};
use crate::services::qmd_index;
use crate::services::wiki_generator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportTaskStatus {
    Queued,
    SavingSource,
    ConvertingToMarkdown,
    WritingWiki,
    RefreshingIndex,
    Completed,
    FailedSave,
    FailedConvert,
    FailedWrite,
    CompletedWithIndexError,
    Cancelled,
}

impl ImportTaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::FailedSave
                | Self::FailedConvert
                | Self::FailedWrite
                | Self::CompletedWithIndexError
                | Self::Cancelled
        )
    }

    pub fn is_success(self) -> bool {
        matches!(self, Self::Completed | Self::CompletedWithIndexError)
    }
}

/// Immutable inputs captured at task creation. Written once, never updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportTaskManifest {
    pub id: String,
    pub source_path: String,
    pub source_filename: String,
    pub source_type: SourceType,
    /// Short SHA-256 (16 hex chars) of the source file content; `None` when
    /// hashing failed (e.g., file vanished before we could read it).
    pub source_hash: Option<String>,
    pub created_at: String,
}

/// Mutable result that gets rewritten as the pipeline progresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportTaskResult {
    pub status: ImportTaskStatus,
    pub raw_path: Option<String>,
    pub wiki_path: Option<String>,
    pub parser: Option<String>,
    pub error: Option<String>,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

/// Combined view exposed to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportTask {
    #[serde(flatten)]
    pub manifest: ImportTaskManifest,
    #[serde(flatten)]
    pub result: ImportTaskResult,
}

// Legacy types retained while the frontend transitions to the task-based API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRecord {
    pub timestamp: String,
    pub source_filename: String,
    pub source_type: SourceType,
    pub raw_path: String,
    pub wiki_path: Option<String>,
    pub success: bool,
}

// ─── Persistence helpers ────────────────────────────────────────────────────

fn imports_dir(workspace_path: &Path) -> PathBuf {
    workspace_path.join(".chawork").join("imports")
}

fn task_dir(workspace_path: &Path, task_id: &str) -> PathBuf {
    imports_dir(workspace_path).join(task_id)
}

fn manifest_path(workspace_path: &Path, task_id: &str) -> PathBuf {
    task_dir(workspace_path, task_id).join("manifest.json")
}

fn result_path(workspace_path: &Path, task_id: &str) -> PathBuf {
    task_dir(workspace_path, task_id).join("result.json")
}

fn write_manifest(workspace_path: &Path, m: &ImportTaskManifest) -> Result<(), String> {
    let dir = task_dir(workspace_path, &m.id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 imports/{}: {e}", m.id))?;
    let p = manifest_path(workspace_path, &m.id);
    let json = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| format!("写入 manifest.json 失败: {e}"))
}

fn write_result(workspace_path: &Path, task_id: &str, r: &ImportTaskResult) -> Result<(), String> {
    let dir = task_dir(workspace_path, task_id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 imports/{task_id}: {e}"))?;
    let p = result_path(workspace_path, task_id);
    let json = serde_json::to_string_pretty(r).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| format!("写入 result.json 失败: {e}"))
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn short_hash(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    let full = format!("{:x}", h.finalize());
    Some(full.chars().take(16).collect())
}

// ─── Task creation + lookup ────────────────────────────────────────────────

/// Create a new import task and persist its manifest. Returns immediately with
/// the assigned task_id. The result file is initialized in `queued` state.
pub fn create_task(workspace_path: &Path, source_path: &Path) -> Result<ImportTask, String> {
    let filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let source_type = SourceType::from_extension(ext);

    let id = uuid::Uuid::new_v4().to_string();
    let manifest = ImportTaskManifest {
        id: id.clone(),
        source_path: source_path.to_string_lossy().into_owned(),
        source_filename: filename,
        source_type,
        source_hash: short_hash(source_path),
        created_at: iso_now(),
    };
    write_manifest(workspace_path, &manifest)?;

    let result = ImportTaskResult {
        status: ImportTaskStatus::Queued,
        raw_path: None,
        wiki_path: None,
        parser: None,
        error: None,
        updated_at: iso_now(),
        completed_at: None,
    };
    write_result(workspace_path, &id, &result)?;

    Ok(ImportTask { manifest, result })
}

/// Load a single task by id.
pub fn get_task(workspace_path: &Path, task_id: &str) -> Result<ImportTask, String> {
    let m_path = manifest_path(workspace_path, task_id);
    let r_path = result_path(workspace_path, task_id);
    let m_raw = fs::read_to_string(&m_path).map_err(|e| format!("manifest 不存在或不可读: {e}"))?;
    let r_raw = fs::read_to_string(&r_path).map_err(|e| format!("result 不存在或不可读: {e}"))?;
    let manifest: ImportTaskManifest =
        serde_json::from_str(&m_raw).map_err(|e| format!("解析 manifest 失败: {e}"))?;
    let result: ImportTaskResult =
        serde_json::from_str(&r_raw).map_err(|e| format!("解析 result 失败: {e}"))?;
    Ok(ImportTask { manifest, result })
}

/// List tasks sorted by `created_at` descending. `limit` caps the result count.
pub fn list_tasks(workspace_path: &Path, limit: usize) -> Vec<ImportTask> {
    let dir = imports_dir(workspace_path);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut tasks: Vec<ImportTask> = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return tasks;
    };
    for entry in entries.flatten() {
        let task_id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(t) = get_task(workspace_path, &task_id) {
            tasks.push(t);
        }
    }
    tasks.sort_by(|a, b| b.manifest.created_at.cmp(&a.manifest.created_at));
    tasks.truncate(limit);
    tasks
}

// ─── Pipeline ──────────────────────────────────────────────────────────────

fn update_result<F>(workspace_path: &Path, task_id: &str, f: F) -> Result<ImportTaskResult, String>
where
    F: FnOnce(&mut ImportTaskResult),
{
    let r_path = result_path(workspace_path, task_id);
    let raw = fs::read_to_string(&r_path).map_err(|e| e.to_string())?;
    let mut result: ImportTaskResult =
        serde_json::from_str(&raw).map_err(|e| format!("解析 result.json 失败: {e}"))?;
    f(&mut result);
    result.updated_at = iso_now();
    if result.status.is_terminal() && result.completed_at.is_none() {
        result.completed_at = Some(result.updated_at.clone());
    }
    write_result(workspace_path, task_id, &result)?;
    Ok(result)
}

fn mark_failed(
    workspace_path: &Path,
    task_id: &str,
    status: ImportTaskStatus,
    err: String,
) -> ImportTaskResult {
    update_result(workspace_path, task_id, |r| {
        r.status = status;
        r.error = Some(err);
    })
    .unwrap_or_else(|_| ImportTaskResult {
        status,
        raw_path: None,
        wiki_path: None,
        parser: None,
        error: None,
        updated_at: iso_now(),
        completed_at: Some(iso_now()),
    })
}

/// Synchronous full pipeline: copy → parse → write wiki → refresh index.
/// Persists state at each transition. Designed to be called inside
/// `spawn_blocking` from the Tauri command layer.
///
/// On unsupported source type, marks the task `failed_convert` immediately
/// (it never reaches the parser stage).
pub fn run_pipeline(workspace_path: &Path, task_id: &str) -> Result<ImportTask, String> {
    let manifest_raw = fs::read_to_string(manifest_path(workspace_path, task_id))
        .map_err(|e| format!("manifest 读取失败: {e}"))?;
    let manifest: ImportTaskManifest =
        serde_json::from_str(&manifest_raw).map_err(|e| format!("manifest 解析失败: {e}"))?;
    let source_path = PathBuf::from(&manifest.source_path);
    let source_type = manifest.source_type;
    let filename = manifest.source_filename.clone();

    // Reject unsupported types — design says the frontend filters them out,
    // but defense-in-depth: never let raw garbage land in raw/uploads/.
    if !source_type.is_supported() {
        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let result = mark_failed(
            workspace_path,
            task_id,
            ImportTaskStatus::FailedConvert,
            format!("不支持的文件类型 '.{ext}'。仅接受 pdf/docx/txt/md/xlsx/csv"),
        );
        append_log(workspace_path, &manifest, &result);
        return Ok(ImportTask { manifest, result });
    }

    // 1. saving_source
    let _ = update_result(workspace_path, task_id, |r| {
        r.status = ImportTaskStatus::SavingSource;
    });
    let raw_path = match copy_to_raw(workspace_path, &source_path, source_type) {
        Ok(p) => p,
        Err(e) => {
            let result = mark_failed(
                workspace_path,
                task_id,
                ImportTaskStatus::FailedSave,
                format!("保存 raw 失败: {e}"),
            );
            append_log(workspace_path, &manifest, &result);
            return Ok(ImportTask { manifest, result });
        }
    };
    let raw_rel = raw_path
        .strip_prefix(workspace_path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| raw_path.display().to_string());
    let _ = update_result(workspace_path, task_id, |r| {
        r.raw_path = Some(raw_rel.clone());
    });

    // 2. converting_to_markdown
    let _ = update_result(workspace_path, task_id, |r| {
        r.status = ImportTaskStatus::ConvertingToMarkdown;
    });
    let parse_result = match parser::parse_file(&raw_path) {
        Ok(r) => r,
        Err(e) => {
            let result = mark_failed(
                workspace_path,
                task_id,
                ImportTaskStatus::FailedConvert,
                format!("解析失败: {e}"),
            );
            append_log(workspace_path, &manifest, &result);
            return Ok(ImportTask { manifest, result });
        }
    };
    let _ = update_result(workspace_path, task_id, |r| {
        r.parser = Some(parse_result.parser.clone());
    });

    // 3. writing_wiki
    let _ = update_result(workspace_path, task_id, |r| {
        r.status = ImportTaskStatus::WritingWiki;
    });
    let title = derive_title(&filename, &parse_result.text);
    let wiki_page = match wiki_generator::generate_wiki_page(
        workspace_path,
        &title,
        &parse_result.text,
        source_type,
        &filename,
        &parse_result.parser,
        &raw_rel,
    ) {
        Ok(wp) => wp,
        Err(e) => {
            let result = mark_failed(
                workspace_path,
                task_id,
                ImportTaskStatus::FailedWrite,
                format!("写入 wiki 失败: {e}"),
            );
            append_log(workspace_path, &manifest, &result);
            return Ok(ImportTask { manifest, result });
        }
    };
    let _ = update_result(workspace_path, task_id, |r| {
        r.wiki_path = Some(wiki_page.path.clone());
    });

    // 4. refreshing_index
    let _ = update_result(workspace_path, task_id, |r| {
        r.status = ImportTaskStatus::RefreshingIndex;
    });
    let index_result = qmd_index::refresh_index(workspace_path);

    let final_status = match &index_result {
        Ok(_) => ImportTaskStatus::Completed,
        Err(e) => {
            eprintln!("[import] qmd refresh failed for task {task_id}: {e}");
            ImportTaskStatus::CompletedWithIndexError
        }
    };
    let final_error = match index_result {
        Ok(_) => None,
        Err(e) => Some(format!("qmd refresh 失败: {e}")),
    };

    let result = update_result(workspace_path, task_id, |r| {
        r.status = final_status;
        if r.error.is_none() {
            r.error = final_error;
        }
    })
    .unwrap_or_else(|_| ImportTaskResult {
        status: final_status,
        raw_path: Some(raw_rel.clone()),
        wiki_path: Some(wiki_page.path.clone()),
        parser: Some(parse_result.parser.clone()),
        error: None,
        updated_at: iso_now(),
        completed_at: Some(iso_now()),
    });

    append_log(workspace_path, &manifest, &result);
    Ok(ImportTask { manifest, result })
}

// ─── Sync helpers retained for legacy / tests ──────────────────────────────

/// One-shot synchronous import — creates task and runs pipeline in caller's
/// thread. Returned value is the final ImportTask state.
pub fn import_file(workspace_path: &Path, source_path: &Path) -> Result<ImportTask, String> {
    let task = create_task(workspace_path, source_path)?;
    run_pipeline(workspace_path, &task.manifest.id)
}

/// Legacy log feed (kept until frontend switches to task list view).
pub fn list_imports(workspace_path: &Path, limit: usize) -> Vec<ImportRecord> {
    let log_path = workspace_path
        .join("logs")
        .join("import")
        .join("imports.jsonl");
    if !log_path.is_file() {
        return Vec::new();
    }
    let Ok(content) = fs::read_to_string(&log_path) else {
        return Vec::new();
    };
    let mut records: Vec<ImportRecord> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    records.reverse();
    records.truncate(limit);
    records
}

fn copy_to_raw(
    workspace_path: &Path,
    source_path: &Path,
    source_type: SourceType,
) -> Result<PathBuf, String> {
    let subdir = source_type.raw_subdirectory();
    let raw_dir = workspace_path.join("raw").join(subdir);
    fs::create_dir_all(&raw_dir).map_err(|e| format!("创建 raw/{subdir} 目录失败: {e}"))?;

    let filename = source_path
        .file_name()
        .ok_or_else(|| "无效文件名".to_string())?;
    let dest = raw_dir.join(filename);

    if let Ok(canon_src) = source_path.canonicalize() {
        if let Ok(canon_dest) = dest.canonicalize() {
            if canon_src == canon_dest {
                return Ok(dest);
            }
        }
    }

    fs::copy(source_path, &dest).map_err(|e| format!("复制文件到 raw/ 失败: {e}"))?;
    Ok(dest)
}

fn derive_title(filename: &str, _text: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未命名");
    stem.trim().to_string()
}

fn append_log(workspace_path: &Path, manifest: &ImportTaskManifest, result: &ImportTaskResult) {
    let log_dir = workspace_path.join("logs").join("import");
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }
    let record = ImportRecord {
        timestamp: iso_now(),
        source_filename: manifest.source_filename.clone(),
        source_type: manifest.source_type,
        raw_path: result.raw_path.clone().unwrap_or_default(),
        wiki_path: result.wiki_path.clone(),
        success: result.status.is_success(),
    };
    let Ok(line) = serde_json::to_string(&record) else {
        return;
    };
    let log_path = log_dir.join("imports.jsonl");
    let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    else {
        return;
    };
    let _ = writeln!(f, "{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_text_pdf_bytes() -> &'static [u8] {
        b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>
endobj
4 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
endobj
5 0 obj
<< /Length 47 >>
stream
BT /F1 24 Tf 100 700 Td (Hello PDF text) Tj ET
endstream
endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000241 00000 n 
0000000311 00000 n 
trailer
<< /Root 1 0 R /Size 6 >>
startxref
407
%%EOF
"
    }

    fn workspace_with_dirs() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("wiki")).unwrap();
        tmp
    }

    #[test]
    fn create_task_persists_manifest_and_initial_result() {
        let tmp = workspace_with_dirs();
        let src = tmp.path().join("note.txt");
        fs::write(&src, "hello").unwrap();

        let task = create_task(tmp.path(), &src).unwrap();
        assert!(!task.manifest.id.is_empty());
        assert_eq!(task.manifest.source_filename, "note.txt");
        assert_eq!(task.manifest.source_type, SourceType::Text);
        assert!(task.manifest.source_hash.is_some());
        assert_eq!(task.result.status, ImportTaskStatus::Queued);

        let loaded = get_task(tmp.path(), &task.manifest.id).unwrap();
        assert_eq!(loaded.manifest.id, task.manifest.id);
        assert_eq!(loaded.result.status, ImportTaskStatus::Queued);
    }

    #[test]
    fn run_pipeline_completes_for_text_source() {
        let tmp = workspace_with_dirs();
        let src = tmp.path().join("greeting.txt");
        fs::write(&src, "hello world").unwrap();

        let task = create_task(tmp.path(), &src).unwrap();
        let done = run_pipeline(tmp.path(), &task.manifest.id).unwrap();

        assert_eq!(done.result.status, ImportTaskStatus::Completed);
        assert!(done.result.raw_path.is_some());
        assert!(done.result.wiki_path.is_some());
        assert_eq!(done.result.parser.as_deref(), Some("std::fs"));
        assert!(done.result.completed_at.is_some());

        let wiki_full = tmp.path().join(done.result.wiki_path.as_ref().unwrap());
        let body = fs::read_to_string(&wiki_full).unwrap();
        assert!(body.contains("hello world"));
    }

    #[test]
    fn run_pipeline_rejects_unsupported_type() {
        let tmp = workspace_with_dirs();
        let src = tmp.path().join("legacy.doc");
        fs::write(&src, b"legacy doc").unwrap();

        let task = create_task(tmp.path(), &src).unwrap();
        let done = run_pipeline(tmp.path(), &task.manifest.id).unwrap();

        assert_eq!(done.result.status, ImportTaskStatus::FailedConvert);
        assert!(done.result.error.unwrap().contains("不支持的文件类型"));
    }

    #[test]
    fn run_pipeline_completes_for_pdf_source() {
        let tmp = workspace_with_dirs();
        let src = tmp.path().join("source.pdf");
        fs::write(&src, minimal_text_pdf_bytes()).unwrap();

        let task = create_task(tmp.path(), &src).unwrap();
        let done = run_pipeline(tmp.path(), &task.manifest.id).unwrap();

        assert_eq!(done.result.status, ImportTaskStatus::Completed);
        assert_eq!(done.manifest.source_type, SourceType::Pdf);
        assert!(done
            .result
            .raw_path
            .as_deref()
            .is_some_and(|p| p.starts_with("raw/uploads/")));
        assert!(done
            .result
            .wiki_path
            .as_deref()
            .is_some_and(|p| p.contains("source-")));
        assert_eq!(done.result.parser.as_deref(), Some("pdf-extract"));

        let wiki_full = tmp.path().join(done.result.wiki_path.as_ref().unwrap());
        let body = fs::read_to_string(&wiki_full).unwrap();
        assert!(body.contains("type: \"pdf\""));
        assert!(body.contains("parser: \"pdf-extract\""));
        assert!(body.contains("Hello PDF text"));
    }

    #[test]
    fn derive_title_uses_source_filename_for_stable_traceable_wiki_name() {
        let title = derive_title(
            "The-Complete-Guide-to-Building-Skill-for-Claude.pdf",
            "Good - specific and actionable\n\nThe Complete Guide to Building Skills for Claude",
        );

        assert_eq!(title, "The-Complete-Guide-to-Building-Skill-for-Claude");
    }

    #[test]
    fn list_tasks_orders_most_recent_first() {
        let tmp = workspace_with_dirs();
        let src = tmp.path().join("a.txt");
        fs::write(&src, "first").unwrap();
        let t1 = create_task(tmp.path(), &src).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));
        let src2 = tmp.path().join("b.txt");
        fs::write(&src2, "second").unwrap();
        let t2 = create_task(tmp.path(), &src2).unwrap();

        let tasks = list_tasks(tmp.path(), 10);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].manifest.id, t2.manifest.id);
        assert_eq!(tasks[1].manifest.id, t1.manifest.id);
    }

    #[test]
    fn sync_import_file_runs_full_pipeline() {
        let tmp = workspace_with_dirs();
        let src = tmp.path().join("alpha.md");
        fs::write(&src, "# Alpha\n\nbody content").unwrap();

        let done = import_file(tmp.path(), &src).unwrap();
        assert_eq!(done.result.status, ImportTaskStatus::Completed);
    }
}
