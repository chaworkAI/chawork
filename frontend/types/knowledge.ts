/** QMD index status returned by the backend (embedded Tantivy). */
export interface QmdStatus {
  index_name: string
  raw_output: string
  is_ready: boolean
  /** Number of indexed documents (0 if not built). */
  doc_count?: number
  /** `ready` | `stale` | `building` | `error` — aligned with workspace index_status. */
  phase?: string
}

/** A single BM25 search result from `qmd search`. */
export interface QmdSearchResult {
  docid: string
  score: number
  file: string
  title: string
  snippet: string
  /** Chunk-level fields (chawork chunker v3): char offsets into the source file. */
  start_char?: number
  end_char?: number
  breadcrumb?: string
  chunk_index?: number
}
