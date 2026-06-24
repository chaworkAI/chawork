use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::{timeout, Duration};

use crate::services::tool_policy::{self, ToolAction, ToolPolicy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolPolicyFile {
    pub version: u32,
    pub default_enabled: bool,
    pub tools: HashMap<String, bool>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub required_by_skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolPolicyView {
    pub default_enabled: bool,
    pub tools: Vec<McpToolItem>,
    pub dirty: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpToolPolicyInput {
    pub default_enabled: bool,
    pub tools: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceMcpServer {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub startup_timeout_sec: Option<f64>,
    #[serde(default)]
    pub tool_timeout_sec: Option<f64>,
    #[serde(default)]
    pub tools: Vec<WorkspaceMcpServerTool>,
    #[serde(default)]
    pub last_tested_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceMcpServerView {
    pub servers: Vec<WorkspaceMcpServer>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceMcpServerTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceMcpServerTestResult {
    pub ok: bool,
    pub message: String,
    pub tools: Vec<WorkspaceMcpServerTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceMcpServerFile {
    version: u32,
    #[serde(default)]
    servers: BTreeMap<String, WorkspaceMcpServer>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ToolsCatalogEntry {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolsCatalogFile {
    tools: Option<Vec<ToolsCatalogEntry>>,
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn policy_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(".chawork").join("mcp-tools.json")
}

fn server_registry_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(".chawork").join("mcp-servers.json")
}

fn default_true() -> bool {
    true
}

const DEFAULT_TOOLS: &[(&str, &str)] = &[
    ("read_file", "Read a file from the workspace"),
    ("write_file", "Write or update a file in the workspace"),
    ("list_directory", "List directory contents"),
    ("search_text", "Search text within workspace files"),
    (
        "search_knowledge",
        "Search the workspace knowledge index (QMD)",
    ),
    ("get_file_info", "Get metadata for a workspace file"),
];

pub fn default_discovered_tools() -> Vec<McpToolItem> {
    DEFAULT_TOOLS
        .iter()
        .map(|(id, desc)| McpToolItem {
            id: id.to_string(),
            name: id.to_string(),
            description: Some(desc.to_string()),
            enabled: true,
            required_by_skills: Vec::new(),
        })
        .collect()
}

pub fn get_discovered_tools(mcp_dir: &Path) -> Vec<McpToolItem> {
    let catalog = mcp_dir.join("tools.json");
    if catalog.is_file() {
        if let Ok(raw) = fs::read_to_string(&catalog) {
            if let Ok(file) = serde_json::from_str::<ToolsCatalogFile>(&raw) {
                if let Some(tools) = file.tools {
                    let parsed: Vec<McpToolItem> = tools
                        .into_iter()
                        .filter_map(|t| {
                            let id = t.id.or(t.name.clone())?;
                            Some(McpToolItem {
                                name: t.name.unwrap_or_else(|| id.clone()),
                                description: t.description,
                                enabled: true,
                                required_by_skills: Vec::new(),
                                id,
                            })
                        })
                        .collect();
                    if !parsed.is_empty() {
                        return parsed;
                    }
                }
            }
        }
    }
    default_discovered_tools()
}

pub fn read_tool_policy(workspace_path: &Path) -> Option<McpToolPolicyFile> {
    let p = policy_path(workspace_path);
    if !p.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&p).ok()?;
    if let Ok(policy) = serde_json::from_str::<McpToolPolicyFile>(&raw) {
        return Some(policy);
    }
    // Legacy tool_policy format (default_action + overrides)
    if let Ok(legacy) = serde_json::from_str::<ToolPolicy>(&raw) {
        let default_enabled = legacy.default_action == ToolAction::Enabled;
        let tools = legacy
            .overrides
            .iter()
            .map(|(k, v)| (k.clone(), *v == ToolAction::Enabled))
            .collect();
        return Some(McpToolPolicyFile {
            version: 1,
            default_enabled,
            tools,
            updated_at: iso_now(),
        });
    }
    None
}

pub fn write_tool_policy(
    mcp_dir: &Path,
    workspace_path: &Path,
    policy: &McpToolPolicyFile,
) -> Result<(), String> {
    let p = policy_path(workspace_path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(policy).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| e.to_string())?;
    sync_legacy_tool_policy(mcp_dir, workspace_path, policy)
}

/// Keeps [`tool_policy`] (used by context builder) in sync with the v1 MCP policy file.
pub fn sync_legacy_tool_policy(
    mcp_dir: &Path,
    workspace_path: &Path,
    policy: &McpToolPolicyFile,
) -> Result<(), String> {
    let default_action = if policy.default_enabled {
        ToolAction::Enabled
    } else {
        ToolAction::Disabled
    };
    let mut overrides = std::collections::BTreeMap::new();
    for tool in get_discovered_tools(mcp_dir) {
        let enabled = policy
            .tools
            .get(&tool.id)
            .copied()
            .unwrap_or(policy.default_enabled);
        if enabled != policy.default_enabled {
            overrides.insert(
                tool.id,
                if enabled {
                    ToolAction::Enabled
                } else {
                    ToolAction::Disabled
                },
            );
        }
    }
    tool_policy::save(
        workspace_path,
        &ToolPolicy {
            default_action,
            overrides,
        },
    )
}

pub fn build_policy_view(mcp_dir: &Path, workspace_path: &Path) -> McpToolPolicyView {
    let discovered = get_discovered_tools(mcp_dir);
    let policy = read_tool_policy(workspace_path);
    let default_enabled = policy.as_ref().map(|p| p.default_enabled).unwrap_or(true);
    let updated_at = policy.as_ref().map(|p| p.updated_at.clone());

    let tools = discovered
        .into_iter()
        .map(|mut item| {
            item.enabled = policy
                .as_ref()
                .and_then(|p| p.tools.get(&item.id).copied())
                .unwrap_or(default_enabled);
            item
        })
        .collect();

    McpToolPolicyView {
        default_enabled,
        tools,
        dirty: false,
        updated_at,
    }
}

pub fn apply_policy_input(
    mcp_dir: &Path,
    workspace_path: &Path,
    input: &McpToolPolicyInput,
) -> Result<McpToolPolicyView, String> {
    let policy = McpToolPolicyFile {
        version: 1,
        default_enabled: input.default_enabled,
        tools: input.tools.clone(),
        updated_at: iso_now(),
    };
    write_tool_policy(mcp_dir, workspace_path, &policy)?;
    Ok(build_policy_view(mcp_dir, workspace_path))
}

fn normalize_server_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("MCP server 名称不能为空".to_string());
    }
    if name == "chawork_workspace" {
        return Err("chawork_workspace 是 ChaWork 内置 MCP server，不能覆盖".to_string());
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '[' | ']'))
    {
        return Err(format!("MCP server 名称包含不支持的字符: {name}"));
    }
    Ok(name.to_string())
}

fn read_string_map(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    value
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn validate_mcp_headers(name: &str, headers: &HashMap<String, String>) -> Result<(), String> {
    for (header_name, value) in headers {
        if header_name.eq_ignore_ascii_case("authorization") && value.contains("...") {
            return Err(format!(
                "MCP server `{name}` 的 Authorization 仍是示例占位值，请粘贴完整 token"
            ));
        }
    }
    Ok(())
}

fn read_string_vec(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_mcp_server(name: &str, value: &serde_json::Value) -> Result<WorkspaceMcpServer, String> {
    let name = normalize_server_name(name)?;
    let obj = value
        .as_object()
        .ok_or_else(|| format!("MCP server `{name}` 配置必须是对象"))?;
    let raw_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or_else(|| {
        if obj.get("command").is_some() {
            "stdio"
        } else {
            "streamable_http"
        }
    });
    let server_type = match raw_type {
        "streamable_http" | "http" => "streamable_http",
        "stdio" => "stdio",
        other => return Err(format!("MCP server `{name}` 不支持的类型: {other}")),
    }
    .to_string();

    let url = obj
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let command = obj
        .get("command")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match server_type.as_str() {
        "streamable_http" if url.is_none() => {
            return Err(format!("MCP server `{name}` 缺少 url"));
        }
        "stdio" if command.is_none() => {
            return Err(format!("MCP server `{name}` 缺少 command"));
        }
        _ => {}
    }

    let headers = {
        let mut headers = read_string_map(obj.get("headers"));
        for (k, v) in read_string_map(obj.get("http_headers")) {
            headers.insert(k, v);
        }
        headers
    };
    validate_mcp_headers(&name, &headers)?;

    Ok(WorkspaceMcpServer {
        name,
        server_type,
        url,
        command,
        args: read_string_vec(obj.get("args")),
        env: read_string_map(obj.get("env")),
        headers,
        enabled: obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        required: obj
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        startup_timeout_sec: obj.get("startup_timeout_sec").and_then(|v| v.as_f64()),
        tool_timeout_sec: obj.get("tool_timeout_sec").and_then(|v| v.as_f64()),
        tools: Vec::new(),
        last_tested_at: None,
    })
}

fn read_server_file(workspace_path: &Path) -> WorkspaceMcpServerFile {
    let p = server_registry_path(workspace_path);
    if !p.is_file() {
        return WorkspaceMcpServerFile {
            version: 1,
            servers: BTreeMap::new(),
            updated_at: iso_now(),
        };
    }
    let Ok(raw) = fs::read_to_string(&p) else {
        return WorkspaceMcpServerFile {
            version: 1,
            servers: BTreeMap::new(),
            updated_at: iso_now(),
        };
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| WorkspaceMcpServerFile {
        version: 1,
        servers: BTreeMap::new(),
        updated_at: iso_now(),
    })
}

fn write_server_file(workspace_path: &Path, file: &WorkspaceMcpServerFile) -> Result<(), String> {
    let p = server_registry_path(workspace_path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(&p, json).map_err(|e| e.to_string())
}

pub fn list_workspace_mcp_servers(workspace_path: &Path) -> WorkspaceMcpServerView {
    let file = read_server_file(workspace_path);
    WorkspaceMcpServerView {
        servers: file.servers.values().cloned().collect(),
        updated_at: Some(file.updated_at),
    }
}

pub fn upsert_workspace_mcp_server(
    workspace_path: &Path,
    server: WorkspaceMcpServer,
) -> Result<WorkspaceMcpServerView, String> {
    let value = serde_json::to_value(&server).map_err(|e| e.to_string())?;
    let server = parse_mcp_server(&server.name, &value)?;
    let name = server.name.clone();
    let mut file = read_server_file(workspace_path);
    file.updated_at = iso_now();
    file.servers.insert(name, server);
    write_server_file(workspace_path, &file)?;
    Ok(list_workspace_mcp_servers(workspace_path))
}

pub fn delete_workspace_mcp_server(
    workspace_path: &Path,
    name: &str,
) -> Result<WorkspaceMcpServerView, String> {
    let name = normalize_server_name(name)?;
    let mut file = read_server_file(workspace_path);
    file.updated_at = iso_now();
    file.servers.remove(&name);
    write_server_file(workspace_path, &file)?;
    Ok(list_workspace_mcp_servers(workspace_path))
}

pub fn import_mcp_servers_json(
    workspace_path: &Path,
    raw_json: &str,
) -> Result<WorkspaceMcpServerView, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw_json).map_err(|e| format!("解析 MCP JSON 失败: {e}"))?;
    let servers_obj = value
        .get("mcpServers")
        .or_else(|| value.get("mcp_servers"))
        .and_then(|v| v.as_object())
        .ok_or_else(|| "JSON 中缺少 mcpServers 对象".to_string())?;

    let mut file = read_server_file(workspace_path);
    for (name, server_value) in servers_obj {
        let server = parse_mcp_server(name, server_value)?;
        file.servers.insert(server.name.clone(), server);
    }
    file.updated_at = iso_now();
    write_server_file(workspace_path, &file)?;
    Ok(list_workspace_mcp_servers(workspace_path))
}

fn cache_workspace_mcp_server_tools(
    workspace_path: &Path,
    name: &str,
    tools: Vec<WorkspaceMcpServerTool>,
) -> Result<(), String> {
    let mut file = read_server_file(workspace_path);
    let server = file
        .servers
        .get_mut(name)
        .ok_or_else(|| format!("MCP server `{name}` 不存在"))?;
    let now = iso_now();
    server.tools = tools;
    server.last_tested_at = Some(now.clone());
    file.updated_at = now;
    write_server_file(workspace_path, &file)
}

fn parse_tools_list_response(
    value: &serde_json::Value,
) -> Result<Vec<WorkspaceMcpServerTool>, String> {
    if let Some(error) = value.get("error") {
        return Err(format!("MCP tools/list 返回错误: {error}"));
    }
    let tools = value
        .get("result")
        .and_then(|v| v.get("tools"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| "MCP tools/list 响应中缺少 result.tools".to_string())?;
    Ok(tools
        .iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(|v| v.as_str())?;
            Some(WorkspaceMcpServerTool {
                name: name.to_string(),
                description: tool
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        })
        .collect())
}

fn parse_json_or_sse_text(text: &str) -> Result<serde_json::Value, String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Ok(value);
    }
    for line in text.lines().rev() {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        return serde_json::from_str::<serde_json::Value>(data)
            .map_err(|e| format!("解析 MCP SSE data 失败: {e}"));
    }
    Err("MCP 响应不是 JSON，也没有可解析的 SSE data".to_string())
}

fn http_header_map(
    headers: &HashMap<String, String>,
    session_id: Option<&str>,
) -> Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| format!("无效 HTTP header 名称 `{name}`: {e}"))?;
        let value = HeaderValue::from_str(value)
            .map_err(|e| format!("无效 HTTP header 值 `{name}`: {e}"))?;
        map.insert(name, value);
    }
    if let Some(session_id) = session_id {
        let value =
            HeaderValue::from_str(session_id).map_err(|e| format!("无效 MCP session id: {e}"))?;
        map.insert(HeaderName::from_static("mcp-session-id"), value);
    }
    Ok(map)
}

struct McpHttpResponse {
    value: serde_json::Value,
    session_id: Option<String>,
}

async fn post_mcp_http(
    client: &reqwest::Client,
    server: &WorkspaceMcpServer,
    session_id: Option<&str>,
    id: i64,
    method: &str,
    params: serde_json::Value,
) -> Result<McpHttpResponse, String> {
    let url = server
        .url
        .as_deref()
        .ok_or_else(|| format!("MCP server `{}` 缺少 url", server.name))?;
    let response = client
        .post(url)
        .headers(http_header_map(&server.headers, session_id)?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .send()
        .await
        .map_err(|e| format!("请求 MCP server `{}` 失败: {e}", server.name))?;
    let status = response.status();
    let response_session_id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取 MCP 响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "MCP server `{}` 返回 HTTP {status}: {text}",
            server.name
        ));
    }
    Ok(McpHttpResponse {
        value: parse_json_or_sse_text(&text)?,
        session_id: response_session_id,
    })
}

