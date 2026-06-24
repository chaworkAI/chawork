import type { ImportTask, ImportTaskStatus } from "@/types/import"
import { isSuccessStatus, isTerminalStatus } from "@/types/import"
import { useChatStore } from "@/stores/chat"
import { useUiLabel } from "@/hooks/useUiLabel"
import { useImportStore } from "@/stores/import"
import { useKnowledgeStore } from "@/stores/knowledge"
import { useUiStore } from "@/stores/ui"

export interface ImportTaskListProps {
  tasks: ImportTask[]
}

interface StatusPresentation {
  label: string
  badgeClass: string
}

const STATUS_PRESENTATION: Record<ImportTaskStatus, StatusPresentation> = {
  queued: { label: "排队中", badgeClass: "bg-muted/30 text-muted-foreground" },
  saving_source: { label: "保存原文…", badgeClass: "bg-[rgba(193,151,98,0.15)] text-ink" },
  converting_to_markdown: {
    label: "转 Markdown…",
    badgeClass: "bg-[rgba(193,151,98,0.15)] text-ink",
  },
  writing_wiki: {
    label: "写入 wiki…",
    badgeClass: "bg-[rgba(193,151,98,0.15)] text-ink",
  },
  refreshing_index: {
    label: "刷新索引…",
    badgeClass: "bg-[rgba(193,151,98,0.15)] text-ink",
  },
  completed: {
    label: "完成",
    badgeClass: "bg-success/15 text-success",
  },
  completed_with_index_error: {
    label: "完成（索引出错）",
    badgeClass: "bg-warning/15 text-warning",
  },
  failed_save: {
    label: "保存失败",
    badgeClass: "bg-danger/15 text-danger",
  },
  failed_convert: {
    label: "解析失败",
    badgeClass: "bg-danger/15 text-danger",
  },
  failed_write: {
    label: "写入失败",
    badgeClass: "bg-danger/15 text-danger",
  },
  cancelled: {
    label: "已取消",
    badgeClass: "bg-muted/30 text-muted-foreground",
  },
}

function formatTimestamp(ts: string): string {
  try {
    return new Date(ts).toLocaleString()
  } catch {
    return ts
  }
}

function basenameNoExt(filename: string): string {
  const base = filename.replace(/^.*[/\\]/, "")
  const dot = base.lastIndexOf(".")
  return dot > 0 ? base.slice(0, dot) : base
}

