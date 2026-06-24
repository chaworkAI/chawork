import { useEffect, useMemo, useState } from "react"
import { Loader2 } from "lucide-react"

import type { ReviewPanelEntry } from "@/types/events"
import { Button } from "@/components/ui/button"
import { MonospaceBlockCard } from "@/components/ui/monospace-block-card"
import { useUiLabel } from "@/hooks/useUiLabel"
import {
  formatApprovalDetail,
  formatPermissionsSummary,
  getRuntimePromptKind,
  initialMcpFormContent,
  type RuntimePromptKind,
} from "@/lib/runtimePromptFormat"
import { cn } from "@/lib/utils"

export interface UserPromptCardProps {
  entry: ReviewPanelEntry
  onNegative: (id: string) => void
  onMiddle: (id: string) => void
  onPositive: (id: string, payload?: unknown) => void
}

const kindLabels: Record<RuntimePromptKind, string> = {
  permissions: "权限审批",
  approval: "操作审批",
  user_input: "需要回答",
  mcp: "MCP 交互",
}

const kindAccent: Record<RuntimePromptKind, string> = {
  permissions: "border-amber-300/80 bg-amber-50/70 dark:border-amber-700/50 dark:bg-amber-950/20",
  approval: "border-orange-300/80 bg-orange-50/60 dark:border-orange-700/50 dark:bg-orange-950/20",
  user_input: "border-sky-300/80 bg-sky-50/60 dark:border-sky-700/50 dark:bg-sky-950/20",
  mcp: "border-violet-300/80 bg-violet-50/60 dark:border-violet-700/50 dark:bg-violet-950/20",
}

/**
 * Blocking prompt card shown above the Composer.
 * Each runtime request kind uses a dedicated layout to avoid overlapping controls.
 */
