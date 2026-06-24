import { create } from "zustand"
import { invoke } from "@tauri-apps/api/core"

import type { MutationWithRuntimeInvalidation } from "@/lib/tauri"
import { useRuntimeStore } from "@/stores/runtime"
import type {
  McpToolPolicyView,
  McpToolPolicyInput,
  WorkspaceMcpServer,
  WorkspaceMcpServerTestResult,
  WorkspaceMcpServerView,
} from "@/types/mcp"

interface McpToolState {
  policy: McpToolPolicyView | null
  serverView: WorkspaceMcpServerView | null
  serverTestResults: Record<string, WorkspaceMcpServerTestResult>
  loading: boolean
  serversLoading: boolean
  testingServers: Record<string, boolean>
  error: string | null
  dirty: boolean

  loadToolPolicy: (workspaceId: string) => Promise<void>
  loadServers: (workspaceId: string) => Promise<void>
  importServersJson: (workspaceId: string, rawJson: string) => Promise<void>
  upsertServer: (workspaceId: string, server: WorkspaceMcpServer) => Promise<void>
  deleteServer: (workspaceId: string, name: string) => Promise<void>
  testServer: (workspaceId: string, name: string) => Promise<void>
  setToolEnabled: (toolId: string, enabled: boolean) => void
  enableAll: () => void
  disableAll: () => void
  resetToDefault: () => void
  savePolicy: (workspaceId: string) => Promise<void>
}

function formatError(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

async function runPostMutationRefresh(refresh: () => Promise<void>): Promise<void> {
  try {
    await refresh()
  } catch {
    // Primary mutation succeeded; refresh failure should not mask outcome.
  }
}

export const useMcpToolStore = create<McpToolState>((set, get) => ({
  policy: null,
  serverView: null,
  serverTestResults: {},
  loading: false,
  serversLoading: false,
  testingServers: {},
  error: null,
  dirty: false,

  loadToolPolicy: async (workspaceId) => {
    set({ loading: true, error: null })
    try {
      const result = await invoke<McpToolPolicyView>("list_mcp_tools", { workspaceId })
      set({ policy: result, loading: false, dirty: false })
    } catch (e) {
      set({ loading: false, error: formatError(e) })
    }
  },

  loadServers: async (workspaceId) => {
    set({ serversLoading: true, error: null })
    try {
      const result = await invoke<WorkspaceMcpServerView>("list_workspace_mcp_servers", {
        workspaceId,
      })
      set({ serverView: result, serversLoading: false })
    } catch (e) {
      set({ serversLoading: false, error: formatError(e) })
    }
  },

  importServersJson: async (workspaceId, rawJson) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<WorkspaceMcpServerView>>(
        "import_workspace_mcp_servers_json",
        { workspaceId, rawJson },
      )
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      const result = mutation.mutation.payload
      set({ serverView: result, error: null })
      await runPostMutationRefresh(() => get().loadServers(workspaceId))
    } catch (e) {
      set({ error: formatError(e) })
      throw e
    }
  },

  upsertServer: async (workspaceId, server) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<WorkspaceMcpServerView>>(
        "upsert_workspace_mcp_server",
        { workspaceId, server },
      )
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      const result = mutation.mutation.payload
      set({ serverView: result, error: null })
      await runPostMutationRefresh(() => get().loadServers(workspaceId))
    } catch (e) {
      set({ error: formatError(e) })
      throw e
    }
  },

  deleteServer: async (workspaceId, name) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<WorkspaceMcpServerView>>(
        "delete_workspace_mcp_server",
        { workspaceId, name },
      )
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      const result = mutation.mutation.payload
      set({ serverView: result, error: null })
      await runPostMutationRefresh(() => get().loadServers(workspaceId))
    } catch (e) {
      set({ error: formatError(e) })
      throw e
    }
  },

  testServer: async (workspaceId, name) => {
    set((state) => ({
      testingServers: { ...state.testingServers, [name]: true },
      error: null,
    }))
    try {
      const result = await invoke<WorkspaceMcpServerTestResult>(
        "test_workspace_mcp_server",
        { workspaceId, name },
      )
      set((state) => ({
        serverTestResults: { ...state.serverTestResults, [name]: result },
        testingServers: { ...state.testingServers, [name]: false },
        error: null,
      }))
      await get().loadServers(workspaceId)
    } catch (e) {
      const message = formatError(e)
      set((state) => ({
        serverTestResults: {
          ...state.serverTestResults,
          [name]: { ok: false, message, tools: [] },
        },
        testingServers: { ...state.testingServers, [name]: false },
        error: message,
      }))
    }
  },

  setToolEnabled: (toolId, enabled) => {
    const { policy } = get()
    if (!policy) return

    const updatedTools = policy.tools.map((t) =>
      t.id === toolId ? { ...t, enabled } : t,
    )
    set({ policy: { ...policy, tools: updatedTools }, dirty: true })
  },

  enableAll: () => {
    const { policy } = get()
    if (!policy) return

    const updatedTools = policy.tools.map((t) => ({ ...t, enabled: true }))
    set({
      policy: { ...policy, tools: updatedTools, default_enabled: true },
      dirty: true,
    })
  },

  disableAll: () => {
    const { policy } = get()
    if (!policy) return

    const updatedTools = policy.tools.map((t) => ({ ...t, enabled: false }))
    set({
      policy: { ...policy, tools: updatedTools, default_enabled: false },
      dirty: true,
    })
  },

  resetToDefault: () => {
    const { policy } = get()
    if (!policy) return

    const updatedTools = policy.tools.map((t) => ({ ...t, enabled: true }))
    set({
      policy: { ...policy, tools: updatedTools, default_enabled: true },
      dirty: true,
    })
  },

  savePolicy: async (workspaceId) => {
    const { policy } = get()
    if (!policy) return

    const input: McpToolPolicyInput = {
      default_enabled: policy.default_enabled,
      tools: Object.fromEntries(policy.tools.map((t) => [t.id, t.enabled])),
    }

    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<McpToolPolicyView>>("set_workspace_mcp_tool_policy", {
        workspaceId,
        policy: input,
      })
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      const result = mutation.mutation.payload
      set({ policy: result, dirty: false, error: null })
      await runPostMutationRefresh(() => get().loadToolPolicy(workspaceId))
    } catch (e) {
      set({ error: formatError(e) })
    }
  },
}))
