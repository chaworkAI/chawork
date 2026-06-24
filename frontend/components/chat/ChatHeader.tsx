import { FolderOpen } from "lucide-react"
import { HelpTip } from "@/components/layout/HelpTip"
import { formatDisplayPath } from "@/lib/formatPath"
import { cn } from "@/lib/utils"
import { useUiLabel } from "@/hooks/useUiLabel"
import type { EffectiveProviderPayload } from "@/lib/tauri"
import type { IndexStatus } from "@/types/workspace"

export interface ChatHeaderChip {
  id: string
  label: string
  className?: string
  onClick?: () => void
}

export interface ChatHeaderProps {
  workspaceName?: string
  workspacePath?: string
  sessionTitle: string
  effectiveProvider?: EffectiveProviderPayload | null
  indexStatus?: IndexStatus
  chips?: readonly ChatHeaderChip[]
  onIndexClick?: () => void
  onProjectMaterialsClick?: () => void
  onWorkspaceConfigClick?: () => void
  boundEmployeeName?: string | null
  onEmployeeClick?: () => void
}

export function ChatHeader({
  workspaceName,
  workspacePath,
  sessionTitle,
  effectiveProvider,
  indexStatus,
  chips = [],
  onIndexClick,
  onProjectMaterialsClick,
  onWorkspaceConfigClick,
  boundEmployeeName,
  onEmployeeClick,
}: ChatHeaderProps) {
  const getLabel = useUiLabel()

  void indexStatus
  void onIndexClick

  const modelLabel = effectiveProvider?.configured
    ? effectiveProvider.model
    : effectiveProvider?.model
      ? getLabel("chat.header.model_unconfigured", "模型未配置")
      : null

  const allChips = chips

  const infoLine = workspaceName
    ? workspacePath
      ? `${workspaceName} · ${formatDisplayPath(workspacePath)}`
      : workspaceName
    : getLabel("chat.session_info.pick_workspace", "选择工作区后，在此开始对话。")

  return (
    <header className="flex min-h-[48px] items-center justify-between gap-[18px] bg-[linear-gradient(180deg,#ffffff,#fbfcfd)] px-6 py-2 shadow-[inset_0_-1px_0_rgba(224,228,234,0.55)] dark:bg-panel dark:bg-none dark:shadow-[inset_0_-1px_0_var(--line-soft)]">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <h1 className="truncate text-[18px] font-bold leading-[1.12] tracking-normal text-ink">
            {sessionTitle}
          </h1>
          {boundEmployeeName ? (
            <button
              type="button"
              className="employee-context-badge min-h-[34px] shrink-0 rounded-full border px-3 text-[13px] font-extrabold leading-none"
              onClick={onEmployeeClick}
            >
              {boundEmployeeName}
            </button>
          ) : null}
          <HelpTip
            variant="bottom"
            tip={getLabel(
              "chat.header.tip",
              "这里是当前工作区里的一个会话。同一个工作区可以有多个会话，它们共享资料和技能，但聊天记录彼此独立。",
            )}
          />
        </div>
        <p className="mt-0.5 truncate text-[12px] font-semibold leading-tight text-muted-foreground">{infoLine}</p>
        {workspaceName ? (
          <div className="mt-1 flex flex-wrap items-center gap-3">
            {modelLabel ? (
              <span
                className={cn(
                  "font-mono text-[11px]",
                  effectiveProvider?.configured
                    ? "text-muted-foreground"
                    : "text-danger",
                )}
              >
                {modelLabel}
              </span>
            ) : null}
            {onWorkspaceConfigClick ? (
              <button
                type="button"
                onClick={onWorkspaceConfigClick}
                className="text-[11px] text-muted-foreground underline-offset-2 hover:text-ink hover:underline"
              >
                工作区配置
              </button>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {onProjectMaterialsClick ? (
          <button
            type="button"
            onClick={onProjectMaterialsClick}
            className="inline-flex min-h-7 items-center gap-1.5 rounded-[10px] bg-[#f5f7fa] px-2.5 text-[12px] font-bold text-muted-foreground transition-colors hover:bg-[#eef2f6] hover:text-ink dark:bg-panel-soft dark:hover:bg-panel-raised"
          >
            <FolderOpen className="size-4" strokeWidth={1.75} />
            <span>项目资料</span>
          </button>
        ) : null}
        {allChips.length > 0 ? (
          <>
          {allChips.map((chip) => {
            const Tag = chip.onClick ? "button" : "span"
            return (
              <Tag
                key={chip.id}
                type={chip.onClick ? "button" : undefined}
                onClick={chip.onClick}
                className={cn(
                  "rounded-full border px-3 py-[7px] text-[12px] font-bold transition-colors",
                  chip.className ??
                    "border-line bg-[rgba(255,255,255,0.44)] text-ink/72 dark:bg-panel-soft",
                  chip.onClick && "hover:bg-[rgba(255,255,255,0.72)] dark:hover:bg-panel-raised",
                )}
              >
                {chip.label}
              </Tag>
            )
          })}
          </>
        ) : null}
      </div>
    </header>
  )
}
