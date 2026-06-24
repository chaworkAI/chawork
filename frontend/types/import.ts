/** V1 (DESIGN §3 post-revision) — five supported types + Unsupported reject. */
export type SourceType =
  | "text"
  | "markdown"
  | "docx"
  | "xlsx"
  | "csv"
  | "pdf"
  | "unsupported"

/** Lifecycle of a single Import Task (DESIGN §4). */
export type ImportTaskStatus =
  | "queued"
  | "saving_source"
  | "converting_to_markdown"
  | "writing_wiki"
  | "refreshing_index"
  | "completed"
  | "failed_save"
  | "failed_convert"
  | "failed_write"
  | "completed_with_index_error"
  | "cancelled"

export interface ImportTask {
  // Manifest fields
  id: string
  source_path: string
  source_filename: string
  source_type: SourceType
  source_hash: string | null
  created_at: string
  // Result fields
  status: ImportTaskStatus
  raw_path: string | null
  wiki_path: string | null
  parser: string | null
  error: string | null
  updated_at: string
  completed_at: string | null
}

/** Legacy flat-log record kept until any consumer migrates fully off it. */
export interface ImportRecord {
  timestamp: string
  source_filename: string
  source_type: SourceType
  raw_path: string
  wiki_path: string | null
  success: boolean
}

/** UI: whitelist of file extensions accepted by the picker / drop zone (DESIGN §8). */
export const SUPPORTED_EXTENSIONS = [
  "docx",
  "txt",
  "md",
  "xlsx",
  "csv",
  "pdf",
] as const

/** UI hint for terminal vs in-flight state. */
export function isTerminalStatus(s: ImportTaskStatus): boolean {
  return (
    s === "completed" ||
    s === "failed_save" ||
    s === "failed_convert" ||
    s === "failed_write" ||
    s === "completed_with_index_error" ||
    s === "cancelled"
  )
}

export function isSuccessStatus(s: ImportTaskStatus): boolean {
  return s === "completed" || s === "completed_with_index_error"
}
