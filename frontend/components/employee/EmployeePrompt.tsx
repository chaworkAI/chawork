import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react"
import { Check, Edit3, Eye, Loader2, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { useUiLabel } from "@/hooks/useUiLabel"
import { highlightFenceCodeToHtml } from "@/lib/highlightFenceCode"
import { cn } from "@/lib/utils"
import { useEmployeeStore } from "@/stores/employee"

const CODE_LANG_RE = /^[\w+#.-]{1,40}$/

interface ContentSegment {
  type: "text" | "code"
  lang?: string
  value: string
}

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
      const firstLine = trimmed.trim()
      lang = CODE_LANG_RE.test(firstLine) ? firstLine : undefined
      body = lang ? "" : trimmed
    } else {
      const firstLine = trimmed.slice(0, nl).trim()
      if (CODE_LANG_RE.test(firstLine)) {
        lang = firstLine
        body = trimmed.slice(nl + 1)
      } else {
        body = trimmed
      }
    }
    out.push({ type: "code", lang, value: body })
  }
  return out
}

function tokenizeInlineMarkdown(text: string): ReactNode[] {
  const parts: ReactNode[] = []
  const re = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*]+\*)|(\~\~[^~]+\~\~)|(\[([^\]]+)\]\(([^)]+)\))/g
  let lastIdx = 0
  let match: RegExpExecArray | null

  while ((match = re.exec(text)) !== null) {
    if (match.index > lastIdx) {
      parts.push(text.slice(lastIdx, match.index))
    }
    if (match[1]) {
      parts.push(
        <code
          key={match.index}
          className="rounded bg-[rgba(0,0,0,0.06)] px-1 py-0.5 font-mono text-[0.9em]"
        >
          {match[1].slice(1, -1)}
        </code>,
      )
    } else if (match[2]) {
      parts.push(<strong key={match.index}>{match[2].slice(2, -2)}</strong>)
    } else if (match[3]) {
      parts.push(<em key={match.index}>{match[3].slice(1, -1)}</em>)
    } else if (match[4]) {
      parts.push(<del key={match.index}>{match[4].slice(2, -2)}</del>)
    } else if (match[5]) {
      parts.push(
        <a
          key={match.index}
          href={match[7]}
          target="_blank"
          rel="noopener noreferrer"
          className="text-primary underline underline-offset-2"
        >
          {match[6]}
        </a>,
      )
    }
    lastIdx = match.index + match[0].length
  }

  if (lastIdx < text.length) {
    parts.push(text.slice(lastIdx))
  }
  return parts
}

