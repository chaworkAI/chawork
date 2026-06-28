import { useMemo, useState } from "react"

import { HelpTip } from "@/components/layout/HelpTip"
import { Badge } from "@/components/ui/badge"
import { MonospaceBlockCard } from "@/components/ui/monospace-block-card"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { humanizeToolName } from "@/lib/runtimeSummary"
import {
  buildRuntimeTimeline,
  isMcpTool,
  type FileTimelineItem,
  type RuntimeTimelineItem,
  type ToolTimelineItem,
} from "@/lib/runtimeTimeline"
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

function eventMatchesFilter(item: RuntimeTimelineItem, filter: RuntimeEventCategory): boolean {
  if (filter === "all") return true
  if (item.kind === "tool") {
    if (filter === "tool") return true
    if (filter === "mcp") return isMcpTool(item.tool)
    return false
  }
  if (item.kind === "file") {
    return filter === "file"
  }
  const t = item.event.event.type
  switch (filter) {
    case "tool":
      return false
    case "file":
      return false
    case "mcp":
      return (
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

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function ToolTimelineCard({ item, defaultOpen }: { item: ToolTimelineItem; defaultOpen: boolean }) {
  const getLabel = useUiLabel()
  const [open, setOpen] = useState(defaultOpen)
  const statusLabel =
    item.status === "failed"
      ? getLabel("events.tool_status.failed", "失败")
      : item.status === "completed"
        ? getLabel("events.tool_status.completed", "完成")
        : getLabel("events.tool_status.running", "运行中")
  const dot =
    item.displayStatus === "error"
      ? "bg-danger shadow-[0_0_0_4px_rgba(184,92,80,0.12)]"
      : item.displayStatus === "success"
        ? "bg-success shadow-[0_0_0_4px_rgba(83,116,90,0.1)]"
        : "bg-warning shadow-[0_0_0_4px_rgba(183,121,45,0.12)]"

  return (
    <details
      open={open}
      onToggle={(e) => setOpen(e.currentTarget.open)}
      className="overflow-hidden rounded-[16px] border border-line bg-[rgba(255,255,255,0.42)]"
    >
      <summary className="flex cursor-pointer list-none items-center gap-2.5 px-3 py-[11px] text-[13px] font-bold text-ink [&::-webkit-details-marker]:hidden">
        <span className={`size-[9px] shrink-0 rounded-full ${dot}`} aria-hidden />
        <span className="min-w-0 flex-1 leading-snug">
          {applyLabelTemplate(
            getLabel("events.tool_lifecycle.title", "工具 · {{tool}} · {{status}}"),
            { tool: humanizeToolName(item.tool, getLabel), status: statusLabel },
          )}
        </span>
      </summary>
      <div className="space-y-2 border-t border-line/70 bg-[rgba(255,252,246,0.35)] pb-3 pl-[31px] pr-3 pt-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={item.status === "failed" ? "destructive" : "outline"}>
            {statusLabel}
          </Badge>
          <span className="break-all font-mono text-[12px] text-ink">{item.tool}</span>
        </div>
        {item.args !== undefined ? (
          <MonospaceBlockCard label={getLabel("events.badge.tool_args", "参数")}>
            {formatJson(item.args)}
          </MonospaceBlockCard>
        ) : null}
        {item.output ? (
          <MonospaceBlockCard label={getLabel("events.badge.tool_output", "输出")} tone="muted">
            {item.output}
          </MonospaceBlockCard>
        ) : null}
        {item.error ? (
          <MonospaceBlockCard label={getLabel("events.badge.tool_error", "错误")} tone="muted">
            {item.error}
          </MonospaceBlockCard>
        ) : item.result !== undefined ? (
          <MonospaceBlockCard label={getLabel("events.badge.tool_result", "结果")}>
            {formatJson(item.result)}
          </MonospaceBlockCard>
        ) : null}
      </div>
    </details>
  )
}

function FileTimelineCard({ item, defaultOpen }: { item: FileTimelineItem; defaultOpen: boolean }) {
  const getLabel = useUiLabel()
  const [open, setOpen] = useState(defaultOpen)
  const statusLabel =
    item.status === "failed" || item.status === "declined"
      ? getLabel("events.file_status.failed", "失败")
      : item.status === "completed"
        ? getLabel("events.file_status.completed", "完成")
        : getLabel("events.file_status.running", "变更中")
  const action =
    item.action === "create"
      ? getLabel("events.file.create", "新建")
      : item.action === "delete"
        ? getLabel("events.file.delete", "删除")
        : getLabel("events.file.modify", "修改")

  return (
    <details
      open={open}
      onToggle={(e) => setOpen(e.currentTarget.open)}
      className="overflow-hidden rounded-[16px] border border-line bg-[rgba(255,255,255,0.42)]"
    >
      <summary className="flex cursor-pointer list-none items-center gap-2.5 px-3 py-[11px] text-[13px] font-bold text-ink [&::-webkit-details-marker]:hidden">
        <span className="size-[9px] shrink-0 rounded-full bg-success shadow-[0_0_0_4px_rgba(83,116,90,0.1)]" aria-hidden />
        <span className="min-w-0 flex-1 leading-snug">
          {action}文件 · {statusLabel}
        </span>
      </summary>
      <div className="space-y-2 border-t border-line/70 bg-[rgba(255,252,246,0.35)] pb-3 pl-[31px] pr-3 pt-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="outline">{statusLabel}</Badge>
          <span className="min-w-0 break-all font-mono text-[12px] text-ink">{item.path}</span>
        </div>
        {item.diff ? (
          <MonospaceBlockCard label={getLabel("events.badge.file_diff", "差异")} tone="muted">
            {item.diff}
          </MonospaceBlockCard>
        ) : null}
        {item.output ? (
          <MonospaceBlockCard label={getLabel("events.file_change_delta.output", "输出")} tone="muted">
            {item.output}
          </MonospaceBlockCard>
        ) : null}
      </div>
    </details>
  )
}

export function RuntimeInspector({ events, statusLabel }: RuntimeInspectorProps) {
  const [activeFilter, setActiveFilter] = useState<RuntimeEventCategory>("all")
  const getLabel = useUiLabel()

  const timeline = useMemo(() => buildRuntimeTimeline(events), [events])
  const filtered = useMemo(
    () => timeline.filter((item) => eventMatchesFilter(item, activeFilter)),
    [timeline, activeFilter],
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
        {filtered.map((item, i) => (
          item.kind === "tool" ? (
            <ToolTimelineCard key={`tool-${item.id}`} item={item} defaultOpen={i === filtered.length - 1} />
          ) : item.kind === "file" ? (
            <FileTimelineCard key={`file-${item.id}`} item={item} defaultOpen={i === filtered.length - 1} />
          ) : (
            <EventCard key={item.event.id} event={item.event} defaultOpen={i === filtered.length - 1} />
          )
        ))}
      </div>
    </section>
  )
}
