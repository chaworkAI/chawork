import { useCallback } from "react"

import { resolveLabel } from "@/lib/resolveLabel"
import { useDomainStore } from "@/stores/domain"
import { useLocaleStore } from "@/stores/locale"

/** Reactive UI label helper — re-renders when locale or domain pack changes. */
export function useUiLabel() {
  const locale = useLocaleStore((s) => s.locale)
  const packLabels = useDomainStore((s) => s.pack?.labels as Record<string, unknown> | undefined)

  return useCallback(
    (path: string, fallback: string) => resolveLabel(path, fallback, locale, packLabels),
    [locale, packLabels],
  )
}
