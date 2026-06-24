import { useEffect, useRef } from "react"
import { listen } from "@tauri-apps/api/event"

import { useEmployeeStore } from "@/stores/employee"
import type { CodexEvent } from "@/types/events"

type DreamRuntimeEventPayload = {
  method: string
  params?: {
    phase?: string
    dream_run_id?: string
    target_employee_id?: string
    error?: {
      message?: string
    }
  }
}

type DreamEventPayload = CodexEvent | DreamRuntimeEventPayload

function isDreamRuntimeEventPayload(payload: DreamEventPayload): payload is DreamRuntimeEventPayload {
  if (typeof payload !== "object" || payload === null || !("method" in payload)) {
    return false
  }
  const method = (payload as { method?: unknown }).method
  return typeof method === "string" && method.startsWith("dream/")
}

export interface DreamEventCallbacks {
  onDelta?: (text: string) => void
  onDone?: (fullText: string) => void
  onError?: (message: string) => void
  onToolCall?: (name: string, args: string) => void
}

/**
 * Listen for Dream runtime events on the `"dream-event"` channel.
 * When `employeeId` is set, only forwards events while that employee is running Dream.
 */
export function useDreamEvents(callbacks: DreamEventCallbacks, employeeId?: string) {
  const cbRef = useRef(callbacks)
  cbRef.current = callbacks
  const employeeIdRef = useRef(employeeId)
  employeeIdRef.current = employeeId

  useEffect(() => {
    let accum = ""

    const unlisten = listen<DreamEventPayload>("dream-event", ({ payload }) => {
      const runningId = useEmployeeStore.getState().dreamRunningEmployeeId
      const scopedId = employeeIdRef.current
      if (scopedId && runningId !== scopedId) {
        return
      }

      if (isDreamRuntimeEventPayload(payload)) {
        const phase = payload.params?.phase
        const prefix = phase ? `[${phase}] ` : ""
        let line: string | null = null

        switch (payload.method) {
          case "dream/run_started":
            line = `${prefix}Dream run started\n`
            break
          case "dream/context_loaded":
            line = `${prefix}Context loaded\n`
            break
          case "dream/output_validated":
            line = `${prefix}Runtime output validated\n`
            break
          case "dream/phase_completed":
            line = `${prefix}Phase completed\n`
            break
          case "dream/failed": {
            const message = payload.params?.error?.message ?? "Dream run failed"
            if (scopedId && runningId === scopedId) {
              useEmployeeStore.setState({ dreamRunningEmployeeId: null })
            }
            cbRef.current.onError?.(message)
            return
          }
          default:
            break
        }

        if (line) {
          accum += line
          cbRef.current.onDelta?.(line)
        }
        return
      }

      const event = payload as CodexEvent

      switch (event.type) {
        case "assistant_delta":
          accum += event.content
          cbRef.current.onDelta?.(event.content)
          break

        case "assistant_done":
          accum = event.content
          cbRef.current.onDone?.(accum)
          break

        case "turn_complete":
          break

        case "error":
          if (scopedId && runningId === scopedId) {
            useEmployeeStore.setState({ dreamRunningEmployeeId: null })
          }
          cbRef.current.onError?.(event.message)
          break

        case "tool_call": {
          const argsText =
            typeof event.args === "string"
              ? event.args
              : JSON.stringify(event.args ?? {})
          cbRef.current.onToolCall?.(event.tool, argsText)
          break
        }

        case "cancelled":
          if (scopedId && runningId === scopedId) {
            useEmployeeStore.setState({ dreamRunningEmployeeId: null })
          }
          cbRef.current.onError?.("Dream run 已取消")
          break

        default:
          break
      }
    })

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])
}
