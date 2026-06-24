import { check } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import { checkForUpdate } from "./checker"
import { performHotUpdate } from "./hot-updater"
import { report } from "./reporter"
import type { OTAConfig, UpdateCheckResponse, UpdateProgress } from "./types"

const DEFAULT_CONFIG: OTAConfig = {
  serverUrl: "https://api.chawork.com",
  pollInterval: 30 * 60 * 1000,
  channel: "stable",
  deviceId: "unknown",
}

type ProgressCallback = (progress: UpdateProgress) => void

/**
 * OTA 管理器 — 统一调度全量更新与热更新
 */
export class OTAManager {
  private config: OTAConfig
  private currentVersion: string
  private timer: ReturnType<typeof setInterval> | null = null
  private listeners: Set<ProgressCallback> = new Set()

  constructor(currentVersion: string, config?: Partial<OTAConfig>) {
    this.currentVersion = currentVersion
    this.config = { ...DEFAULT_CONFIG, ...config }
  }

  /** 订阅进度变化 */
  onProgress(callback: ProgressCallback): () => void {
    this.listeners.add(callback)
    return () => this.listeners.delete(callback)
  }

  private emit(progress: UpdateProgress): void {
    for (const cb of this.listeners) cb(progress)
  }

  /** 启动定时轮询 */
  startPolling(): void {
    if (this.timer) return
    this.checkNow({ silent: true })
    this.timer = setInterval(() => {
      this.checkNow({ silent: true })
    }, this.config.pollInterval)
  }

  stopPolling(): void {
    if (this.timer) {
      clearInterval(this.timer)
      this.timer = null
    }
  }

  /** 手动触发检查 */
  async checkNow(options?: { silent?: boolean }): Promise<UpdateCheckResponse | null> {
    this.emit({ status: "checking", progress: 0 })

    try {
      const info = await checkForUpdate(this.config, this.currentVersion)
      if (!info) {
        this.emit({ status: "idle", progress: 0 })
        return null
      }

      this.emit({ status: "ready", progress: 0, updateInfo: info })
      return info
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (!options?.silent) {
        this.emit({ status: "error", progress: 0, errorMessage: msg })
      } else {
        this.emit({ status: "idle", progress: 0 })
      }
      return null
    }
  }

  /** 执行升级 */
  async performUpdate(info: UpdateCheckResponse): Promise<void> {
    if (info.update_type === "hot") {
      await this.performHotUpdate(info)
    } else {
      await this.performFullUpdate(info)
    }
  }

  private async performHotUpdate(info: UpdateCheckResponse): Promise<void> {
    this.emit({ status: "downloading", progress: 0, updateInfo: info })

    const success = await performHotUpdate(
      info,
      this.config.serverUrl,
      this.currentVersion,
      this.config.deviceId,
      (percent) => {
        const status = percent < 50 ? "downloading" : "applying"
        this.emit({ status, progress: percent, updateInfo: info })
      },
    )

    if (success) {
      this.emit({ status: "ready", progress: 100, updateInfo: info })
      window.location.reload()
    } else {
      this.emit({ status: "error", progress: 0, errorMessage: "热更新失败", updateInfo: info })
    }
  }

  private async performFullUpdate(info: UpdateCheckResponse): Promise<void> {
    this.emit({ status: "downloading", progress: 0, updateInfo: info })

    try {
      await report(this.config.serverUrl, {
        device_id: this.config.deviceId,
        from_version: this.currentVersion,
        to_version: info.version,
        update_type: "full",
        status: "downloading",
      })

      const update = await check()
      if (!update?.available) {
        this.emit({ status: "error", progress: 0, errorMessage: "Tauri updater 未发现更新" })
        return
      }

      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          this.emit({
            status: "downloading",
            progress: 0,
            totalSize: event.data.contentLength,
            updateInfo: info,
          })
        } else if (event.event === "Progress") {
          this.emit({
            status: "downloading",
            progress: event.data.chunkLength,
            updateInfo: info,
          })
        } else if (event.event === "Finished") {
          this.emit({ status: "applying", progress: 95, updateInfo: info })
        }
      })

      await report(this.config.serverUrl, {
        device_id: this.config.deviceId,
        from_version: this.currentVersion,
        to_version: info.version,
        update_type: "full",
        status: "success",
      })

      this.emit({ status: "ready", progress: 100, updateInfo: info })
      await relaunch()
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err)

      await report(this.config.serverUrl, {
        device_id: this.config.deviceId,
        from_version: this.currentVersion,
        to_version: info.version,
        update_type: "full",
        status: "failed",
        error_message: errorMessage,
      })

      this.emit({ status: "error", progress: 0, errorMessage, updateInfo: info })
    }
  }

  updateConfig(partial: Partial<OTAConfig>): void {
    this.config = { ...this.config, ...partial }
  }

  getConfig(): Readonly<OTAConfig> {
    return this.config
  }
}

export { type OTAConfig, type UpdateCheckResponse, type UpdateProgress } from "./types"
