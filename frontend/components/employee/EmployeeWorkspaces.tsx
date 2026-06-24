import { FolderOpen, Unlink } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { useEmployeeStore } from "@/stores/employee"
import { useLocaleStore } from "@/stores/locale"
import { useWorkspaceStore } from "@/stores/workspace"

export function EmployeeWorkspaces() {
  const t = useUiLabel()
  const appLocale = useLocaleStore((s) => s.locale)
  const dateLocale = appLocale === "zh-CN" ? "zh-CN" : "en-US"

  const selectedEmployeeId = useEmployeeStore((s) => s.selectedEmployeeId)
  const workspaces = useEmployeeStore((s) => s.selectedWorkspaces)
  const unbindWorkspace = useEmployeeStore((s) => s.unbindWorkspace)
  const openWorkspaceDialog = useWorkspaceStore((s) => s.openWorkspaceDialog)

  const handleUnbind = async (path: string, name: string) => {
    if (
      !window.confirm(
        applyLabelTemplate(
          t(
            "employee.workspaces.confirm_unbind",
            "确定解绑工作区「{{name}}」？解绑后此工作区将不再使用当前员工的 Prompt 和 Skills。",
          ),
          { name },
        ),
      )
    ) {
      return
    }
    await unbindWorkspace(path)
  }

  if (!selectedEmployeeId) return null

  return (
    <div className="grid gap-4">
      <div className="flex items-center justify-between">
        <h3 className="text-[14px] font-bold text-ink">
          {t("employee.workspaces.bound_title", "已绑定工作区")}
        </h3>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-[36px] rounded-[12px] bg-white px-4"
          onClick={() => void openWorkspaceDialog(selectedEmployeeId)}
        >
          <FolderOpen className="mr-1.5 size-3.5" />
          {t("employee.workspaces.add_workspace", "添加工作区")}
        </Button>
      </div>

      {workspaces.length === 0 ? (
        <p className="py-8 text-center text-[13px] text-muted-foreground">
          {t(
            "employee.workspaces.empty_hint",
            "该员工还没有专属工作区。点击「添加工作区」选择或新建文件夹。",
          )}
        </p>
      ) : (
        <div className="grid gap-2">
          {workspaces.map((ws) => (
            <div
              key={ws.id}
              className="flex items-center justify-between gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3"
            >
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px] font-medium text-ink">{ws.name}</p>
                <p className="truncate font-mono text-[11px] text-muted-foreground">{ws.path}</p>
                <p className="text-[11px] text-muted-foreground">
                  {applyLabelTemplate(
                    t("employee.workspaces.bound_at", "绑定于 {{time}}"),
                    { time: new Date(ws.added_at).toLocaleString(dateLocale) },
                  )}
                </p>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="rounded-[10px]"
                title={t("employee.workspaces.unbind", "解绑此工作区")}
                onClick={() => void handleUnbind(ws.path, ws.name)}
              >
                <Unlink className="size-3.5 text-muted-foreground" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
