import type { CSSProperties } from "react"
import { AlertCircle, CheckCircle2 } from "lucide-react"

import type { HubInstallFeedback } from "@/stores/hub"

const PARTICLES = [
  ["#f97316", "-92px", "-46px", "0ms", "6px"],
  ["#22c55e", "88px", "-52px", "35ms", "7px"],
  ["#3b82f6", "-78px", "54px", "75ms", "6px"],
  ["#ec4899", "84px", "58px", "115ms", "6px"],
  ["#eab308", "0px", "-88px", "45ms", "7px"],
  ["#8b5cf6", "0px", "84px", "135ms", "6px"],
  ["#06b6d4", "-112px", "4px", "95ms", "5px"],
  ["#ef4444", "112px", "2px", "155ms", "5px"],
  ["#84cc16", "-48px", "-86px", "185ms", "5px"],
  ["#f59e0b", "52px", "86px", "215ms", "5px"],
  ["#a855f7", "48px", "-82px", "245ms", "5px"],
  ["#14b8a6", "-52px", "82px", "275ms", "5px"],
] as const

export function HubCardFeedback({
  feedback,
}: {
  feedback: HubInstallFeedback | undefined
}) {
  if (!feedback) return null

  if (feedback.status === "success") {
    return (
      <div
        key={feedback.nonce}
        className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center"
      >
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(34,197,94,0.16),transparent_48%)]" />
        <span className="absolute left-1/2 top-1/2 block size-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-[#22c55e]/35 opacity-70 animate-ping" />
        {PARTICLES.map(([color, x, y, delay, size]) => (
          <span
            key={`${color}-${x}-${y}`}
            className="hub-firework-particle absolute left-1/2 top-1/2 block rounded-full shadow-[0_0_12px_currentColor]"
            style={{
              "--hub-firework-x": x,
              "--hub-firework-y": y,
              animationDelay: delay,
              backgroundColor: color,
              color,
              height: size,
              width: size,
            } as CSSProperties}
          />
        ))}
        <div className="hub-firework-card relative rounded-[12px] border border-[#bfe5c9] bg-white/95 px-3.5 py-2.5 text-[12px] font-bold text-[#126a34] shadow-[0_18px_46px_rgba(18,106,52,0.24)] backdrop-blur">
          <span className="absolute left-1/2 top-1/2 block size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#22c55e] opacity-60 animate-ping" />
          <span className="relative flex items-center gap-1.5">
            <CheckCircle2 className="size-3.5" />
            {feedback.message}
          </span>
        </div>
      </div>
    )
  }

  return (
    <div
      key={feedback.nonce}
      className="pointer-events-none absolute inset-x-3 top-3 z-10 rounded-[12px] border border-[#efb2b2] bg-[#fff6f6] px-3 py-2 text-[11px] font-bold leading-4 text-[#9b1c1c] shadow-[0_10px_26px_rgba(155,28,28,0.10)]"
      title={feedback.message}
    >
      <span className="flex items-start gap-1.5">
        <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
        <span className="line-clamp-2">{feedback.message}</span>
      </span>
    </div>
  )
}
