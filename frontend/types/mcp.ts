export interface McpToolItem {
  id: string
  name: string
  description?: string
  enabled: boolean
  required_by_skills: string[]
}

export interface McpToolPolicyView {
  default_enabled: boolean
  tools: McpToolItem[]
  dirty: boolean
  updated_at?: string
}

export interface McpToolPolicyInput {
  default_enabled: boolean
  tools: Record<string, boolean>
}

export interface WorkspaceMcpServer {
  name: string
  type: "streamable_http" | "stdio"
  url?: string
  command?: string
  args: string[]
  env: Record<string, string>
  headers: Record<string, string>
  enabled: boolean
  required: boolean
  startup_timeout_sec?: number
  tool_timeout_sec?: number
  tools?: WorkspaceMcpServerTool[]
  last_tested_at?: string
}

export interface WorkspaceMcpServerView {
  servers: WorkspaceMcpServer[]
  updated_at?: string
}

export interface WorkspaceMcpServerTool {
  name: string
  description?: string
}

export interface WorkspaceMcpServerTestResult {
  ok: boolean
  message: string
  tools: WorkspaceMcpServerTool[]
}
