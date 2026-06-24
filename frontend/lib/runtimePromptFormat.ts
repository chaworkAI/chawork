import type { ReviewPanelEntry, UserInputQuestion } from "@/types/events"

export type RuntimePromptKind = "permissions" | "approval" | "user_input" | "mcp"

export function getRuntimePromptKind(entry: ReviewPanelEntry): RuntimePromptKind {
  if (entry.runtime_permissions) return "permissions"
  if (entry.user_input_request) return "user_input"
  if (entry.mcp_elicitation) return "mcp"
  return "approval"
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null
  return value as Record<string, unknown>
}

/** Skip empty `{}` / `[]` JSON blocks in the UI. */
export function formatJsonForDisplay(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (typeof value === "string") {
    const trimmed = value.trim()
    if (!trimmed || trimmed === "{}" || trimmed === "[]") return null
    return trimmed
  }
  try {
    const text = JSON.stringify(value, null, 2)
    if (!text || text === "{}" || text === "[]") return null
    return text
  } catch {
    return String(value)
  }
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined
}

/** Human-readable lines for permissions escalation requests. */
export function formatPermissionsSummary(permissions: unknown): string[] {
  const record = asRecord(permissions)
  if (!record) return []

  const lines: string[] = []
  const push = (label: string, value: unknown) => {
    if (value === null || value === undefined) return
    if (typeof value === "string" && !value.trim()) return
    if (Array.isArray(value) && value.length === 0) return
    if (typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === 0) {
      return
    }
    if (typeof value === "boolean") {
      lines.push(`${label}：${value ? "是" : "否"}`)
      return
    }
    if (Array.isArray(value)) {
      lines.push(`${label}：${value.map(String).join("、")}`)
      return
    }
    if (typeof value === "object") {
      const nested = formatJsonForDisplay(value)
      if (nested) lines.push(`${label}：${nested}`)
      return
    }
    lines.push(`${label}：${String(value)}`)
  }

  push("沙箱模式", record.sandboxMode ?? record.sandbox_mode)
  push("网络访问", record.networkAccess ?? record.network_access)
  push("可写目录", record.writableRoots ?? record.writable_roots)
  push("可读目录", record.readableRoots ?? record.readable_roots)
  push("执行权限", record.execPolicy ?? record.exec_policy)
  push("审批策略", record.approvalPolicy ?? record.approval_policy)

  if (lines.length === 0) {
    for (const [key, value] of Object.entries(record)) {
      push(key, value)
    }
  }

  return lines
}

/** Detail text for command / file-change approvals. */
export function formatApprovalDetail(entry: ReviewPanelEntry): {
  label: string
  body: string
} | null {
  const params = entry.runtime_approval?.params ?? entry.runtime_permissions?.params
  const record = asRecord(params)
  if (!record) return null

  const command =
    readString(record.command) ??
    readString(record.cmd) ??
    readString(record.executable)
  if (command) {
    const cwd = readString(record.cwd) ?? readString(record.workingDirectory)
    const body = cwd ? `$ ${command}\n\ncwd: ${cwd}` : `$ ${command}`
    return { label: "命令", body }
  }

  const path = readString(record.path) ?? readString(record.filePath)
  const diff = readString(record.diff)
  if (path || diff) {
    const parts = [path ? `路径：${path}` : "", diff ?? ""].filter(Boolean)
    return { label: path ? "文件变更" : "差异", body: parts.join("\n\n") }
  }

  const fallback = formatJsonForDisplay(params)
  return fallback ? { label: "详情", body: fallback } : null
}

export function normalizeUserInputQuestions(raw: unknown): UserInputQuestion[] {
  if (!Array.isArray(raw)) return []
  const out: UserInputQuestion[] = []
  for (const item of raw) {
    const row = asRecord(item)
    if (!row) continue
    const id = readString(row.id)
    const question = readString(row.question)
    if (!id || !question) continue
    const header =
      readString(row.header) ?? readString(row.label) ?? question
    const options = Array.isArray(row.options)
      ? row.options
          .map((option) => {
            const opt = asRecord(option)
            if (!opt) return null
            const label = readString(opt.label)
            if (!label) return null
            return {
              label,
              description: readString(opt.description) ?? "",
            }
          })
          .filter((option): option is NonNullable<typeof option> => option !== null)
      : null
    out.push({
      id,
      header,
      question,
      isOther: row.isOther === true,
      isSecret: row.isSecret === true,
      options: options && options.length > 0 ? options : null,
    })
  }
  return out
}

export function initialMcpFormContent(params: unknown): string {
  const record = asRecord(params)
  const schema = record?.schema ?? record?.requestedSchema
  const formatted = formatJsonForDisplay(schema)
  return formatted ?? ""
}
