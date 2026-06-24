//! Standalone MCP server binary (JSON-RPC over stdio). Not linked to the Tauri `lib` crate.

#[path = "../mcp_server.rs"]
mod mcp_server;
#[path = "../path_safety.rs"]
mod path_safety;

use mcp_server::{parse_workspace_arg, McpServer};

fn main() {
    // FIRST THING — write to /tmp before doing anything else, so we know
    // the binary was at least launched, even if argv parsing fails.
    {
        use std::io::Write;
        let pid = std::process::id();
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let argv: Vec<String> = std::env::args().collect();
        let argv_joined = argv.join(" ");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/chawork-mcp-server-launch.log")
        {
            let _ = writeln!(f, "{ts} pid={pid} argv={argv_joined}");
        }
        // Also stderr so codex captures it
        eprintln!("[chawork-mcp-server] launched pid={pid} argv={argv_joined}");
    }

    let args: Vec<String> = std::env::args().collect();
    let parsed = parse_workspace_arg(&args)
        .expect("Usage: chawork-mcp-server --workspace <path> [--disabled-tool <name> ...]");

    let server = McpServer::with_args(parsed);
    server.run_stdio();
}
