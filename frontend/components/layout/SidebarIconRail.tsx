import { ChevronRight, FolderOpen } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiStore } from "@/stores/ui"

export interface SidebarIconRailProps {
  onNewSession?: () => void
  onOpenWorkspace?: () => void
  onOpenWorkspaceConfig?: () => void
}

export function SidebarIconRail({
  onOpenWorkspace,
}: SidebarIconRailProps) {
  const setSidebarCollapsed = useUiStore((s) => s.setSidebarCollapsed)

  const iconBtn =
    "size-10 rounded-[14px] border-line bg-white text-muted-foreground shadow-none hover:bg-[#f6f7f9] hover:text-ink dark:bg-panel dark:hover:bg-panel-raised"

  return (
    <nav className="flex min-h-0 flex-1 flex-col items-center gap-2 py-1">
      <Button
        type="button"
        variant="outline"
        size="icon"
        className="size-[30px] rounded-full border-line bg-white text-muted-foreground shadow-none hover:bg-[#f6f7f9] hover:text-ink dark:bg-panel dark:hover:bg-panel-raised"
        title="展开工作区栏"
        aria-label="展开工作区栏"
        onClick={() => setSidebarCollapsed(false)}
      >
        <ChevronRight className="size-4" />
      </Button>
      <Button
        type="button"
        variant="outline"
        size="icon"
        className={iconBtn}
        title="打开工作区"
        onClick={onOpenWorkspace}
      >
        <FolderOpen className="size-4" />
      </Button>
    </nav>
  )
}
