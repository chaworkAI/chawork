import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import { DEFAULT_LOCALE, type AppLocale } from "@/types/locale"

const STORAGE_KEY = "chawork-locale"

interface LocaleState {
  locale: AppLocale
  loadGlobalLocale: () => Promise<void>
  setLocale: (locale: AppLocale) => void
}

function readStoredLocale(): AppLocale {
  if (typeof localStorage === "undefined") return DEFAULT_LOCALE
  const saved = localStorage.getItem(STORAGE_KEY)
  return saved === "en-US" || saved === "zh-CN" ? saved : DEFAULT_LOCALE
}

function applyDocumentLang(locale: AppLocale) {
  if (typeof document === "undefined") return
  document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : "en"
}

export const useLocaleStore = create<LocaleState>((set) => ({
  locale: DEFAULT_LOCALE,
  loadGlobalLocale: async () => {
    try {
      const payload = await ipc.getUiLocale()
      const locale = normalizeLocale(payload.locale)
      localStorage.setItem(STORAGE_KEY, locale)
      applyDocumentLang(locale)
      set({ locale })
    } catch {
      // Keep the localStorage fallback when the backend is not ready.
    }
  },
  setLocale: (locale) => {
    const next = normalizeLocale(locale)
    localStorage.setItem(STORAGE_KEY, next)
    applyDocumentLang(next)
    set({ locale: next })
    void ipc.setUiLocale(next).catch(() => {
      // localStorage remains the fallback if global config write fails.
    })
  },
}))

function normalizeLocale(locale: string): AppLocale {
  return locale === "en-US" ? "en-US" : "zh-CN"
}

/** Call once before React render so the first paint uses the saved locale. */
export function initLocaleStore() {
  const locale = readStoredLocale()
  applyDocumentLang(locale)
  useLocaleStore.setState({ locale })
  void useLocaleStore.getState().loadGlobalLocale()
}
