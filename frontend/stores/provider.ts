import { create } from "zustand"

import * as ipc from "@/lib/tauri"
import { useToastStore } from "@/stores/toast"
import { useRuntimeStore } from "@/stores/runtime"
import type { EffectiveProviderPayload } from "@/lib/tauri"
import type {
  ProviderConfigInput,
  ProviderModelListResult,
  ProviderConfigView,
  ProviderResolution,
} from "@/types/provider"

let providerModelListRequestId = 0

const DASHSCOPE_COMPATIBLE_BASE_URL = "https://dashscope.aliyuncs.com/compatible-mode/v1"

// DashScope's OpenAI-compatible Responses endpoint currently supports only this Qwen allowlist.
// Keep this in sync with Aliyun's compatibility notes:
// https://help.aliyun.com/zh/model-studio/compatibility-with-openai-responses-api
const DASHSCOPE_ALLOWED_MODELS = new Set([
  "qwen3-max",
  "qwen3-max-2026-01-23",
  "qwen3.6-plus",
  "qwen3.6-plus-2026-04-02",
  "qwen3.5-plus",
  "qwen3.5-plus-2026-02-15",
  "qwen3.6-flash",
  "qwen3.6-flash-2026-04-16",
  "qwen3.5-flash",
  "qwen3.5-flash-2026-02-23",
  "qwen3.6-35b-a3b",
  "qwen3.5-397b-a17b",
  "qwen3.5-122b-a10b",
  "qwen3.5-27b",
  "qwen3.5-35b-a3b",
  "qwen-plus",
  "qwen-flash",
  "qwen3-coder-plus",
  "qwen3-coder-flash",
  "qwen3-coder-next",
])

function isDashScopeCompatibleBaseUrl(baseUrl: string): boolean {
  return baseUrl.trim().replace(/\/+$/, "") === DASHSCOPE_COMPATIBLE_BASE_URL
}

function filterDisplayModelsForProvider(
  baseUrl: string,
  models: string[],
): string[] {
  if (!isDashScopeCompatibleBaseUrl(baseUrl)) return models
  return models.filter((model) => DASHSCOPE_ALLOWED_MODELS.has(model))
}

interface ProviderState {
  globalProvider: ProviderConfigView | null
  globalProviderLoading: boolean
  globalProviderError: string | null

  effective: ProviderResolution | null

  providerModels: string[]
  providerModelsLoading: boolean
  providerModelsError: string | null
  providerModelsMessage: string | null

  globalForm: ProviderConfigInput

  loadGlobalProvider: () => Promise<void>
  saveGlobalProvider: (config: ProviderConfigInput) => Promise<void>
  loadProviderModels: (
    config?: ProviderConfigInput,
  ) => Promise<ProviderModelListResult | null>
  resetProviderModels: () => void
  loadEffectiveProvider: () => Promise<void>
  updateGlobalForm: (partial: Partial<ProviderConfigInput>) => void
  canSend: () => boolean
  getBlockedReason: () => string | null
}

const emptyForm: ProviderConfigInput = {
  provider: "",
  model: "",
  openai_base_url: "",
  openai_api_key: "",
  instructions: "",
}

function mapGlobalPayload(result: Awaited<ReturnType<typeof ipc.getGlobalProvider>>): ProviderConfigView {
  return {
    provider: "openai-compatible",
    model: result.model,
    openai_base_url: result.openai_base_url,
    openai_api_key_masked: result.openai_api_key
      ? "••••" + result.openai_api_key.slice(-4)
      : "",
    instructions: result.instructions,
    valid: result.configured,
    errors: result.configured ? [] : ["还没配置 AI 模型"],
    updated_at: undefined,
  }
}

const PROVIDER_NOT_CONFIGURED_MESSAGE = "还没配置 AI 模型，完成后就可以开始聊天啦"

function friendlyEffectiveBlockedReason(
  payload: EffectiveProviderPayload,
): string | undefined {
  if (payload.configured && payload.error_kind === null) return undefined
  if (payload.error_kind === "global_not_configured") {
    return PROVIDER_NOT_CONFIGURED_MESSAGE
  }
  if (payload.error_kind === "no_workspace") {
    return "请先打开或选择一个工作区"
  }
  return payload.error_message ?? PROVIDER_NOT_CONFIGURED_MESSAGE
}

function mapEffectiveToResolution(
  payload: EffectiveProviderPayload,
): ProviderResolution {
  const effective_scope: ProviderResolution["effective_scope"] =
    payload.origin === "inherit_global" ? "root" : "none"

  const effective_provider: ProviderConfigView | null = payload.configured
    ? {
        provider: "openai-compatible",
        model: payload.model,
        openai_base_url: "",
        openai_api_key_masked: "",
        instructions: "",
        valid: !payload.error_kind,
        errors: payload.error_message ? [payload.error_message] : [],
      }
    : null

  return {
    effective_scope,
    effective_provider,
    can_send: payload.configured && payload.error_kind === null,
    blocked_reason: friendlyEffectiveBlockedReason(payload),
  }
}

