import type { MouseEvent } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Menu, Settings, UserRound } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { useEmployeeStore } from "@/stores/employee"
import { useRootConfigStore } from "@/stores/rootConfig"
import { useUiStore } from "@/stores/ui"
import { useWorkspaceStore } from "@/stores/workspace"

function getOptionalCurrentWindow() {
  try {
    return getCurrentWindow()
  } catch {
    return null
  }
}

export function TopBar() {
  const appWindow = getOptionalCurrentWindow()
  const platformName =
    typeof navigator !== "undefined"
      ? navigator.platform
      : ""
  const isMacOS = /mac/i.test(platformName)
  const openSettingsPanel = useRootConfigStore((s) => s.openSettingsPanel)
  const keepManagementDrawerOpen = useUiStore((s) => s.keepManagementDrawerOpen)
  const scheduleManagementDrawerClose = useUiStore((s) => s.scheduleManagementDrawerClose)
  const activeBinding = useWorkspaceStore((s) => s.activeBinding)
  const pendingReviewEmployeeIds = useEmployeeStore((s) => s.pendingReviewEmployeeIds)
  const openPanel = useEmployeeStore((s) => s.openPanel)
  const selectEmployee = useEmployeeStore((s) => s.selectEmployee)
  const setActiveTab = useEmployeeStore((s) => s.setActiveTab)

  const currentEmployeeId =
    activeBinding?.status === "bound" ? activeBinding.employee_id : null
  const currentEmployee =
    activeBinding?.status === "bound" && activeBinding.employee_id
      ? {
          id: activeBinding.employee_id,
          name: activeBinding.employee_name,
        }
      : null
  const hasPendingReview =
    Boolean(currentEmployeeId) &&
    pendingReviewEmployeeIds.includes(currentEmployeeId ?? "")

  const openCurrentEmployee = () => {
    if (currentEmployeeId) {
      setActiveTab(hasPendingReview ? "dream" : "overview")
      void selectEmployee(currentEmployeeId)
    }
    openPanel("detail")
  }

  const startDragging = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0) return
    if (!appWindow) return
    void appWindow.startDragging()
  }

  const toggleMaximize = () => {
    if (!appWindow) return
    void appWindow.toggleMaximize()
  }

  const stopWindowControlDrag = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation()
  }

  const windowControlClass =
    "group/control relative grid size-3 place-items-center rounded-full transition-[filter,transform,box-shadow] duration-150 hover:brightness-105 active:scale-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"

  const windowGlyphClass =
    "absolute inset-0 grid place-items-center opacity-0 transition-opacity duration-150 group-hover/window-controls:opacity-100"

  return (
    <header
      className={cn(
        "flex h-[44px] shrink-0 items-center gap-[14px] border-b border-line-soft bg-white px-[18px] dark:bg-shell-bg",
        isMacOS && "pl-[82px]",
      )}
    >
      {!isMacOS && appWindow ? (
        <div className="group/window-controls flex shrink-0 gap-2.5" aria-label="窗口控制">
          <button
            type="button"
            className={cn(windowControlClass, "bg-[#ee6a68]")}
            title="关闭窗口"
            aria-label="关闭窗口"
            onMouseDown={stopWindowControlDrag}
            onClick={() => void appWindow.close()}
          >
            <span className={windowGlyphClass} aria-hidden>
              <span className="absolute h-[1.5px] w-[6px] rotate-45 rounded-full bg-[#7f1d1d]" />
              <span className="absolute h-[1.5px] w-[6px] -rotate-45 rounded-full bg-[#7f1d1d]" />
            </span>
          </button>
          <button
            type="button"
            className={cn(windowControlClass, "bg-[#edc14c]")}
            title="最小化窗口"
            aria-label="最小化窗口"
            onMouseDown={stopWindowControlDrag}
            onClick={() => void appWindow.minimize()}
          >
            <span className={windowGlyphClass} aria-hidden>
              <span className="h-[1.5px] w-[6px] rounded-full bg-[#8a5a00]" />
            </span>
          </button>
          <button
            type="button"
            className={cn(windowControlClass, "bg-[#62bf69]")}
            title="最大化窗口"
            aria-label="最大化窗口"
            onMouseDown={stopWindowControlDrag}
            onClick={toggleMaximize}
          >
            <span className={windowGlyphClass} aria-hidden>
              <span className="absolute h-[1.5px] w-[6px] rounded-full bg-[#166534]" />
              <span className="absolute h-[6px] w-[1.5px] rounded-full bg-[#166534]" />
            </span>
          </button>
        </div>
      ) : null}

      <div
        className={cn(
          "flex shrink-0 select-none items-center gap-2 text-[16px] font-bold tracking-normal text-[#4d5562] dark:text-ink",
          isMacOS && "-translate-y-[5px]",
        )}
        onMouseDown={startDragging}
        onDoubleClick={toggleMaximize}
      >
        <img
          src="/logo.png"
          alt=""
          draggable={false}
          className="size-6 rounded-md"
        />
        ChaWork
      </div>

      <div
        className="min-w-0 flex-1 self-stretch"
        onMouseDown={startDragging}
        onDoubleClick={toggleMaximize}
      />

      <Button
        type="button"
        data-tour-id="employee-entry"
        variant="outline"
        size="icon"
        className="relative h-8 w-8 shrink-0 rounded-[11px] border-line bg-white shadow-none hover:bg-[#f6f7f9] dark:bg-panel-soft dark:hover:bg-panel-raised"
        title={currentEmployee?.name ? `当前员工：${currentEmployee.name}` : "当前员工"}
        aria-label={hasPendingReview ? "当前 AI 员工，有待审批内容" : "当前 AI 员工"}
        onClick={openCurrentEmployee}
      >
        <UserRound className="size-4 text-muted-foreground" />
        {hasPendingReview ? (
          <span className="absolute -right-0.5 -top-0.5 size-2.5 rounded-full bg-danger ring-2 ring-shell-surface" />
        ) : null}
      </Button>

      <Button
        type="button"
        variant="outline"
        size="icon"
        className="h-8 w-8 shrink-0 rounded-[11px] border-line bg-white shadow-none hover:bg-[#f6f7f9] dark:bg-panel-soft dark:hover:bg-panel-raised"
        title="管理"
        aria-label="打开管理抽屉"
        aria-haspopup="true"
        onMouseEnter={keepManagementDrawerOpen}
        onMouseLeave={scheduleManagementDrawerClose}
        onFocus={keepManagementDrawerOpen}
        onBlur={scheduleManagementDrawerClose}
        onClick={keepManagementDrawerOpen}
      >
        <Menu className="size-4" />
      </Button>

      <Button
        type="button"
        data-tour-id="settings-entry"
        variant="outline"
        size="icon"
        className="h-8 w-8 shrink-0 rounded-[11px] border-line bg-white shadow-none hover:bg-[#f6f7f9] dark:bg-panel-soft dark:hover:bg-panel-raised"
        title="设置"
        aria-label="打开设置"
        onClick={() => openSettingsPanel()}
      >
        <Settings className="size-4" />
      </Button>
    </header>
  )
}
