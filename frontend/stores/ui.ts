import { create } from "zustand"

export type AppTheme = "light" | "dark"

interface UiStore {
  theme: AppTheme
  rightRailDrawerOpen: boolean
  managementDrawerOpen: boolean
  dreamSchedulePanelOpen: boolean
  projectMaterialsOpen: boolean
  sidebarCollapsed: boolean
  setTheme: (theme: AppTheme) => void
  toggleTheme: () => void
  openRightRail: () => void
  setRightRailDrawerOpen: (open: boolean) => void
  setManagementDrawerOpen: (open: boolean) => void
  setDreamSchedulePanelOpen: (open: boolean) => void
  keepManagementDrawerOpen: () => void
  scheduleManagementDrawerClose: () => void
  setProjectMaterialsOpen: (open: boolean) => void
  setSidebarCollapsed: (collapsed: boolean) => void
}

let managementDrawerCloseTimer: ReturnType<typeof setTimeout> | null = null
const THEME_STORAGE_KEY = "chawork.theme"

function readStoredTheme(): AppTheme {
  if (typeof window === "undefined") return "light"
  return window.localStorage.getItem(THEME_STORAGE_KEY) === "dark" ? "dark" : "light"
}

function applyTheme(theme: AppTheme) {
  if (typeof document === "undefined") return
  document.documentElement.classList.toggle("dark", theme === "dark")
  document.documentElement.style.colorScheme = theme
}

const initialTheme = readStoredTheme()
applyTheme(initialTheme)

export const useUiStore = create<UiStore>((set, get) => ({
  theme: initialTheme,
  rightRailDrawerOpen: false,
  managementDrawerOpen: false,
  dreamSchedulePanelOpen: false,
  projectMaterialsOpen: false,
  sidebarCollapsed: false,

  setTheme: (theme) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme)
    }
    applyTheme(theme)
    set({ theme })
  },

  toggleTheme: () => {
    const nextTheme: AppTheme = get().theme === "dark" ? "light" : "dark"
    get().setTheme(nextTheme)
  },

  openRightRail: () => set({ rightRailDrawerOpen: true }),

  setRightRailDrawerOpen: (open) => set({ rightRailDrawerOpen: open }),

  setManagementDrawerOpen: (open) => {
    if (managementDrawerCloseTimer) {
      clearTimeout(managementDrawerCloseTimer)
      managementDrawerCloseTimer = null
    }
    set({ managementDrawerOpen: open })
  },

  setDreamSchedulePanelOpen: (open) => set({ dreamSchedulePanelOpen: open }),

  keepManagementDrawerOpen: () => {
    if (managementDrawerCloseTimer) {
      clearTimeout(managementDrawerCloseTimer)
      managementDrawerCloseTimer = null
    }
    set({ managementDrawerOpen: true })
  },

  scheduleManagementDrawerClose: () => {
    if (managementDrawerCloseTimer) clearTimeout(managementDrawerCloseTimer)
    managementDrawerCloseTimer = setTimeout(() => {
      set({ managementDrawerOpen: false })
      managementDrawerCloseTimer = null
    }, 140)
  },

  setProjectMaterialsOpen: (open) => set({ projectMaterialsOpen: open }),

  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
}))