export function UserPromptCard({ entry, onNegative, onMiddle, onPositive }: UserPromptCardProps) {
  const getLabel = useUiLabel()
  const kind = getRuntimePromptKind(entry)
  const userInputQuestions = entry.user_input_request?.questions ?? []
  const [selectedAnswers, setSelectedAnswers] = useState<Record<string, string>>({})
  const [textAnswers, setTextAnswers] = useState<Record<string, string>>({})
  const [mcpContentText, setMcpContentText] = useState(() =>
    initialMcpFormContent(entry.mcp_elicitation?.params),
  )
  const [mcpContentError, setMcpContentError] = useState<string | null>(null)

  const isUserInputRequest = kind === "user_input"
  const isMcpElicitation = kind === "mcp"
  const isMcpForm = entry.mcp_elicitation?.mode === "form"
  const isMcpUrl = entry.mcp_elicitation?.mode === "url"
  const permissionLines = useMemo(
    () => formatPermissionsSummary(entry.runtime_permissions?.permissions),
    [entry.runtime_permissions?.permissions],
  )
  const approvalDetail = useMemo(() => formatApprovalDetail(entry), [entry])

  const questionAnswer = (
    questionId: string,
    selected = selectedAnswers,
    texts = textAnswers,
  ) => selected[questionId] ?? texts[questionId]?.trim() ?? ""
  const answeredCount = userInputQuestions.filter((q) => Boolean(questionAnswer(q.id))).length
  const hasTextQuestions = userInputQuestions.some(
    (q) => (q.options ?? []).length === 0 || q.isOther,
  )

  const { negative, middle, positive } = entry.actionLabels ?? {
    negative: getLabel("review.action.reject", "拒绝"),
    middle: getLabel("review.action.accept", "本次允许"),
    positive: getLabel("review.action.accept_session", "本会话允许"),
  }

  const riskLabel = getLabel(`review.risk.${entry.risk}`, entry.risk)
  const isApplying = entry.status === "applying"
  const isError = entry.status === "error"
  const buttonsDisabled = isApplying
  const panelButtonClass =
    "min-h-8 rounded-lg px-3 text-[12px] shadow-none disabled:cursor-not-allowed disabled:opacity-60"
  const fieldClass =
    "rounded-lg border border-border bg-background px-2.5 text-[12px] leading-[1.45] text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-ring focus:ring-2 focus:ring-ring/15 disabled:cursor-not-allowed disabled:opacity-60"

  const buildUserInputAnswers = (
    selected = selectedAnswers,
    texts = textAnswers,
  ) =>
    Object.fromEntries(
      userInputQuestions.map((q) => [
        q.id,
        {
          answers: questionAnswer(q.id, selected, texts)
            ? [questionAnswer(q.id, selected, texts)]
            : [],
        },
      ]),
    )

  const hasAllUserInputAnswers = (
    selected = selectedAnswers,
    texts = textAnswers,
  ) =>
    userInputQuestions.length > 0 &&
    userInputQuestions.every((q) => Boolean(questionAnswer(q.id, selected, texts)))

  const selectAnswer = (questionId: string, answer: string) => {
    if (buttonsDisabled) return
    const next = { ...selectedAnswers, [questionId]: answer }
    setSelectedAnswers(next)
    if (hasAllUserInputAnswers(next, textAnswers)) {
      onPositive(entry.id, buildUserInputAnswers(next, textAnswers))
    }
  }

  const submitUserInputAnswers = () => {
    if (!hasAllUserInputAnswers()) return
    onPositive(entry.id, buildUserInputAnswers())
  }

  const submitMcpElicitation = () => {
    if (!isMcpForm) {
      onPositive(entry.id)
      return
    }
    const trimmed = mcpContentText.trim()
    if (!trimmed) {
      onPositive(entry.id)
      return
    }
    try {
      const content = JSON.parse(trimmed)
      setMcpContentError(null)
      onPositive(entry.id, content)
    } catch {
      setMcpContentError(getLabel("review.mcp.invalid_json", "JSON 格式无效"))
    }
  }

  const showDescription =
    Boolean(entry.description?.trim()) &&
    !(kind === "approval" && approvalDetail)

  const bodyScrolls = isUserInputRequest || isMcpElicitation

  useEffect(() => {
    if (buttonsDisabled || isUserInputRequest || isMcpElicitation) return

    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target
      if (
        target instanceof HTMLElement &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT" ||
          target.isContentEditable)
      ) {
        return
      }
      if (event.key === "Escape") {
        event.preventDefault()
        onNegative(entry.id)
        return
      }
      if (event.key === "Enter" && !event.shiftKey && !event.metaKey && !event.ctrlKey) {
        event.preventDefault()
        if (middle) onMiddle(entry.id)
        else onPositive(entry.id)
      }
    }

    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [
    buttonsDisabled,
    entry.id,
    isMcpElicitation,
    isUserInputRequest,
    middle,
    onMiddle,
    onNegative,
    onPositive,
  ])

  return (
    <article
      className={cn(
        "mx-4 flex max-h-[min(40vh,340px)] flex-col gap-0 rounded-xl border px-3.5 py-3 text-muted-foreground shadow-[0_1px_2px_rgba(15,23,42,0.04)] sm:mx-[30px]",
        kindAccent[kind],
      )}
    >
      <header className="min-w-0 shrink-0 space-y-1">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <span className="shrink-0 rounded-md bg-background/80 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-foreground">
            {kindLabels[kind]}
          </span>
          <strong className="min-w-0 truncate text-[13px] leading-[1.2] text-foreground">
            {entry.title}
          </strong>
          {isApplying ? (
            <span className="inline-flex shrink-0 items-center gap-1 text-[11px] font-bold text-accent-dark">
              <Loader2 className="size-3 animate-spin" />
              {getLabel("review.status.applying", "处理中…")}
            </span>
          ) : null}
          {isError ? (
            <span className="shrink-0 text-[11px] font-bold text-danger">
              {getLabel("review.status.error", "处理失败")}
            </span>
          ) : null}
          {!isApplying && !isError ? (
            <span className="shrink-0 rounded-md bg-background/70 px-1.5 py-0.5 text-[11px] font-bold text-muted-foreground">
              {riskLabel}
            </span>
          ) : null}
        </div>
        {showDescription ? (
          <p className="line-clamp-2 text-[12px] leading-[1.45] text-foreground/85">
            {entry.description}
          </p>
        ) : null}
      </header>

      <div
        className={cn(
          "min-h-0 flex-1 space-y-3 py-3",
          bodyScrolls ? "overflow-y-auto overscroll-contain" : "overflow-hidden",
        )}
      >
      {kind === "permissions" ? (
        <section className="space-y-2">
          {permissionLines.length > 0 ? (
            <ul className="grid gap-1 rounded-lg border border-border/70 bg-background/70 px-3 py-2 text-[12px] text-foreground">
              {permissionLines.map((line) => (
                <li key={line} className="leading-snug">
                  {line}
                </li>
              ))}
            </ul>
          ) : (
            <p className="rounded-lg border border-border/70 bg-background/70 px-3 py-2 text-[12px] text-foreground">
              AI 请求扩展本轮执行权限。请确认是否授予。
            </p>
          )}
        </section>
      ) : null}

      {kind === "approval" && approvalDetail ? (
        <MonospaceBlockCard
          label={approvalDetail.label}
          className="min-h-0"
          maxHeightClassName="max-h-[min(36vh,280px)]"
        >
          {approvalDetail.body}
        </MonospaceBlockCard>
      ) : null}

      {isUserInputRequest && userInputQuestions.length > 0 ? (
        <section className="grid gap-2">
          {userInputQuestions.map((question) => {
            const options = question.options ?? []
            return (
              <div
                key={question.id}
                className="rounded-lg border border-border bg-background/80 px-2.5 py-2"
              >
                {question.header !== question.question ? (
                  <p className="mb-1 text-[11px] font-semibold text-muted-foreground">
                    {question.header}
                  </p>
                ) : null}
                <p className="mb-2 text-[12px] leading-[1.45] text-foreground">
                  {question.question}
                </p>
                {options.length > 0 ? (
                  <div className="flex flex-wrap gap-1.5">
                    {options.map((option) => {
                      const selected = selectedAnswers[question.id] === option.label
                      return (
                        <button
                          key={`${question.id}-${option.label}`}
                          type="button"
                          disabled={buttonsDisabled}
                          aria-pressed={selected}
                          className={cn(
                            "inline-flex cursor-pointer items-center rounded-lg border px-2.5 py-1.5 text-left text-[12px] leading-snug transition-colors disabled:cursor-not-allowed disabled:opacity-60",
                            selected
                              ? "border-primary/35 bg-primary/10 text-foreground"
                              : "border-border bg-background text-foreground hover:bg-muted",
                          )}
                          onClick={() => selectAnswer(question.id, option.label)}
                        >
                          <span className="font-bold">{option.label}</span>
                        </button>
                      )
                    })}
                  </div>
                ) : null}
                {options.length === 0 || question.isOther ? (
                  <div className={options.length > 0 ? "mt-2" : undefined}>
                    <input
                      id={`user-input-${entry.id}-${question.id}`}
                      type={question.isSecret ? "password" : "text"}
                      value={textAnswers[question.id] ?? ""}
                      disabled={buttonsDisabled}
                      autoComplete="off"
                      placeholder={getLabel("review.user_input.placeholder", "输入你的回答")}
                      onChange={(event) =>
                        setTextAnswers((prev) => ({
                          ...prev,
                          [question.id]: event.target.value,
                        }))
                      }
                      onKeyDown={(event) => {
                        if (event.key !== "Enter") return
                        event.preventDefault()
                        submitUserInputAnswers()
                      }}
                      className={cn("min-h-9 w-full py-1.5", fieldClass)}
                    />
                  </div>
                ) : null}
              </div>
            )
          })}
        </section>
      ) : null}

      {isUserInputRequest && userInputQuestions.length === 0 ? (
        <p className="rounded-lg border border-border bg-background/80 px-3 py-2 text-[12px] text-foreground">
          {getLabel(
            "review.user_input.empty",
            "当前没有可展示的问题，请取消后重试或查看 Runtime Inspector。",
          )}
        </p>
      ) : null}

      {isMcpElicitation && isMcpUrl && entry.mcp_elicitation?.message ? (
        <p className="rounded-lg border border-border bg-background/80 px-3 py-2 text-[12px] text-foreground">
          {entry.mcp_elicitation.message}
        </p>
      ) : null}

      {isMcpElicitation && isMcpForm ? (
        <section className="grid gap-1.5 rounded-lg border border-border bg-background/80 px-2.5 py-2">
          <label
            htmlFor={`mcp-content-${entry.id}`}
            className="text-[12px] leading-[1.35] text-foreground"
          >
            {getLabel("review.mcp.content_json", "响应内容 JSON（可选）")}
          </label>
          <textarea
            id={`mcp-content-${entry.id}`}
            value={mcpContentText}
            disabled={buttonsDisabled}
            spellCheck={false}
            placeholder={getLabel("review.mcp.content_placeholder", "留空则直接接受")}
            onChange={(event) => {
              setMcpContentText(event.target.value)
              setMcpContentError(null)
            }}
            className={cn("min-h-[88px] resize-y py-2 font-mono", fieldClass)}
          />
          {mcpContentError ? (
            <span className="text-[11px] font-bold text-danger">{mcpContentError}</span>
          ) : null}
        </section>
      ) : null}
      </div>

      <footer className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-border/60 bg-inherit pt-2">
        {!isUserInputRequest && !isMcpElicitation && !buttonsDisabled ? (
          <span className="mr-auto text-[10px] text-muted-foreground">
            {getLabel(
              "review.keyboard.hint_allow_once",
              "Enter 本次允许 · Esc 拒绝",
            )}
          </span>
        ) : null}
        {isUserInputRequest ? (
          <>
            <Button
              type="button"
              variant="outline"
              disabled={buttonsDisabled}
              className={panelButtonClass}
              onClick={() => onNegative(entry.id)}
            >
              {negative}
            </Button>
            <span className="mr-auto text-[11px] text-muted-foreground">
              {answeredCount}/{userInputQuestions.length} 已填
            </span>
            {(hasTextQuestions || userInputQuestions.every((q) => (q.options ?? []).length === 0)) ? (
              <Button
                type="button"
                variant="outline"
                disabled={buttonsDisabled || !hasAllUserInputAnswers()}
                className={cn(panelButtonClass, "font-bold")}
                onClick={submitUserInputAnswers}
              >
                {positive}
              </Button>
            ) : null}
          </>
        ) : isMcpElicitation ? (
          <>
            <Button
              type="button"
              variant="outline"
              disabled={buttonsDisabled}
              className={cn(panelButtonClass, "bg-transparent")}
              onClick={() => onNegative(entry.id)}
            >
              {negative}
            </Button>
            {middle ? (
              <Button
                type="button"
                variant="outline"
                disabled={buttonsDisabled}
                className={panelButtonClass}
                onClick={() => onMiddle(entry.id)}
              >
                {middle}
              </Button>
            ) : null}
            <Button
              type="button"
              variant="outline"
              disabled={buttonsDisabled}
              className={cn(panelButtonClass, "font-bold")}
              onClick={submitMcpElicitation}
            >
              {positive}
            </Button>
          </>
        ) : (
          <>
            <Button
              type="button"
              variant="outline"
              disabled={buttonsDisabled}
              className={cn(panelButtonClass, "bg-transparent")}
              onClick={() => onNegative(entry.id)}
            >
              {negative}
            </Button>
            {middle ? (
              <Button
                type="button"
                variant="outline"
                disabled={buttonsDisabled}
                className={panelButtonClass}
                onClick={() => onMiddle(entry.id)}
              >
                {middle}
              </Button>
            ) : null}
            <Button
              type="button"
              variant="outline"
              disabled={buttonsDisabled}
              className={cn(panelButtonClass, "font-bold")}
              onClick={() => onPositive(entry.id)}
            >
              {positive}
            </Button>
          </>
        )}
      </footer>
    </article>
  )
}
