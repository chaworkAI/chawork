import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react"
import { createPortal } from "react-dom"

/** 首帧尚无真实 DOM 高度时，用文案粗估气泡高度以便决定上下翻转 */
function estimateTooltipHeight(tip: string, bubbleWidth: number): number {
  const innerW = Math.max(40, bubbleWidth - 24)
  const approxCharPx = 7.1
  const charsPerLine = Math.max(16, Math.floor(innerW / approxCharPx))
  const lines = Math.max(1, Math.ceil(tip.length / charsPerLine))
  const lineHeight = 22
  const verticalPad = 24
  return Math.min(verticalPad + lines * lineHeight, 360)
}

function layoutBottomCentered(
  r: DOMRect,
  width: number,
  h: number,
  gap: number,
  pad: number,
  vw: number,
  vh: number,
): CSSProperties {
  const spaceBelow = vh - pad - r.bottom - gap
  const spaceAbove = r.top - gap - pad

  let placement: "below" | "above" = "below"
  if (h <= spaceBelow) placement = "below"
  else if (h <= spaceAbove) placement = "above"
  else placement = spaceBelow >= spaceAbove ? "below" : "above"

  const maxAvail = placement === "below" ? spaceBelow : spaceAbove
  const maxHeight = h > maxAvail ? Math.max(100, maxAvail) : undefined

  const top = placement === "below" ? r.bottom + gap : r.top - gap
  const transform =
    placement === "below" ? "translateX(-50%)" : "translate(-50%, -100%)"

  let left = r.left + r.width / 2
  const half = width / 2
  left = Math.min(Math.max(left, pad + half), vw - pad - half)

  return {
    position: "fixed",
    left,
    top,
    width,
    transform,
    zIndex: 320,
    ...(maxHeight !== undefined
      ? { maxHeight, overflowY: "auto" as const }
      : {}),
  }
}

function layoutTopRight(
  r: DOMRect,
  width: number,
  h: number,
  gap: number,
  pad: number,
  vw: number,
  vh: number,
): CSSProperties {
  const spaceAbove = r.top - gap - pad
  const spaceBelow = vh - pad - r.bottom - gap

  let placement: "above" | "below" = "above"
  if (h <= spaceAbove) placement = "above"
  else if (h <= spaceBelow) placement = "below"
  else placement = spaceAbove >= spaceBelow ? "above" : "below"

  const maxAvail = placement === "above" ? spaceAbove : spaceBelow
  const maxHeight = h > maxAvail ? Math.max(100, maxAvail) : undefined

  let left = Math.min(Math.max(r.right, pad + width), vw - pad)

  if (placement === "above") {
    return {
      position: "fixed",
      left,
      top: r.top - gap,
      width,
      transform: "translate(-100%, -100%)",
      zIndex: 320,
      ...(maxHeight !== undefined
        ? { maxHeight, overflowY: "auto" as const }
        : {}),
    }
  }
  return {
    position: "fixed",
    left,
    top: r.bottom + gap,
    width,
    transform: "translateX(-100%)",
    zIndex: 320,
    ...(maxHeight !== undefined
      ? { maxHeight, overflowY: "auto" as const }
      : {}),
  }
}

function layoutBottomRight(
  r: DOMRect,
  width: number,
  h: number,
  gap: number,
  pad: number,
  vw: number,
  vh: number,
): CSSProperties {
  const spaceBelow = vh - pad - r.bottom - gap
  const spaceAbove = r.top - gap - pad

  let placement: "below" | "above" = "below"
  if (h <= spaceBelow) placement = "below"
  else if (h <= spaceAbove) placement = "above"
  else placement = spaceBelow >= spaceAbove ? "below" : "above"

  const maxAvail = placement === "below" ? spaceBelow : spaceAbove
  const maxHeight = h > maxAvail ? Math.max(100, maxAvail) : undefined

  let left = Math.min(Math.max(r.right, pad + width), vw - pad)

  if (placement === "below") {
    return {
      position: "fixed",
      left,
      top: r.bottom + gap,
      width,
      transform: "translateX(-100%)",
      zIndex: 320,
      ...(maxHeight !== undefined
        ? { maxHeight, overflowY: "auto" as const }
        : {}),
    }
  }
  return {
    position: "fixed",
    left,
    top: r.top - gap,
    width,
    transform: "translate(-100%, -100%)",
    zIndex: 320,
    ...(maxHeight !== undefined
      ? { maxHeight, overflowY: "auto" as const }
      : {}),
  }
}

