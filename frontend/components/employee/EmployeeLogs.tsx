import { useCallback, useEffect, useState } from "react"
import { RefreshCw, Loader2, ScrollText } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import type { DreamLogEntry } from "@/types/employee"

const EVENT_COLORS: Record<string, string> = {
  run_started: "bg-blue-100 text-blue-700",
  sessions_selected: "bg-blue-100 text-blue-700",
  run_completed: "bg-emerald-100 text-emerald-700",
  prompt_applied: "bg-emerald-100 text-emerald-700",
  request_approved: "bg-emerald-100 text-emerald-700",
  parse_failed: "bg-red-100 text-red-700",
  apply_failed: "bg-red-100 text-red-700",
  snapshot_error: "bg-red-100 text-red-700",
  request_rejected: "bg-orange-100 text-orange-700",
}

function badgeClass(event: string): string {
  return EVENT_COLORS[event] ?? "bg-gray-100 text-gray-600"
}

export function EmployeeLogs() {
  const label = useUiLabel()
  const [entries, setEntries] = useState<DreamLogEntry[]>([])
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const { getDreamLog } = await import("@/lib/tauri")
      const data = await getDreamLog(50)
      setEntries(data)
    } catch {
      // silently ignore — log may not exist yet
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h4 className="flex items-center gap-1.5 text-[13px] font-bold">
          <ScrollText className="size-3.5" />
          {label("employee.logs.title", "Dream 运行日志")}
        </h4>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          title={label("employee.logs.refresh", "刷新日志")}
          disabled={loading}
          onClick={() => void load()}
        >
          {loading ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <RefreshCw className="size-3.5" />
          )}
        </Button>
      </div>

      {entries.length === 0 ? (
        <p className="py-4 text-center text-[12px] text-muted-foreground">
          {loading
            ? label("employee.logs.loading", "加载中…")
            : label("employee.logs.empty", "暂无日志记录")}
        </p>
      ) : (
        <div className="max-h-[260px] overflow-y-auto rounded-[8px] border border-line bg-white">
          {entries.map((entry, i) => (
            <div
              key={`${entry.timestamp}-${i}`}
              className="flex items-start gap-2 border-b border-line/50 px-3 py-2 last:border-b-0"
            >
              <span className="shrink-0 pt-0.5 font-mono text-[10px] text-muted-foreground">
                {entry.timestamp.replace("T", " ").replace("Z", "")}
              </span>
              <span
                className={`shrink-0 rounded-[4px] px-1.5 py-0.5 text-[10px] font-medium ${badgeClass(entry.event)}`}
              >
                {entry.event}
              </span>
              <span className="min-w-0 break-all text-[11px] text-ink">
                {entry.message}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
