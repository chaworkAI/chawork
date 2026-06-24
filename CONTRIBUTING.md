# Contributing to ChaWork

Thank you for helping improve ChaWork. This project has a strict product/runtime boundary, so contribution quality depends as much on preserving ownership rules as on code changes.

## Language

The default public documentation language is English.

- `README.md` is the default English product entry.
- `README.zh-CN.md` is the Simplified Chinese mirror and must stay structurally aligned with `README.md`.
- `CONTRIBUTING.md` is English by default. Short Chinese notes may be added in this file when useful, but do not create `CONTRIBUTING.zh-CN.md`.

## Development Setup

```bash
git clone --recurse-submodules https://github.com/chaworkAI/chawork.git
cd chawork
pnpm install
pnpm run build:runtime
pnpm run tauri:dev
```

If the runtime submodule is missing:

```bash
git submodule update --init --recursive
```

## Contributor Development Guide

Before coding, identify the layer that owns the behavior you want to change:

- Frontend changes live under `frontend/` and should call Tauri commands instead of talking to model providers directly.
- Backend product-state changes live under `backend/src/commands/` and `backend/src/services/`.
- Runtime protocol changes live in the `chawork-runtime` submodule and must preserve the stable ChaWork runtime contract.
- Workspace, Employee, and Dream behavior must keep a single persistence owner. Do not duplicate state updates across frontend, backend, and runtime.

Recommended workflow:

```bash
git submodule update --init --recursive
pnpm install
pnpm build
cargo check --manifest-path backend/Cargo.toml --bins --locked
```

For frontend work:

- Keep UI state in the existing stores under `frontend/stores/`.
- Keep IPC names in `frontend/lib/ipcConstants.ts`.
- Use existing runtime event mapping helpers instead of parsing raw Codex payloads in components.
- Run `pnpm build` before opening a PR.

For backend work:

- Put Tauri command wiring in `backend/src/commands/`.
- Put persistence and side effects in the owning service under `backend/src/services/`.
- Keep runtime transport details inside `backend/src/runtime/`.
- Run the focused backend test for the changed service, then `cargo test --manifest-path backend/Cargo.toml --locked` when the change crosses service boundaries.

For runtime submodule work:

- Make the change inside `chawork-runtime/`.
- Commit or otherwise publish the runtime change first.
- Update the root repository submodule pointer in the same PR when ChaWork needs the new runtime behavior.
- Mention both the runtime commit and the root submodule pointer update in the PR description.

Local workspace data should not be committed. The public repository ignores generated workspace folders such as `schema/`, `sessions/`, `wiki/`, `raw/`, `templates/`, `proposals/`, and `.chawork/`.

## Common Checks

Run the smallest check set that covers your change.

Frontend:

```bash
pnpm build
```

Backend:

```bash
cargo check --manifest-path backend/Cargo.toml --bins --locked
cargo test --manifest-path backend/Cargo.toml --locked
```

Runtime:

```bash
cargo check --manifest-path chawork-runtime/codex-rs/Cargo.toml -p chawork-runtime
cargo test --manifest-path chawork-runtime/codex-rs/Cargo.toml -p chawork-runtime
```

## Architecture Rules

ChaWork app code must consume the stable `chawork-runtime` contract. It must not parse Codex raw app-server protocol as product state.

Keep these invariants intact:

- Codex owns thread/turn/tool/MCP/skill/approval/sandbox execution semantics.
- `chawork-runtime` owns the ChaWork-facing runtime contract, capability matrix, event mapper, raw policy, audit, and Dream runtime prompt/schema execution.
- ChaWork backend owns Workspace, Session, Employee, Dream persistence, runtime process pool, provider env injection, and transcript/review persistence.
- Frontend owns product UI projection only.
- Raw/debug events can be inspected, but must not drive transcript, busy state, review queue, Dream state, or persistent product decisions.
- Provider credentials must not be committed, logged, written into workspace files, or sent to frontend payloads.

## Runtime Changes

Runtime-facing changes need extra care:

- Public schema changes start in `chawork-runtime/codex-rs/chawork-runtime/src/contract.rs`.
- Codex protocol surface changes must be registered in the capability matrix.
- Request fields must be mapped to Codex or rejected by validation. Silent ignore is a bug.
- Unknown Codex notifications must be classified as normalized, raw, unsupported, or drop-with-reason.
- Unknown Codex ServerRequest variants must be classified and audited.
- Raw request behavior must go through `raw_policy.rs`.
- Dream prompt/schema stay inside `chawork-runtime`; final Employee prompt writes stay in the ChaWork Employee service.

## Pull Requests

Each PR should include:

- What changed and why.
- Which layer owns the change: frontend, backend, runtime facade, Codex upstream area, Employee, Workspace, Dream, packaging, or docs.
- Tests or checks run.
- Screenshots or recordings for visible UI changes.
- Runtime boundary notes when touching contract, mapper, raw policy, audit, lifecycle, provider/security policy, Dream, or session persistence.

Do not mix unrelated cleanup with feature or bug-fix changes.

## Documentation

Public product documentation lives in:

- `README.md`
- `README.zh-CN.md`
- `CONTRIBUTING.md`

When changing the public product README, update both English and Chinese versions in the same PR. Keep their section structure aligned.

The cleaned public repository intentionally does not include the internal design archive. Public onboarding should be possible from the two README files and this CONTRIBUTING file.

## Security Reports

Do not report ChaWork issues to OpenAI Bugcrowd. ChaWork is independently maintained.

Use GitHub private vulnerability reporting when it is enabled on the repository. If it is not enabled, contact the maintainers through the private channel listed by the repository owner.

Please do not open public issues for active vulnerabilities, leaked credentials, or exploitable security problems.

## Maintainers

The public repositories are maintained under the `chaworkAI` GitHub organization:

- [chaworkAI/chawork](https://github.com/chaworkAI/chawork)
- [chaworkAI/chawork-runtime](https://github.com/chaworkAI/chawork-runtime)

Initial admin maintainers must be members of the `@chaworkAI/maintainers` team or explicitly listed in `.github/CODEOWNERS`.

Initial maintainer invitation emails:

- 1104133609@qq.com
- 970795248@qq.com

Default branch protection should require:

- Pull request review before merge.
- At least one approval.
- CODEOWNERS review.
- Required CI checks.
- No force-push or branch deletion on the default branch.
