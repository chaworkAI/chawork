export interface GithubUrlValidation {
  normalized: string | null
  formatOk: boolean
  message: string | null
}

const GITHUB_HOSTS = new Set(["github.com", "www.github.com"])

export function normalizeGithubRepoUrl(raw: string): string | null {
  let value = raw.trim()
  if (!value) return null

  if (!/^https?:\/\//i.test(value)) {
    if (/^github\.com\//i.test(value)) {
      value = `https://${value}`
    } else if (/^[\w.-]+\/[\w.-]+/.test(value)) {
      value = `https://github.com/${value.replace(/^\/+/, "")}`
    } else {
      return null
    }
  }

  let parsed: URL
  try {
    parsed = new URL(value)
  } catch {
    return null
  }

  const host = parsed.hostname.toLowerCase()
  if (!GITHUB_HOSTS.has(host)) return null

  const segments = parsed.pathname.split("/").filter(Boolean)
  if (segments.length < 2) return null

  const owner = segments[0]?.trim()
  let repo = segments[1]?.trim().replace(/\.git$/i, "")
  if (!owner || !repo || owner === "." || repo === ".") return null

  return `https://github.com/${owner}/${repo}`
}

export function validateGithubRepoUrl(raw: string): GithubUrlValidation {
  const trimmed = raw.trim()
  if (!trimmed) {
    return {
      normalized: null,
      formatOk: false,
      message: null,
    }
  }

  const normalized = normalizeGithubRepoUrl(trimmed)
  if (!normalized) {
    return {
      normalized: null,
      formatOk: false,
      message:
        "请输入有效的 GitHub 仓库地址，例如 https://github.com/owner/repo 或 owner/repo",
    }
  }

  return {
    normalized,
    formatOk: true,
    message: null,
  }
}

export function githubRepoNameFromUrl(url: string) {
  const normalized = normalizeGithubRepoUrl(url) ?? url.trim().replace(/\/$/, "")
  const segments = normalized.split("/").filter(Boolean)
  return segments[segments.length - 1]?.replace(/\.git$/i, "") ?? "repo"
}

/** GitHub 仓库 owner，例如 https://github.com/slavingia/skills → slavingia */
export function githubOwnerNameFromUrl(url: string) {
  const normalized = normalizeGithubRepoUrl(url) ?? url.trim().replace(/\/$/, "")
  const segments = normalized.split("/").filter(Boolean)
  return segments[segments.length - 2] ?? "owner"
}

/** Hub catalog 中 GitHub 来源的 repo 键，例如 slavingia/skills → slavingia-skills */
export function githubHubRepoKeyFromUrl(url: string): string | null {
  const normalized = normalizeGithubRepoUrl(url)
  if (!normalized) return null
  const segments = normalized.split("/").filter(Boolean)
  const owner = segments[segments.length - 2]
  const repo = segments[segments.length - 1]?.replace(/\.git$/i, "")
  if (!owner || !repo) return null
  return `${owner}-${repo}`
}

const TERMINAL_GITHUB_IMPORT_STATUSES = new Set([
  "done",
  "completed",
  "failed",
  "error",
  "cancelled",
  "canceled",
])

export function isTerminalGithubImportJobStatus(status: string) {
  return TERMINAL_GITHUB_IMPORT_STATUSES.has(status.trim().toLowerCase())
}

export function githubImportJobStatusLabel(status: string) {
  const normalized = status.trim().toLowerCase()
  if (normalized === "syncing") return "正在扫描仓库…"
  if (normalized === "translating") return "正在翻译技能元数据…"
  if (normalized === "done" || normalized === "completed") return "扫描完成"
  if (normalized === "failed" || normalized === "error") return "导入失败"
  return "正在处理…"
}

export function formatGithubImportJobError(error: string | null | undefined, url: string) {
  const detail = error?.trim() ?? ""
  const lower = detail.toLowerCase()

  if (
    lower.includes("not found") ||
    lower.includes("repository not found") ||
    lower.includes("could not read username") ||
    lower.includes("fatal:")
  ) {
    return `无法访问 GitHub 仓库 ${url}，请确认 owner/repo 是否正确、仓库为公开且地址可复制访问。`
  }

  if (detail) {
    const firstLine = detail.split("\n").find((line) => line.trim()) ?? detail
    return firstLine.length > 180 ? `${firstLine.slice(0, 180)}…` : firstLine
  }

  return `GitHub 仓库 ${url} 导入失败，请检查地址后重试。`
}
