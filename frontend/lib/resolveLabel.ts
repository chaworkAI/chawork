import { BUILTIN_UI_LABELS } from "@/lib/builtinLabels"
import { EN_US_UI_LABELS } from "@/lib/localeLabels/en-US"
import type { AppLocale } from "@/types/locale"

function labelAtPath(
  labels: Record<string, unknown> | null | undefined,
  path: string,
): string | undefined {
  if (!labels) return undefined
  const parts = path.split(".").filter(Boolean)
  let current: unknown = labels
  for (const key of parts) {
    if (current === null || current === undefined || typeof current !== "object") {
      return undefined
    }
    current = (current as Record<string, unknown>)[key]
  }
  return typeof current === "string" ? current : undefined
}

export function resolveLabel(
  path: string,
  fallback: string,
  locale: AppLocale,
  packLabels?: Record<string, unknown> | null,
): string {
  const fromPack = labelAtPath(packLabels, path)
  if (fromPack !== undefined && fromPack !== "") return fromPack

  if (locale === "en-US") {
    const en = EN_US_UI_LABELS[path]
    if (en !== undefined && en !== "") return en
  }

  const builtin = BUILTIN_UI_LABELS[path]
  if (builtin !== undefined && builtin !== "") return builtin

  return fallback
}
