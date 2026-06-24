export interface Attachment {
  kind: "image"
  path?: string
  data_url?: string
  name?: string
  mime_type?: string
}

export interface SourceRef {
  path: string
  title?: string
  snippet?: string
}

export type TranscriptEntry =
  | { role: "user"; content: string; timestamp: string; attachments?: Attachment[] }
  | { role: "assistant"; content: string; timestamp: string; sources?: SourceRef[] }

export interface Message {
  id: string
  role: "user" | "assistant"
  content: string
  timestamp: string
  attachments?: Attachment[]
  sources?: SourceRef[]
  isStreaming?: boolean
  /** Chain-of-thought / reasoning stream (shown above final answer). */
  thinkingContent?: string
  isThinkingStreaming?: boolean
  /** User-controlled collapse; auto-collapses when answer text starts. */
  isThinkingExpanded?: boolean
  /** Muted auxiliary line shown under assistant content */
  footnote?: string
  /** Turn ended after thinking with no assistant body text. */
  emptyReply?: boolean
}
