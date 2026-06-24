import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import { CHAWORK_DIALOG_CANCELLED } from "@/lib/ipcConstants"
import { pathKeysEqual } from "@/lib/formatPath"
import type { BindingValidation } from "@/types/employee"
import type { WorkspaceSidebarItem, WorkspaceState } from "@/types/workspace"

import { useDomainStore } from "@/stores/domain"
import { useEmployeeStore } from "@/stores/employee"
import { useSessionStore } from "@/stores/session"
import { useSkillStore } from "@/stores/skill"
import { useChatStore } from "@/stores/chat"
import { useRuntimeStore } from "@/stores/runtime"
import { useToastStore } from "@/stores/toast"

function workspaceMetaLine(ws: WorkspaceState): string {
  const getLabel = useDomainStore.getState().getLabel
  const prefix = getLabel("workspace.card.meta_prefix", "文件夹")
  const localTime = new Date(ws.last_active_at).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  })
  return `${prefix} · ${localTime}`
}

function sidebarFromWorkspaces(ws: WorkspaceState[]): WorkspaceSidebarItem[] {
  return ws.map((w) => ({
    workspace: w,
    metaLine: workspaceMetaLine(w),
  }))
}

function patchWorkspaceBindingInList(
  workspaces: WorkspaceState[],
  workspacePath: string,
  employeeName: string | null | undefined,
  employeeId?: string | null,
): WorkspaceState[] {
  return workspaces.map((w) =>
    pathKeysEqual(w.path, workspacePath)
      ? {
          ...w,
          bound_employee_name: employeeName ?? null,
          bound_employee_id: employeeId ?? w.bound_employee_id ?? null,
        }
      : w,
  )
}

function mergeWorkspaceIntoList(
  workspaces: WorkspaceState[],
  workspace: WorkspaceState,
): WorkspaceState[] {
  const withoutDuplicate = workspaces.filter(
    (w) => !pathKeysEqual(w.path, workspace.path),
  )
  return [...withoutDuplicate, workspace].sort((a, b) =>
    a.last_active_at < b.last_active_at ? 1 : -1,
  )
}

export type WorkspaceConfigTabKey =
  | "overview"
  | "domain_pack"
  | "index"
  | "provider"
  | "tools"

export interface OpenWorkspaceDialogOptions {
  /** false = register only, keep current workspace active. Default: switch when none open. */
  activate?: boolean
}

interface WorkspaceStore {
  workspaces: WorkspaceState[]
  activeWorkspaceId: string | null
  activeBinding: BindingValidation | null
  bindingLoading: boolean
  isLoading: boolean
  error: string | null
  workspaceSidebarItems: WorkspaceSidebarItem[]
  workspaceConfigOpen: boolean
  workspaceConfigTab: WorkspaceConfigTabKey
  setWorkspaceConfigOpen: (open: boolean) => void
  openWorkspaceConfig: (tab?: WorkspaceConfigTabKey) => void
  loadWorkspaces: () => Promise<void>
  refreshActiveBinding: (workspacePath?: string) => Promise<BindingValidation | null>
  /** Returns created workspace once backend finishes. */
  createWorkspace: (name: string, path: string) => Promise<void>
  switchWorkspace: (path: string) => Promise<void>
  openWorkspaceDialog: (
    bindToEmployeeId?: string,
    options?: OpenWorkspaceDialogOptions,
  ) => Promise<void>
  clearActiveWorkspace: () => void
  setActiveWorkspaceId: (id: string | null) => void
}

