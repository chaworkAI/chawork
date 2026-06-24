#!/usr/bin/env bash
# 初始化 `chawork-runtime` Git 子模块并提示如何构建 ChaWork runtime 二进制。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> 拉取子模块 chawork-runtime（https://github.com/chaworkAI/chawork-runtime）…"
git submodule update --init --recursive

CODEX_RS="$ROOT/chawork-runtime/codex-rs"
RELEASE_BIN="$CODEX_RS/target/release/codex"
DEBUG_BIN="$CODEX_RS/target/debug/codex"

if [[ ! -d "$CODEX_RS" ]]; then
  echo "错误: 未找到 $CODEX_RS（子模块可能未配置成功）。" >&2
  exit 1
fi

echo ""
echo "子模块就绪: $CODEX_RS"
echo ""
echo "从源码构建 ChaWork runtime CLI（debug，供本地开发使用）："
echo "  pnpm run build:chawork-runtime"
echo ""
echo "也可手动构建 release 产物："
echo "  cd \"$CODEX_RS\""
echo "  cargo build --release -p codex-cli --bin codex"
echo ""
echo "默认情况下，ChaWork 只查找仓库内 chawork-runtime/codex-rs/target/{release,debug}/codex。"
echo ""
if [[ -f "$RELEASE_BIN" ]]; then
  echo "检测到已有 release 二进制: $RELEASE_BIN"
elif [[ -f "$DEBUG_BIN" ]]; then
  echo "检测到仅有 debug 二进制: $DEBUG_BIN"
fi
echo ""
echo "ChaWork 不会搜索 PATH 上的全局 codex，也不会使用根目录 codex/ checkout。"
echo "必须先构建仓库内 chawork-runtime 产物。"
echo "更完整的安装说明见: README.md / README.zh-CN.md 和子模块 README。"
