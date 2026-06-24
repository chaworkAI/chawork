import { create } from "zustand"

import { pickDreamConfigTargetEmployee, filterUserVisibleEmployees, DREAM_WORKFLOW_EMPLOYEE_ID } from "@/lib/employeeDream"
import { syncGithubEmployeeRemoved } from "@/lib/hubEmployeeSync"
import * as ipc from "@/lib/tauri"
import { useRuntimeStore } from "@/stores/runtime"
import { useToastStore } from "@/stores/toast"
import { useWorkspaceStore } from "@/stores/workspace"
import type {
  ApplyResult,
  CreateEmployeeInput,
  DreamConfig,
  DreamDefaults,
  EmployeeDetail,
  EmployeeSkillSummary,
  PendingUpdateRequest,
  RecentDreamResult,
  RegistryEntry,
  UpdateEmployeeInput,
  WorkspaceMembership,
} from "@/types/employee"

export type EmployeeTab =
  | "overview"
  | "prompt"
  | "skills"
  | "dream"

interface EmployeeStore {
  employees: RegistryEntry[]
  selectedEmployeeId: string | null
  selectedDetail: EmployeeDetail | null
  selectedSkills: EmployeeSkillSummary[]
  selectedWorkspaces: WorkspaceMembership[]
  promptContent: string | null

  dreamConfig: DreamConfig | null
  dreamDefaults: DreamDefaults | null
  recentDreamResult: RecentDreamResult | null
  pendingRequest: PendingUpdateRequest | null
  /** Employee id whose dreamConfig / recentDreamResult / pendingRequest are loaded. */
  dreamStateEmployeeId: string | null
  /** Employee id currently running Dream Phase 1 (global backend is single-flight). */
  dreamRunningEmployeeId: string | null
  /** Employee id currently applying an approved prompt update. */
  applyingEmployeeId: string | null
  applyResult: ApplyResult | null

  panelOpen: boolean
  panelMode: "list" | "detail"
  activeTab: EmployeeTab
  createDialogOpen: boolean
  /** When set, a newly created employee is auto-bound to this workspace path. */
  bindWorkspacePathOnCreate: string | null

  /** Employee ids with a pending Dream prompt update request (for badge UI). */
  pendingReviewEmployeeIds: string[]

  isLoading: boolean
  /** Which employee detail is being loaded (for per-row / per-tab loading UI). */
  detailLoadingEmployeeId: string | null
  error: string | null

  openPanel: (mode?: "list" | "detail") => void
  openEmployeeDetail: (employeeId: string) => Promise<void>
  openEmployeeSkills: (employeeId: string) => Promise<void>
  /** Open dream schedule config for an ordinary employee (not Dream Workflow). */
  openDreamConfigPanel: () => Promise<void>
  closePanel: () => void
  setPanelMode: (mode: "list" | "detail") => void
  setActiveTab: (tab: EmployeeTab) => void
  openCreateDialog: () => void
  openCreateDialogForWorkspace: (workspacePath: string) => void
  closeCreateDialog: () => void

  loadEmployees: () => Promise<void>
  selectEmployee: (id: string) => Promise<void>
  createEmployee: (input: CreateEmployeeInput) => Promise<EmployeeDetail>
  updateMetadata: (id: string, input: UpdateEmployeeInput) => Promise<void>
  deleteEmployee: (id: string) => Promise<void>

  loadSkills: (employeeId: string) => Promise<void>
  copySkill: (employeeId: string, skillId: string) => Promise<void>
  toggleSkill: (employeeId: string, skillId: string, enabled: boolean) => Promise<void>
  deleteSkill: (employeeId: string, skillId: string) => Promise<void>

  loadPrompt: (employeeId: string) => Promise<void>
  updatePrompt: (employeeId: string, content: string) => Promise<void>
  loadWorkspaces: (employeeId: string) => Promise<void>
  bindWorkspace: (employeeId: string, workspacePath?: string) => Promise<void>
  unbindWorkspace: (workspacePath?: string) => Promise<void>

  initEmployees: () => Promise<void>
  refreshPendingReviewBadges: () => Promise<void>

