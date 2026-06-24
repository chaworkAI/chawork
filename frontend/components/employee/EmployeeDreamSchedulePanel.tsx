import { useCallback, useEffect, useMemo, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { Loader2, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { filterUserVisibleEmployees, isOrdinaryEmployee } from "@/lib/employeeDream"
import * as ipc from "@/lib/tauri"
import { useUiStore } from "@/stores/ui"
import type {
  DreamConfig,
  DreamDefaults,
  RecentDreamResult,
  RegistryEntry,
} from "@/types/employee"

interface DreamScheduleRow {
  employee: RegistryEntry
  description: string
  config: DreamConfig
  recent: RecentDreamResult | null
}

interface DraftState {
  enabled: boolean
  scheduleType: string
  time: string
  useGlobalTime: boolean
  latestSessions: number
}

const DEFAULT_DREAM_TIME = "09:00"

function defaultDreamConfig(): DreamConfig {
  return {
    enabled: false,
    schedule: {
      type: "daily",
      time: null,
    },
    session_scan: {
      scope: "all",
      workspace_subset: [],
      latest_sessions: 3,
    },
  }
}

function draftFromConfig(config: DreamConfig, defaults: DreamDefaults | null): DraftState {
  return {
    enabled: config.enabled,
    scheduleType: config.schedule.type || "daily",
    time: config.schedule.time || defaults?.default_dream_time || DEFAULT_DREAM_TIME,
    useGlobalTime: !config.schedule.time,
    latestSessions: config.session_scan.latest_sessions || 3,
  }
}

function configFromDraft(previous: DreamConfig, draft: DraftState): DreamConfig {
  return {
    ...previous,
    enabled: draft.enabled,
    schedule: {
      ...previous.schedule,
      type: draft.scheduleType,
      time: draft.useGlobalTime ? null : draft.time,
    },
    session_scan: {
      ...previous.session_scan,
      latest_sessions: Math.max(1, Math.min(20, Math.floor(draft.latestSessions || 1))),
    },
  }
}

function formatDateTime(value: string | null | undefined) {
  if (!value) return "无"
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  })
}

function nextDailyRunLabel(config: DreamConfig, defaults: DreamDefaults | null) {
  if (!config.enabled) return "开启后计算"
  if (config.schedule.type === "manual") return "手动触发"
  const time = config.schedule.time || defaults?.default_dream_time || DEFAULT_DREAM_TIME
  const [hoursRaw, minutesRaw] = time.split(":")
  const hours = Number(hoursRaw)
  const minutes = Number(minutesRaw)
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) return `每日 ${time}`
  const now = new Date()
  const next = new Date(now)
  next.setHours(hours, minutes, 0, 0)
  if (next <= now) next.setDate(next.getDate() + 1)
  const day = next.toDateString() === now.toDateString() ? "今天" : "明天"
  return `${day} ${time}`
}

function scheduleLabel(config: DreamConfig) {
  return config.schedule.type === "manual" ? "手动触发" : "每日一次"
}

function effectiveTimeLabel(config: DreamConfig, defaults: DreamDefaults | null) {
  if (config.schedule.type === "manual") return "不定时"
  if (config.schedule.time) return config.schedule.time
  return `${defaults?.default_dream_time || DEFAULT_DREAM_TIME}（全局）`
}

function createRowConfigPatch(row: DreamScheduleRow, patch: Partial<DraftState>) {
  return configFromDraft(row.config, {
    ...draftFromConfig(row.config, null),
    ...patch,
  })
}