async fn post_mcp_http_notification(
    client: &reqwest::Client,
    server: &WorkspaceMcpServer,
    session_id: Option<&str>,
    method: &str,
) -> Result<Option<String>, String> {
    let url = server
        .url
        .as_deref()
        .ok_or_else(|| format!("MCP server `{}` 缺少 url", server.name))?;
    let response = client
        .post(url)
        .headers(http_header_map(&server.headers, session_id)?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": method,
        }))
        .send()
        .await
        .map_err(|e| format!("请求 MCP server `{}` 失败: {e}", server.name))?;
    let status = response.status();
    let response_session_id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取 MCP 响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "MCP server `{}` 返回 HTTP {status}: {text}",
            server.name
        ));
    }
    Ok(response_session_id)
}

pub async fn test_workspace_mcp_server(
    workspace_path: &Path,
    name: &str,
) -> Result<WorkspaceMcpServerTestResult, String> {
    let name = normalize_server_name(name)?;
    let file = read_server_file(workspace_path);
    let server = file
        .servers
        .get(&name)
        .ok_or_else(|| format!("MCP server `{name}` 不存在"))?
        .clone();
    if server.server_type != "streamable_http" {
        return Err("当前只支持手动测试 Streamable HTTP MCP server".to_string());
    }
    validate_mcp_headers(&server.name, &server.headers)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let initialize_response = timeout(
        Duration::from_secs(25),
        post_mcp_http(
            &client,
            &server,
            None,
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "chawork", "version": env!("CARGO_PKG_VERSION") }
            }),
        ),
    )
    .await
    .map_err(|_| format!("MCP server `{}` initialize 超时", server.name))??;
    let mut session_id = initialize_response.session_id;

    if let Some(current_session_id) = session_id.as_deref() {
        let initialized_session_id = timeout(
            Duration::from_secs(25),
            post_mcp_http_notification(
                &client,
                &server,
                Some(current_session_id),
                "notifications/initialized",
            ),
        )
        .await
        .map_err(|_| format!("MCP server `{}` initialized notification 超时", server.name))??;
        if initialized_session_id.is_some() {
            session_id = initialized_session_id;
        }
    }

    let tools_response = timeout(
        Duration::from_secs(25),
        post_mcp_http(
            &client,
            &server,
            session_id.as_deref(),
            2,
            "tools/list",
            json!({}),
        ),
    )
    .await
    .map_err(|_| format!("MCP server `{}` tools/list 超时", server.name))??;
    let tools = parse_tools_list_response(&tools_response.value)?;
    cache_workspace_mcp_server_tools(workspace_path, &server.name, tools.clone())?;
    Ok(WorkspaceMcpServerTestResult {
        ok: true,
        message: format!("获取到 {} 个工具", tools.len()),
        tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tools_list_response_extracts_tool_names() {
        let value = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    { "name": "search", "description": "Search docs" },
                    { "name": "write" }
                ]
            }
        });

        let tools = parse_tools_list_response(&value).expect("tools");

        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "search");
        assert_eq!(tools[0].description.as_deref(), Some("Search docs"));
        assert_eq!(tools[1].name, "write");
        assert_eq!(tools[1].description, None);
    }

    #[test]
    fn http_header_map_adds_mcp_session_id_for_followup_requests() {
        let headers = HashMap::from([(
            "Authorization".to_string(),
            "Bearer ms-inline-token".to_string(),
        )]);

        let map = http_header_map(&headers, Some("session-123")).expect("headers");

        assert_eq!(
            map.get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer ms-inline-token")
        );
        assert_eq!(
            map.get("mcp-session-id").and_then(|v| v.to_str().ok()),
            Some("session-123")
        );
    }

    #[test]
    fn import_json_overwrites_existing_server_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork")).unwrap();

        let first = r#"{
          "mcpServers": {
            "xsct-bench": {
              "type": "streamable_http",
              "url": "https://old.example/mcp"
            }
          }
        }"#;
        let second = r#"{
          "mcpServers": {
            "xsct-bench": {
              "type": "streamable_http",
              "url": "https://mcp.api-inference.modelscope.net/6460f2d4ed8347/mcp",
              "headers": {
                "Authorization": "Bearer ms-inline-token"
              }
            }
          }
        }"#;

        import_mcp_servers_json(ws, first).expect("first import");
        let view = import_mcp_servers_json(ws, second).expect("second import");

        assert_eq!(view.servers.len(), 1);
        let server = &view.servers[0];
        assert_eq!(server.name, "xsct-bench");
        assert_eq!(
            server.url.as_deref(),
            Some("https://mcp.api-inference.modelscope.net/6460f2d4ed8347/mcp")
        );
        assert_eq!(
            server.headers.get("Authorization").map(String::as_str),
            Some("Bearer ms-inline-token")
        );
    }

    #[test]
    fn import_json_rejects_placeholder_authorization_header() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork")).unwrap();

        let err = import_mcp_servers_json(
            ws,
            r#"{
              "mcpServers": {
                "xsct-bench": {
                  "type": "streamable_http",
                  "url": "https://mcp.api-inference.modelscope.net/6460f2d4ed8347/mcp",
                  "headers": {
                    "Authorization": "Bearer ms-..."
                  }
                }
              }
            }"#,
        )
        .expect_err("placeholder token should be rejected");

        assert!(err.contains("Authorization"));
        assert!(err.contains("示例占位"));
    }

    #[tokio::test]
    async fn test_server_rejects_existing_placeholder_authorization_header() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork")).unwrap();
        fs::write(
            server_registry_path(ws),
            r#"{
              "version": 1,
              "updated_at": "2026-06-03T00:00:00Z",
              "servers": {
                "xsct-bench": {
                  "name": "xsct-bench",
                  "type": "streamable_http",
                  "url": "http://127.0.0.1:1/mcp",
                  "headers": {
                    "Authorization": "Bearer ms-..."
                  },
                  "enabled": true,
                  "required": false
                }
              }
            }"#,
        )
        .unwrap();

        let err = test_workspace_mcp_server(ws, "xsct-bench")
            .await
            .expect_err("placeholder token should be rejected before HTTP");

        assert!(err.contains("Authorization"));
        assert!(err.contains("示例占位"));
    }

    #[tokio::test]
    async fn test_server_sends_initialized_notification_before_tools_list() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        use axum::extract::State;
        use axum::http::HeaderMap;
        use axum::http::HeaderValue;
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use axum::response::Response;
        use axum::routing::post;
        use axum::Json;
        use axum::Router;

        #[derive(Clone)]
        struct MockState {
            initialized: Arc<AtomicBool>,
        }

        async fn handler(
            State(state): State<MockState>,
            headers: HeaderMap,
            Json(body): Json<serde_json::Value>,
        ) -> Response {
            let method = body.get("method").and_then(|v| v.as_str()).unwrap_or("");
            match method {
                "initialize" => {
                    let mut response = Json(json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id").cloned().unwrap_or(json!(1)),
                        "result": {
                            "protocolVersion": "2025-03-26",
                            "capabilities": { "tools": { "listChanged": false } },
                            "serverInfo": { "name": "mock", "version": "1.0.0" }
                        }
                    }))
                    .into_response();
                    response
                        .headers_mut()
                        .insert("mcp-session-id", HeaderValue::from_static("session-123"));
                    response
                }
                "notifications/initialized" => {
                    if headers.get("mcp-session-id").and_then(|v| v.to_str().ok())
                        == Some("session-123")
                    {
                        state.initialized.store(true, Ordering::SeqCst);
                    }
                    let mut response = StatusCode::ACCEPTED.into_response();
                    response
                        .headers_mut()
                        .insert("mcp-session-id", HeaderValue::from_static("session-123"));
                    response
                }
                "tools/list" if state.initialized.load(Ordering::SeqCst) => Json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(json!(2)),
                    "result": {
                        "tools": [
                            { "name": "mock_tool", "description": "Mock tool" }
                        ]
                    }
                }))
                .into_response(),
                "tools/list" => Json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(json!(2)),
                    "error": {
                        "code": -32602,
                        "message": "Invalid request parameters",
                        "data": ""
                    }
                }))
                .into_response(),
                _ => StatusCode::BAD_REQUEST.into_response(),
            }
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let state = MockState {
            initialized: Arc::new(AtomicBool::new(false)),
        };
        let app = Router::new()
            .route("/mcp", post(handler))
            .with_state(state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork")).unwrap();
        import_mcp_servers_json(
            ws,
            &format!(
                r#"{{
                  "mcpServers": {{
                    "mock": {{
                      "type": "streamable_http",
                      "url": "{url}"
                    }}
                  }}
                }}"#
            ),
        )
        .unwrap();

        let result = test_workspace_mcp_server(ws, "mock").await.unwrap();

        assert!(state.initialized.load(Ordering::SeqCst));
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].name, "mock_tool");

        let view = list_workspace_mcp_servers(ws);
        assert_eq!(view.servers.len(), 1);
        assert_eq!(view.servers[0].tools.len(), 1);
        assert_eq!(view.servers[0].tools[0].name, "mock_tool");
        assert!(view.servers[0].last_tested_at.is_some());
    }

    #[test]
    fn round_trip_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join(".chawork")).unwrap();
        let mut tools = HashMap::new();
        tools.insert("read_file".to_string(), false);
        let policy = McpToolPolicyFile {
            version: 1,
            default_enabled: true,
            tools,
            updated_at: iso_now(),
        };
        write_tool_policy(Path::new("/nonexistent/mcp"), ws, &policy).unwrap();
        let loaded = read_tool_policy(ws).expect("policy");
        assert!(!loaded.tools.get("read_file").copied().unwrap_or(true));
    }
}
