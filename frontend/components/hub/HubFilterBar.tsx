import type { HubDownloadFilter } from "@/types/hub"

const FILTERS: Array<{ key: HubDownloadFilter; label: string }> = [
  { key: "all", label: "全部" },
  { key: "remote", label: "远程" },
  { key: "local", label: "本地" },
  { key: "update_available", label: "待更新" },
  { key: "custom", label: "自定义" },
]

export function HubFilterBar({
  value,
  onChange,
}: {
  value: HubDownloadFilter
  onChange: (filter: HubDownloadFilter) => void
}) {
  return (
    <div className="inline-flex h-9 rounded-[12px] border border-line bg-[#f6f7f9] p-1">
      {FILTERS.map((filter) => (
        <button
          key={filter.key}
          type="button"
          className={[
            "h-7 rounded-[9px] px-3 text-[12px] font-bold transition",
            value === filter.key
              ? "bg-white text-ink shadow-sm"
              : "text-muted-foreground hover:text-ink",
          ].join(" ")}
          onClick={() => onChange(filter.key)}
        >
          {filter.label}
        </button>
      ))}
    </div>
  )
}
