import { Children, isValidElement, useMemo, type ReactNode } from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import type { Message } from "@/types/message"
import { ThinkingBlock } from "@/components/chat/ThinkingBlock"
import { useChatStore } from "@/stores/chat"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardHeader,
} from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"
import { useUiLabel } from "@/hooks/useUiLabel"
import { highlightFenceCodeToHtml } from "@/lib/highlightFenceCode"
import { cn } from "@/lib/utils"

export interface MessageBubbleProps {
  message: Message
}

function formatTime(ts: string) {
  try {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "numeric",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    })
  } catch {
    return ts
  }
}

function attachmentName(path: string | undefined, name?: string) {
  if (!path) return name ?? "pasted-image"
  return path.split(/[\\/]/).pop() || path
}

function attachmentPreviewSrc(attachment: NonNullable<Message["attachments"]>[number]) {
  if (attachment.data_url) return attachment.data_url
  if (attachment.path) return convertFileSrc(attachment.path)
  return undefined
}

const CODE_LANG_RE = /^[\w+#.-]{1,40}$/

interface ContentSegment {
  type: "text" | "code"
  lang?: string
  value: string
}

/** Best-effort ``` fence split for user messages (no full MD parser). */
function splitFencedCodeBlocks(raw: string): ContentSegment[] {
  const chunks = raw.split("```")
  const out: ContentSegment[] = []
  for (let i = 0; i < chunks.length; i++) {
    const piece = chunks[i] ?? ""
    if (i % 2 === 0) {
      if (piece.length > 0) out.push({ type: "text", value: piece })
      continue
    }
    const trimmed = piece.replace(/^\n/, "")
    const nl = trimmed.indexOf("\n")
    let lang: string | undefined
    let body: string
    if (nl === -1) {
      const t = trimmed.trim()
      lang = CODE_LANG_RE.test(t) ? t : undefined
      body = lang ? "" : trimmed
    } else {
      const first = trimmed.slice(0, nl).trim()
      if (CODE_LANG_RE.test(first)) {
        lang = first
        body = trimmed.slice(nl + 1)
      } else {
        body = trimmed
      }
    }
    out.push({ type: "code", lang, value: body })
  }
  return out
}

/** highlight.js HTML for fenced code blocks. */
function HighlightedFenceCode({ code, lang }: { code: string; lang?: string }) {
  const { html, usePlain } = useMemo(() => {
    try {
      return {
        html: highlightFenceCodeToHtml(code, lang),
        usePlain: false,
      }
    } catch {
      return { html: "", usePlain: true }
    }
  }, [code, lang])

  if (usePlain) {
    return (
      <code className="block w-full min-w-0 whitespace-pre-wrap bg-transparent px-0 py-0 text-left font-mono text-[12px] leading-relaxed text-ink">
        {code.replace(/\n+$/, "")}
      </code>
    )
  }

  return (
    <code
      className="hljs block w-full min-w-0 bg-transparent px-0 py-0 text-left font-mono text-[12px] leading-relaxed"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}

/* ---- Assistant markdown (react-markdown + remark-gfm) ---- */

function reactNodeToText(node: ReactNode): string {
  if (typeof node === "string") return node
  if (typeof node === "number") return String(node)
  if (!node) return ""
  if (Array.isArray(node)) return node.map(reactNodeToText).join("")
  if (isValidElement(node)) {
    return reactNodeToText((node.props as Record<string, unknown>).children as ReactNode)
  }
  return ""
}

const remarkPlugins = [remarkGfm]

/* eslint-disable @typescript-eslint/no-explicit-any */
const mdComponents: Record<string, React.ComponentType<any>> = {
  pre({ children }: { children?: ReactNode }) {
    const codeChild = Children.toArray(children)[0]
    let lang: string | undefined
    let codeText = ""
    if (isValidElement(codeChild)) {
      const props = codeChild.props as Record<string, unknown>
      const match = /language-([\w+#.-]+)/.exec(String(props.className ?? ""))
      lang = match?.[1]
      codeText = reactNodeToText(props.children as ReactNode)
    } else {
      codeText = reactNodeToText(children)
    }
    return (
      <Card className="min-w-0 gap-0 overflow-hidden py-0 ring-1 ring-line border-[#e8ecf2] bg-white ring-[#e8ecf2] dark:border-line dark:bg-panel dark:ring-line">
        {lang ? (
          <CardHeader className="flex-row items-center gap-2 border-b border-line px-3 py-2 pb-2 pt-2">
            <Badge variant="outline" className="font-mono text-[11px]">
              {lang}
            </Badge>
          </CardHeader>
        ) : null}
        <CardContent className={cn("px-0 py-0", lang ? "" : "pt-0")}>
          <pre className="max-h-[min(320px,50vh)] max-w-full overflow-auto px-3 py-2.5 font-mono text-[12px] leading-relaxed text-ink [&_.hljs]:bg-transparent">
            <HighlightedFenceCode code={codeText} lang={lang} />
          </pre>
        </CardContent>
      </Card>
    )
  },

  code({ children }: { children?: ReactNode }) {
    return (
      <code className="rounded bg-[rgba(0,0,0,0.06)] px-1 py-0.5 font-mono text-[0.9em]">
        {children}
      </code>
    )
  },

  h1: ({ children }: { children?: ReactNode }) => (
    <div className="mt-1 text-[1.1em] font-bold">{children}</div>
  ),
  h2: ({ children }: { children?: ReactNode }) => (
    <div className="mt-1 text-[1.1em] font-bold">{children}</div>
  ),
  h3: ({ children }: { children?: ReactNode }) => (
    <div className="mt-1 text-[1em] font-semibold">{children}</div>
  ),
  h4: ({ children }: { children?: ReactNode }) => (
    <div className="mt-1 text-[1em] font-semibold">{children}</div>
  ),
  h5: ({ children }: { children?: ReactNode }) => (
    <div className="mt-1 text-[1em] font-semibold">{children}</div>
  ),
  h6: ({ children }: { children?: ReactNode }) => (
    <div className="mt-1 text-[1em] font-semibold">{children}</div>
  ),

  p: ({ children }: { children?: ReactNode }) => (
    <p className="mb-1 last:mb-0">{children}</p>
  ),

  ul: ({ children }: { children?: ReactNode }) => (
    <ul className="list-disc space-y-0.5 pl-5 marker:text-muted-foreground">{children}</ul>
  ),
  ol: ({ children }: { children?: ReactNode }) => (
    <ol className="list-decimal space-y-0.5 pl-5 marker:text-muted-foreground">{children}</ol>
  ),
  li: ({ children }: { children?: ReactNode }) => <li>{children}</li>,

  a: ({ href, children }: { href?: string; children?: ReactNode }) => (
    <a href={href} target="_blank" rel="noopener noreferrer" className="text-accent-dark underline">
      {children}
    </a>
  ),

  blockquote: ({ children }: { children?: ReactNode }) => (
    <blockquote className="border-l-2 border-accent/40 pl-3 italic text-muted-foreground">
      {children}
    </blockquote>
  ),

  hr: () => <hr className="my-3 border-line" />,

  table: ({ children }: { children?: ReactNode }) => (
    <div className="my-2 overflow-x-auto rounded border border-line">
      <table className="w-full border-collapse text-[13px]">{children}</table>
    </div>
  ),
  thead: ({ children }: { children?: ReactNode }) => <thead>{children}</thead>,
  tbody: ({ children }: { children?: ReactNode }) => <tbody>{children}</tbody>,
  tr: ({ children }: { children?: ReactNode }) => <tr>{children}</tr>,
  th: ({ children }: { children?: ReactNode }) => (
    <th className="border-b border-line bg-panel-soft px-3 py-1.5 text-left text-[12px] font-semibold">
      {children}
    </th>
  ),
  td: ({ children }: { children?: ReactNode }) => (
    <td className="border-b border-line/50 px-3 py-1.5">{children}</td>
  ),
}
/* eslint-enable @typescript-eslint/no-explicit-any */

function AssistantMarkdown({ content }: { content: string }) {
  return (
    <Markdown remarkPlugins={remarkPlugins} components={mdComponents}>
      {content}
    </Markdown>
  )
}

/* ---- Bubble ---- */

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === "user"
  const getLabel = useUiLabel()
  const toggleThinkingExpanded = useChatStore((s) => s.toggleThinkingExpanded)
  const userSegments = useMemo(
    () => (isUser ? splitFencedCodeBlocks(message.content) : []),
    [isUser, message.content],
  )

  const showThinking = Boolean(
    !isUser && (message.thinkingContent?.trim() || message.isThinkingStreaming),
  )
  const imageAttachments = isUser
    ? (message.attachments ?? []).filter((attachment) => attachment.kind === "image")
    : []
  const showSources = Boolean(message.sources?.length && !isUser)
  const showFootnote = Boolean(message.footnote && !isUser)
  const showMetaDividerBeforeTime = showSources || showFootnote
  const hasAnswerContent = Boolean(message.content.trim())
  const showEmptyReply = Boolean(
    !isUser && message.emptyReply && !hasAnswerContent && !message.isStreaming,
  )

  return (
    <div
      className={cn(
        "flex w-fit min-w-0 flex-col border border-line-soft px-4 py-3.5 text-[14px] leading-[1.5] text-ink shadow-none",
        isUser &&
          "max-w-[min(72%,720px)] rounded-[17px_17px_5px_17px] border-[#ded6ca] bg-[#f4f1ed] text-right text-[#2c313a] dark:border-line-strong dark:bg-panel-raised dark:text-ink",
        !isUser &&
          "max-w-[min(78%,860px)] rounded-[17px_17px_17px_5px] border-[#e8ecf2] bg-[#fdfefe] text-left dark:border-line dark:bg-panel-soft",
      )}
    >
      {showThinking ? (
        <ThinkingBlock
          content={message.thinkingContent ?? ""}
          isStreaming={message.isThinkingStreaming}
          isExpanded={message.isThinkingExpanded ?? Boolean(message.isThinkingStreaming)}
          onToggleExpanded={() => toggleThinkingExpanded(message.id)}
        />
      ) : null}

      {imageAttachments.length > 0 ? (
        <div className="mb-2 flex max-w-[260px] flex-wrap gap-2 self-start text-left">
          {imageAttachments.map((attachment, index) => (
            <figure
              key={attachment.path ?? attachment.data_url ?? `${attachment.name ?? "image"}-${index}`}
              className="w-24 min-w-0 overflow-hidden rounded-[10px] border border-[#ded6ca] bg-white/70 dark:border-line dark:bg-panel"
            >
              {attachmentPreviewSrc(attachment) ? (
                <img
                  src={attachmentPreviewSrc(attachment)}
                  alt={attachmentName(attachment.path, attachment.name)}
                  className="size-24 object-cover"
                />
              ) : null}
              <figcaption className="truncate px-2 py-1 text-[11px] font-semibold text-muted-foreground">
                {attachmentName(attachment.path, attachment.name)}
              </figcaption>
            </figure>
          ))}
        </div>
      ) : null}

      {showEmptyReply ? (
        <p className="mt-2 text-[13px] leading-relaxed text-muted-foreground">
          {getLabel(
            "chat.empty_reply",
            "模型只完成了思考，未生成正文回复。请重试，或换一种问法。",
          )}
        </p>
      ) : null}

      {hasAnswerContent ? (
        <div className="min-w-0 space-y-2">
          {isUser ? (
            userSegments.map((seg, idx) =>
              seg.type === "text" ? (
                <div key={idx} className="min-w-0 whitespace-pre-wrap break-words">
                  {seg.value}
                </div>
              ) : (
                <Card
                  key={idx}
                  className="min-w-0 gap-0 overflow-hidden py-0 ring-1 ring-line border-white/15 bg-[rgba(0,0,0,0.22)] ring-white/10 dark:border-line-strong dark:bg-panel dark:ring-line"
                >
                  {seg.lang ? (
                    <CardHeader className="flex-row items-center gap-2 border-b border-line px-3 py-2 pb-2 pt-2">
                      <Badge
                        variant="outline"
                        className="font-mono text-[11px] border-white/25 text-[#fff8ed]/90 hover:text-[#fff8ed]"
                      >
                        {seg.lang}
                      </Badge>
                    </CardHeader>
                  ) : null}
                  <CardContent className={cn("px-0 py-0", seg.lang ? "" : "pt-0")}>
                    <pre className="max-h-[min(320px,50vh)] max-w-full overflow-auto px-3 py-2.5 font-mono text-[12px] leading-relaxed whitespace-pre-wrap text-[#fff8ed]/95">
                      {seg.value.replace(/\n+$/, "")}
                    </pre>
                  </CardContent>
                </Card>
              ),
            )
          ) : (
            <AssistantMarkdown content={message.content} />
          )}
        </div>
      ) : null}

      {message.isStreaming && hasAnswerContent ? (
        <span
          className="mt-1 inline-block h-[1em] w-2 animate-pulse rounded-sm bg-accent align-[-0.15em]"
          aria-hidden
        />
      ) : null}

      {message.isStreaming && !hasAnswerContent && !showThinking ? (
        <span
          className="mt-1 inline-block h-[1em] w-2 animate-pulse rounded-sm bg-accent align-[-0.15em]"
          aria-hidden
        />
      ) : null}

      {showSources ? (
        <>
          <Separator className="my-3 bg-line" />
          <div className="grid gap-2">
            {(message.sources ?? []).map((s) => (
              <Card
                key={s.path + (s.title ?? "")}
                size="sm"
                className="border-[#e8ecf2] bg-white py-3 ring-1 ring-[#e8ecf2] dark:border-line dark:bg-panel dark:ring-line"
              >
                <CardContent className="flex items-center justify-between gap-3 px-3.5 py-0">
                  <span className="min-w-0 truncate text-[13px] text-accent-dark">
                    {s.title ?? s.path}
                  </span>
                  <Button
                    type="button"
                    variant="link"
                    className="h-auto shrink-0 px-0 py-0 text-[13px] font-bold text-accent-dark"
                    onClick={() => {
                      void navigator.clipboard?.writeText(s.path)
                    }}
                  >
                    {getLabel("message.source_copy", "复制路径")}
                  </Button>
                </CardContent>
              </Card>
            ))}
          </div>
        </>
      ) : null}

      {showFootnote ? (
        <>
          <Separator className="my-3 bg-line" />
          <small className="block text-[12px] text-muted-foreground">
            {message.footnote}
          </small>
        </>
      ) : null}

      {showMetaDividerBeforeTime ? (
        <Separator className="my-3 bg-line" />
      ) : null}
      <small
        className={cn(
          "block text-[12px] font-semibold",
          !showMetaDividerBeforeTime && "mt-2.5",
          "text-muted-foreground",
        )}
      >
        {formatTime(message.timestamp)}
      </small>
    </div>
  )
}
