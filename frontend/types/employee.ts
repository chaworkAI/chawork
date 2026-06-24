export type EmployeeKind = "ordinary" | "dream"
export type EmployeeStatus = "active" | "archived"

export interface RegistryEntry {
  id: string
  kind: EmployeeKind
  name: string
  path: string
  status: EmployeeStatus
}

export interface EmployeeManifest {
  id: string
  name: string
  description: string
  kind: EmployeeKind
  status: EmployeeStatus
  created_at: string
  updated_at: string
}

export interface SkillEntry {
  id: string
  source: string
  copied_from?: string
  enabled: boolean
}

export interface SkillRegistry {
  version: number
  skills: SkillEntry[]
}

export interface WorkspaceMembership {
  id: string
  path: string
  name: string
  added_at: string
}

export interface IntegrityIssue {
  code: string
  message: string
}

export type IntegrityStatus = "ok" | "repair_needed"

export interface IntegrityReport {
  employee_id: string
  status: IntegrityStatus
  issues: IntegrityIssue[]
}

export interface EmployeeDetail {
  registry_entry: RegistryEntry
  manifest: EmployeeManifest | null
  integrity: IntegrityReport
}

export interface CreateEmployeeInput {
  /** Omit or leave empty — backend assigns a UUID automatically. */
  id?: string
  name: string
  description?: string
  initial_prompt?: string
  root_skill_ids?: string[]
}

export interface UpdateEmployeeInput {
  name?: string
  description?: string
  status?: EmployeeStatus
}

export interface EmployeeSkillSummary {
  id: string
  source: string
  copied_from?: string
  enabled: boolean
  name: string
  description: string
  path: string
  has_skill_md: boolean
}

// ── Workspace Binding ──────────────────────────────────────────────────────

export type BindingStatus =
  | "unbound"
  | "bound"
  | "employee_missing"
  | "membership_missing"
  | "path_mismatch"

export interface BindingValidation {
  status: BindingStatus
  employee_id: string | null
  employee_name: string | null
  message: string
}

// ── Dream Config ────────────────────────────────────────────────────────

export interface ScheduleConfig {
  /** "manual" | "daily" */
  type: string
  /** Employee-specific trigger time ("HH:MM"). null/undefined = use global default. */
  time?: string | null
}

export interface SessionScanConfig {
  /** "all" | "selected" */
  scope: string
  /** Workspace IDs to scan when scope is "selected" */
  workspace_subset: string[]
  /** Max sessions to scan per dream run */
  latest_sessions: number
}

export interface DreamConfig {
  enabled: boolean
  schedule: ScheduleConfig
  session_scan: SessionScanConfig
}

export interface DreamDefaults {
  /** Global default trigger time, e.g. "09:00" */
  default_dream_time: string
}

export interface DreamReadyPayload {
  employee_id: string
  employee_name: string
  dream_run_id: string
  selected_session_count: number
}

// ── Dream Types ─────────────────────────────────────────────────────────

export type DreamDecision = "no_update" | "update_required"

export interface SourceSessionRef {
  workspace_id: string
  session_id: string
  /** ISO 8601 timestamp of the session's last update */
  last_updated_at?: string | null
}

export interface PromptUpdate {
  section: string
  action: string
  content: string
  reason: string
}

export interface DreamResult {
  decision: DreamDecision
  target_employee_id: string
  dream_run_id: string
  summary: string
  source_sessions: SourceSessionRef[]
  updates?: PromptUpdate[]
  impact?: string
  /** "pending" | "approved" | "applying" | "applied" | "rejected" | "failed" */
  status: string
  /** e.g. "employees/{target}/prompt.md" */
  source_prompt_path?: string | null
  /** ISO 8601 creation timestamp */
  created_at?: string | null
}

export interface RecentDreamResult {
  dream_run_id: string
  target_employee_id: string
  decision: DreamDecision
  summary: string
  source_sessions: SourceSessionRef[]
  created_at: string
  parse_failed: boolean
  raw_output?: string
}

export interface PendingUpdateRequest {
  dream_run_id: string
  target_employee_id: string
  created_at: string
  result: DreamResult
  error_message?: string | null
}

export interface DreamLogEntry {
  timestamp: string
  event: string
  message: string
}

export interface ApplyResult {
  success: boolean
  target_employee_id: string
  dream_run_id: string
  error?: string
}

export interface DreamPrepareResult {
  dream_run_id: string
  run_workspace_path: string
  selected_sessions: SelectedSession[]
  skipped_reason?: string
}

export interface SelectedSession {
  workspace_id: string
  workspace_name: string
  workspace_path: string
  session_id: string
  title: string
  last_message_at: string
  message_count: number
}