export const useWorkspaceStore = create<WorkspaceStore>((set, get) => ({
  workspaces: [],
  activeWorkspaceId: null,
  activeBinding: null,
  bindingLoading: false,
  isLoading: false,
  error: null,
  workspaceSidebarItems: [],
  workspaceConfigOpen: false,
  workspaceConfigTab: "overview",

  setWorkspaceConfigOpen: (workspaceConfigOpen) =>
    set({ workspaceConfigOpen }),

  openWorkspaceConfig: (tab = "overview") =>
    set({ workspaceConfigOpen: true, workspaceConfigTab: tab }),

  setActiveWorkspaceId: (activeWorkspaceId) => set({ activeWorkspaceId }),

  loadWorkspaces: async () => {
    set({ isLoading: true, error: null })
    try {
      let list = await ipc.listWorkspaces()
      list = [...list].sort((a, b) =>
        a.last_active_at < b.last_active_at ? 1 : -1,
      )
      set({
        workspaces: list,
        workspaceSidebarItems: sidebarFromWorkspaces(list),
      })
      const activeId = get().activeWorkspaceId
      const active = activeId ? list.find((w) => w.id === activeId) : undefined
      if (active?.path) {
        await get().refreshActiveBinding(active.path)
      } else {
        await useDomainStore.getState().loadDomainPack()
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ error: msg })
    } finally {
      set({ isLoading: false })
    }
  },

  refreshActiveBinding: async (workspacePath) => {
    set({ bindingLoading: true })
    try {
      const validation = await ipc.validateWorkspaceBinding(workspacePath)
      set({ activeBinding: validation })
      if (workspacePath) {
        const employeeName =
          validation.status === "bound" ? validation.employee_name : null
        const list = patchWorkspaceBindingInList(
          get().workspaces,
          workspacePath,
          employeeName,
          validation.employee_id,
        )
        set({
          workspaces: list,
          workspaceSidebarItems: sidebarFromWorkspaces(list),
        })
      }
      return validation
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ error: msg })
      return null
    } finally {
      set({ bindingLoading: false })
    }
  },

  createWorkspace: async (name, path) => {
    set({ isLoading: true, error: null })
    try {
      const ws = await ipc.createWorkspace(name, path)
      const list = mergeWorkspaceIntoList(get().workspaces, ws)
      set({
        workspaces: list,
        workspaceSidebarItems: sidebarFromWorkspaces(list),
      })
      await get().switchWorkspace(ws.path)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ error: msg })
    } finally {
      set({ isLoading: false })
    }
  },

  clearActiveWorkspace: () => {
    useChatStore.getState().loadHistory([])
    useSessionStore.setState({ sessions: [], activeSessionId: null })
    set({
      activeWorkspaceId: null,
      activeBinding: null,
    })
  },

  switchWorkspace: async (path: string) => {
    set({ isLoading: true, error: null })
    try {
      const result = await ipc.switchWorkspace(path)
      const { workspace, sessions } = result

      const list = mergeWorkspaceIntoList(get().workspaces, workspace)

      set({
        activeWorkspaceId: workspace.id,
        workspaces: list,
        workspaceSidebarItems: sidebarFromWorkspaces(list),
      })

      await useSessionStore.getState().hydrateAfterWorkspaceSwitch(workspace, sessions)
      await useDomainStore.getState().loadDomainPack()
      await get().refreshActiveBinding(workspace.path)
      const binding = get().activeBinding
      if (
        binding?.status === "bound" &&
        binding.employee_id &&
        useEmployeeStore.getState().selectedEmployeeId === binding.employee_id
      ) {
        await useEmployeeStore.getState().loadWorkspaces(binding.employee_id)
      }
      if (result.needs_skill_setup) {
        useSkillStore.getState().openSkillSetup()
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ error: msg })
    } finally {
      set({ isLoading: false })
    }
  },

  openWorkspaceDialog: async (bindToEmployeeId, options) => {
    set({ isLoading: true, error: null })
    const activate = options?.activate ?? !get().activeWorkspaceId
    try {
      const result = await ipc.openWorkspaceDialog(activate)
      const { workspace, sessions } = result

      const binding = await ipc.validateWorkspaceBinding(workspace.path)
      const boundEmployeeId =
        workspace.bound_employee_id ?? binding.employee_id ?? null
      const boundEmployeeName =
        workspace.bound_employee_name ?? binding.employee_name ?? null
      const enrichedWorkspace = {
        ...workspace,
        bound_employee_id: boundEmployeeId,
        bound_employee_name: boundEmployeeName,
      }
      const boundToOtherEmployee = Boolean(
        bindToEmployeeId &&
          boundEmployeeId &&
          boundEmployeeId !== bindToEmployeeId,
      )

      const list = mergeWorkspaceIntoList(get().workspaces, enrichedWorkspace)

      if (activate) {
        set({
          activeWorkspaceId: enrichedWorkspace.id,
          workspaces: list,
          workspaceSidebarItems: sidebarFromWorkspaces(list),
        })
      } else {
        set({
          workspaces: list,
          workspaceSidebarItems: sidebarFromWorkspaces(list),
        })
      }

      if (bindToEmployeeId) {
        if (boundToOtherEmployee) {
          const boundLabel = boundEmployeeName ?? boundEmployeeId
          useToastStore.getState().show(
            activate
              ? `已打开工作区「${enrichedWorkspace.name}」（绑定员工「${boundLabel}」）`
              : `已添加工作区「${enrichedWorkspace.name}」，该文件夹已绑定员工「${boundLabel}」。`,
            "info",
          )
        } else {
          try {
            const mutation = await ipc.bindWorkspaceToEmployee(
              bindToEmployeeId,
              workspace.path,
            )
            useRuntimeStore
              .getState()
              .handleRuntimeInvalidation(mutation.runtimeInvalidation)
            if (useEmployeeStore.getState().selectedEmployeeId === bindToEmployeeId) {
              await useEmployeeStore.getState().loadWorkspaces(bindToEmployeeId)
            }
          } catch (e) {
            const msg = e instanceof Error ? e.message : String(e)
            useToastStore.getState().show(msg, "error")
          }
        }
      }

      if (activate) {
        await useSessionStore.getState().hydrateAfterWorkspaceSwitch(
          enrichedWorkspace,
          sessions,
        )
        await useDomainStore.getState().loadDomainPack()
        await get().refreshActiveBinding(enrichedWorkspace.path)
        if (result.needs_skill_setup) {
          useSkillStore.getState().openSkillSetup()
        }
      } else if (!boundToOtherEmployee) {
        useToastStore.getState().show(
          `已添加工作区「${enrichedWorkspace.name}」，点击左侧工作区卡片可切换`,
          "info",
        )
      }

      await get().loadWorkspaces()
      if (bindToEmployeeId && !boundToOtherEmployee) {
        if (useEmployeeStore.getState().selectedEmployeeId === bindToEmployeeId) {
          await useEmployeeStore.getState().loadWorkspaces(bindToEmployeeId)
        }
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      if (msg !== CHAWORK_DIALOG_CANCELLED) set({ error: msg })
    } finally {
      set({ isLoading: false })
    }
  },
}))
