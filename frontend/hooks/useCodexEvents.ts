import { useEffect } from "react"
import { listen } from "@tauri-apps/api/event"

import { applyLabelTemplate } from "@/lib/builtinLabels"
import { formatJsonForDisplay, normalizeUserInputQuestions } from "@/lib/runtimePromptFormat"
import { eventMatchesActiveView, resolveRuntimeOwner } from "@/lib/runtimeOwner"
import { requestKindForReview, useRuntimeStore } from "@/stores/runtime"
import { useChatStore } from "@/stores/chat"
import { useDomainStore } from "@/stores/domain"
import { useSessionStore } from "@/stores/session"
import { useWorkspaceStore } from "@/stores/workspace"
import type { CodexEvent, ReviewPanelEntry, RuntimeEvent } from "@/types/events"

function makeId() {
  return crypto.randomUUID()
}

function now() {
  return new Date().toISOString()
}

function addRuntimeEvent(owner: { workspaceId: string | null; sessionId: string | null }, event: RuntimeEvent) {
  useRuntimeStore.getState().addEventForOwner(owner, event)
}

function runtimeEvent(
  payload: CodexEvent,
  displayLabel: string,
  displayStatus: RuntimeEvent["displayStatus"],
  detail?: string,
): RuntimeEvent {
  return {
    id: makeId(),
    timestamp: now(),
    event: payload,
    displayLabel,
    displayStatus,
    detail,
  }
}

function detailJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function readStringField(value: unknown, ...keys: string[]): string | undefined {
  if (!value || typeof value !== "object") return undefined
  const record = value as Record<string, unknown>
  for (const key of keys) {
    const field = record[key]
    if (typeof field === "string" && field.trim()) return field.trim()
  }
  return undefined
}

function formatRuntimeDebugLabel(payload: {
  method: string
  category: string
  params: unknown
}): string {
  const params = payload.params
  if (payload.method === "runtime/audit") {
    const capability = readStringField(params, "capability", "type")
    if (capability) {
      const short = capability.split(".").slice(-2).join(".")
      return `审计 · ${short}`
    }
    return "运行时审计"
  }
  if (payload.method.startsWith("codex/")) {
    const kind = readStringField(params, "codexKind", "kind", "type")
    return kind ? `Codex · ${kind}` : "Codex 通知"
  }
  return `Runtime · ${payload.category}`
}

function addReview(
  owner: { workspaceId: string | null; sessionId: string | null },
  entry: ReviewPanelEntry,
) {
  useRuntimeStore
    .getState()
    .addReviewForOwner(owner, entry, requestKindForReview(entry))
}

