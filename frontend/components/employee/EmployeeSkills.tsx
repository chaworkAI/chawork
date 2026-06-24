import { useCallback, useState } from "react"
import { Copy, Power, PowerOff, Trash2, Loader2 } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { useEmployeeStore } from "@/stores/employee"
import { useSkillStore } from "@/stores/skill"

export function EmployeeSkills() {
  const t = useUiLabel()
  const selectedEmployeeId = useEmployeeStore((s) => s.selectedEmployeeId)
  const skills = useEmployeeStore((s) => s.selectedSkills)
  const copySkill = useEmployeeStore((s) => s.copySkill)
  const toggleSkill = useEmployeeStore((s) => s.toggleSkill)
  const deleteSkill = useEmployeeStore((s) => s.deleteSkill)

  const rootCatalog = useSkillStore((s) => s.rootCatalog)
  const loadRootSkills = useSkillStore((s) => s.loadSkills)

  const [showCopyPicker, setShowCopyPicker] = useState(false)
  const [copyFilter, setCopyFilter] = useState("")
  const [copyingSkillId, setCopyingSkillId] = useState<string | null>(null)

  const openCopyPicker = useCallback(() => {
    void loadRootSkills()
    setShowCopyPicker(true)
    setCopyFilter("")
  }, [loadRootSkills])

  const handleDelete = useCallback(
    async (skillId: string) => {
      if (!selectedEmployeeId) return
      if (
        !window.confirm(
          applyLabelTemplate(
            t(
              "employee.skills.confirm_delete",
              "确定删除员工内的 Skill「{{id}}」快照？Root skills 中的原始技能不会被删除。",
            ),
            { id: skillId },
          ),
        )
      ) {
        return
      }
      await deleteSkill(selectedEmployeeId, skillId)
    },
    [selectedEmployeeId, deleteSkill, t],
  )

  const handleCopy = useCallback(
    async (skillId: string) => {
      if (!selectedEmployeeId) return
      setCopyingSkillId(skillId)
      try {
        await copySkill(selectedEmployeeId, skillId)
      } finally {
        setCopyingSkillId(null)
      }
    },
    [selectedEmployeeId, copySkill],
  )

  if (!selectedEmployeeId) return null

  const alreadyCopied = new Set(skills.map((s) => s.copied_from).filter(Boolean))
  const filteredRoot = rootCatalog.filter(
    (s) =>
      !alreadyCopied.has(s.id) &&
      (copyFilter === "" || s.name.toLowerCase().includes(copyFilter.toLowerCase())),
  )

  return (
    <div className="grid gap-4">
      <div className="flex items-center justify-between">
        <h3 className="text-[14px] font-bold text-ink">
          {applyLabelTemplate(
            t("employee.skills.title", "员工 Skills ({{count}})"),
            { count: String(skills.length) },
          )}
        </h3>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-[36px] rounded-[12px] bg-white px-4"
          onClick={openCopyPicker}
        >
          <Copy className="mr-1.5 size-3.5" />
          {t("employee.skills.copy_from_root", "添加 Skill")}
        </Button>
      </div>

      {showCopyPicker && (
        <section className="grid gap-2 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <div className="flex items-center justify-between">
            <span className="text-[12px] font-medium text-ink">
              {t("employee.skills.picker_title", "从 Root 添加 Skill")}
            </span>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              className="rounded-[10px]"
              onClick={() => setShowCopyPicker(false)}
            >
              {t("employee.skills.close", "关闭")}
            </Button>
          </div>
          <Input
            value={copyFilter}
            onChange={(e) => setCopyFilter(e.target.value)}
            placeholder={t("employee.skills.search_placeholder", "搜索 skill…")}
            className="min-h-[38px] rounded-[12px] border-line bg-white px-3 text-[12px]"
          />
          <div className="max-h-[360px] overflow-y-auto pr-1">
            {filteredRoot.length === 0 ? (
              <p className="py-2 text-center text-[11px] text-muted-foreground">
                {t("employee.skills.no_copyable", "无可复制的 Skill")}
              </p>
            ) : (
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
                {filteredRoot.map((s) => {
                  const copying = copyingSkillId === s.id
                  return (
                    <div
                      key={s.id}
                      className="grid min-h-[150px] grid-rows-[1fr_auto] rounded-[8px] border border-line-soft bg-white p-3.5 shadow-[0_1px_0_rgba(36,40,50,0.03)]"
                    >
                      <div className="min-w-0">
                        <div className="flex items-start justify-between gap-2">
                          <p className="min-w-0 truncate text-[13px] font-extrabold text-ink">{s.name}</p>
                          <span className="shrink-0 rounded-[8px] bg-[#eef1f5] px-2 py-0.5 text-[10px] font-bold text-muted-foreground">
                            {t("employee.skills.source_root", "Root")}
                          </span>
                        </div>
                        {s.description ? (
                          <p className="mt-2 line-clamp-3 text-[11px] leading-5 text-muted-foreground">
                            {s.description}
                          </p>
                        ) : (
                          <p className="mt-2 line-clamp-3 text-[11px] leading-5 text-muted-foreground">
                            {t("employee.skills.no_description", "无描述")}
                          </p>
                        )}
                        <p className="mt-2 truncate font-mono text-[10px] text-muted-foreground">
                          {s.id}
                        </p>
                      </div>
                      <div className="mt-3 flex items-center justify-end">
                        <Button
                          type="button"
                          variant="outline"
                          size="xs"
                          className="h-[32px] rounded-[10px] bg-white px-3 text-[12px] font-bold"
                          disabled={copyingSkillId != null}
                          onClick={() => void handleCopy(s.id)}
                        >
                          {copying ? (
                            <Loader2 className="size-3 animate-spin" />
                          ) : (
                            <Copy className="size-3" />
                          )}
                          {t("employee.skills.copy_action", "添加")}
                        </Button>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        </section>
      )}

      {skills.length === 0 ? (
        <p className="py-8 text-center text-[13px] text-muted-foreground">
          {t("employee.skills.empty", "暂无 Skill，点击上方按钮添加")}
        </p>
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {skills.map((skill) => (
            <div
              key={skill.id}
              className={cn(
                "grid min-h-[150px] grid-rows-[1fr_auto] rounded-[8px] border p-3.5 shadow-[0_1px_0_rgba(36,40,50,0.03)]",
                skill.enabled
                  ? "border-line-soft bg-white"
                  : "border-line-soft bg-[#f8f9fb] opacity-70",
              )}
            >
              <div className="min-w-0">
                <div className="flex items-start justify-between gap-2">
                  <p className="min-w-0 truncate text-[13px] font-extrabold text-ink">{skill.name}</p>
                  <span
                    className={cn(
                      "shrink-0 rounded-[8px] px-2 py-0.5 text-[10px] font-bold",
                      skill.enabled
                        ? "bg-success/10 text-success"
                        : "bg-[#eef1f5] text-muted-foreground",
                    )}
                  >
                    {skill.enabled
                      ? t("employee.skills.status_enabled", "已启用")
                      : t("employee.skills.status_disabled", "已禁用")}
                  </span>
                </div>
                {skill.description ? (
                  <p className="mt-2 line-clamp-3 text-[11px] leading-5 text-muted-foreground">
                    {skill.description}
                  </p>
                ) : (
                  <p className="mt-2 line-clamp-3 text-[11px] leading-5 text-muted-foreground">
                    {t("employee.skills.no_description", "无描述")}
                  </p>
                )}
                <p className="mt-2 truncate font-mono text-[10px] text-muted-foreground">
                  {skill.source}
                  {skill.copied_from
                    ? ` (${applyLabelTemplate(
                        t("employee.skills.copied_from", "copied from {{id}}"),
                        { id: skill.copied_from },
                      )})`
                    : ""}
                </p>
              </div>
              <div className="mt-3 flex items-center justify-end gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  className="h-[32px] rounded-[10px] px-3 text-[12px] font-bold"
                  title={
                    skill.enabled
                      ? t("employee.skills.disable", "禁用")
                      : t("employee.skills.enable", "启用")
                  }
                  onClick={() => void toggleSkill(selectedEmployeeId, skill.id, !skill.enabled)}
                >
                  {skill.enabled ? (
                    <Power className="size-3.5 text-success" />
                  ) : (
                    <PowerOff className="size-3.5 text-muted-foreground" />
                  )}
                  {skill.enabled
                    ? t("employee.skills.disable", "禁用")
                    : t("employee.skills.enable", "启用")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  className="rounded-[10px]"
                  title={t("employee.skills.delete", "删除")}
                  onClick={() => void handleDelete(skill.id)}
                >
                  <Trash2 className="size-3.5 text-muted-foreground hover:text-danger" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
