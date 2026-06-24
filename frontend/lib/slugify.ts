/**
 * Slug for employee IDs and similar kebab identifiers.
 * Preserves Unicode letters/numbers (CJK, kana, etc.).
 */
export function slugify(name: string): string {
  let slug = ""
  let lastWasSep = false

  for (const ch of name.trim()) {
    if (/\p{L}|\p{N}/u.test(ch)) {
      slug += /[a-zA-Z]/.test(ch) ? ch.toLowerCase() : ch
      lastWasSep = false
    } else if (!lastWasSep && slug.length > 0) {
      slug += "-"
      lastWasSep = true
    }
  }

  const trimmed = slug.replace(/-+$/g, "")
  return trimmed.length > 0 ? trimmed : "untitled"
}

/** Employee ID: ASCII kebab-case or CJK alphanumeric segments separated by hyphens. */
export const EMPLOYEE_ID_PATTERN = /^[\p{L}\p{N}]+(-[\p{L}\p{N}]+)*$/u

export function isValidEmployeeId(id: string): boolean {
  if (!id || id.startsWith("-") || id.endsWith("-")) return false
  if (!EMPLOYEE_ID_PATTERN.test(id)) return false
  for (const ch of id) {
    if (ch >= "A" && ch <= "Z") return false
  }
  return true
}
