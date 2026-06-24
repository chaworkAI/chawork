import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import type { ImportTask, ImportTaskStatus } from "@/types/import"
import { isTerminalStatus } from "@/types/import"

interface ImportState {
  /** Whether any task currently in-flight for the active workspace. */
  isImporting: boolean
  /** Snapshot of recent tasks (most recent first). */
  tasks: ImportTask[]
  /** Panel visibility. */
  isPanelOpen: boolean
  /** Last error from a synchronous submit (NOT a task error). */
  submitError: string | null

  openPanel: () => void
  closePanel: () => void

  /** Submit a file path. Returns the task id so callers can wait for it. */
  importFile: (sourcePath: string) => Promise<string | null>

  /** Pull the latest task list. */
  loadTasks: () => Promise<void>
}

export const useImportStore = create<ImportState>((set, get) => ({
  isImporting: false,
  tasks: [],
  isPanelOpen: false,
  submitError: null,

  openPanel: () => set({ isPanelOpen: true }),
  closePanel: () => set({ isPanelOpen: false, submitError: null }),

  importFile: async (sourcePath) => {
    set({ submitError: null })
    try {
      const taskId = await ipc.importFile(sourcePath)
      // Optimistic: refresh list so the new task shows up immediately.
      void get().loadTasks()
      return taskId
    } catch (e) {
      set({ submitError: e instanceof Error ? e.message : String(e) })
      return null
    }
  },

  loadTasks: async () => {
    try {
      const tasks = await ipc.listImportTasks(50)
      const anyInFlight = tasks.some(
        (t: ImportTask) => !isTerminalStatus(t.status as ImportTaskStatus),
      )
      set({ tasks, isImporting: anyInFlight })
    } catch {
      // best-effort polling — surface nothing
    }
  },
}))
