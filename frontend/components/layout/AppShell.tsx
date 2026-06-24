import type { ReactNode } from "react"

import { cn } from "@/lib/utils"
import { useUiStore } from "@/stores/ui"

import {
  ShellLayoutProvider,
  useShellBreakpoint,
  type ShellLayout,
} from "./shellLayout"

export interface AppShellProps {
  sidebar: ReactNode
  sidebarRail?: ReactNode
  main: ReactNode
  rightRail: ReactNode
}

function gridClassFor(layout: ShellLayout, collapsed: boolean): string {
  switch (layout) {
    case "wide":
      return collapsed
        ? "grid-cols-[60px_minmax(560px,1fr)_260px]"
        : "grid-cols-[236px_minmax(560px,1fr)_260px]"
    case "tablet":
      return collapsed
        ? "grid-cols-[60px_minmax(0,1fr)]"
        : "grid-cols-[236px_minmax(0,1fr)]"
    case "mobile":
      return "grid-cols-[60px_minmax(0,1fr)]"
  }
}

export function AppShell({ sidebar, sidebarRail, main, rightRail }: AppShellProps) {
  const layout = useShellBreakpoint()
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed)
  const collapsed = layout === "mobile" || sidebarCollapsed

  return (
    <ShellLayoutProvider layout={layout}>
      <section
        data-shell={layout}
        className={cn(
          "grid h-full min-h-0 gap-0",
          layout === "mobile" ? "pl-0" : "pl-0",
          gridClassFor(layout, collapsed),
        )}
      >
        <aside
          className={cn(
            "flex min-h-0 flex-col overflow-hidden border-r border-line-soft bg-[#f8fafc] dark:bg-panel-soft",
            collapsed
              ? "items-center rounded-br-[20px] rounded-tr-[20px] p-2"
              : "rounded-br-[20px] rounded-tr-[20px] px-3.5 py-[18px]",
          )}
        >
          {collapsed ? (sidebarRail ?? sidebar) : sidebar}
        </aside>

        <section className="relative z-[2] min-h-0 overflow-hidden rounded-[20px] bg-white dark:bg-panel">
          {main}
        </section>

        {layout === "wide" ? (
          <aside className="flex min-h-0 flex-col overflow-hidden rounded-bl-[20px] rounded-tl-[20px] border-l border-line-soft bg-[#f8fafc] dark:bg-panel-soft">{rightRail}</aside>
        ) : (
          rightRail
        )}
      </section>
    </ShellLayoutProvider>
  )
}
