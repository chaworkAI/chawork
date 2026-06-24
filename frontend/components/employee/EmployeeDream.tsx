import { useCallback, useEffect, useState } from "react"
import { Loader2, Play, StopCircle, AlertTriangle, CheckCircle2, XCircle, Settings2, ChevronDown, Clock } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { isOrdinaryEmployee } from "@/lib/employeeDream"
import { useDreamEvents } from "@/hooks/useDreamEvents"
import { useEmployeeStore } from "@/stores/employee"
import type { DreamConfig } from "@/types/employee"
import { EmployeeLogs } from "./EmployeeLogs"

function defaultDreamConfig(): DreamConfig {
  return {
    enabled: false,
    schedule: { type: "daily", time: null },
    session_scan: { scope: "all", workspace_subset: [], latest_sessions: 3 },
  }
}

function resolveScheduleType(config: DreamConfig | null | undefined): "manual" | "daily" {
  const raw = config?.schedule?.type
  return raw === "manual" ? "manual" : "daily"
}

export function EmployeeDream() {
  const label = useUiLabel()
  const selectedDetail = useEmployeeStore((s) => s.selectedDetail)
  const employeeId = selectedDetail?.registry_entry.id
  const dreamConfig = useEmployeeStore((s) =>
    employeeId && s.dreamStateEmployeeId === employeeId ? s.dreamConfig : null,
  )
  const dreamDefaults = useEmployeeStore((s) => s.dreamDefaults)
  const recentDreamResult = useEmployeeStore((s) =>
    employeeId && s.dreamStateEmployeeId === employeeId ? s.recentDreamResult : null,
  )
  const isDreamRunning = useEmployeeStore(
    (s) => employeeId != null && s.dreamRunningEmployeeId === employeeId,
  )
  const isDreamStateLoading = useEmployeeStore(
    (s) =>
      employeeId != null &&
      s.detailLoadingEmployeeId === employeeId &&
      s.dreamStateEmployeeId !== employeeId,
  )
  const updateDreamConfig = useEmployeeStore((s) => s.updateDreamConfig)
  const runDream = useEmployeeStore((s) => s.runDream)
  const error = useEmployeeStore((s) => s.error)
  const pendingRequest = useEmployeeStore((s) =>
    employeeId && s.dreamStateEmployeeId === employeeId ? s.pendingRequest : null,
  )
  const applyingEmployeeId = useEmployeeStore((s) => s.applyingEmployeeId)

  const activeReviewStatus = pendingRequest?.result.status?.toLowerCase()
  const dreamRunBlocked = Boolean(
    pendingRequest &&
      activeReviewStatus &&
      ["pending", "approved", "applying", "failed"].includes(activeReviewStatus),
  )
  const dreamRunBlockedReason = dreamRunBlocked
    ? label(
        "employee.dream.run_blocked_active_review",
        "请先处理 Review Queue 中的待审、进行中或失败的更新请求。",
      )
    : applyingEmployeeId === employeeId
      ? label("employee.dream.run_blocked_applying", "正在应用 prompt 更新，请稍候。")
      : undefined

  const [logsExpanded, setLogsExpanded] = useState(false)
  const [dreamOutput, setDreamOutput] = useState("")
  const [dreamError, setDreamError] = useState<string | null>(null)

  useDreamEvents(
    {
      onDelta: useCallback((text: string) => {
        setDreamOutput((prev) => prev + text)
      }, []),
      onDone: useCallback((fullText: string) => {
        setDreamOutput(fullText)
      }, []),
      onError: useCallback((message: string) => {
        setDreamError(message)
        if (employeeId && useEmployeeStore.getState().dreamRunningEmployeeId === employeeId) {
          useEmployeeStore.setState({ dreamRunningEmployeeId: null })
        }
      }, [employeeId]),
    },
    employeeId,
  )

  useEffect(() => {
    setDreamOutput("")
    setDreamError(null)
  }, [employeeId])

  const isOrdinary = isOrdinaryEmployee(selectedDetail?.registry_entry)

  if (!employeeId || !isOrdinary) {
    return (
      <p className="py-12 text-center text-[13px] text-muted-foreground">
        {label(
          "employee.dream.ordinary_only",
          "Dream 调度仅适用于普通员工。Dream Workflow 为系统内置执行器，请在概览页查看说明。",
        )}
      </p>
    )
  }

  if (isDreamStateLoading) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-16">
        <Loader2 className="size-6 animate-spin text-primary" />
        <p className="text-[13px] text-muted-foreground">
          {label("employee.dream.loading", "加载 Dream 状态...")}
        </p>
      </div>
    )
  }

  const globalDefaultTime = dreamDefaults?.default_dream_time ?? "09:00"

  const handleConfigChange = (patch: Partial<DreamConfig>) => {
    const current: DreamConfig = dreamConfig ?? defaultDreamConfig()
    const next: DreamConfig = {
      ...current,
      ...patch,
      schedule: { ...current.schedule, ...(patch.schedule ?? {}) },
      session_scan: { ...current.session_scan, ...(patch.session_scan ?? {}) },
    }
    void updateDreamConfig(employeeId, next)
  }

  const handleScheduleChange = (type: "manual" | "daily") => {
    if (resolveScheduleType(dreamConfig) === type) return
    handleConfigChange({
      schedule:
        type === "manual"
          ? { type: "manual", time: null }
          : { type: "daily", time: dreamConfig?.schedule?.time ?? null },
    })
  }

  const handleRunDream = async () => {
    setDreamOutput("")
    setDreamError(null)
    await runDream(employeeId)
  }

  const handleCancelDream = async () => {
    useEmployeeStore.setState({ dreamRunningEmployeeId: null })
    try {
      const { cancelDreamRun } = await import("@/lib/tauri")
      await cancelDreamRun()
    } catch {
      // ignore
    }
  }

  const cfg = dreamConfig ?? defaultDreamConfig()
  const scheduleType = resolveScheduleType(cfg)

  return (
    <div className="grid gap-4">
      {/* Dream Config */}
      <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <h3 className="flex items-center gap-1.5 text-[14px] font-bold text-ink">
          <Settings2 className="size-4" />
          {label("employee.dream.config_title", "Dream 配置")}
        </h3>
        <div className="grid gap-4">
          {/* Enabled toggle */}
          <div className="flex items-center justify-between">
            <div>
              <p className="text-[13px] font-medium">
                {label("employee.dream.enable_title", "启用 Dream")}
              </p>
              <p className="text-[11px] text-muted-foreground">
                {label(
                  "employee.dream.enable_hint",
                  "开启后该员工将参与 Dream prompt 改进流程",
                )}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={cfg.enabled}
              onClick={() => handleConfigChange({ enabled: !cfg.enabled })}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 ${
                cfg.enabled ? "bg-primary" : "bg-[rgba(0,0,0,0.15)]"
              }`}
            >
              <span
                className={`pointer-events-none block size-5 rounded-full bg-white shadow transition-transform ${
                  cfg.enabled ? "translate-x-5" : "translate-x-0.5"
                }`}
              />
            </button>
          </div>

          {/* Schedule type */}
          <div>
            <p className="mb-1.5 text-[13px] font-medium">
              {label("employee.dream.schedule_type", "调度方式")}
            </p>
            <div className="inline-flex rounded-[12px] border border-line-soft bg-white p-0.5">
              <button
                type="button"
                aria-pressed={scheduleType === "manual"}
                onClick={() => handleScheduleChange("manual")}
                className={`rounded-[10px] px-3 py-1.5 text-[12px] transition-colors focus:outline-none ${
                  scheduleType === "manual"
                    ? "bg-[#eef1f5] text-primary font-bold"
                    : "text-ink hover:text-primary"
                }`}
              >
                {label("employee.dream.schedule_manual", "手动触发")}
              </button>
              <button
                type="button"
                aria-pressed={scheduleType === "daily"}
                onClick={() => handleScheduleChange("daily")}
                className={`rounded-[10px] px-3 py-1.5 text-[12px] transition-colors focus:outline-none ${
                  scheduleType === "daily"
                    ? "bg-[#eef1f5] text-primary font-bold"
                    : "text-ink hover:text-primary"
                }`}
              >
                {label("employee.dream.schedule_daily", "每日定时")}
              </button>
            </div>
            {scheduleType === "manual" && (
              <p className="mt-1 text-[11px] text-muted-foreground">
                {label(
                  "employee.dream.schedule_manual_hint",
                  "通过下方「运行 Dream」入口手动触发。",
                )}
              </p>
            )}
          </div>

          {/* Daily time picker */}
          {scheduleType === "daily" && (
            <div>
              <p className="mb-1.5 text-[13px] font-medium flex items-center gap-1">
                <Clock className="size-3.5" />
                {label("employee.dream.trigger_time", "触发时间")}
              </p>
              <div className="flex items-center gap-2">
                <input
                  type="time"
                  value={cfg.schedule.time ?? ""}
                  onChange={(e) => {
                    const val = e.target.value
                    handleConfigChange({ schedule: { ...cfg.schedule, time: val || null } })
                  }}
                  className="min-h-[42px] rounded-[12px] border border-line bg-white px-3 text-[13px] text-ink focus:border-primary focus:outline-none"
                />
                {!cfg.schedule.time && (
                  <span className="rounded-[10px] border border-line-soft bg-white px-2.5 py-1.5 text-[11px] text-muted-foreground">
                    {applyLabelTemplate(
                      label("employee.dream.global_default_badge", "全局默认 {{time}}"),
                      { time: globalDefaultTime },
                    )}
                  </span>
                )}
                {cfg.schedule.time && (
                  <button
                    type="button"
                    onClick={() => handleConfigChange({ schedule: { ...cfg.schedule, time: null } })}
                    className="rounded-[10px] px-2.5 py-1.5 text-[11px] text-muted-foreground underline transition-colors hover:bg-white hover:text-ink"
                  >
                    {label("employee.dream.restore_global_default", "恢复全局默认")}
                  </button>
                )}
              </div>
              <p className="mt-1 text-[11px] text-muted-foreground">
                {cfg.schedule.time
                  ? applyLabelTemplate(
                      label(
                        "employee.dream.daily_at_time",
                        "该员工将在每日 {{time}} 自动触发 Dream",
                      ),
                      { time: cfg.schedule.time },
                    )
                  : applyLabelTemplate(
                      label(
                        "employee.dream.daily_use_global",
                        "使用全局默认时间 {{time}}，可在全局设置 → Dream 中修改",
                      ),
                      { time: globalDefaultTime },
                    )}
              </p>
            </div>
          )}

          {/* Session scan scope */}
          <div>
            <p className="mb-1.5 text-[13px] font-medium">
              {label("employee.dream.session_scan", "会话扫描范围")}
            </p>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() =>
                  handleConfigChange({ session_scan: { ...cfg.session_scan, scope: "all", workspace_subset: [] } })
                }
                className={`rounded-[12px] border px-3 py-2 text-[12px] transition-colors ${
                  cfg.session_scan.scope === "all"
                    ? "border-[#d5e4d8] bg-white font-bold text-primary"
                    : "border-line bg-white text-ink hover:bg-[#f8f9fb]"
                }`}
              >
                {label("employee.dream.scan_all", "全部工作区")}
              </button>
              <button
                type="button"
                onClick={() => handleConfigChange({ session_scan: { ...cfg.session_scan, scope: "selected" } })}
                className={`rounded-[12px] border px-3 py-2 text-[12px] transition-colors ${
                  cfg.session_scan.scope === "selected"
                    ? "border-[#d5e4d8] bg-white font-bold text-primary"
                    : "border-line bg-white text-ink hover:bg-[#f8f9fb]"
                }`}
              >
                {label("employee.dream.scan_selected", "指定工作区")}
              </button>
            </div>
            {cfg.session_scan.scope === "selected" && (
              <p className="mt-1.5 text-[11px] text-muted-foreground">
                {label(
                  "employee.dream.scan_selected_hint",
                  "指定工作区功能将在后续版本中提供可视化选择器。当前通过 dream.yaml 手动配置 workspace_subset。",
                )}
              </p>
            )}
          </div>
        </div>
      </section>

      {/* Run Dream */}
      <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <h3 className="text-[14px] font-bold text-ink">
          {label("employee.dream.run_title", "运行 Dream")}
        </h3>
        <p className="text-[12px] text-muted-foreground">
          {label(
            "employee.dream.run_hint",
            "Dream 会分析该员工最近的会话记录，生成 prompt 改进建议。",
          )}
        </p>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            size="sm"
            className="h-[36px] rounded-[12px] px-4"
            disabled={isDreamRunning || dreamRunBlocked || applyingEmployeeId === employeeId}
            title={dreamRunBlockedReason}
            onClick={handleRunDream}
          >
            {isDreamRunning ? (
              <Loader2 className="mr-1.5 size-3.5 animate-spin" />
            ) : (
              <Play className="mr-1.5 size-3.5" />
            )}
            {isDreamRunning
              ? label("employee.dream.running", "Dream 运行中...")
              : label("employee.dream.run_button", "运行 Dream")}
          </Button>
          {dreamRunBlockedReason ? (
            <p className="text-[11px] text-muted-foreground">{dreamRunBlockedReason}</p>
          ) : null}
          {isDreamRunning && (
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-[36px] rounded-[12px] bg-white px-4"
              onClick={handleCancelDream}
            >
              <StopCircle className="mr-1.5 size-3.5" />
              {label("employee.dream.cancel", "取消")}
            </Button>
          )}
        </div>
      </section>

      {/* Dream Runtime Output */}
      {(isDreamRunning || dreamOutput) && (
        <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <h4 className="mb-2 text-[13px] font-bold">
            {isDreamRunning
              ? label("employee.dream.output_live", "Dream 输出（实时）")
              : label("employee.dream.output_done", "Dream 执行输出")}
          </h4>
          <pre className="max-h-[300px] overflow-auto whitespace-pre-wrap rounded-[13px] border border-line-soft bg-white p-3 font-mono text-[11px] text-ink">
            {dreamOutput || label("employee.dream.waiting_codex", "等待 Codex 响应...")}
            {isDreamRunning && <span className="animate-pulse">▌</span>}
          </pre>
        </section>
      )}

      {/* Dream Error */}
      {(dreamError || error) && (
        <section className="rounded-[15px] border border-danger/30 bg-danger/5 p-3.5">
          <div className="flex items-center gap-2 mb-1">
            <XCircle className="size-4 text-danger" />
            <span className="text-[13px] font-medium text-danger">
              {label("employee.dream.error_title", "Dream 执行出错")}
            </span>
          </div>
          <p className="text-[12px] text-danger/80">{dreamError ?? error}</p>
        </section>
      )}

      {/* Recent Result */}
      {recentDreamResult && (
        <section>
          <h3 className="mb-3 text-[14px] font-bold">
            {label("employee.dream.recent_title", "最近 Dream 结果")}
          </h3>
          <div className="grid gap-2 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
            <div className="flex items-center gap-2">
              {recentDreamResult.parse_failed ? (
                <XCircle className="size-4 text-danger" />
              ) : recentDreamResult.decision === "update_required" ? (
                <AlertTriangle className="size-4 text-amber-600" />
              ) : (
                <CheckCircle2 className="size-4 text-emerald-600" />
              )}
              <span className="text-[13px] font-medium">
                {recentDreamResult.parse_failed
                  ? label("employee.dream.result_parse_failed", "解析失败")
                  : recentDreamResult.decision === "update_required"
                    ? label("employee.dream.result_update_required", "建议更新")
                    : label("employee.dream.result_no_update", "无需更新")}
              </span>
              <span className="text-[11px] text-muted-foreground">
                {recentDreamResult.dream_run_id}
              </span>
            </div>
            <p className="text-[12px] text-ink">{recentDreamResult.summary}</p>
            {recentDreamResult.source_sessions.length > 0 && (
              <p className="text-[11px] text-muted-foreground">
                {applyLabelTemplate(
                  label("employee.dream.sessions_analyzed", "基于 {{count}} 个会话分析"),
                  { count: String(recentDreamResult.source_sessions.length) },
                )}
              </p>
            )}
            <p className="text-[11px] text-muted-foreground">
              {recentDreamResult.created_at}
            </p>
            {recentDreamResult.parse_failed && recentDreamResult.raw_output && (
              <details className="mt-2">
                <summary className="cursor-pointer text-[11px] text-muted-foreground hover:text-ink">
                  {label("employee.dream.view_raw_output", "查看原始输出")}
                </summary>
                <pre className="mt-1 max-h-[200px] overflow-auto rounded-[13px] border border-line-soft bg-white p-3 font-mono text-[11px] text-ink">
                  {recentDreamResult.raw_output}
                </pre>
              </details>
            )}
          </div>
        </section>
      )}

      {/* Dream Logs (collapsible) */}
      <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <button
          type="button"
          onClick={() => setLogsExpanded((v) => !v)}
          className="flex w-full items-center gap-1.5 text-[13px] font-bold text-ink hover:text-primary transition-colors"
        >
          <ChevronDown
            className={`size-4 transition-transform ${logsExpanded ? "" : "-rotate-90"}`}
          />
          {label("employee.dream.logs_toggle", "运行日志")}
        </button>
        {logsExpanded && (
          <div className="mt-3">
            <EmployeeLogs />
          </div>
        )}
      </section>
    </div>
  )
}
