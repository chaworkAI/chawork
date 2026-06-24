#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_root="$repo_root/backend/target"
tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/chawork-macos-install-smoke.XXXXXX")"
mount_dir="$tmp_root/dmg"
install_dir="$tmp_root/Applications"
home_dir="$tmp_root/home"
root_dir="$tmp_root/ChaWork Root"
log_file="$tmp_root/chawork.log"
app_pid=""

cleanup() {
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  if mount | grep -F "on $mount_dir " >/dev/null 2>&1; then
    hdiutil detach "$mount_dir" -quiet || true
  fi
}
trap cleanup EXIT

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS install smoke must run on Darwin." >&2
  exit 1
fi

dmg_path="$(find "$target_root" -path '*/bundle/dmg/*.dmg' -type f -print -quit)"
if [[ -z "$dmg_path" ]]; then
  echo "No DMG artifact found under $target_root." >&2
  exit 1
fi

mkdir -p "$mount_dir" "$install_dir" "$home_dir" "$root_dir"
hdiutil attach "$dmg_path" -readonly -nobrowse -mountpoint "$mount_dir" -quiet

mounted_app="$(find "$mount_dir" -maxdepth 2 -name '*.app' -type d -print -quit)"
if [[ -z "$mounted_app" ]]; then
  echo "Mounted DMG does not contain an app bundle: $dmg_path" >&2
  find "$mount_dir" -maxdepth 2 -print >&2
  exit 1
fi

cp -R "$mounted_app" "$install_dir/"
hdiutil detach "$mount_dir" -quiet

installed_app="$install_dir/$(basename "$mounted_app")"
app_exe="$installed_app/Contents/MacOS/ChaWork"
if [[ ! -x "$app_exe" ]]; then
  echo "Installed app executable missing: $app_exe" >&2
  exit 1
fi

for sidecar in chawork-runtime codex chawork-mcp-server; do
  if ! find "$installed_app" -type f -name "$sidecar" -perm -111 -print -quit | grep -q .; then
    echo "Installed app is missing executable sidecar: $sidecar" >&2
    find "$installed_app" -type f | sed -n '1,160p' >&2
    exit 1
  fi
done

HOME="$home_dir" CHAWORK_ROOT_DIR="$root_dir" "$app_exe" >"$log_file" 2>&1 &
app_pid="$!"
sleep 8

if ! kill -0 "$app_pid" 2>/dev/null; then
  echo "Installed ChaWork app exited during startup." >&2
  sed -n '1,200p' "$log_file" >&2 || true
  exit 1
fi

kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

for path in \
  "$root_dir/.chawork-root" \
  "$root_dir/runtime" \
  "$root_dir/employees" \
  "$root_dir/skills" \
  "$root_dir/mcp" \
  "$root_dir/logs"; do
  if [[ ! -e "$path" ]]; then
    echo "Root workspace path missing after installed app startup: $path" >&2
    sed -n '1,200p' "$log_file" >&2 || true
    exit 1
  fi
done

app_data_dir="$home_dir/Library/Application Support/com.chawork.app"
if [[ ! -d "$app_data_dir" ]]; then
  echo "App data directory was not created: $app_data_dir" >&2
  exit 1
fi

rm -rf "$installed_app" "$root_dir" "$app_data_dir"

for path in "$installed_app" "$root_dir" "$app_data_dir"; do
  if [[ -e "$path" ]]; then
    echo "Manual uninstall cleanup left path behind: $path" >&2
    exit 1
  fi
done

echo "macOS DMG install, startup, and manual uninstall smoke checks passed."
