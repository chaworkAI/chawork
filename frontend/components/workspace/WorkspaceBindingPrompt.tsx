import { useCallback, useEffect, useMemo, useState } from "react"
import { AlertTriangle, Loader2, Sparkles, UserPlus, Users } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { useEmployeeStore } from "@/stores/employee"
import type { BindingValidation } from "@/types/employee"

export interface WorkspaceBindingPromptProps {
  workspacePath: string
  binding: BindingValidation
  isLoading?: boolean
  onGeneralBindActionReady?: (action: (() => void) | null) => void
}

export function WorkspaceBindingPrompt({
  workspacePath,
  binding,
  isLoading = false,
  onGeneralBindActionReady,
}: WorkspaceBindingPromptProps) {
  const getLabel = useUiLabel()
  const employees = useEmployeeStore((s) => s.employees)
  const loadEmployees = useEmployeeStore((s) => s.loadEmployees)
  const bindWorkspace = useEmployeeStore((s) => s.bindWorkspace)
  const unbindWorkspace = useEmployeeStore((s) => s.unbindWorkspace)
  const openCreateDialogForWorkspace = useEmployeeStore(
    (s) => s.openCreateDialogForWorkspace,
  )
  const openPanel = useEmployeeStore((s) => s.openPanel)

  const [showPicker, setShowPicker] = useState(false)
  const [bindingAction, setBindingAction] = useState(false)

  useEffect(() => {
    if ((showPicker || binding.status === "unbound") && employees.length === 0) {
      void loadEmployees()
    }
  }, [binding.status, showPicker, employees.length, loadEmployees])

  const ordinaryEmployees = useMemo(
    () => employees.filter((e) => e.kind === "ordinary" && e.status === "active"),
    [employees],
  )
  const generalEmployee = ordinaryEmployees.find((e) => e.id === "general")

  const handleBind = useCallback(
    async (employeeId: string) => {
      setBindingAction(true)
      try {
        await bindWorkspace(employeeId, workspacePath)
        setShowPicker(false)
      } finally {
        setBindingAction(false)
      }
    },
    [bindWorkspace, workspacePath],
  )

  const handleGeneralBind = useCallback(() => {
    void handleBind("general")
  }, [handleBind])

  useEffect(() => {
    if (!onGeneralBindActionReady) return
    if (binding.status === "unbound" && generalEmployee && !bindingAction && !isLoading) {
      onGeneralBindActionReady(handleGeneralBind)
      return () => onGeneralBindActionReady(null)
    }
    onGeneralBindActionReady(null)
    return undefined
  }, [
    binding.status,
    bindingAction,
    generalEmployee,
    handleGeneralBind,
    isLoading,
    onGeneralBindActionReady,
  ])

  const handleRebind = useCallback(async () => {
    if (!binding.employee_id) return
    setBindingAction(true)
    try {
      await bindWorkspace(binding.employee_id, workspacePath)
    } finally {
      setBindingAction(false)
    }
  }, [bindWorkspace, binding.employee_id, workspacePath])

  const handleUnbind = useCallback(async () => {
    setBindingAction(true)
    try {
      await unbindWorkspace(workspacePath)
    } finally {
      setBindingAction(false)
    }
  }, [unbindWorkspace, workspacePath])

  if (binding.status === "bound") {
    return null
  }

  const title =
    binding.status === "unbound"
      ? "此工作区尚未绑定员工"
      : binding.status === "path_mismatch"
        ? "工作区路径已变更"
        : "工作区员工绑定需要修复"

  return (
    <section
      className="mx-4 mb-2 rounded-[14px] border border-amber-200/80 bg-[rgba(255,248,237,0.95)] px-4 py-3.5 shadow-[inset_0_1px_0_rgba(255,255,255,0.65)]"
      role="status"
      aria-live="polite"
    >
      <div className="mb-2 flex items-start gap-2">
        <AlertTriangle className="mt-0.5 size-4 shrink-0 text-amber-700" />
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-semibold text-ink">{title}</p>
          <p className="mt-1 text-[12px] leading-relaxed text-muted-foreground">
            {isLoading
              ? "正在检查绑定状态…"
              : binding.message ||
                "普通对话需要使用员工的 Prompt 和 Skills。请先绑定或创建员工。"}
          </p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2 pl-6">
        {binding.status === "unbound" ? (
          <>
            {generalEmployee ? (
              <Button
                type="button"
                data-tour-id="binding-general"
                size="sm"
                variant="default"
                disabled={bindingAction || isLoading}
                onClick={handleGeneralBind}
              >
                {bindingAction ? (
                  <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                ) : (
                  <Sparkles className="mr-1.5 size-3.5" />
                )}
                {getLabel("onboarding.binding.use_general", "使用通用员工")}
              </Button>
            ) : null}
            <Button
              type="button"
              size="sm"
              variant={generalEmployee ? "outline" : "default"}
              disabled={bindingAction || isLoading}
              onClick={() => setShowPicker((v) => !v)}
            >
              <Users className="mr-1.5 size-3.5" />
              {getLabel("onboarding.binding.other_employee", "绑定到其他员工")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={bindingAction || isLoading}
              onClick={() => openCreateDialogForWorkspace(workspacePath)}
            >
              <UserPlus className="mr-1.5 size-3.5" />
              {getLabel("onboarding.binding.create_employee", "创建员工并绑定")}
            </Button>
          </>
        ) : (
          <>
            {binding.employee_id ? (
              <Button
                type="button"
                size="sm"
                variant="default"
                disabled={bindingAction || isLoading}
                onClick={() => void handleRebind()}
              >
                {bindingAction ? (
                  <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                ) : null}
                重新绑定到 {binding.employee_name ?? binding.employee_id}
              </Button>
            ) : null}
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={bindingAction || isLoading}
              onClick={() => void handleUnbind()}
            >
              移除本地绑定
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={bindingAction || isLoading}
              onClick={() => setShowPicker(true)}
            >
              绑定到其他员工
            </Button>
          </>
        )}
        <Button
          type="button"
          size="sm"
          variant="ghost"
          onClick={() => openPanel("list")}
        >
          打开员工管理
        </Button>
      </div>

      {showPicker ? (
        <div className="mt-3 space-y-1.5 border-t border-amber-200/60 pt-3 pl-6">
          {ordinaryEmployees.length === 0 ? (
            <p className="text-[12px] text-muted-foreground">
              暂无普通员工，请先创建员工。
            </p>
          ) : (
            ordinaryEmployees.map((emp) => (
              <button
                key={emp.id}
                type="button"
                disabled={bindingAction}
                onClick={() => void handleBind(emp.id)}
                className="flex w-full items-center justify-between rounded-[12px] px-2.5 py-2 text-left text-[12px] transition-colors hover:bg-[#f8f9fb] disabled:opacity-50"
              >
                <span className="font-medium text-ink">{emp.name}</span>
                <span className="font-mono text-[11px] text-muted-foreground">
                  {emp.id}
                </span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </section>
  )
}
