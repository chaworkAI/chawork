import { useCallback } from "react"
import { Activity, X } from "lucide-react"

import { RuntimeInspector, type RuntimeInspectorProps } from "@/components/runtime/RuntimeInspector"
import { Button } from "@/components/ui/button"
import { formatSummaryDetail, formatSummaryLabel, pickSummaryEvents } from "@/lib/runtimeSummary"
import { cn } from "@/lib/utils"
import type { RuntimeBusyState } from "@/stores/runtime"
import { useUiStore } from "@/stores/ui"

import { useShellLayout } from "./shellLayout"

export interface RightRailProps {
  runtime: RuntimeInspectorProps
  status: RuntimeBusyState
}

function runtimeChipLabel(status: RuntimeBusyState): string {
  if (status === "error") return "错误"
  if (status === "idle") return "空闲"
  if (status === "pending_request") return "待确认"
  return "运行中"
}

function runtimeChipClass(status: RuntimeBusyState): string {
  if (status === "error") return "border-danger/30 bg-danger/10 text-danger"
  if (status === "idle") return "border-line bg-panel text-muted-foreground"
  if (status === "pending_request") return "border-warning/30 bg-warning/10 text-warning"
  return "border-success/30 bg-success/10 text-success"
}

function companionStatusLabel(status: RuntimeBusyState): string {
  if (status === "error") return "需要处理"
  if (status === "idle") return "待命"
  if (status === "pending_request") return "等你确认"
  if (status === "cancelling") return "收尾中"
  if (status === "thinking") return "思考中"
  return "专注中"
}

