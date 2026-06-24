import { create } from "zustand"

import { syncGithubEmployeeRemoved } from "@/lib/hubEmployeeSync"
import {
  loadGithubImportEmployeeViews,
  mergeHubEmployeeViews,
  readUserCustomGithubEmployeeIds,
} from "@/lib/hubCustomEmployees"
import {
  readCustomSkillPreviews,
  readUserCustomHubSkillIds,
  writeCustomSkillPreviews,
  writeUserCustomHubSkillIds,
} from "@/lib/hubCustomSkills"
import * as ipc from "@/lib/tauri"
import { useEmployeeStore } from "@/stores/employee"
import { useRuntimeStore } from "@/stores/runtime"
import { useSkillStore } from "@/stores/skill"
import { useToastStore } from "@/stores/toast"
import { useWorkspaceStore } from "@/stores/workspace"
import type {
  HubDownloadFilter,
  HubEmployeeView,
  HubGithubImportSkillPreview,
  HubManifest,
  HubSkillView,
  ProfessionInfo,
} from "@/types/hub"

export type HubTab = "skills" | "employees"
export type HubInstallFeedbackStatus = "success" | "error"

export interface HubInstallFeedback {
  status: HubInstallFeedbackStatus
  message: string
  nonce: number
}

const HUB_LIST_LIMIT = 50

const FEEDBACK_TTL_MS = 3600
const feedbackTimers = new Map<string, ReturnType<typeof setTimeout>>()

function feedbackKey(kind: HubTab, id: string) {
  return `${kind}:${id}`
}

function skillsListFilter(filter: HubDownloadFilter): HubDownloadFilter {
  return filter
}

interface HubStore {
  marketOpen: boolean
  githubImportOpen: boolean
  activeTab: HubTab
  skillsFilter: HubDownloadFilter
  employeesFilter: HubDownloadFilter
  query: string
  profession: string | null
  customImportSkillIds: string[]
  userCustomHubSkillIds: string[]
  userCustomGithubEmployeeIds: string[]
  githubImportPreviews: HubGithubImportSkillPreview[]

  manifest: HubManifest | null
  professions: ProfessionInfo[]
  skills: HubSkillView[]
  skillsPage: number
  skillsTotal: number
  skillsLoadingMore: boolean
  employees: HubEmployeeView[]

  installingIds: string[]
  installFeedback: Record<string, HubInstallFeedback>
  loading: boolean
  error: string | null

  openMarket: (tab?: HubTab) => void
  closeMarket: () => void
  openGithubImport: () => void
  closeGithubImport: () => void
  setActiveTab: (tab: HubTab) => void
  setQuery: (query: string) => void
  setSkillsFilter: (filter: HubDownloadFilter) => void
  setEmployeesFilter: (filter: HubDownloadFilter) => void
  setProfession: (profession: string | null) => void

  loadManifest: () => Promise<void>
  loadProfessions: () => Promise<void>
  loadSkills: () => Promise<void>
  loadMoreSkills: () => Promise<void>
  loadEmployees: () => Promise<void>
  reloadActive: () => Promise<void>
  clearInstallFeedback: (key: string) => void
  installSkill: (hubSkillId: string) => Promise<void>
  deleteSkill: (hubSkillId: string, options?: { installed?: boolean }) => Promise<void>
  installEmployee: (hubEmployeeId: string) => Promise<void>
  deleteEmployee: (
    employeeId: string,
    options?: { installed?: boolean },
  ) => Promise<void>
}

