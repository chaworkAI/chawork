import { useState } from "react"

import { applyLabelTemplate } from "@/lib/builtinLabels"
import { Badge } from "@/components/ui/badge"
import { MonospaceBlockCard } from "@/components/ui/monospace-block-card"
import { useUiLabel } from "@/hooks/useUiLabel"
import type { CodexEvent, FileAction, RuntimeEvent } from "@/types/events"

export interface EventCardProps {
  event: RuntimeEvent
  defaultOpen?: boolean
}

const dotClasses: Record<RuntimeEvent["displayStatus"], string> = {
  success: "bg-success shadow-[0_0_0_4px_rgba(83,116,90,0.1)]",
  info: "bg-success shadow-[0_0_0_4px_rgba(83,116,90,0.1)]",
  warning: "bg-warning shadow-[0_0_0_4px_rgba(183,121,45,0.12)]",
  error: "bg-danger shadow-[0_0_0_4px_rgba(184,92,80,0.12)]",
}

const fileActionStyle: Record<FileAction, string> = {
  create:
    "border-success/35 bg-success/[0.07] text-success",
  modify:
    "border-warning/40 bg-warning/[0.09] text-[#8a5f16]",
  delete:
    "border-danger/35 bg-danger/[0.08] text-danger",
}

function fileActionWord(
  getLabel: (key: string, fallback: string) => string,
  action: FileAction,
): string {
  if (action === "create") return getLabel("events.file.create", "新建")
  if (action === "modify") return getLabel("events.file.modify", "修改")
  return getLabel("events.file.delete", "删除")
}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function SummaryPrefix({
  ev,
  getLabel,
}: {
  ev: CodexEvent
  getLabel: (key: string, fallback: string) => string
}) {
  if (ev.type === "thinking") {
    return (
      <span
        className="mr-1.5 inline-flex size-[22px] shrink-0 items-center justify-center rounded-full border border-line bg-[rgba(255,255,255,0.55)] text-[13px] shadow-[inset_0_1px_0_rgba(255,255,255,0.65)]"
        title={getLabel("events.thinking.title", "思考")}
        aria-hidden
      >
        🧠
      </span>
    )
  }
  return null
}

