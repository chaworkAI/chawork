/** OTA 系统类型定义 */

export interface UpdateCheckResponse {
  update_type: "full" | "hot"
  version: string
  release_notes: string | null
  force_update: boolean
  pub_date: string
  /** 全量更新 URL */
  url?: string
  /** 全量包签名 */
  signature?: string
  /** 热更新补丁 URL */
  patch_url?: string
  /** 补丁 hash (sha256:xxx) */
  patch_hash?: string
  /** 补丁大小 (bytes) */
  patch_size?: number
  /** 热更新失败时的全量回退 URL */
  full_fallback_url?: string
}

export interface OTAConfig {
  /** OTA 服务器地址 */
  serverUrl: string
  /** 检查间隔 (ms)，默认 30 分钟 */
  pollInterval: number
  /** 更新渠道 */
  channel: "stable" | "beta" | "canary"
  /** 设备唯一 ID */
  deviceId: string
}

export interface UpdateProgress {
  status: "checking" | "downloading" | "applying" | "ready" | "idle" | "error"
  progress: number
  totalSize?: number
  downloadedSize?: number
  errorMessage?: string
  updateInfo?: UpdateCheckResponse
}

export interface ReportPayload {
  device_id: string
  from_version: string
  to_version: string
  update_type: "full" | "hot"
  status: "downloading" | "installing" | "success" | "failed"
  error_message?: string
}
