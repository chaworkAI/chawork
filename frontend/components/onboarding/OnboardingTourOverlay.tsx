import { useCallback, useEffect, useMemo, useState } from "react"
import { X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { cn } from "@/lib/utils"

export interface OnboardingTourStep {
  id: string
  targetId: string
  title: string
  body: string
  actionLabel?: string
  onAction?: () => void
}

export interface OnboardingTourOverlayProps {
  steps: OnboardingTourStep[]
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface TargetRect {
  top: number
  left: number
  width: number
  height: number
}

const PADDING = 8

export function OnboardingTourOverlay({
  steps,
  open,
  onOpenChange,
}: OnboardingTourOverlayProps) {
  const getLabel = useUiLabel()
  const [stepIndex, setStepIndex] = useState(0)
  const [targetRect, setTargetRect] = useState<TargetRect | null>(null)

  const currentStep = steps[stepIndex] ?? steps[0] ?? null

  const measureTarget = useCallback(() => {
    if (!currentStep) {
      setTargetRect(null)
      return
    }
    const target = document.querySelector<HTMLElement>(
      `[data-tour-id="${currentStep.targetId}"]`,
    )
    if (!target) {
      setTargetRect(null)
      return
    }
    const rect = target.getBoundingClientRect()
    setTargetRect({
      top: Math.max(PADDING, rect.top - PADDING),
      left: Math.max(PADDING, rect.left - PADDING),
      width: rect.width + PADDING * 2,
      height: rect.height + PADDING * 2,
    })
  }, [currentStep])

  useEffect(() => {
    if (!open) return
    setStepIndex((current) => Math.min(current, Math.max(steps.length - 1, 0)))
  }, [open, steps.length])

  useEffect(() => {
    if (!open) return
    measureTarget()
    window.addEventListener("resize", measureTarget)
    window.addEventListener("scroll", measureTarget, true)
    return () => {
      window.removeEventListener("resize", measureTarget)
      window.removeEventListener("scroll", measureTarget, true)
    }
  }, [measureTarget, open])

  const popoverStyle = useMemo(() => {
    if (!targetRect) {
      return {
        left: "50%",
        top: "50%",
        transform: "translate(-50%, -50%)",
      }
    }
    const viewportWidth = window.innerWidth
    const estimatedWidth = 340
    const preferredLeft = targetRect.left + targetRect.width + 16
    const left =
      preferredLeft + estimatedWidth <= viewportWidth - 16
        ? preferredLeft
        : Math.max(16, targetRect.left - estimatedWidth - 16)
    return {
      left,
      top: Math.max(16, targetRect.top),
      transform: "none",
    }
  }, [targetRect])

  if (!open || steps.length === 0 || !currentStep) return null

  const onPrevious = () => {
    setStepIndex((current) => Math.max(0, current - 1))
  }

  const onNext = () => {
    if (stepIndex >= steps.length - 1) {
      onOpenChange(false)
      return
    }
    setStepIndex((current) => Math.min(steps.length - 1, current + 1))
  }

  const onSkip = () => {
    onOpenChange(false)
  }

  const skipLabel = getLabel("onboarding.tour.skip", "跳过")
  const previousLabel = getLabel("onboarding.tour.previous", "上一步")
  const nextLabel = getLabel("onboarding.tour.next", "下一步")
  const doneLabel = getLabel("onboarding.tour.done", "完成")
  const skipAriaLabel = getLabel("onboarding.tour.skip_aria", "跳过引导")
  const targetMissingHint = getLabel(
    "onboarding.tour.target_missing_hint",
    "完成前面的步骤后，这个位置会出现在界面中。你也可以继续查看下一步。",
  )

  return (
    <div
      className="onboarding-tour-overlay fixed inset-0 z-[90]"
      role="dialog"
      aria-modal="true"
      aria-labelledby="onboarding-tour-title"
    >
      <div className="absolute inset-0 bg-foreground/42 backdrop-blur-[1px]" />
      {targetRect ? (
        <div
          className="absolute rounded-[18px] border border-white/90 shadow-[0_0_0_9999px_rgba(15,23,42,0.42),0_0_0_4px_rgba(255,255,255,0.32),0_18px_48px_rgba(15,23,42,0.22)]"
          style={targetRect}
          aria-hidden
        />
      ) : null}
      <div
        className="absolute w-[min(340px,calc(100vw-32px))] rounded-[16px] border border-line bg-white p-4 text-ink shadow-[0_24px_70px_rgba(15,23,42,0.24)]"
        style={popoverStyle}
      >
        <div className="mb-3 flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[11px] font-bold uppercase text-muted-foreground">
              {stepIndex + 1} / {steps.length}
            </p>
            <h2
              id="onboarding-tour-title"
              className="mt-1 text-[15px] font-extrabold leading-snug text-ink"
            >
              {currentStep.title}
            </h2>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="-mr-1 -mt-1 size-8 rounded-full text-muted-foreground hover:text-ink"
            aria-label={skipAriaLabel}
            onClick={onSkip}
          >
            <X className="size-4" />
          </Button>
        </div>
        <p className="text-[13px] leading-relaxed text-muted-foreground">
          {currentStep.body}
        </p>
        {!targetRect ? (
          <p className="mt-2 rounded-[10px] bg-muted/45 px-3 py-2 text-[12px] leading-relaxed text-muted-foreground">
            {targetMissingHint}
          </p>
        ) : null}
        {currentStep.actionLabel && currentStep.onAction ? (
          <Button
            type="button"
            size="sm"
            variant="default"
            className="mt-4 h-9 rounded-[12px] px-3 text-[13px]"
            onClick={currentStep.onAction}
          >
            {currentStep.actionLabel}
          </Button>
        ) : null}
        <div className="mt-4 flex items-center justify-between gap-2 border-t border-line-soft pt-3">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-8 rounded-[11px] px-3"
            onClick={onSkip}
          >
            {skipLabel}
          </Button>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-8 rounded-[11px] px-3"
              disabled={stepIndex === 0}
              onClick={onPrevious}
            >
              {previousLabel}
            </Button>
            <Button
              type="button"
              variant="default"
              size="sm"
              className={cn("h-8 rounded-[11px] px-3")}
              onClick={onNext}
            >
              {stepIndex >= steps.length - 1 ? doneLabel : nextLabel}
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
