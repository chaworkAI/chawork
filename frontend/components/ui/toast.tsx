import { CheckCircle2, XCircle, Info, TriangleAlert, X } from "lucide-react"

import { useToastStore, type ToastVariant } from "@/stores/toast"

const variantStyles: Record<ToastVariant, string> = {
  success:
    "border-success/30 bg-success/10 text-success",
  error:
    "border-danger/30 bg-danger/10 text-danger",
  info:
    "border-line bg-panel text-ink",
  warning:
    "border-warning/30 bg-warning/10 text-warning",
}

const variantIcons: Record<ToastVariant, typeof CheckCircle2> = {
  success: CheckCircle2,
  error: XCircle,
  info: Info,
  warning: TriangleAlert,
}

function ToastItem({
  id,
  message,
  variant,
}: {
  id: string
  message: string
  variant: ToastVariant
}) {
  const dismiss = useToastStore((s) => s.dismiss)
  const Icon = variantIcons[variant]

  return (
    <div
      className={`flex items-center gap-2.5 rounded-[12px] border px-3.5 py-2.5 shadow-panel backdrop-blur-[18px] animate-in slide-in-from-bottom-2 fade-in ${variantStyles[variant]}`}
      role="status"
      aria-live="polite"
    >
      <Icon className="size-4 shrink-0" />
      <span className="text-[13px] font-medium">{message}</span>
      <button
        type="button"
        onClick={() => dismiss(id)}
        className="ml-1 shrink-0 rounded p-0.5 opacity-60 transition-opacity hover:opacity-100"
        aria-label="关闭"
      >
        <X className="size-3.5" />
      </button>
    </div>
  )
}

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts)

  if (toasts.length === 0) return null

  return (
    <div className="fixed bottom-6 left-1/2 z-[9999] flex -translate-x-1/2 flex-col gap-2">
      {toasts.map((t) => (
        <ToastItem key={t.id} {...t} />
      ))}
    </div>
  )
}
