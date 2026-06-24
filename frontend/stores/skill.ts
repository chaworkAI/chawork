import { create } from "zustand"
import { invoke } from "@tauri-apps/api/core"

import { useEmployeeStore } from "@/stores/employee"
import { useRuntimeStore } from "@/stores/runtime"
import { useToastStore } from "@/stores/toast"
import { useWorkspaceStore } from "@/stores/workspace"
import type { MutationWithRuntimeInvalidation } from "@/lib/tauri"

import type {
  SkillSummary,
  SkillListView,
  SkillSelectionView,
  SkillPromotionResult,
} from "@/types/skill"

interface SkillState {
  rootCatalog: SkillSummary[]
  workspaceSelection: SkillSummary[]
  workspaceLocal: SkillSummary[]
  loading: boolean
  error: string | null

  skillManagerOpen: boolean
  skillSetupOpen: boolean

  openSkillManager: () => void
  closeSkillManager: () => void
  openSkillSetup: () => void
  closeSkillSetup: () => void
  loadSkills: (workspaceId?: string) => Promise<void>
  enableRootSkill: (workspaceId: string, rootSkillId: string) => Promise<void>
  disableRootSkill: (workspaceId: string, rootSkillId: string) => Promise<void>
  createWorkspaceOverride: (workspaceId: string, rootSkillId: string) => Promise<void>
  deleteWorkspaceSkill: (workspaceId: string, skillId: string) => Promise<void>
  promoteToGlobal: (
    proposalId: string,
    workspaceId: string,
    skillId: string,
  ) => Promise<SkillPromotionResult>
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

export const useSkillStore = create<SkillState>((set, get) => ({
  rootCatalog: [],
  workspaceSelection: [],
  workspaceLocal: [],
  loading: false,
  error: null,

  skillManagerOpen: false,
  skillSetupOpen: false,

  openSkillManager: () => {
    const binding = useWorkspaceStore.getState().activeBinding
    if (binding?.status === "bound" && binding.employee_id) {
      void useEmployeeStore.getState().openEmployeeSkills(binding.employee_id)
      useToastStore.getState().show(
        "此工作区已绑定员工，Skills 请在该员工中配置。",
        "info",
      )
      return
    }
    set({ skillManagerOpen: true })
  },
  closeSkillManager: () => set({ skillManagerOpen: false }),
  openSkillSetup: () => {
    const binding = useWorkspaceStore.getState().activeBinding
    if (binding?.status === "bound") {
      return
    }
    set({ skillSetupOpen: true })
  },
  closeSkillSetup: () => set({ skillSetupOpen: false }),

  loadSkills: async (workspaceId) => {
    set({ loading: true, error: null })
    try {
      const result = await invoke<SkillListView>("list_skills", { workspaceId })
      set({
        rootCatalog: result.root_catalog,
        workspaceSelection: result.workspace_selection,
        workspaceLocal: result.workspace_local,
        loading: false,
      })
    } catch (e) {
      set({ loading: false, error: formatError(e) })
    }
  },

  enableRootSkill: async (workspaceId, rootSkillId) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<SkillSelectionView>>("set_workspace_skill_selection", {
        workspaceId,
        rootSkillId,
        enabled: true,
      })
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(workspaceId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  disableRootSkill: async (workspaceId, rootSkillId) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<SkillSelectionView>>("set_workspace_skill_selection", {
        workspaceId,
        rootSkillId,
        enabled: false,
      })
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(workspaceId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  createWorkspaceOverride: async (workspaceId, rootSkillId) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<SkillSummary>>("create_workspace_skill_override", {
        workspaceId,
        rootSkillId,
      })
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(workspaceId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  deleteWorkspaceSkill: async (workspaceId, skillId) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<void>>(
        "delete_workspace_skill",
        { workspaceId, skillId },
      )
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(workspaceId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  promoteToGlobal: async (proposalId, workspaceId, skillId) => {
    try {
      const mutation = await invoke<MutationWithRuntimeInvalidation<SkillPromotionResult>>("promote_skill_to_global", {
        proposalId,
        workspaceId,
        skillId,
      })
      useRuntimeStore.getState().handleRuntimeInvalidation(mutation.runtimeInvalidation)
      const result = mutation.mutation.payload
      await runPostMutationRefresh(async () => {
        await get().loadSkills(workspaceId)
      })
      return result
    } catch (e) {
      set({ error: formatError(e) })
      throw e
    }
  },
}))