function EventCardBody({
  ev,
  getLabel,
  fallbackDetail,
}: {
  ev: CodexEvent
  getLabel: (key: string, fallback: string) => string
  fallbackDetail?: string
}) {
  switch (ev.type) {
    case "tool_call":
      return (
        <div className="space-y-2">
          <div>
            <span className="text-[10px] font-bold uppercase tracking-[0.12em] text-muted-foreground">
              {getLabel("runtime.filter.tool", "工具")}
            </span>
            <p className="font-mono text-[14px] font-bold tracking-tight text-accent-dark">
              {ev.tool}
            </p>
          </div>
          <MonospaceBlockCard
            label={getLabel("events.badge.tool_args", "参数")}
            maxHeightClassName="max-h-[220px]"
          >
            {formatJson(ev.args)}
          </MonospaceBlockCard>
        </div>
      )
    case "tool_delta":
      return (
        <MonospaceBlockCard
          label={ev.tool}
          maxHeightClassName="max-h-[220px]"
          tone="muted"
        >
          {ev.content}
        </MonospaceBlockCard>
      )
    case "file_change":
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={`rounded-full border px-2 py-0.5 text-[11px] font-bold ${fileActionStyle[ev.action]}`}
            >
              {fileActionWord(getLabel, ev.action)}
            </span>
            <span className="min-w-0 break-all font-mono text-[12px] text-ink">{ev.path}</span>
          </div>
          {ev.diff ? (
            <MonospaceBlockCard
              label={getLabel("events.badge.file_diff", "差异")}
              maxHeightClassName="max-h-[200px]"
              tone="muted"
            >
              {ev.diff}
            </MonospaceBlockCard>
          ) : null}
        </div>
      )
    case "file_change_delta":
      return (
        <MonospaceBlockCard
          label="输出"
          maxHeightClassName="max-h-[220px]"
          tone="muted"
        >
          {ev.content}
        </MonospaceBlockCard>
      )
    case "mcp_oauth_login_completed":
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={ev.success ? "default" : "destructive"}>
              {ev.success
                ? getLabel("events.mcp_oauth.badge_ok", "OAuth 完成")
                : getLabel("events.mcp_oauth.badge_fail", "OAuth 失败")}
            </Badge>
            <span className="font-mono text-[12px] text-ink">{ev.server_name}</span>
          </div>
          {ev.error ? (
            <MonospaceBlockCard
              label={getLabel("events.badge.tool_error", "错误")}
              maxHeightClassName="max-h-[180px]"
              tone="muted"
            >
              {ev.error}
            </MonospaceBlockCard>
          ) : null}
        </div>
      )
    case "mcp_server_status_updated":
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={ev.status === "failed" ? "destructive" : "outline"}>
              {ev.status}
            </Badge>
            <span className="font-mono text-[12px] text-ink">{ev.server_name}</span>
          </div>
          {ev.error ? (
            <MonospaceBlockCard
              label={getLabel("events.badge.tool_error", "错误")}
              maxHeightClassName="max-h-[180px]"
              tone="muted"
            >
              {ev.error}
            </MonospaceBlockCard>
          ) : null}
        </div>
      )
    case "plan_update":
      return (
        <div className="space-y-2">
          {ev.explanation ? (
            <p className="text-[12px] leading-relaxed text-muted-foreground">
              {ev.explanation}
            </p>
          ) : null}
          <ul className="m-0 list-none space-y-1.5 p-0">
            {ev.steps.map((step, i) => (
              <li
                key={`${step.status}-${i}`}
                className="rounded-[8px] bg-[rgba(255,252,246,0.7)] px-2 py-1.5 text-[12px] text-ink"
              >
                <span className="mr-2 font-mono text-[10px] uppercase text-muted-foreground">
                  {step.status}
                </span>
                {step.step}
              </li>
            ))}
          </ul>
        </div>
      )
    case "plan_delta":
    case "plan_done":
      return (
        <MonospaceBlockCard
          label={ev.type === "plan_done" ? "计划" : "计划片段"}
          maxHeightClassName="max-h-[220px]"
          tone="muted"
        >
          {ev.content}
        </MonospaceBlockCard>
      )
    case "thinking":
      return (
        <div className="border-l-2 border-accent/35 pl-3">
          <p className="text-[12px] leading-relaxed text-muted-foreground">
            {ev.summary}
          </p>
          {fallbackDetail ? (
            <p className="mt-1.5 text-[12px] leading-relaxed text-ink/80">
              {fallbackDetail}
              <span className="ml-0.5 inline-block size-[6px] animate-pulse rounded-full bg-accent/50 align-middle" />
            </p>
          ) : null}
        </div>
      )
    case "tool_result": {
      const body = ev.error
        ? ev.error
        : formatJson(ev.result)
      return (
        <MonospaceBlockCard
          label={
            ev.error
              ? getLabel("events.badge.tool_error", "错误")
              : getLabel("events.badge.tool_result", "结果")
          }
          maxHeightClassName="max-h-[220px]"
          tone={ev.error ? "muted" : "default"}
        >
          {body}
        </MonospaceBlockCard>
      )
    }
    case "retrieval":
      return (
        <div className="space-y-2">
          <Badge variant="outline">{getLabel("events.retrieval.badge", "知识检索")}</Badge>
          <p className="text-[12px] text-ink">
            <span className="text-muted-foreground">查询: </span>
            {ev.query}
          </p>
          {ev.results.length > 0 ? (
            <ul className="m-0 list-none space-y-1 p-0">
              {ev.results.slice(0, 8).map((r, i) => (
                <li
                  key={i}
                  className="rounded-[8px] bg-[rgba(255,252,246,0.7)] px-2 py-1 font-mono text-[11px] text-muted-foreground"
                >
                  {r.path}{" "}
                  <span className="text-[10px]">(score: {r.score.toFixed(2)})</span>
                </li>
              ))}
            </ul>
          ) : null}
          {ev.results.length > 8 ? (
            <p className="text-[11px] text-muted-foreground">
              {applyLabelTemplate(
                getLabel("events.retrieval.more", "… 还有 {{n}} 条结果"),
                { n: String(ev.results.length - 8) },
              )}
            </p>
          ) : null}
        </div>
      )
    case "runtime_debug":
      return (
        <MonospaceBlockCard
          label={`${ev.category} · ${ev.method}`}
          maxHeightClassName="max-h-[260px]"
          tone="muted"
        >
          {formatJson(ev.params)}
        </MonospaceBlockCard>
      )
    case "skill_write":
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge>Skill 写入</Badge>
            <span className="text-[12px] text-ink">
              {ev.scope} / {ev.executor}
            </span>
            <Badge
              variant="outline"
              className={
                ev.status === "synced"
                  ? "border-success/30 bg-success/10 text-success"
                  : ev.status === "error"
                    ? "border-danger/30 bg-danger/10 text-danger"
                    : "border-warning/35 bg-warning/10 text-warning"
              }
            >
              {ev.status}
            </Badge>
          </div>
          <code className="block break-all rounded-[8px] bg-[rgba(255,252,246,0.7)] px-2 py-1 font-mono text-[11px] text-accent-dark">
            {ev.target_path}
          </code>
          <p className="text-[12px] text-muted-foreground">{ev.message}</p>
        </div>
      )
    case "skill_refresh":
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge>Runtime 刷新</Badge>
            {ev.generation !== undefined ? (
              <span className="text-[12px] text-ink">generation: {ev.generation}</span>
            ) : null}
            <Badge
              variant="outline"
              className={
                ev.status === "synced"
                  ? "border-success/30 bg-success/10 text-success"
                  : ev.status === "error"
                    ? "border-danger/30 bg-danger/10 text-danger"
                    : "border-warning/35 bg-warning/10 text-warning"
              }
            >
              {ev.status}
            </Badge>
          </div>
          <p className="text-[12px] text-muted-foreground">{ev.message}</p>
        </div>
      )
    default:
      return fallbackDetail ? (
        <p className="text-[12px] leading-[1.45] text-muted-foreground">{fallbackDetail}</p>
      ) : null
  }
}

