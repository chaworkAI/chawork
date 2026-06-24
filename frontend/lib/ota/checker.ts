import { invoke } from "@tauri-apps/api/core"
import type { OTAConfig, UpdateCheckResponse, ReportPayload } from "./types"

const DEFAULT_SERVER_URL = "https://api.chawork.com"

/**
 * 向 OTA 服务器发起更新检查
 */
export async function checkForUpdate(config: OTAConfig, currentVersion: string): Promise<UpdateCheckResponse | null> {
  const { target, arch } = await invoke<{ target: string; arch: string }>("get_platform_info")

  const params = new URLSearchParams({
    current_version: currentVersion,
    target,
    arch,
    device_id: config.deviceId,
    channel: config.channel,
  })

  const url = `${config.serverUrl || DEFAULT_SERVER_URL}/api/ota/check?${params}`
  const resp = await fetch(url)

  if (resp.status === 204) return null
  if (!resp.ok) throw new Error(`Check failed: ${resp.status}`)

  return resp.json()
}

/**
 * 上报升级状态
 */
export async function reportStatus(serverUrl: string, payload: ReportPayload): Promise<void> {
  const url = `${serverUrl || DEFAULT_SERVER_URL}/api/ota/report`
  await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  }).catch(() => {
    // 上报失败不阻塞主流程
  })
}
