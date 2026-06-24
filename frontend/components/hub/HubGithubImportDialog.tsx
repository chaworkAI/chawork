import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { GitBranch, Link2, Loader2, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import {
  githubOwnerNameFromUrl,
  githubRepoNameFromUrl,
  validateGithubRepoUrl,
} from "@/lib/githubUrl"
import { scanGithubRepo } from "@/lib/githubDirectScan"
import * as ipc from "@/lib/tauri"
import { useHubStore } from "@/stores/hub"
import { useEmployeeStore } from "@/stores/employee"
import { useToastStore } from "@/stores/toast"
import type { GithubSkillPreview } from "@/types/hub"

type ImportPhase = "form" | "preview" | "importing"

function defaultEmployeePrompt(ownerName: string, repoName: string) {
  return `你是 ${ownerName}（${repoName} 仓库）的技能合集员工。优先使用已绑定的 repo skills 完成用户任务。`
}

function SyncAsEmployeeSection({
  ownerName,
  repoName,
  syncAsEmployee,
  employeePrompt,
  disabled,
  onSyncChange,
  onPromptChange,
}: {
  ownerName: string
  repoName: string
  syncAsEmployee: boolean
  employeePrompt: string
  disabled?: boolean
  onSyncChange: (checked: boolean) => void
  onPromptChange: (value: string) => void
}) {
  return (
    <div className="grid gap-3 rounded-[12px] border border-line bg-[#f6f8fb] px-4 py-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-extrabold text-ink">同步为员工</p>
          <p className="mt-1 text-[12px] leading-5 text-muted-foreground">
            导入后把仓库全部技能写入 Root，并以 GitHub 用户名「{ownerName}」创建员工（{repoName}）。
          </p>
        </div>
        <Switch
          checked={syncAsEmployee}
          disabled={disabled}
          aria-label="同步为员工"
          onCheckedChange={onSyncChange}
        />
      </div>
      {syncAsEmployee ? (
        <label className="grid gap-2">
          <span className="text-[12px] font-bold text-ink">员工定义 prompt</span>
          <Textarea
            value={employeePrompt}
            disabled={disabled}
            onChange={(event) => onPromptChange(event.target.value)}
            placeholder="描述该员工的角色、工作方式与技能使用原则…"
            className="min-h-[120px] rounded-[12px] border-line bg-white text-[13px] leading-6"
          />
        </label>
      ) : null}
    </div>
  )
}

export function HubGithubImportDialog() {
  const open = useHubStore((s) => s.githubImportOpen)
  const closeGithubImport = useHubStore((s) => s.closeGithubImport)

  const [phase, setPhase] = useState<ImportPhase>("form")
  const [url, setUrl] = useState("")
  const [gitRef, setGitRef] = useState("")
  const [repoUrl, setRepoUrl] = useState("")
  const [skills, setSkills] = useState<GithubSkillPreview[]>([])
  const [selectedPaths, setSelectedPaths] = useState<string[]>([])
  const [syncAsEmployee, setSyncAsEmployee] = useState(true)
  const [employeePrompt, setEmployeePrompt] = useState("")
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [urlTouched, setUrlTouched] = useState(false)
  const abortRef = useRef<AbortController | null>(null)

  const handleClose = useCallback(() => {
    abortRef.current?.abort()
    abortRef.current = null
    closeGithubImport()
  }, [closeGithubImport])

  const urlValidation = useMemo(() => validateGithubRepoUrl(url), [url])
  const normalizedUrl = urlValidation.normalized
  const urlFormatError = urlTouched ? urlValidation.message : null

  const reset = useCallback(() => {
    setPhase("form")
    setUrl("")
    setGitRef("")
    setRepoUrl("")
    setSkills([])
    setSelectedPaths([])
    setSyncAsEmployee(true)
    setEmployeePrompt("")
    setLoading(false)
    setError(null)
    setUrlTouched(false)
  }, [])

  useEffect(() => {
    if (!open) {
      abortRef.current?.abort()
      abortRef.current = null
      reset()
    }
  }, [open, reset])

  const repoName = useMemo(
    () => githubRepoNameFromUrl(repoUrl || normalizedUrl || url),
    [normalizedUrl, repoUrl, url],
  )
  const ownerName = useMemo(
    () => githubOwnerNameFromUrl(repoUrl || normalizedUrl || url),
    [normalizedUrl, repoUrl, url],
  )

  useEffect(() => {
    if (!syncAsEmployee || employeePrompt.trim() || !ownerName || !repoName) return
    setEmployeePrompt(defaultEmployeePrompt(ownerName, repoName))
  }, [employeePrompt, ownerName, repoName, syncAsEmployee])

  const canPreview =
    urlValidation.formatOk &&
    !loading &&
    (!syncAsEmployee || employeePrompt.trim().length > 0)
  const allSelected = skills.length > 0 && selectedPaths.length === skills.length
  const selectedCount = selectedPaths.length
  const canImport =
    selectedCount > 0 &&
    !loading &&
    (!syncAsEmployee || employeePrompt.trim().length > 0)

  const previewSubtitle = useMemo(() => {
    if (!repoUrl) return ""
    return `在 ${repoUrl} 找到 ${skills.length} 个技能`
  }, [repoUrl, skills.length])

  const handleSyncAsEmployeeChange = (checked: boolean) => {
    setSyncAsEmployee(checked)
    if (checked && !employeePrompt.trim()) {
      setEmployeePrompt(defaultEmployeePrompt(ownerName, repoName))
    }
  }

  const handlePreview = async () => {
    setUrlTouched(true)
    const validation = validateGithubRepoUrl(url)
    if (!validation.formatOk || !validation.normalized) {
      setError(validation.message ?? "请输入有效的 GitHub 仓库地址")
      return
    }
    if (syncAsEmployee && !employeePrompt.trim()) {
      setError("开启同步为员工时请填写员工定义 prompt")
      return
    }

    abortRef.current?.abort()
    const abortController = new AbortController()
    abortRef.current = abortController

    setLoading(true)
    setError(null)
    try {
      const previewSkills = await scanGithubRepo(
        validation.normalized,
        gitRef.trim() || undefined,
      )
      if (abortController.signal.aborted) return

      if (previewSkills.length === 0) {
        throw new Error(
          `未在 ${validation.normalized} 找到可导入的技能。请确认仓库为公开、根目录或子目录中包含 SKILL.md。`,
        )
      }

      setSkills(previewSkills)
      setSelectedPaths(previewSkills.map((s) => s.path))
      setRepoUrl(validation.normalized)

      if (syncAsEmployee && !employeePrompt.trim()) {
        setEmployeePrompt(
          defaultEmployeePrompt(
            githubOwnerNameFromUrl(validation.normalized),
            githubRepoNameFromUrl(validation.normalized),
          ),
        )
      }
      setPhase("preview")
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") return
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      if (!abortController.signal.aborted) {
        setLoading(false)
      }
    }
  }

  const toggleSkill = (skillPath: string) => {
    setSelectedPaths((current) =>
      current.includes(skillPath)
        ? current.filter((p) => p !== skillPath)
        : [...current, skillPath],
    )
  }

  const toggleAll = () => {
    setSelectedPaths(allSelected ? [] : skills.map((s) => s.path))
  }

  const handleImportSelected = async () => {
    if (selectedPaths.length === 0) return
    if (syncAsEmployee && !employeePrompt.trim()) {
      setError("开启同步为员工时请填写员工定义 prompt")
      return
    }

    abortRef.current?.abort()
    const abortController = new AbortController()
    abortRef.current = abortController

    setPhase("importing")
    setLoading(true)
    setError(null)
    try {
      const result = await ipc.githubCompleteImport(
        repoUrl,
        selectedPaths,
        gitRef.trim() || undefined,
        syncAsEmployee,
        syncAsEmployee ? employeePrompt.trim() : undefined,
      )
      if (abortController.signal.aborted) return

      // 1. 更新 employee localStorage 追踪（skill 数据源由后端 .hub_origin.json 管理）
      if (result.employeeId) {
        const {
          writeUserCustomGithubEmployeeIds,
          readUserCustomGithubEmployeeIds,
        } = await import("@/lib/hubCustomEmployees")
        const existingEmpIds = readUserCustomGithubEmployeeIds()
        const newEmpIds = [...new Set([...existingEmpIds, result.employeeId])]
        writeUserCustomGithubEmployeeIds(newEmpIds)
        useHubStore.setState({ userCustomGithubEmployeeIds: newEmpIds })
      }

      // 2. 刷新视图数据
      const hubStore = useHubStore.getState()
      await Promise.allSettled([
        hubStore.loadManifest(),
        hubStore.loadSkills(),
        hubStore.loadEmployees(),
      ])
      if (result.employeeId) {
        await useEmployeeStore.getState().loadEmployees()
      }

      // 3. Toast 提示
      const failedCount = result.failed?.length ?? 0
      const failedSuffix = failedCount > 0 ? `，${failedCount} 个技能安装失败` : ""
      const employeeSuffix =
        result.employeeId && result.employeeName
          ? `，并创建员工「${result.employeeName}」`
          : ""
      useToastStore.getState().show(
        `已从 GitHub 导入 ${result.installedCount} 个技能到 Root${employeeSuffix}${failedSuffix}`,
        failedCount > 0 ? "error" : "success",
      )

      // 4. 切换视图
      const targetTab = result.employeeId ? "employees" : "skills"
      useHubStore.setState({
        githubImportOpen: false,
        marketOpen: true,
      })
      hubStore.setActiveTab(targetTab)
      hubStore.setSkillsFilter("custom")
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") return
      setError(e instanceof Error ? e.message : String(e))
      setPhase("preview")
    } finally {
      if (!abortController.signal.aborted) {
        setLoading(false)
      }
    }
  }

  const previewLoadingLabel = loading
    ? skills.length === 0
      ? "正在扫描仓库…"
      : "正在导入技能…"
    : ""

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) handleClose()
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[100] bg-black/30 backdrop-blur-[1px]" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 z-[101] grid max-h-[min(92vh,860px)] w-[min(560px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 grid-rows-[auto_minmax(0,1fr)] overflow-hidden rounded-[18px] border border-line bg-panel shadow-2xl outline-none"
          onEscapeKeyDown={(event) => {
            event.preventDefault()
            handleClose()
          }}
        >
          <Dialog.Description className="sr-only">
            从 GitHub 仓库导入技能到 Root，可选同步创建员工。
          </Dialog.Description>
          <header className="flex items-start justify-between border-b border-line px-6 py-5">
            <div>
              <p className="text-[11px] font-extrabold uppercase tracking-wide text-muted-foreground">
                GitHub Import
              </p>
              <Dialog.Title className="mt-1 text-[20px] font-black text-ink">
                从 GitHub 导入技能
              </Dialog.Title>
            </div>
            <Button
              type="button"
              variant="outline"
              size="icon-lg"
              onClick={handleClose}
            >
              <X className="size-4" />
            </Button>
          </header>

          <div className="min-h-0 overflow-y-auto px-6 py-5">
            <div className="grid gap-4">
              {phase === "form" ? (
                <>
                  <label className="grid gap-2">
                    <span className="text-[12px] font-bold text-ink">GitHub 仓库地址</span>
                    <div className="relative">
                      <Link2 className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                      <Input
                        value={url}
                        onChange={(event) => {
                          setUrl(event.target.value)
                          if (error) setError(null)
                        }}
                        onBlur={() => setUrlTouched(true)}
                        placeholder="https://github.com/user/repo 或 user/repo"
                        aria-invalid={urlTouched && !urlValidation.formatOk}
                        className={[
                          "h-10 rounded-[12px] border-line bg-white pl-9 text-[13px]",
                          urlTouched && !urlValidation.formatOk ? "border-[#f0b8b8]" : "",
                        ].join(" ")}
                      />
                    </div>
                    {urlFormatError ? (
                      <span className="text-[12px] text-[#8f2424]">{urlFormatError}</span>
                    ) : urlValidation.formatOk && normalizedUrl ? (
                      <span className="text-[12px] text-muted-foreground">
                        将导入：{normalizedUrl}
                      </span>
                    ) : null}
                  </label>
                  <label className="grid gap-2">
                    <span className="text-[12px] font-bold text-ink">分支（可选）</span>
                    <div className="relative">
                      <GitBranch className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                      <Input
                        value={gitRef}
                        onChange={(event) => setGitRef(event.target.value)}
                        placeholder="main"
                        className="h-10 rounded-[12px] border-line bg-white pl-9 text-[13px]"
                      />
                    </div>
                  </label>

                  <SyncAsEmployeeSection
                    ownerName={ownerName}
                    repoName={repoName}
                    syncAsEmployee={syncAsEmployee}
                    employeePrompt={employeePrompt}
                    disabled={loading}
                    onSyncChange={handleSyncAsEmployeeChange}
                    onPromptChange={setEmployeePrompt}
                  />

                  <Button
                    type="button"
                    size="lg"
                    className="w-full"
                    disabled={!canPreview}
                    onClick={() => void handlePreview()}
                  >
                    {loading ? <Loader2 className="size-4 animate-spin" /> : null}
                    预览仓库中的技能
                  </Button>
                  {loading && phase === "form" ? (
                    <p className="text-center text-[12px] text-muted-foreground">
                      {previewLoadingLabel}
                    </p>
                  ) : null}
                </>
              ) : (
                <>
                  <div className="flex items-center justify-between gap-3">
                    <p className="text-[12px] text-muted-foreground">{previewSubtitle}</p>
                    <button
                      type="button"
                      className="text-[12px] font-bold text-[#2457b7] hover:underline"
                      onClick={toggleAll}
                    >
                      {allSelected ? "取消全选" : "全选"}
                    </button>
                  </div>
                  <div className="max-h-[min(36vh,280px)] space-y-2 overflow-y-auto pr-1">
                    {skills.map((skill) => {
                      const checked = selectedPaths.includes(skill.path)
                      return (
                        <label
                          key={skill.path}
                          className={[
                            "flex cursor-pointer items-start gap-3 rounded-[12px] border px-3 py-3 transition",
                            checked
                              ? "border-[#2457b7] bg-[#f5f9ff]"
                              : "border-line-soft bg-white hover:border-line",
                          ].join(" ")}
                        >
                          <input
                            type="checkbox"
                            checked={checked}
                            onChange={() => toggleSkill(skill.path)}
                            className="mt-1 size-4 rounded border-line"
                          />
                          <span className="min-w-0 flex-1">
                            <span className="block text-[14px] font-extrabold text-ink">
                              {skill.name}
                            </span>
                            {skill.description ? (
                              <span className="mt-1 block text-[12px] leading-5 text-muted-foreground">
                                {skill.description}
                              </span>
                            ) : null}
                            <span className="mt-1 block text-[11px] font-medium text-muted-foreground">
                              {skill.path}
                            </span>
                          </span>
                        </label>
                      )
                    })}
                  </div>

                  <SyncAsEmployeeSection
                    ownerName={ownerName}
                    repoName={repoName}
                    syncAsEmployee={syncAsEmployee}
                    employeePrompt={employeePrompt}
                    disabled={loading}
                    onSyncChange={handleSyncAsEmployeeChange}
                    onPromptChange={setEmployeePrompt}
                  />

                  <div className="flex items-center justify-end gap-2 border-t border-line-soft pt-4">
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => setPhase("form")}
                    >
                      上一步
                    </Button>
                    <Button type="button" variant="outline" onClick={handleClose}>
                      取消
                    </Button>
                    <Button
                      type="button"
                      disabled={!canImport}
                      onClick={() => void handleImportSelected()}
                    >
                      {loading ? <Loader2 className="size-4 animate-spin" /> : null}
                      导入选中的技能 ({selectedCount})
                    </Button>
                  </div>
                  {loading && phase === "importing" ? (
                    <p className="text-center text-[12px] text-muted-foreground">
                      {previewLoadingLabel}
                    </p>
                  ) : null}
                </>
              )}

              {error ? (
                <div className="rounded-[12px] border border-[#f0b8b8] bg-[#fff1f1] px-3 py-2 text-[12px] text-[#8f2424]">
                  {error}
                </div>
              ) : null}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