export function EventCard({ event, defaultOpen }: EventCardProps) {
  const getLabel = useUiLabel()
  const [open, setOpen] = useState(defaultOpen ?? false)
  const dot = dotClasses[event.displayStatus]
  const { event: ev } = event

  const rich =
    ev.type === "tool_call" ||
    ev.type === "tool_delta" ||
    ev.type === "file_change" ||
    ev.type === "file_change_delta" ||
    ev.type === "plan_update" ||
    ev.type === "plan_delta" ||
    ev.type === "plan_done" ||
    ev.type === "thinking" ||
    ev.type === "tool_result" ||
    ev.type === "retrieval" ||
    ev.type === "runtime_debug" ||
    ev.type === "skill_write" ||
    ev.type === "skill_refresh"

  const detailBlock =
    rich ? (
      <EventCardBody
        ev={ev}
        getLabel={getLabel}
        fallbackDetail={ev.type === "thinking" ? event.liveContent : event.detail}
      />
    ) : event.detail ? (
      <p className="text-[12px] leading-[1.45] text-muted-foreground">{event.detail}</p>
    ) : null

  return (
    <details
      open={open}
      onToggle={(e) => setOpen(e.currentTarget.open)}
      className="overflow-hidden rounded-[16px] border border-line bg-[rgba(255,255,255,0.42)]"
    >
      <summary className="flex cursor-pointer list-none items-center gap-2.5 px-3 py-[11px] text-[13px] font-bold text-ink [&::-webkit-details-marker]:hidden">
        <span className={`size-[9px] shrink-0 rounded-full ${dot}`} aria-hidden />
        <span className="flex min-w-0 flex-1 items-start gap-0">
          <SummaryPrefix ev={ev} getLabel={getLabel} />
          <span className="min-w-0 flex-1 leading-snug">{event.displayLabel}</span>
        </span>
      </summary>
      {detailBlock ? (
        <div className="border-t border-line/70 bg-[rgba(255,252,246,0.35)] pb-3 pl-[31px] pr-3 pt-2.5">
          {detailBlock}
        </div>
      ) : null}
    </details>
  )
}