export function ImportTaskList({ tasks }: ImportTaskListProps) {
  const getLabel = useUiLabel()
  const importFile = useImportStore((s) => s.importFile)
  const openDocument = useKnowledgeStore((s) => s.openDocument)
  const setKnowledgeQuery = useKnowledgeStore((s) => s.setQuery)
  const searchKnowledge = useKnowledgeStore((s) => s.search)
  const setProjectMaterialsOpen = useUiStore((s) => s.setProjectMaterialsOpen)
  const setPendingComposerPrefill = useChatStore((s) => s.setPendingComposerPrefill)

  const handleOpenWiki = (wikiPath: string) => {
    void openDocument(wikiPath)
  }

  const handleSummarize = (filename: string) => {
    const title = basenameNoExt(filename)
    setProjectMaterialsOpen(false)
    setPendingComposerPrefill(
      `请总结资料「${title}」的要点，并说明与当前工作区任务的关系。`,
    )
  }

  const handleSearchKnowledge = (filename: string) => {
    const q = basenameNoExt(filename)
    setKnowledgeQuery(q)
    void searchKnowledge(q)
  }

  const handleRetry = (sourcePath: string) => {
    void importFile(sourcePath)
  }

  const handleCopyError = async (error: string) => {
    try {
      await navigator.clipboard.writeText(error)
    } catch {
      /* ignore */
    }
  }

  if (tasks.length === 0) {
    return (
      <p className="py-4 text-center text-[12px] text-muted-foreground">
        {getLabel("import.task_list_empty", "暂无导入任务")}
      </p>
    )
  }

  return (
    <ul className="space-y-1.5">
      {tasks.map((t) => {
        const p = STATUS_PRESENTATION[t.status] ?? STATUS_PRESENTATION.queued
        const inFlight = !isTerminalStatus(t.status)
        const succeeded = isSuccessStatus(t.status)
        const failed =
          isTerminalStatus(t.status) &&
          !succeeded &&
          t.status !== "cancelled"

        return (
          <li
            key={t.id}
            className="rounded-[10px] border border-line bg-[rgba(255,255,255,0.42)] px-3 py-2"
          >
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-ink">
                  {t.source_filename}
                </div>
                <div className="mt-0.5 font-mono text-[10px] text-muted-foreground/70">
                  {t.id.slice(0, 8)} · {t.source_type}
                </div>
              </div>
              <span
                className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] ${p.badgeClass}`}
              >
                {inFlight ? (
                  <span className="mr-1 inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-current align-middle" />
                ) : null}
                {p.label}
              </span>
            </div>

            {t.wiki_path ? (
              <p className="mt-1 truncate text-[11px] text-muted-foreground">
                → <span className="font-mono">{t.wiki_path}</span>
              </p>
            ) : null}

            {t.error && !isSuccessStatus(t.status) ? (
              <p className="mt-1 text-[11px] text-danger">{t.error}</p>
            ) : null}

            {t.error && t.status === "completed_with_index_error" ? (
              <p className="mt-1 text-[11px] text-warning">{t.error}</p>
            ) : null}

            {succeeded && t.wiki_path ? (
              <div className="mt-2 flex flex-wrap gap-1.5">
                <button
                  type="button"
                  className="rounded-[8px] border border-line bg-[rgba(255,255,255,0.55)] px-2 py-1 text-[11px] text-ink hover:bg-[rgba(255,255,255,0.85)]"
                  onClick={() => handleOpenWiki(t.wiki_path!)}
                >
                  {getLabel("import.action.open_wiki", "打开 wiki")}
                </button>
                <button
                  type="button"
                  className="rounded-[8px] border border-line bg-[rgba(255,255,255,0.55)] px-2 py-1 text-[11px] text-ink hover:bg-[rgba(255,255,255,0.85)]"
                  onClick={() => handleSummarize(t.source_filename)}
                >
                  {getLabel("import.action.summarize", "让 AI 总结")}
                </button>
                <button
                  type="button"
                  className="rounded-[8px] border border-line bg-[rgba(255,255,255,0.55)] px-2 py-1 text-[11px] text-ink hover:bg-[rgba(255,255,255,0.85)]"
                  onClick={() => handleSearchKnowledge(t.source_filename)}
                >
                  {getLabel("import.action.search", "在知识库搜索")}
                </button>
              </div>
            ) : null}

            {failed ? (
              <div className="mt-2 flex flex-wrap gap-1.5">
                <button
                  type="button"
                  className="rounded-[8px] border border-line bg-[rgba(255,255,255,0.55)] px-2 py-1 text-[11px] text-ink hover:bg-[rgba(255,255,255,0.85)]"
                  onClick={() => handleRetry(t.source_path)}
                >
                  {getLabel("import.action.retry", "重试")}
                </button>
                {t.error ? (
                  <button
                    type="button"
                    className="rounded-[8px] border border-line bg-[rgba(255,255,255,0.55)] px-2 py-1 text-[11px] text-ink hover:bg-[rgba(255,255,255,0.85)]"
                    onClick={() => void handleCopyError(t.error!)}
                  >
                    {getLabel("import.action.copy_error", "复制错误")}
                  </button>
                ) : null}
              </div>
            ) : null}

            <p className="mt-1 text-[10px] text-muted-foreground/70">
              {formatTimestamp(t.updated_at)}
            </p>
          </li>
        )
      })}
    </ul>
  )
}
