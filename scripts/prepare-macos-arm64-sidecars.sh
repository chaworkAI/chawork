#!/usr/bin/env bash
set -euo pipefail

target_triple="${1:-aarch64-apple-darwin}"
profile="${2:-release}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
backend_target="$repo_root/backend/target/$profile"
runtime_target="$repo_root/chawork-runtime/codex-rs/target/$profile"
binaries_dir="$repo_root/backend/binaries"

mkdir -p "$binaries_dir"

copy_sidecar() {
  local source="$1"
  local target="$2"
  if [[ ! -f "$source" ]]; then
    echo "Missing sidecar source: $source" >&2
    exit 1
  fi
  cp -f "$source" "$target"
  chmod +x "$target"
  if [[ ! -s "$target" ]]; then
    echo "Copied sidecar is empty: $target" >&2
    exit 1
  fi
  echo "Prepared $target ($(stat -f%z "$target") bytes)"
}

copy_sidecar "$runtime_target/chawork-runtime" "$binaries_dir/chawork-runtime-$target_triple"
copy_sidecar "$runtime_target/codex" "$binaries_dir/codex-$target_triple"
copy_sidecar "$backend_target/chawork-mcp-server" "$binaries_dir/chawork-mcp-server-$target_triple"
