import {
  useCallback,
  useMemo,
  type ClipboardEvent,
  type KeyboardEvent,
} from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { X } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { shouldSendComposerMessage } from "@/lib/composerKeyboard"
import { useUiLabel } from "@/hooks/useUiLabel"
import { cn } from "@/lib/utils"
import type { Attachment } from "@/types/message"

const IMAGE_MIME_TYPES = ["image/png", "image/jpeg", "image/webp", "image/gif"] as const

function imageExtensionFromMime(mimeType: string): string | undefined {
  if (mimeType === "image/png") return "png"
  if (mimeType === "image/jpeg") return "jpg"
  if (mimeType === "image/webp") return "webp"
  if (mimeType === "image/gif") return "gif"
  return undefined
}

function fileName(path: string | undefined): string {
  if (!path) return ""
  return path.split(/[\\/]/).pop() || path
}

function attachmentName(attachment: Attachment): string {
  return fileName(attachment.path) || attachment.name || "pasted-image"
}

function attachmentKey(attachment: Attachment, index: number): string {
  return attachment.path ?? attachment.data_url ?? `${attachment.name ?? "image"}-${index}`
}

function attachmentPreviewSrc(attachment: Attachment): string | undefined {
  if (attachment.data_url) return attachment.data_url
  if (attachment.path) return convertFileSrc(attachment.path)
  return undefined
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result)
      } else {
        reject(new Error("image read returned non-string result"))
      }
    }
    reader.onerror = () => reject(reader.error ?? new Error("image read failed"))
    reader.readAsDataURL(file)
  })
}

export interface ComposerProps {
  value: string
  attachments?: Attachment[]
  placeholder?: string
  onChange: (value: string) => void
  onAttachmentsChange?: (attachments: Attachment[]) => void
  onSend: () => void
  /** When true, send is disabled even if there is draft text (e.g. no session). */
  sendBlocked?: boolean
  /** Shown as native tooltip on the send button when `sendBlocked` is true. */
  sendBlockedReason?: string
  /** Optional action shown beside the blocked banner (e.g. open settings). */
  sendBlockedAction?: {
    label: string
    onClick: () => void
  }
  /** When true, show cancel control for the in-flight Codex turn. */
  isStreaming?: boolean
  notice?: {
    message: string
    actionLabel?: string
    actionDisabled?: boolean
    onAction?: () => void
    onDismiss?: () => void
  }
  onCancelTurn?: () => void
  onProjectMaterialsClick?: () => void
}