export function RightRail({ runtime, status }: RightRailProps) {
  const layout = useShellLayout()
  const drawerOpen = useUiStore((s) => s.rightRailDrawerOpen)
  const setRightRailDrawerOpen = useUiStore((s) => s.setRightRailDrawerOpen)

  const closeDrawer = useCallback(() => setRightRailDrawerOpen(false), [setRightRailDrawerOpen])
  const recentEvents = pickSummaryEvents(runtime.events, 3)

  if (layout === "wide") {
    return (
      <>
        <section className="flex min-h-0 flex-1 flex-col p-4">
          <div className="mb-3 flex items-start justify-between gap-2">
            <div>
              <p className="text-[16px] font-bold text-ink">执行摘要</p>
              <p className="mt-0.5 text-[13px] text-muted-foreground">
                {runtimeChipLabel(status)}
              </p>
            </div>
            <Button
              type="button"
              variant="outline"
              size="icon-sm"
              aria-label="打开 Runtime 详情"
              onClick={() => setRightRailDrawerOpen(true)}
            >
              <Activity className="size-4" />
            </Button>
          </div>
          <ol className="min-h-0 space-y-[7px] overflow-y-auto">
            {recentEvents.length === 0 ? (
              <li className="grid min-h-[49px] grid-cols-[10px_minmax(0,1fr)] items-center gap-2.5 rounded-[11px] bg-white/60 px-2 py-[7px] text-[12px] text-muted-foreground dark:bg-panel">
                <span className="size-[9px] rounded-full bg-[#7d91aa]" />
                {runtime.statusLabel}
              </li>
            ) : (
              recentEvents.map((event) => {
                const summaryLabel = formatSummaryLabel(event)
                const summaryDetail = formatSummaryDetail(event)
                return (
                  <li
                    key={event.id}
                    className="grid min-h-[49px] grid-cols-[10px_minmax(0,1fr)] items-center gap-2.5 rounded-[11px] bg-white/60 px-2 py-[7px] dark:bg-panel"
                  >
                    <span
                      className={cn(
                        "size-[9px] rounded-full",
                        event.displayStatus === "error"
                          ? "bg-danger"
                          : event.displayStatus === "warning"
                            ? "bg-warning"
                            : event.displayStatus === "success"
                              ? "bg-success"
                              : "bg-[#7d91aa]",
                      )}
                    />
                    <div className="min-w-0 truncate text-[11px] font-bold text-ink">
                      {summaryLabel}
                    </div>
                    {summaryDetail ? (
                      <div className="col-start-2 line-clamp-2 truncate text-[10px] text-muted-foreground">
                        {summaryDetail}
                      </div>
                    ) : null}
                  </li>
                )
              })
            )}
          </ol>

          <section
            className="vovo-stage mt-auto"
            data-status={status}
            aria-label={`VOVO，${companionStatusLabel(status)}`}
          >
            <div className="vovo-scene" aria-hidden>
              <div className="vovo-orbit" />
              <div className="vovo-signal vovo-signal-one" />
              <div className="vovo-signal vovo-signal-two" />
              <div className="vovo-shadow" />
              <div className={cn("vovo-pet", status !== "idle" && "is-awake")}>
                <span className="vovo-ear vovo-ear-left" />
                <span className="vovo-ear vovo-ear-right" />
                <span className="vovo-face">
                  <span className="vovo-eye vovo-eye-left" />
                  <span className="vovo-eye vovo-eye-right" />
                  <span className="vovo-cheek vovo-cheek-left" />
                  <span className="vovo-cheek vovo-cheek-right" />
                  <span className="vovo-mouth" />
                </span>
                <span className="vovo-collar" />
                <span className="vovo-paw vovo-paw-left" />
                <span className="vovo-paw vovo-paw-right" />
                <span className="vovo-tail" />
              </div>
            </div>
          </section>
        </section>
        {drawerOpen ? (
          <div className="fixed inset-0 z-50" role="presentation" onClick={closeDrawer}>
            <div className="absolute inset-0 bg-[rgba(34,41,54,0.18)] backdrop-blur-[2px] dark:bg-[rgba(0,0,0,0.48)]" />
            <aside
              className="absolute inset-y-0 right-0 flex w-[min(420px,calc(100vw-24px))] flex-col border-l border-line bg-panel p-3.5 shadow-panel"
              role="dialog"
              aria-modal="true"
              aria-label="Runtime"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="mb-3 flex shrink-0 items-center justify-between gap-2">
                <span className="text-[13px] font-semibold text-ink">Runtime</span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label="关闭"
                  onClick={closeDrawer}
                >
                  <X className="size-4" />
                </Button>
              </div>
              <div className="min-h-0 flex-1 overflow-hidden">
                <RuntimeInspector {...runtime} />
              </div>
            </aside>
          </div>
        ) : null}
      </>
    )
  }

  return (
    <>
      <button
        type="button"
        onClick={() => setRightRailDrawerOpen(true)}
        className={cn(
          "fixed bottom-6 right-4 z-40 flex cursor-pointer items-center gap-2 rounded-full border px-3 py-2 shadow-panel backdrop-blur-[18px] transition-colors hover:bg-[rgba(255,255,255,0.72)]",
          runtimeChipClass(status),
        )}
        aria-label="展开 Runtime 面板"
      >
        <Activity className="size-4 shrink-0" />
        <span className="text-[12px] font-medium">{runtimeChipLabel(status)}</span>
      </button>

      {drawerOpen ? (
        <div
          className="fixed inset-0 z-50"
          role="presentation"
          onClick={closeDrawer}
          onKeyDown={(e) => {
            if (e.key === "Escape") closeDrawer()
          }}
        >
          <div className="absolute inset-0 bg-[rgba(58,44,31,0.32)] backdrop-blur-[2px] dark:bg-[rgba(0,0,0,0.48)]" />
          <aside
            className="absolute inset-y-0 right-0 flex w-[min(382px,calc(100vw-24px))] flex-col border-l border-line bg-panel p-3.5 shadow-panel"
            role="dialog"
            aria-modal="true"
            aria-label="Runtime"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex shrink-0 items-center justify-between gap-2">
              <span className="text-[13px] font-semibold text-ink">Runtime</span>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label="关闭"
                onClick={closeDrawer}
              >
                <X className="size-4" />
              </Button>
            </div>
            <div className="min-h-0 flex-1 overflow-hidden">
              <RuntimeInspector {...runtime} />
            </div>
          </aside>
        </div>
      ) : null}
    </>
  )
}
