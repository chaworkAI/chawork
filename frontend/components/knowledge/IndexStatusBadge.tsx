import type { QmdStatus } from "@/types/knowledge"
import { useUiLabel } from "@/hooks/useUiLabel"

export interface IndexStatusBadgeProps {
  status: QmdStatus | null
  onRefresh: () => void
}

/** User-facing index state from `phase` when present, else `is_ready` + legacy copy. */
function indexStatusMessage(
  getLabel: (key: string, fallback: string) => string,
  status: QmdStatus,
): string {
  const { phase, is_ready } = status
  if (phase === "ready") return getLabel("knowledge.index.ready", "索引就绪")
  if (phase === "building") return getLabel("knowledge.index.building", "索引构建中")
  if (phase === "error") return getLabel("knowledge.index.error", "索引出错")
  if (phase === "stale") return getLabel("knowledge.index.stale", "索引待构建")
  if (!phase) {
    return is_ready
      ? getLabel("knowledge.index.ready", "索引就绪")
      : getLabel("knowledge.index.legacy_busy", "索引处理中")
  }
  return is_ready
    ? getLabel("knowledge.index.ready", "索引就绪")
    : getLabel("knowledge.index.stale", "索引待构建")
}

export function IndexStatusBadge({ status, onRefresh }: IndexStatusBadgeProps) {
  const getLabel = useUiLabel()

  if (!status) {
    return (
      <span className="text-[11px] text-muted-foreground">
        {getLabel("knowledge.index.none", "索引未初始化")}
      </span>
    )
  }

  const dotClass =
    status.phase === "error"
      ? "bg-red-400"
      : status.phase === "ready" || (!status.phase && status.is_ready)
        ? "bg-[#5c7456]"
        : "bg-[#c19762]"

  return (
    <div className="flex items-center gap-2">
      <span className={`inline-block h-1.5 w-1.5 rounded-full ${dotClass}`} />
      <span className="text-[11px] text-muted-foreground">
        {indexStatusMessage(getLabel, status)}
      </span>
      <button
        type="button"
        onClick={onRefresh}
        className="text-[11px] text-muted-foreground underline-offset-2 transition-colors hover:text-ink hover:underline"
      >
        {getLabel("knowledge.refresh", "刷新")}
      </button>
    </div>
  )
}
