export type HubDownloadFilter =
  | "all"
  | "remote"
  | "local"
  | "update_available"
  | "custom"

export interface PaginatedResponse<T> {
  total: number
  page: number
  limit: number
  items: T[]
}

export interface HubLocalState {
  downloaded: boolean
  update_available: boolean
  local_id?: string | null
  local_source?: HubLocalSource | null
  local_source_detail?: string | null
  installed_at?: string | null
  local_hub_updated_at?: string | null
  remote_updated_at: string
}

export type HubLocalSource = "hub" | "custom" | "other_hub" | "other_kind"

export interface HubSkill {
  id: string
  name: string
  description_zh: string
  description_en: string
  profession: string
  content_hash: string
  source: Record<string, unknown>
  tags: string[]
  created_at: string
  updated_at: string
}

export interface HubSkillDetail extends HubSkill {
  skill_md: string
  referenced_by_employees: string[]
}

export type HubSkillView = HubSkill & HubLocalState

export interface HubEmployee {
  id: string
  name: string
  description: string
  kind: string
  prompt_preview: string
  skill_ids: string[]
  skill_count: number
  tags: string[]
  source: Record<string, unknown>
  created_at: string
  updated_at: string
}

export interface HubEmployeeSkillRef {
  id: string
  name: string
  description_zh: string
}

export interface HubEmployeeDetail extends HubEmployee {
  prompt_md: string
  skills: HubEmployeeSkillRef[]
}

export interface HubEmployeeDependencySummary {
  total: number
  downloaded: number
  missing: number
  update_available: number
  conflicts: string[]
}

export interface HubEmployeeView extends HubEmployee, HubLocalState {
  dependency_summary: HubEmployeeDependencySummary
}

export interface HubSkillInstallResult {
  hub_id: string
  local_id: string
  path: string
  installed_at: string
}

export interface HubEmployeeInstallResult {
  hub_id: string
  local_id: string
  path: string
  installed_at: string
  root_skill_ids: string[]
}

export interface HubManifest {
  total_skills: number
  total_employees: number
  professions: ProfessionInfo[]
}

export interface ProfessionInfo {
  name: string
  skill_count: number
  employee_count: number
}

export interface HubListSkillsParams {
  query?: string
  profession?: string
  filter?: HubDownloadFilter
  page?: number
  limit?: number
}

export interface HubListEmployeesParams {
  query?: string
  tags?: string
  filter?: HubDownloadFilter
  page?: number
  limit?: number
}

export interface HubGithubImportSkillPreview {
  id: string
  name: string
  profession: string
  description_zh?: string
  description_en?: string
}

export interface HubGithubImportJob {
  id: string
  job_id?: string
  url: string
  ref?: string | null
  status: string
  imported: number
  skills: HubGithubImportSkillPreview[]
  started_at?: string | null
  error?: string | null
  finished_at?: string | null
}

export interface HubGithubImportCompleteResult {
  installedSkillCount: number
  rootSkillIds: string[]
  employeeId?: string | null
  employeeName?: string | null
  employeeCreated?: boolean
  failedHubSkillIds: string[]
}

export interface HubCompleteGithubImportOptions {
  createEmployee: boolean
  employeePrompt?: string
}

// ─── GitHub 直接导入 ──────────────────────────────────────────────

export interface GithubDirectImportResult {
  installedCount: number
  skillIds: string[]
  failed: GithubDownloadResult[]
  employeeId: string | null
  employeeName: string | null
}

// ─── GitHub 直接扫描 ──────────────────────────────────────────────

export interface GithubSkillPreview {
  path: string
  name: string
  description: string
}

export interface GithubDownloadResult {
  skillId: string
  installed: boolean
  error?: string | null
}

export interface GithubBulkDownloadResult {
  installedCount: number
  skillIds: string[]
  failed: GithubDownloadResult[]
}
