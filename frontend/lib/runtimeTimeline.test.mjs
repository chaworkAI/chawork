import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"

async function readProjectFile(path) {
  return readFile(new URL(`../../${path}`, import.meta.url), "utf8")
}

test("runtime timeline groups tool and file lifecycles by stable ids", async () => {
  const timeline = await readProjectFile("frontend/lib/runtimeTimeline.ts")

  assert.match(timeline, /case "tool_call"/)
  assert.match(timeline, /case "tool_delta"/)
  assert.match(timeline, /case "tool_complete"/)
  assert.match(timeline, /case "tool_result"/)
  assert.match(timeline, /ensureTool\(event, ev\.id/)
  assert.match(timeline, /item\.output \+= ev\.content/)

  assert.match(timeline, /case "file_change"/)
  assert.match(timeline, /case "file_change_delta"/)
  assert.match(timeline, /ensureFile\(event, fileKey\(event\)\)/)
  assert.match(timeline, /item\.status = ev\.status/)
})

test("runtime timeline mcp detection includes progress placeholder tool", async () => {
  const timeline = await readProjectFile("frontend/lib/runtimeTimeline.ts")

  assert.match(timeline, /tool === "mcp"/)
  assert.match(timeline, /tool\.startsWith\("mcp:"\)/)
  assert.match(timeline, /tool\.startsWith\("mcp__"\)/)
})

test("runtime inspector renders timeline items instead of flat tool events", async () => {
  const inspector = await readProjectFile("frontend/components/runtime/RuntimeInspector.tsx")

  assert.match(inspector, /buildRuntimeTimeline\(events\)/)
  assert.match(inspector, /ToolTimelineCard/)
  assert.match(inspector, /FileTimelineCard/)
  assert.match(inspector, /defaultOpen=\{i === filtered\.length - 1\}/)
})
