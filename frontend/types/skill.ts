export type SkillScope = "root" | "workspace"
export type SkillEffectiveMode = "unselected_root" | "selected_root" | "workspace_local" | "workspace_override"
export type SkillExecutor = "workspace-tools" | "chawork-app"

export interface SkillSummary {
  id: string
  name: string
  scope: SkillScope
  effective_mode: SkillEffectiveMode
  path: string
  description: string
  version?: string
  checksum: string
  root_checksum?: string
  source?: "manual" | "curated" | "root_catalog" | "workspace_local" | "workspace_override"
  updated_at: string
  enabled: boolean
  executor_for_write: SkillExecutor
  runtime_status: "synced" | "dirty" | "error"
  depends_on_tools?: string[]
}

export interface SkillSelectionView {
  root_skills: Record<string, {
    enabled: boolean
    mode: "follow_root"
    root_checksum: string
  }>
  workspace_skills: Record<string, {
    enabled: boolean
  }>
  dirty: boolean
  updated_at?: string
}

export interface SkillProposal {
  id: string
  skill_id: string
  source_scope: SkillScope
  target_scope: SkillScope
  target_kind: "skill_selection" | "skill_source" | "skill_override" | "skill_promotion"
  executor: SkillExecutor
  workspace_id?: string
  source_root_skill_id?: string
  target_path: string
  affected_workspaces: string[]
  impact_summary: string
  diff: string
}

export interface SkillPromotionResult {
  ok: boolean
  root_skill: SkillSummary
  affected_workspaces: string[]
  message?: string
}

export interface SkillListView {
  root_catalog: SkillSummary[]
  workspace_selection: SkillSummary[]
  workspace_local: SkillSummary[]
}

export interface SkillRuntimeEvent {
  type: "skill_write" | "skill_refresh"
  scope: SkillScope
  executor: SkillExecutor
  target_path: string
  workspace_id?: string
  source_checksum?: string
  target_checksum?: string
  generation?: number
  status: "started" | "synced" | "pending" | "error"
  message: string
}
