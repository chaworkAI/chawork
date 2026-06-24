export interface ToolDef {
  name: string
  description: string
  parameters: Record<string, unknown>
}

export interface ContextSnippet {
  source: string
  content: string
  relevance?: number
}

export interface SearchResult {
  path: string
  snippet: string
  score: number
}

export interface TokenUsage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  input_tokens?: number
  cached_input_tokens?: number
  output_tokens?: number
  reasoning_output_tokens?: number
  model_context_window?: number
}

export type FileAction = "create" | "modify" | "delete"

export interface PlanStep {
  step: string
  status: "pending" | "inProgress" | "completed" | string
}

export interface CodexEventOwner {
  workspace_id?: string
  session_id?: string
}

export type CodexEvent = CodexEventOwner & (
  | { type: "ready" }
  | { type: "assistant_delta"; content: string }
  | { type: "assistant_done"; content: string }
  | { type: "thinking"; summary: string }
  | { type: "thinking_delta"; content: string }
  | { type: "thinking_done" }
  | { type: "tool_call"; tool: string; args: unknown; id: string }
  | { type: "tool_delta"; tool: string; content: string; id: string }
  | { type: "tool_result"; id: string; tool?: string; result: unknown; error?: string }
  | { type: "file_change"; path: string; diff: string; action: FileAction }
  | { type: "file_change_delta"; id: string; content: string }
  | {
      type: "mcp_oauth_login_completed"
      server_name: string
      success: boolean
      error?: string
    }
  | {
      type: "mcp_server_status_updated"
      server_name: string
      status: string
      error?: string
    }
  | { type: "plan_update"; explanation?: string; steps: PlanStep[] }
  | { type: "plan_delta"; content: string }
  | { type: "plan_done"; content: string }
  | { type: "retrieval"; query: string; results: SearchResult[] }
  | {
      type: "approval_request"
      id: string
      method: string
      title: string
      description: string
      risk: "low" | "medium" | "high" | string
      params: unknown
    }
  | {
      type: "user_input_request"
      id: string
      method: string
      title: string
      description: string
      questions: UserInputQuestion[]
      params: unknown
    }
  | {
      type: "mcp_elicitation_request"
      id: string
      server_name: string
      mode: "form" | "url" | string
      message: string
      params: unknown
    }
  | {
      type: "runtime_debug"
      method: string
      category: "raw" | "audit" | "runtime" | string
      params: unknown
    }
  | {
      type: "skill_write"
      scope: "workspace" | "root"
      executor: string
      target_path: string
      status: "started" | "synced" | "pending" | "error"
      message: string
    }
  | {
      type: "skill_refresh"
      generation?: number
      status: "started" | "synced" | "pending" | "error"
      message: string
    }
  | { type: "error"; message: string; recoverable: boolean }
  | { type: "turn_complete"; usage?: TokenUsage }
  | { type: "cancelled" }
)

export type RuntimeEventCategory = "all" | "tool" | "file" | "mcp" | "system"

export type CodexStatus =
  | "idle"
  | "thinking"
  | "executing"
  | "pending_request"
  | "cancelling"
  | "error"

export interface RuntimeRequestOwner {
  workspaceId: string
  sessionId: string | null
  requestId: string
  kind: "approval" | "permissions" | "user_input" | "mcp_elicitation"
}

export interface UserInputOption {
  label: string
  description: string
}

export interface UserInputQuestion {
  id: string
  header: string
  question: string
  isOther?: boolean
  isSecret?: boolean
  options?: UserInputOption[] | null
}

export interface RuntimeEvent {
  id: string
  timestamp: string
  event: CodexEvent
  displayLabel: string
  displayStatus: "success" | "warning" | "error" | "info"
  /** Body shown below the summary when expanded */
  detail?: string
  /** Live-updating text content (e.g. streaming thinking deltas) */
  liveContent?: string
}

export type ProposalType =
  | "schema_update"
  | "wiki_update"
  | "skill_update"
  | "template_update"
  | "report_draft"

export type ProposalStatus = "draft" | "accepted" | "rejected"

export interface Proposal {
  id: string
  title: string
  description: string
  proposal_type: ProposalType
  source_session?: string
  diff?: string
  target_path?: string
  new_content?: string
  created_at: string
  status: ProposalStatus
  resolved_at?: string
  risk?: string
  is_skill_proposal?: boolean
  target_scope?: "workspace" | "root"
  executor?: "workspace-tools" | "chawork-app"
  impact_summary?: string
  skill_id?: string
  affected_workspaces?: string[]
}

export interface ReviewItem {
  id: string
  title: string
  description: string
  risk: "low" | "medium" | "high"
  diff?: string
  status: "pending" | "accepted" | "rejected" | "applying" | "error"
  proposalType?: ProposalType
  targetPath?: string
  is_skill_proposal?: boolean
  target_scope?: "workspace" | "root"
  executor?: "workspace-tools" | "chawork-app"
  impact_summary?: string
  skill_id?: string
  runtime_approval?: {
    method: string
    params: unknown
  }
  user_input_request?: {
    method: string
    questions: UserInputQuestion[]
    params: unknown
  }
  /** Permissions grant request: accept replays the requested profile as granted. */
  runtime_permissions?: {
    /** RequestPermissionProfile from Codex; replayed as GrantedPermissionProfile on accept. */
    permissions: unknown
    params: unknown
  }
  /** MCP server elicitation: form (fill schema) or url (open link) mode. */
  mcp_elicitation?: {
    serverName: string
    mode: "form" | "url" | string
    message: string
    params: unknown
  }
  owner?: RuntimeRequestOwner
}

/** Review row including optional localized action button labels */
export interface ReviewPanelEntry extends ReviewItem {
  actionLabels?: {
    negative: string
    middle?: string
    positive: string
  }
  affected_workspaces?: string[]
}
