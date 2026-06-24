import type { QmdSearchResult } from "@/types/knowledge"
import { useUiLabel } from "@/hooks/useUiLabel"

export interface SearchResultListProps {
  results: QmdSearchResult[]
  onSelect: (filePath: string) => void
}

/** Extracts a short display name from a `qmd://collection/path.md` URI. */
function displayPath(file: string): string {
  return file.replace(/^qmd:\/\//, "")
}

export function SearchResultList({ results, onSelect }: SearchResultListProps) {
  const getLabel = useUiLabel()

  if (results.length === 0) {
    return (
      <p className="py-4 text-center text-[12px] text-muted-foreground">
        {getLabel("knowledge.results_empty", "暂无搜索结果")}
      </p>
    )
  }

  return (
    <ul className="grid gap-2">
      {results.map((r) => (
        <li key={r.docid}>
          <button
            type="button"
            onClick={() => onSelect(r.file)}
            className="w-full rounded-[15px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3 text-left transition-colors hover:bg-[#eef1f5]"
          >
            <div className="flex items-baseline justify-between gap-2">
              <span className="truncate text-[13px] font-medium text-ink">
                {r.title || displayPath(r.file)}
              </span>
              {r.score > 0 && (
                <span className="shrink-0 text-[11px] text-muted-foreground">
                  {r.score.toFixed(2)}
                </span>
              )}
            </div>
            {r.breadcrumb && r.breadcrumb.length > 0 ? (
              <p className="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
                <span className="text-ink/70">{r.breadcrumb}</span>
                {typeof r.start_char === "number" &&
                typeof r.end_char === "number" &&
                r.end_char > r.start_char ? (
                  <span className="ml-1.5 font-mono text-[10px] text-muted-foreground/70">
                    [{r.start_char}-{r.end_char})
                  </span>
                ) : null}
              </p>
            ) : null}
            <p className="mt-0.5 text-[12px] leading-relaxed text-muted-foreground line-clamp-2">
              {displayPath(r.file)}
            </p>
            {r.snippet && (
              <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground/70 line-clamp-3">
                {r.snippet.replace(/@@ .+ @@/g, "").trim()}
              </p>
            )}
          </button>
        </li>
      ))}
    </ul>
  )
}
