//! Workspace tools MCP server — JSON-RPC 2.0 over stdio (used by `chawork-mcp-server` binary only).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chawork_lib::services::qmd_index;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "chawork-workspace";
const SERVER_VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct McpServerArgs {
    pub workspace: PathBuf,
    pub disabled_tools: HashSet<String>,
}

pub fn parse_workspace_arg(args: &[String]) -> Result<McpServerArgs, String> {
    let mut workspace: Option<PathBuf> = None;
    let mut disabled: HashSet<String> = HashSet::new();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--workspace" => {
                if i + 1 >= args.len() {
                    return Err("--workspace requires a path".to_string());
                }
                workspace = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--disabled-tool" => {
                if i + 1 >= args.len() {
                    return Err("--disabled-tool requires a name".to_string());
                }
                let n = args[i + 1].trim();
                if !n.is_empty() {
                    disabled.insert(n.to_string());
                }
                i += 2;
            }
            other if other.starts_with("--disabled-tool=") => {
                let n = other.trim_start_matches("--disabled-tool=").trim();
                if !n.is_empty() {
                    disabled.insert(n.to_string());
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    let workspace = workspace.ok_or_else(|| "missing --workspace".to_string())?;
    Ok(McpServerArgs {
        workspace,
        disabled_tools: disabled,
    })
}

pub struct McpServer {
    workspace_root: PathBuf,
    disabled_tools: HashSet<String>,
}

impl McpServer {
    pub fn with_args(args: McpServerArgs) -> Self {
        let workspace_root = args
            .workspace
            .canonicalize()
            .unwrap_or_else(|e| panic!("Invalid workspace path: {e}"));
        Self {
            workspace_root,
            disabled_tools: args.disabled_tools,
        }
    }

    pub fn run_stdio(&self) {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) else {
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::to_string(&error_response(Value::Null, -32700, "Parse error",))
                        .unwrap_or_default()
                );
                let _ = stdout.flush();
                continue;
            };

            if req.jsonrpc != "2.0" {
                if let Some(id) = req.id.clone() {
                    let _ = writeln!(
                        stdout,
                        "{}",
                        serde_json::to_string(&error_response(id, -32600, "Invalid Request",))
                            .unwrap_or_default()
                    );
                    let _ = stdout.flush();
                }
                continue;
            }

            let Some(method) = req.method.clone() else {
                if let Some(id) = req.id.clone() {
                    let _ = writeln!(
                        stdout,
                        "{}",
                        serde_json::to_string(&error_response(id, -32600, "Invalid Request",))
                            .unwrap_or_default()
                    );
                    let _ = stdout.flush();
                }
                continue;
            };

            let Some(id) = req.id.clone() else {
                if method == "notifications/initialized" {
                    continue;
                }
                continue;
            };

            let response = match method.as_str() {
                "initialize" => self.handle_initialize(id, req.params),
                "resources/list" => self.handle_resources_list(id),
                "resources/templates/list" => self.handle_resource_templates_list(id),
                "tools/list" => self.handle_tools_list(id),
                "tools/call" => self.handle_tools_call(id, req.params),
                _ => error_response(id, -32601, "Method not found"),
            };

            let _ = writeln!(
                stdout,
                "{}",
                serde_json::to_string(&response).unwrap_or_default()
            );
            let _ = stdout.flush();
        }
    }

    fn handle_initialize(&self, id: Value, _params: Option<Value>) -> Value {
        success_response(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
            }),
        )
    }

    fn handle_resources_list(&self, id: Value) -> Value {
        success_response(id, json!({ "resources": [] }))
    }

    fn handle_resource_templates_list(&self, id: Value) -> Value {
        success_response(id, json!({ "resourceTemplates": [] }))
    }

    fn handle_tools_list(&self, id: Value) -> Value {
        let all: Vec<Value> = vec![
            tool_def(
                "read_file",
                "Read a file from the workspace",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path within workspace" }
                    },
                    "required": ["path"]
                }),
            ),
            tool_def(
                "write_file",
                "Write content to a file in the workspace",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path within workspace" },
                        "content": { "type": "string", "description": "File content" }
                    },
                    "required": ["path", "content"]
                }),
            ),
            tool_def(
                "list_directory",
                "List files and directories in a workspace path",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative directory path (default: root)" },
                        "recursive": { "type": "boolean", "description": "List recursively" }
                    },
                    "required": []
                }),
            ),
            tool_def(
                "search_text",
                "Search for text in workspace files",
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "path": { "type": "string", "description": "Subdirectory to search in" }
                    },
                    "required": ["query"]
                }),
            ),
            tool_def(
                "search_knowledge",
                "BM25 search over wiki/ and raw/transcripts/ (embedded index)",
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results (default 10, max 100)" }
                    },
                    "required": ["query"]
                }),
            ),
            tool_def(
                "get_file_info",
                "Get file metadata",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" }
                    },
                    "required": ["path"]
                }),
            ),
            tool_def(
                "read_file_range",
                "Read a UTF-8 character slice of a workspace file. Pair with `search_knowledge` results (start_char/end_char) to fetch just the matching chunk instead of the whole document.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path within workspace" },
                        "start_char": { "type": "integer", "description": "Inclusive UTF-8 character offset to start reading from (0-based)" },
                        "end_char": { "type": "integer", "description": "Exclusive UTF-8 character offset to stop at" }
                    },
                    "required": ["path", "start_char", "end_char"]
                }),
            ),
        ];
        let tools: Vec<Value> = all
            .into_iter()
            .filter(|t| {
                let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                !self.disabled_tools.contains(name)
            })
            .collect();
        success_response(id, json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, id: Value, params: Option<Value>) -> Value {
        let Some(params) = params else {
            return Self::tools_call_error(id, "Missing params");
        };
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(Value::Null);

        if self.disabled_tools.contains(name) {
            return Self::tools_call_error(
                id,
                &format!("Tool '{name}' is disabled by workspace policy"),
            );
        }

        let result = match name {
            "read_file" => self.tool_read_file(&args),
            "read_file_range" => self.tool_read_file_range(&args),
            "write_file" => self.tool_write_file(&args),
            "list_directory" => self.tool_list_directory(&args),
            "search_text" => self.tool_search_text(&args),
            "search_knowledge" => self.tool_search_knowledge(&args),
            "get_file_info" => self.tool_get_file_info(&args),
            _ => Err(format!("Unknown tool: {name}")),
        };

        match result {
            Ok(text) => {
                success_response(id, json!({ "content": [{ "type": "text", "text": text }] }))
            }
            Err(msg) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": [{ "type": "text", "text": format!("Error: {msg}") }], "isError": true }
            }),
        }
    }

    fn tools_call_error(id: Value, msg: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "content": [{ "type": "text", "text": format!("Error: {msg}") }], "isError": true }
        })
    }

    fn tool_read_file(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing path".to_string())?;
        let full = self.validate_path(path)?;
        if !full.is_file() {
            return Err("not a file".to_string());
        }
        fs::read_to_string(&full).map_err(|e| e.to_string())
    }

    fn tool_read_file_range(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing path".to_string())?;
        let start_char =
            args.get("start_char")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "missing or invalid start_char".to_string())? as usize;
        let end_char =
            args.get("end_char")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "missing or invalid end_char".to_string())? as usize;
        if end_char < start_char {
            return Err(format!(
                "end_char ({end_char}) must be >= start_char ({start_char})"
            ));
        }
        let full = self.validate_path(path)?;
        if !full.is_file() {
            return Err("not a file".to_string());
        }
        let body = fs::read_to_string(&full).map_err(|e| e.to_string())?;
        let total = body.chars().count();
        if start_char > total {
            return Err(format!(
                "start_char ({start_char}) is beyond end of file ({total} chars)"
            ));
        }
        let take = end_char.saturating_sub(start_char).min(total - start_char);
        // 防止单次返回过大字符串塞爆 Codex 上下文 — 上限 200KB 字符
        const MAX_RANGE_CHARS: usize = 200_000;
        if take > MAX_RANGE_CHARS {
            return Err(format!(
                "requested range too large ({take} chars); cap is {MAX_RANGE_CHARS}"
            ));
        }
        let slice: String = body.chars().skip(start_char).take(take).collect();
        Ok(slice)
    }

    fn tool_write_file(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing path".to_string())?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing content".to_string())?;

        let full = self.validate_write_path(path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&full, content.as_bytes()).map_err(|e| e.to_string())?;
        let rel = relative_display(&self.workspace_root, &full);
        self.append_operation_log("write_file", &rel, "ok");

        if Self::is_knowledge_path(path) {
            self.mark_qmd_stale();
        }

        Ok(format!("Wrote {}", rel))
    }

    /// Returns true if a relative path falls under knowledge-indexed directories.
    fn is_knowledge_path(rel_path: &str) -> bool {
        let normalized = rel_path.replace('\\', "/");
        normalized.starts_with("wiki/")
            || normalized.starts_with("wiki\\")
            || normalized == "wiki"
            || normalized.starts_with("raw/transcripts/")
            || normalized.starts_with("raw\\transcripts\\")
    }

    /// Write a stale marker so the Tauri backend knows the QMD index needs a refresh.
    fn mark_qmd_stale(&self) {
        let marker = self.workspace_root.join(".chawork").join("qmd-index-stale");
        let _ = fs::create_dir_all(self.workspace_root.join(".chawork"));
        let _ = fs::write(&marker, chrono::Utc::now().to_rfc3339());
    }

    fn tool_list_directory(&self, args: &Value) -> Result<String, String> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let recursive = args
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let dir = self.validate_path_for_list(path_str)?;
        if !dir.is_dir() {
            return Err("not a directory".to_string());
        }

        let mut lines = Vec::new();
        if recursive {
            self.list_recursive(&dir, &self.workspace_root, &mut lines)?;
            lines.sort();
        } else {
            for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                lines.push(format_entry(&entry.path(), &self.workspace_root));
            }
            lines.sort();
        }
        Ok(lines.join("\n"))
    }

    fn list_recursive(
        &self,
        dir: &Path,
        workspace: &Path,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            out.push(format_entry(&p, workspace));
            if p.is_dir() {
                self.list_recursive(&p, workspace, out)?;
            }
        }
        Ok(())
    }

    fn tool_search_text(&self, args: &Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing query".to_string())?;
        if query.is_empty() {
            return Err("empty query".to_string());
        }
        let sub = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let search_root = if sub.is_empty() {
            self.workspace_root.clone()
        } else {
            self.validate_path_for_list(sub)?
        };
        if !search_root.is_dir() {
            return Err("search path is not a directory".to_string());
        }

        const EXT: &[&str] = &["md", "yaml", "yml", "json", "txt"];
        let mut matches_out = Vec::new();
        self.search_dir(&search_root, query, EXT, &mut matches_out)?;
        if matches_out.is_empty() {
            return Ok("(no matches)".to_string());
        }
        matches_out.sort();
        Ok(matches_out.join("\n"))
    }

    fn tool_search_knowledge(&self, args: &Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing query".to_string())?;
        if query.trim().is_empty() {
            return Err("empty query".to_string());
        }
        let limit = args.get("limit").and_then(|v| {
            v.as_u64()
                .map(|n| n as usize)
                .or_else(|| v.as_i64().map(|n| n.max(1) as usize))
        });
        let results = qmd_index::search(&self.workspace_root, query, limit)?;
        serde_json::to_string_pretty(&results).map_err(|e| e.to_string())
    }

    fn search_dir(
        &self,
        dir: &Path,
        query: &str,
        extensions: &[&str],
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        const MAX_MATCHES: usize = 500;
        const MAX_FILE_BYTES: u64 = 512 * 1024;

        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            if out.len() >= MAX_MATCHES {
                return Ok(());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.is_dir() {
                self.search_dir(&p, query, extensions, out)?;
            } else if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !extensions.iter().any(|x| *x == ext) {
                    continue;
                }
                let meta = fs::metadata(&p).map_err(|e| e.to_string())?;
                if meta.len() > MAX_FILE_BYTES {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&p) else {
                    continue;
                };
                for (i, line) in text.lines().enumerate() {
                    if out.len() >= MAX_MATCHES {
                        return Ok(());
                    }
                    if line.contains(query) {
                        let rel = relative_display(&self.workspace_root, &p);
                        out.push(format!("{}:{}: {}", rel, i + 1, truncate_line(line, 200)));
                    }
                }
            }
        }
        Ok(())
    }

    fn tool_get_file_info(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing path".to_string())?;
        let full = self.validate_path(path)?;
        let meta = fs::metadata(&full).map_err(|e| e.to_string())?;
        let kind = if meta.is_dir() {
            "directory"
        } else if meta.is_file() {
            "file"
        } else {
            "other"
        };
        let size = meta.len();
        let modified = meta.modified().ok().map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        });
        let rel = relative_display(&self.workspace_root, &full);
        let modified_str = modified.as_deref().unwrap_or("unknown");
        Ok(format!(
            "path: {rel}\ntype: {kind}\nsize: {size}\nmodified: {modified_str}"
        ))
    }

    fn validate_path(&self, relative: &str) -> Result<PathBuf, String> {
        let full = self.safe_join(relative)?;
        let full = fs::canonicalize(&full).map_err(|e| format!("路径无效: {e}"))?;
        if !full.starts_with(&self.workspace_root) {
            return Err("路径不在工作区范围内".to_string());
        }
        Ok(full)
    }

    fn validate_write_path(&self, relative: &str) -> Result<PathBuf, String> {
        let full = self.safe_join(relative)?;
        if full.exists() {
            let c = fs::canonicalize(&full).map_err(|e| format!("路径无效: {e}"))?;
            if !c.starts_with(&self.workspace_root) {
                return Err("路径不在工作区范围内".to_string());
            }
            return Ok(c);
        }
        let parent = full
            .parent()
            .ok_or_else(|| "路径不在工作区范围内".to_string())?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let canon_parent = fs::canonicalize(parent).map_err(|e| format!("路径无效: {e}"))?;
        if !canon_parent.starts_with(&self.workspace_root) {
            return Err("路径不在工作区范围内".to_string());
        }
        let name = full
            .file_name()
            .ok_or_else(|| "路径不在工作区范围内".to_string())?;
        Ok(canon_parent.join(name))
    }

    fn validate_path_for_list(&self, relative: &str) -> Result<PathBuf, String> {
        let full = self.safe_join(relative)?;
        if full.exists() {
            let c = fs::canonicalize(&full).map_err(|e| format!("路径无效: {e}"))?;
            if !c.starts_with(&self.workspace_root) {
                return Err("路径不在工作区范围内".to_string());
            }
            return Ok(c);
        }
        if !full.starts_with(&self.workspace_root) {
            return Err("路径不在工作区范围内".to_string());
        }
        Ok(full)
    }

    fn safe_join(&self, relative: &str) -> Result<PathBuf, String> {
        crate::path_safety::safe_join_workspace(&self.workspace_root, relative)
    }

    fn append_operation_log(&self, tool: &str, path: &str, status: &str) {
        let log_dir = self.workspace_root.join("logs").join("operations");
        if fs::create_dir_all(&log_dir).is_err() {
            return;
        }
        let log_path = log_dir.join("ops.jsonl");
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let line = serde_json::to_string(&json!({
            "timestamp": ts,
            "tool": tool,
            "path": path,
            "status": status
        }))
        .unwrap_or_default();

        let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) else {
            return;
        };
        let _ = writeln!(f, "{line}");
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcErrorData {
    code: i32,
    message: String,
}

