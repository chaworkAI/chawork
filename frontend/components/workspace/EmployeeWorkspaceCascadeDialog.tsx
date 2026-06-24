import { useEffect, useMemo, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { FolderPlus, Loader2, RefreshCcw, UserPlus, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { isOrdinaryEmployee } from "@/lib/employeeDream"
import { pathKeysEqual } from "@/lib/formatPath"
import { sidebarItemsForEmployee } from "@/lib/employeeWorkspaceScope"
import { useUiLabel } from "@/hooks/useUiLabel"
import * as ipc from "@/lib/tauri"
import { useEmployeeStore } from "@/stores/employee"
import type { WorkspaceMembership } from "@/types/employee"
import type { WorkspaceSidebarItem } from "@/types/workspace"

export interface EmployeeWorkspaceCascadeDialogProps {
  open: boolean
  preferredEmployeeId?: string | null
  allWorkspaceItems: WorkspaceSidebarItem[]
  activeWorkspaceId: string | null
  onOpenChange: (open: boolean) => void
  onWorkspaceSelect?: (workspacePath: string) => void
  onAddWorkspace?: (employeeId: string) => void | Promise<void>
}

export function EmployeeWorkspaceCascadeDialog({
  open,
  preferredEmployeeId,
  allWorkspaceItems,
  activeWorkspaceId,
  onOpenChange,
  onWorkspaceSelect,
  onAddWorkspace,
}: EmployeeWorkspaceCascadeDialogProps) {
  const getLabel = useUiLabel()
  const employees = useEmployeeStore((s) => s.employees)
  const loadEmployees = useEmployeeStore((s) => s.loadEmployees)
  const openCreateDialog = useEmployeeStore((s) => s.openCreateDialog)
  const createdEmployeeId = useEmployeeStore((s) => s.selectedEmployeeId)
  const isLoading = useEmployeeStore((s) => s.isLoading)
  const storeError = useEmployeeStore((s) => s.error)

  const [selectedEmployeeId, setSelectedEmployeeId] = useState<string | null>(null)
  const [memberships, setMemberships] = useState<WorkspaceMembership[]>([])
  const [membershipsLoading, setMembershipsLoading] = useState(false)
  const [submitting, setSubmitting] = useState(false)

  const ordinaryEmployees = useMemo(() => {
    const ordinary = employees.filter(
      (e) => isOrdinaryEmployee(e) && e.status === "active",
    )
    return [
      ...ordinary.filter((e) => e.id === "general"),
      ...ordinary.filter((e) => e.id !== "general"),
    ]
  }, [employees])

  const hasGeneral = ordinaryEmployees.some((e) => e.id === "general")

  const scopedItems = useMemo(
    () =>
      sidebarItemsForEmployee(
        allWorkspaceItems,
        selectedEmployeeId,
        memberships,
      ),
    [allWorkspaceItems, selectedEmployeeId, memberships],
  )

  const activeWorkspacePath = useMemo(
    () =>
      allWorkspaceItems.find((item) => item.workspace.id === activeWorkspaceId)
        ?.workspace.path ?? null,
    [allWorkspaceItems, activeWorkspaceId],
  )

  const activeWorkspacePathForEmployee = useMemo(() => {
    if (!activeWorkspacePath || !selectedEmployeeId) return null
    const belongsToSelectedEmployee = scopedItems.some((item) =>
      pathKeysEqual(item.workspace.path, activeWorkspacePath),
    )
    return belongsToSelectedEmployee ? activeWorkspacePath : null
  }, [activeWorkspacePath, scopedItems, selectedEmployeeId])

  useEffect(() => {
    if (!open) return
    void loadEmployees()
  }, [open, loadEmployees])

  useEffect(() => {
    if (!open) return
    const preferred =
      preferredEmployeeId &&
      ordinaryEmployees.some((e) => e.id === preferredEmployeeId)
        ? preferredEmployeeId
        : null
    const activeItem = allWorkspaceItems.find(
      (item) => item.workspace.id === activeWorkspaceId,
    )
    const fromActive =
      activeItem?.workspace.bound_employee_id &&
      ordinaryEmployees.some((e) => e.id === activeItem.workspace.bound_employee_id)
        ? activeItem.workspace.bound_employee_id
        : null
    setSelectedEmployeeId(
      preferred ??
        fromActive ??
        (hasGeneral ? "general" : ordinaryEmployees[0]?.id ?? null),
    )
  }, [open, preferredEmployeeId, ordinaryEmployees, hasGeneral, activeWorkspaceId, allWorkspaceItems])

  useEffect(() => {
    if (!open || !createdEmployeeId) return
    if (ordinaryEmployees.some((e) => e.id === createdEmployeeId)) {
      setSelectedEmployeeId(createdEmployeeId)
    }
  }, [open, createdEmployeeId, ordinaryEmployees])

  useEffect(() => {
    if (!open || !selectedEmployeeId) {
      setMemberships([])
      return
    }
    setMemberships([])
    let cancelled = false
    setMembershipsLoading(true)
    void ipc.listWorkspacesForEmployee(selectedEmployeeId).then(
      (rows) => {
        if (!cancelled) setMemberships(rows)
      },
      () => {
        if (!cancelled) setMemberships([])
      },
    ).finally(() => {
      if (!cancelled) setMembershipsLoading(false)
    })
    return () => {
      cancelled = true
    }
  }, [open, selectedEmployeeId])

  const handleAddWorkspace = async () => {
    if (!selectedEmployeeId) return
    setSubmitting(true)
    try {
      await onAddWorkspace?.(selectedEmployeeId)
      onOpenChange(false)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-80 bg-[rgba(36,40,50,0.28)]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-81 flex max-h-[min(640px,calc(100dvh-48px))] w-[min(720px,calc(100vw-48px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none">
          <header className="flex shrink-0 items-start justify-between gap-4 border-b border-line-soft px-[22px] py-5">
            <div className="min-w-0 flex-1">
              <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
                {getLabel("workspace.cascade.kicker", "工作区")}
              </p>
              <Dialog.Title className="mt-[3px] text-[21px] font-extrabold tracking-normal text-ink">
                {getLabel("workspace.cascade.title", "选择员工与工作区")}
              </Dialog.Title>
              <Dialog.Description className="mt-1 text-[13px] text-muted-foreground">
                {getLabel(
                  "workspace.cascade.description",
                  "先选员工，再选该员工下的工作区文件夹。",
                )}
              </Dialog.Description>
            </div>
            <Button
              type="button"
              variant="outline"
              size="icon-sm"
              className="size-[38px] shrink-0 rounded-[12px] bg-white"
              aria-label={getLabel("common.close", "关闭")}
              onClick={() => onOpenChange(false)}
            >
              <X className="size-4" />
            </Button>
          </header>

          {!hasGeneral ? (
            <div className="px-[22px] py-[22px]">
              <div className="rounded-[12px] border border-danger/25 bg-danger/10 p-3 text-[12px] leading-5 text-danger">
                Root employee infrastructure 未返回通用员工 general。请重新检测 root 初始化状态后再打开工作区。
                <div className="mt-3">
                  <Button type="button" size="sm" variant="outline" onClick={() => void loadEmployees()}>
                    <RefreshCcw className="mr-1.5 size-3.5" />
                    重新检测
                  </Button>
                </div>
              </div>
            </div>
          ) : (
            <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,220px)_minmax(0,1fr)] divide-x divide-line-soft">
              <section className="flex min-h-0 flex-col px-3 py-4">
                <div className="mb-3 flex items-center justify-between gap-2 px-1">
                  <p className="text-[15px] font-extrabold text-ink">
                    {getLabel("workspace.cascade.employees", "员工")}
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-[30px] shrink-0 rounded-[10px] px-2.5 text-[12px] font-semibold text-muted-foreground hover:text-ink"
                    disabled={!hasGeneral || isLoading}
                    onClick={() => openCreateDialog()}
                  >
                    <UserPlus className="mr-1 size-3.5" />
                    {getLabel("workspace.cascade.new_employee", "新建")}
                  </Button>
                </div>
                <div className="min-h-0 flex-1 space-y-1.5 overflow-y-auto">
                  {ordinaryEmployees.map((employee) => {
                    const active = employee.id === selectedEmployeeId
                    return (
                      <button
                        key={employee.id}
                        type="button"
                        className={`flex w-full items-center justify-between rounded-[12px] border px-3 py-2.5 text-left transition-colors ${
                          active
                            ? "border-accent/40 bg-[#f4f1ed]"
                            : "border-line-soft bg-[#f8f9fb] hover:bg-[#eef1f5]"
                        }`}
                        onClick={() => setSelectedEmployeeId(employee.id)}
                      >
                        <span className="min-w-0">
                          <span className="flex min-w-0 items-center gap-1.5">
                            <span className="truncate text-[13px] font-medium text-ink">
                              {employee.name}
                            </span>
                            {employee.id === "general" ? (
                              <span className="shrink-0 rounded-full border border-accent/25 bg-accent/10 px-1.5 py-0.5 text-[10px] font-semibold text-accent">
                                {getLabel(
                                  "workspace.cascade.general_recommended_badge",
                                  "推荐 · 首次使用",
                                )}
                              </span>
                            ) : null}
                          </span>
                          <span className="mt-0.5 block truncate font-mono text-[10px] text-muted-foreground">
                            {employee.id}
                          </span>
                        </span>
                      </button>
                    )
                  })}
                </div>
              </section>

              <section className="flex min-h-0 flex-col px-4 py-4">
                <div className="mb-3 flex items-center justify-between gap-2">
                  <p className="text-[15px] font-extrabold text-ink">
                    {getLabel("workspace.cascade.workspaces", "工作区")}
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-[30px] shrink-0 rounded-[10px] px-2.5 text-[12px] font-semibold text-muted-foreground hover:text-ink"
                    disabled={!selectedEmployeeId || submitting || isLoading || !hasGeneral}
                    onClick={() => void handleAddWorkspace()}
                  >
                    {submitting ? (
                      <Loader2 className="mr-1 size-3.5 animate-spin" />
                    ) : (
                      <FolderPlus className="mr-1 size-3.5" />
                    )}
                    {getLabel("workspace.cascade.new_workspace", "新建")}
                  </Button>
                </div>
                <div className="min-h-0 flex-1 overflow-y-auto">
                  {membershipsLoading ? (
                    <div className="flex items-center gap-2 px-1 py-4 text-[12px] text-muted-foreground">
                      <Loader2 className="size-3.5 animate-spin" />
                      {getLabel("workspace.cascade.loading_workspaces", "加载工作区…")}
                    </div>
                  ) : scopedItems.length === 0 ? (
                    <p className="rounded-[12px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3 text-[12px] leading-5 text-muted-foreground">
                      {getLabel(
                        "workspace.cascade.empty_workspaces",
                        "该员工还没有工作区。点击右侧「新建」添加文件夹。",
                      )}
                    </p>
                  ) : (
                    <div className="space-y-1.5">
                      {scopedItems.map((item) => {
                        const active =
                          activeWorkspacePathForEmployee !== null &&
                          pathKeysEqual(item.workspace.path, activeWorkspacePathForEmployee)
                        return (
                          <button
                            key={item.workspace.path}
                            type="button"
                            className={`flex w-full items-center justify-between rounded-[12px] border px-3 py-2.5 text-left transition-colors ${
                              active
                                ? "border-accent/40 bg-[#f4f1ed]"
                                : "border-line-soft bg-[#f8f9fb] hover:bg-[#eef1f5]"
                            }`}
                            onClick={() => {
                              if (!active) onWorkspaceSelect?.(item.workspace.path)
                              onOpenChange(false)
                            }}
                          >
                            <span className="min-w-0">
                              <span className="block truncate text-[13px] font-medium text-ink">
                                {item.workspace.name}
                              </span>
                              <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
                                {item.metaLine}
                              </span>
                            </span>
                            <span
                              className={
                                active
                                  ? "size-2.5 shrink-0 rounded-full bg-success"
                                  : "size-2.5 shrink-0 rounded-full border border-line"
                              }
                            />
                          </button>
                        )
                      })}
                    </div>
                  )}
                </div>
                {storeError ? (
                  <p className="mt-2 text-[11px] text-danger">{storeError}</p>
                ) : null}
              </section>
            </div>
          )}

          {hasGeneral ? (
            <footer className="flex shrink-0 items-center justify-end border-t border-line-soft px-[22px] py-4">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-[36px] rounded-[12px] px-4"
                onClick={() => onOpenChange(false)}
              >
                {getLabel("common.cancel", "取消")}
              </Button>
            </footer>
          ) : null}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
