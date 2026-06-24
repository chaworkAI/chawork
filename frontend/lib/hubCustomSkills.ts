import * as ipc from "@/lib/tauri"
import type {
  HubGithubImportSkillPreview,
  HubSkillView,
} from "@/types/hub"

const HUB_USER_CUSTOM_SKILLS_KEY = "chawork.hub.userCustomSkillIds"
const HUB_CUSTOM_SKILL_PREVIEWS_KEY = "chawork.hub.customSkillPreviews"
const HUB_CUSTOM_CLEAR_MIGRATION_KEY = "chawork.hub.migration.clearedCustom20260615"

export function readUserCustomHubSkillIds(): string[] {
  if (!localStorage.getItem(HUB_CUSTOM_CLEAR_MIGRATION_KEY)) {
    writeUserCustomHubSkillIds([])
    localStorage.setItem(HUB_CUSTOM_CLEAR_MIGRATION_KEY, "1")
    return []
  }
  try {
    const raw = localStorage.getItem(HUB_USER_CUSTOM_SKILLS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((id) => typeof id === "string") : []
  } catch {
    return []
  }
}

export function writeUserCustomHubSkillIds(ids: string[]) {
  localStorage.setItem(HUB_USER_CUSTOM_SKILLS_KEY, JSON.stringify(ids))
}

export function readCustomSkillPreviews(): HubGithubImportSkillPreview[] {
  try {
    const raw = localStorage.getItem(HUB_CUSTOM_SKILL_PREVIEWS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (item): item is HubGithubImportSkillPreview =>
        item != null &&
        typeof item === "object" &&
        typeof item.id === "string" &&
        typeof item.name === "string",
    )
  } catch {
    return []
  }
}

export function writeCustomSkillPreviews(previews: HubGithubImportSkillPreview[]) {
  localStorage.setItem(HUB_CUSTOM_SKILL_PREVIEWS_KEY, JSON.stringify(previews))
}

export function mergeCustomSkillPreviews(
  existing: HubGithubImportSkillPreview[],
  incoming: HubGithubImportSkillPreview[],
): HubGithubImportSkillPreview[] {
  const byId = new Map(existing.map((preview) => [preview.id, preview]))
  for (const preview of incoming) {
    byId.set(preview.id, preview)
  }
  return Array.from(byId.values())
}

export function previewToSkillView(preview: HubGithubImportSkillPreview): HubSkillView {
  return {
    id: preview.id,
    name: preview.name,
    profession: preview.profession,
    description_zh: preview.description_zh ?? "",
    description_en: preview.description_en ?? "",
    content_hash: "",
    source: { type: "github" },
    tags: [],
    created_at: "",
    updated_at: "",
    downloaded: false,
    update_available: false,
    remote_updated_at: "",
  }
}

export async function loadCustomHubSkills(ids: string[]): Promise<HubSkillView[]> {
  if (ids.length === 0) return []

  const previewById = new Map(
    readCustomSkillPreviews().map((preview) => [preview.id, preview]),
  )

  const localById = new Map<string, HubSkillView>()
  try {
    const localResult = await ipc.hubListSkills({
      filter: "local",
      page: 1,
      limit: 500,
    })
    for (const skill of localResult.items) {
      if (ids.includes(skill.id)) {
        localById.set(skill.id, skill)
      }
    }
  } catch {
    // 本地状态拉取失败时仍用预览/metadata 展示自定义列表。
  }

  const items: HubSkillView[] = []
  for (const id of ids) {
    const local = localById.get(id)
    const preview = previewById.get(id)
    if (local) {
      const description_zh =
        local.description_zh || preview?.description_zh || preview?.description_en || ""
      const description_en =
        local.description_en || preview?.description_en || preview?.description_zh || ""
      items.push({
        ...local,
        description_zh,
        description_en,
      })
      continue
    }
    if (preview) {
      items.push(previewToSkillView(preview))
      continue
    }
    try {
      const detail = await ipc.hubGetSkillDetail(id)
      items.push({
        ...detail,
        downloaded: false,
        update_available: false,
        remote_updated_at: detail.updated_at,
      })
    } catch {
      // 跳过 Hub 上已不存在的 orphan id。
    }
  }
  return items
}
