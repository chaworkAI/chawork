export type IndexStatus = "ready" | "stale" | "building" | "error"

export interface WorkspaceState {
  id: string
  name: string
  path: string
  created_at: string
  last_active_at: string
  active_session_id: string | null
  domain_pack_id: string | null
  index_status: IndexStatus
  /** Draft proposals count; filled when listing workspaces from backend. */
  pending_proposals_count?: number
  /** Bound employee name, if workspace is bound to an employee. */
  bound_employee_name?: string | null
  /** Bound employee id, if workspace is bound to an employee. */
  bound_employee_id?: string | null
}

/** Sidebar row: workspace folder + metadata for navigation UI */
export interface WorkspaceSidebarItem {
  workspace: WorkspaceState
  metaLine: string
}
