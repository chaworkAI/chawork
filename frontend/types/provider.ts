export interface ProviderConfigView {
  provider: string
  model: string
  openai_base_url: string
  openai_api_key_masked: string
  instructions: string
  valid: boolean
  errors: string[]
  updated_at?: string
}

export interface ProviderConfigInput {
  provider: string
  model: string
  openai_base_url: string
  openai_api_key: string
  instructions: string
}

export interface ProviderResolution {
  effective_scope: "root" | "none"
  effective_provider: ProviderConfigView | null
  can_send: boolean
  blocked_reason?: string
}

export interface ProviderModelListResult {
  models: string[]
  message: string
  latency_ms?: number
}