export function useCodexEvents() {
  const addAssistantDelta = useChatStore((s) => s.addAssistantDelta)
  const addThinkingDelta = useChatStore((s) => s.addThinkingDelta)
  const finishThinking = useChatStore((s) => s.finishThinking)
  const completeAssistantMessage = useChatStore((s) => s.completeAssistantMessage)
  const revealAssistantAnimated = useChatStore((s) => s.revealAssistantAnimated)
  const setStreaming = useChatStore((s) => s.setStreaming)
  const syncTranscriptFromBackend = useChatStore((s) => s.syncTranscriptFromBackend)
  const finalizeTurn = useChatStore((s) => s.finalizeTurn)
  const finalizeEmptyAssistantTurn = useChatStore((s) => s.finalizeEmptyAssistantTurn)
  const cancelStream = useChatStore((s) => s.cancelStream)

  useEffect(() => {
    const unlisten = listen<CodexEvent>("codex-event", ({ payload }) => {
      const context = {
        activeWorkspaceId: useWorkspaceStore.getState().activeWorkspaceId,
        activeSessionId: useSessionStore.getState().activeSessionId,
      }
      const owner = resolveRuntimeOwner(payload, context)
      const ownerInput = {
        workspaceId: owner.workspaceId,
        sessionId: owner.sessionId,
      }
      const activeView = eventMatchesActiveView(owner, context)
      const runtime = useRuntimeStore.getState()
      const getLabel = useDomainStore.getState().getLabel

      runtime.applyCodexEvent(ownerInput, payload)

      switch (payload.type) {
        case "ready":
          break

        case "runtime_debug":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              formatRuntimeDebugLabel(payload),
              "info",
              detailJson(payload.params),
            ),
          )
          break

        case "assistant_delta":
          if (activeView) {
            setStreaming(true)
            addAssistantDelta(payload.content)
          }
          break

        case "assistant_done": {
          if (!activeView) break
          const msgs = useChatStore.getState().messages
          const last = msgs[msgs.length - 1]
          const streamedLen =
            last?.role === "assistant" ? last.content.length : 0
          const fullLen = payload.content.length
          if (fullLen > 80 && streamedLen < Math.floor(fullLen * 0.35)) {
            void revealAssistantAnimated(payload.content)
          } else {
            completeAssistantMessage(payload.content)
          }
          break
        }

        case "thinking": {
          if (activeView) {
            setStreaming(true)
            if (payload.summary !== "正在生成回复…") {
              addThinkingDelta(payload.summary)
            }
          }
          const summary =
            payload.summary.length > 64
              ? `${payload.summary.slice(0, 64)}…`
              : payload.summary
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                getLabel("events.thinking.prefix", "思考 · {{summary}}"),
                { summary },
              ),
              "info",
            ),
          )
          break
        }

        case "thinking_delta": {
          if (activeView) {
            setStreaming(true)
            addThinkingDelta(payload.content)
          }
          const runtimeEvents = runtime.getWorkspaceEvents(owner.workspaceId)
          for (let i = runtimeEvents.length - 1; i >= 0; i--) {
            if (runtimeEvents[i].event.type === "thinking") {
              runtime.updateEventLiveContentForOwner(
                ownerInput,
                runtimeEvents[i].id,
                payload.content,
              )
              break
            }
          }
          break
        }

        case "thinking_done":
          if (activeView) finishThinking()
          break

        case "tool_call":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                getLabel("events.tool.prefix", "工具 · {{tool}}"),
                { tool: payload.tool },
              ),
              "warning",
            ),
          )
          break

        case "tool_delta":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                getLabel("events.tool_delta.prefix", "工具输出 · {{tool}}"),
                { tool: payload.tool },
              ),
              "info",
              payload.content,
            ),
          )
          break

        case "tool_result":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                payload.error
                  ? getLabel("events.tool_result.fail", "工具结果（失败） · {{id}}")
                  : getLabel("events.tool_result.ok", "工具结果 · {{id}}"),
                { id: payload.tool ?? `${payload.id.slice(0, 8)}…` },
              ),
              payload.error ? "error" : "success",
            ),
          )
          break

        case "file_change": {
          const actionKey =
            payload.action === "create"
              ? "events.file.create"
              : payload.action === "modify"
                ? "events.file.modify"
                : "events.file.delete"
          const actionWord = getLabel(
            actionKey,
            payload.action === "create"
              ? "新建"
              : payload.action === "modify"
                ? "修改"
                : "删除",
          )
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                getLabel("events.file_change.label", "{{action}}文件 · {{path}}"),
                { action: actionWord, path: payload.path },
              ),
              payload.action === "delete"
                ? "warning"
                : payload.action === "modify"
                  ? "info"
                  : "success",
            ),
          )
          break
        }

        case "file_change_delta":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              getLabel("events.file_change_delta.label", "文件变更输出"),
              "info",
              payload.content,
            ),
          )
          break

        case "mcp_oauth_login_completed":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              payload.success
                ? applyLabelTemplate(
                    getLabel("events.mcp_oauth.ok", "MCP OAuth 完成 · {{server}}"),
                    { server: payload.server_name },
                  )
                : applyLabelTemplate(
                    getLabel("events.mcp_oauth.fail", "MCP OAuth 失败 · {{server}}"),
                    { server: payload.server_name },
                  ),
              payload.success ? "success" : "error",
              payload.error,
            ),
          )
          break

        case "mcp_server_status_updated":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                getLabel("events.mcp_server_status.label", "MCP 服务 · {{server}} · {{status}}"),
                { server: payload.server_name, status: payload.status },
              ),
              payload.status === "failed"
                ? "error"
                : payload.status === "ready"
                  ? "success"
                  : "info",
              payload.error,
            ),
          )
          break

        case "plan_update":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              payload.explanation
                ? applyLabelTemplate(
                    getLabel("events.plan.update_with_reason", "计划 · {{reason}}"),
                    { reason: payload.explanation.slice(0, 64) },
                  )
                : getLabel("events.plan.update", "计划更新"),
              "info",
            ),
          )
          break

        case "plan_delta":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              getLabel("events.plan.delta", "计划生成中"),
              "info",
              payload.content,
            ),
          )
          break

        case "plan_done":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              getLabel("events.plan.done", "计划完成"),
              "success",
              payload.content,
            ),
          )
          break

        case "retrieval":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              applyLabelTemplate(
                getLabel("events.retrieval.label", "检索 · {{query}}（{{n}} 条）"),
                {
                  query:
                    payload.query.slice(0, 40) +
                    (payload.query.length > 40 ? "…" : ""),
                  n: String(payload.results.length),
                },
              ),
              "info",
            ),
          )
          break

        case "approval_request": {
          const risk =
            payload.risk === "high" || payload.risk === "medium" || payload.risk === "low"
              ? payload.risk
              : "medium"
          const isPermissions = payload.method.includes("permissions")
          const params = payload.params as { permissions?: unknown } | null
          const approvalDiff = formatJsonForDisplay(payload.params) ?? undefined
          addReview(ownerInput, {
            id: payload.id,
            title: payload.title,
            description: payload.description,
            risk,
            ...(approvalDiff ? { diff: approvalDiff } : {}),
            status: "pending",
            ...(isPermissions
              ? {
                  runtime_permissions: {
                    permissions: params?.permissions ?? {},
                    params: payload.params,
                  },
                }
              : {
                  runtime_approval: {
                    method: payload.method,
                    params: payload.params,
                  },
                }),
            actionLabels: isPermissions
              ? {
                  negative: "拒绝",
                  middle: "本轮授予",
                  positive: "本会话授予",
                }
              : {
                  negative: "拒绝",
                  middle: "本次允许",
                  positive: "本会话允许",
                },
          })
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(payload, `审批 · ${payload.title}`, "warning", payload.description),
          )
          break
        }

        case "mcp_elicitation_request": {
          const title =
            payload.mode === "url"
              ? `MCP 授权 · ${payload.server_name}`
              : `MCP 表单 · ${payload.server_name}`
          const mcpDiff = formatJsonForDisplay(payload.params) ?? undefined
          addReview(ownerInput, {
            id: payload.id,
            title,
            description: payload.message,
            risk: "medium",
            ...(mcpDiff ? { diff: mcpDiff } : {}),
            status: "pending",
            mcp_elicitation: {
              serverName: payload.server_name,
              mode: payload.mode,
              message: payload.message,
              params: payload.params,
            },
            actionLabels: {
              negative: "拒绝",
              middle: "取消",
              positive: "接受",
            },
          })
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(payload, title, "warning", payload.message),
          )
          break
        }

        case "user_input_request": {
          const questions = normalizeUserInputQuestions(payload.questions)
          addReview(ownerInput, {
            id: payload.id,
            title: payload.title,
            description: payload.description,
            risk: "medium",
            status: "pending",
            user_input_request: {
              method: payload.method,
              questions,
              params: payload.params,
            },
            actionLabels: {
              negative: "取消",
              positive: "提交",
            },
          })
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(payload, `提问 · ${payload.title}`, "warning", payload.description),
          )
          break
        }

        case "error":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              payload.recoverable
                ? getLabel("events.error.recoverable", "遇到问题，正在重试...")
                : getLabel("events.error.fatal", "出现错误"),
              "error",
              payload.message,
            ),
          )
          if (activeView && !payload.recoverable) {
            setStreaming(false)
            finalizeTurn()
          }
          break

        case "turn_complete": {
          if (!activeView) break
          const msgs = useChatStore.getState().messages
          const last = msgs[msgs.length - 1]
          const hasAssistantText =
            last?.role === "assistant" && last.content.trim().length > 0
          if (hasAssistantText) {
            finalizeTurn()
          } else {
            const hasThinking =
              last?.role === "assistant" &&
              Boolean(last.thinkingContent?.trim() || last.isThinkingStreaming)
            if (hasThinking) {
              finalizeEmptyAssistantTurn()
            } else {
              void syncTranscriptFromBackend().finally(() => {
                finalizeTurn()
              })
            }
          }
          break
        }

        case "cancelled":
          if (activeView) {
            cancelStream()
            finalizeTurn()
          }
          break

        case "skill_write":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              `Skill 写入 · ${payload.target_path}`,
              payload.status === "error"
                ? "error"
                : payload.status === "synced"
                  ? "success"
                  : "warning",
              payload.message,
            ),
          )
          break

        case "skill_refresh":
          addRuntimeEvent(
            ownerInput,
            runtimeEvent(
              payload,
              "Runtime 刷新",
              payload.status === "error"
                ? "error"
                : payload.status === "synced"
                  ? "success"
                  : "warning",
              payload.message,
            ),
          )
          break

        default:
          break
      }
    })

    return () => {
      void unlisten.then((fn) => fn())
    }
  }, [
    addAssistantDelta,
    addThinkingDelta,
    finishThinking,
    completeAssistantMessage,
    revealAssistantAnimated,
    cancelStream,
    finalizeTurn,
    finalizeEmptyAssistantTurn,
    setStreaming,
    syncTranscriptFromBackend,
  ])
}
