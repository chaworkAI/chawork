import { create } from "zustand"

import { mapStreamEventToDisplay } from "@/lib/runtimeEventMap"
import * as ipc from "@/lib/tauri"
import type {
  RefreshRuntimeContextResult,
  RuntimeInvalidationResult,
  RuntimeInvalidationUserMessageKey,
} from "@/lib/tauri"
import { useToastStore } from "@/stores/toast"
import type { CodexEvent, ReviewPanelEntry, RuntimeEvent } from "@/types/events"
import type { RuntimeEvent as StreamRuntimeEvent } from "@/types/runtime-events"

export type RuntimeBusyState =
  | "idle"
  | "thinking"
  | "executing"
  | "pending_request"
  | "cancelling"
  | "error"

export type RuntimeRequestKind =
  | "approval"
  | "permissions"
  | "user_input"
  | "mcp_elicitation"

export interface RuntimeRequestOwner {
  workspaceId: string
  sessionId: string | null
  requestId: string
  kind: RuntimeRequestKind
}

export interface RuntimeOwnerInput {
  workspaceId: string | null
  sessionId: string | null
}

export interface WorkspaceRuntimeState {
  workspaceId: string
  status: RuntimeBusyState
  activeSessionId: string | null
  activeTurnId: string | null
  events: RuntimeEvent[]
  reviews: ReviewPanelEntry[]
  pendingRequests: Record<string, RuntimeRequestOwner>
  restartRequired: boolean
  canRestartNow: boolean
  restartMessage: string | null
  lifecycleMessage: string | null
  lastUpdatedAt: string | null
}

interface RuntimeStore {
  runtimeByWorkspace: Record<string, WorkspaceRuntimeState>
  getWorkspaceRuntime: (workspaceId: string | null) => WorkspaceRuntimeState | null
  getWorkspaceEvents: (workspaceId: string | null) => RuntimeEvent[]
  getWorkspaceReviews: (workspaceId: string | null) => ReviewPanelEntry[]
  getRequestOwner: (requestId: string) => RuntimeRequestOwner | null
  isWorkspaceBusy: (workspaceId: string | null) => boolean
  applyRuntimeRefreshResult: (
    workspaceId: string,
    result: RefreshRuntimeContextResult,
  ) => void
  refreshWorkspaceRuntimeContext: (
    workspaceId: string,
    sessionId?: string,
  ) => Promise<RefreshRuntimeContextResult>
  applyRuntimeInvalidation: (payload: RuntimeInvalidationResult) => void
  upsertLifecycleNotice: (payload: RuntimeInvalidationResult) => void
  handleRuntimeInvalidation: (payload: RuntimeInvalidationResult) => void
  clearLifecycleNotice: (workspaceId: string | null) => void
  restartWorkspaceRuntime: (workspaceId: string) => Promise<void>
  markWorkspaceThinking: (workspaceId: string, sessionId: string | null) => void
  setWorkspaceStatus: (
    owner: RuntimeOwnerInput,
    status: RuntimeBusyState,
    activeTurnId?: string | null,
  ) => void
  applyCodexEvent: (owner: RuntimeOwnerInput, payload: CodexEvent) => void
  addEventForOwner: (owner: RuntimeOwnerInput, event: RuntimeEvent) => void
  appendEventForOwner: (owner: RuntimeOwnerInput, event: StreamRuntimeEvent) => void
  updateEventLiveContentForOwner: (
    owner: RuntimeOwnerInput,
    id: string,
    delta: string,
  ) => void
  clearWorkspaceEvents: (workspaceId: string | null) => void
  addReviewForOwner: (
    owner: RuntimeOwnerInput,
    item: ReviewPanelEntry,
    kind: RuntimeRequestKind,
  ) => void
  setReviewStatusForOwner: (
    owner: RuntimeOwnerInput,
    id: string,
    status: ReviewPanelEntry["status"],
  ) => void
  acceptReview: (id: string) => Promise<void>
  acceptReviewForSession: (id: string) => Promise<void>
  answerUserInput: (
    id: string,
    answers: Record<string, { answers: string[] }>,
  ) => Promise<void>
  answerMcpElicitation: (
    id: string,
    content?: unknown,
    meta?: unknown,
  ) => Promise<void>
  rejectReview: (id: string) => Promise<void>
}