export function Composer({
  value,
  attachments = [],
  placeholder,
  onChange,
  onAttachmentsChange,
  onSend,
  sendBlocked = false,
  sendBlockedReason,
  sendBlockedAction,
  isStreaming,
  notice,
  onCancelTurn,
  onProjectMaterialsClick,
}: ComposerProps) {
  const getLabel = useUiLabel()

  const resolvedSendBlocked = sendBlocked
  const resolvedBlockedReason = sendBlockedReason

  const resolvedPlaceholder =
    placeholder ??
    getLabel("composer.placeholder", "问 ChaWork，或拖入文档（PDF / DOCX / TXT / MD / XLSX / CSV）...")
  const onKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (
        !shouldSendComposerMessage({
          key: e.key,
          shiftKey: e.shiftKey,
          sendBlocked: resolvedSendBlocked,
          nativeEvent: e.nativeEvent,
        })
      ) {
        return
      }
      e.preventDefault()
      onSend()
    },
    [onSend, resolvedSendBlocked],
  )

  const showBlockedBanner = useMemo(
    () => Boolean(resolvedBlockedReason),
    [resolvedBlockedReason],
  )

  const updateAttachments = useCallback(
    (next: Attachment[]) => {
      onAttachmentsChange?.(next)
    },
    [onAttachmentsChange],
  )

  const addImageFiles = useCallback(
    async (files: File[]) => {
      const next = [...attachments]
      const existing = new Set(
        next.map((attachment) => attachment.path ?? attachment.data_url).filter(Boolean),
      )
      for (const file of files) {
        const mimeType = file.type
        if (!IMAGE_MIME_TYPES.includes(mimeType as (typeof IMAGE_MIME_TYPES)[number])) {
          continue
        }
        const dataUrl = await readFileAsDataUrl(file)
        if (existing.has(dataUrl)) continue
        const ext = imageExtensionFromMime(mimeType) ?? "png"
        next.push({
          kind: "image",
          data_url: dataUrl,
          name: file.name || `pasted-image.${ext}`,
          mime_type: mimeType,
        })
        existing.add(dataUrl)
      }
      updateAttachments(next)
    },
    [attachments, updateAttachments],
  )

  const removeAttachment = useCallback(
    (index: number) => {
      updateAttachments(attachments.filter((_, itemIndex) => itemIndex !== index))
    },
    [attachments, updateAttachments],
  )

  const handlePaste = useCallback(
    (e: ClipboardEvent<HTMLTextAreaElement>) => {
      const imageFiles: File[] = []
      for (let i = 0; i < e.clipboardData.items.length; i++) {
        const item = e.clipboardData.items[i]
        if (item.kind !== "file" || !item.type.startsWith("image/")) continue
        const file = item.getAsFile()
        if (file) imageFiles.push(file)
      }
      if (imageFiles.length === 0) return
      e.preventDefault()
      void addImageFiles(imageFiles)
    },
    [addImageFiles],
  )

  const canSend = (Boolean(value.trim()) || attachments.length > 0) && !resolvedSendBlocked
  return (
    <footer className="pointer-events-none bg-transparent px-[30px] pb-5 pt-0">
      {showBlockedBanner ? (
        <div
          className="pointer-events-auto mb-2 flex items-center justify-between gap-3 rounded-[15px] border border-[#e4d4ac] bg-[#fff8e7] px-3.5 py-2 text-[12px] leading-relaxed text-[#6f5b34] dark:border-warning/30 dark:bg-warning/10 dark:text-warning"
          role="status"
        >
          <span>{resolvedBlockedReason}</span>
          {sendBlockedAction ? (
            <Button
              type="button"
              variant="outline"
              className="h-8 shrink-0 rounded-[11px] border-[#dac48d] bg-white px-3 text-[#6f5b34] dark:border-warning/30 dark:bg-panel-soft dark:text-warning"
              onClick={sendBlockedAction.onClick}
            >
              {sendBlockedAction.label}
            </Button>
          ) : null}
        </div>
      ) : null}
      {notice ? (
        <div
          className="pointer-events-auto mb-2 flex items-center justify-between gap-3 rounded-[15px] border border-[#e4d4ac] bg-[#fff8e7] px-3.5 py-2 text-[12px] leading-relaxed text-[#6f5b34] dark:border-warning/30 dark:bg-warning/10 dark:text-warning"
          role="status"
        >
          <span className="min-w-0 flex-1">{notice.message}</span>
          <div className="flex shrink-0 items-center gap-1.5">
            {notice.actionLabel && notice.onAction ? (
              <Button
                type="button"
                variant="outline"
                className="h-8 shrink-0 rounded-[11px] border-[#dac48d] bg-white px-3 text-[#6f5b34] dark:border-warning/30 dark:bg-panel-soft dark:text-warning"
                disabled={notice.actionDisabled}
                onClick={notice.onAction}
              >
                {notice.actionLabel}
              </Button>
            ) : null}
            {notice.onDismiss ? (
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                title={getLabel("composer.notice.dismiss", "关闭提示")}
                className="size-8 rounded-[10px] text-[#6f5b34] hover:bg-[#f4e6bf] dark:text-warning dark:hover:bg-warning/15"
                onClick={notice.onDismiss}
              >
                <X className="size-3.5" />
              </Button>
            ) : null}
          </div>
        </div>
      ) : null}
      {attachments.length > 0 ? (
        <div className="pointer-events-auto mb-2 flex max-w-full flex-wrap gap-2">
          {attachments.map((attachment, index) => (
            <div
              key={attachmentKey(attachment, index)}
              className="group flex max-w-[220px] items-center gap-2 rounded-[14px] border border-line bg-white/92 px-2 py-1.5 shadow-[0_8px_22px_rgba(34,41,54,0.10)] dark:bg-panel-soft"
            >
              {attachmentPreviewSrc(attachment) ? (
                <img
                  src={attachmentPreviewSrc(attachment)}
                  alt=""
                  className="size-10 shrink-0 rounded-[10px] object-cover"
                />
              ) : null}
              <span className="min-w-0 truncate text-[12px] font-semibold text-ink">
                {attachmentName(attachment)}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                title={getLabel("composer.remove_attachment", "移除图片")}
                className="size-7 shrink-0 rounded-[9px]"
                onClick={() => removeAttachment(index)}
              >
                <X className="size-3.5" />
              </Button>
            </div>
          ))}
        </div>
      ) : null}
      <div
        className="pointer-events-auto grid min-h-[58px] grid-cols-[40px_1fr_auto] items-center gap-3 rounded-[21px] bg-white/92 px-2.5 py-[7px] shadow-[0_14px_38px_rgba(34,41,54,0.12),0_3px_10px_rgba(34,41,54,0.06),inset_0_0_0_1px_rgba(216,221,228,0.82)] backdrop-blur-[18px] dark:bg-panel-soft/92 dark:shadow-[0_14px_38px_rgba(0,0,0,0.28),inset_0_0_0_1px_var(--line)]"
        data-tour-id="composer"
      >
        <Button
          type="button"
          variant="outline"
          size="icon"
          title={getLabel("composer.upload_title", "项目资料")}
          onClick={onProjectMaterialsClick}
          className="size-9 shrink-0 rounded-[13px] border-line bg-white text-[22px] leading-none text-ink shadow-none hover:bg-[#f6f7f9] dark:bg-panel dark:hover:bg-panel-raised"
        >
          ＋
        </Button>
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          onPaste={handlePaste}
          rows={1}
          className={cn(
            "field-sizing-content max-h-[160px] min-h-9 min-w-0 resize-none border-0 bg-transparent py-1 text-[14px] leading-[1.45] text-ink shadow-none outline-none",
            "placeholder:text-muted-foreground focus-visible:ring-0 focus-visible:ring-offset-0 md:text-[14px]",
          )}
          placeholder={resolvedPlaceholder}
        />
        <div className="flex shrink-0 items-center gap-2">
          <span
            className="inline-flex"
            title={
              resolvedSendBlocked && resolvedBlockedReason
                ? resolvedBlockedReason
                : undefined
            }
          >
            <Button
              type="button"
              onClick={onSend}
              className={cn(
                "min-h-[42px] rounded-full px-[18px] text-white shadow-none hover:opacity-100",
                canSend
                  ? "bg-[#2d2821] hover:bg-[#2d2821]/90 dark:bg-accent dark:hover:brightness-95"
                  : "cursor-not-allowed bg-[#a9a7a2] opacity-50 hover:bg-[#a9a7a2] dark:bg-accent dark:opacity-50",
              )}
              disabled={!canSend}
            >
              {getLabel("composer.send", "发送")}
            </Button>
          </span>
          {isStreaming && onCancelTurn ? (
            <Button
              type="button"
              variant="outline"
              onClick={onCancelTurn}
              className="rounded-[16px] border-line bg-[rgba(255,255,255,0.55)] px-3 py-2.5 text-[13px] font-bold text-ink hover:bg-[rgba(255,255,255,0.85)] dark:bg-panel-soft dark:hover:bg-panel-raised"
            >
              {getLabel("composer.cancel_turn", "停止")}
            </Button>
          ) : null}
        </div>
      </div>
    </footer>
  )
}
