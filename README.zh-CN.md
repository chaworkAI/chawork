# ChaWork

[English](README.md)

ChaWork 是一个本地优先的 AI Employee Workbench，用来把反复发生的知识工作沉淀成可审查、可复用、可持续改进的方法。

它不是通用聊天应用，不是新的 agent 内核，也不是 OpenAI 官方产品。ChaWork 使用基于 OpenAI 开源 Codex 项目的 ChaWork Runtime facade；ChaWork 桌面应用负责 Workspace、Employee、Review、Dream 等产品状态。

项目官网：[https://chavoai.cn/](https://chavoai.cn/)

## 核心闭环

```text
打开 Workspace
  -> 绑定 Employee
  -> 使用 runtime 完成真实任务
  -> 审查执行痕迹和文件变化
  -> Dream 从最近 sessions 中提出方法更新
  -> 用户批准或拒绝
  -> 后续 session 使用更新后的 Employee prompt
```

## 架构

ChaWork 分为四层职责：

| 层 | 拥有 | 不拥有 |
| --- | --- | --- |
| Codex | thread/turn/item 生命周期、agent loop、tools、MCP、skills、approval、sandbox、resume 语义 | ChaWork Workspace、Employee、Dream 产品状态 |
| `chawork-runtime` | 稳定 ChaWork runtime contract、capability matrix、event mapper、raw policy、audit、Dream 执行 contract | ChaWork 持久化、UI 状态、Employee registry |
| ChaWork backend | workspace/session/employee/Dream 状态、runtime process pool、scoped `CODEX_HOME`、provider env 注入、transcript/review 持久化 | Codex raw protocol 解析 |
| Frontend | Chat UI、workspace/session 展示、Runtime Inspector、Review Queue、Employee/Dream 管理 | raw Codex payload 驱动业务状态 |

ChaWork app 只能消费稳定的 `chawork-runtime` contract。Raw Codex payload 可以进入 debug/inspector，但不得驱动 chat transcript、busy state、review queue、Dream state 或持久化产品决策。

## 仓库结构

- `backend/`：Rust backend、Tauri app、产品服务、`chawork-mcp-server`。
- `frontend/`：Vite + React 桌面 UI。
- `src-tauri/`：指向 `backend/` 的符号链接，满足 Tauri CLI 约定。
- `chawork-runtime/`：公开 Git 子模块：[chaworkAI/chawork-runtime](https://github.com/chaworkAI/chawork-runtime)。
公开产品入口是 [README.md](README.md)、本文档和 [CONTRIBUTING.md](CONTRIBUTING.md)。清理后的公开仓库不包含内部设计档案。

## 环境准备

- Node.js 22+。
- pnpm 10.32+。
- Rust stable。
- 可选导入工具：`pandoc`、`ffmpeg`、`whisper-cli`、`tesseract`。

macOS 可选工具安装：

```bash
brew install pandoc ffmpeg whisper-cpp tesseract tesseract-lang
```

## 初始化

带子模块 clone：

```bash
git clone --recurse-submodules https://github.com/chaworkAI/chawork.git
cd chawork
```

如果 clone 时没有拉子模块：

```bash
git submodule update --init --recursive
```

安装前端依赖：

```bash
pnpm install
```

## 开发

构建 runtime sidecars：

```bash
pnpm run build:runtime
```

启动桌面开发模式：

```bash
pnpm run tauri:dev
```

常用检查：

```bash
pnpm build
cargo check --manifest-path backend/Cargo.toml --bins --locked
cargo test --manifest-path backend/Cargo.toml --locked
cargo check --manifest-path chawork-runtime/codex-rs/Cargo.toml -p chawork-runtime
cargo test --manifest-path chawork-runtime/codex-rs/Cargo.toml -p chawork-runtime
```

## Runtime

ChaWork 桌面应用会构建并打包三个 sidecar：

- `chawork-runtime/codex-rs/target/{debug,release}/chawork-runtime`
- `chawork-runtime/codex-rs/target/{debug,release}/codex`
- `backend/target/{debug,release}/chawork-mcp-server`

桌面应用只使用仓库内 sidecar。它不会搜索 PATH 上的全局 `codex`，也不会读取或修改用户全局 Codex 配置。

当前 runtime 来源：

- ChaWork runtime 子模块：[chaworkAI/chawork-runtime](https://github.com/chaworkAI/chawork-runtime)。
- 当前 runtime release unit 记录的 OpenAI Codex source commit：见 `chawork-runtime/codex-rs/chawork-runtime/src/capability_matrix.rs` 和 runtime README。
- ChaWork Runtime 由 ChaWork 独立维护，基于 OpenAI 开源 Codex 项目，但不是 OpenAI 官方产品。

## Provider 配置与数据

ChaWork 本地优先，但模型请求会发送到用户配置的 provider。

- 用户自行提供 OpenAI-compatible base URL、model 和 API key。
- Provider credential 保存在本地 ChaWork root workspace 的 provider 配置中。
- Runtime child 通过环境变量接收 provider credential。
- Provider credential 不得提交到仓库、写入 workspace 文件、发送到 frontend payload，或进入普通日志/audit detail。
- Workspace、session、Employee prompt、Dream result 和 logs 保存在本地 ChaWork root workspace 与用户选择的 workspace 中。
- Dream 只读取目标 Employee prompt snapshot 和选中的最近 session snapshots。

## 桌面发布

公开首版有意保持 release automation 精简。桌面产物可以使用上面的命令从源码构建；签名 release workflow 可在后续公开变更中恢复。

当前公开源码构建默认为 unsigned preview artifacts，除非 release 明确说明已签名。Unsigned build 可能触发 macOS Gatekeeper 或 Windows SmartScreen 提示。

开源开发配置不需要私有 `chawork-runtime` token。runtime 子模块是公开仓库。

ChaWork 使用 `https://api.chawork.com` 作为官方 OTA 更新服务。更新检查可能包含当前版本、target、architecture、device id 和 channel。发布 signed public release 前必须配置 production updater signing。

## 贡献

见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 安全报告

请不要把 ChaWork 安全问题报告给 OpenAI Bugcrowd。ChaWork 是独立维护项目。

当前请通过 GitHub private vulnerability reporting（如果仓库已启用）或 [CONTRIBUTING.md](CONTRIBUTING.md) 中列出的维护者联系方式报告安全敏感问题。

## 许可证与归属

ChaWork 使用 [Apache License 2.0](LICENSE)。

ChaWork Runtime 包含来自 OpenAI 开源 Codex 项目的代码，并在 runtime 仓库中保留上游 license 和 notice。归属信息见 [NOTICE](NOTICE)。

## 致谢

如果没有 OpenAI Codex，ChaWork 当前的 runtime foundation 不会以现在的方式存在。我们会在 vendored runtime source、package metadata、tests 和 protocol code 中保留用于说明上游行为或维持兼容性的 Codex 引用。ChaWork 是独立维护项目，但明确致谢并基于 Codex 项目及其贡献者的开源工作继续构建。
