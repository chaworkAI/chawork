import { useCallback } from "react"
import { ChevronDown, ChevronRight, Loader2 } from "lucide-react"

import { useUiLabel } from "@/hooks/useUiLabel"
import { cn } from "@/lib/utils"

export interface ThinkingBlockProps {
  content: string
  isStreaming?: boolean
  isExpanded: boolean
  onToggleExpanded: () => void
}

export function ThinkingBlock({
  content,
  isStreaming = false,
  isExpanded,
  onToggleExpanded,
}: ThinkingBlockProps) {
  const getLabel = useUiLabel()

  const handleToggle = useCallback(() => {
    onToggleExpanded()
  }, [onToggleExpanded])

  if (!content.trim() && !isStreaming) {
    return null
  }

  return (
    <div
      className={cn(
        "mb-3 min-w-0 overflow-hidden rounded-[12px] border border-[#d6dee8]",
        "bg-[#f4f7fb] dark:border-line dark:bg-panel",
      )}
    >
      <button
        type="button"
        onClick={handleToggle}
        className="flex w-full items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-[#eef3f8] dark:hover:bg-panel-raised"
        aria-expanded={isExpanded}
      >
        {isExpanded ? (
          <ChevronDown className="size-3.5 shrink-0 text-[#4f6178] dark:text-accent-dark" />
        ) : (
          <ChevronRight className="size-3.5 shrink-0 text-[#4f6178] dark:text-accent-dark" />
        )}
        <span className="text-[12px] font-bold text-[#3b4858] dark:text-ink">
          {getLabel("chat.thinking.title", "思考过程")}
        </span>
        {isStreaming ? (
          <Loader2 className="ml-auto size-3.5 animate-spin text-[#4f6178] dark:text-accent-dark" />
        ) : (
          <span className="ml-auto text-[11px] text-muted-foreground">
            {getLabel("chat.thinking.done", "已完成")}
          </span>
        )}
      </button>
      {isExpanded ? (
        <div className="max-h-[min(280px,40vh)] overflow-y-auto border-t border-[#e0e6ee] px-3 py-2.5 dark:border-line">
          <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed text-ink/78">
            {content}
            {isStreaming ? (
              <span
                className="ml-0.5 inline-block h-[1em] w-1.5 animate-pulse rounded-sm bg-[#4f6178] align-[-0.12em] dark:bg-accent"
                aria-hidden
              />
            ) : null}
          </p>
        </div>
      ) : (
        <button
          type="button"
          onClick={handleToggle}
          className="w-full break-words border-t border-[#e0e6ee] px-3 py-2 text-left text-[12px] text-muted-foreground hover:bg-[#eef3f8] dark:border-line dark:hover:bg-panel-raised"
        >
          {content.trim().length > 96
            ? `${content.trim().slice(0, 96)}…`
            : content.trim() || getLabel("chat.thinking.streaming", "思考中…")}
          <span className="ml-1 text-[#4f6178] dark:text-accent-dark">
            {getLabel("chat.thinking.expand", "展开")}
          </span>
        </button>
      )}
    </div>
  )
}
