import { useMemo, useState } from "react"

import { HelpTip } from "@/components/layout/HelpTip"
import type { RuntimeEvent, RuntimeEventCategory } from "@/types/events"
import { EventCard } from "@/components/runtime/EventCard"
import { useUiLabel } from "@/hooks/useUiLabel"

export interface RuntimeInspectorProps {
  events: RuntimeEvent[]
  statusLabel?: string
}

const FILTER_KEYS: { key: RuntimeEventCategory; labelPath: string; fallback: string }[] = [
  { key: "all", labelPath: "runtime.filter.all", fallback: "全部" },
  { key: "tool", labelPath: "runtime.filter.tool", fallback: "工具" },
  { key: "file", labelPath: "runtime.filter.file", fallback: "文件" },
  { key: "mcp", labelPath: "runtime.filter.mcp", fallback: "MCP" },
  { key: "system", labelPath: "runtime.filter.system", fallback: "系统" },
]

function isMcpTool(tool?: string): boolean {
  return !!tool && (tool.startsWith("mcp:") || tool.startsWith("mcp__"))
}

function eventMatchesFilter(ev: RuntimeEvent, filter: RuntimeEventCategory): boolean {
  if (filter === "all") return true
  const t = ev.event.type
  switch (filter) {
    case "tool":
      return t === "tool_call" || t === "tool_delta" || t === "tool_result"
    case "file":
      return t === "file_change" || t === "file_change_delta"
    case "mcp":
      return (
        (t === "tool_call" && isMcpTool(ev.event.tool)) ||
        (t === "tool_delta" && isMcpTool(ev.event.tool)) ||
        (t === "tool_result" && isMcpTool(ev.event.tool)) ||
        t === "retrieval" ||
        t === "mcp_oauth_login_completed" ||
        t === "mcp_server_status_updated" ||
        t === "mcp_elicitation_request" ||
        t === "skill_write" ||
        t === "skill_refresh"
      )
    case "system":
      return (
        t === "thinking" ||
        t === "error" ||
        t === "turn_complete" ||
        t === "cancelled" ||
        t === "ready" ||
        t === "runtime_debug" ||
        t === "approval_request" ||
        t === "user_input_request" ||
        t === "plan_update" ||
        t === "plan_delta" ||
        t === "plan_done"
      )
    default:
      return true
  }
}

export function RuntimeInspector({ events, statusLabel }: RuntimeInspectorProps) {
  const [activeFilter, setActiveFilter] = useState<RuntimeEventCategory>("all")
  const getLabel = useUiLabel()

  const filtered = useMemo(
    () => events.filter((ev) => eventMatchesFilter(ev, activeFilter)),
    [events, activeFilter],
  )

  return (
    <section className="flex min-h-0 flex-col rounded-panel border border-line bg-panel p-3.5 shadow-panel backdrop-blur-[24px]">
      <div className="mb-2 flex shrink-0 items-center justify-between">
        <div className="flex items-center gap-2">
          <h2 className="text-[15px] font-normal tracking-[-0.02em] text-ink">
            {getLabel("runtime.title", "AI 执行过程")}
          </h2>
          <HelpTip
            variant="bottomRight"
            tip={getLabel(
              "runtime.inspector_tip",
              "这里显示 AI 正在做什么：读取资料、搜索、调用工具、整理候选更新。它只用于观察过程，不在这里确认保存。",
            )}
          />
        </div>
        {statusLabel ? <span className="text-[12px] text-muted-foreground">{statusLabel}</span> : null}
      </div>

      <div className="mb-2 flex shrink-0 gap-1">
        {FILTER_KEYS.map((f) => (
          <button
            key={f.key}
            type="button"
            onClick={() => setActiveFilter(f.key)}
            className={`rounded-full px-2.5 py-1 text-[11px] transition-colors ${
              activeFilter === f.key
                ? "bg-[rgba(45,40,33,0.08)] font-bold text-ink"
                : "text-muted-foreground hover:bg-[rgba(255,255,255,0.42)] hover:text-ink"
            }`}
          >
            {getLabel(f.labelPath, f.fallback)}
          </button>
        ))}
      </div>

      <div className="min-h-[120px] flex-1 space-y-2 overflow-auto pr-0.5">
        {filtered.length === 0 && (
          <p className="py-6 text-center text-[12px] text-muted-foreground">
            {events.length === 0
              ? getLabel("runtime.no_events", "暂无运行事件")
              : getLabel("runtime.no_events_in_filter", "当前分类下无事件")}
          </p>
        )}
        {filtered.map((ev, i) => (
          <EventCard key={ev.id} event={ev} defaultOpen={i === 0} />
        ))}
      </div>
    </section>
  )
}
