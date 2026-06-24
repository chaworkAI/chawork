import type { ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { cn } from "@/lib/utils"

export interface MonospaceBlockCardProps {
  /** Optional label in the card header (outline badge). */
  label?: string
  children: ReactNode
  className?: string
  maxHeightClassName?: string
  /** Default uses `text-ink`; muted uses `text-muted-foreground` (e.g. diffs). */
  tone?: "default" | "muted"
  /** Extra classes on the inner `pre` (e.g. serif diff styling). */
  preClassName?: string
}

/**
 * Compact Card + Badge + `pre` for JSON, diffs, and tool output in runtime / review UIs.
 */
export function MonospaceBlockCard({
  label,
  children,
  className,
  maxHeightClassName = "max-h-[220px]",
  tone = "default",
  preClassName,
}: MonospaceBlockCardProps) {
  return (
    <Card
      className={cn(
        "gap-0 overflow-hidden border-line bg-[rgba(255,252,246,0.9)] py-0 ring-1 ring-line",
        className,
      )}
    >
      {label ? (
        <CardHeader className="border-b border-line px-2.5 py-1.5">
          <Badge variant="outline" className="font-mono text-[10px] tracking-wide">
            {label}
          </Badge>
        </CardHeader>
      ) : null}
      <CardContent className="px-0 py-0">
        <pre
          className={cn(
            maxHeightClassName,
            "overflow-auto whitespace-pre-wrap break-words px-2.5 py-2 font-mono text-[11px] leading-snug",
            tone === "muted" ? "text-muted-foreground" : "text-ink",
            preClassName,
          )}
        >
          {children}
        </pre>
      </CardContent>
    </Card>
  )
}
