import { useEffect, useRef } from "react"
import type { Message } from "@/types/message"
import { ChatLoadingBubble } from "@/components/chat/ChatLoadingBubble"
import { MessageBubble } from "@/components/chat/MessageBubble"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { useUiLabel } from "@/hooks/useUiLabel"

export interface MessageListEmptyAction {
  label: string
  onClick: () => void
}

export interface MessageListProps {
  messages: Message[]
  /** Waiting for assistant reply (Codex turn in flight). */
  isStreaming?: boolean
  /** When there are no messages, optional centered guidance (e.g. pick workspace). */
  emptyHint?: string
  emptyPrimaryAction?: MessageListEmptyAction
}

function shouldShowChatLoading(messages: Message[], isStreaming: boolean): boolean {
  if (!isStreaming) return false
  const last = messages[messages.length - 1]
  if (!last || last.role === "user") return true
  if (last.role === "assistant") {
    if (last.thinkingContent?.trim() || last.content.trim()) return false
    return true
  }
  return false
}

export function MessageList({
  messages,
  isStreaming = false,
  emptyHint,
  emptyPrimaryAction,
}: MessageListProps) {
  const getLabel = useUiLabel()
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" })
  }, [messages, isStreaming])

  const showEmpty = messages.length === 0 && Boolean(emptyHint)

  return (
    <div
      className="h-full min-h-0 min-w-0 overflow-y-auto overscroll-contain bg-[radial-gradient(circle_at_72%_18%,rgba(34,41,54,0.035),transparent_28%),linear-gradient(90deg,rgba(248,249,251,0.52),rgba(255,255,255,0.16)_28%,rgba(255,255,255,0)),#ffffff] px-9 py-[30px] dark:bg-panel dark:bg-none"
      aria-label="消息列表"
    >
      <div className="flex min-h-0 flex-col gap-3.5">
        {showEmpty ? (
          <Card className="mx-auto w-full max-w-lg border-line-soft bg-white shadow-none dark:bg-panel-soft">
            <CardHeader className="pb-2">
              <CardTitle className="text-[15px] font-semibold text-ink">
                {emptyPrimaryAction
                  ? getLabel("chat.empty.card_title", "开始之前")
                  : getLabel("chat.empty.card_title_idle", "暂无消息")}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-left text-[13px] leading-relaxed text-ink/90">
                {emptyHint}
              </p>
            </CardContent>
            {emptyPrimaryAction ? (
              <CardFooter className="border-t border-line pt-3">
                <Button
                  type="button"
                  variant="default"
                  className="w-full bg-accent/30 text-ink hover:bg-accent/45"
                  onClick={() => void emptyPrimaryAction.onClick()}
                >
                  {emptyPrimaryAction.label}
                </Button>
              </CardFooter>
            ) : null}
          </Card>
        ) : null}
        {messages.map((msg) => (
          <div
            key={msg.id}
            className={
              msg.role === "user"
                ? "flex w-full justify-end"
                : "flex w-full justify-start"
            }
          >
            <MessageBubble message={msg} />
          </div>
        ))}
        {shouldShowChatLoading(messages, isStreaming) ? <ChatLoadingBubble /> : null}
        <div ref={bottomRef} className="h-px shrink-0" aria-hidden />
      </div>
    </div>
  )
}
