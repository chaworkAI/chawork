import * as Dialog from "@radix-ui/react-dialog"
import { useCallback, useEffect, useRef, useState } from "react"
import { X } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import * as ipc from "@/lib/tauri"
import { cn } from "@/lib/utils"
import { useProviderStore } from "@/stores/provider"
import { useRootConfigStore } from "@/stores/rootConfig"

function isGlobalProviderConfigured(
  globalProvider: ReturnType<typeof useProviderStore.getState>["globalProvider"],
): boolean {
  return Boolean(globalProvider?.valid)
}

export function ProviderSetupDialog() {
  const globalProvider = useProviderStore((s) => s.globalProvider)
  const globalProviderLoading = useProviderStore((s) => s.globalProviderLoading)
  const loadGlobalProvider = useProviderStore((s) => s.loadGlobalProvider)
  const loadEffectiveProvider = useProviderStore((s) => s.loadEffectiveProvider)
  const openSettingsPanel = useRootConfigStore((s) => s.openSettingsPanel)
  const settingsPanelOpen = useRootConfigStore((s) => s.settingsPanelOpen)

  const [dismissed, setDismissed] = useState(false)
  const prevSettingsPanelOpenRef = useRef(settingsPanelOpen)

  const needsSetup =
    !globalProviderLoading && !isGlobalProviderConfigured(globalProvider)
  const open = needsSetup && !dismissed && !settingsPanelOpen

  useEffect(() => {
    void loadGlobalProvider()
  }, [loadGlobalProvider])

  useEffect(() => {
    if (prevSettingsPanelOpenRef.current && !settingsPanelOpen) {
      void loadGlobalProvider()
    }
    prevSettingsPanelOpenRef.current = settingsPanelOpen
  }, [loadGlobalProvider, settingsPanelOpen])

  useEffect(() => {
    if (globalProviderLoading) return
    if (isGlobalProviderConfigured(globalProvider)) {
      setDismissed(false)
    }
  }, [globalProvider, globalProviderLoading])

  const dismiss = useCallback(() => {
    setDismissed(true)
  }, [])

  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (!next) dismiss()
    },
    [dismiss],
  )

  const handleRecheck = useCallback(async () => {
    await loadGlobalProvider()
    await loadEffectiveProvider()
    const view = useProviderStore.getState().globalProvider
    if (isGlobalProviderConfigured(view)) {
      setDismissed(false)
    }
  }, [loadEffectiveProvider, loadGlobalProvider])

  const handleReveal = useCallback(() => {
    void ipc.revealGlobalProviderConfig()
  }, [])

  return (
    <Dialog.Root open={open} onOpenChange={handleOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-80 bg-foreground/24 backdrop-blur-[2px] dark:bg-black/50" />
        <Dialog.Content
          className={cn(
            "fixed left-1/2 top-1/2 z-81 max-h-[min(560px,calc(100vh-48px))] w-[min(440px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 overflow-y-auto p-0 outline-none",
            "font-sans text-foreground shadow-[0_22px_48px_rgba(15,23,42,0.18)] dark:shadow-[0_22px_48px_rgba(0,0,0,0.4)]",
          )}
        >
          <Card className="gap-0 border border-border bg-card py-0 shadow-none ring-1 ring-foreground/10">
            <CardHeader className="gap-2 border-b border-border px-5 pb-4 pt-5">
              <div className="flex items-start justify-between gap-3">
                <Dialog.Title asChild>
                  <CardTitle className="font-bold text-[17px] leading-snug tracking-tight text-foreground">
                    开始使用前
                  </CardTitle>
                </Dialog.Title>
                <Dialog.Close asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="shrink-0 text-muted-foreground hover:text-foreground"
                    aria-label="关闭"
                    onClick={dismiss}
                  >
                    <X className="size-4" />
                  </Button>
                </Dialog.Close>
              </div>
              <Dialog.Description asChild>
                <CardDescription className="text-[13px] leading-relaxed text-muted-foreground">
                  先配置好 AI 模型，ChaWork 才能帮你对话和处理任务。
                </CardDescription>
              </Dialog.Description>
            </CardHeader>

            <CardContent className="space-y-3 px-5 py-5">
              <ol className="list-decimal space-y-2 pl-5 text-[13px] leading-relaxed text-muted-foreground">
                <li>在设置里填写接口地址和 API Key，然后从模型列表中选择模型</li>
                <li>打开或绑定一个工作区文件夹</li>
                <li>发送第一条消息，开始协作</li>
              </ol>
            </CardContent>

            <CardFooter className="flex flex-col gap-4 border-t border-border bg-transparent px-5 py-4">
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="default"
                  onClick={() => openSettingsPanel("provider")}
                >
                  去配置
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => void handleRecheck()}
                >
                  重新检测
                </Button>
                <Button type="button" variant="outline" onClick={handleReveal}>
                  打开配置文件
                </Button>
              </div>
              <div className="flex w-full justify-end">
                <Button
                  type="button"
                  variant="secondary"
                  className="text-foreground"
                  onClick={dismiss}
                >
                  稍后继续
                </Button>
              </div>
            </CardFooter>
          </Card>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
