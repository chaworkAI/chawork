#!/usr/bin/env bash
set -euo pipefail

profile="${1:-release}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
backend_target="$repo_root/backend/target/$profile"
runtime_target="$repo_root/chawork-runtime/codex-rs/target/$profile"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "macOS release build must run on Apple Silicon arm64; got $(uname -m)" >&2
  exit 1
fi

runtime_exe="$runtime_target/chawork-runtime"
codex_exe="$runtime_target/codex"
mcp_exe="$backend_target/chawork-mcp-server"

for exe in "$runtime_exe" "$codex_exe" "$mcp_exe"; do
  if [[ ! -x "$exe" ]]; then
    echo "Missing executable for smoke test: $exe" >&2
    exit 1
  fi
done

workspace="${TMPDIR:-/tmp}/ChaWork Smoke Workspace With Spaces"
mkdir -p "$workspace"

runtime_init="$(node -e 'const ws=process.argv[1]; console.log(JSON.stringify({id:1,method:"runtime/initialize",params:{contractVersion:1,client:{name:"macos-smoke",version:"0.1.0"},workspacePath:ws,requiredCapabilities:[]}}))' "$workspace")"
runtime_out="$(printf '%s\n' "$runtime_init" | "$runtime_exe" --protocol=jsonrpc)"
if [[ "$runtime_out" != *'"contractVersion":1'* ]]; then
  echo "runtime/initialize did not return contractVersion=1" >&2
  echo "$runtime_out" >&2
  exit 1
fi

mcp_init='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"macos-smoke","version":"0.1.0"}}}'
mcp_tools='{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
mcp_out="$(printf '%s\n%s\n' "$mcp_init" "$mcp_tools" | "$mcp_exe" --workspace "$workspace")"
if [[ "$mcp_out" != *'"tools"'* ]]; then
  echo "MCP tools/list did not return tools" >&2
  echo "$mcp_out" >&2
  exit 1
fi

echo "macOS arm64 runtime and MCP smoke checks passed."
