/**
 * Normalize filesystem paths for stable comparison across Win/Mac and
 * extended-length (`\\?\`) Windows prefixes.
 */
export function normalizePathKey(path: string): string {
  if (!path) return ""
  let normalized = path.trim().replace(/\\/g, "/")
  if (/^\/\/\?\//i.test(normalized)) {
    normalized = normalized.slice(4)
  }
  if (/^[a-zA-Z]:\//.test(normalized)) {
    normalized = normalized[0].toLowerCase() + normalized.slice(1)
  }
  return normalized.replace(/\/+$/g, "")
}

export function pathKeysEqual(a: string, b: string): boolean {
  return normalizePathKey(a) === normalizePathKey(b)
}

/**
 * Human-readable path for UI (strips `\\?\`, uses native separators).
 */
export function formatDisplayPath(path: string): string {
  if (!path) return ""
  let display = path.trim()
  if (display.startsWith("\\\\?\\")) {
    display = display.slice(4)
  } else if (display.startsWith("\\\\?/")) {
    display = display.slice(4)
  }
  const isWindowsPath = /^[a-zA-Z]:[\\/]/.test(display) || display.startsWith("\\\\")
  const separator = isWindowsPath ? "\\" : "/"
  return display.replace(/\//g, separator).replace(/\\+/g, separator)
}
