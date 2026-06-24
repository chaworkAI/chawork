import { useCallback, useEffect, useRef, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { ChevronDown, Eye, EyeOff, Loader2, Clock, X, RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ToastContainer } from "@/components/ui/toast"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import * as ipc from "@/lib/tauri"
import { useEmployeeStore } from "@/stores/employee"
import { useHubStore } from "@/stores/hub"
import { useLocaleStore } from "@/stores/locale"
import { useProviderStore } from "@/stores/provider"
import { useRootConfigStore } from "@/stores/rootConfig"
import { useSkillStore } from "@/stores/skill"
import { useUiStore } from "@/stores/ui"
import { LOCALE_OPTIONS, type AppLocale } from "@/types/locale"
import { checkForUpdate } from "@/lib/ota/checker"

const TAB_KEYS = [
  { key: "general", labelKey: "global_settings.tab.general" },
  { key: "root_runtime", labelKey: "global_settings.tab.root_runtime" },
  { key: "provider", labelKey: "global_settings.tab.provider" },
  { key: "dream", labelKey: "global_settings.tab.dream" },
  { key: "global_skills", labelKey: "global_settings.tab.global_skills" },
  { key: "templates", labelKey: "global_settings.tab.templates" },
] as const

type TabKey = (typeof TAB_KEYS)[number]["key"]

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[13px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3 dark:bg-panel-soft">
      <span className="text-[11px] font-extrabold uppercase text-[var(--subtle)]">
        {label}
      </span>
      <p className="mt-0.5 break-all font-mono text-[12px] text-ink">{value}</p>
    </div>
  )
}

