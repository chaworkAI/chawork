import type { RuntimeEvent as DisplayRuntimeEvent, CodexEvent } from "@/types/events"
import type { RuntimeEvent as StreamRuntimeEvent } from "@/types/runtime-events"

function displayStatusForSkill(
  status: "started" | "synced" | "pending" | "error",
): DisplayRuntimeEvent["displayStatus"] {
  if (status === "error") return "error"
  if (status === "synced") return "success"
  return "warning"
}

function streamEventToCodex(event: StreamRuntimeEvent): CodexEvent | null {
  switch (event.type) {
    case "thinking":
      return { type: "thinking", summary: event.content }
    case "tool_call":
      return {
        type: "tool_call",
        tool: event.tool,
        args: event.args,
        id: event.call_id ?? crypto.randomUUID(),
      }
    case "tool_delta":
      return {
        type: "tool_delta",
        tool: event.tool,
        content: event.content,
        id: event.call_id ?? crypto.randomUUID(),
      }
    case "tool_result":
      return {
        type: "tool_result",
        id: event.call_id ?? crypto.randomUUID(),
        tool: event.tool,
        result: event.result,
        error: event.is_error ? "error" : undefined,
      }
    case "retrieval":
      return {
        type: "retrieval",
        query: event.query,
        results: event.paths.map((path, i) => ({
          path,
          snippet: "",
          score: event.scores[i] ?? 0,
        })),
      }
    case "file_change":
      return {
        type: "file_change",
        path: event.path,
        diff: event.diff ?? "",
        action:
          event.action === "write"
            ? "modify"
            : event.action === "delete"
              ? "delete"
              : "modify",
      }
    case "file_change_delta":
      return {
        type: "file_change_delta",
        id: event.id,
        content: event.content,
      }
    case "mcp_oauth_login_completed":
      return {
        type: "mcp_oauth_login_completed",
        server_name: event.server_name,
        success: event.success,
        error: event.error,
      }
    case "mcp_server_status_updated":
      return {
        type: "mcp_server_status_updated",
        server_name: event.server_name,
        status: event.status,
        error: event.error,
      }
    case "plan_update":
      return {
        type: "plan_update",
        explanation: event.explanation,
        steps: event.steps,
      }
    case "plan_delta":
      return {
        type: "plan_delta",
        content: event.content,
      }
    case "plan_done":
      return {
        type: "plan_done",
        content: event.content,
      }
    case "skill_write":
      return {
        type: "skill_write",
        scope: event.scope,
        executor: event.executor,
        target_path: event.target_path,
        status: event.status,
        message: event.message,
      }
    case "skill_refresh":
      return {
        type: "skill_refresh",
        generation: event.generation,
        status: event.status,
        message: event.message,
      }
    case "error":
      return {
        type: "error",
        message: event.message,
        recoverable: event.recoverable,
      }
    case "turn_complete":
      return {
        type: "turn_complete",
        usage: event.usage
          ? {
              prompt_tokens: event.usage.prompt_tokens ?? 0,
              completion_tokens: event.usage.completion_tokens ?? 0,
              total_tokens: event.usage.total_tokens ?? 0,
              input_tokens: event.usage.input_tokens,
              cached_input_tokens: event.usage.cached_input_tokens,
              output_tokens: event.usage.output_tokens,
              reasoning_output_tokens: event.usage.reasoning_output_tokens,
              model_context_window: event.usage.model_context_window,
            }
          : undefined,
      }
    case "cancelled":
      return { type: "cancelled" }
    case "assistant_delta":
      return null
    default:
      return null
  }
}

function displayLabel(event: StreamRuntimeEvent): string {
  switch (event.type) {
    case "thinking":
      return event.content.slice(0, 64) || "思考中…"
    case "tool_call":
      return `工具 · ${event.tool}`
    case "tool_delta":
      return `工具输出 · ${event.tool}`
    case "tool_result":
      return `工具结果 · ${event.tool}`
    case "retrieval":
      return `知识检索 · ${event.query.slice(0, 40)}`
    case "file_change":
      return `文件 · ${event.path}`
    case "file_change_delta":
      return "文件变更输出"
    case "mcp_oauth_login_completed":
      return event.success
        ? `MCP OAuth 完成 · ${event.server_name}`
        : `MCP OAuth 失败 · ${event.server_name}`
    case "mcp_server_status_updated":
      return `MCP 服务 · ${event.server_name} · ${event.status}`
    case "plan_update":
      return "计划更新"
    case "plan_delta":
      return "计划生成中"
    case "plan_done":
      return "计划完成"
    case "skill_write":
      return `Skill 写入 · ${event.target_path}`
    case "skill_refresh":
      return "Runtime 刷新"
    case "error":
      return event.recoverable ? "遇到问题，正在重试…" : "出现错误"
    case "turn_complete":
      return "本轮完成"
    case "cancelled":
      return "已取消"
    default:
      return event.type
  }
}

function displayStatus(event: StreamRuntimeEvent): DisplayRuntimeEvent["displayStatus"] {
  switch (event.type) {
    case "tool_call":
      return "warning"
    case "tool_delta":
      return "info"
    case "tool_result":
      return event.is_error ? "error" : "success"
    case "file_change":
      return "info"
    case "file_change_delta":
      return "info"
    case "mcp_oauth_login_completed":
      return event.success ? "success" : "error"
    case "mcp_server_status_updated":
      return event.status === "failed" ? "error" : event.status === "ready" ? "success" : "info"
    case "plan_update":
    case "plan_delta":
      return "info"
    case "plan_done":
      return "success"
    case "skill_write":
    case "skill_refresh":
      return displayStatusForSkill(event.status)
    case "error":
      return "error"
    case "turn_complete":
    case "cancelled":
      return "success"
    case "thinking":
      return "info"
    case "retrieval":
      return "info"
    default:
      return "info"
  }
}

export function mapStreamEventToDisplay(event: StreamRuntimeEvent): DisplayRuntimeEvent | null {
  const codex = streamEventToCodex(event)
  if (!codex && event.type !== "assistant_delta") return null

  if (event.type === "assistant_delta") return null

  return {
    id: crypto.randomUUID(),
    timestamp: event.timestamp ?? new Date().toISOString(),
    event: codex!,
    displayLabel: displayLabel(event),
    displayStatus: displayStatus(event),
    detail:
      event.type === "error"
        ? event.message
        : event.type === "skill_write" || event.type === "skill_refresh"
          ? event.message
          : undefined,
  }
}
