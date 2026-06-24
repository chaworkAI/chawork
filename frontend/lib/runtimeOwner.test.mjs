import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"
import ts from "typescript"

async function importTs(path) {
  const source = await readFile(new URL(path, import.meta.url), "utf8")
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2021,
      verbatimModuleSyntax: true,
    },
  })
  return import(`data:text/javascript,${encodeURIComponent(outputText)}`)
}

test("runtime owner uses event workspace/session when present", async () => {
  const { resolveRuntimeOwner } = await importTs("./runtimeOwner.ts")

  assert.deepEqual(
    resolveRuntimeOwner(
      { type: "thinking", workspace_id: "workspace-a", session_id: "session-1" },
      { activeWorkspaceId: "workspace-b", activeSessionId: "session-2" },
    ),
    {
      workspaceId: "workspace-a",
      sessionId: "session-1",
      legacyOwner: false,
    },
  )
})

test("runtime owner maps legacy events to active workspace only", async () => {
  const { resolveRuntimeOwner } = await importTs("./runtimeOwner.ts")

  assert.deepEqual(
    resolveRuntimeOwner(
      { type: "thinking" },
      { activeWorkspaceId: "workspace-active", activeSessionId: "session-active" },
    ),
    {
      workspaceId: "workspace-active",
      sessionId: "session-active",
      legacyOwner: true,
    },
  )
})

test("chat view accepts only the active workspace and active session", async () => {
  const { eventMatchesActiveView } = await importTs("./runtimeOwner.ts")

  assert.equal(
    eventMatchesActiveView(
      { workspaceId: "workspace-a", sessionId: "session-1", legacyOwner: false },
      { activeWorkspaceId: "workspace-a", activeSessionId: "session-1" },
    ),
    true,
  )
  assert.equal(
    eventMatchesActiveView(
      { workspaceId: "workspace-a", sessionId: "session-2", legacyOwner: false },
      { activeWorkspaceId: "workspace-a", activeSessionId: "session-1" },
    ),
    false,
  )
  assert.equal(
    eventMatchesActiveView(
      { workspaceId: "workspace-b", sessionId: "session-1", legacyOwner: false },
      { activeWorkspaceId: "workspace-a", activeSessionId: "session-1" },
    ),
    false,
  )
})
