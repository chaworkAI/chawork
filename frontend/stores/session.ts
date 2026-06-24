import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import type { SessionMeta } from "@/types/session"
import type { WorkspaceState } from "@/types/workspace"

import { useChatStore } from "@/stores/chat"
import { useRuntimeStore } from "@/stores/runtime"
import { useWorkspaceStore } from "@/stores/workspace"

interface SessionStore {
  sessions: SessionMeta[]
  activeSessionId: string | null
  isLoading: boolean
  hydrateAfterWorkspaceSwitch: (
    workspace: WorkspaceState,
    sessions: SessionMeta[],
  ) => Promise<void>
  createSession: () => Promise<void>
  switchSession: (id: string) => Promise<void>
  renameSession: (id: string, title: string) => Promise<void>
  deleteSession: (id: string) => Promise<void>
  loadSessions: () => Promise<void>
}

export const useSessionStore = create<SessionStore>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  isLoading: false,

  hydrateAfterWorkspaceSwitch: async (workspace, sessions) => {
    set({ sessions, isLoading: true })
    try {
      if (sessions.length === 0) {
        await get().createSession()
        return
      }

      let preferred =
        workspace.active_session_id &&
        sessions.some((s) => s.id === workspace.active_session_id)
          ? workspace.active_session_id
          : (sessions[0]?.id ?? null)

      if (!preferred) {
        await get().createSession()
        return
      }

      // 勿先设 activeSessionId，否则 switchSession 会因「同会话」提前 return 而不加载 transcript
      await get().switchSession(preferred)
    } finally {
      set({ isLoading: false })
    }
  },

  loadSessions: async () => {
    set({ isLoading: true })
    try {
      const sessions = await ipc.listSessions()
      set({ sessions })
    } finally {
      set({ isLoading: false })
    }
  },

  createSession: async () => {
    const meta = await ipc.createSession()
    const list = await ipc.listSessions()
    const activeWorkspaceId = useWorkspaceStore.getState().activeWorkspaceId
    set({
      sessions: list,
      activeSessionId: meta.id,
    })
    useRuntimeStore.getState().clearWorkspaceEvents(activeWorkspaceId)
    useRuntimeStore.getState().clearLifecycleNotice(activeWorkspaceId)
    useChatStore.getState().loadHistory([])
  },

  renameSession: async (id: string, title: string) => {
    const trimmed = title.trim()
    if (!trimmed) return
    try {
      await ipc.renameSession(id, trimmed)
      await get().loadSessions()
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      useRuntimeStore.getState().addEventForOwner(
        {
          workspaceId: useWorkspaceStore.getState().activeWorkspaceId,
          sessionId: get().activeSessionId,
        },
        {
        id: crypto.randomUUID(),
        timestamp: new Date().toISOString(),
        event: { type: "error", message: msg, recoverable: true },
        displayLabel: "重命名会话失败",
        displayStatus: "error",
        detail: msg,
        },
      )
    }
  },

  deleteSession: async (id: string) => {
    const activeWorkspaceId = useWorkspaceStore.getState().activeWorkspaceId
    const runtime = useRuntimeStore.getState().getWorkspaceRuntime(activeWorkspaceId)
    if (
      runtime?.activeSessionId === id &&
      useRuntimeStore.getState().isWorkspaceBusy(activeWorkspaceId)
    ) {
      useRuntimeStore.getState().addEventForOwner(
        { workspaceId: activeWorkspaceId, sessionId: id },
        {
          id: crypto.randomUUID(),
          timestamp: new Date().toISOString(),
          event: {
            type: "error",
            message: "当前会话正在运行，请先停止或等待完成后再删除",
            recoverable: true,
          },
          displayLabel: "删除会话被拒绝",
          displayStatus: "error",
          detail: "当前会话正在运行，请先停止或等待完成后再删除",
        },
      )
      return
    }
    set({ isLoading: true })
    try {
      const result = await ipc.deleteSession(id)
      set({
        sessions: result.sessions,
        activeSessionId: result.active_session_id,
      })
      useChatStore.getState().loadHistory(result.transcript)
      useRuntimeStore.getState().clearWorkspaceEvents(activeWorkspaceId)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      useRuntimeStore.getState().addEventForOwner(
        { workspaceId: activeWorkspaceId, sessionId: get().activeSessionId },
        {
        id: crypto.randomUUID(),
        timestamp: new Date().toISOString(),
        event: { type: "error", message: msg, recoverable: true },
        displayLabel: "删除会话失败",
        displayStatus: "error",
        detail: msg,
        },
      )
    } finally {
      set({ isLoading: false })
    }
  },

  switchSession: async (id: string) => {
    if (id === get().activeSessionId && !useChatStore.getState().isStreaming) {
      return
    }
    set({ activeSessionId: id, isLoading: true })
    try {
      const result = await ipc.switchSession(id)
      useChatStore.getState().loadHistory(result.transcript)
      await get().loadSessions()
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      useRuntimeStore.getState().addEventForOwner(
        {
          workspaceId: useWorkspaceStore.getState().activeWorkspaceId,
          sessionId: id,
        },
        {
        id: crypto.randomUUID(),
        timestamp: new Date().toISOString(),
        event: { type: "error", message: msg, recoverable: true },
        displayLabel: "切换会话失败",
        displayStatus: "error",
        detail: msg,
        },
      )
    } finally {
      set({ isLoading: false })
    }
  },
}))
