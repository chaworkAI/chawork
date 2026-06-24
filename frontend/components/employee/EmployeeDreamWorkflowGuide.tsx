import { Clock, Info, UserRound } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { pickDreamConfigTargetEmployee } from "@/lib/employeeDream"
import { useEmployeeStore } from "@/stores/employee"
import { useRootConfigStore } from "@/stores/rootConfig"
import { useWorkspaceStore } from "@/stores/workspace"

export function EmployeeDreamWorkflowGuide() {
  const label = useUiLabel()
  const employees = useEmployeeStore((s) => s.employees)
  const activeBinding = useWorkspaceStore((s) => s.activeBinding)
  const selectEmployee = useEmployeeStore((s) => s.selectEmployee)
  const setActiveTab = useEmployeeStore((s) => s.setActiveTab)
  const openSettingsPanel = useRootConfigStore((s) => s.openSettingsPanel)

  const preferredEmployeeId =
    activeBinding?.status === "bound" ? activeBinding.employee_id : null
  const targetEmployeeId = pickDreamConfigTargetEmployee(
    employees,
    preferredEmployeeId,
  )
  const targetEmployee = employees.find((entry) => entry.id === targetEmployeeId)

  return (
    <section className="grid gap-4 rounded-[15px] border border-[#d5e4d8] bg-[#f3faf4] p-4">
      <div className="flex items-start gap-3">
        <Info className="mt-0.5 size-4 shrink-0 text-primary" />
        <div className="min-w-0 grid gap-2">
          <h3 className="text-[14px] font-bold text-ink">
            {label("employee.dream.workflow.title", "Dream Workflow 为系统内置执行器")}
          </h3>
          <p className="text-[13px] leading-6 text-muted-foreground">
            {label(
              "employee.dream.workflow.body",
              "它负责执行普通员工的 Dream 分析，提示词与执行逻辑由系统内置管理，不能在这里配置 dream.yaml 或调度。请在普通员工的「Dream」标签页启用定时做梦；全局默认触发时间可在设置 → Dream 中修改。",
            )}
          </p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          size="sm"
          className="h-[36px] rounded-[12px] px-4"
          disabled={!targetEmployeeId}
          onClick={() => {
            if (!targetEmployeeId) return
            void selectEmployee(targetEmployeeId).then(() => setActiveTab("dream"))
          }}
        >
          <UserRound className="mr-1.5 size-3.5" />
          {targetEmployee
            ? applyLabelTemplate(
                label("employee.dream.workflow.open_employee", "配置 {{name}} 的 Dream"),
                { name: targetEmployee.name },
              )
            : label("employee.dream.workflow.open_first", "配置普通员工 Dream")}
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-[36px] rounded-[12px] bg-white px-4"
          onClick={() => openSettingsPanel("dream")}
        >
          <Clock className="mr-1.5 size-3.5" />
          {label("employee.dream.workflow.open_global", "全局默认时间")}
        </Button>
      </div>
    </section>
  )
}
