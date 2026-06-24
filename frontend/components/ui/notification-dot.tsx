import { cn } from "@/lib/utils"

export interface NotificationDotProps {
  className?: string
  /** Screen-reader label when the dot conveys status on its own. */
  label?: string
}

export function NotificationDot({ className, label }: NotificationDotProps) {
  return (
    <span
      className={cn(
        "pointer-events-none absolute size-2 rounded-full bg-danger ring-2 ring-[rgba(255,252,246,0.95)]",
        className,
      )}
      aria-hidden={label ? undefined : true}
      aria-label={label}
      role={label ? "status" : undefined}
    />
  )
}
