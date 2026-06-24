import assert from "node:assert/strict"
import test from "node:test"

import {
  formatSummaryDetail,
  formatSummaryLabel,
  humanizeToolName,
  isSummaryRelevant,
  pickSummaryEvents,
} from "./runtimeSummary.ts"

function event(type, extra = {}) {
  return {
    id: crypto.randomUUID(),
    timestamp: new Date().toISOString(),
    displayLabel: '{"noise":true}',
    displayStatus: "info",
    event: { type, ...extra },
  }
}

test("humanizeToolName maps common tools", () => {
  assert.equal(humanizeToolName("search"), "搜索")
  assert.equal(humanizeToolName("mcp__chawork__grep"), "搜索")
})

test("isSummaryRelevant uses action whitelist", () => {
  assert.equal(isSummaryRelevant(event("tool_call", { tool: "search", args: {}, id: "1" })), true)
  assert.equal(isSummaryRelevant(event("thinking", { summary: "x" })), false)
  assert.equal(isSummaryRelevant(event("runtime_debug", { method: "x", category: "raw", params: {} })), false)
  assert.equal(isSummaryRelevant(event("tool_result", { id: "1", result: {} })), false)
})

test("formatSummaryLabel never surfaces raw JSON display labels", () => {
  const label = formatSummaryLabel(
    event("tool_call", { tool: "search", args: { query: "foo" }, id: "1" }),
  )
  assert.equal(label, "调用 · 搜索")
})

test("pickSummaryEvents keeps recent actions only", () => {
  const events = [
    event("thinking", { summary: "plan" }),
    event("tool_delta", { tool: "search", content: "{}", id: "1" }),
    event("tool_call", { tool: "search", args: { query: "alpha" }, id: "2" }),
    event("file_change", { path: "/tmp/a.md", diff: "", action: "modify" }),
  ]
  const picked = pickSummaryEvents(events, 3)
  assert.equal(picked.length, 2)
  assert.equal(picked[0].event.type, "file_change")
  assert.equal(picked[1].event.type, "tool_call")
})

test("formatSummaryDetail extracts tool args", () => {
  const detail = formatSummaryDetail(
    event("tool_call", { tool: "grep", args: { pattern: "TODO" }, id: "1" }),
  )
  assert.equal(detail, "TODO")
})