export function HelpTip({
  tip,
  variant = "bottom",
}: {
  tip: string
  variant?: "bottom" | "bottomRight" | "topRight"
}) {
  const anchorRef = useRef<HTMLSpanElement>(null)
  const bubbleRef = useRef<HTMLSpanElement>(null)
  const [open, setOpen] = useState(false)
  const [bubbleStyle, setBubbleStyle] = useState<CSSProperties | null>(null)
  const [bubbleOpaque, setBubbleOpaque] = useState(false)

  const computeBubbleStyle = useCallback((): CSSProperties | null => {
    const anchor = anchorRef.current
    if (!anchor || typeof window === "undefined") return null

    const r = anchor.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight
    const pad = 14
    const gap = 12
    const width = Math.min(260, vw - 24)

    const measured = bubbleRef.current?.getBoundingClientRect().height
    const h =
      measured !== undefined && measured > 4
        ? measured
        : estimateTooltipHeight(tip, width)

    if (variant === "bottom") {
      return layoutBottomCentered(r, width, h, gap, pad, vw, vh)
    }
    if (variant === "bottomRight") {
      return layoutBottomRight(r, width, h, gap, pad, vw, vh)
    }
    return layoutTopRight(r, width, h, gap, pad, vw, vh)
  }, [variant, tip])

  useLayoutEffect(() => {
    if (!open) {
      setBubbleStyle(null)
      setBubbleOpaque(false)
      return
    }

    setBubbleOpaque(false)
    setBubbleStyle(computeBubbleStyle())
    let raf2 = 0
    const raf1 = window.requestAnimationFrame(() => {
      setBubbleStyle(computeBubbleStyle())
      raf2 = window.requestAnimationFrame(() => {
        setBubbleOpaque(true)
      })
    })

    const onScrollOrResize = () => {
      setBubbleStyle(computeBubbleStyle())
    }
    window.addEventListener("scroll", onScrollOrResize, true)
    window.addEventListener("resize", onScrollOrResize)
    const ro =
      typeof ResizeObserver !== "undefined" && anchorRef.current
        ? new ResizeObserver(onScrollOrResize)
        : null
    if (ro && anchorRef.current) ro.observe(anchorRef.current)

    return () => {
      window.cancelAnimationFrame(raf1)
      window.cancelAnimationFrame(raf2)
      window.removeEventListener("scroll", onScrollOrResize, true)
      window.removeEventListener("resize", onScrollOrResize)
      ro?.disconnect()
    }
  }, [open, computeBubbleStyle])

  const bubbleClass =
    "pointer-events-none rounded-[14px] border border-[rgba(46,38,29,0.14)] bg-[rgba(42,35,27,0.94)] px-3 py-[11px] text-left text-[12px] font-normal leading-[1.55] tracking-normal text-[#fff8ed] shadow-[0_18px_42px_rgba(42,35,27,0.22)] transition-opacity duration-[180ms] ease-out normal-case motion-reduce:transition-none"

  const bubble =
    typeof document !== "undefined" && open && bubbleStyle
      ? createPortal(
          <span
            ref={bubbleRef}
            role="tooltip"
            className={bubbleClass}
            style={{ ...bubbleStyle, opacity: bubbleOpaque ? 1 : 0 }}
          >
            {tip}
          </span>,
          document.body,
        )
      : null

  return (
    <span className="inline-flex shrink-0 align-middle">
      <span
        ref={anchorRef}
        className="inline-grid size-[18px] cursor-help place-items-center rounded-full border border-line-strong bg-[rgba(255,255,255,0.55)] text-[11px] font-bold text-accent-dark"
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
      >
        ?
      </span>
      {bubble}
    </span>
  )
}
