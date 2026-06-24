import { create } from "zustand"
import type { DomainPack } from "@/types/domain"
import { resolveLabel } from "@/lib/resolveLabel"
import { useLocaleStore } from "@/stores/locale"
import * as ipc from "@/lib/tauri"

interface DomainStore {
  pack: DomainPack | null
  isLoading: boolean
  loadDomainPack: () => Promise<void>
  /** Get a UI label from the domain pack, with fallback */
  getLabel: (path: string, fallback: string) => string
}

export const useDomainStore = create<DomainStore>((set, get) => ({
  pack: null,
  isLoading: false,

  loadDomainPack: async () => {
    set({ isLoading: true })
    try {
      const pack = await ipc.getDomainPack()
      set({ pack })
    } finally {
      set({ isLoading: false })
    }
  },

  getLabel: (path, fallback) => {
    const locale = useLocaleStore.getState().locale
    return resolveLabel(
      path,
      fallback,
      locale,
      get().pack?.labels as Record<string, unknown> | undefined,
    )
  },
}))