export function selectProviderCanSend(state: ProviderState): boolean {
  if (state.effective?.can_send) return true
  if (state.globalProvider?.valid) return true
  if (state.effective) return false
  return state.globalProvider?.valid ?? false
}

export function selectProviderBlockedReason(state: ProviderState): string | null {
  if (state.effective?.can_send || state.globalProvider?.valid) return null
  if (state.effective?.blocked_reason) return state.effective.blocked_reason
  if (!state.globalProvider?.valid) {
    return state.globalProvider?.errors[0] ?? PROVIDER_NOT_CONFIGURED_MESSAGE
  }
  return null
}

export const useProviderStore = create<ProviderState>((set, get) => ({
  globalProvider: null,
  globalProviderLoading: false,
  globalProviderError: null,

  effective: null,

  providerModels: [],
  providerModelsLoading: false,
  providerModelsError: null,
  providerModelsMessage: null,

  globalForm: { ...emptyForm },

  loadGlobalProvider: async () => {
    set({ globalProviderLoading: true, globalProviderError: null })
    try {
      const result = await ipc.getGlobalProvider()
      const view = mapGlobalPayload(result)
      set({
        globalProvider: view,
        globalProviderLoading: false,
        globalForm: {
          provider: view.provider,
          model: view.model,
          openai_base_url: view.openai_base_url,
          openai_api_key: result.openai_api_key,
          instructions: "",
        },
      })
      await get().loadEffectiveProvider()
    } catch (e) {
      set({
        globalProviderLoading: false,
        globalProviderError: e instanceof Error ? e.message : String(e),
      })
    }
  },

  saveGlobalProvider: async (config) => {
    set({ globalProviderLoading: true, globalProviderError: null })
    try {
      const result = await ipc.setGlobalProvider(config)
      useRuntimeStore
        .getState()
        .handleRuntimeInvalidation(result.runtimeInvalidation)
      await get().loadGlobalProvider()
      await get().loadEffectiveProvider()
      if (
        result.runtimeInvalidation.invalidatedNowCount === 0 &&
        result.runtimeInvalidation.deferredCount === 0
      ) {
        useToastStore.getState().show("全局模型配置已保存", "success")
      }
    } catch (e) {
      set({
        globalProviderLoading: false,
        globalProviderError: e instanceof Error ? e.message : String(e),
      })
    }
  },

  loadProviderModels: async (config) => {
    const requestId = ++providerModelListRequestId
    set({
      providerModels: [],
      providerModelsLoading: true,
      providerModelsError: null,
      providerModelsMessage: null,
    })
    try {
      const result = await ipc.listProviderModels(config)
      if (requestId !== providerModelListRequestId) return null
      const displayModels = filterDisplayModelsForProvider(
        config?.openai_base_url ?? get().globalForm.openai_base_url,
        result.models,
      )
      const displayResult: ProviderModelListResult = {
        ...result,
        models: displayModels,
        message:
          displayModels.length === result.models.length
            ? result.message
            : `已获取 ${displayModels.length} 个可用模型`,
      }
      set({
        providerModels: displayResult.models,
        providerModelsLoading: false,
        providerModelsError: null,
        providerModelsMessage: displayResult.message,
      })
      return displayResult
    } catch (e) {
      if (requestId !== providerModelListRequestId) return null
      set({
        providerModels: [],
        providerModelsLoading: false,
        providerModelsError: e instanceof Error ? e.message : String(e),
        providerModelsMessage: null,
      })
      return null
    }
  },

  resetProviderModels: () => {
    providerModelListRequestId += 1
    set({
      providerModels: [],
      providerModelsLoading: false,
      providerModelsError: null,
      providerModelsMessage: null,
    })
  },

  loadEffectiveProvider: async () => {
    try {
      const effectivePayload = await ipc.getEffectiveProvider()
      const resolution = mapEffectiveToResolution(effectivePayload)
      set({
        effective: resolution,
      })
    } catch {
      const global = get().globalProvider
      if (global?.valid) {
        set({
          effective: {
            effective_scope: "root",
            effective_provider: global,
            can_send: true,
            blocked_reason: undefined,
          },
        })
        return
      }
      set({ effective: null })
    }
  },

  updateGlobalForm: (partial) => {
    set({ globalForm: { ...get().globalForm, ...partial } })
  },

  canSend: () => selectProviderCanSend(get()),

  getBlockedReason: () => selectProviderBlockedReason(get()),
}))
