import type { HubLocalState } from "@/types/hub"

export type HubMarketEntryKind = "skill" | "employee"
export type HubRelationTone = "neutral" | "success" | "warning" | "local"

export interface HubRelationView {
  label: string
  detail: string
  tone: HubRelationTone
  actionBlocked: boolean
}

export function isGithubHubSource(source: Record<string, unknown> | undefined) {
  return (source as { type?: string } | undefined)?.type === "github"
}

/** 用户从 GitHub 导入并记录在 localStorage 中的技能。 */
export function isUserCustomHubSkill(
  skill: HubLocalState & { id?: string; source?: Record<string, unknown> },
  userCustomHubSkillIds: string[],
) {
  return skill.id != null && userCustomHubSkillIds.includes(skill.id)
}

export function isUserCustomGithubEmployee(
  employee: HubLocalState & { id?: string },
  userCustomGithubEmployeeIds: string[],
) {
  return employee.id != null && userCustomGithubEmployeeIds.includes(employee.id)
}

export function isDeletableHubEmployee(
  employee: HubLocalState & { id?: string; downloaded?: boolean },
  userCustomGithubEmployeeIds: string[],
) {
  return (
    employee.downloaded === true ||
    isUserCustomGithubEmployee(employee, userCustomGithubEmployeeIds)
  )
}

export function getHubRelation(
  item: HubLocalState,
  kind: HubMarketEntryKind,
  options?: {
    source?: Record<string, unknown>
    isUserCustomImport?: boolean
  },
): HubRelationView {
  const source = options?.source
  const isUserCustomImport = options?.isUserCustomImport ?? false

  if (item.update_available) {
    return {
      label: "待更新",
      detail: "远程更新时间晚于本地 Hub 快照。",
      tone: "warning",
      actionBlocked: false,
    }
  }
  if (item.downloaded) {
    const isGithubImport = isUserCustomImport || isGithubHubSource(source)
    if (kind === "skill" && isGithubImport) {
      return {
        label: "自定义",
        detail: "从 GitHub 导入并已安装到 Root。",
        tone: "local",
        actionBlocked: false,
      }
    }
    if (kind === "skill") {
      return {
        label: "本地",
        detail: "从 Hub 官方目录下载到 Root。",
        tone: "success",
        actionBlocked: false,
      }
    }
    if (isUserCustomImport) {
      return {
        label: "自定义",
        detail: "从 GitHub 导入的员工。",
        tone: "local",
        actionBlocked: false,
      }
    }
    return {
      label: "本地",
      detail: "本地员工来自这个 Hub 条目。",
      tone: "success",
      actionBlocked: false,
    }
  }
  if (kind === "skill" && isUserCustomImport) {
    return {
      label: "自定义",
      detail: isGithubHubSource(source)
        ? item.downloaded
          ? "从 GitHub 导入并已安装到 Root。"
          : "从 GitHub 导入的技能，下载后将覆盖本地同名技能。"
        : "本地已有同名技能，下载 Hub 版本将覆盖。",
      tone: "local",
      actionBlocked: false,
    }
  }
  if (item.local_source === "custom") {
    return {
      label: "自定义",
      detail: "本地已有同名技能，下载将覆盖现有内容。",
      tone: "local",
      actionBlocked: false,
    }
  }
  if (item.local_source === "other_hub") {
    return {
      label: "自定义",
      detail: item.local_source_detail
        ? `本地同名技能来自 ${item.local_source_detail}，下载将覆盖。`
        : "本地同名技能来自其他 Hub 来源，下载将覆盖。",
      tone: "local",
      actionBlocked: false,
    }
  }
  if (item.local_source === "other_kind") {
    return {
      label: "自定义",
      detail: "本地已有同名技能，下载将覆盖。",
      tone: "local",
      actionBlocked: false,
    }
  }
  return {
    label: "远程",
    detail: kind === "skill" ? "Hub 有，Root 尚未下载。" : "Hub 有，本地员工仓库尚未下载。",
    tone: "neutral",
    actionBlocked: false,
  }
}

export function relationBadgeClass(tone: HubRelationTone) {
  if (tone === "success") return "bg-[#e4efff] text-[#2457b7] ring-1 ring-[#a9c7fb]"
  if (tone === "warning") return "bg-[#dff8e7] text-[#126a34] ring-1 ring-[#9edfb2]"
  if (tone === "local") return "bg-[#efe7ff] text-[#6735a8] ring-1 ring-[#c4a7f2]"
  return "bg-[#ffe4e1] text-[#b42318] ring-1 ring-[#f1aaa2]"
}

export function countHubRelations(items: HubLocalState[]) {
  return items.reduce(
    (acc, item) => {
      const relation = getHubRelation(item, "skill")
      if (item.update_available) acc.updateAvailable += 1
      else if (item.downloaded) acc.downloaded += 1
      else if (relation.actionBlocked) acc.localSource += 1
      else acc.remoteOnly += 1
      return acc
    },
    { remoteOnly: 0, downloaded: 0, updateAvailable: 0, localSource: 0 },
  )
}