export function GlobalSettingsPanel() {
  const label = useUiLabel()
  const locale = useLocaleStore((s) => s.locale)
  const setLocale = useLocaleStore((s) => s.setLocale)
  const theme = useUiStore((s) => s.theme)
  const setTheme = useUiStore((s) => s.setTheme)

  const settingsPanelOpen = useRootConfigStore((s) => s.settingsPanelOpen)
  const settingsActiveTab = useRootConfigStore((s) => s.settingsActiveTab)
  const closeSettingsPanel = useRootConfigStore((s) => s.closeSettingsPanel)
  const rootInfo = useRootConfigStore((s) => s.rootInfo)
  const loadRootInfo = useRootConfigStore((s) => s.loadRootInfo)

  const globalForm = useProviderStore((s) => s.globalForm)
  const globalProvider = useProviderStore((s) => s.globalProvider)
  const globalProviderLoading = useProviderStore((s) => s.globalProviderLoading)
  const globalProviderError = useProviderStore((s) => s.globalProviderError)
  const providerModels = useProviderStore((s) => s.providerModels)
  const providerModelsLoading = useProviderStore((s) => s.providerModelsLoading)
  const providerModelsError = useProviderStore((s) => s.providerModelsError)
  const providerModelsMessage = useProviderStore((s) => s.providerModelsMessage)
  const updateGlobalForm = useProviderStore((s) => s.updateGlobalForm)
  const loadGlobalProvider = useProviderStore((s) => s.loadGlobalProvider)
  const saveGlobalProvider = useProviderStore((s) => s.saveGlobalProvider)
  const loadProviderModels = useProviderStore((s) => s.loadProviderModels)
  const resetProviderModels = useProviderStore((s) => s.resetProviderModels)

  const rootCatalog = useSkillStore((s) => s.rootCatalog)
  const loadSkills = useSkillStore((s) => s.loadSkills)
  const skillsLoading = useSkillStore((s) => s.loading)
  const openHubMarket = useHubStore((s) => s.openMarket)

  const dreamDefaults = useEmployeeStore((s) => s.dreamDefaults)
  const loadDreamDefaults = useEmployeeStore((s) => s.loadDreamDefaults)
  const updateDreamDefaults = useEmployeeStore((s) => s.updateDreamDefaults)

  const [runtimeStatus, setRuntimeStatus] = useState("unknown")
  const [runtimeChecking, setRuntimeChecking] = useState(false)
  const [updateChecking, setUpdateChecking] = useState(false)
  const [dreamTimeInput, setDreamTimeInput] = useState("")
  const [dreamTimeSaving, setDreamTimeSaving] = useState(false)
  const [apiKeyVisible, setApiKeyVisible] = useState(false)
  const providerConnectionKeyRef = useRef<string | null>(null)

  const activeTab = (TAB_KEYS.some((t) => t.key === settingsActiveTab)
    ? settingsActiveTab
    : "general") as TabKey

  useEffect(() => {
    if (!settingsPanelOpen) return
    void loadRootInfo()
    void loadGlobalProvider()
    void loadSkills()
    void loadDreamDefaults()
    void ipc.getRuntimeStatus().then(setRuntimeStatus).catch(() => {})
  }, [
    settingsPanelOpen,
    loadRootInfo,
    loadGlobalProvider,
    loadSkills,
    loadDreamDefaults,
  ])

  useEffect(() => {
    if (dreamDefaults) {
      setDreamTimeInput(dreamDefaults.default_dream_time)
    }
  }, [dreamDefaults])

  useEffect(() => {
    if (!settingsPanelOpen || activeTab !== "provider") return

    const hasBaseUrl = Boolean(globalForm.openai_base_url.trim())
    const hasApiKey = Boolean(
      globalForm.openai_api_key.trim() || globalProvider?.openai_api_key_masked,
    )
    if (!hasBaseUrl || !hasApiKey) {
      providerConnectionKeyRef.current = null
      resetProviderModels()
      return
    }

    const connectionKey = `${globalForm.openai_base_url.trim()}\n${globalForm.openai_api_key.trim() || globalProvider?.openai_api_key_masked || ""}`
    if (
      providerConnectionKeyRef.current != null &&
      providerConnectionKeyRef.current !== connectionKey
    ) {
      updateGlobalForm({ model: "" })
      resetProviderModels()
    }
    providerConnectionKeyRef.current = connectionKey

    const timer = window.setTimeout(() => {
      void loadProviderModels(globalForm).then((result) => {
        if (!result) return
        const currentModel = useProviderStore.getState().globalForm.model.trim()
        if (currentModel && !result.models.includes(currentModel)) {
          updateGlobalForm({ model: "" })
        }
      })
    }, 350)

    return () => window.clearTimeout(timer)
  }, [
    activeTab,
    globalForm.openai_base_url,
    globalForm.openai_api_key,
    globalProvider?.openai_api_key_masked,
    loadProviderModels,
    resetProviderModels,
    settingsPanelOpen,
    updateGlobalForm,
  ])

  const checkRuntime = useCallback(async () => {
    setRuntimeChecking(true)
    try {
      const status = await ipc.getRuntimeStatus()
      setRuntimeStatus(status)
    } finally {
      setRuntimeChecking(false)
    }
  }, [])

  const handleRevealProvider = useCallback(() => {
    void ipc.revealGlobalProviderConfig()
  }, [])

  const handleReopenOnboardingTour = useCallback(() => {
    closeSettingsPanel()
    window.dispatchEvent(new Event("chawork:open-onboarding-tour"))
  }, [closeSettingsPanel])

  const handleCheckUpdate = useCallback(async () => {
    setUpdateChecking(true)
    try {
      const info = await checkForUpdate(
        { serverUrl: "https://api.chawork.com", pollInterval: 0, channel: "stable", deviceId: "manual-check" },
        "0.1.0",
      )
      if (!info) {
        alert("当前已是最新版本。")
      } else {
        const yes = confirm(`发现新版本 v${info.version}\n${info.release_notes ?? ""}\n\n是否立即更新？`)
        if (yes) {
          const { OTAManager } = await import("@/lib/ota")
          const mgr = new OTAManager("0.1.0", { serverUrl: "https://api.chawork.com", pollInterval: 0, channel: "stable", deviceId: "manual" })
          await mgr.performUpdate(info)
        }
      }
    } catch (err) {
      alert(`检查更新失败: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      setUpdateChecking(false)
    }
  }, [])

  const savedApiKeyHint = globalProvider?.openai_api_key_masked
    ? applyLabelTemplate(
        label(
          "global_settings.provider.api_key_saved_hint",
          "已保存 {{masked}}，留空表示不修改",
        ),
        { masked: globalProvider.openai_api_key_masked },
      )
    : label(
        "global_settings.provider.api_key_empty_hint",
        "留空表示不在 provider.json 中保存密钥",
      )

  const modalMeta: Record<TabKey, { kicker: string; title: string; description: string }> = {
    general: {
      kicker: label("global_settings.kicker.settings", "设置"),
      title: label("global_settings.basic_title", "基础设置"),
      description: label("global_settings.basic_description", "配置界面语言。"),
    },
    provider: {
      kicker: label("global_settings.kicker.settings", "设置"),
      title: label("global_settings.basic_title", "基础设置"),
      description: label(
        "global_settings.provider.note",
        "说明：目前仅支持 OpenAI Responses 格式。",
      ),
    },
    root_runtime: {
      kicker: label("global_settings.kicker.runtime", "Root Runtime"),
      title: label("global_settings.root_runtime.title", "根工作区 Runtime"),
      description: label("global_settings.root_runtime.description", "检查根工作区与运行时路径。"),
    },
    dream: {
      kicker: label("global_settings.kicker.dream", "Dream"),
      title: label("global_settings.dream.title", "Dream 全局配置"),
      description: label("global_settings.dream.description", "设置员工 Dream 的默认触发时间。"),
    },
    global_skills: {
      kicker: label("global_settings.kicker.skills", "Skills"),
      title: label("global_settings.skills.title", "全局 Skill 目录"),
      description: label("global_settings.skills.description", "查看根工作区中的全局技能。"),
    },
    templates: {
      kicker: label("global_settings.kicker.templates", "Templates"),
      title: label("global_settings.templates.title", "模板管理"),
      description: label("global_settings.templates.description", "管理全局模板。"),
    },
  }
  const meta = modalMeta[activeTab]
  const themeOptions = [
    { value: "light" as const, labelKey: "theme.option.light", fallback: "浅色" },
    { value: "dark" as const, labelKey: "theme.option.vscode_dark", fallback: "VS Code 深色" },
  ]

  return (
    <Dialog.Root open={settingsPanelOpen} onOpenChange={(open) => !open && closeSettingsPanel()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-80 bg-[rgba(36,40,50,0.28)] dark:bg-[rgba(0,0,0,0.48)]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-81 flex max-h-[min(720px,calc(100dvh-48px))] w-[min(620px,calc(100vw-80px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none dark:bg-panel dark:shadow-[0_24px_70px_rgba(0,0,0,0.42)]">
          <div className="flex shrink-0 items-start justify-between gap-4 border-b border-line-soft px-[22px] py-5">
            <div className="min-w-0">
              <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
                {meta.kicker}
              </p>
              <Dialog.Title className="mt-[3px] text-[21px] font-extrabold tracking-normal text-ink">
                {meta.title}
              </Dialog.Title>
            </div>
            <Dialog.Description className="sr-only">
              {label("global_settings.description", "配置全局 Provider、根工作区与 Dream 默认项")}
            </Dialog.Description>
            <div className="flex shrink-0 items-center gap-2">
              <div
                className="inline-flex min-h-[38px] items-center rounded-[13px] border border-line bg-[#f8f9fb] p-1 dark:bg-panel-soft"
                aria-label={label("theme.section.title", "外观")}
              >
                {themeOptions.map((option) => (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => setTheme(option.value)}
                    className={`min-h-[30px] rounded-[10px] px-2.5 text-[12px] font-bold transition-colors ${
                      theme === option.value
                        ? "bg-white text-ink shadow-[0_1px_0_rgba(34,41,54,0.08)] dark:bg-panel-strong"
                        : "text-muted-foreground hover:bg-white/70 hover:text-ink dark:hover:bg-panel-raised"
                    }`}
                  >
                    {label(option.labelKey, option.fallback)}
                  </button>
                ))}
              </div>
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="grid size-[38px] place-items-center rounded-[12px] border border-line bg-white text-muted-foreground transition-colors hover:bg-[#f8f9fb] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring dark:bg-panel-soft dark:hover:bg-panel-raised"
                  aria-label={label("global_settings.close", "关闭")}
                  onClick={closeSettingsPanel}
                >
                  <X className="size-4" />
                </button>
              </Dialog.Close>
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-auto px-[22px] py-[22px]">
            <p className="mb-4 text-[13px] text-muted-foreground">{meta.description}</p>
            {activeTab === "general" && (
              <div className="grid gap-3">
                <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5 dark:bg-panel-soft">
                  <h3 className="text-[14px] font-bold text-ink">
                    {label("locale.section.title", "语言")}
                  </h3>
                  <p className="text-[12px] text-muted-foreground">
                    {label(
                      "locale.section.description",
                      "切换界面显示语言。默认使用简体中文。",
                    )}
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {LOCALE_OPTIONS.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        onClick={() => setLocale(option.value as AppLocale)}
                        className={`rounded-[12px] border px-3 py-2 text-[12px] transition-colors ${
                          locale === option.value
                            ? "border-line-strong bg-white font-bold text-primary dark:bg-panel-strong"
                            : "border-line bg-white text-ink hover:bg-[#f8f9fb] dark:bg-panel dark:hover:bg-panel-raised"
                        }`}
                      >
                        {label(option.labelKey, option.value)}
                      </button>
                    ))}
                  </div>
                </section>
                <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5 dark:bg-panel-soft">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <h3 className="text-[14px] font-bold text-ink">
                        {label("onboarding.tour.settings_title", "新手引导")}
                      </h3>
                      <p className="mt-1 text-[12px] text-muted-foreground">
                        {label(
                          "onboarding.tour.settings_description",
                          "重新查看首次使用的工作区与员工绑定引导。",
                        )}
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-9 shrink-0 rounded-[12px] bg-white px-3 text-[12px]"
                      onClick={handleReopenOnboardingTour}
                    >
                      {label("onboarding.tour.reopen", "重新打开新手引导")}
                    </Button>
                  </div>
                </section>
                <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5 dark:bg-panel-soft">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <h3 className="text-[14px] font-bold text-ink">
                        {label("global_settings.update.title", "软件更新")}
                      </h3>
                      <p className="mt-1 text-[12px] text-muted-foreground">
                        {label(
                          "global_settings.update.description",
                          "检查是否有新版本可供下载安装。",
                        )}
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-9 shrink-0 rounded-[12px] bg-white px-3 text-[12px]"
                      disabled={updateChecking}
                      onClick={() => void handleCheckUpdate()}
                    >
                      {updateChecking ? (
                        <>
                          <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                          {label("global_settings.update.checking", "检查中…")}
                        </>
                      ) : (
                        <>
                          <RefreshCw className="mr-1.5 size-3.5" />
                          {label("global_settings.update.check", "检查更新")}
                        </>
                      )}
                    </Button>
                  </div>
                </section>
              </div>
            )}

            {activeTab === "root_runtime" && (
              <div className="space-y-4">
                <h3 className="text-[14px] font-bold text-ink">
                  {label("global_settings.root_runtime.title", "根工作区 Runtime")}
                </h3>
                {rootInfo ? (
                  <div className="grid gap-2">
                    <InfoRow
                      label={label("global_settings.field.path", "路径")}
                      value={rootInfo.path || "—"}
                    />
                    <InfoRow
                      label={label("global_settings.field.codex_home", "Codex Home")}
                      value={rootInfo.codex_home || "—"}
                    />
                  </div>
                ) : (
                  <p className="text-[12px] text-muted-foreground">
                    {label("global_settings.loading", "加载中…")}
                  </p>
                )}
                <div className="flex items-center gap-3">
                  <Button
                    type="button"
                    variant="outline"
                    className="h-[36px] rounded-[12px] bg-white px-4"
                    disabled={runtimeChecking}
                    onClick={() => void checkRuntime()}
                  >
                    {runtimeChecking ? (
                      <>
                        <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                        {label("global_settings.runtime.checking", "检查中…")}
                      </>
                    ) : (
                      label("global_settings.runtime.check", "检查 Runtime")
                    )}
                  </Button>
                  <span className="font-mono text-[12px] text-muted-foreground">{runtimeStatus}</span>
                </div>
              </div>
            )}

            {activeTab === "provider" && (
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <h3 className="text-[14px] font-bold text-ink">
                    {label("global_settings.provider.title", "全局模型配置")}
                  </h3>
                  <span
                    className={`rounded-full px-2 py-0.5 text-[11px] ${
                      globalProvider?.valid
                        ? "bg-success/15 text-success"
                        : "bg-danger/15 text-danger"
                    }`}
                  >
                    {globalProvider?.valid
                      ? label("global_settings.provider.configured", "已配置")
                      : label("global_settings.provider.not_configured", "未配置")}
                  </span>
                </div>

                {rootInfo?.provider_path ? (
                  <InfoRow
                    label={label("global_settings.provider.config_file", "全局配置文件")}
                    value={rootInfo.provider_path}
                  />
                ) : null}

                <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
                  <label className="block text-[13px] font-bold text-muted-foreground">
                    {label("global_settings.provider.base_url", "Base URL")}
                  </label>
                  <Input
                    value={globalForm.openai_base_url}
                    onChange={(e) => updateGlobalForm({ openai_base_url: e.target.value })}
                    className="min-h-[42px] rounded-[12px] border-line bg-white px-3 text-[13px]"
                    autoComplete="off"
                  />
                  <label className="block text-[13px] font-bold text-muted-foreground">
                    {label("global_settings.provider.api_key", "API Key")}
                  </label>
                  <div className="relative">
                    <Input
                      type={apiKeyVisible ? "text" : "password"}
                      value={globalForm.openai_api_key}
                      onChange={(e) => updateGlobalForm({ openai_api_key: e.target.value })}
                      placeholder={savedApiKeyHint}
                      className="min-h-[42px] rounded-[12px] border-line bg-white px-3 pr-11 text-[13px]"
                      autoComplete="off"
                      spellCheck={false}
                    />
                    <button
                      type="button"
                      className="absolute right-1.5 top-1/2 grid size-[34px] -translate-y-1/2 place-items-center rounded-[10px] text-muted-foreground transition-colors hover:bg-[#f8f9fb] hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      aria-label={
                        apiKeyVisible
                          ? label("global_settings.provider.hide_api_key", "隐藏 API Key")
                          : label("global_settings.provider.show_api_key", "显示 API Key")
                      }
                      aria-pressed={apiKeyVisible}
                      onClick={() => setApiKeyVisible((visible) => !visible)}
                    >
                      {apiKeyVisible ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
                    </button>
                  </div>
                  <p className="text-[11px] text-muted-foreground">{savedApiKeyHint}</p>
                  <label className="block text-[13px] font-bold text-muted-foreground">
                    {label("global_settings.provider.model", "模型")}
                  </label>
                  <div className="relative">
                    <select
                      value={globalForm.model}
                      onChange={(e) => updateGlobalForm({ model: e.target.value })}
                      disabled={providerModelsLoading || providerModels.length === 0}
                      className="h-[42px] w-full appearance-none rounded-[12px] border border-line bg-white px-3 pr-11 text-[13px] leading-[42px] text-ink outline-none transition-colors focus:border-ring focus:ring-2 focus:ring-ring/20 disabled:cursor-not-allowed disabled:opacity-60 dark:bg-panel"
                    >
                      <option value="">
                        {providerModelsLoading
                          ? label("global_settings.provider.models_loading", "正在获取模型列表…")
                          : providerModels.length > 0
                            ? label("global_settings.provider.model_placeholder", "选择模型")
                            : label(
                                "global_settings.provider.model_waiting",
                                "先填写 Base URL 和 API Key",
                              )}
                      </option>
                      {providerModels.map((model) => (
                        <option key={model} value={model}>
                          {model}
                        </option>
                      ))}
                    </select>
                    <ChevronDown className="pointer-events-none absolute right-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                  </div>
                  {providerModelsError ? (
                    <p className="text-[11px] text-danger">{providerModelsError}</p>
                  ) : providerModelsMessage ? (
                    <p className="text-[11px] text-muted-foreground">
                      {providerModelsMessage}
                    </p>
                  ) : null}
                </section>

                {globalProviderError ? (
                  <p className="text-[12px] text-danger">{globalProviderError}</p>
                ) : null}

                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="default"
                    className="h-[36px] rounded-[12px] bg-primary px-4 font-bold text-primary-foreground hover:bg-primary/90"
                    disabled={globalProviderLoading || !globalForm.model.trim()}
                    onClick={() => void saveGlobalProvider(globalForm)}
                  >
                    {globalProviderLoading
                      ? label("global_settings.provider.saving", "保存中…")
                      : label("global_settings.provider.save", "保存全局模型")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="h-[36px] rounded-[12px] bg-white px-4"
                    onClick={handleRevealProvider}
                  >
                    {label("global_settings.provider.reveal", "打开全局 provider.json")}
                  </Button>
                </div>
                <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
                  <h3 className="text-[14px] font-bold text-ink">
                    {label("locale.section.title", "语言选择")}
                  </h3>
                  <div className="flex flex-wrap gap-2">
                    {LOCALE_OPTIONS.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        onClick={() => setLocale(option.value as AppLocale)}
                        className={`rounded-[12px] border px-3 py-2 text-[12px] transition-colors ${
                          locale === option.value
                            ? "border-[#d5e4d8] bg-white font-bold text-primary"
                            : "border-line bg-white text-ink hover:bg-[#f8f9fb]"
                        }`}
                      >
                        {label(option.labelKey, option.value)}
                      </button>
                    ))}
                  </div>
                </section>
                <section className="grid gap-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <h3 className="text-[14px] font-bold text-ink">
                        {label("onboarding.tour.settings_title", "新手引导")}
                      </h3>
                      <p className="mt-1 text-[12px] text-muted-foreground">
                        {label(
                          "onboarding.tour.settings_description",
                          "重新查看首次使用的工作区与员工绑定引导。",
                        )}
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-9 shrink-0 rounded-[12px] bg-white px-3 text-[12px]"
                      onClick={handleReopenOnboardingTour}
                    >
                      {label("onboarding.tour.reopen", "重新打开新手引导")}
                    </Button>
                  </div>
                </section>
              </div>
            )}

            {activeTab === "dream" && (
              <div className="space-y-4">
                <h3 className="text-[14px] font-bold text-ink">
                  {label("global_settings.dream.title", "Dream 全局配置")}
                </h3>

                <section className="space-y-3 rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
                  <div>
                    <label className="mb-1.5 flex items-center gap-1.5 text-[13px] font-medium text-ink">
                      <Clock className="size-3.5" />
                      {label("global_settings.dream.default_time", "默认触发时间")}
                    </label>
                    <p className="mb-2 text-[11px] text-muted-foreground">
                      {label(
                        "global_settings.dream.default_time_hint",
                        "每日定时调度模式下，如果员工未单独配置触发时间，将使用此全局默认时间。",
                      )}
                    </p>
                    <div className="flex items-center gap-2">
                      <input
                        type="time"
                        value={dreamTimeInput}
                        onChange={(e) => setDreamTimeInput(e.target.value)}
                        className="min-h-[42px] rounded-[12px] border border-line bg-white px-3 text-[13px] text-ink focus:border-primary focus:outline-none"
                      />
                      <Button
                        type="button"
                        size="sm"
                        className="h-[36px] rounded-[12px] px-4"
                        disabled={
                          dreamTimeSaving ||
                          !dreamTimeInput ||
                          dreamTimeInput === dreamDefaults?.default_dream_time
                        }
                        onClick={async () => {
                          setDreamTimeSaving(true)
                          try {
                            await updateDreamDefaults({
                              default_dream_time: dreamTimeInput,
                            })
                          } finally {
                            setDreamTimeSaving(false)
                          }
                        }}
                      >
                        {dreamTimeSaving
                          ? label("global_settings.dream.saving", "保存中…")
                          : label("global_settings.dream.save", "保存")}
                      </Button>
                    </div>
                    <p className="mt-1.5 text-[11px] text-muted-foreground">
                      {applyLabelTemplate(
                        label(
                          "global_settings.dream.current_default",
                          "当前全局默认：{{time}}",
                        ),
                        { time: dreamDefaults?.default_dream_time ?? "09:00" },
                      )}
                    </p>
                  </div>

                  <div className="border-t border-line-soft pt-3">
                    <p className="text-[12px] text-muted-foreground">
                      {label(
                        "global_settings.dream.per_employee_hint",
                        "每位员工可以在自己的 Dream 配置中覆盖此时间。未设置覆盖的员工将使用此全局默认值。",
                      )}
                    </p>
                  </div>
                </section>
              </div>
            )}

            {activeTab === "global_skills" && (
              <div className="space-y-4">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="text-[14px] font-bold text-ink">
                    {label("global_settings.skills.title", "全局 Skill 目录")}
                  </h3>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="rounded-[12px]"
                    onClick={() => openHubMarket("skills")}
                  >
                    {label("global_settings.skills.open_market", "打开技能市场")}
                  </Button>
                </div>
                {rootInfo?.mcp_dir ? (
                  <InfoRow
                    label={label("global_settings.skills.mcp_dir", "MCP 目录")}
                    value={rootInfo.mcp_dir}
                  />
                ) : null}
                {skillsLoading ? (
                  <p className="text-[12px] text-muted-foreground">
                    {label("global_settings.skills.loading", "加载技能目录…")}
                  </p>
                ) : rootCatalog.length === 0 ? (
                  <p className="text-[12px] text-muted-foreground">
                    {label("global_settings.skills.empty", "暂无全局 Skill")}
                  </p>
                ) : (
                  <div className="space-y-1.5">
                    {rootCatalog.map((skill) => (
                      <div
                        key={skill.id}
                        className="rounded-[13px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3"
                      >
                        <div className="text-[12px] font-bold text-ink">{skill.name}</div>
                        {skill.description ? (
                          <p className="text-[11px] text-muted-foreground">{skill.description}</p>
                        ) : null}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {activeTab === "templates" && (
              <p className="py-8 text-center text-[13px] text-muted-foreground">
                {label("global_settings.templates.empty", "模板管理暂未实现")}
              </p>
            )}
          </div>
          <ToastContainer />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
