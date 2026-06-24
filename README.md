# ChaWork

[简体中文](README.zh-CN.md)

ChaWork is a local-first AI Employee Workbench for turning repeatable knowledge work into reviewable, reusable, and improvable working methods.

It is not a general chat app, not a new agent kernel, and not an official OpenAI product. ChaWork uses a ChaWork-maintained runtime facade based on the open-source OpenAI Codex project, while the ChaWork desktop app owns Workspace, Employee, Review, and Dream product state.

Official website: [https://chavoai.cn/](https://chavoai.cn/)

## Core Loop

```text
Open a Workspace
  -> bind an Employee
  -> complete a real task with the runtime
  -> review execution traces and file changes
  -> let Dream propose a method update from recent sessions
  -> approve or reject the update
  -> future sessions use the improved Employee prompt
```

## Architecture

ChaWork is split into four responsibility layers:

| Layer | Owns | Does not own |
| --- | --- | --- |
| Codex | thread/turn/item lifecycle, agent loop, tools, MCP, skills, approvals, sandbox, resume semantics | ChaWork Workspace, Employee, Dream product state |
| `chawork-runtime` | stable ChaWork runtime contract, capability matrix, event mapper, raw policy, audit, Dream execution contract | ChaWork persistence, UI state, Employee registry |
| ChaWork backend | workspace/session/employee/Dream state, runtime process pool, scoped `CODEX_HOME`, provider env injection, transcript/review persistence | Codex raw protocol parsing |
| Frontend | chat UI, workspace/session views, Runtime Inspector, Review Queue, Employee/Dream management | raw Codex payload driven business state |

ChaWork app code must consume the stable `chawork-runtime` contract. Raw Codex payloads may be shown in debug/inspector views, but they must not drive chat transcript, busy state, review queue, Dream state, or persistent product decisions.

## Repository Layout

- `backend/`: Rust backend, Tauri app, product services, and `chawork-mcp-server`.
- `frontend/`: Vite + React desktop UI.
- `src-tauri/`: symlink to `backend/` for Tauri CLI compatibility.
- `chawork-runtime/`: public Git submodule at [chaworkAI/chawork-runtime](https://github.com/chaworkAI/chawork-runtime).
The public product entry points are this README, [README.zh-CN.md](README.zh-CN.md), and [CONTRIBUTING.md](CONTRIBUTING.md). Internal design archives are intentionally not included in the cleaned public repository.

## Prerequisites

- Node.js 22+.
- pnpm 10.32+.
- Rust stable via `rustup`.
- Optional import tools: `pandoc`, `ffmpeg`, `whisper-cli`, `tesseract`.

On macOS, optional tools can be installed with:

```bash
brew install pandoc ffmpeg whisper-cpp tesseract tesseract-lang
```

## Setup

Clone with submodules:

```bash
git clone --recurse-submodules https://github.com/chaworkAI/chawork.git
cd chawork
```

If you already cloned without submodules:

```bash
git submodule update --init --recursive
```

Install frontend dependencies:

```bash
pnpm install
```

## Development

Build the runtime sidecars:

```bash
pnpm run build:runtime
```

Start the desktop app in development mode:

```bash
pnpm run tauri:dev
```

Common checks:

```bash
pnpm build
cargo check --manifest-path backend/Cargo.toml --bins --locked
cargo test --manifest-path backend/Cargo.toml --locked
cargo check --manifest-path chawork-runtime/codex-rs/Cargo.toml -p chawork-runtime
cargo test --manifest-path chawork-runtime/codex-rs/Cargo.toml -p chawork-runtime
```

## Runtime

ChaWork builds and bundles three sidecars for desktop use:

- `chawork-runtime/codex-rs/target/{debug,release}/chawork-runtime`
- `chawork-runtime/codex-rs/target/{debug,release}/codex`
- `backend/target/{debug,release}/chawork-mcp-server`

The desktop app uses repository-local sidecars. It does not search for a global `codex` on `PATH` and does not read or modify the user's global Codex configuration.

Current runtime provenance:

- ChaWork runtime submodule: [chaworkAI/chawork-runtime](https://github.com/chaworkAI/chawork-runtime).
- Vendored OpenAI Codex source commit recorded by the current runtime release unit: see `chawork-runtime/codex-rs/chawork-runtime/src/capability_matrix.rs` and the runtime README.
- ChaWork Runtime is independently maintained by ChaWork. It is based on the open-source OpenAI Codex project, but it is not an official OpenAI product.

## Provider Configuration and Data

ChaWork is local-first, but model requests are sent to the provider configured by the user.

- Users provide their own OpenAI-compatible base URL, model, and API key.
- Provider credentials are stored locally in the ChaWork root workspace provider configuration.
- Runtime children receive provider credentials through environment variables.
- Provider credentials must not be committed, written to workspace files, sent to frontend payloads, or included in ordinary logs/audit details.
- Workspaces, sessions, Employee prompts, Dream results, and logs are stored locally under the ChaWork root workspace and user-selected workspace folders.
- Dream only reads the target Employee prompt snapshot and selected recent session snapshots.

## Desktop Releases

The initial public-source baseline keeps release automation intentionally small. Desktop artifacts can be built from source with the commands above; signed release workflows may be restored in a later public change.

Current public-source builds are unsigned preview artifacts unless a release explicitly says otherwise. Unsigned builds may trigger macOS Gatekeeper or Windows SmartScreen warnings.

The open-source development configuration does not require a private `chawork-runtime` token. The runtime submodule is public.

ChaWork uses `https://api.chawork.com` as its official OTA update service. Update checks may include the current version, target, architecture, device id, and channel. Production updater signing must be configured before publishing signed public releases.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security Reports

Please do not report ChaWork security issues to OpenAI Bugcrowd. ChaWork is independently maintained.

For now, report security-sensitive issues through GitHub private vulnerability reporting when enabled on the repository, or contact the maintainers listed in [CONTRIBUTING.md](CONTRIBUTING.md).

## License and Attribution

ChaWork is licensed under the [Apache License 2.0](LICENSE).

ChaWork Runtime includes code derived from the open-source OpenAI Codex project and preserves the upstream license and notices in the runtime repository. See [NOTICE](NOTICE) for attribution.

## Acknowledgements

ChaWork's runtime foundation would not exist in its current form without OpenAI Codex. We keep Codex references inside the vendored runtime source, package metadata, tests, and protocol code where they describe upstream behavior or preserve compatibility. ChaWork is independently maintained, but it intentionally acknowledges and builds on the open-source work of the Codex project and its contributors.
