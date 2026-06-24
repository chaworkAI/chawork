import { useCallback, useEffect, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { Loader2, RefreshCw, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import * as ipc from "@/lib/tauri"
import { useMcpToolStore } from "@/stores/mcpTool"
import { useProviderStore } from "@/stores/provider"
import { useRuntimeStore } from "@/stores/runtime"
import { useSessionStore } from "@/stores/session"
import {
  useWorkspaceStore,
  type WorkspaceConfigTabKey,
} from "@/stores/workspace"
import type { QmdStatus } from "@/types/knowledge"
import type { WorkspaceMcpServer } from "@/types/mcp"

const TABS = [
  { key: "overview", label: "概览" },
  { key: "domain_pack", label: "Domain Pack" },
  { key: "index", label: "索引" },
  { key: "provider", label: "模型配置" },
  { key: "tools", label: "工具" },
] as const

const RUNTIME_MCP_ENABLED = false

const SAMPLE_MCP_JSON = `{
  "mcpServers": {
    "xsct-bench": {
      "type": "streamable_http",
      "url": "https://mcp.api-inference.modelscope.net/6460f2d4ed8347/mcp",
      "headers": {
        "Authorization": "Bearer ms-..."
      }
    }
  }
}`

type TabKey = WorkspaceConfigTabKey

export interface WorkspaceConfigPanelProps {
  open: boolean
  onClose: () => void
}

function IndexTab() {
  const [status, setStatus] = useState<QmdStatus | null>(null)
  const [loading, setLoading] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const s = await ipc.qmdStatus()
      setStatus(s)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const handleRefresh = async () => {
    setRefreshing(true)
    setError(null)
    try {
      await ipc.qmdRefresh()
      const s = await ipc.qmdStatus()
      setStatus(s)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setRefreshing(false)
    }
  }

  if (loading) {
    return (
      <div className="flex items-center gap-2 py-8 justify-center text-[13px] text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        加载索引状态…
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h4 className="text-[14px] font-semibold text-ink">知识索引</h4>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={refreshing}
          onClick={() => void handleRefresh()}
        >
          {refreshing ? <Loader2 className="mr-1.5 size-3.5 animate-spin" /> : <RefreshCw className="mr-1.5 size-3.5" />}
          刷新索引
        </Button>
      </div>

      {error ? (
        <p className="rounded-md border border-danger/30 bg-danger/5 px-3 py-2 text-[13px] text-danger">
          {error}
        </p>
      ) : null}

      <dl className="space-y-2 text-[13px]">
        <div className="flex items-center gap-2">
          <dt className="font-medium text-ink">状态：</dt>
          <dd className={status?.is_ready ? "text-success" : "text-warning"}>
            {status?.is_ready ? "就绪" : "未就绪"}
          </dd>
        </div>
        {status?.doc_count !== undefined ? (
          <div className="flex items-center gap-2">
            <dt className="font-medium text-ink">已索引文档：</dt>
            <dd className="text-muted-foreground">{status.doc_count} 篇</dd>
          </div>
        ) : null}
        {status?.index_name ? (
          <div className="flex items-center gap-2">
            <dt className="font-medium text-ink">索引名称：</dt>
            <dd className="font-mono text-[12px] text-muted-foreground">{status.index_name}</dd>
          </div>
        ) : null}
      </dl>
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[13px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3">
      <span className="text-[11px] font-extrabold uppercase text-[var(--subtle)]">
        {label}
      </span>
      <p className="mt-0.5 break-all text-[12px] text-ink">{value}</p>
    </div>
  )
}

export function WorkspaceConfigPanel({ open, onClose }: WorkspaceConfigPanelProps) {
  const [activeTab, setActiveTab] = useState<TabKey>("overview")
  const [serverEditorOpen, setServerEditorOpen] = useState(false)
  const [serverEditorMode, setServerEditorMode] = useState<"paste" | "manual">("paste")
  const [pasteJson, setPasteJson] = useState(SAMPLE_MCP_JSON)
  const [serverName, setServerName] = useState("")
  const [serverType, setServerType] = useState<WorkspaceMcpServer["type"]>("streamable_http")
  const [serverUrl, setServerUrl] = useState("")
  const [serverCommand, setServerCommand] = useState("")
  const [serverArgs, setServerArgs] = useState("")
  const [serverHeaders, setServerHeaders] = useState(`{\n  "Authorization": "Bearer "\n}`)
  const [serverSaving, setServerSaving] = useState(false)
  const [serverLocalError, setServerLocalError] = useState<string | null>(null)
  const workspaceConfigTab = useWorkspaceStore((s) => s.workspaceConfigTab)

  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId)
  const workspaceSidebarItems = useWorkspaceStore((s) => s.workspaceSidebarItems)
  const sessions = useSessionStore((s) => s.sessions)

  const effective = useProviderStore((s) => s.effective)
  const globalProvider = useProviderStore((s) => s.globalProvider)
  const loadEffectiveProvider = useProviderStore((s) => s.loadEffectiveProvider)

  const policy = useMcpToolStore((s) => s.policy)
  const serverView = useMcpToolStore((s) => s.serverView)
  const serverTestResults = useMcpToolStore((s) => s.serverTestResults)
  const mcpDirty = useMcpToolStore((s) => s.dirty)
  const mcpLoading = useMcpToolStore((s) => s.loading)
  const serversLoading = useMcpToolStore((s) => s.serversLoading)
  const testingServers = useMcpToolStore((s) => s.testingServers)
  const mcpError = useMcpToolStore((s) => s.error)
  const loadToolPolicy = useMcpToolStore((s) => s.loadToolPolicy)
  const loadServers = useMcpToolStore((s) => s.loadServers)
  const importServersJson = useMcpToolStore((s) => s.importServersJson)
  const upsertServer = useMcpToolStore((s) => s.upsertServer)
  const deleteServer = useMcpToolStore((s) => s.deleteServer)
  const testServer = useMcpToolStore((s) => s.testServer)
  const setToolEnabled = useMcpToolStore((s) => s.setToolEnabled)
  const enableAll = useMcpToolStore((s) => s.enableAll)
  const disableAll = useMcpToolStore((s) => s.disableAll)
  const resetToDefault = useMcpToolStore((s) => s.resetToDefault)
  const savePolicy = useMcpToolStore((s) => s.savePolicy)
  const activeRuntime = useRuntimeStore((s) =>
    activeWorkspaceId ? s.runtimeByWorkspace[activeWorkspaceId] : null,
  )

  const ws = workspaceSidebarItems.find((w) => w.workspace.id === activeWorkspaceId)?.workspace

  useEffect(() => {
    if (!open) return
    setActiveTab(workspaceConfigTab)
  }, [open, workspaceConfigTab])

  useEffect(() => {
    if (!open || !activeWorkspaceId) return
    void loadEffectiveProvider()
    void loadToolPolicy(activeWorkspaceId)
    void loadServers(activeWorkspaceId)
  }, [open, activeWorkspaceId, loadEffectiveProvider, loadToolPolicy, loadServers])

  const effectiveModel =
    effective?.effective_provider?.model ||
    (effective?.effective_scope === "none" ? "—" : "未配置")

  const parseHeaders = () => {
    const text = serverHeaders.trim()
    if (!text) return {}
    const parsed = JSON.parse(text) as unknown
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Headers 必须是 JSON 对象")
    }
    return Object.fromEntries(
      Object.entries(parsed).map(([key, value]) => [key, String(value)]),
    )
  }

  const handleImportServers = async () => {
    if (!activeWorkspaceId) return
    setServerLocalError(null)
    setServerSaving(true)
    try {
      await importServersJson(activeWorkspaceId, pasteJson)
      setServerEditorOpen(false)
    } catch (e) {
      setServerLocalError(e instanceof Error ? e.message : String(e))
    } finally {
      setServerSaving(false)
    }
  }

  const handleSaveManualServer = async () => {
    if (!activeWorkspaceId) return
    setServerLocalError(null)
    const name = serverName.trim()
    setServerSaving(true)
    try {
      const server: WorkspaceMcpServer = {
        name,
        type: serverType,
        url: serverType === "streamable_http" ? serverUrl.trim() : undefined,
        command: serverType === "stdio" ? serverCommand.trim() : undefined,
        args: serverArgs
          .split("\n")
          .map((s) => s.trim())
          .filter(Boolean),
        env: {},
        headers: serverType === "streamable_http" ? parseHeaders() : {},
        enabled: true,
        required: false,
      }
      await upsertServer(activeWorkspaceId, server)
      setServerEditorOpen(false)
      setServerName("")
      setServerUrl("")
      setServerCommand("")
      setServerArgs("")
    } catch (e) {
      setServerLocalError(e instanceof Error ? e.message : String(e))
    } finally {
      setServerSaving(false)
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-80 bg-[rgba(36,40,50,0.28)]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-81 flex max-h-[min(720px,calc(100dvh-48px))] w-[min(760px,calc(100vw-80px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none">
          <div className="flex shrink-0 items-start justify-between gap-4 border-b border-line-soft px-[22px] py-5">
            <div className="min-w-0">
              <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
                工作区
              </p>
              <Dialog.Title className="mt-[3px] text-[21px] font-extrabold tracking-normal text-ink">
                当前工作区配置
              </Dialog.Title>
            </div>
            <Dialog.Description className="sr-only">
              配置当前工作区的 Provider、工具策略与 MCP
            </Dialog.Description>
            <Dialog.Close asChild>
              <button
                type="button"
                className="grid size-[38px] place-items-center rounded-[12px] border border-line bg-white text-muted-foreground transition-colors hover:bg-[#f8f9fb] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label="关闭"
              >
                <X className="size-4" />
              </button>
            </Dialog.Close>
          </div>

          {ws ? (
            <div className="shrink-0 border-b border-line-soft px-[22px] py-3 text-[12px] text-muted-foreground">
              <span className="text-ink">Workspace:</span> {ws.name}{" "}
              <span className="mx-1 text-line">·</span>
              <span className="text-ink">Path:</span>{" "}
              <span className="font-mono text-[11px]">{ws.path}</span>
            </div>
          ) : null}

          {activeRuntime?.lifecycleMessage ? (
            <div className="flex shrink-0 items-center gap-3 border-b border-[#e4d4ac] bg-[#fff8e7] px-[22px] py-3 text-[12px] text-[#6f5b34]">
              <span>{activeRuntime.lifecycleMessage}</span>
            </div>
          ) : null}

          <div className="flex shrink-0 flex-wrap gap-2 border-b border-line-soft px-[22px] py-3">
            {TABS.map((tab) => (
              <button
                key={tab.key}
                type="button"
                onClick={() => setActiveTab(tab.key)}
                className={`min-h-[34px] rounded-full border px-3.5 text-[12px] font-bold transition-colors ${
                  activeTab === tab.key
                    ? "border-[#cbdccc] bg-[#f3faf4] text-ink"
                    : "border-line bg-white text-muted-foreground hover:bg-[#f8f9fb] hover:text-ink"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>

          <div className="min-h-0 flex-1 overflow-auto px-[22px] py-[22px]">
            {activeTab === "overview" && (
              <div className="space-y-3">
                {ws ? (
                  <>
                    <InfoRow label="名称" value={ws.name} />
                    <InfoRow label="路径" value={ws.path} />
                    <InfoRow label="会话数量" value={String(sessions.length)} />
                  </>
                ) : (
                  <p className="text-center text-[12px] text-muted-foreground">未选择工作区</p>
                )}
              </div>
            )}

            {activeTab === "domain_pack" && (
              <p className="py-8 text-center text-[13px] text-muted-foreground">
                Domain Pack 配置暂未实现
              </p>
            )}

            {activeTab === "index" && <IndexTab />}

            {activeTab === "provider" && (
              <div className="space-y-4">
                <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
                  <span className="text-[11px] font-extrabold uppercase text-[var(--subtle)]">
                    当前有效全局模型
                  </span>
                  <p className="mt-1 font-mono text-[14px] text-accent-dark">{effectiveModel}</p>
                  {effective?.blocked_reason && !globalProvider?.valid ? (
                    <p className="mt-1 text-[11px] text-danger">{effective.blocked_reason}</p>
                  ) : null}
                </section>

                <section className="rounded-[15px] border border-line-soft bg-white p-3.5">
                  <h4 className="text-[14px] font-bold text-ink">模型配置归属</h4>
                  <p className="mt-1 text-[12px] leading-5 text-muted-foreground">
                    对话执行只使用全局 provider 配置。当前工作区不保存 API Key、Base URL 或模型覆盖；工作区内只管理知识索引，MCP 配置源当前不会接入普通对话。
                  </p>
                </section>

              </div>
            )}

            {activeTab === "tools" && (
              <div className="space-y-4">
                {!RUNTIME_MCP_ENABLED ? (
                  <div className="rounded-[15px] border border-[#e4d4ac] bg-[#fff8e7] px-3.5 py-3 text-[12px] leading-5 text-[#6f5b34]">
                    当前版本普通对话不接入 MCP。这里保留已有配置用于诊断和后续恢复，但保存、开关或测试不会改变 Chat runtime 可用工具。
                  </div>
                ) : null}
                <section className="space-y-3 rounded-[15px] border border-line-soft bg-white p-3.5">
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <h3 className="text-[13px] font-bold text-ink">MCP Servers</h3>
                      <p className="text-[11px] text-muted-foreground">
                        以 server 名称去重；再次添加同名 server 会直接覆盖。
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      className="h-[34px] rounded-[12px] bg-white px-3"
                      disabled={!RUNTIME_MCP_ENABLED}
                      onClick={() => {
                        if (!RUNTIME_MCP_ENABLED) return
                        setServerEditorOpen((v) => !v)
                      }}
                    >
                      {serverEditorOpen ? "收起" : "添加 MCP Server"}
                    </Button>
                  </div>

                  {serverEditorOpen ? (
                    <div className="space-y-3 rounded-[13px] border border-line-soft bg-[#f8f9fb] p-3">
                      <div className="inline-flex rounded-[11px] border border-line bg-white p-1">
                        {(["paste", "manual"] as const).map((mode) => (
                          <button
                            key={mode}
                            type="button"
                            className={`rounded-[9px] px-3 py-1.5 text-[12px] font-bold ${
                              serverEditorMode === mode
                                ? "bg-primary text-primary-foreground"
                                : "text-muted-foreground"
                            }`}
                            onClick={() => setServerEditorMode(mode)}
                          >
                            {mode === "paste" ? "粘贴配置" : "手动填写"}
                          </button>
                        ))}
                      </div>

                      {serverEditorMode === "paste" ? (
                        <div className="space-y-2">
                          <textarea
                            value={pasteJson}
                            onChange={(e) => setPasteJson(e.target.value)}
                            className="min-h-[190px] w-full resize-y rounded-[12px] border border-line bg-white px-3 py-2 font-mono text-[12px] outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                            spellCheck={false}
                          />
                          <Button
                            type="button"
                            className="h-[34px] rounded-[12px] px-4"
                            disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId || serverSaving}
                            onClick={() => void handleImportServers()}
                          >
                            {serverSaving ? "导入中…" : "导入并覆盖同名"}
                          </Button>
                        </div>
                      ) : (
                        <div className="grid gap-3">
                          <div className="grid grid-cols-[1fr_180px] gap-3">
                            <Input
                              value={serverName}
                              onChange={(e) => setServerName(e.target.value)}
                              placeholder="server 名称，例如 xsct-bench"
                            />
                            <select
                              value={serverType}
                              onChange={(e) =>
                                setServerType(e.target.value as WorkspaceMcpServer["type"])
                              }
                              className="h-10 rounded-[12px] border border-line bg-white px-3 text-[13px] outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                            >
                              <option value="streamable_http">Streamable HTTP</option>
                              <option value="stdio">Stdio</option>
                            </select>
                          </div>
                          {serverType === "streamable_http" ? (
                            <>
                              <Input
                                value={serverUrl}
                                onChange={(e) => setServerUrl(e.target.value)}
                                placeholder="https://example.com/mcp"
                              />
                              <textarea
                                value={serverHeaders}
                                onChange={(e) => setServerHeaders(e.target.value)}
                                className="min-h-[96px] w-full resize-y rounded-[12px] border border-line bg-white px-3 py-2 font-mono text-[12px] outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                                spellCheck={false}
                              />
                            </>
                          ) : (
                            <>
                              <Input
                                value={serverCommand}
                                onChange={(e) => setServerCommand(e.target.value)}
                                placeholder="/absolute/path/to/mcp-server"
                              />
                              <textarea
                                value={serverArgs}
                                onChange={(e) => setServerArgs(e.target.value)}
                                className="min-h-[76px] w-full resize-y rounded-[12px] border border-line bg-white px-3 py-2 font-mono text-[12px] outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                                placeholder="每行一个参数"
                                spellCheck={false}
                              />
                            </>
                          )}
                          <Button
                            type="button"
                            className="h-[34px] w-fit rounded-[12px] px-4"
                            disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId || serverSaving}
                            onClick={() => void handleSaveManualServer()}
                          >
                            {serverSaving ? "保存中…" : "保存并覆盖同名"}
                          </Button>
                        </div>
                      )}
                      {serverLocalError ? (
                        <p className="text-[12px] text-danger">{serverLocalError}</p>
                      ) : null}
                    </div>
                  ) : null}

                  {mcpError ? (
                    <p className="text-[12px] text-danger">{mcpError}</p>
                  ) : null}

                  {!activeWorkspaceId ? (
                    <p className="text-[12px] text-danger">
                      未选择工作区，无法加载 MCP Server 配置。
                    </p>
                  ) : serversLoading ? (
                    <p className="text-[12px] text-muted-foreground">加载 MCP Servers…</p>
                  ) : serverView?.servers.length ? (
                    <div className="space-y-2">
                      {serverView.servers.map((server) => {
                        const testResultForServer = serverTestResults[server.name]
                        const canTestServer = server.type === "streamable_http"
                        const visibleTools =
                          testResultForServer?.tools.length
                            ? testResultForServer.tools
                            : (server.tools ?? [])
                        return (
                          <div
                            key={server.name}
                            className="rounded-[13px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3"
                          >
                            <div className="flex items-center justify-between gap-3">
                              <div className="min-w-0 flex-1">
                                <div className="font-mono text-[12px] font-bold text-ink">
                                  {server.name}
                                </div>
                                <p className="truncate text-[11px] text-muted-foreground">
                                  {server.type} · {server.url || server.command || "未配置入口"}
                                </p>
                                {server.last_tested_at ? (
                                  <p className="mt-0.5 text-[10px] text-muted-foreground">
                                    最近测试：{new Date(server.last_tested_at).toLocaleString("zh-CN")}
                                  </p>
                                ) : null}
                              </div>
                              <div className="flex shrink-0 gap-2">
                                <Button
                                  type="button"
                                  variant="outline"
                                  className="h-[30px] rounded-[10px] bg-white px-3"
                                  disabled={
                                    !RUNTIME_MCP_ENABLED ||
                                    !activeWorkspaceId ||
                                    !canTestServer ||
                                    testingServers[server.name]
                                  }
                                  title={
                                    canTestServer
                                      ? undefined
                                      : "当前只支持手动测试 Streamable HTTP MCP server"
                                  }
                                  onClick={() => {
                                    if (!RUNTIME_MCP_ENABLED || !activeWorkspaceId || !canTestServer) return
                                    void testServer(activeWorkspaceId, server.name)
                                  }}
                                >
                                  {canTestServer
                                    ? testingServers[server.name]
                                      ? "测试中…"
                                      : "测试"
                                    : "随运行时启动"}
                                </Button>
                                <Button
                                  type="button"
                                  variant="outline"
                                  className="h-[30px] rounded-[10px] bg-white px-3"
                                  disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId}
                                  onClick={() => {
                                    if (!RUNTIME_MCP_ENABLED || !activeWorkspaceId) return
                                    void deleteServer(activeWorkspaceId, server.name)
                                  }}
                                >
                                  删除
                                </Button>
                              </div>
                            </div>
                            {testResultForServer ? (
                              <div
                                className={`mt-3 rounded-[11px] border px-3 py-2 text-[12px] ${
                                  testResultForServer.ok
                                    ? "border-success/25 bg-success/10 text-ink"
                                    : "border-danger/25 bg-danger/10 text-danger"
                                }`}
                              >
                                <div className="font-bold">
                                  {testResultForServer.message}
                                </div>
                                {visibleTools.length ? (
                                  <div className="mt-2 grid gap-1">
                                    {visibleTools.map((tool) => (
                                      <div key={tool.name} className="min-w-0">
                                        <span className="font-mono text-[11px] font-bold">
                                          {tool.name}
                                        </span>
                                        {tool.description ? (
                                          <span className="ml-2 text-[11px] text-muted-foreground">
                                            {tool.description}
                                          </span>
                                        ) : null}
                                      </div>
                                    ))}
                                  </div>
                                ) : null}
                              </div>
                            ) : visibleTools.length ? (
                              <div className="mt-3 rounded-[11px] border border-line-soft bg-white px-3 py-2 text-[12px] text-ink">
                                <div className="font-bold">已发现 {visibleTools.length} 个工具</div>
                                <div className="mt-2 grid gap-1">
                                  {visibleTools.map((tool) => (
                                    <div key={tool.name} className="min-w-0">
                                      <span className="font-mono text-[11px] font-bold">
                                        {tool.name}
                                      </span>
                                      {tool.description ? (
                                        <span className="ml-2 text-[11px] text-muted-foreground">
                                          {tool.description}
                                        </span>
                                      ) : null}
                                    </div>
                                  ))}
                                </div>
                              </div>
                            ) : null}
                          </div>
                        )
                      })}
                    </div>
                  ) : (
                    <p className="text-[12px] text-muted-foreground">
                      尚未添加自定义 MCP Server。
                    </p>
                  )}
                </section>

                <section className="space-y-3 rounded-[15px] border border-line-soft bg-white p-3.5">
                  <div>
                    <h3 className="text-[13px] font-bold text-ink">Workspace Tools</h3>
                    <p className="text-[11px] text-muted-foreground">
                      ChaWork 内置 MCP 工作区工具，当前版本不接入普通对话。
                    </p>
                  </div>
                {!activeWorkspaceId ? (
                  <p className="text-[12px] text-danger">
                    未选择工作区，无法加载工具策略。
                  </p>
                ) : mcpLoading ? (
                  <p className="text-[12px] text-muted-foreground">加载工具策略…</p>
                ) : policy ? (
                  <div className="space-y-2">
                    {policy.tools.map((tool) => (
                      <div
                        key={tool.id}
                        className="flex items-center justify-between gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="font-mono text-[12px] text-ink">{tool.name}</div>
                          {tool.description ? (
                            <p className="text-[11px] text-muted-foreground">{tool.description}</p>
                          ) : null}
                        </div>
                        <button
                          type="button"
                          role="switch"
                          aria-checked={tool.enabled}
                          disabled={!RUNTIME_MCP_ENABLED}
                          onClick={() => {
                            if (!RUNTIME_MCP_ENABLED) return
                            setToolEnabled(tool.id, !tool.enabled)
                          }}
                          className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
                            !RUNTIME_MCP_ENABLED
                              ? "cursor-not-allowed bg-[rgba(0,0,0,0.12)] opacity-60"
                              : tool.enabled
                                ? "bg-success"
                                : "bg-[rgba(0,0,0,0.15)]"
                          }`}
                        >
                          <span
                            className={`absolute top-0.5 size-5 rounded-full bg-white shadow transition-transform ${
                              tool.enabled ? "left-[22px]" : "left-0.5"
                            }`}
                          />
                        </button>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-[12px] text-muted-foreground">暂无工具策略数据</p>
                )}

                <div className="flex flex-wrap gap-2">
                  <Button type="button" variant="outline" className="h-[36px] rounded-[12px] bg-white px-4" disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId || !policy} onClick={enableAll}>
                    全部启用
                  </Button>
                  <Button type="button" variant="outline" className="h-[36px] rounded-[12px] bg-white px-4" disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId || !policy} onClick={disableAll}>
                    全部关闭
                  </Button>
                  <Button type="button" variant="outline" className="h-[36px] rounded-[12px] bg-white px-4" disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId || !policy} onClick={resetToDefault}>
                    恢复默认
                  </Button>
                  {mcpDirty ? (
                    <Button
                      type="button"
                      variant="default"
                      className="h-[36px] rounded-[12px] bg-primary px-4 font-bold text-primary-foreground hover:bg-primary/90"
                      disabled={!RUNTIME_MCP_ENABLED || !activeWorkspaceId}
                      onClick={() => {
                        if (!activeWorkspaceId) return
                        void savePolicy(activeWorkspaceId)
                      }}
                    >
                      保存
                    </Button>
                  ) : null}
                </div>
                </section>
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