#[derive(Serialize)]
struct JsonRpcError {
    jsonrpc: &'static str,
    id: Value,
    error: JsonRpcErrorData,
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    serde_json::to_value(JsonRpcError {
        jsonrpc: "2.0",
        id,
        error: JsonRpcErrorData {
            code,
            message: message.to_string(),
        },
    })
    .unwrap_or(json!({}))
}

fn tool_def(name: &'static str, description: &'static str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn format_entry(path: &Path, workspace: &Path) -> String {
    let rel = path.strip_prefix(workspace).unwrap_or(path).display();
    let suffix = if path.is_dir() {
        "/"
    } else if path.is_file() {
        ""
    } else {
        "?"
    };
    let kind = if path.is_dir() {
        "dir"
    } else if path.is_file() {
        "file"
    } else {
        "other"
    };
    format!("[{kind}] {rel}{suffix}")
}

fn relative_display(root: &Path, full: &Path) -> String {
    full.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| full.display().to_string())
}

fn truncate_line(s: &str, max: usize) -> String {
    let t = s.trim_end();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let mut out = String::new();
        for (i, ch) in t.chars().enumerate() {
            if i >= max {
                out.push('…');
                break;
            }
            out.push(ch);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server(disabled: &[&str]) -> (tempfile::TempDir, McpServer) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let server = McpServer::with_args(McpServerArgs {
            workspace: tmp.path().to_path_buf(),
            disabled_tools: disabled.iter().map(|s| s.to_string()).collect(),
        });
        (tmp, server)
    }

    #[test]
    fn read_file_range_returns_char_slice() {
        let (_tmp, server) = make_server(&[]);
        let body = "Hello world. 中文测试。Another sentence.";
        fs::write(server.workspace_root.join("note.md"), body).unwrap();

        let args = json!({ "path": "note.md", "start_char": 6, "end_char": 11 });
        let out = server.tool_read_file_range(&args).expect("read");
        assert_eq!(out, "world");
    }

    #[test]
    fn read_file_range_handles_cjk_correctly() {
        let (_tmp, server) = make_server(&[]);
        let body = "Hello 中文测试 Done";
        fs::write(server.workspace_root.join("note.md"), body).unwrap();

        // chars 6..10 covers the 4 CJK characters
        let args = json!({ "path": "note.md", "start_char": 6, "end_char": 10 });
        let out = server.tool_read_file_range(&args).expect("read");
        assert_eq!(out, "中文测试");
    }

    #[test]
    fn read_file_range_rejects_inverted_range() {
        let (_tmp, server) = make_server(&[]);
        fs::write(server.workspace_root.join("note.md"), "abc").unwrap();

        let args = json!({ "path": "note.md", "start_char": 5, "end_char": 1 });
        let err = server.tool_read_file_range(&args).unwrap_err();
        assert!(err.contains("must be >= start_char"));
    }

    #[test]
    fn read_file_range_clamps_end_to_file_length() {
        let (_tmp, server) = make_server(&[]);
        fs::write(server.workspace_root.join("note.md"), "abcdef").unwrap();

        let args = json!({ "path": "note.md", "start_char": 3, "end_char": 999 });
        let out = server.tool_read_file_range(&args).expect("read");
        assert_eq!(out, "def");
    }

    #[test]
    fn read_file_range_rejects_start_beyond_file() {
        let (_tmp, server) = make_server(&[]);
        fs::write(server.workspace_root.join("note.md"), "abc").unwrap();

        let args = json!({ "path": "note.md", "start_char": 100, "end_char": 200 });
        let err = server.tool_read_file_range(&args).unwrap_err();
        assert!(err.contains("beyond end of file"));
    }

    #[test]
    fn read_file_range_rejects_path_escape() {
        let (_tmp, server) = make_server(&[]);

        let args = json!({ "path": "../escape.md", "start_char": 0, "end_char": 10 });
        let err = server.tool_read_file_range(&args).unwrap_err();
        assert!(!err.is_empty(), "should reject escape attempt");
    }

    #[test]
    fn tools_list_includes_read_file_range_when_not_disabled() {
        let (_tmp, server) = make_server(&[]);
        let resp = server.handle_tools_list(json!(1));
        let tools = resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .expect("tools array");
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();
        assert!(names.contains(&"read_file_range"));
    }

    #[test]
    fn tools_list_excludes_disabled_tools() {
        let (_tmp, server) = make_server(&["read_file_range", "write_file"]);
        let resp = server.handle_tools_list(json!(1));
        let tools = resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .expect("tools array");
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();
        assert!(!names.contains(&"read_file_range"));
        assert!(!names.contains(&"write_file"));
        assert!(names.contains(&"read_file"));
    }

    #[test]
    fn resources_list_returns_empty_resources() {
        let (_tmp, server) = make_server(&[]);
        let resp = server.handle_resources_list(json!(1));
        let resources = resp
            .get("result")
            .and_then(|r| r.get("resources"))
            .and_then(|r| r.as_array())
            .expect("resources array");
        assert!(resources.is_empty());
    }

    #[test]
    fn resource_templates_list_returns_empty_templates() {
        let (_tmp, server) = make_server(&[]);
        let resp = server.handle_resource_templates_list(json!(1));
        let templates = resp
            .get("result")
            .and_then(|r| r.get("resourceTemplates"))
            .and_then(|r| r.as_array())
            .expect("resourceTemplates array");
        assert!(templates.is_empty());
    }

    #[test]
    fn tools_call_rejects_disabled_tool() {
        let (_tmp, server) = make_server(&["read_file_range"]);
        let resp = server.handle_tools_call(
            json!(1),
            Some(json!({ "name": "read_file_range", "arguments": {} })),
        );
        let text = resp
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        assert!(text.contains("disabled by workspace policy"));
    }
}
