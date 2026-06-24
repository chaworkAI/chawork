import type { CodexEvent, RuntimeEvent } from "@/types/events"

/** Event types worth a single line in the compact execution summary rail. */
const SUMMARY_ACTION_TYPES = new Set<CodexEvent["type"]>([
  "tool_call",
  "file_change",
  "retrieval",
  "error",
  "skill_write",
  "plan_update",
])

const FILE_ACTION_LABEL: Record<string, string> = {
  create: "新建",
  modify: "修改",
  delete: "删除",
}

const TOOL_NAME_LABEL: Record<string, string> = {
  search: "搜索",
  search_knowledge: "知识检索",
  grep: "搜索",
  ripgrep: "搜索",
  read: "读取",
  read_file: "读取",
  write: "写入",
  write_file: "写入",
  edit: "编辑",
  apply_patch: "编辑",
  bash: "命令",
  shell: "命令",
  exec: "命令",
  list_dir: "列目录",
  glob: "匹配文件",
}

function basename(path: string): string {
  const normalized = path.replace(/\\/g, "/")
  const parts = normalized.split("/").filter(Boolean)
  return parts[parts.length - 1] ?? path
}

function truncate(text: string, max: number): string {
  const trimmed = text.trim()
  if (trimmed.length <= max) return trimmed
  return `${trimmed.slice(0, max)}…`
}

/** Turn raw tool ids (mcp__foo__search) into a short Chinese-friendly label. */
export function humanizeToolName(tool: string): string {
  const normalized = tool.trim()
  if (!normalized) return "工具"

  const segments = normalized.split(/[/.__:-]+/).filter(Boolean)
  const candidate = (segments[segments.length - 1] ?? normalized).toLowerCase()
  return TOOL_NAME_LABEL[candidate] ?? segments[segments.length - 1] ?? normalized
}

/** Whether an event should appear in the compact execution summary rail. */
export function isSummaryRelevant(event: RuntimeEvent): boolean {
  return SUMMARY_ACTION_TYPES.has(event.event.type)
}

/** Pick the most recent user-meaningful events for the summary panel. */
export function pickSummaryEvents(events: RuntimeEvent[], limit = 3): RuntimeEvent[] {
  const seen = new Set<string>()
  const picked: RuntimeEvent[] = []

  for (let i = events.length - 1; i >= 0 && picked.length < limit; i--) {
    const event = events[i]
    if (!isSummaryRelevant(event)) continue
    const key = summaryDedupeKey(event)
    if (seen.has(key)) continue
    seen.add(key)
    picked.push(event)
  }

  return picked
}

function summaryDedupeKey(event: RuntimeEvent): string {
  const ev = event.event
  if (ev.type === "tool_call") {
    return `tool_call:${ev.tool}`
  }
  if (ev.type === "file_change") {
    return `${ev.type}:${ev.path}:${ev.action}`
  }
  if (ev.type === "retrieval") {
    return `retrieval:${ev.query}`
  }
  if (ev.type === "plan_update") {
    return `plan:${ev.explanation ?? "update"}`
  }
  return `${ev.type}:${formatSummaryLabel(event)}`
}

function readStringField(value: unknown, ...keys: string[]): string | undefined {
  if (!value || typeof value !== "object") return undefined
  const record = value as Record<string, unknown>
  for (const key of keys) {
    const field = record[key]
    if (typeof field === "string" && field.trim()) return field.trim()
  }
  return undefined
}

function looksLikeJson(text: string): boolean {
  const trimmed = text.trim()
  return trimmed.startsWith("{") || trimmed.startsWith("[")
}

/** Primary one-line label for summary cards (never raw JSON). */
export function formatSummaryLabel(event: RuntimeEvent): string {
  const ev = event.event

  switch (ev.type) {
    case "tool_call":
      return `调用 · ${humanizeToolName(ev.tool)}`
    case "file_change": {
      const action = FILE_ACTION_LABEL[ev.action] ?? "变更"
      return `${action} · ${basename(ev.path)}`
    }
    case "retrieval":
      return `检索 · ${truncate(ev.query, 36)}`
    case "error":
      return truncate(ev.message || "出现错误", 48)
    case "skill_write":
      return `Skill · ${basename(ev.target_path)}`
    case "plan_update":
      return ev.explanation
        ? `计划 · ${truncate(ev.explanation, 36)}`
        : "更新计划"
    default:
      if (looksLikeJson(event.displayLabel)) return "执行中"
      return event.displayLabel
  }
}

/** Short secondary line for summary cards (never raw JSON blobs). */
export function formatSummaryDetail(event: RuntimeEvent): string | undefined {
  const ev = event.event

  if (ev.type === "tool_call") {
    const args =
      ev.args && typeof ev.args === "object"
        ? (ev.args as Record<string, unknown>)
        : undefined
    const query = readStringField(
      args,
      "query",
      "search_term",
      "pattern",
      "path",
      "command",
      "glob_pattern",
    )
    if (query) return truncate(query, 72)
    return undefined
  }

  if (ev.type === "retrieval") {
    if (ev.results.length === 0) return undefined
    return `${ev.results.length} 条结果`
  }

  if (ev.type === "file_change") {
    return truncate(ev.path, 72)
  }

  if (ev.type === "skill_write") {
    return ev.message ? truncate(ev.message, 72) : undefined
  }

  if (ev.type === "plan_update" && ev.steps.length > 0) {
    const active = ev.steps.find((s) => s.status === "inProgress") ?? ev.steps[0]
    return active?.step ? truncate(active.step, 72) : undefined
  }

  if (event.detail && !looksLikeJson(event.detail)) {
    return truncate(event.detail, 72)
  }

  return undefined
}
