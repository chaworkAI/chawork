import { useEffect } from "react"
import { listen } from "@tauri-apps/api/event"

import type { RuntimeInvalidationResult } from "@/lib/tauri"
import { useRuntimeStore } from "@/stores/runtime"

export function useRuntimeLifecycleEvents() {
  useEffect(() => {
    const unlisten = listen<RuntimeInvalidationResult>(
      "runtime-lifecycle/invalidated",
      ({ payload }) => {
        useRuntimeStore.getState().handleRuntimeInvalidation(payload)
      },
    )
    return () => {
      void unlisten.then((fn) => fn())
    }
  }, [])
}
