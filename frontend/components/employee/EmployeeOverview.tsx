import { useCallback, useState } from "react"
import {
  CheckCircle,
  AlertTriangle,
  Loader2,
  XCircle,
  CheckCircle2,
  Play,
  FolderOpen,
  Unlink,
  Trash2,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { isOrdinaryEmployee } from "@/lib/employeeDream"
import { useEmployeeStore } from "@/stores/employee"
import { useLocaleStore } from "@/stores/locale"
import { useWorkspaceStore } from "@/stores/workspace"

function MetaPill({
  label,
  value,
  mono = false,
}: {
  label: string
  value: string
  mono?: boolean
}) {
  return (
    <span className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-line-soft bg-white px-2.5 py-1 text-[11px] text-muted-foreground">
      <span className="shrink-0 font-bold text-[var(--subtle)]">
        {label}
      </span>
      <span className={mono ? "min-w-0 truncate font-mono text-ink" : "min-w-0 truncate text-ink"}>
        {value || "—"}
      </span>
    </span>
  )
}

function DetailField({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[13px] border border-line-soft bg-white px-3.5 py-3">
      <span className="text-[11px] font-extrabold uppercase text-[var(--subtle)]">
        {label}
      </span>
      <p className="mt-1 whitespace-pre-wrap break-words text-[13px] leading-5 text-ink">
        {value || "—"}
      </p>
    </div>
  )
}

export function EmployeeOverview() {
  const t = useUiLabel()
  const appLocale = useLocaleStore((s) => s.locale)
  const dateLocale = appLocale === "zh-CN" ? "zh-CN" : "en-US"

  const detail = useEmployeeStore((s) => s.selectedDetail)
  const employeeId = detail?.registry_entry.id
  const updateMetadata = useEmployeeStore((s) => s.updateMetadata)
  const deleteEmployee = useEmployeeStore((s) => s.deleteEmployee)
  const runDream = useEmployeeStore((s) => s.runDream)
  const setActiveTab = useEmployeeStore((s) => s.setActiveTab)
  const workspaces = useEmployeeStore((s) => s.selectedWorkspaces)
  const unbindWorkspace = useEmployeeStore((s) => s.unbindWorkspace)
  const openWorkspaceDialog = useWorkspaceStore((s) => s.openWorkspaceDialog)
  const isDreamRunning = useEmployeeStore(
    (s) => employeeId != null && s.dreamRunningEmployeeId === employeeId,
  )
  const recentDreamResult = useEmployeeStore((s) =>
    employeeId && s.dreamStateEmployeeId === employeeId ? s.recentDreamResult : null,
  )

  const manifest = detail?.manifest
  const integrity = detail?.integrity

  const [editing, setEditing] = useState(false)
  const [editName, setEditName] = useState("")
  const [editDesc, setEditDesc] = useState("")
  const [saving, setSaving] = useState(false)

  const startEdit = useCallback(() => {
    setEditName(manifest?.name ?? "")
    setEditDesc(manifest?.description ?? "")
    setEditing(true)
  }, [manifest])

  const cancelEdit = useCallback(() => {
    setEditing(false)
  }, [])

  const saveEdit = useCallback(async () => {
    if (!detail) return
    setSaving(true)
    try {
      await updateMetadata(detail.registry_entry.id, {
        name: editName,
        description: editDesc,
      })
      setEditing(false)
    } finally {
      setSaving(false)
    }
  }, [detail, editName, editDesc, updateMetadata])

  const handleUnbindWorkspace = useCallback(
    async (path: string, name: string) => {
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
    },
    [t, unbindWorkspace],
  )

  const runManualDream = useCallback(() => {
    if (!employeeId || isDreamRunning) return
    setActiveTab("dream")
    void runDream(employeeId)
  }, [employeeId, isDreamRunning, runDream, setActiveTab])

  const handleDeleteEmployee = useCallback(async () => {
    if (!employeeId || !manifest) return
    if (
      !window.confirm(
        `确认删除员工「${manifest.name}」？将同时从 Hub 本地/自定义列表和员工面板移除。`,
      )
    ) {
      return
    }
    await deleteEmployee(employeeId)
  }, [deleteEmployee, employeeId, manifest])

  if (!detail || !manifest) {
    return (
      <p className="py-8 text-center text-[13px] text-muted-foreground">
        {t("employee.overview.no_manifest", "未找到员工 manifest 数据")}
      </p>
    )
  }

  const createdAt = new Date(manifest.created_at).toLocaleString(dateLocale)
  const updatedAt = new Date(manifest.updated_at).toLocaleString(dateLocale)
  const canDeleteEmployee = isOrdinaryEmployee(detail.registry_entry)

  return (
    <div className="grid gap-4">
      <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <div className="flex items-center justify-between">
          <h3 className="text-[14px] font-bold text-ink">
            {t("employee.overview.basic_info", "基本信息")}
          </h3>
          {!editing && (
            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-[34px] rounded-[12px] bg-white px-3 text-[12px]"
                onClick={startEdit}
              >
                {t("employee.overview.edit", "编辑")}
              </Button>
              {canDeleteEmployee ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-[34px] rounded-[12px] border-[#f0b8b8] bg-white px-3 text-[12px] text-[#8f2424] hover:bg-[#fff1f1]"
                  onClick={() => void handleDeleteEmployee()}
                >
                  <Trash2 className="mr-1.5 size-3.5" />
                  {t("employee.overview.delete", "删除员工")}
                </Button>
              ) : null}
            </div>
          )}
        </div>

        {editing ? (
          <div className="grid gap-3">
            <label className="block text-[13px] font-bold text-muted-foreground">
              {t("employee.overview.field.name", "名称")}
            </label>
            <Input
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              className="min-h-[42px] rounded-[12px] border-line bg-white px-3 text-[13px]"
            />
            <label className="block text-[13px] font-bold text-muted-foreground">
              {t("employee.overview.field.description", "描述")}
            </label>
            <Textarea
              value={editDesc}
              onChange={(e) => setEditDesc(e.target.value)}
              rows={3}
              className="min-h-[84px] rounded-[12px] border-line bg-white px-3 py-2 text-[13px]"
            />
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                className="h-[36px] rounded-[12px] px-4"
                disabled={saving}
                onClick={() => void saveEdit()}
              >
                {saving ? <Loader2 className="mr-1 size-3 animate-spin" /> : null}
                {t("employee.overview.save", "保存")}
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-[36px] rounded-[12px] px-4"
                onClick={cancelEdit}
              >
                {t("employee.overview.cancel", "取消")}
              </Button>
            </div>
          </div>
        ) : (
          <div className="grid gap-2">
            <div className="flex flex-wrap gap-2">
              <MetaPill label="ID" value={manifest.id} mono />
              <MetaPill
                label={t("employee.overview.field.kind", "类型")}
                value={
                  manifest.kind === "dream"
                    ? t("employee.kind.dream", "Dream 员工")
                    : t("employee.kind.ordinary", "普通员工")
                }
              />
              <MetaPill
                label={t("employee.overview.field.status", "状态")}
                value={manifest.status}
              />
              <MetaPill
                label={t("employee.overview.field.created_at", "创建时间")}
                value={createdAt}
              />
              <MetaPill
                label={t("employee.overview.field.updated_at", "更新时间")}
                value={updatedAt}
              />
            </div>
            <DetailField
              label={t("employee.overview.field.name", "名称")}
              value={manifest.name}
            />
            <DetailField
              label={t("employee.overview.field.description", "描述")}
              value={manifest.description}
            />
          </div>
        )}
      </section>

      <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0">
            <h3 className="text-[14px] font-bold text-ink">
              {t("employee.workspaces.bound_title", "已绑定工作区")}
            </h3>
            <p className="mt-1 text-[12px] text-muted-foreground">
              {t("employee.overview.workspaces_hint", "这些工作区会使用当前员工的 Prompt 和 Skills。")}
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-[36px] shrink-0 rounded-[12px] bg-white px-4"
            onClick={() => employeeId && void openWorkspaceDialog(employeeId)}
          >
            <FolderOpen className="mr-1.5 size-3.5" />
            {t("employee.workspaces.add_workspace", "添加工作区")}
          </Button>
        </div>

        {workspaces.length === 0 ? (
          <p className="rounded-[13px] border border-dashed border-line bg-white px-4 py-6 text-center text-[13px] text-muted-foreground">
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
                className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-[13px] border border-line-soft bg-white px-3.5 py-3"
              >
                <div className="min-w-0">
                  <p className="truncate text-[13px] font-semibold text-ink">{ws.name}</p>
                  <p className="truncate font-mono text-[11px] text-muted-foreground">
                    {ws.path}
                  </p>
                  <p className="mt-0.5 text-[11px] text-muted-foreground">
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
                  onClick={() => void handleUnbindWorkspace(ws.path, ws.name)}
                >
                  <Unlink className="size-3.5 text-muted-foreground" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>

      {detail.registry_entry.kind === "ordinary" && (
        <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <h3 className="text-[14px] font-bold text-ink">
                {t("employee.overview.manual_dream_title", "手动测试 Dream")}
              </h3>
              <p className="mt-1 text-[12px] text-muted-foreground">
                {t(
                  "employee.overview.manual_dream_hint",
                  "立即分析最近会话，生成待审批的 prompt 更新建议。",
                )}
              </p>
            </div>
            <Button
              type="button"
              size="sm"
              disabled={isDreamRunning}
              onClick={runManualDream}
              className="h-[36px] shrink-0 rounded-[12px] px-4"
            >
              {isDreamRunning ? (
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
              ) : (
                <Play className="mr-1.5 size-3.5" />
              )}
              {isDreamRunning
                ? t("employee.overview.manual_dream_running", "Dream 运行中...")
                : t("employee.overview.manual_dream_button", "运行 Dream")}
            </Button>
          </div>
        </section>
      )}

      {integrity && (
        <section className="grid gap-2 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <h3 className="text-[14px] font-bold text-ink">
            {t("employee.overview.integrity", "完整性检查")}
          </h3>
          <div className="flex items-center gap-2">
            {integrity.status === "ok" ? (
              <>
                <CheckCircle className="size-4 text-success" />
                <span className="text-[13px] text-success">
                  {t("employee.overview.integrity_ok", "完整性正常")}
                </span>
              </>
            ) : (
              <>
                <AlertTriangle className="size-4 text-warning" />
                <span className="text-[13px] text-warning">
                  {t("employee.overview.integrity_fix", "需要修复")}
                </span>
              </>
            )}
          </div>
          {integrity.issues.length > 0 && (
            <ul className="list-inside list-disc space-y-1 text-[12px] text-muted-foreground">
              {integrity.issues.map((issue, i) => (
                <li key={i}>
                  <span className="font-mono text-[11px]">[{issue.code}]</span> {issue.message}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      {detail.registry_entry.kind === "ordinary" && recentDreamResult && (
        <section className="grid gap-2 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <h3 className="text-[14px] font-bold text-ink">
            {t("employee.overview.recent_dream", "最近 Dream 结果")}
          </h3>
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
                ? t("employee.dream.result_parse_failed", "解析失败")
                : recentDreamResult.decision === "update_required"
                  ? t("employee.dream.result_update_required", "建议更新")
                  : t("employee.dream.result_no_update", "无需更新")}
            </span>
          </div>
          <p className="text-[12px] text-ink">{recentDreamResult.summary}</p>
          <div className="flex items-center gap-3 text-[11px] text-muted-foreground">
            <span className="font-mono">{recentDreamResult.dream_run_id}</span>
            <span>{recentDreamResult.created_at.replace("T", " ").replace("Z", "")}</span>
          </div>
        </section>
      )}
    </div>
  )
}
