import type { RuntimeEvent } from "@/types/events"

export interface ToolTimelineItem {
  kind: "tool"
  id: string
  timestamp: string
  tool: string
  args?: unknown
  output: string
  result?: unknown
  error?: string
  status: "running" | "completed" | "failed"
  displayStatus: RuntimeEvent["displayStatus"]
}

export interface FileTimelineItem {
  kind: "file"
  id: string
  timestamp: string
  path: string
  action: "create" | "modify" | "delete"
  diff: string
  output: string
  status?: string
  displayStatus: RuntimeEvent["displayStatus"]
}

export type RuntimeTimelineItem =
  | { kind: "event"; event: RuntimeEvent }
  | ToolTimelineItem
  | FileTimelineItem

interface IndexedItem {
  firstIndex: number
  item: RuntimeTimelineItem
}

function toolStatus(error?: string, completed?: boolean): ToolTimelineItem["status"] {
  if (error) return "failed"
  return completed ? "completed" : "running"
}

function fileDisplayStatus(status?: string): RuntimeEvent["displayStatus"] {
  if (status === "failed" || status === "declined") return "error"
  if (status === "completed") return "success"
  return "info"
}

function fileKey(event: RuntimeEvent): string {
  const ev = event.event
  if (ev.type === "file_change") {
    return ev.id?.trim() || `${ev.path}:${ev.action}`
  }
  if (ev.type === "file_change_delta") {
    return ev.id.trim() || event.id
  }
  return event.id
}

export function isMcpTool(tool?: string): boolean {
  return !!tool && (tool === "mcp" || tool.startsWith("mcp:") || tool.startsWith("mcp__"))
}

export function buildRuntimeTimeline(events: RuntimeEvent[]): RuntimeTimelineItem[] {
  const items: IndexedItem[] = []
  const toolItems = new Map<string, IndexedItem & { item: ToolTimelineItem }>()
  const fileItems = new Map<string, IndexedItem & { item: FileTimelineItem }>()

  const ensureTool = (
    event: RuntimeEvent,
    id: string,
    tool: string,
  ): IndexedItem & { item: ToolTimelineItem } => {
    const existing = toolItems.get(id)
    if (existing) {
      if (!existing.item.tool || existing.item.tool === "tool") {
        existing.item.tool = tool
      }
      return existing
    }
    const created: IndexedItem & { item: ToolTimelineItem } = {
      firstIndex: items.length,
      item: {
        kind: "tool",
        id,
        timestamp: event.timestamp,
        tool,
        output: "",
        status: "running",
        displayStatus: "warning",
      },
    }
    toolItems.set(id, created)
    items.push(created)
    return created
  }

  const ensureFile = (
    event: RuntimeEvent,
    key: string,
  ): IndexedItem & { item: FileTimelineItem } => {
    const existing = fileItems.get(key)
    if (existing) return existing
    const ev = event.event
    const created: IndexedItem & { item: FileTimelineItem } = {
      firstIndex: items.length,
      item: {
        kind: "file",
        id: key,
        timestamp: event.timestamp,
        path: ev.type === "file_change" ? ev.path : key,
        action: ev.type === "file_change" ? ev.action : "modify",
        diff: ev.type === "file_change" ? ev.diff : "",
        output: "",
        status: ev.type === "file_change" ? ev.status : undefined,
        displayStatus: ev.type === "file_change" ? fileDisplayStatus(ev.status) : "info",
      },
    }
    fileItems.set(key, created)
    items.push(created)
    return created
  }

  events.forEach((event) => {
    const ev = event.event
    switch (ev.type) {
      case "tool_call": {
        const item = ensureTool(event, ev.id, ev.tool).item
        item.args = ev.args
        item.displayStatus = "warning"
        break
      }
      case "tool_delta": {
        const item = ensureTool(event, ev.id, ev.tool).item
        item.output += ev.content
        item.displayStatus = item.status === "running" ? "info" : item.displayStatus
        break
      }
      case "tool_complete": {
        const item = ensureTool(event, ev.id, ev.tool).item
        item.status = toolStatus(undefined, true)
        item.displayStatus = "success"
        break
      }
      case "tool_result": {
        const item = ensureTool(event, ev.id, ev.tool ?? "tool").item
        item.result = ev.result
        item.error = ev.error
        item.status = toolStatus(ev.error, true)
        item.displayStatus = ev.error ? "error" : "success"
        break
      }
      case "file_change": {
        const item = ensureFile(event, fileKey(event)).item
        item.path = ev.path
        item.action = ev.action
        item.diff = ev.diff
        item.status = ev.status
        item.displayStatus = fileDisplayStatus(ev.status)
        break
      }
      case "file_change_delta": {
        const item = ensureFile(event, fileKey(event)).item
        item.output += ev.content
        break
      }
      default:
        items.push({
          firstIndex: items.length,
          item: { kind: "event", event },
        })
        break
    }
  })

  return items.sort((a, b) => a.firstIndex - b.firstIndex).map((entry) => entry.item)
}