function formatError(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

function addBusy(id: string, list: string[]) {
  return list.includes(id) ? list : [...list, id]
}

function removeBusy(id: string, list: string[]) {
  return list.filter((item) => item !== id)
}

function compactErrorMessage(error: string) {
  const trimmed = error.trim()
  if (trimmed.length <= 180) return trimmed
  return `${trimmed.slice(0, 180)}...`
}

async function refreshLocalStores() {
  await Promise.allSettled([
    useSkillStore.getState().loadSkills(),
    useEmployeeStore.getState().loadEmployees(),
  ])
}

export const useHubStore = create<HubStore>((set, get) => ({
  marketOpen: false,
  githubImportOpen: false,
  activeTab: "skills",
  skillsFilter: "all",
  employeesFilter: "all",
  query: "",
  profession: null,
  customImportSkillIds: [],
  userCustomHubSkillIds: readUserCustomHubSkillIds(),
  userCustomGithubEmployeeIds: readUserCustomGithubEmployeeIds(),
  githubImportPreviews: [],

  manifest: null,
  professions: [],
  skills: [],
  skillsPage: 0,
  skillsTotal: 0,
  skillsLoadingMore: false,
  employees: [],

  installingIds: [],
  installFeedback: {},
  loading: false,
  error: null,

  openMarket: (tab = "skills") => {
    set({ marketOpen: true, activeTab: tab, error: null })
    void Promise.allSettled([get().loadManifest(), get().loadProfessions(), get().reloadActive()])
  },
  closeMarket: () => set({ marketOpen: false }),
  openGithubImport: () =>
    set({ githubImportOpen: true, marketOpen: false, error: null }),
  closeGithubImport: () => set({ githubImportOpen: false }),
  setActiveTab: (activeTab) => {
    set({ activeTab })
    const state = get()
    if (activeTab === "employees" && state.employees.length === 0) void state.loadEmployees()
    if (activeTab === "skills" && state.skills.length === 0) void state.loadSkills()
  },
  setQuery: (query) => set({ query }),
  setSkillsFilter: (skillsFilter) => {
    set({
      skillsFilter,
      ...(skillsFilter !== "custom" ? { githubImportPreviews: [] } : {}),
    })
    if (get().activeTab === "skills") void get().loadSkills()
  },
  setEmployeesFilter: (employeesFilter) => {
    set({ employeesFilter })
    if (get().activeTab === "employees") void get().loadEmployees()
  },
  setProfession: (profession) => {
    set({ profession })
    if (get().activeTab === "skills") void get().loadSkills()
  },

  loadManifest: async () => {
    try {
      const manifest = await ipc.hubGetManifest()
      set({ manifest })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },
  loadProfessions: async () => {
    try {
      const professions = await ipc.hubListProfessions()
      set({ professions })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },
  loadSkills: async () => {
    set({ loading: true, error: null, skillsPage: 0, skillsTotal: 0 })
    try {
      const result = await ipc.hubListSkills({
        profession: get().profession ?? undefined,
        filter: get().skillsFilter,
        page: 1,
        limit: HUB_LIST_LIMIT,
      })
      set({
        skills: result.items,
        skillsPage: result.page,
        skillsTotal: result.total,
      })
    } catch (e) {
      set({ error: formatError(e) })
    } finally {
      set({ loading: false })
    }
  },
  loadMoreSkills: async () => {
    const state = get()
    if (state.loading || state.skillsLoadingMore) return
    if (state.skills.length >= state.skillsTotal) return

    set({ skillsLoadingMore: true, error: null })
    try {
      const filter = skillsListFilter(state.skillsFilter)
      const result = await ipc.hubListSkills({
        profession: state.profession ?? undefined,
        filter,
        page: state.skillsPage + 1,
        limit: HUB_LIST_LIMIT,
      })
      set((current) => ({
        skills: [...current.skills, ...result.items],
        skillsPage: result.page,
        skillsTotal: result.total,
      }))
    } catch (e) {
      set({ error: formatError(e) })
    } finally {
      set({ skillsLoadingMore: false })
    }
  },
  loadEmployees: async () => {
    set({ loading: true, error: null })
    try {
      const employeesFilter = get().employeesFilter
      const githubEmployeeIds = get().userCustomGithubEmployeeIds
      const needsLocalGithubEmployees =
        githubEmployeeIds.length > 0 &&
        (employeesFilter === "all" ||
          employeesFilter === "local" ||
          employeesFilter === "custom")
      const [result, localRegistry] = await Promise.all([
        ipc.hubListEmployees({
          filter: employeesFilter === "custom" ? "all" : employeesFilter,
          page: 1,
          limit: HUB_LIST_LIMIT,
        }),
        needsLocalGithubEmployees ? ipc.listEmployees() : Promise.resolve([]),
      ])
      const localGithubEmployees = needsLocalGithubEmployees
        ? await loadGithubImportEmployeeViews(githubEmployeeIds, localRegistry)
        : []
      const employees = mergeHubEmployeeViews(result.items, localGithubEmployees)
      set({ employees })
    } catch (e) {
      set({ error: formatError(e) })
    } finally {
      set({ loading: false })
    }
  },
  reloadActive: async () => {
    if (get().activeTab === "employees") await get().loadEmployees()
    else await get().loadSkills()
  },
  clearInstallFeedback: (key) => {
    const timer = feedbackTimers.get(key)
    if (timer) {
      clearTimeout(timer)
      feedbackTimers.delete(key)
    }
    set((state) => {
      const next = { ...state.installFeedback }
      delete next[key]
      return { installFeedback: next }
    })
  },
  installSkill: async (hubSkillId) => {
    const key = feedbackKey("skills", hubSkillId)
    get().clearInstallFeedback(key)
    set((state) => ({ installingIds: addBusy(hubSkillId, state.installingIds), error: null }))
    try {
      await ipc.hubInstallSkill(hubSkillId)
      set((state) => ({
        installFeedback: {
          ...state.installFeedback,
          [key]: {
            status: "success",
            message: "下载完成，已写入 Root 技能库。",
            nonce: Date.now(),
          },
        },
      }))
      feedbackTimers.set(
        key,
        setTimeout(() => get().clearInstallFeedback(key), FEEDBACK_TTL_MS),
      )
      const activeBinding = useWorkspaceStore.getState().activeBinding
      useToastStore.getState().show(
        activeBinding?.status === "bound"
          ? "Skill 已下载到 Root，可通过 Skill 管理添加给当前员工。"
          : "Skill 已下载到 Root。",
        "success",
      )
      await refreshLocalStores()
      await get().loadSkills()
    } catch (e) {
      const error = formatError(e)
      set((state) => ({
        installFeedback: {
          ...state.installFeedback,
          [key]: {
            status: "error",
            message: compactErrorMessage(error),
            nonce: Date.now(),
          },
        },
      }))
      useToastStore.getState().show(error, "error")
    } finally {
      set((state) => ({ installingIds: removeBusy(hubSkillId, state.installingIds) }))
    }
  },
  deleteSkill: async (hubSkillId, options) => {
    const installed = options?.installed ?? true
    const key = feedbackKey("skills", hubSkillId)
    get().clearInstallFeedback(key)
    set((state) => ({ installingIds: addBusy(hubSkillId, state.installingIds), error: null }))
    try {
      if (installed) {
        await ipc.hubUninstallSkill(hubSkillId)
      }
      const userCustomHubSkillIds = get().userCustomHubSkillIds.filter(
        (id) => id !== hubSkillId,
      )
      writeUserCustomHubSkillIds(userCustomHubSkillIds)
      const customSkillPreviews = readCustomSkillPreviews().filter(
        (preview) => preview.id !== hubSkillId,
      )
      writeCustomSkillPreviews(customSkillPreviews)
      set((state) => ({
        userCustomHubSkillIds,
        skills: state.skills.filter((skill) => skill.id !== hubSkillId),
        githubImportPreviews: state.githubImportPreviews.filter(
          (skill) => skill.id !== hubSkillId,
        ),
        customImportSkillIds: state.customImportSkillIds.filter((id) => id !== hubSkillId),
      }))
      useToastStore.getState().show(
        installed ? "技能已删除。" : "已从自定义列表移除。",
        "success",
      )
      await refreshLocalStores()
      await get().loadSkills()
    } catch (e) {
      const error = formatError(e)
      set((state) => ({
        installFeedback: {
          ...state.installFeedback,
          [key]: {
            status: "error",
            message: compactErrorMessage(error),
            nonce: Date.now(),
          },
        },
      }))
      useToastStore.getState().show(error, "error")
    } finally {
      set((state) => ({ installingIds: removeBusy(hubSkillId, state.installingIds) }))
    }
  },
  installEmployee: async (hubEmployeeId) => {
    const key = feedbackKey("employees", hubEmployeeId)
    get().clearInstallFeedback(key)
    set((state) => ({ installingIds: addBusy(hubEmployeeId, state.installingIds), error: null }))
    try {
      await ipc.hubInstallEmployee(hubEmployeeId)
      set((state) => ({
        installFeedback: {
          ...state.installFeedback,
          [key]: {
            status: "success",
            message: "下载完成，已写入本地员工仓库。",
            nonce: Date.now(),
          },
        },
      }))
      feedbackTimers.set(
        key,
        setTimeout(() => get().clearInstallFeedback(key), FEEDBACK_TTL_MS),
      )
      useToastStore.getState().show("员工已下载。", "success")
      await refreshLocalStores()
      await get().loadEmployees()
    } catch (e) {
      const error = formatError(e)
      set((state) => ({
        installFeedback: {
          ...state.installFeedback,
          [key]: {
            status: "error",
            message: compactErrorMessage(error),
            nonce: Date.now(),
          },
        },
      }))
      useToastStore.getState().show(error, "error")
    } finally {
      set((state) => ({ installingIds: removeBusy(hubEmployeeId, state.installingIds) }))
    }
  },
  deleteEmployee: async (employeeId, options) => {
    const installed = options?.installed ?? true
    const key = feedbackKey("employees", employeeId)
    get().clearInstallFeedback(key)
    set((state) => ({ installingIds: addBusy(employeeId, state.installingIds), error: null }))
    try {
      if (installed) {
        const result = await ipc.deleteEmployee(employeeId)
        useRuntimeStore.getState().handleRuntimeInvalidation(result.runtimeInvalidation)
        await useWorkspaceStore.getState().refreshActiveBinding()
      }
      await syncGithubEmployeeRemoved(employeeId)
      if (useEmployeeStore.getState().selectedEmployeeId === employeeId) {
        useEmployeeStore.setState({
          selectedEmployeeId: null,
          selectedDetail: null,
          panelMode: "list",
        })
      }
      useToastStore.getState().show(
        installed ? "员工已删除。" : "已从自定义列表移除。",
        "success",
      )
      await refreshLocalStores()
      await get().loadEmployees()
    } catch (e) {
      const error = formatError(e)
      set((state) => ({
        installFeedback: {
          ...state.installFeedback,
          [key]: {
            status: "error",
            message: compactErrorMessage(error),
            nonce: Date.now(),
          },
        },
      }))
      useToastStore.getState().show(error, "error")
    } finally {
      set((state) => ({ installingIds: removeBusy(employeeId, state.installingIds) }))
    }
  },
}))