function now() {
  return new Date().toISOString()
}

function emptyWorkspaceRuntime(workspaceId: string): WorkspaceRuntimeState {
  return {
    workspaceId,
    status: "idle",
    activeSessionId: null,
    activeTurnId: null,
    events: [],
    reviews: [],
    pendingRequests: {},
    restartRequired: false,
    canRestartNow: false,
    restartMessage: null,
    lifecycleMessage: null,
    lastUpdatedAt: null,
  }
}

function withWorkspace(
  state: Pick<RuntimeStore, "runtimeByWorkspace">,
  workspaceId: string,
  update: (runtime: WorkspaceRuntimeState) => WorkspaceRuntimeState,
): Record<string, WorkspaceRuntimeState> {
  const current =
    state.runtimeByWorkspace[workspaceId] ?? emptyWorkspaceRuntime(workspaceId)
  return {
    ...state.runtimeByWorkspace,
    [workspaceId]: update(current),
  }
}

function statusForEvent(event: CodexEvent): RuntimeBusyState | null {
  switch (event.type) {
    case "thinking":
    case "thinking_delta":
      return "thinking"
    case "tool_call":
    case "tool_result":
    case "file_change":
    case "retrieval":
    case "skill_write":
    case "skill_refresh":
      return "executing"
    case "approval_request":
    case "user_input_request":
    case "mcp_elicitation_request":
      return "pending_request"
    case "error":
      return event.recoverable ? "executing" : "error"
    case "turn_complete":
    case "cancelled":
      return "idle"
    default:
      return null
  }
}

function requestKindForReview(entry: ReviewPanelEntry): RuntimeRequestKind {
  if (entry.runtime_permissions) return "permissions"
  if (entry.user_input_request) return "user_input"
  if (entry.mcp_elicitation) return "mcp_elicitation"
  return "approval"
}

function ownerOrNull(owner: RuntimeOwnerInput): RuntimeRequestOwner | null {
  if (!owner.workspaceId) return null
  return {
    workspaceId: owner.workspaceId,
    sessionId: owner.sessionId,
    requestId: "",
    kind: "approval",
  }
}

function findReview(
  runtimes: Record<string, WorkspaceRuntimeState>,
  requestId: string,
): { owner: RuntimeRequestOwner; entry: ReviewPanelEntry } | null {
  for (const runtime of Object.values(runtimes)) {
    const owner = runtime.pendingRequests[requestId]
    if (!owner) continue
    const entry = runtime.reviews.find((r) => r.id === requestId)
    if (entry) return { owner, entry }
  }
  return null
}

function invokeOwner(owner: RuntimeRequestOwner) {
  return {
    workspaceId: owner.workspaceId,
    sessionId: owner.sessionId,
  }
}

const lifecycleMessages: Record<RuntimeInvalidationUserMessageKey, string> = {
  settings_saved_no_active_turn:
    "设置已保存。后续消息将使用最新设置；当前会话仍保留已有上下文。",
  settings_saved_active_task_uses_previous:
    "设置已保存。当前任务会按修改前的设置完成，后续消息将使用最新设置。",
  provider_settings_saved_active_task_uses_previous:
    "模型配置已保存。当前任务会按修改前的模型配置完成，后续消息将使用最新配置。",
  employee_settings_saved_active_task_uses_previous:
    "员工设置已保存。当前任务会按修改前的员工设置完成，后续消息将使用最新设置。",
  dream_prompt_applied_later_messages:
    "方法更新已应用。后续消息将使用更新后的方法；当前会话仍保留已有上下文。",
  workspace_binding_saved_later_messages:
    "工作区绑定已更新。后续消息将使用新的员工设置；当前会话仍保留已有上下文。",
  settings_saved_cleanup_warning:
    "设置已保存。后台清理出现异常，已记录诊断。",
}

function lifecycleToastKey(payload: RuntimeInvalidationResult): string {
  return `runtime-lifecycle:${payload.scope}:${payload.scopeIdentity}:${payload.reason}`
}

