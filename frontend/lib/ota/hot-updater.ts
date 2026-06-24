import { invoke } from "@tauri-apps/api/core"
import type { UpdateCheckResponse } from "./types"
import { report } from "./reporter"

/**
 * 热更新引擎
 * 下载 bsdiff 补丁并通过 Rust 侧 apply
 */
export async function performHotUpdate(
  updateInfo: UpdateCheckResponse,
  serverUrl: string,
  currentVersion: string,
  deviceId: string,
  onProgress?: (percent: number) => void,
): Promise<boolean> {
  if (!updateInfo.patch_url) return false

  try {
    await report(serverUrl, {
      device_id: deviceId,
      from_version: currentVersion,
      to_version: updateInfo.version,
      update_type: "hot",
      status: "downloading",
    })

    onProgress?.(0)

    // 通过 Rust 命令下载补丁（利用 Tauri 的网络能力处理代理等）
    const patchPath = await invoke<string>("download_ota_file", {
      url: updateInfo.patch_url,
      expectedHash: updateInfo.patch_hash?.replace("sha256:", "") ?? null,
    })

    onProgress?.(50)

    await report(serverUrl, {
      device_id: deviceId,
      from_version: currentVersion,
      to_version: updateInfo.version,
      update_type: "hot",
      status: "installing",
    })

    // 应用补丁
    await invoke("apply_hot_patch", {
      patchPath,
      currentVersion,
    })

    onProgress?.(90)

    await report(serverUrl, {
      device_id: deviceId,
      from_version: currentVersion,
      to_version: updateInfo.version,
      update_type: "hot",
      status: "success",
    })

    onProgress?.(100)
    return true
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : String(err)

    await report(serverUrl, {
      device_id: deviceId,
      from_version: currentVersion,
      to_version: updateInfo.version,
      update_type: "hot",
      status: "failed",
      error_message: errorMessage,
    })

    // 热更新失败，尝试全量回退
    if (updateInfo.full_fallback_url) {
      return performFullFallback(updateInfo, serverUrl, currentVersion, deviceId)
    }

    return false
  }
}

/** 热更新失败后回退到全量前端包下载 */
async function performFullFallback(
  updateInfo: UpdateCheckResponse,
  serverUrl: string,
  currentVersion: string,
  deviceId: string,
): Promise<boolean> {
  if (!updateInfo.full_fallback_url) return false

  try {
    const bundlePath = await invoke<string>("download_ota_file", {
      url: updateInfo.full_fallback_url,
      expectedHash: null,
    })

    await invoke("apply_full_frontend_bundle", { bundlePath })

    await report(serverUrl, {
      device_id: deviceId,
      from_version: currentVersion,
      to_version: updateInfo.version,
      update_type: "hot",
      status: "success",
    })

    return true
  } catch {
    return false
  }
}
