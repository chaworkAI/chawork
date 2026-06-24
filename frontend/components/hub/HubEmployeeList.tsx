import { useState } from "react"
import { Download, Loader2, RefreshCw, Trash2 } from "lucide-react"

import { HubCardFeedback } from "@/components/hub/HubCardFeedback"
import {
  getHubRelation,
  isDeletableHubEmployee,
  isUserCustomGithubEmployee,
  relationBadgeClass,
} from "@/components/hub/hubLocalState"
import { Button } from "@/components/ui/button"
import type { HubInstallFeedback } from "@/stores/hub"
import type { HubDownloadFilter, HubEmployeeView } from "@/types/hub"

const DEFAULT_VISIBLE_SKILLS = 10
const TYPE_TAG_CLASSES = [
  "text-[#9a3412]",
  "text-[#15803d]",
  "text-[#2457b7]",
  "text-[#7e22ce]",
  "text-[#be185d]",
  "text-[#0e7490]",
]

function localSkillNameFromHubId(id: string) {
  const parts = id.split("--")
  const last = parts[parts.length - 1]
  if (last && last !== "." && last !== "..") return last
  return id
}

function typeTagClass(tag: string) {
  let hash = 0
  for (const char of tag) hash = (hash * 31 + char.charCodeAt(0)) >>> 0
  return TYPE_TAG_CLASSES[hash % TYPE_TAG_CLASSES.length]
}

export function HubEmployeeList({
  employees,
  filter,
  userCustomGithubEmployeeIds,
  installingIds,
  feedbackByKey,
  onInstall,
  onDelete,
}: {
  employees: HubEmployeeView[]
  filter: HubDownloadFilter
  userCustomGithubEmployeeIds: string[]
  installingIds: string[]
  feedbackByKey: Record<string, HubInstallFeedback>
  onInstall: (id: string) => void
  onDelete: (id: string, installed: boolean) => void
}) {
  const [expandedSkillEmployees, setExpandedSkillEmployees] = useState<Set<string>>(
    () => new Set(),
  )

  const toggleSkillExpansion = (employeeId: string) => {
    setExpandedSkillEmployees((current) => {
      const next = new Set(current)
      if (next.has(employeeId)) next.delete(employeeId)
      else next.add(employeeId)
      return next
    })
  }

  if (employees.length === 0) {
    return (
      <p className="py-10 text-center text-[13px] text-muted-foreground">
        没有匹配的员工
      </p>
    )
  }

  return (
    <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
      {employees.map((employee) => {
        const installing = installingIds.includes(employee.id)
        const busy = installing
        const isGithubCustom = isUserCustomGithubEmployee(
          employee,
          userCustomGithubEmployeeIds,
        )
        const relation = getHubRelation(employee, "employee", {
          isUserCustomImport: isGithubCustom,
        })
        const deletable = isDeletableHubEmployee(employee, userCustomGithubEmployeeIds)
        const showDelete =
          (filter === "local" || filter === "custom") && deletable
        const deleteInstalled =
          filter === "local"
            ? employee.downloaded || isGithubCustom
            : isGithubCustom
        const skillsExpanded = expandedSkillEmployees.has(employee.id)
        const visibleSkills = skillsExpanded
          ? employee.skill_ids
          : employee.skill_ids.slice(0, DEFAULT_VISIBLE_SKILLS)
        const hiddenSkillCount = Math.max(
          0,
          employee.skill_ids.length - DEFAULT_VISIBLE_SKILLS,
        )
        return (
          <div
            key={employee.id}
            className="relative grid min-h-[168px] grid-rows-[1fr_auto] overflow-hidden rounded-[8px] border border-line-soft bg-white p-4"
          >
            <HubCardFeedback feedback={feedbackByKey[`employees:${employee.id}`]} />
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <p className="text-[13px] font-extrabold text-ink">{employee.name}</p>
                <span
                  className={[
                    "rounded-[8px] px-2 py-0.5 text-[10px] font-bold",
                    relationBadgeClass(relation.tone),
                  ].join(" ")}
                >
                  {relation.label}
                </span>
              </div>
              {employee.description ? (
                <p className="mt-1 line-clamp-2 text-[12px] leading-5 text-muted-foreground">
                  {employee.description}
                </p>
              ) : null}
              {employee.skill_ids.length > 0 ? (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {visibleSkills.map((skillId) => (
                    <span
                      key={skillId}
                      className="rounded-[7px] bg-[#f4f6f8] px-1.5 py-0.5 font-mono text-[10px] font-medium text-muted-foreground"
                      title={skillId}
                    >
                      {localSkillNameFromHubId(skillId)}
                    </span>
                  ))}
                  {!skillsExpanded && hiddenSkillCount > 0 ? (
                    <span className="rounded-[7px] bg-[#f4f6f8] px-1.5 py-0.5 font-mono text-[10px] font-semibold text-muted-foreground">
                      +{hiddenSkillCount} 更多
                    </span>
                  ) : null}
                  {hiddenSkillCount > 0 ? (
                    <button
                      type="button"
                      className="rounded-[7px] border border-line-soft bg-white px-1.5 py-0.5 text-[10px] font-bold text-ink transition hover:bg-[#f8f9fb]"
                      onClick={() => toggleSkillExpansion(employee.id)}
                    >
                      {skillsExpanded ? "收起" : "展开全部"}
                    </button>
                  ) : null}
                </div>
              ) : null}
            </div>
            <div className="mt-3 flex items-end justify-between gap-3">
              <div className="flex min-w-0 flex-wrap gap-x-2 gap-y-1">
                {employee.tags.slice(0, 4).map((tag) => (
                  <span
                    key={tag}
                    className={[
                      "text-[10px] font-bold leading-5",
                      typeTagClass(tag),
                    ].join(" ")}
                    title={`类型：${tag}`}
                  >
                    {tag}
                  </span>
                ))}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {showDelete ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onClick={() => {
                      const message = deleteInstalled
                        ? `确认删除员工「${employee.name}」？将同时从本地、自定义列表和员工面板移除。`
                        : `确认从自定义列表移除「${employee.name}」？`
                      if (!window.confirm(message)) {
                        return
                      }
                      onDelete(employee.id, deleteInstalled)
                    }}
                  >
                    {busy ? (
                      <Loader2 className="size-3 animate-spin" />
                    ) : (
                      <Trash2 className="size-3" />
                    )}
                    删除
                  </Button>
                ) : null}
                {employee.downloaded ? (
                  <span className="rounded-[8px] bg-[#e4efff] px-2 py-1 text-[10px] font-bold text-[#2457b7]">
                    本地
                  </span>
                ) : (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onClick={() => onInstall(employee.id)}
                  >
                    {busy ? (
                      <Loader2 className="size-3 animate-spin" />
                    ) : employee.update_available ? (
                      <RefreshCw className="size-3" />
                    ) : (
                      <Download className="size-3" />
                    )}
                    下载
                  </Button>
                )}
              </div>
            </div>
          </div>
        )
      })}
    </div>
  )
}