function lifecycleUserMessage(payload: RuntimeInvalidationResult): string | null {
  if (payload.phase === "completed") {
    return payload.terminationWarnings.length > 0
      ? lifecycleMessages.settings_saved_cleanup_warning
      : null
  }
  if (payload.invalidatedNowCount === 0 && payload.deferredCount === 0) {
    return null
  }
  if (payload.terminationWarnings.length > 0) {
    return lifecycleMessages.settings_saved_cleanup_warning
  }
  if (payload.userMessageKey) {
    return lifecycleMessages[payload.userMessageKey]
  }
  if (payload.deferredCount > 0) {
    return lifecycleMessages.settings_saved_active_task_uses_previous
  }
  return lifecycleMessages.settings_saved_no_active_turn
}

export const useRuntimeStore = create<RuntimeStore>((set, get) => ({
  runtimeByWorkspace: {},

  getWorkspaceRuntime: (workspaceId) => {
    if (!workspaceId) return null
    return get().runtimeByWorkspace[workspaceId] ?? emptyWorkspaceRuntime(workspaceId)
  },

  getWorkspaceEvents: (workspaceId) => {
    if (!workspaceId) return []
    return get().runtimeByWorkspace[workspaceId]?.events ?? []
  },

  getWorkspaceReviews: (workspaceId) => {
    if (!workspaceId) return []
    return get().runtimeByWorkspace[workspaceId]?.reviews ?? []
  },

  getRequestOwner: (requestId) => {
    for (const runtime of Object.values(get().runtimeByWorkspace)) {
      const owner = runtime.pendingRequests[requestId]
      if (owner) return owner
    }
    return null
  },

  isWorkspaceBusy: (workspaceId) => {
    if (!workspaceId) return false
    const runtime = get().runtimeByWorkspace[workspaceId]
    if (!runtime) return false
    if (Object.values(runtime.pendingRequests).length > 0) return true
    return !["idle", "error"].includes(runtime.status)
  },

  applyRuntimeRefreshResult: (workspaceId, result) => {
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, workspaceId, (runtime) => ({
        ...runtime,
        restartRequired: result.restart_required,
        canRestartNow: result.can_restart_now,
        restartMessage: result.restart_required ? (result.message ?? null) : null,
        lastUpdatedAt: now(),
      })),
    }))
  },

  refreshWorkspaceRuntimeContext: async (workspaceId, sessionId) => {
    const result = await ipc.refreshRuntimeContext(workspaceId, sessionId)
    get().applyRuntimeRefreshResult(workspaceId, result)
    return result
  },

  applyRuntimeInvalidation: (payload) => {
    const message = lifecycleUserMessage(payload)
    set((state) => {
      let next = state.runtimeByWorkspace
      for (const affected of payload.affectedWorkspaces) {
        next = withWorkspace({ runtimeByWorkspace: next }, affected.workspaceId, (runtime) => {
          const inactive =
            affected.mode === "immediate" ||
            affected.mode === "completed" ||
            affected.mode === "noop"
          return {
            ...runtime,
            status: inactive ? "idle" : runtime.status,
            restartRequired: false,
            canRestartNow: false,
            restartMessage: null,
            lifecycleMessage:
              payload.phase === "marked" && message
                ? message
                : inactive
                  ? null
                  : runtime.lifecycleMessage,
            lastUpdatedAt: now(),
          }
        })
      }
      return { runtimeByWorkspace: next }
    })
  },

  upsertLifecycleNotice: (payload) => {
    const message = lifecycleUserMessage(payload)
    const toastId = lifecycleToastKey(payload)
    if (!message) {
      return
    }
    useToastStore
      .getState()
      .show(
        message,
        payload.terminationWarnings.length > 0 ? "warning" : "info",
        toastId,
      )
  },

  handleRuntimeInvalidation: (payload) => {
    get().applyRuntimeInvalidation(payload)
    get().upsertLifecycleNotice(payload)
  },

  clearLifecycleNotice: (workspaceId) => {
    if (!workspaceId) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, workspaceId, (runtime) => ({
        ...runtime,
        lifecycleMessage: null,
        lastUpdatedAt: now(),
      })),
    }))
  },

  restartWorkspaceRuntime: async (workspaceId) => {
    await ipc.startWorkspaceRuntime(workspaceId)
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, workspaceId, (runtime) => ({
        ...runtime,
        status: "idle",
        restartRequired: false,
        canRestartNow: false,
        restartMessage: null,
        lifecycleMessage: null,
        lastUpdatedAt: now(),
      })),
    }))
  },

  markWorkspaceThinking: (workspaceId, sessionId) =>
    get().setWorkspaceStatus({ workspaceId, sessionId }, "thinking"),

  setWorkspaceStatus: (owner, status, activeTurnId) => {
    if (!owner.workspaceId) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, owner.workspaceId!, (runtime) => ({
        ...runtime,
        status,
        canRestartNow: runtime.restartRequired
          ? ["idle", "error"].includes(status)
          : runtime.canRestartNow,
        activeSessionId:
          status === "idle" || status === "error"
            ? null
            : (owner.sessionId ?? runtime.activeSessionId),
        activeTurnId:
          activeTurnId === undefined
            ? runtime.activeTurnId
            : activeTurnId,
        lastUpdatedAt: now(),
      })),
    }))
  },

  applyCodexEvent: (owner, payload) => {
    if (!owner.workspaceId) return
    const status = statusForEvent(payload)
    if (!status) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, owner.workspaceId!, (runtime) => {
        const pendingRequests =
          status === "idle"
            ? Object.fromEntries(
                Object.entries(runtime.pendingRequests).filter(
                  ([, req]) =>
                    req.sessionId &&
                    owner.sessionId &&
                    req.sessionId !== owner.sessionId,
                ),
              )
            : runtime.pendingRequests
        return {
          ...runtime,
          status,
          canRestartNow: runtime.restartRequired
            ? ["idle", "error"].includes(status)
            : runtime.canRestartNow,
          activeSessionId:
            status === "idle" || status === "error"
              ? null
              : (owner.sessionId ?? runtime.activeSessionId),
          activeTurnId: status === "idle" ? null : runtime.activeTurnId,
          pendingRequests,
          lastUpdatedAt: now(),
        }
      }),
    }))
  },

  addEventForOwner: (owner, event) => {
    if (!owner.workspaceId) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, owner.workspaceId!, (runtime) => ({
        ...runtime,
        events: [...runtime.events, event],
        activeSessionId: owner.sessionId ?? runtime.activeSessionId,
        lastUpdatedAt: now(),
      })),
    }))
  },

  appendEventForOwner: (owner, event) => {
    const mapped = mapStreamEventToDisplay(event)
    if (mapped) get().addEventForOwner(owner, mapped)
  },

  updateEventLiveContentForOwner: (owner, id, delta) => {
    if (!owner.workspaceId) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, owner.workspaceId!, (runtime) => ({
        ...runtime,
        events: runtime.events.map((ev) =>
          ev.id === id
            ? { ...ev, liveContent: (ev.liveContent ?? "") + delta }
            : ev,
        ),
        lastUpdatedAt: now(),
      })),
    }))
  },

  clearWorkspaceEvents: (workspaceId) => {
    if (!workspaceId) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, workspaceId, (runtime) => ({
        ...runtime,
        events: [],
        lastUpdatedAt: now(),
      })),
    }))
  },

  addReviewForOwner: (owner, item, kind) => {
    const baseOwner = ownerOrNull(owner)
    if (!baseOwner) {
      set((state) => ({
        runtimeByWorkspace: state.runtimeByWorkspace,
      }))
      return
    }
    const requestOwner: RuntimeRequestOwner = {
      ...baseOwner,
      requestId: item.id,
      kind,
    }
    const itemWithOwner = { ...item, owner: requestOwner }
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, requestOwner.workspaceId, (runtime) => ({
        ...runtime,
        status: "pending_request",
        canRestartNow: runtime.restartRequired ? false : runtime.canRestartNow,
        activeSessionId: requestOwner.sessionId ?? runtime.activeSessionId,
        reviews: [
          ...runtime.reviews.filter((r) => r.id !== item.id),
          itemWithOwner,
        ],
        pendingRequests: {
          ...runtime.pendingRequests,
          [item.id]: requestOwner,
        },
        lastUpdatedAt: now(),
      })),
    }))
  },

  setReviewStatusForOwner: (owner, id, status) => {
    if (!owner.workspaceId) return
    set((state) => ({
      runtimeByWorkspace: withWorkspace(state, owner.workspaceId!, (runtime) => {
        const pendingRequests = { ...runtime.pendingRequests }
        if (status === "accepted" || status === "rejected") {
          delete pendingRequests[id]
        }
        const nextStatus =
          Object.keys(pendingRequests).length > 0
            ? "pending_request"
            : runtime.status === "pending_request"
              ? "idle"
              : runtime.status
        return {
          ...runtime,
          status: nextStatus,
          canRestartNow: runtime.restartRequired
            ? ["idle", "error"].includes(nextStatus)
            : runtime.canRestartNow,
          reviews: runtime.reviews.map((r) =>
            r.id === id ? { ...r, status } : r,
          ),
          pendingRequests,
          lastUpdatedAt: now(),
        }
      }),
    }))
  },

  acceptReview: async (id) => {
    const found = findReview(get().runtimeByWorkspace, id)
    if (!found) return
    const { owner, entry } = found
    get().setReviewStatusForOwner(owner, id, "applying")
    try {
      if (entry.runtime_permissions) {
        await ipc.respondRuntimePermissions(
          invokeOwner(owner),
          id,
          true,
          entry.runtime_permissions.permissions,
          "turn",
        )
      } else if (entry.mcp_elicitation) {
        await ipc.respondRuntimeMcpElicitation(invokeOwner(owner), id, "cancel")
        get().setReviewStatusForOwner(owner, id, "rejected")
        return
      } else {
        await ipc.respondRuntimeApproval(invokeOwner(owner), id, "accept")
      }
      get().setReviewStatusForOwner(owner, id, "accepted")
    } catch {
      get().setReviewStatusForOwner(owner, id, "error")
    }
  },

  acceptReviewForSession: async (id) => {
    const found = findReview(get().runtimeByWorkspace, id)
    if (!found) return
    const { owner, entry } = found
    get().setReviewStatusForOwner(owner, id, "applying")
    try {
      if (entry.runtime_permissions) {
        await ipc.respondRuntimePermissions(
          invokeOwner(owner),
          id,
          true,
          entry.runtime_permissions.permissions,
          "session",
        )
      } else if (entry.mcp_elicitation) {
        await ipc.respondRuntimeMcpElicitation(invokeOwner(owner), id, "accept")
      } else if (entry.runtime_approval) {
        await ipc.respondRuntimeApproval(invokeOwner(owner), id, "acceptForSession")
      } else {
        await ipc.respondRuntimeApproval(invokeOwner(owner), id, "accept")
      }
      get().setReviewStatusForOwner(owner, id, "accepted")
    } catch {
      get().setReviewStatusForOwner(owner, id, "error")
    }
  },

  rejectReview: async (id) => {
    const found = findReview(get().runtimeByWorkspace, id)
    if (!found) return
    const { owner, entry } = found
    get().setReviewStatusForOwner(owner, id, "applying")
    try {
      if (entry.user_input_request) {
        await ipc.respondRuntimeUserInput(invokeOwner(owner), id, {})
      } else if (entry.runtime_permissions) {
        await ipc.respondRuntimePermissions(invokeOwner(owner), id, false, {}, "turn")
      } else if (entry.mcp_elicitation) {
        await ipc.respondRuntimeMcpElicitation(invokeOwner(owner), id, "decline")
      } else {
        await ipc.respondRuntimeApproval(invokeOwner(owner), id, "decline")
      }
      get().setReviewStatusForOwner(owner, id, "rejected")
    } catch {
      get().setReviewStatusForOwner(owner, id, "error")
    }
  },

  answerUserInput: async (id, answers) => {
    const found = findReview(get().runtimeByWorkspace, id)
    if (!found?.entry.user_input_request) return
    const { owner } = found
    get().setReviewStatusForOwner(owner, id, "applying")
    try {
      await ipc.respondRuntimeUserInput(invokeOwner(owner), id, answers)
      get().setReviewStatusForOwner(owner, id, "accepted")
    } catch {
      get().setReviewStatusForOwner(owner, id, "error")
    }
  },

  answerMcpElicitation: async (id, content, meta) => {
    const found = findReview(get().runtimeByWorkspace, id)
    if (!found?.entry.mcp_elicitation) return
    const { owner } = found
    get().setReviewStatusForOwner(owner, id, "applying")
    try {
      await ipc.respondRuntimeMcpElicitation(
        invokeOwner(owner),
        id,
        "accept",
        content,
        meta,
      )
      get().setReviewStatusForOwner(owner, id, "accepted")
    } catch {
      get().setReviewStatusForOwner(owner, id, "error")
    }
  },
}))

export { requestKindForReview }