function renderMarkdownLine(line: string, key: number | string): ReactNode {
  if (/^#{1,6}\s/.test(line)) {
    const level = line.match(/^(#{1,6})\s/)![1].length
    const content = line.replace(/^#{1,6}\s+/, "")
    const className =
      level <= 1
        ? "mt-1 text-[18px] font-extrabold leading-7"
        : level === 2
          ? "mt-3 text-[15px] font-bold leading-6"
          : "mt-2 text-[13px] font-bold leading-5"
    return (
      <div key={key} className={className}>
        {tokenizeInlineMarkdown(content)}
      </div>
    )
  }

  if (/^[-*]\s/.test(line)) {
    return (
      <div key={key} className="grid grid-cols-[14px_minmax(0,1fr)] gap-1.5 pl-1">
        <span className="text-muted-foreground">-</span>
        <span>{tokenizeInlineMarkdown(line.replace(/^[-*]\s+/, ""))}</span>
      </div>
    )
  }

  if (/^\d+\.\s/.test(line)) {
    const num = line.match(/^(\d+)\.\s/)![1]
    return (
      <div key={key} className="grid grid-cols-[24px_minmax(0,1fr)] gap-1.5 pl-1">
        <span className="text-muted-foreground">{num}.</span>
        <span>{tokenizeInlineMarkdown(line.replace(/^\d+\.\s+/, ""))}</span>
      </div>
    )
  }

  if (line.trim() === "") {
    return <div key={key} className="h-2" />
  }

  return <p key={key}>{tokenizeInlineMarkdown(line)}</p>
}

function HighlightedFence({ code, lang }: { code: string; lang?: string }) {
  const { html, usePlain } = useMemo(() => {
    try {
      return { html: highlightFenceCodeToHtml(code, lang), usePlain: false }
    } catch {
      return { html: "", usePlain: true }
    }
  }, [code, lang])

  return (
    <pre className="my-2 max-h-[260px] overflow-auto rounded-[13px] border border-line-soft bg-[#f8f9fb] p-3">
      {usePlain ? (
        <code className="whitespace-pre-wrap font-mono text-[12px] leading-relaxed text-ink">
          {code.replace(/\n+$/, "")}
        </code>
      ) : (
        <code
          className="hljs block bg-transparent p-0 font-mono text-[12px] leading-relaxed"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      )}
    </pre>
  )
}

function MarkdownPreview({ content }: { content: string }) {
  const segments = useMemo(() => splitFencedCodeBlocks(content), [content])

  if (content.trim().length === 0) {
    return null
  }

  return (
    <div className="space-y-1 break-words text-[13px] leading-6 text-ink">
      {segments.map((segment, index) =>
        segment.type === "code" ? (
          <HighlightedFence key={index} code={segment.value} lang={segment.lang} />
        ) : (
          <div key={index} className="space-y-1">
            {segment.value.split("\n").map((line, lineIndex) =>
              renderMarkdownLine(line, `${index}-${lineIndex}`),
            )}
          </div>
        ),
      )}
    </div>
  )
}

export function EmployeePrompt() {
  const t = useUiLabel()
  const detail = useEmployeeStore((s) => s.selectedDetail)
  const promptContent = useEmployeeStore((s) => s.promptContent)
  const updatePrompt = useEmployeeStore((s) => s.updatePrompt)

  const employeeId = detail?.registry_entry.id
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState("")
  const [saving, setSaving] = useState(false)
  const [viewMode, setViewMode] = useState<"preview" | "source">("preview")

  useEffect(() => {
    if (!editing) {
      setDraft(promptContent ?? "")
    }
  }, [editing, promptContent])

  const startEdit = useCallback(() => {
    setDraft(promptContent ?? "")
    setEditing(true)
    setViewMode("source")
  }, [promptContent])

  const cancelEdit = useCallback(() => {
    setDraft(promptContent ?? "")
    setEditing(false)
    setViewMode("preview")
  }, [promptContent])

  const save = useCallback(async () => {
    if (!employeeId) return
    setSaving(true)
    try {
      await updatePrompt(employeeId, draft)
      setEditing(false)
      setViewMode("preview")
    } finally {
      setSaving(false)
    }
  }, [draft, employeeId, updatePrompt])

  if (!detail) return null

  const content = editing ? draft : (promptContent ?? "")
  const hasContent = content.trim().length > 0
  const isLoading = promptContent === null

  return (
    <div className="grid gap-4">
      <div className="flex justify-end">
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          {editing ? (
            <>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-[34px] rounded-[12px] bg-white px-3"
                onClick={() => setViewMode(viewMode === "preview" ? "source" : "preview")}
              >
                <Eye className="mr-1.5 size-3.5" />
                {viewMode === "preview"
                  ? t("employee.prompt.show_source", "源码")
                  : t("employee.prompt.show_preview", "预览")}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-[34px] rounded-[12px] bg-white px-3"
                disabled={saving}
                onClick={cancelEdit}
              >
                <X className="mr-1.5 size-3.5" />
                {t("employee.prompt.cancel", "取消")}
              </Button>
              <Button
                type="button"
                size="sm"
                className="h-[34px] rounded-[12px] px-3"
                disabled={saving}
                onClick={() => void save()}
              >
                {saving ? (
                  <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                ) : (
                  <Check className="mr-1.5 size-3.5" />
                )}
                {t("employee.prompt.save", "保存")}
              </Button>
            </>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-[34px] rounded-[12px] bg-white px-3"
              disabled={isLoading}
              onClick={startEdit}
            >
              <Edit3 className="mr-1.5 size-3.5" />
              {t("employee.prompt.edit", "编辑")}
            </Button>
          )}
        </div>
      </div>

      <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        {isLoading ? (
          <p className="text-[13px] text-muted-foreground">
            {t("employee.prompt.loading", "正在加载 Prompt...")}
          </p>
        ) : editing && viewMode === "source" ? (
          <Textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            className="min-h-[420px] resize-y rounded-[13px] border-line bg-white px-3 py-3 font-mono text-[12px] leading-relaxed text-ink"
            spellCheck={false}
          />
        ) : hasContent ? (
          <div
            className={cn(
              "rounded-[13px] border border-line-soft bg-white p-3",
              editing && "min-h-[420px]",
            )}
          >
            <MarkdownPreview content={content} />
          </div>
        ) : (
          <p className="rounded-[13px] border border-dashed border-line bg-white px-4 py-8 text-center text-[13px] text-muted-foreground">
            {t(
              "employee.prompt.empty",
              "Prompt 为空。可点击「编辑」直接填写 prompt.md。",
            )}
          </p>
        )}
      </section>
    </div>
  )
}