export function EmployeeDreamSchedulePanel() {
  const open = useUiStore((s) => s.dreamSchedulePanelOpen)
  const setOpen = useUiStore((s) => s.setDreamSchedulePanelOpen)
  const [rows, setRows] = useState<DreamScheduleRow[]>([])
  const [defaults, setDefaults] = useState<DreamDefaults | null>(null)
  const [defaultTimeInput, setDefaultTimeInput] = useState(DEFAULT_DREAM_TIME)
  const [loading, setLoading] = useState(false)
  const [savingDefaults, setSavingDefaults] = useState(false)
  const [savingEmployeeIds, setSavingEmployeeIds] = useState<Set<string>>(() => new Set())
  const [editingEmployeeId, setEditingEmployeeId] = useState<string | null>(null)
  const [drafts, setDrafts] = useState<Record<string, DraftState>>({})
  const [error, setError] = useState<string | null>(null)

  const activeCount = useMemo(
    () => rows.filter((row) => row.config.enabled).length,
    [rows],
  )

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [allEmployees, dreamDefaults] = await Promise.all([
        ipc.listEmployees(),
        ipc.getDreamDefaults(),
      ])
      const employees = filterUserVisibleEmployees(allEmployees).filter(
        (employee) => isOrdinaryEmployee(employee) && employee.status === "active",
      )
      const loadedRows = await Promise.all(
        employees.map(async (employee) => {
          const [detail, config, recent] = await Promise.all([
            ipc.getEmployeeDetail(employee.id).catch(() => null),
            ipc.getDreamConfig(employee.id).catch(() => defaultDreamConfig()),
            ipc.getRecentDreamResult(employee.id).catch(() => null),
          ])
          return {
            employee,
            description: detail?.manifest?.description || "",
            config,
            recent,
          }
        }),
      )
      setDefaults(dreamDefaults)
      setDefaultTimeInput(dreamDefaults.default_dream_time || DEFAULT_DREAM_TIME)
      setRows(loadedRows)
      setDrafts(
        Object.fromEntries(
          loadedRows.map((row) => [
            row.employee.id,
            draftFromConfig(row.config, dreamDefaults),
          ]),
        ),
      )
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!open) return
    void load()
  }, [load, open])

  const saveDefaults = async () => {
    setSavingDefaults(true)
    setError(null)
    try {
      const next = { default_dream_time: defaultTimeInput || DEFAULT_DREAM_TIME }
      await ipc.setDreamDefaults(next)
      setDefaults(next)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSavingDefaults(false)
    }
  }

  const saveEmployeeConfig = async (employeeId: string, config: DreamConfig) => {
    setSavingEmployeeIds((current) => new Set(current).add(employeeId))
    setError(null)
    try {
      await ipc.setDreamConfig(employeeId, config)
      setRows((current) =>
        current.map((row) =>
          row.employee.id === employeeId ? { ...row, config } : row,
        ),
      )
      setDrafts((current) => ({
        ...current,
        [employeeId]: draftFromConfig(config, defaults),
      }))
      setEditingEmployeeId(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSavingEmployeeIds((current) => {
        const next = new Set(current)
        next.delete(employeeId)
        return next
      })
    }
  }

  const updateDraft = (employeeId: string, patch: Partial<DraftState>) => {
    setDrafts((current) => ({
      ...current,
      [employeeId]: {
        ...current[employeeId],
        ...patch,
      },
    }))
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-80 bg-transparent backdrop-blur-[2px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-81 grid h-[min(88vh,860px)] w-[min(1120px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 grid-rows-[auto_1fr] overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none">
          <header className="flex items-start justify-between gap-4 border-b border-line px-6 py-5">
            <div>
              <p className="text-[11px] font-extrabold uppercase text-muted-foreground">
                定时做梦
              </p>
              <Dialog.Title className="mt-1 text-[24px] font-black text-ink">
                员工定时做梦配置
              </Dialog.Title>
            </div>
            <Button
              type="button"
              variant="outline"
              size="icon-lg"
              onClick={() => setOpen(false)}
            >
              <X className="size-4" />
            </Button>
          </header>

          <div className="min-h-0 overflow-y-auto px-6 py-5">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-4 py-3">
                <p className="text-[12px] font-bold text-muted-foreground">全局策略</p>
                <p className="mt-1 text-[14px] font-extrabold text-ink">
                  仅生成更新建议，需人工审批
                </p>
              </div>
              <div className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-4 py-3">
                <p className="text-[12px] font-bold text-muted-foreground">触发范围</p>
                <p className="mt-1 text-[14px] font-extrabold text-ink">
                  按员工独立执行
                </p>
              </div>
              <div className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-4 py-3">
                <p className="text-[12px] font-bold text-muted-foreground">默认频率</p>
                <p className="mt-1 text-[14px] font-extrabold text-ink">
                  每日一次
                </p>
              </div>
              <div className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-4 py-3">
                <p className="text-[12px] font-bold text-muted-foreground">已启用员工</p>
                <p className="mt-1 text-[14px] font-extrabold text-ink">
                  {activeCount} / {rows.length}
                </p>
              </div>
            </div>

            <div className="mt-4 flex flex-wrap items-end gap-3 rounded-[15px] border border-line-soft bg-white px-4 py-3">
              <label className="grid gap-1">
                <span className="text-[12px] font-bold text-muted-foreground">
                  全局默认时间
                </span>
                <input
                  type="time"
                  value={defaultTimeInput}
                  onChange={(event) => setDefaultTimeInput(event.target.value)}
                  className="h-10 rounded-[12px] border border-line bg-white px-3 text-[13px] font-bold text-ink outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                />
              </label>
              <Button
                type="button"
                variant="outline"
                className="h-10 rounded-[12px] bg-white px-4"
                disabled={savingDefaults || defaultTimeInput === defaults?.default_dream_time}
                onClick={() => void saveDefaults()}
              >
                {savingDefaults ? <Loader2 className="size-3 animate-spin" /> : null}
                保存默认时间
              </Button>
              <Button
                type="button"
                variant="ghost"
                className="h-10 rounded-[12px] px-4"
                disabled={loading}
                onClick={() => void load()}
              >
                {loading ? <Loader2 className="size-3 animate-spin" /> : null}
                刷新
              </Button>
            </div>

            {error ? (
              <div className="mt-4 rounded-[12px] border border-[#f0b8b8] bg-[#fff1f1] px-3 py-2 text-[12px] text-[#8f2424]">
                {error}
              </div>
            ) : null}

            {loading ? (
              <div className="flex min-h-[220px] items-center justify-center gap-2 text-[13px] text-muted-foreground">
                <Loader2 className="size-4 animate-spin" />
                加载中
              </div>
            ) : rows.length === 0 ? (
              <p className="py-10 text-center text-[13px] text-muted-foreground">
                暂无可配置的普通员工
              </p>
            ) : (
              <div className="mt-4 grid gap-4">
                {rows.map((row) => {
                  const employeeId = row.employee.id
                  const saving = savingEmployeeIds.has(employeeId)
                  const editing = editingEmployeeId === employeeId
                  const draft = drafts[employeeId] ?? draftFromConfig(row.config, defaults)
                  return (
                    <section
                      key={employeeId}
                      className={[
                        "rounded-[15px] border px-4 py-4",
                        row.config.enabled
                          ? "border-[#cbdccc] bg-[#f4faf5]"
                          : "border-line-soft bg-[#f8f9fb]",
                      ].join(" ")}
                    >
                      <div className="flex items-start justify-between gap-4">
                        <div className="min-w-0">
                          <h3 className="text-[16px] font-extrabold text-ink">
                            {row.employee.name}
                          </h3>
                          <p className="mt-1 line-clamp-2 text-[13px] leading-5 text-muted-foreground">
                            {row.description || "暂无员工描述。"}
                          </p>
                        </div>
                        <div className="flex shrink-0 items-center gap-3">
                          <span className="text-[13px] font-bold text-muted-foreground">
                            {row.config.enabled ? "已开启" : "已关闭"}
                          </span>
                          <Button
                            type="button"
                            variant="outline"
                            className="h-10 rounded-[14px] bg-white px-4"
                            disabled={saving}
                            onClick={() => {
                              if (!editing) {
                                setEditingEmployeeId(employeeId)
                                updateDraft(employeeId, draftFromConfig(row.config, defaults))
                              } else {
                                void saveEmployeeConfig(
                                  employeeId,
                                  configFromDraft(row.config, draft),
                                )
                              }
                            }}
                          >
                            {saving ? <Loader2 className="size-3 animate-spin" /> : null}
                            {editing ? "保存" : "编辑"}
                          </Button>
                          {!editing ? (
                            <Button
                              type="button"
                              variant="outline"
                              className="h-10 rounded-[14px] bg-white px-4"
                              disabled={saving}
                              onClick={() =>
                                void saveEmployeeConfig(
                                  employeeId,
                                  createRowConfigPatch(row, {
                                    enabled: !row.config.enabled,
                                  }),
                                )
                              }
                            >
                              {row.config.enabled ? "关闭" : "开启"}
                            </Button>
                          ) : null}
                        </div>
                      </div>

                      <div className="mt-4 grid gap-2 md:grid-cols-4">
                        <InfoPill label="频率" value={scheduleLabel(row.config)} />
                        <InfoPill label="时间" value={effectiveTimeLabel(row.config, defaults)} />
                        <InfoPill label="上次" value={formatDateTime(row.recent?.created_at)} />
                        <InfoPill label="下次" value={nextDailyRunLabel(row.config, defaults)} />
                      </div>

                      {editing ? (
                        <div className="mt-4 grid gap-3 rounded-[13px] border border-line-soft bg-white p-3 md:grid-cols-4">
                          <label className="grid gap-1">
                            <span className="text-[11px] font-bold text-muted-foreground">
                              状态
                            </span>
                            <select
                              value={draft.enabled ? "enabled" : "disabled"}
                              onChange={(event) =>
                                updateDraft(employeeId, {
                                  enabled: event.target.value === "enabled",
                                })
                              }
                              className="h-10 rounded-[12px] border border-line bg-white px-3 text-[13px] font-bold outline-none"
                            >
                              <option value="enabled">开启</option>
                              <option value="disabled">关闭</option>
                            </select>
                          </label>
                          <label className="grid gap-1">
                            <span className="text-[11px] font-bold text-muted-foreground">
                              频率
                            </span>
                            <select
                              value={draft.scheduleType}
                              onChange={(event) =>
                                updateDraft(employeeId, {
                                  scheduleType: event.target.value,
                                })
                              }
                              className="h-10 rounded-[12px] border border-line bg-white px-3 text-[13px] font-bold outline-none"
                            >
                              <option value="daily">每日一次</option>
                              <option value="manual">手动触发</option>
                            </select>
                          </label>
                          <label className="grid gap-1">
                            <span className="text-[11px] font-bold text-muted-foreground">
                              时间
                            </span>
                            <input
                              type="time"
                              value={draft.time}
                              disabled={draft.scheduleType === "manual" || draft.useGlobalTime}
                              onChange={(event) =>
                                updateDraft(employeeId, { time: event.target.value })
                              }
                              className="h-10 rounded-[12px] border border-line bg-white px-3 text-[13px] font-bold outline-none disabled:opacity-50"
                            />
                            <label className="mt-1 flex items-center gap-1.5 text-[11px] font-bold text-muted-foreground">
                              <input
                                type="checkbox"
                                checked={draft.useGlobalTime}
                                disabled={draft.scheduleType === "manual"}
                                onChange={(event) =>
                                  updateDraft(employeeId, {
                                    useGlobalTime: event.target.checked,
                                  })
                                }
                              />
                              使用全局时间
                            </label>
                          </label>
                          <label className="grid gap-1">
                            <span className="text-[11px] font-bold text-muted-foreground">
                              最近会话数
                            </span>
                            <input
                              type="number"
                              min={1}
                              max={20}
                              value={draft.latestSessions}
                              onChange={(event) =>
                                updateDraft(employeeId, {
                                  latestSessions: Number(event.target.value),
                                })
                              }
                              className="h-10 rounded-[12px] border border-line bg-white px-3 text-[13px] font-bold outline-none"
                            />
                          </label>
                        </div>
                      ) : null}
                    </section>
                  )
                })}
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

function InfoPill({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[12px] border border-line-soft bg-white px-3 py-2">
      <span className="text-[12px] font-extrabold text-muted-foreground">
        {label}：
      </span>
      <span className="text-[13px] font-extrabold text-muted-foreground">
        {value}
      </span>
    </div>
  )
}
