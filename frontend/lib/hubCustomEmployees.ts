import * as ipc from "@/lib/tauri"
import type { HubEmployeeView } from "@/types/hub"
import type { RegistryEntry } from "@/types/employee"

const HUB_USER_GITHUB_EMPLOYEE_IDS_KEY = "chawork.hub.userGithubEmployeeIds"

export function readUserCustomGithubEmployeeIds(): string[] {
  try {
    const raw = localStorage.getItem(HUB_USER_GITHUB_EMPLOYEE_IDS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((id) => typeof id === "string") : []
  } catch {
    return []
  }
}

export function writeUserCustomGithubEmployeeIds(ids: string[]) {
  localStorage.setItem(HUB_USER_GITHUB_EMPLOYEE_IDS_KEY, JSON.stringify(ids))
}

export function removeUserCustomGithubEmployeeId(employeeId: string): string[] {
  const next = readUserCustomGithubEmployeeIds().filter((id) => id !== employeeId)
  writeUserCustomGithubEmployeeIds(next)
  return next
}

function registryEntryById(
  entries: RegistryEntry[],
  employeeId: string,
): RegistryEntry | undefined {
  return entries.find((entry) => entry.id === employeeId)
}

export async function loadGithubImportEmployeeViews(
  employeeIds: string[],
  registry: RegistryEntry[],
): Promise<HubEmployeeView[]> {
  if (employeeIds.length === 0) return []

  const items: HubEmployeeView[] = []
  for (const employeeId of employeeIds) {
    const entry = registryEntryById(registry, employeeId)
    if (!entry) continue

    let skillIds: string[] = []
    try {
      const skills = await ipc.listEmployeeSkills(employeeId)
      skillIds = skills.map((skill) => skill.id)
    } catch {
      // 员工目录不完整时仍展示 registry 条目。
    }

    items.push({
      id: entry.id,
      name: entry.name,
      description: "从 GitHub 仓库同步创建的本地员工",
      kind: entry.kind,
      prompt_preview: "",
      skill_ids: skillIds,
      skill_count: skillIds.length,
      tags: ["GitHub"],
      source: { type: "github" },
      created_at: "",
      updated_at: "",
      downloaded: true,
      update_available: false,
      local_id: entry.id,
      local_source: null,
      local_source_detail: null,
      installed_at: null,
      local_hub_updated_at: null,
      remote_updated_at: "",
      dependency_summary: {
        total: skillIds.length,
        downloaded: skillIds.length,
        missing: 0,
        update_available: 0,
        conflicts: [],
      },
    })
  }
  return items
}

export function mergeHubEmployeeViews(
  remote: HubEmployeeView[],
  localGithub: HubEmployeeView[],
): HubEmployeeView[] {
  const byId = new Map(remote.map((employee) => [employee.id, employee]))
  for (const employee of localGithub) {
    if (!byId.has(employee.id)) {
      byId.set(employee.id, employee)
    }
  }
  return Array.from(byId.values())
}
