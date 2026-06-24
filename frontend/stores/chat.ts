import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import { useRuntimeStore } from "@/stores/runtime"
import { useSessionStore } from "@/stores/session"
import { useWorkspaceStore } from "@/stores/workspace"
import type { Attachment, Message, TranscriptEntry } from "@/types/message"

function asTranscriptRow(value: unknown): TranscriptEntry | null {
  if (!value || typeof value !== "object") return null
  const row = value as Record<string, unknown>
  if (row.role !== "user" && row.role !== "assistant") return null
  if (typeof row.content !== "string" || typeof row.timestamp !== "string") {
    return null
  }
  return row as TranscriptEntry
}

export function transcriptRowsToMessages(rows: unknown[]): Message[] {
  const out: Message[] = []
  let i = 0
  for (const raw of rows) {
    const entry = asTranscriptRow(raw)
    if (!entry) continue

    const id = `msg-${entry.timestamp}-${i}`
    i += 1

    if (entry.role === "user") {
      out.push({
        id,
        role: "user",
        content: entry.content,
        timestamp: entry.timestamp,
        attachments: entry.attachments,
      })
    } else {
      if (!entry.content.trim()) continue
      out.push({
        id,
        role: "assistant",
        content: entry.content,
        timestamp: entry.timestamp,
        sources: entry.sources,
      })
    }
  }

  return out
}

interface ChatStore {
  messages: Message[]
  isStreaming: boolean
  sendMessage: (content: string, attachments?: Attachment[]) => Promise<void>
  addAssistantDelta: (content: string) => void
  addThinkingDelta: (delta: string) => void
  appendThinkingAnimated: (chunk: string) => Promise<void>
  finishThinking: () => void
  toggleThinkingExpanded: (messageId: string) => void
  appendAssistantDelta: (content: string) => void
  completeAssistantMessage: (content: string) => void
  revealAssistantAnimated: (content: string) => Promise<void>
  setStreaming: (value: boolean) => void
  startStreaming: () => void
  finishStreaming: () => void
  loadHistory: (entries: unknown[]) => void
  /** Prefill Composer on next read (e.g. after import → summarize). */
  pendingComposerPrefill: string | null
  setPendingComposerPrefill: (text: string | null) => void
  takePendingComposerPrefill: () => string | null
  syncTranscriptFromBackend: () => Promise<void>
  ensureAssistantPlaceholder: () => void
  cancelStream: () => void
  cancelActiveTurn: () => Promise<void>
  finalizeTurn: () => void
  finalizeEmptyAssistantTurn: () => void
}

