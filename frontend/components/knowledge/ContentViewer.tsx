import { useUiLabel } from "@/hooks/useUiLabel"

export interface ContentViewerProps {
  filePath: string
  content: string | null
  onClose: () => void
}

/** Renders a document's markdown content in a simple readable view. */
export function ContentViewer({ filePath, content, onClose }: ContentViewerProps) {
  const getLabel = useUiLabel()
  const displayName = filePath.replace(/^qmd:\/\//, "")

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center justify-between border-b border-line px-3 py-2">
        <span className="truncate text-[12px] text-muted-foreground" title={filePath}>
          {displayName}
        </span>
        <button
          type="button"
          onClick={onClose}
          className="ml-2 shrink-0 rounded-[10px] px-2.5 py-1.5 text-[12px] text-muted-foreground transition-colors hover:bg-[#f8f9fb] hover:text-ink"
        >
          {getLabel("knowledge.viewer.close", "关闭")}
        </button>
      </div>

      <div className="flex-1 overflow-auto p-3">
        {content === null ? (
          <p className="text-[12px] text-muted-foreground">{getLabel("knowledge.viewer.loading", "加载中…")}</p>
        ) : (
          <pre className="whitespace-pre-wrap break-words rounded-[13px] border border-line-soft bg-[#f8f9fb] p-3 font-mono text-[12px] leading-relaxed text-ink">
            {content}
          </pre>
        )}
      </div>
    </div>
  )
}
