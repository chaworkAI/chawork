import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import type { QmdSearchResult, QmdStatus } from "@/types/knowledge"

interface KnowledgeState {
  /** Current search query */
  query: string
  /** Search results */
  results: QmdSearchResult[]
  /** Whether a search is in progress */
  isSearching: boolean
  /** QMD index status */
  status: QmdStatus | null
  /** Currently viewed document path */
  activeDocPath: string | null
  /** Currently viewed document content */
  activeDocContent: string | null
  /** Whether the knowledge panel is visible */
  isPanelOpen: boolean

  setQuery: (q: string) => void
  search: (q?: string) => Promise<void>
  loadStatus: () => Promise<void>
  openDocument: (filePath: string) => Promise<void>
  closeDocument: () => void
  togglePanel: () => void
  openPanel: () => void
  closePanel: () => void
  initialize: () => Promise<void>
  refresh: () => Promise<void>
  refreshIfStale: () => Promise<boolean>
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
  query: "",
  results: [],
  isSearching: false,
  status: null,
  activeDocPath: null,
  activeDocContent: null,
  isPanelOpen: false,

  setQuery: (q) => set({ query: q }),

  search: async (q) => {
    const query = q ?? get().query
    if (!query.trim()) {
      set({ results: [], isSearching: false })
      return
    }
    set({ isSearching: true, query })
    try {
      const results = await ipc.qmdSearch(query, 20)
      set({ results, isSearching: false })
    } catch {
      set({ results: [], isSearching: false })
    }
  },

  loadStatus: async () => {
    try {
      const status = await ipc.qmdStatus()
      set({ status })
    } catch {
      /* qmd may not be available */
    }
  },

  openDocument: async (filePath) => {
    set({ activeDocPath: filePath, activeDocContent: null })
    try {
      const content = await ipc.qmdGetDocument(filePath)
      set({ activeDocContent: content })
    } catch {
      set({ activeDocContent: "(无法读取文档)" })
    }
  },

  closeDocument: () => set({ activeDocPath: null, activeDocContent: null }),

  togglePanel: () => set((s) => ({ isPanelOpen: !s.isPanelOpen })),
  openPanel: () => set({ isPanelOpen: true }),
  closePanel: () =>
    set({
      isPanelOpen: false,
      activeDocPath: null,
      activeDocContent: null,
    }),

  initialize: async () => {
    try {
      await ipc.qmdInitialize()
      await get().loadStatus()
    } catch {
      /* best-effort */
    }
  },

  refresh: async () => {
    try {
      await ipc.qmdRefresh()
      await get().loadStatus()
    } catch {
      /* best-effort */
    }
  },

  refreshIfStale: async () => {
    try {
      const refreshed = await ipc.qmdRefreshIfStale()
      if (refreshed) {
        await get().loadStatus()
      }
      return refreshed
    } catch {
      return false
    }
  },
}))
