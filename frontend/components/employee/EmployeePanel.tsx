import { useEffect, useMemo } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { Plus, AlertTriangle, Loader2, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { NotificationDot } from "@/components/ui/notification-dot"
import { cn } from "@/lib/utils"
import { isDreamWorkflowEmployee, pickDreamConfigTargetEmployee } from "@/lib/employeeDream"
import { useUiLabel } from "@/hooks/useUiLabel"
import { useEmployeeStore, type EmployeeTab } from "@/stores/employee"
import { useWorkspaceStore } from "@/stores/workspace"
import { EmployeeOverview } from "./EmployeeOverview"
import { EmployeePrompt } from "./EmployeePrompt"
import { EmployeeSkills } from "./EmployeeSkills"
import { EmployeeDream } from "./EmployeeDream"
import { EmployeeDreamWorkflowGuide } from "./EmployeeDreamWorkflowGuide"
import { EmployeeReviewQueue } from "./EmployeeReviewQueue"

const TAB_KEYS: { key: EmployeeTab; labelKey: string }[] = [
  { key: "overview", labelKey: "employee.tab.overview" },
  { key: "prompt", labelKey: "employee.tab.prompt" },
  { key: "skills", labelKey: "employee.tab.skills" },
  { key: "dream", labelKey: "employee.tab.dream" },
]

function TabContent({
  tab,
  isDreamWorkflow,
}: {
  tab: EmployeeTab
  isDreamWorkflow: boolean
}) {
  switch (tab) {
    case "overview":
      return (
        <div className="grid gap-4">
          {isDreamWorkflow ? <EmployeeDreamWorkflowGuide /> : null}
          <EmployeeOverview />
        </div>
      )
    case "prompt":
      return <EmployeePrompt />
    case "skills":
      return <EmployeeSkills />
    case "dream":
      return (
        <div className="grid gap-4">
          <EmployeeDream />
          <EmployeeReviewQueue />
        </div>
      )
  }
}

export function EmployeePanel() {
  const label = useUiLabel()
  const tabs = useMemo(
    () =>
      TAB_KEYS.map((tab) => ({
        key: tab.key,
        label: label(tab.labelKey, tab.labelKey),
      })),
    [label],
  )

  const panelOpen = useEmployeeStore((s) => s.panelOpen)
  const panelMode = useEmployeeStore((s) => s.panelMode)
  const closePanel = useEmployeeStore((s) => s.closePanel)
  const setPanelMode = useEmployeeStore((s) => s.setPanelMode)
  const employees = useEmployeeStore((s) => s.employees)
  const selectedEmployeeId = useEmployeeStore((s) => s.selectedEmployeeId)
  const selectEmployee = useEmployeeStore((s) => s.selectEmployee)
  const selectedDetail = useEmployeeStore((s) => s.selectedDetail)
  const activeTab = useEmployeeStore((s) => s.activeTab)
  const setActiveTab = useEmployeeStore((s) => s.setActiveTab)
  const openCreateDialog = useEmployeeStore((s) => s.openCreateDialog)
  const isLoading = useEmployeeStore((s) => s.isLoading)
  const error = useEmployeeStore((s) => s.error)
  const pendingReviewEmployeeIds = useEmployeeStore((s) => s.pendingReviewEmployeeIds)
  const activeBinding = useWorkspaceStore((s) => s.activeBinding)
  const isDreamWorkflow = isDreamWorkflowEmployee(selectedDetail?.registry_entry)
  const selectedHasPendingReview =
    selectedEmployeeId != null && pendingReviewEmployeeIds.includes(selectedEmployeeId)

  const preferredDreamEmployeeId =
    activeBinding?.status === "bound" ? activeBinding.employee_id : null

  useEffect(() => {
    if (panelOpen && employees.length > 0 && !selectedEmployeeId) {
      const targetId = pickDreamConfigTargetEmployee(
        employees,
        preferredDreamEmployeeId,
      )
      if (targetId) {
        void selectEmployee(targetId)
      }
    }
  }, [
    panelOpen,
    employees,
    selectedEmployeeId,
    selectEmployee,
    preferredDreamEmployeeId,
  ])

  const visibleTabs = useMemo(
    () => (isDreamWorkflow ? tabs.filter((tab) => tab.key !== "dream") : tabs),
    [isDreamWorkflow, tabs],
  )

  useEffect(() => {
    if (isDreamWorkflow && activeTab === "dream") {
      setActiveTab("overview")
    }
  }, [isDreamWorkflow, activeTab, setActiveTab])

  return (
    <>
      <Dialog.Root open={panelOpen} onOpenChange={(open) => !open && closePanel()}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-[110] bg-[rgba(36,40,50,0.28)]" />
          <Dialog.Content
            className={cn(
              "fixed left-1/2 top-1/2 z-[111] flex max-h-[min(760px,calc(100dvh-48px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none",
              panelMode === "detail"
                ? "w-[min(860px,calc(100vw-80px))]"
                : "w-[min(860px,calc(100vw-80px))]",
            )}
          >
            <header className="flex shrink-0 items-center justify-between gap-4 border-b border-line-soft px-[22px] py-5">
              <div className="min-w-0">
                <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
                  {panelMode === "detail"
                    ? label("employee.detail.kicker", "当前 AI 员工")
                    : label("employee.list.kicker", "AI 数字员工")}
                </p>
                <Dialog.Title className="mt-[3px] truncate text-[21px] font-extrabold tracking-normal text-ink">
                  {panelMode === "detail"
                    ? selectedDetail?.manifest?.name ??
                      selectedDetail?.registry_entry.name ??
                      label("employee.detail.loading_title", "员工详情")
                    : label("employee.list.modal_title", "员工列表")}
                </Dialog.Title>
                <Dialog.Description className="sr-only">
                  {label("employee.panel.description", "管理员工 Prompt、Skills、Dream 与工作区绑定")}
                </Dialog.Description>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <Dialog.Close asChild>
                  <button
                    type="button"
                    className="grid size-[38px] place-items-center rounded-[12px] border border-line bg-white text-[22px] text-muted-foreground transition-colors hover:bg-[#f8f9fb] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    aria-label={label("employee.panel.close", "关闭")}
                    onClick={closePanel}
                  >
                    <X className="size-4" />
                  </button>
                </Dialog.Close>
              </div>
            </header>

            {panelMode === "list" ? (
              <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto px-[22px] py-[22px]">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <strong className="block text-[15px] text-ink">
                      {label("employee.list.existing", "已有员工")}
                    </strong>
                    <span className="mt-0.5 block text-[13px] text-muted-foreground">
                      {label(
                        "employee.list.summary_hint",
                        "查看能力范围、项目绑定和待审批学习结果。",
                      )}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      type="button"
                      className="h-[36px] rounded-[12px] bg-primary px-4 text-[13px] font-bold text-primary-foreground hover:bg-primary/90"
                      onClick={openCreateDialog}
                    >
                      <Plus className="mr-1.5 size-3.5" />
                      {label("employee.list.create_employee", "新增员工")}
                    </Button>
                  </div>
                </div>

                {error && (
                  <div className="flex items-center gap-2 rounded-[13px] border border-danger/25 bg-danger/5 px-3 py-2 text-[12px] text-danger">
                    <AlertTriangle className="size-3.5 shrink-0" />
                    {error}
                  </div>
                )}

                <div className="grid gap-3">
                  {isLoading && employees.length === 0 ? (
                    <div className="grid min-h-[160px] place-items-center">
                      <Loader2 className="size-5 animate-spin text-muted-foreground" />
                    </div>
                  ) : null}
                  {employees.length === 0 && !isLoading ? (
                    <p className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-4 py-8 text-center text-[13px] text-muted-foreground">
                      {label("employee.list.empty", "暂无员工")}
                    </p>
                  ) : null}
                  {employees.map((emp) => {
                    const hasPendingReview = pendingReviewEmployeeIds.includes(emp.id)
                    const selected = selectedEmployeeId === emp.id
                    return (
                      <article
                        key={emp.id}
                        className={cn(
                          "grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-3 rounded-[15px] border px-3.5 py-3.5",
                          selected
                            ? "border-[#d5e4d8] bg-[#f3faf4]"
                            : "border-line-soft bg-[#f8f9fb]",
                        )}
                      >
                        <div className="min-w-0">
                          <strong className="block truncate text-[15px] text-ink">{emp.name}</strong>
                          <span className="mt-0.5 block truncate text-[12px] text-muted-foreground">
                            {emp.kind === "dream"
                              ? label("employee.kind.dream", "Dream 员工")
                              : label("employee.kind.ordinary", "普通员工")}
                            {" · "}
                            {emp.status}
                          </span>
                        </div>
                        <small className="min-w-[150px] text-right font-mono text-[11px] text-muted-foreground">
                          {hasPendingReview
                            ? label("employee.list.pending_review", "待审批 1")
                            : label("employee.list.no_pending_review", "待审批 0")}
                        </small>
                        <Button
                          type="button"
                          variant={selected ? "secondary" : "ghost"}
                          size="sm"
                          className={cn(
                            "relative h-[34px] rounded-[11px] px-3 text-[12px]",
                            selected
                              ? "border border-line bg-white"
                              : "border border-transparent hover:border-line hover:bg-white",
                          )}
                          onClick={() => {
                            void selectEmployee(emp.id)
                            setPanelMode("detail")
                          }}
                        >
                          {label("employee.list.view_detail", "查看详情")}
                          {hasPendingReview ? <NotificationDot className="-right-1 -top-1" /> : null}
                        </Button>
                      </article>
                    )
                  })}
                </div>
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
                {selectedDetail ? (
                  <>
                    <div className="flex shrink-0 flex-wrap gap-2 border-b border-line-soft px-[22px] py-3">
                      {visibleTabs.map((tab) => (
                        <button
                          key={tab.key}
                          type="button"
                          onClick={() => setActiveTab(tab.key)}
                          className={cn(
                            "relative min-h-[34px] rounded-full border px-3.5 text-[12px] font-bold transition-colors",
                            activeTab === tab.key
                              ? "border-[#cbdccc] bg-[#f3faf4] text-ink"
                              : "border-line bg-white text-muted-foreground hover:bg-[#f8f9fb] hover:text-ink",
                          )}
                        >
                          {tab.label}
                          {tab.key === "dream" && selectedHasPendingReview ? (
                            <NotificationDot className="-right-0.5 -top-0.5" />
                          ) : null}
                        </button>
                      ))}
                    </div>
                    <div className="min-h-0 flex-1 overflow-y-auto px-[22px] py-[22px]">
                      {error && (
                        <div className="mb-3 flex items-center gap-2 rounded-[13px] border border-danger/25 bg-danger/5 px-3 py-2 text-[12px] text-danger">
                          <AlertTriangle className="size-3.5 shrink-0" />
                          {error}
                        </div>
                      )}
                      <TabContent tab={activeTab} isDreamWorkflow={isDreamWorkflow} />
                    </div>
                  </>
                ) : (
                  <div className="flex flex-1 items-center justify-center">
                    {isLoading ? (
                      <Loader2 className="size-5 animate-spin text-muted-foreground" />
                    ) : (
                      <p className="text-[13px] text-muted-foreground">
                        {employees.length === 0
                          ? label("employee.list.create_hint", "点击左上角 + 创建第一个员工")
                          : label("employee.list.select_hint", "选择左侧员工查看详情")}
                      </p>
                    )}
                  </div>
                )}
              </div>
            )}
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </>
  )
}
