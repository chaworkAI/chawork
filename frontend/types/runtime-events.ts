export type RuntimeEventType =
  | "thinking"
  | "assistant_delta"
  | "tool_call"
  | "tool_delta"
  | "tool_complete"
  | "tool_result"
  | "retrieval"
  | "file_change"
  | "file_change_delta"
  | "mcp_oauth_login_completed"
  | "mcp_server_status_updated"
  | "plan_update"
  | "plan_delta"
  | "plan_done"
  | "skill_write"
  | "skill_refresh"
  | "error"
  | "turn_complete"
  | "cancelled"

interface BaseEvent {
  type: RuntimeEventType
  timestamp?: string
}

export interface ThinkingEvent extends BaseEvent {
  type: "thinking"
  content: string
}

export interface AssistantDeltaEvent extends BaseEvent {
  type: "assistant_delta"
  content: string
}

export interface ToolCallEvent extends BaseEvent {
  type: "tool_call"
  tool: string
  args: Record<string, unknown>
  call_id?: string
}

export interface ToolDeltaEvent extends BaseEvent {
  type: "tool_delta"
  tool: string
  content: string
  call_id?: string
}

export interface ToolCompleteEvent extends BaseEvent {
  type: "tool_complete"
  tool: string
  call_id?: string
}

export interface ToolResultEvent extends BaseEvent {
  type: "tool_result"
  tool: string
  result: unknown
  call_id?: string
  is_error?: boolean
}

export interface RetrievalEvent extends BaseEvent {
  type: "retrieval"
  query: string
  paths: string[]
  scores: number[]
}

export interface FileChangeEvent extends BaseEvent {
  type: "file_change"
  action: "write" | "delete" | "rename"
  id?: string
  path: string
  diff?: string
  summary?: string
}

export interface FileChangeDeltaEvent extends BaseEvent {
  type: "file_change_delta"
  id: string
  content: string
}

export interface McpOauthLoginCompletedEvent extends BaseEvent {
  type: "mcp_oauth_login_completed"
  server_name: string
  success: boolean
  error?: string
}

export interface McpServerStatusUpdatedEvent extends BaseEvent {
  type: "mcp_server_status_updated"
  server_name: string
  status: string
  error?: string
}

export interface PlanStep {
  step: string
  status: "pending" | "inProgress" | "completed" | string
}

export interface PlanUpdateEvent extends BaseEvent {
  type: "plan_update"
  explanation?: string
  steps: PlanStep[]
}

export interface PlanDeltaEvent extends BaseEvent {
  type: "plan_delta"
  content: string
}

export interface PlanDoneEvent extends BaseEvent {
  type: "plan_done"
  content: string
}

export interface SkillWriteEvent extends BaseEvent {
  type: "skill_write"
  scope: "workspace" | "root"
  executor: "workspace-tools" | "chawork-app"
  target_path: string
  workspace_id?: string
  source_checksum?: string
  target_checksum?: string
  status: "started" | "synced" | "pending" | "error"
  message: string
}

export interface SkillRefreshEvent extends BaseEvent {
  type: "skill_refresh"
  workspace_id?: string
  generation?: number
  status: "started" | "synced" | "pending" | "error"
  message: string
}

export interface ErrorEvent extends BaseEvent {
  type: "error"
  message: string
  recoverable: boolean
}

export interface TurnCompleteEvent extends BaseEvent {
  type: "turn_complete"
  usage?: {
    prompt_tokens?: number
    completion_tokens?: number
    total_tokens?: number
    input_tokens?: number
    cached_input_tokens?: number
    output_tokens?: number
    reasoning_output_tokens?: number
    model_context_window?: number
  }
}

export interface CancelledEvent extends BaseEvent {
  type: "cancelled"
}

export type RuntimeEvent =
  | ThinkingEvent
  | AssistantDeltaEvent
  | ToolCallEvent
  | ToolDeltaEvent
  | ToolCompleteEvent
  | ToolResultEvent
  | RetrievalEvent
  | FileChangeEvent
  | FileChangeDeltaEvent
  | McpOauthLoginCompletedEvent
  | McpServerStatusUpdatedEvent
  | PlanUpdateEvent
  | PlanDeltaEvent
  | PlanDoneEvent
  | SkillWriteEvent
  | SkillRefreshEvent
  | ErrorEvent
  | TurnCompleteEvent
  | CancelledEvent
