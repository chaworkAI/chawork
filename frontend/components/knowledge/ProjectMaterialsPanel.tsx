import { useCallback, useEffect, useRef, type DragEvent } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { X } from "lucide-react"

import { ContentViewer } from "@/components/knowledge/ContentViewer"
import { ImportTaskList } from "@/components/import/ImportTaskList"
import { IndexStatusBadge } from "@/components/knowledge/IndexStatusBadge"
import { SearchBar } from "@/components/knowledge/SearchBar"
import { SearchResultList } from "@/components/knowledge/SearchResultList"
import { Button } from "@/components/ui/button"
import { useDomainStore } from "@/stores/domain"
import { useImportStore } from "@/stores/import"
import { useKnowledgeStore } from "@/stores/knowledge"
import { useUiStore } from "@/stores/ui"
import { SUPPORTED_EXTENSIONS } from "@/types/import"

const ACCEPT_ATTR = SUPPORTED_EXTENSIONS.map((e) => `.${e}`).join(",")

export function ProjectMaterialsPanel() {
  const openPanel = useUiStore((s) => s.projectMaterialsOpen)
  const setOpen = useUiStore((s) => s.setProjectMaterialsOpen)
  const importFile = useImportStore((s) => s.importFile)
  const loadTasks = useImportStore((s) => s.loadTasks)
  const tasks = useImportStore((s) => s.tasks)
  const isImporting = useImportStore((s) => s.isImporting)
  const submitError = useImportStore((s) => s.submitError)
  const query = useKnowledgeStore((s) => s.query)
  const results = useKnowledgeStore((s) => s.results)
  const isSearching = useKnowledgeStore((s) => s.isSearching)
  const status = useKnowledgeStore((s) => s.status)
  const activeDocPath = useKnowledgeStore((s) => s.activeDocPath)
  const activeDocContent = useKnowledgeStore((s) => s.activeDocContent)
  const setQuery = useKnowledgeStore((s) => s.setQuery)
  const search = useKnowledgeStore((s) => s.search)
  const loadStatus = useKnowledgeStore((s) => s.loadStatus)
  const refresh = useKnowledgeStore((s) => s.refresh)
  const openDocument = useKnowledgeStore((s) => s.openDocument)
  const closeDocument = useKnowledgeStore((s) => s.closeDocument)
  const dropRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!openPanel) return
    void loadStatus()
    void loadTasks()
    const id = setInterval(() => void loadTasks(), 1500)
    return () => clearInterval(id)
  }, [openPanel, loadStatus, loadTasks])

  const close = () => {
    closeDocument()
    setOpen(false)
  }

  const handleFileSelect = useCallback(async () => {
    const filterName = useDomainStore
      .getState()
      .getLabel("import.dialog.supported_files", "支持的文档")
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: filterName,
          extensions: [...SUPPORTED_EXTENSIONS],
        },
      ],
    })
    if (!selected) return
    const paths = Array.isArray(selected) ? selected : [selected]
    for (const path of paths) {
      if (typeof path === "string") {
        await importFile(path)
      }
    }
  }, [importFile])

  const isAllowed = useCallback((name: string) => {
    const dot = name.lastIndexOf(".")
    if (dot < 0) return false
    const ext = name.slice(dot + 1).toLowerCase()
    return (SUPPORTED_EXTENSIONS as readonly string[]).includes(ext)
  }, [])

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const files = e.dataTransfer.files
      for (let i = 0; i < files.length; i++) {
        const path = (files[i] as unknown as { path?: string }).path
        if (typeof path === "string" && isAllowed(path)) {
          void importFile(path)
        }
      }
    },
    [importFile, isAllowed],
  )

  if (!openPanel) return null

  return (
    <div
      className="fixed inset-0 z-70 flex items-center justify-center bg-[rgba(36,40,50,0.28)]"
      role="presentation"
      onClick={close}
      onKeyDown={(e) => {
        if (e.key === "Escape") close()
      }}
    >
      <section
        className="flex max-h-[min(760px,calc(100dvh-48px))] w-[min(920px,calc(100vw-80px))] flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)]"
        role="dialog"
        aria-modal="true"
        aria-label="项目资料"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex shrink-0 items-start justify-between gap-4 border-b border-line-soft px-[22px] py-5">
          <div className="min-w-0">
            <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
              工作区
            </p>
            <h2 className="mt-[3px] text-[21px] font-extrabold text-ink">项目资料</h2>
            <p className="mt-1 text-[13px] text-muted-foreground">
              导入资料、刷新索引、搜索并查看已入库文档
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="icon-sm"
            className="size-[38px] rounded-[12px] bg-white"
            aria-label="关闭项目资料"
            onClick={close}
          >
            <X className="size-4" />
          </Button>
        </header>

        <div className="grid min-h-0 flex-1 grid-cols-[320px_minmax(0,1fr)] gap-0 max-[760px]:grid-cols-1">
          <aside className="min-h-0 overflow-y-auto border-r border-line-soft p-[22px] max-[760px]:border-r-0 max-[760px]:border-b">
            <section>
              <h3 className="text-[12px] font-semibold text-ink">导入资料</h3>
              <div
                ref={dropRef}
                onDrop={handleDrop}
                onDragOver={(e) => {
                  e.preventDefault()
                  e.stopPropagation()
                }}
                className="mt-3 rounded-[15px] border border-dashed border-line bg-[#f8f9fb] px-3.5 py-5 text-center"
              >
                <p className="text-[12px] text-muted-foreground">拖入文件，或选择本地文档</p>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  className="mt-3 h-[36px] rounded-[12px] bg-white px-4"
                  onClick={() => void handleFileSelect()}
                >
                  选择文件
                </Button>
                <p className="mt-2 font-mono text-[10px] text-muted-foreground">{ACCEPT_ATTR}</p>
              </div>
              {submitError ? (
                <div className="mt-3 rounded-[13px] border border-danger/20 bg-danger/10 px-3 py-2 text-[12px] text-danger">
                  {submitError}
                </div>
              ) : null}
              {isImporting ? (
                <div className="mt-3 rounded-[13px] border border-warning/20 bg-warning/10 px-3 py-2 text-[12px] text-warning">
                  有导入任务正在运行
                </div>
              ) : null}
            </section>

            <section className="mt-5">
              <h3 className="text-[12px] font-semibold text-ink">导入任务</h3>
              <div className="mt-2">
                <ImportTaskList tasks={tasks} />
              </div>
            </section>
          </aside>

          <main className="flex min-h-0 flex-col overflow-hidden p-[22px]">
            <div className="mb-3 flex shrink-0 items-center justify-between gap-3">
              <h3 className="text-[12px] font-semibold text-ink">搜索资料</h3>
              <IndexStatusBadge status={status} onRefresh={() => void refresh()} />
            </div>
            <div className="shrink-0">
              <SearchBar
                value={query}
                onChange={setQuery}
                onSearch={() => void search()}
                isSearching={isSearching}
              />
            </div>
            <div className="mt-3 min-h-0 flex-1 overflow-hidden">
              {activeDocPath ? (
                <ContentViewer
                  filePath={activeDocPath}
                  content={activeDocContent}
                  onClose={closeDocument}
                />
              ) : (
                <div className="h-full overflow-y-auto">
                  <SearchResultList
                    results={results}
                    onSelect={(filePath) => void openDocument(filePath)}
                  />
                </div>
              )}
            </div>
          </main>
        </div>
      </section>
    </div>
  )
}
