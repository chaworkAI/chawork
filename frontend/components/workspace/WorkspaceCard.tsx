import type { WorkspaceSidebarItem } from "@/types/workspace"

export interface WorkspaceCardProps {
  item: WorkspaceSidebarItem
  isActive: boolean
  onSelect?: () => void
}

export function WorkspaceCard({ item, isActive, onSelect }: WorkspaceCardProps) {
  const { workspace, metaLine } = item

  return (
    <button
      type="button"
      onClick={onSelect}
      className={`grid w-full grid-cols-[24px_1fr] items-start gap-2.5 rounded-[15px] border p-3 text-left transition-colors duration-150 ease-out hover:border-line hover:bg-[#f7f9fb] ${
        isActive ? "border-line bg-white" : "border-transparent bg-transparent"
      }`}
    >
      <span
        className="mt-0.5 block h-[16px] w-[20px] rounded-[3px] border border-[#aeb8c4] bg-[#eef1f5] before:block before:h-[5px] before:w-[10px] before:-translate-y-[5px] before:rounded-t-[3px] before:border before:border-b-0 before:border-[#aeb8c4] before:bg-[#eef1f5]"
        aria-hidden
      />
      <span className="min-w-0">
        <span className="block truncate text-[14px] font-bold text-ink">{workspace.name}</span>
        <span className="mt-[5px] block text-[12px] text-muted-foreground">{metaLine}</span>
      </span>
    </button>
  )
}
