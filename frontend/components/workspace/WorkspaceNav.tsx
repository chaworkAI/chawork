import type { WorkspaceSidebarItem } from "@/types/workspace"
import { useUiLabel } from "@/hooks/useUiLabel"

export interface WorkspaceNavProps {
  items: WorkspaceSidebarItem[]
  activeWorkspaceId: string | null
  boundEmployeeName?: string | null
  onOpenCascade?: () => void
}

export function WorkspaceNav({
  items,
  activeWorkspaceId,
  boundEmployeeName,
  onOpenCascade,
}: WorkspaceNavProps) {
  const getLabel = useUiLabel()

  const activeItem = items.find((i) => i.workspace.id === activeWorkspaceId)

  return (
    <button
      type="button"
      data-tour-id="workspace-entry"
      onClick={onOpenCascade}
      className="mx-1.5 grid min-h-[76px] w-[calc(100%-12px)] grid-cols-[24px_1fr] items-center gap-2.5 rounded-[18px] bg-white/70 p-3 text-left shadow-[inset_0_0_0_1px_rgba(224,228,234,0.58)] transition-colors hover:bg-white dark:bg-panel dark:shadow-[inset_0_0_0_1px_var(--line)] dark:hover:bg-panel-raised"
    >
      <span
        className="h-[15px] w-5 rounded-[4px] border border-[#aeb7c3] bg-[linear-gradient(#dfe5ec_0_35%,#c6ced8_35%)] dark:border-line-strong dark:bg-[linear-gradient(var(--line)_0_35%,var(--panel-soft)_35%)]"
        aria-hidden
      />
      <div className="min-w-0">
        <div className="truncate text-[14px] font-bold text-ink">
          {activeItem?.workspace.name ?? getLabel("workspace.none_selected", "未选择工作区")}
        </div>
        {activeItem ? (
          <p className="mt-[5px] truncate text-[11px] font-semibold text-muted-foreground">
            {boundEmployeeName
              ? `${getLabel("workspace.cascade.bound_employee_prefix", "员工")}「${boundEmployeeName}」 · ${activeItem.metaLine}`
              : activeItem.metaLine}
          </p>
        ) : (
          <p className="mt-[5px] truncate text-[11px] font-semibold text-muted-foreground">
            {getLabel(
              "workspace.click_to_switch",
              "点击选择员工与工作区",
            )}
          </p>
        )}
      </div>
    </button>
  )
}
