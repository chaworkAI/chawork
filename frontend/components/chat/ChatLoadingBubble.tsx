import { Loader2 } from "lucide-react"

import { useUiLabel } from "@/hooks/useUiLabel"

export function ChatLoadingBubble() {
  const getLabel = useUiLabel()

  return (
    <div
      className="flex max-w-[min(100%,42rem)] items-start gap-2.5 rounded-[14px] border border-[#e8ecf2] bg-[#fdfefe] px-3.5 py-3 text-ink dark:border-line dark:bg-panel-soft"
      role="status"
      aria-live="polite"
    >
      <Loader2 className="mt-0.5 size-4 shrink-0 animate-spin text-[#4f6178] dark:text-accent-dark" />
      <div className="min-w-0">
        <p className="text-[13px] font-medium text-ink">
          {getLabel("chat.loading.title", "正在生成回复")}
        </p>
        <p className="mt-0.5 text-[12px] text-muted-foreground">
          {getLabel("chat.loading.hint", "模型思考中，请稍候…")}
        </p>
      </div>
    </div>
  )
}
