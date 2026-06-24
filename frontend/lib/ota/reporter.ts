import type { ReportPayload } from "./types"

const DEFAULT_SERVER_URL = "https://api.chawork.com"

/**
 * 上报升级事件状态
 * 失败时静默忽略，不阻塞主流程
 */
export async function report(serverUrl: string, payload: ReportPayload): Promise<void> {
  try {
    await fetch(`${serverUrl || DEFAULT_SERVER_URL}/api/ota/report`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    })
  } catch {
    // 上报失败静默处理
  }
}
