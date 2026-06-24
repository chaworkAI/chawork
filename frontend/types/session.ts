export interface SessionMeta {
  id: string
  workspace_id: string
  title: string
  created_at: string
  last_message_at: string
  message_count: number
  status: "active" | "archived"
  /** User-renamed titles are not overwritten by auto sync. */
  title_locked?: boolean
}