export const useChatStore = create<ChatStore>((set, get) => ({
  messages: [],
  isStreaming: false,

  setStreaming: (value: boolean) => set({ isStreaming: value }),

  startStreaming: () => set({ isStreaming: true }),

  finishStreaming: () => get().finalizeTurn(),

  cancelStream: () =>
    set((state) => {
      const msgs = [...state.messages]
      const idx = msgs.length - 1
      const last = msgs[idx]
      if (last?.role === "assistant" && last.isStreaming) {
        if (!last.content.trim() && !last.thinkingContent?.trim()) {
          msgs.pop()
          return { messages: msgs, isStreaming: false }
        }
        msgs[idx] = { ...last, isStreaming: false, isThinkingStreaming: false }
        return { messages: msgs, isStreaming: false }
      }
      return { isStreaming: false }
    }),

  cancelActiveTurn: async () => {
    const workspaceId = useWorkspaceStore.getState().activeWorkspaceId
    const sessionId = useSessionStore.getState().activeSessionId
    if (!workspaceId) return
    get().cancelStream()
    useRuntimeStore.getState().setWorkspaceStatus({ workspaceId, sessionId }, "cancelling")
    try {
      await ipc.cancelCurrentTurn(workspaceId)
    } catch {
      /* idle or no runtime */
    } finally {
      useRuntimeStore.getState().setWorkspaceStatus({ workspaceId, sessionId }, "idle")
      get().finalizeTurn()
    }
  },

  finalizeTurn: () =>
    set((state) => {
      const msgs = [...state.messages]
      const idx = msgs.length - 1
      const last = msgs[idx]
      if (last?.role === "assistant" && last.isStreaming) {
        msgs[idx] = { ...last, isStreaming: false, isThinkingStreaming: false }
        return { messages: msgs, isStreaming: false }
      }
      return { isStreaming: false }
    }),

  finalizeEmptyAssistantTurn: () =>
    set((state) => {
      const msgs = [...state.messages]
      const idx = msgs.length - 1
      const last = msgs[idx]
      if (last?.role === "assistant" && last.isStreaming) {
        msgs[idx] = {
          ...last,
          isStreaming: false,
          isThinkingStreaming: false,
          emptyReply: true,
        }
        return { messages: msgs, isStreaming: false }
      }
      return { isStreaming: false }
    }),

  appendAssistantDelta: (delta) => get().addAssistantDelta(delta),

  addThinkingDelta: (delta) =>
    set((state) => {
      if (!delta) return {}
      const msgs = [...state.messages]
      const last = msgs.length > 0 ? msgs[msgs.length - 1] : undefined
      const ts = new Date().toISOString()

      const patchThinking = (msg: Message): Message => ({
        ...msg,
        thinkingContent: (msg.thinkingContent ?? "") + delta,
        isThinkingStreaming: true,
        isThinkingExpanded: msg.isThinkingExpanded ?? true,
      })

      if (last?.role === "assistant" && last.isStreaming) {
        const idx = msgs.length - 1
        msgs[idx] = patchThinking(last)
        return { messages: msgs, isStreaming: true }
      }

      msgs.push({
        id: `msg-stream-${crypto.randomUUID()}`,
        role: "assistant",
        content: "",
        timestamp: ts,
        isStreaming: true,
        thinkingContent: delta,
        isThinkingStreaming: true,
        isThinkingExpanded: true,
      })
      return { messages: msgs, isStreaming: true }
    }),

  finishThinking: () =>
    set((state) => {
      const msgs = [...state.messages]
      const idx = msgs.length - 1
      const last = msgs[idx]
      if (last?.role !== "assistant" || !last.thinkingContent?.trim()) {
        return {}
      }
      msgs[idx] = { ...last, isThinkingStreaming: false }
      return { messages: msgs }
    }),

  appendThinkingAnimated: async (chunk) => {
    const text = chunk.trim()
    if (!text) return
    const pieceSize = 28
    const delayMs = 14
    for (let i = 0; i < text.length; i += pieceSize) {
      get().addThinkingDelta(text.slice(i, i + pieceSize))
      await new Promise((resolve) => setTimeout(resolve, delayMs))
    }
  },

  toggleThinkingExpanded: (messageId) =>
    set((state) => ({
      messages: state.messages.map((m) => {
        if (m.id !== messageId) return m
        const isExpanded =
          m.isThinkingExpanded ?? Boolean(m.isThinkingStreaming)
        return { ...m, isThinkingExpanded: !isExpanded }
      }),
    })),

  addAssistantDelta: (delta) =>
    set((state) => {
      if (!delta) return {}
      const msgs = [...state.messages]
      const last = msgs.length > 0 ? msgs[msgs.length - 1] : undefined
      const ts = new Date().toISOString()

      const collapseThinking = (msg: Message): Message => {
        const hasThinking = Boolean(msg.thinkingContent?.trim())
        const isFirstAnswerChunk = !msg.content.trim()
        return {
          ...msg,
          content: msg.content + delta,
          ...(hasThinking && isFirstAnswerChunk
            ? { isThinkingStreaming: false, isThinkingExpanded: false }
            : {}),
        }
      }

      if (last?.role === "assistant" && last.isStreaming) {
        const idx = msgs.length - 1
        msgs[idx] = collapseThinking(last)
        return { messages: msgs }
      }

      msgs.push({
        id: `msg-stream-${crypto.randomUUID()}`,
        role: "assistant",
        content: delta,
        timestamp: ts,
        isStreaming: true,
      })
      return { messages: msgs, isStreaming: true }
    }),

  /**
   * When Codex returns the full reply in one chunk (typical for DashScope HTTP/SSE),
   * complete immediately instead of character-by-character animation.
   * react-markdown re-parses the full AST on every delta, and slicing at arbitrary
   * byte offsets breaks markdown tokens (e.g. `**bo` + `ld**`), causing visual flashes.
   */
  revealAssistantAnimated: async (fullContent: string) => {
    const text = fullContent.trim()
    if (!text) {
      get().finalizeTurn()
      return
    }
    get().completeAssistantMessage(text)
  },

  completeAssistantMessage: (content) =>
    set((state) => {
      const msgs = [...state.messages]
      const last = msgs.length > 0 ? msgs[msgs.length - 1] : undefined
      if (last?.role === "assistant" && last.isStreaming) {
        const idx = msgs.length - 1
        msgs[idx] = {
          ...last,
          content,
          isStreaming: false,
          isThinkingStreaming: false,
        }
        return { messages: msgs }
      }
      const ts = new Date().toISOString()
      msgs.push({
        id: `msg-asst-${crypto.randomUUID()}`,
        role: "assistant",
        content,
        timestamp: ts,
      })
      return { messages: msgs }
    }),

  loadHistory: (entries) => {
    set({
      messages: transcriptRowsToMessages(entries),
      isStreaming: false,
    })
  },

  pendingComposerPrefill: null,

  setPendingComposerPrefill: (text) => set({ pendingComposerPrefill: text }),

  takePendingComposerPrefill: () => {
    const text = get().pendingComposerPrefill
    set({ pendingComposerPrefill: null })
    return text
  },

  syncTranscriptFromBackend: async () => {
    try {
      const rows = await ipc.getActiveSessionTranscript()
      set({
        messages: transcriptRowsToMessages(rows),
        isStreaming: false,
      })
    } catch {
      get().finalizeTurn()
    }
  },

  ensureAssistantPlaceholder: () => {
    set((state) => {
      const last = state.messages[state.messages.length - 1]
      if (last?.role === "assistant" && last.isStreaming) {
        return { isStreaming: true }
      }
      const ts = new Date().toISOString()
      return {
        messages: [
          ...state.messages,
          {
            id: `msg-pending-${crypto.randomUUID()}`,
            role: "assistant",
            content: "",
            timestamp: ts,
            isStreaming: true,
          },
        ],
        isStreaming: true,
      }
    })
  },

  sendMessage: async (content: string, attachments: Attachment[] = []) => {
    const text = content.trim()
    if (!text && attachments.length === 0) return

    const sessionId = useSessionStore.getState().activeSessionId
    if (!sessionId) return

    const ts = new Date().toISOString()
    const entry: Message = {
      id: `msg-${ts}-${crypto.randomUUID()}`,
      role: "user",
      content: text,
      timestamp: ts,
      attachments: attachments.length > 0 ? attachments : undefined,
    }
    set((state) => ({
      messages: [...state.messages, entry],
      isStreaming: true,
    }))

    const workspaceId = useWorkspaceStore.getState().activeWorkspaceId
    if (!workspaceId) {
      get().finalizeTurn()
      return
    }

    useRuntimeStore.getState().clearWorkspaceEvents(workspaceId)
    useRuntimeStore.getState().markWorkspaceThinking(workspaceId, sessionId)

    try {
      // Returns immediately; Codex runs in background and streams `codex-event`.
      await ipc.sendCodexMessage(text, attachments, sessionId)
    } catch (err) {
      const msg =
        err instanceof Error ? err.message : typeof err === "string" ? err : String(err)
      get().finalizeTurn()
      useRuntimeStore.getState().setWorkspaceStatus({ workspaceId, sessionId }, "error")
      useRuntimeStore.getState().addEventForOwner({ workspaceId, sessionId }, {
        id: crypto.randomUUID(),
        timestamp: new Date().toISOString(),
        event: { type: "error", message: msg, recoverable: false },
        displayLabel: "无法发送消息",
        displayStatus: "error",
        detail: msg,
      })
    }
  },
}))
