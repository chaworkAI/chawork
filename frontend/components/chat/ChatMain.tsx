import type { Message } from "@/types/message"
import type { Attachment } from "@/types/message"
import type { ReviewPanelEntry } from "@/types/events"
import type { BindingValidation } from "@/types/employee"
import { ChatHeader, type ChatHeaderProps } from "@/components/chat/ChatHeader"
import { Composer } from "@/components/chat/Composer"
import { MessageList } from "@/components/chat/MessageList"
import { UserPromptCard } from "@/components/chat/UserPromptCard"
import { WorkspaceBindingPrompt } from "@/components/workspace/WorkspaceBindingPrompt"

export interface ChatMainProps {
  header: ChatHeaderProps
  messages: Message[]
  composerValue: string
  composerAttachments?: Attachment[]
  onComposerChange: (value: string) => void
  onComposerAttachmentsChange?: (attachments: Attachment[]) => void
  onSend: () => void
  composerSendBlocked?: boolean
  composerSendBlockedReason?: string
  composerSendBlockedAction?: {
    label: string
    onClick: () => void
  }
  messageEmptyHint?: string
  messageEmptyPrimaryAction?: {
    label: string
    onClick: () => void
  }
  /** When true, show cancel control for the in-flight Codex turn. */
  isStreaming?: boolean
  runtimeNotice?: {
    message: string
    actionLabel?: string
    actionDisabled?: boolean
    onAction?: () => void
    onDismiss?: () => void
  }
  onCancelTurn?: () => void
  onProjectMaterialsClick?: () => void
  /** Pending runtime approvals and user input requests shown above the composer. */
  prompts?: ReviewPanelEntry[]
  onPromptNegative?: (id: string) => void
  onPromptMiddle?: (id: string) => void
  onPromptPositive?: (id: string, payload?: unknown) => void
  workspacePath?: string | null
  workspaceBinding?: BindingValidation | null
  workspaceBindingLoading?: boolean
  onGeneralBindActionReady?: (action: (() => void) | null) => void
}

export function ChatMain({
  header,
  messages,
  composerValue,
  composerAttachments = [],
  onComposerChange,
  onComposerAttachmentsChange,
  onSend,
  composerSendBlocked,
  composerSendBlockedReason,
  composerSendBlockedAction,
  messageEmptyHint,
  messageEmptyPrimaryAction,
  isStreaming,
  runtimeNotice,
  onCancelTurn,
  onProjectMaterialsClick,
  prompts = [],
  onPromptNegative,
  onPromptMiddle,
  onPromptPositive,
  workspacePath,
  workspaceBinding,
  workspaceBindingLoading,
  onGeneralBindActionReady,
}: ChatMainProps) {
  const showBindingPrompt =
    workspacePath &&
    workspaceBinding &&
    workspaceBinding.status !== "bound"

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)_auto_auto_auto]">
      <ChatHeader {...header} />
      <div className="min-h-0 min-w-0 overflow-hidden">
        <MessageList
          messages={messages}
          isStreaming={isStreaming}
          emptyHint={messageEmptyHint}
          emptyPrimaryAction={messageEmptyPrimaryAction}
        />
      </div>
      {prompts.length > 0 ? (
        <div className="space-y-2 py-2">
          {prompts.length > 1 ? (
            <p className="px-4 text-[11px] font-semibold text-muted-foreground sm:px-[30px]">
              {prompts.length} 项待处理（按顺序处理）
            </p>
          ) : null}
          {prompts.map((entry) => (
            <UserPromptCard
              key={entry.id}
              entry={entry}
              onNegative={onPromptNegative ?? (() => {})}
              onMiddle={onPromptMiddle ?? (() => {})}
              onPositive={onPromptPositive ?? (() => {})}
            />
          ))}
        </div>
      ) : null}
      {showBindingPrompt ? (
        <WorkspaceBindingPrompt
          workspacePath={workspacePath}
          binding={workspaceBinding}
          isLoading={workspaceBindingLoading}
          onGeneralBindActionReady={onGeneralBindActionReady}
        />
      ) : null}
      <Composer
        value={composerValue}
        attachments={composerAttachments}
        onChange={onComposerChange}
        onAttachmentsChange={onComposerAttachmentsChange}
        onSend={onSend}
        sendBlocked={composerSendBlocked}
        sendBlockedReason={composerSendBlockedReason}
        sendBlockedAction={composerSendBlockedAction}
        isStreaming={isStreaming}
        notice={runtimeNotice}
        onCancelTurn={onCancelTurn}
        onProjectMaterialsClick={onProjectMaterialsClick}
      />
    </div>
  )
}