  loadDreamConfig: (employeeId: string) => Promise<void>
  updateDreamConfig: (employeeId: string, config: DreamConfig) => Promise<void>
  loadDreamDefaults: () => Promise<void>
  updateDreamDefaults: (defaults: DreamDefaults) => Promise<void>
  loadDreamState: (employeeId: string) => Promise<void>
  runDream: (employeeId: string) => Promise<void>
  approveRequest: (employeeId: string) => Promise<void>
  rejectRequest: (employeeId: string) => Promise<void>
  clearApplyResult: () => void
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

export const useEmployeeStore = create<EmployeeStore>((set, get) => ({
  employees: [],
  selectedEmployeeId: null,
  selectedDetail: null,
  selectedSkills: [],
  selectedWorkspaces: [],
  promptContent: null,

  dreamConfig: null,
  dreamDefaults: null,
  recentDreamResult: null,
  pendingRequest: null,
  dreamStateEmployeeId: null,
  dreamRunningEmployeeId: null,
  applyingEmployeeId: null,
  applyResult: null,

  panelOpen: false,
  panelMode: "list",
  activeTab: "overview",
  createDialogOpen: false,
  bindWorkspacePathOnCreate: null,

  pendingReviewEmployeeIds: [],

  isLoading: false,
  detailLoadingEmployeeId: null,
  error: null,

  openPanel: (mode = "list") => {
    set({ panelOpen: true, panelMode: mode })
    void get().loadEmployees()
    void get().loadDreamDefaults()
    void get().refreshPendingReviewBadges()
  },
  openEmployeeDetail: async (employeeId) => {
    set({ panelOpen: true, panelMode: "detail", activeTab: "overview", error: null })
    await get().loadEmployees()
    const exists = get().employees.some((entry) => entry.id === employeeId)
    if (!exists) {
      useToastStore.getState().show(
        `员工「${employeeId}」未在列表中找到，请稍后刷新员工列表`,
        "error",
      )
      set({ panelMode: "list", selectedEmployeeId: null, selectedDetail: null })
      return
    }
    await get().selectEmployee(employeeId)
  },
  openEmployeeSkills: async (employeeId) => {
    set({ panelOpen: true, panelMode: "detail", activeTab: "skills", error: null })
    await get().loadEmployees()
    await get().selectEmployee(employeeId)
    if (get().selectedEmployeeId === employeeId) {
      set({ activeTab: "skills" })
    }
  },
  openDreamConfigPanel: async () => {
    set({ panelOpen: true, panelMode: "detail", activeTab: "dream", error: null })
    await get().loadEmployees()
    void get().loadDreamDefaults()
    void get().refreshPendingReviewBadges()

    const targetId = pickDreamConfigTargetEmployee(
      get().employees,
      (() => {
        const binding = useWorkspaceStore.getState().activeBinding
        return binding?.status === "bound" ? binding.employee_id : null
      })(),
    )
    if (!targetId) {
      set({ panelMode: "list", activeTab: "overview" })
      useToastStore.getState().show(
        "请先创建普通员工再配置定时做梦。Dream Workflow 为系统内置执行器，无需单独设置。",
        "info",
      )
      return
    }

    await get().selectEmployee(targetId)
  },
  closePanel: () => set({ panelOpen: false }),
  setPanelMode: (panelMode) => set({ panelMode }),
  setActiveTab: (activeTab) => {
    set({ activeTab })
    if (activeTab === "prompt") {
      const employeeId = get().selectedEmployeeId
      if (employeeId) {
        void get().loadPrompt(employeeId)
      }
    }
  },
  openCreateDialog: () =>
    set({ createDialogOpen: true, bindWorkspacePathOnCreate: null }),
  openCreateDialogForWorkspace: (workspacePath) =>
    set({ createDialogOpen: true, bindWorkspacePathOnCreate: workspacePath }),
  closeCreateDialog: () =>
    set({ createDialogOpen: false, bindWorkspacePathOnCreate: null }),

  loadEmployees: async () => {
    set({ isLoading: true, error: null })
    try {
      const employees = filterUserVisibleEmployees(await ipc.listEmployees())
      set({ employees })
      const selectedId = get().selectedEmployeeId
      if (
        selectedId === DREAM_WORKFLOW_EMPLOYEE_ID ||
        (selectedId != null && !employees.some((entry) => entry.id === selectedId))
      ) {
        set({ selectedEmployeeId: null, selectedDetail: null })
      }
      await get().refreshPendingReviewBadges()
    } catch (e) {
      set({ error: formatError(e) })
    } finally {
      set({ isLoading: false })
    }
  },

  selectEmployee: async (id) => {
    if (id === DREAM_WORKFLOW_EMPLOYEE_ID) return
    const previousId = get().selectedEmployeeId
    set({
      selectedEmployeeId: id,
      isLoading: true,
      detailLoadingEmployeeId: id,
      error: null,
      applyResult: null,
      ...(previousId !== id
        ? {
            promptContent: null,
            dreamConfig: null,
            recentDreamResult: null,
            pendingRequest: null,
            dreamStateEmployeeId: null,
          }
        : {}),
    })
    try {
      const detail = await ipc.getEmployeeDetail(id)
      if (get().selectedEmployeeId !== id) return
      set({ selectedDetail: detail })
      await Promise.all([
        get().loadSkills(id),
        get().loadWorkspaces(id),
        get().loadPrompt(id),
        get().loadDreamState(id),
      ])
    } catch (e) {
      if (get().selectedEmployeeId === id) {
        set({ error: formatError(e) })
      }
    } finally {
      if (get().selectedEmployeeId === id) {
        set({ isLoading: false, detailLoadingEmployeeId: null })
      }
    }
  },

  createEmployee: async (input) => {
    set({ isLoading: true, error: null })
    const bindPath = get().bindWorkspacePathOnCreate
    try {
      const detail = await ipc.createEmployee(input)
      await get().loadEmployees()
      if (bindPath) {
        const binding = await ipc.bindWorkspaceToEmployee(
          detail.registry_entry.id,
          bindPath,
        )
        useRuntimeStore
          .getState()
          .handleRuntimeInvalidation(binding.runtimeInvalidation)
        await useWorkspaceStore.getState().refreshActiveBinding(bindPath)
        await useWorkspaceStore.getState().loadWorkspaces()
      }
      set({
        selectedEmployeeId: detail.registry_entry.id,
        selectedDetail: detail,
        createDialogOpen: false,
        bindWorkspacePathOnCreate: null,
      })
      await get().loadWorkspaces(detail.registry_entry.id)
      return detail
    } catch (e) {
      set({ error: formatError(e) })
      throw e
    } finally {
      set({ isLoading: false })
    }
  },

  updateMetadata: async (id, input) => {
    set({ error: null })
    try {
      const detail = await ipc.updateEmployeeMetadata(id, input)
      set({ selectedDetail: detail })
      await get().loadEmployees()
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  deleteEmployee: async (id) => {
    set({ isLoading: true, error: null })
    try {
      const result = await ipc.deleteEmployee(id)
      useRuntimeStore.getState().handleRuntimeInvalidation(result.runtimeInvalidation)
      await syncGithubEmployeeRemoved(id)
      if (get().selectedEmployeeId === id) {
        set({
          selectedEmployeeId: null,
          selectedDetail: null,
          panelMode: "list",
        })
      }
      await get().loadEmployees()
      await useWorkspaceStore.getState().refreshActiveBinding()
      useToastStore.getState().show("员工已删除。", "success")
    } catch (e) {
      set({ error: formatError(e) })
      useToastStore.getState().show(formatError(e), "error")
      throw e
    } finally {
      set({ isLoading: false })
    }
  },

  loadSkills: async (employeeId) => {
    try {
      const selectedSkills = await ipc.listEmployeeSkills(employeeId)
      set({ selectedSkills })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  copySkill: async (employeeId, skillId) => {
    set({ error: null })
    try {
      const result = await ipc.copyRootSkillToEmployee(employeeId, skillId)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(employeeId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  toggleSkill: async (employeeId, skillId, enabled) => {
    set({ error: null })
    try {
      const result = await ipc.toggleEmployeeSkill(employeeId, skillId, enabled)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(employeeId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  deleteSkill: async (employeeId, skillId) => {
    set({ error: null })
    try {
      const result = await ipc.deleteEmployeeSkill(employeeId, skillId)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        await get().loadSkills(employeeId)
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  loadPrompt: async (employeeId) => {
    try {
      const promptContent = await ipc.readEmployeePrompt(employeeId)
      if (get().selectedEmployeeId === employeeId) {
        set({ promptContent })
      }
    } catch (e) {
      if (get().selectedEmployeeId === employeeId) {
        set({ error: formatError(e) })
      }
    }
  },

  updatePrompt: async (employeeId, content) => {
    set({ error: null })
    try {
      const result = await ipc.writeEmployeePrompt(employeeId, content)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      if (get().selectedEmployeeId === employeeId) {
        set({ promptContent: content })
      }
    } catch (e) {
      set({ error: formatError(e) })
      throw e
    }
  },

  loadWorkspaces: async (employeeId) => {
    try {
      const selectedWorkspaces = await ipc.listWorkspacesForEmployee(employeeId)
      set({ selectedWorkspaces })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  initEmployees: async () => {
    await get().loadEmployees()
  },

  refreshPendingReviewBadges: async () => {
    try {
      const pendingReviewEmployeeIds =
        await ipc.listEmployeesWithPendingDreamRequests()
      set({ pendingReviewEmployeeIds })
    } catch {
      // Non-blocking badge refresh.
    }
  },

  bindWorkspace: async (employeeId, workspacePath) => {
    set({ error: null })
    try {
      const result = await ipc.bindWorkspaceToEmployee(employeeId, workspacePath)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        if (get().selectedEmployeeId === employeeId) {
          await get().loadWorkspaces(employeeId)
        }
        await useWorkspaceStore.getState().refreshActiveBinding(workspacePath)
        await useWorkspaceStore.getState().loadWorkspaces()
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  unbindWorkspace: async (workspacePath) => {
    set({ error: null })
    try {
      const result = await ipc.unbindWorkspaceFromEmployee(workspacePath)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      await runPostMutationRefresh(async () => {
        const eid = get().selectedEmployeeId
        if (eid) await get().loadWorkspaces(eid)
        await useWorkspaceStore.getState().refreshActiveBinding(workspacePath)
        await useWorkspaceStore.getState().loadWorkspaces()
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  loadDreamConfig: async (employeeId) => {
    try {
      const dreamConfig = await ipc.getDreamConfig(employeeId)
      if (get().selectedEmployeeId !== employeeId) return
      set({ dreamConfig })
    } catch {
      if (get().selectedEmployeeId !== employeeId) return
      set({ dreamConfig: null })
    }
  },

  updateDreamConfig: async (employeeId, config) => {
    const previousConfig = get().dreamConfig
    const previousOwner = get().dreamStateEmployeeId
    set({
      error: null,
      dreamConfig: config,
      dreamStateEmployeeId: employeeId,
    })
    try {
      await ipc.setDreamConfig(employeeId, config)
    } catch (e) {
      if (get().selectedEmployeeId === employeeId) {
        set({
          dreamConfig: previousConfig,
          dreamStateEmployeeId: previousOwner,
          error: formatError(e),
        })
      }
    }
  },

  loadDreamDefaults: async () => {
    try {
      const dreamDefaults = await ipc.getDreamDefaults()
      set({ dreamDefaults })
    } catch {
      set({ dreamDefaults: null })
    }
  },

  updateDreamDefaults: async (defaults) => {
    set({ error: null })
    try {
      await ipc.setDreamDefaults(defaults)
      set({ dreamDefaults: defaults })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  loadDreamState: async (employeeId) => {
    try {
      const [recentDreamResult, pendingRequest] = await Promise.all([
        ipc.getRecentDreamResult(employeeId),
        ipc.getPendingRequest(employeeId),
        get().loadDreamConfig(employeeId),
      ])
      if (get().selectedEmployeeId !== employeeId) return
      set({
        recentDreamResult,
        pendingRequest,
        dreamStateEmployeeId: employeeId,
      })
      await get().refreshPendingReviewBadges()
    } catch (e) {
      if (get().selectedEmployeeId === employeeId) {
        set({ error: formatError(e) })
      }
    }
  },

  runDream: async (employeeId) => {
    set({ dreamRunningEmployeeId: employeeId, error: null })
    try {
      await ipc.runDreamPhase1(employeeId)
      if (get().dreamRunningEmployeeId !== employeeId) return
      set({ dreamRunningEmployeeId: null })
      try {
        await get().loadDreamState(employeeId)
      } catch {
        // Phase 1 already succeeded; state refresh failure should not mask outcome.
      }
    } catch (e) {
      if (get().dreamRunningEmployeeId === employeeId) {
        set({ error: formatError(e), dreamRunningEmployeeId: null })
      }
    }
  },

  approveRequest: async (employeeId) => {
    set({ applyingEmployeeId: employeeId, error: null, applyResult: null })
    try {
      const mutation = await ipc.approveDreamRequest(employeeId)
      if (get().applyingEmployeeId !== employeeId) return
      const result = mutation.mutation.payload
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(mutation.runtimeInvalidation)
      set({ applyResult: result, error: null })
      try {
        await Promise.all([
          get().loadDreamState(employeeId),
          get().loadPrompt(employeeId),
          get().refreshPendingReviewBadges(),
        ])
      } catch {
        // Approve already succeeded; refresh failures should not mask the outcome.
      }
    } catch (e) {
      if (get().applyingEmployeeId === employeeId) {
        set({ error: formatError(e) })
      }
      await runPostMutationRefresh(async () => {
        await Promise.all([
          get().loadDreamState(employeeId),
          get().refreshPendingReviewBadges(),
        ])
      })
    } finally {
      if (get().applyingEmployeeId === employeeId) {
        set({ applyingEmployeeId: null })
      }
    }
  },

  rejectRequest: async (employeeId) => {
    set({ error: null })
    try {
      await ipc.rejectDreamRequest(employeeId)
      await runPostMutationRefresh(async () => {
        await get().loadDreamState(employeeId)
        await get().refreshPendingReviewBadges()
      })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  clearApplyResult: () => set({ applyResult: null }),
}))
