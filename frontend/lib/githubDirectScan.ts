import * as ipc from "@/lib/tauri"
import type { GithubSkillPreview, GithubBulkDownloadResult } from "@/types/hub"

/**
 * 直接扫描 GitHub 仓库中的技能（不依赖 Hub API）。
 * 通过 GitHub API 获取仓库文件树，筛选 SKILL.md 文件并解析 frontmatter。
 */
export async function scanGithubRepo(
  url: string,
  gitRef?: string,
): Promise<GithubSkillPreview[]> {
  return ipc.githubScanRepo(url, gitRef)
}

/**
 * 从 GitHub 直接下载并安装多个技能到 Root skills 目录。
 */
export async function downloadGithubSkills(
  url: string,
  skillPaths: string[],
  gitRef?: string,
): Promise<GithubBulkDownloadResult> {
  return ipc.githubDownloadAllSkills(url, skillPaths, gitRef)
}

/**
 * 将 GithubSkillPreview 转换为 HubGithubImportSkillPreview 格式，
 * 以保持与现有 UI 组件的兼容性。
 */
export function toHubPreview(preview: GithubSkillPreview, repoUrl: string) {
  // 从路径推断 skill_id
  const parts = preview.path.replace(/\/SKILL\.md$/, "").split("/")
  const dirName = parts[parts.length - 1] || "skill"

  return {
    id: dirName,
    name: preview.name,
    description_zh: preview.description,
    description_en: preview.description,
    profession: "",
    // 额外字段用于导入
    _githubPath: preview.path,
    _repoUrl: repoUrl,
  }
}
