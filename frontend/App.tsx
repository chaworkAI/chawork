import { useCallback, useEffect, useMemo, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import type { DreamReadyPayload } from "@/types/employee"
import type { Attachment } from "@/types/message"
import { useToastStore } from "@/stores/toast"
import {
  OnboardingTourOverlay,
  type OnboardingTourStep,
} from "@/components/onboarding/OnboardingTourOverlay"
import { WorkspaceSkillSetupDialog } from "@/components/onboarding/WorkspaceSkillSetupDialog"
import { AppShell } from "@/components/layout/AppShell"
import { HelpTip } from "@/components/layout/HelpTip"
import { TopBar } from "@/components/layout/TopBar"
import { ManagementDrawer } from "@/components/layout/ManagementDrawer"
import { ChatMain } from "@/components/chat/ChatMain"
import { ProjectMaterialsPanel } from "@/components/knowledge/ProjectMaterialsPanel"
import { EmployeePanel } from "@/components/employee/EmployeePanel"
import { EmployeeDreamSchedulePanel } from "@/components/employee/EmployeeDreamSchedulePanel"
import { GlobalSettingsPanel } from "@/components/settings/GlobalSettingsPanel"
import { WorkspaceConfigPanel } from "@/components/settings/WorkspaceConfigPanel"
import { ToastContainer } from "@/components/ui/toast"
import { RightRail } from "@/components/layout/RightRail"
import { SidebarIconRail } from "@/components/layout/SidebarIconRail"
import { HubGithubImportDialog } from "@/components/hub/HubGithubImportDialog"
import { HubMarketDialog } from "@/components/hub/HubMarketDialog"
import { SessionList } from "@/components/workspace/SessionList"
import { WorkspaceNav } from "@/components/workspace/WorkspaceNav"
import { EmployeeWorkspaceCascadeDialog } from "@/components/workspace/EmployeeWorkspaceCascadeDialog"
import { CreateEmployeeDialog } from "@/components/employee/CreateEmployeeDialog"
import { useChatStore } from "@/stores/chat"
import { useUiLabel } from "@/hooks/useUiLabel"
import { useRuntimeStore } from "@/stores/runtime"
import { useSessionStore } from "@/stores/session"
import { useProviderStore, selectProviderCanSend, selectProviderBlockedReason } from "@/stores/provider"
import { useRootConfigStore } from "@/stores/rootConfig"
import { useWorkspaceConfigStore } from "@/stores/workspaceConfig"
import { useWorkspaceStore } from "@/stores/workspace"
import { useEmployeeStore } from "@/stores/employee"
import { useUiStore } from "@/stores/ui"
import { useCodexEvents } from "@/hooks/useCodexEvents"
import { useRuntimeLifecycleEvents } from "@/hooks/useRuntimeLifecycleEvents"
import { useWorkspace } from "@/hooks/useWorkspace"
import { getUiPreferences, setUiPreferences } from "@/lib/tauri"
import { getRuntimePromptKind } from "@/lib/runtimePromptFormat"
import { OTAManager } from "@/lib/ota"
import { invoke } from "@tauri-apps/api/core"

export function App() {
  useCodexEvents()
  useRuntimeLifecycleEvents()
  useWorkspace()
  const [composerDraft, setComposerDraft] = useState("")
  const [composerAttachments, setComposerAttachments] = useState<Attachment[]>([])
  const [workspaceCascadeOpen, setWorkspaceCascadeOpen] = useState(false)
  const [workspaceCascadePreferredEmployeeId, setWorkspaceCascadePreferredEmployeeId] =
    useState<string | null>(null)
  const [onboardingTourOpen, setOnboardingTourOpen] = useState(false)
  const [generalBindTourAction, setGeneralBindTourAction] = useState<(() => void) | null>(null)
  const pendingComposerPrefill = useChatStore((s) => s.pendingComposerPrefill)
  const takePendingComposerPrefill = useChatStore((s) => s.takePendingComposerPrefill)
  const workspaceSidebarItems = useWorkspaceStore((s) => s.workspaceSidebarItems)
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId)
  const workspaceConfigOpen = useWorkspaceStore((s) => s.workspaceConfigOpen)
  const setWorkspaceConfigOpen = useWorkspaceStore((s) => s.setWorkspaceConfigOpen)
  const workspaceLoading = useWorkspaceStore((s) => s.isLoading)
  const activeBinding = useWorkspaceStore((s) => s.activeBinding)
  const bindingLoading = useWorkspaceStore((s) => s.bindingLoading)
  const switchWorkspace = useWorkspaceStore((s) => s.switchWorkspace)
  const openWorkspaceDialog = useWorkspaceStore((s) => s.openWorkspaceDialog)

  const openEmployeePanel = useEmployeeStore((s) => s.openPanel)
  const employeePanelOpen = useEmployeeStore((s) => s.panelOpen)
  const openDreamConfigPanel = useEmployeeStore((s) => s.openDreamConfigPanel)
  const selectEmployee = useEmployeeStore((s) => s.selectEmployee)
  const setEmployeeActiveTab = useEmployeeStore((s) => s.setActiveTab)

  const sessions = useSessionStore((s) => s.sessions)
  const activeSessionId = useSessionStore((s) => s.activeSessionId)
  const sessionLoading = useSessionStore((s) => s.isLoading)
  const switchSession = useSessionStore((s) => s.switchSession)
  const createSession = useSessionStore((s) => s.createSession)
  const renameSession = useSessionStore((s) => s.renameSession)
  const deleteSession = useSessionStore((s) => s.deleteSession)

  const messages = useChatStore((s) => s.messages)
  const sendChatMessage = useChatStore((s) => s.sendMessage)
  const isStreaming = useChatStore((s) => s.isStreaming)
  const cancelActiveTurn = useChatStore((s) => s.cancelActiveTurn)

  const activeRuntime = useRuntimeStore((s) =>
    activeWorkspaceId ? s.runtimeByWorkspace[activeWorkspaceId] : null,
  )
  const runtimeEvents = activeRuntime?.events ?? []
  const reviews = activeRuntime?.reviews ?? []
  const acceptReview = useRuntimeStore((s) => s.acceptReview)
  const acceptReviewForSession = useRuntimeStore((s) => s.acceptReviewForSession)
  const rejectReview = useRuntimeStore((s) => s.rejectReview)
  const answerUserInput = useRuntimeStore((s) => s.answerUserInput)
  const answerMcpElicitation = useRuntimeStore((s) => s.answerMcpElicitation)
  const isWorkspaceBusy = useRuntimeStore((s) => s.isWorkspaceBusy)
  const clearLifecycleNotice = useRuntimeStore((s) => s.clearLifecycleNotice)
  const workspaceRuntimeStatus = activeRuntime?.status ?? "idle"
  const workspaceRuntimeBusy = isWorkspaceBusy(activeWorkspaceId)

  const setProjectMaterialsOpen = useUiStore((s) => s.setProjectMaterialsOpen)
  const setSidebarCollapsed = useUiStore((s) => s.setSidebarCollapsed)
  const getLabel = useUiLabel()
  const openWorkspaceConfig = useWorkspaceStore((s) => s.openWorkspaceConfig)

  const globalProvider = useProviderStore((s) => s.globalProvider)
  const providerEffective = useProviderStore((s) => s.effective)
  const providerSendAllowed = useProviderStore(selectProviderCanSend)
  const providerBlockedReasonText = useProviderStore(selectProviderBlockedReason)

  const effectiveProviderForHeader = useMemo((): import("@/lib/tauri").EffectiveProviderPayload | null => {
    const configured = providerSendAllowed
    const model =
      providerEffective?.effective_provider?.model ??
      globalProvider?.model ??
      ""
    if (!configured && !model && !providerBlockedReasonText) return null
    return {
      configured,
      origin: configured ? "inherit_global" : "none",
      model,
      error_kind: configured ? null : "global_not_configured",
      error_message: providerBlockedReasonText,
    }
  }, [globalProvider, providerEffective, providerSendAllowed, providerBlockedReasonText])
  const loadWorkspaceConfig = useWorkspaceConfigStore((s) => s.load)
  const loadGlobalProvider = useProviderStore((s) => s.loadGlobalProvider)
  const loadEffectiveProvider = useProviderStore((s) => s.loadEffectiveProvider)
  const loadHttpServerPort = useRootConfigStore((s) => s.loadHttpServerPort)
  const loadRootInfo = useRootConfigStore((s) => s.loadRootInfo)
  const settingsPanelOpen = useRootConfigStore((s) => s.settingsPanelOpen)
  const openSettingsPanel = useRootConfigStore((s) => s.openSettingsPanel)

  const employeeBindingBlocked = useMemo(() => {
    if (!activeWorkspaceId) return false
    if (bindingLoading) return true
    if (!activeBinding) return true
    return activeBinding.status !== "bound"
  }, [activeWorkspaceId, bindingLoading, activeBinding])

  const composerSendBlocked = useMemo(() => {
    if (workspaceLoading || sessionLoading) return true
    if (!activeWorkspaceId) return true
    if (employeeBindingBlocked) return true
    if (!activeSessionId) return true
    if (isStreaming || workspaceRuntimeBusy) return true
    if (!providerSendAllowed) return true
    return false
  }, [
    workspaceLoading,
    sessionLoading,
    activeWorkspaceId,
    employeeBindingBlocked,
    activeSessionId,
    isStreaming,
    workspaceRuntimeBusy,
    providerSendAllowed,
  ])

  const composerSendBlockedReason = useMemo(() => {
    if (workspaceLoading || sessionLoading) {
      return getLabel(
        "composer.send_blocked_loading",
        "正在同步工作区或会话，请稍候",
      )
    }
    if (!activeWorkspaceId) {
      return getLabel(
        "composer.send_blocked_no_workspace",
        "请先打开或选择一个工作区",
      )
    }
    if (bindingLoading) {
      return getLabel(
        "composer.send_blocked_binding_check",
        "正在检查员工绑定状态，请稍候",
      )
    }
    if (activeBinding && activeBinding.status !== "bound") {
      return (
        activeBinding.message ||
        getLabel(
          "composer.send_blocked_no_employee",
          "此工作区尚未绑定员工，请先在下方完成绑定后再发送消息",
        )
      )
    }
    if (!activeSessionId) {
      return getLabel(
        "composer.send_blocked_no_session",
        "请在左侧「新建会话」或点选一条会话后再发送",
      )
    }
    if (isStreaming || workspaceRuntimeBusy) {
      if (workspaceRuntimeBusy && !isStreaming) {
        return getLabel(
          "composer.send_blocked_workspace_runtime_busy",
          "当前工作区已有会话正在运行，请等待完成后再发送",
        )
      }
      return getLabel(
        "composer.send_blocked_streaming",
        "正在等待 AI 回复，请等待完成或点击停止",
      )
    }
    const reason = providerBlockedReasonText
    if (reason) {
      return getLabel(
        "composer.send_blocked_provider",
        "还没配置 AI 模型，完成后就可以开始聊天啦",
      )
    }
    return undefined
  }, [
    workspaceLoading,
    sessionLoading,
    activeWorkspaceId,
    bindingLoading,
    activeBinding,
    activeSessionId,
    isStreaming,
    workspaceRuntimeBusy,
    providerBlockedReasonText,
    getLabel,
  ])

  const composerSendBlockedAction = useMemo(() => {
    if (workspaceLoading || sessionLoading) return undefined
    if (!activeWorkspaceId) {
      return {
        label: getLabel("composer.send_blocked_open_workspace", "打开工作区"),
        onClick: () => {
          setWorkspaceCascadePreferredEmployeeId("general")
          setWorkspaceCascadeOpen(true)
        },
      }
    }
    if (providerSendAllowed) return undefined
    if (bindingLoading || employeeBindingBlocked) return undefined
    if (!activeSessionId) return undefined
    if (isStreaming || workspaceRuntimeBusy) return undefined
    if (!providerBlockedReasonText) return undefined
    return {
      label: getLabel("composer.send_blocked_configure", "去配置"),
      onClick: () => openSettingsPanel("provider"),
    }
  }, [
    workspaceLoading,
    sessionLoading,
    activeWorkspaceId,
    providerSendAllowed,
    bindingLoading,
    employeeBindingBlocked,
    activeSessionId,
    isStreaming,
    workspaceRuntimeBusy,
    providerBlockedReasonText,
    getLabel,
    openSettingsPanel,
  ])

  useEffect(() => {
    void loadRootInfo()
    void loadGlobalProvider()
    void loadHttpServerPort()

    // OTA 初始化：标记健康 + 启动轮询检查
    void invoke("ota_mark_healthy").catch(() => {})
    const otaManager = new OTAManager("0.1.0", {
      serverUrl: "https://api.chawork.com",
      channel: "stable",
      deviceId: crypto.randomUUID(),
    })
    otaManager.onProgress((progress) => {
      if (progress.status === "ready" && progress.updateInfo) {
        const info = progress.updateInfo
        if (info.force_update) {
          otaManager.performUpdate(info)
        }
      }
    })
    otaManager.startPolling()
    return () => otaManager.stopPolling()
  }, [loadRootInfo, loadGlobalProvider, loadHttpServerPort])

  useEffect(() => {
    let cancelled = false
    void getUiPreferences()
      .then((preferences) => {
        if (!cancelled && !preferences.onboarding_tour_completed) {
          setOnboardingTourOpen(true)
        }
      })
      .catch(() => {
        if (!cancelled) setOnboardingTourOpen(true)
      })
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    const openTour = () => setOnboardingTourOpen(true)
    window.addEventListener("chawork:open-onboarding-tour", openTour)
    return () => {
      window.removeEventListener("chawork:open-onboarding-tour", openTour)
    }
  }, [])

  useEffect(() => {
    if (!activeWorkspaceId) return
    void loadWorkspaceConfig()
    void loadEffectiveProvider()
  }, [activeWorkspaceId, loadWorkspaceConfig, loadEffectiveProvider])

  useEffect(() => {
    if (!pendingComposerPrefill) return
    setComposerDraft(takePendingComposerPrefill() ?? "")
  }, [pendingComposerPrefill, takePendingComposerPrefill])

  const handleUseGeneralWorkspaceGuide = useCallback(() => {
    setWorkspaceCascadePreferredEmployeeId("general")
    setWorkspaceCascadeOpen(true)
  }, [])

  useEffect(() => {
    const unlisten = listen<DreamReadyPayload>("dream-run-ready", (event) => {
      const { employee_name, employee_id, selected_session_count } = event.payload
      const label = employee_name || employee_id
      useToastStore.getState().show(
        `Dream 已为「${label}」准备就绪（${selected_session_count} 条会话），请前往员工 Dream 面板查看`,
        "info",
      )
      void useEmployeeStore.getState().refreshPendingReviewBadges()
    })
    return () => {
      void unlisten.then((fn) => fn())
    }
  }, [])

  const activeWorkspaceSidebar = useMemo(
    () => workspaceSidebarItems.find((w) => w.workspace.id === activeWorkspaceId),
    [workspaceSidebarItems, activeWorkspaceId],
  )
  const activeSession = sessions.find((s) => s.id === activeSessionId)
  const currentEmployeeId =
    activeBinding?.status === "bound" ? activeBinding.employee_id : null
  const shouldShowRuntimeNotice =
    Boolean(activeRuntime?.lifecycleMessage) &&
    Boolean(activeSession && activeSession.message_count > 0)

  const onboardingTourSteps = useMemo<OnboardingTourStep[]>(() => {
    if (settingsPanelOpen || workspaceCascadeOpen || employeePanelOpen) {
      return []
    }
    const steps: OnboardingTourStep[] = [
      {
        id: "provider",
        targetId: "settings-entry",
        title: getLabel("onboarding.tour.provider.title", "先配置 AI 模型"),
        body: getLabel(
          "onboarding.tour.provider.body",
          "ChaWork 需要一个可用的模型 Provider 才能对话。先从设置里填写 Base URL、API Key 并选择模型。",
        ),
        actionLabel: getLabel("onboarding.tour.provider.action", "去配置"),
        onAction: () => openSettingsPanel("provider"),
      },
      {
        id: "workspace",
        targetId: "workspace-entry",
        title: getLabel("onboarding.tour.workspace.title", "先打开项目文件夹"),
        body: getLabel(
          "onboarding.tour.workspace.body",
          "ChaWork 围绕工作区协作。第一次打开文件夹时，会默认帮你使用通用员工。",
        ),
        actionLabel: getLabel("onboarding.tour.workspace.action", "打开工作区"),
        onAction: handleUseGeneralWorkspaceGuide,
      },
      {
        id: "binding",
        targetId: "binding-general",
        title: getLabel("onboarding.tour.binding.title", "绑定通用员工"),
        body: getLabel(
          "onboarding.tour.binding.body",
          "新手可以直接使用通用员工。绑定后，这个工作区就能使用员工 prompt 和 skills。",
        ),
        actionLabel: generalBindTourAction
          ? getLabel("onboarding.binding.use_general", "使用通用员工")
          : undefined,
        onAction: generalBindTourAction ?? undefined,
      },
      {
        id: "session",
        targetId: "session-list",
        title: getLabel("onboarding.tour.session.title", "会话保存在工作区里"),
        body: getLabel(
          "onboarding.tour.session.body",
          "同一个工作区可以有多条会话。每条会话共享项目资料，但聊天记录彼此独立。",
        ),
      },
      {
        id: "dream",
        targetId: "employee-entry",
        title: getLabel("onboarding.tour.dream.title", "认识 Dream 学习闭环"),
        body: getLabel(
          "onboarding.tour.dream.body",
          "完成几次真实任务后，可在员工的 Dream 标签页触发分析。Dream 会提议 prompt 更新，你在 Review Queue 中批准后才生效。",
        ),
        actionLabel: getLabel("onboarding.tour.dream.action", "打开 Dream"),
        onAction: () => void openDreamConfigPanel(),
      },
    ]
    return steps
  }, [
    employeePanelOpen,
    getLabel,
    generalBindTourAction,
    handleUseGeneralWorkspaceGuide,
    openDreamConfigPanel,
    openSettingsPanel,
    settingsPanelOpen,
    workspaceCascadeOpen,
  ])

  const sessionTitle =
    activeSession?.title ??
    getLabel("chat.session_default_title", "新会话")

  const pendingPrompts = useMemo(() => {
    const kindOrder = { permissions: 0, approval: 1, user_input: 2, mcp: 3 } as const
    return reviews
      .filter(
        (r) =>
          (r.status === "pending" || r.status === "applying" || r.status === "error") &&
          (r.owner?.sessionId === null || r.owner?.sessionId === activeSessionId) &&
          (r.runtime_approval ||
            r.runtime_permissions ||
            r.user_input_request ||
            r.mcp_elicitation),
      )
      .sort(
        (a, b) =>
          kindOrder[getRuntimePromptKind(a)] - kindOrder[getRuntimePromptKind(b)],
      )
  }, [reviews, activeSessionId])

  const handlePromptPositive = useCallback(
    (id: string, payload?: unknown) => {
      const entry = reviews.find((r) => r.id === id)
      if (entry?.user_input_request) {
        void answerUserInput(
          id,
          (payload ?? {}) as Record<string, { answers: string[] }>,
        )
        return
      }
      if (entry?.mcp_elicitation) {
        void answerMcpElicitation(id, payload)
        return
      }
      void acceptReviewForSession(id)
    },
    [reviews, answerUserInput, answerMcpElicitation, acceptReviewForSession],
  )

  const handlePromptMiddle = useCallback(
    (id: string) => {
      void acceptReview(id)
    },
    [acceptReview],
  )

  const messageEmptyHint = useMemo(() => {
    if (messages.length > 0) return undefined
    if (workspaceLoading || sessionLoading) {
      return getLabel(
        "composer.send_blocked_loading",
        "正在同步工作区或会话，请稍候",
      )
    }
    if (!activeWorkspaceId) {
      return getLabel(
        "chat.empty.pick_workspace",
        "当前还没有打开工作区。先用「通用员工」打开一个项目文件夹，就可以开始对话。",
      )
    }
    if (activeBinding && activeBinding.status !== "bound") {
      return getLabel(
        "chat.empty.bind_employee",
        "此工作区需要先绑定员工。请在下方选择已有员工或创建新员工，绑定后才能开始对话。",
      )
    }
    if (!activeSessionId) {
      return getLabel(
        "chat.empty.new_session",
        "已打开工作区，但还没有可用会话。请点击左侧「会话」旁的 ＋ 新建一条会话，然后再发送消息。",
      )
    }
    return undefined
  }, [
    messages.length,
    workspaceLoading,
    sessionLoading,
    activeWorkspaceId,
    activeBinding,
    activeSessionId,
    getLabel,
  ])

  const messageEmptyPrimaryAction = useMemo(() => {
    if (messages.length > 0) return undefined
    if (workspaceLoading || sessionLoading) return undefined
    if (!activeWorkspaceId) {
      return {
        label: getLabel("chat.empty.open_workspace", "用通用员工打开文件夹…"),
        onClick: () => {
          setWorkspaceCascadePreferredEmployeeId("general")
          setWorkspaceCascadeOpen(true)
        },
      }
    }
    if (!activeSessionId) {
      return {
        label: getLabel("chat.empty.create_session", "新建会话"),
        onClick: () => {
          void createSession()
        },
      }
    }
    return undefined
  }, [
    messages.length,
    workspaceLoading,
    sessionLoading,
    activeWorkspaceId,
    activeSessionId,
    getLabel,
    createSession,
    openWorkspaceDialog,
  ])

  const handleWorkspaceSelect = useCallback(
    (path: string) => {
      if (!path) return
      void switchWorkspace(path)
    },
    [switchWorkspace],
  )

  const handleOpenWorkspaceCascade = useCallback(() => {
    setWorkspaceCascadePreferredEmployeeId("general")
    setWorkspaceCascadeOpen(true)
  }, [])

  const handleGeneralBindActionReady = useCallback((action: (() => void) | null) => {
    setGeneralBindTourAction(() => action)
  }, [])

  const handleWorkspaceCascadeOpenChange = useCallback((open: boolean) => {
    setWorkspaceCascadeOpen(open)
    if (!open) {
      setWorkspaceCascadePreferredEmployeeId(null)
    }
  }, [])

  const handleCascadeAddWorkspace = useCallback(
    async (employeeId: string) => {
      await openWorkspaceDialog(employeeId, {
        activate: !activeWorkspaceId,
      })
    },
    [openWorkspaceDialog, activeWorkspaceId],
  )

  const handleComposerSend = useCallback(() => {
    const text = composerDraft.trim()
    if ((!text && composerAttachments.length === 0) || !activeSessionId) return
    void sendChatMessage(text, composerAttachments)
    setComposerDraft("")
    setComposerAttachments([])
  }, [activeSessionId, composerAttachments, composerDraft, sendChatMessage])

  const handleCurrentEmployeeOpen = useCallback(() => {
    if (currentEmployeeId) {
      setEmployeeActiveTab("overview")
      void selectEmployee(currentEmployeeId)
    }
    openEmployeePanel("detail")
  }, [currentEmployeeId, openEmployeePanel, selectEmployee, setEmployeeActiveTab])

  return (
    <div className="relative h-screen min-h-[640px] min-w-[980px] overflow-hidden bg-shell-bg font-sans text-ink">
      <ProjectMaterialsPanel />
      <GlobalSettingsPanel />
      <WorkspaceConfigPanel
        open={workspaceConfigOpen}
        onClose={() => setWorkspaceConfigOpen(false)}
      />
      <EmployeePanel />
      <EmployeeDreamSchedulePanel />
      <ManagementDrawer />
      <EmployeeWorkspaceCascadeDialog
        open={workspaceCascadeOpen}
        preferredEmployeeId={
          workspaceCascadePreferredEmployeeId ??
          (activeBinding?.status === "bound" ? activeBinding.employee_id : null)
        }
        allWorkspaceItems={workspaceSidebarItems}
        activeWorkspaceId={activeWorkspaceId}
        onOpenChange={handleWorkspaceCascadeOpenChange}
        onWorkspaceSelect={handleWorkspaceSelect}
        onAddWorkspace={handleCascadeAddWorkspace}
      />
      <CreateEmployeeDialog />
      {!settingsPanelOpen && <ToastContainer />}

      <WorkspaceSkillSetupDialog />
      <HubMarketDialog />
      <HubGithubImportDialog />
      <OnboardingTourOverlay
        open={onboardingTourOpen && onboardingTourSteps.length > 0}
        steps={onboardingTourSteps}
        onOpenChange={(open) => {
          setOnboardingTourOpen(open)
          if (!open) {
            void setUiPreferences({ onboarding_tour_completed: true })
          }
        }}
      />

      <main
        className="relative z-10 grid h-full grid-rows-[44px_minmax(0,1fr)] overflow-hidden bg-shell-surface"
        aria-label="ChaWork workbench"
      >
        <TopBar />

        <AppShell
          sidebarRail={
            <SidebarIconRail
              onNewSession={() => void createSession()}
              onOpenWorkspace={handleOpenWorkspaceCascade}
              onOpenWorkspaceConfig={() => openWorkspaceConfig("overview")}
            />
          }
          sidebar={
            <div className="flex min-h-0 flex-1 flex-col max-[899px]:hidden">
              <div className="mx-1.5 mb-2.5 mt-1 flex items-center justify-between gap-2 text-[13px] font-bold text-muted-foreground">
                <span className="inline-flex items-center gap-2">
                  {getLabel("sidebar.workspace_section", "工作区")}
                  <HelpTip
                    variant="bottom"
                    tip={getLabel(
                      "sidebar.workspace_section_tip",
                      "这里列出已添加的工作区文件夹。切换工作区即可使用对应项目资料与绑定员工的 Prompt/Skills。",
                    )}
                  />
                </span>
                <button
                  type="button"
                  className="grid size-[26px] place-items-center rounded-full border border-line bg-white text-[16px] font-extrabold leading-none text-muted-foreground hover:bg-[#f6f7f9] hover:text-ink"
                  aria-label="折叠工作区栏"
                  onClick={() => setSidebarCollapsed(true)}
                >
                  ‹
                </button>
              </div>
              <WorkspaceNav
                items={workspaceSidebarItems}
                activeWorkspaceId={activeWorkspaceId}
                boundEmployeeName={
                  activeBinding?.status === "bound"
                    ? activeBinding.employee_name
                    : activeWorkspaceSidebar?.workspace.bound_employee_name ?? null
                }
                onOpenCascade={handleOpenWorkspaceCascade}
              />
              <SessionList
                sessions={sessions}
                activeSessionId={activeSessionId}
                onSessionSelect={(id) => void switchSession(id)}
                onNewSession={() => void createSession()}
                onRenameSession={(id, title) => renameSession(id, title)}
                onDeleteSession={(id) => void deleteSession(id)}
              />
            </div>
          }
          main={
            <ChatMain
              header={{
                workspaceName: activeWorkspaceSidebar?.workspace.name,
                workspacePath: activeWorkspaceSidebar?.workspace.path,
                sessionTitle,
                effectiveProvider: effectiveProviderForHeader,
                indexStatus: activeWorkspaceSidebar?.workspace.index_status,
                onIndexClick: () => setProjectMaterialsOpen(true),
                onProjectMaterialsClick: () => setProjectMaterialsOpen(true),
                onWorkspaceConfigClick: () => openWorkspaceConfig("provider"),
                boundEmployeeName:
                  activeBinding?.status === "bound"
                    ? activeBinding.employee_name
                    : activeWorkspaceSidebar?.workspace.bound_employee_name ??
                      null,
                onEmployeeClick: handleCurrentEmployeeOpen,
              }}
              messages={messages}
              composerValue={composerDraft}
              composerAttachments={composerAttachments}
              onComposerChange={setComposerDraft}
              onComposerAttachmentsChange={setComposerAttachments}
              onSend={handleComposerSend}
              composerSendBlocked={composerSendBlocked}
              composerSendBlockedReason={composerSendBlockedReason}
              composerSendBlockedAction={composerSendBlockedAction}
              runtimeNotice={
                shouldShowRuntimeNotice && activeRuntime?.lifecycleMessage
                  ? {
                      message: activeRuntime.lifecycleMessage,
                      actionLabel: getLabel(
                        "composer.runtime_notice_new_session",
                        "新建会话",
                      ),
                      onAction: () => void createSession(),
                      onDismiss: () => clearLifecycleNotice(activeWorkspaceId),
                    }
                  : undefined
              }
              workspacePath={activeWorkspaceSidebar?.workspace.path ?? null}
              workspaceBinding={activeBinding}
              workspaceBindingLoading={bindingLoading}
              onGeneralBindActionReady={handleGeneralBindActionReady}
              messageEmptyHint={messageEmptyHint}
              messageEmptyPrimaryAction={messageEmptyPrimaryAction}
              isStreaming={isStreaming}
              onCancelTurn={() => void cancelActiveTurn()}
              onProjectMaterialsClick={() => setProjectMaterialsOpen(true)}
              prompts={pendingPrompts}
              onPromptNegative={(id) => void rejectReview(id)}
              onPromptMiddle={handlePromptMiddle}
              onPromptPositive={handlePromptPositive}
            />
          }
          rightRail={
            <RightRail
              status={workspaceRuntimeStatus}
              runtime={{
                events: runtimeEvents,
                statusLabel:
                  workspaceRuntimeStatus !== "idle"
                    ? getLabel("runtime.status_running", "运行中…")
                    : getLabel("runtime.status_idle", "就绪 · 暂无运行事件"),
              }}
            />
          }
        />
      </main>
    </div>
  )
}
