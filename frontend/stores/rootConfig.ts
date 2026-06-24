import { create } from "zustand"

import * as ipc from "@/lib/tauri"

/** Maps legacy tab ids to GlobalSettingsPanel tab keys. */
function mapGlobalSettingsTab(tab: string): string {
  if (tab === "general" || tab === "locale") return "general"
  if (tab === "provider" || tab === "global_provider") return "provider"
  if (tab === "root_runtime" || tab === "runtime") return "root_runtime"
  if (tab === "global_skills") return "global_skills"
  if (tab === "templates" || tab === "global_templates") return "templates"
  return "provider"
}

interface RootWorkspaceInfo {
  path: string
  initialized: boolean
  codex_home: string
  provider_path: string
  skills_dir: string
  templates_dir: string
  mcp_dir: string
}

interface RootConfigState {
  rootInfo: RootWorkspaceInfo | null
  httpServerPort: number | null
  loading: boolean
  error: string | null

  settingsPanelOpen: boolean
  settingsActiveTab: string

  loadRootInfo: () => Promise<void>
  loadHttpServerPort: () => Promise<void>
  openSettingsPanel: (tab?: string) => void
  closeSettingsPanel: () => void
  setSettingsTab: (tab: string) => void
}

function formatError(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

export const useRootConfigStore = create<RootConfigState>((set) => ({
  rootInfo: null,
  httpServerPort: null,
  loading: false,
  error: null,

  settingsPanelOpen: false,
  settingsActiveTab: "provider",

  loadRootInfo: async () => {
    set({ loading: true, error: null })
    try {
      const info = await ipc.getRootWorkspaceInfo()
      set({
        rootInfo: {
          path: info.path,
          initialized: info.path.length > 0,
          codex_home: info.codex_home,
          provider_path: info.provider_path,
          skills_dir: info.skills_dir,
          templates_dir: info.templates_dir,
          mcp_dir: info.mcp_dir,
        },
        loading: false,
      })
    } catch (e) {
      set({ loading: false, error: formatError(e) })
    }
  },

  loadHttpServerPort: async () => {
    try {
      const port = await ipc.getHttpServerPort()
      set({ httpServerPort: port })
    } catch (e) {
      set({ error: formatError(e) })
    }
  },

  openSettingsPanel: (tab) => {
    set({
      settingsPanelOpen: true,
      settingsActiveTab: tab ? mapGlobalSettingsTab(tab) : "provider",
    })
  },

  closeSettingsPanel: () => {
    set({ settingsPanelOpen: false })
  },

  setSettingsTab: (tab) => {
    set({ settingsActiveTab: tab })
  },
}))
