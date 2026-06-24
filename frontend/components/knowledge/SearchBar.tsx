import { useCallback, type KeyboardEvent } from "react"

import { useUiLabel } from "@/hooks/useUiLabel"

export interface SearchBarProps {
  value: string
  onChange: (val: string) => void
  onSearch: () => void
  isSearching: boolean
  placeholder?: string
}

export function SearchBar({
  value,
  onChange,
  onSearch,
  isSearching,
  placeholder,
}: SearchBarProps) {
  const getLabel = useUiLabel()
  const resolvedPlaceholder =
    placeholder ?? getLabel("knowledge.search_placeholder", "搜索知识库…")
  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" && !e.nativeEvent.isComposing) {
        e.preventDefault()
        onSearch()
      }
    },
    [onSearch],
  )

  return (
    <div className="flex items-center gap-2">
      <div className="relative flex-1">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={resolvedPlaceholder}
          className="w-full min-h-[42px] rounded-[12px] border border-line bg-white px-3 text-[13px] text-ink outline-none transition-colors placeholder:text-muted-foreground focus:ring-2 focus:ring-ring/35"
        />
        {isSearching && (
          <span className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[11px] text-muted-foreground">
            {getLabel("knowledge.search_busy", "搜索中…")}
          </span>
        )}
      </div>
      <button
        type="button"
        onClick={onSearch}
        disabled={isSearching || !value.trim()}
        className="h-[42px] shrink-0 rounded-[12px] border border-line bg-white px-4 text-[13px] font-bold text-ink transition-colors hover:bg-[#f8f9fb] disabled:opacity-40"
      >
        {getLabel("knowledge.search_button", "搜索")}
      </button>
    </div>
  )
}
