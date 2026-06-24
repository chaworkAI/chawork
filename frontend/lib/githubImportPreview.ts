import {
  githubHubRepoKeyFromUrl,
  isTerminalGithubImportJobStatus,
} from "@/lib/githubUrl"
import * as ipc from "@/lib/tauri"
import type { HubGithubImportSkillPreview, HubGithubImportJob } from "@/types/hub"

const HUB_LIST_PAGE_SIZE = 50

function skillDescription(skill: {
  description_zh?: string
  description_en?: string
}) {
  return skill.description_zh?.trim() || skill.description_en?.trim() || ""
}

function isGithubRepoSkill(
  skill: { id: string; source?: Record<string, unknown> },
  repoKey: string,
) {
  if (skill.id.startsWith(`${repoKey}--`)) {
    return true
  }
  return (
    (skill.source as { type?: string; repo?: string } | undefined)?.type ===
      "github" &&
    (skill.source as { repo?: string }).repo === repoKey
  )
}

export async function resolveGithubImportPreviewFromHub(
  repoUrl: string,
): Promise<HubGithubImportSkillPreview[]> {
  const repoKey = githubHubRepoKeyFromUrl(repoUrl)
  if (!repoKey) return []

  const byId = new Map<string, HubGithubImportSkillPreview>()
  let page = 1

  while (true) {
    const result = await ipc.hubListSkills({
      query: repoKey,
      filter: "all",
      page,
      limit: HUB_LIST_PAGE_SIZE,
    })

    for (const skill of result.items) {
      if (!isGithubRepoSkill(skill, repoKey)) continue
      byId.set(skill.id, {
        id: skill.id,
        name: skill.name,
        profession: skill.profession,
        description_zh: skill.description_zh,
        description_en: skill.description_en,
      })
    }

    if (result.items.length === 0 || page * result.limit >= result.total) {
      break
    }
    page += 1
  }

  return Array.from(byId.values()).sort((left, right) =>
    left.name.localeCompare(right.name),
  )
}

export async function enrichGithubImportPreviewDescriptions(
  repoUrl: string,
  previews: HubGithubImportSkillPreview[],
): Promise<HubGithubImportSkillPreview[]> {
  if (previews.length === 0) return previews
  if (previews.some((skill) => skillDescription(skill))) {
    return previews
  }

  const catalog = await resolveGithubImportPreviewFromHub(repoUrl)
  if (catalog.length === 0) return previews

  const byId = new Map(catalog.map((skill) => [skill.id, skill]))
  return previews.map((skill) => {
    const enriched = byId.get(skill.id)
    if (!enriched) return skill
    return {
      ...skill,
      description_zh: enriched.description_zh,
      description_en: enriched.description_en,
    }
  })
}

export async function resolveGithubImportPreviewSkills(
  repoUrl: string,
  job: Awaited<ReturnType<typeof ipc.hubGetGithubImportJob>>,
): Promise<HubGithubImportSkillPreview[]> {
  let previews = job.skills
  if (previews.length === 0 && isTerminalGithubImportJobStatus(job.status)) {
    previews = await resolveGithubImportPreviewFromHub(repoUrl)
  }
  return enrichGithubImportPreviewDescriptions(repoUrl, previews)
}

export function resolveGithubImportJobId(job: HubGithubImportJob): string {
  const extended = job as HubGithubImportJob & { job_id?: string; jobId?: string }
  return extended.id?.trim() || extended.job_id?.trim() || extended.jobId?.trim() || ""
}

export function githubImportPreviewDescription(
  skill: HubGithubImportSkillPreview,
) {
  return skillDescription(skill)
}
