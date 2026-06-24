import { X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useHubStore } from "@/stores/hub"
import { useUiStore } from "@/stores/ui"
import { useWorkspaceStore } from "@/stores/workspace"

interface DrawerAction {
  label: string
  description: string
  primary?: boolean
  run: () => void
}

export function ManagementDrawer() {
  const open = useUiStore((s) => s.managementDrawerOpen)
  const setOpen = useUiStore((s) => s.setManagementDrawerOpen)
  const keepOpen = useUiStore((s) => s.keepManagementDrawerOpen)
  const scheduleClose = useUiStore((s) => s.scheduleManagementDrawerClose)
  const setDreamSchedulePanelOpen = useUiStore((s) => s.setDreamSchedulePanelOpen)
  const hubStore = useHubStore
  const workspaceStore = useWorkspaceStore

  if (!open) return null

  const close = () => setOpen(false)
  const run = (fn: () => void) => {
    close()
    fn()
  }

  const actions: DrawerAction[] = [
    {
      label: "员工市场",
      description: "搜索、筛选并下载远程或本地员工。",
      primary: true,
      run: () => hubStore.getState().openMarket("employees"),
    },
    {
      label: "技能市场",
      description: "浏览并同步根工作区技能库。",
      run: () => hubStore.getState().openMarket("skills"),
    },
    {
      label: "定时做梦",
      description: "统一配置所有员工的定时做梦策略。",
      run: () => {
        setDreamSchedulePanelOpen(true)
      },
    },
    {
      label: "工具管理",
      description: "管理 MCP 工具、权限策略和员工分配。",
      run: () => workspaceStore.getState().openWorkspaceConfig("tools"),
    },
  ]

  const renderAction = (action: DrawerAction) => (
    <button
      key={action.label}
      type="button"
      className={`block min-h-[64px] w-full rounded-[13px] border px-3 py-[11px] text-left transition-colors hover:bg-[#f7f9fb] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
        action.primary
          ? "border-[#cbdccc] bg-[#f4faf5] dark:border-accent/45 dark:bg-accent/20"
          : "border-line bg-white dark:bg-panel"
      }`}
      onClick={() => run(action.run)}
    >
      <strong className="mb-[3px] block text-[13px] text-ink">{action.label}</strong>
      <small className="block text-[11px] leading-[1.45] text-muted-foreground">
        {action.description}
      </small>
    </button>
  )

  return (
    <aside
      className="fixed right-[18px] top-[54px] z-50 flex max-h-[calc(100dvh-68px)] w-[330px] flex-col gap-3.5 overflow-y-auto rounded-[18px] border border-line bg-white p-4 shadow-[0_22px_58px_rgba(31,38,50,0.18)] dark:bg-panel-soft dark:shadow-[0_22px_58px_rgba(0,0,0,0.38)]"
      role="dialog"
      aria-modal="false"
      aria-label="管理中心"
      onMouseEnter={keepOpen}
      onMouseLeave={scheduleClose}
      onFocus={keepOpen}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          scheduleClose()
        }
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") close()
      }}
    >
      <header className="flex items-start justify-between gap-3">
        <div>
          <strong className="block text-[15px] text-ink">管理中心</strong>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="h-[30px] w-[30px] text-[20px]"
          aria-label="关闭管理抽屉"
          onClick={close}
        >
          <X className="size-4" />
        </Button>
      </header>

      <section className="grid gap-2">
        {actions.map(renderAction)}
      </section>
    </aside>
  )
}
