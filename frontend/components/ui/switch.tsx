import { useId } from "react"

export function Switch({
  checked,
  onCheckedChange,
  disabled = false,
  id,
  "aria-label": ariaLabel,
}: {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  disabled?: boolean
  id?: string
  "aria-label"?: string
}) {
  const autoId = useId()
  const switchId = id ?? autoId

  return (
    <button
      id={switchId}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={[
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/25 disabled:cursor-not-allowed disabled:opacity-50",
        checked ? "bg-[#2457b7]" : "bg-[#d5d9e0]",
      ].join(" ")}
    >
      <span
        className={[
          "pointer-events-none block size-5 rounded-full bg-white shadow-sm transition",
          checked ? "translate-x-[22px]" : "translate-x-[2px]",
        ].join(" ")}
      />
    </button>
  )
}
