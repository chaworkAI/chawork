import { check } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import { ask, message } from "@tauri-apps/plugin-dialog"

export interface UpdateCheckResult {
  available: boolean
  version?: string
  body?: string
}

/**
 * @deprecated 使用 OTAManager 替代
 * Check whether a newer version exists without prompting.
 */
export async function checkForUpdate(): Promise<UpdateCheckResult> {
  const update = await check()
  if (!update?.available) return { available: false }
  return {
    available: true,
    version: update.version,
    body: update.body ?? undefined,
  }
}

/**
 * @deprecated 使用 OTAManager 替代
 * Check for updates, prompt the user via native dialog if one is found,
 * and install + relaunch on confirmation.
 */
export async function checkAndPromptUpdate(options?: {
  silent?: boolean
}): Promise<void> {
  try {
    const update = await check()
    if (!update?.available) {
      if (!options?.silent) {
        await message("当前已是最新版本。", { title: "检查更新", kind: "info" })
      }
      return
    }

    const body = [
      `发现新版本 ${update.version}`,
      update.body ?? "",
    ]
      .filter(Boolean)
      .join("\n\n")
      .trim()

    const yes = await ask(body, {
      title: "软件更新",
      kind: "info",
      okLabel: "立即更新",
      cancelLabel: "稍后",
    })
    if (!yes) return

    await update.downloadAndInstall()
    await relaunch()
  } catch (err) {
    if (!options?.silent) {
      const msg = err instanceof Error ? err.message : String(err)
      await message(`检查更新失败：${msg}`, { title: "更新错误", kind: "error" })
    }
  }
}
