import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import { useRuntimeStore } from "@/stores/runtime"
import type { ToolPolicyPayload } from "@/lib/tauri"

interface WorkspaceConfigState {
  policy: ToolPolicyPayload
  isLoading: boolean
  error: string | null

  load: () => Promise<void>
  setPolicy: (policy: ToolPolicyPayload) => Promise<void>
}

const defaultPolicy: ToolPolicyPayload = {
  default_action: "enabled",
  overrides: {},
}

export const useWorkspaceConfigStore = create<WorkspaceConfigState>((set) => ({
  policy: defaultPolicy,
  isLoading: false,
  error: null,

  load: async () => {
    set({ isLoading: true, error: null })
    try {
      const policy = await ipc.getToolPolicy()
      set({ policy, isLoading: false })
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      })
    }
  },

  setPolicy: async (policy) => {
    const result = await ipc.setToolPolicy(policy)
    useRuntimeStore
      .getState()
      .handleRuntimeInvalidation(result.runtimeInvalidation)
    set({ policy })
  },
}))
