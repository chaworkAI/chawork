use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use reqwest::header::{ACCEPT, ACCEPT_ENCODING};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

// ─── 公开类型 ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct GithubSkillPreview {
    pub path: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubDownloadResult {
    pub skill_id: String,
    pub installed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubBulkDownloadResult {
    pub installed_count: u32,
    pub skill_ids: Vec<String>,
    pub failed: Vec<GithubDownloadResult>,
}

// ─── 工具函数 ────────────────────────────────────────────────────

const CACHE_READY_FILE: &str = ".chawork-github-import-ready";
const CACHE_LOCK_DIR: &str = ".lock";
const CACHE_LOCK_STALE_AFTER: Duration = Duration::from_secs(30 * 60);

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("chawork/1.0")
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("创建 HTTP client 失败: {e}"))
}

fn validate_zip_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() >= 2 && bytes[0..2] == *b"PK" {
        return Ok(());
    }
    let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(160)]);
    let compact = preview.chars().take(80).collect::<String>();
    Err(format!(
        "GitHub 返回的不是有效 zip 文件（可能是私有仓库、速率限制或网络代理干扰）。响应开头: {compact}"
    ))
}

fn validate_zip_file(path: &Path) -> Result<(), String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("打开 zip 文件失败 ({}): {e}", path.display()))?;
    let mut preview = [0u8; 160];
    let read = file
        .read(&mut preview)
        .map_err(|e| format!("读取 zip 文件失败 ({}): {e}", path.display()))?;
    validate_zip_bytes(&preview[..read])
}

async fn write_zip_response(response: reqwest::Response, dest: &Path) -> Result<PathBuf, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let preview = body.chars().take(120).collect::<String>();
        return Err(format!(
            "GitHub 仓库下载失败: HTTP {status}。请确认分支名正确、仓库为公开。{preview}"
        ));
    }
    let zip_path = dest.join("repo.zip");
    let tmp_path = dest.join(format!(".repo-{}.zip.download", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("创建 zip 文件失败: {e}"))?;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("下载 zip 数据失败: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入 zip 文件失败: {e}"))?;
    }
    file.flush()
        .await
        .map_err(|e| format!("刷新 zip 文件失败: {e}"))?;
    drop(file);
    if let Err(error) = validate_zip_file(&tmp_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(error);
    }
    std::fs::rename(&tmp_path, &zip_path).map_err(|e| format!("保存 zip 文件失败: {e}"))?;
    Ok(zip_path)
}

/// 解析 GitHub URL 为 (owner, repo)
pub fn parse_github_url(url: &str) -> Result<(String, String), String> {
    let trimmed = url.trim().trim_end_matches('/');
    let marker = "github.com/";
    let path = trimmed
        .split(marker)
        .nth(1)
        .ok_or_else(|| "无效的 GitHub 仓库地址".to_string())?;
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if segments.len() < 2 {
        return Err("GitHub 仓库地址缺少 owner/repo".to_string());
    }
    let owner = segments[0].to_string();
    let repo = segments[1].trim_end_matches(".git").to_string();
    Ok((owner, repo))
}

/// 从路径推断 skill_id（目录名）
pub fn skill_id_from_path(skill_md_path: &str) -> String {
    let parent = Path::new(skill_md_path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("skill");
    sanitize_id(parent)
}

fn validate_skill_md_relative_path(skill_md_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(skill_md_path);
    if path.is_absolute() {
        return Err(format!("技能路径必须是相对路径: {skill_md_path}"));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("技能路径越界: {skill_md_path}"));
            }
        }
    }
    if normalized.file_name().and_then(|v| v.to_str()) != Some("SKILL.md") {
        return Err(format!("技能路径必须指向 SKILL.md: {skill_md_path}"));
    }
    if normalized.parent().is_none() {
        return Err(format!("技能路径缺少技能目录: {skill_md_path}"));
    }
    Ok(normalized)
}

fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            ch if ch.is_control() => '-',
            ch => ch,
        })
        .collect::<String>()
}

/// 解析 SKILL.md 的 YAML frontmatter
fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>) {
    let t = content.trim_start();
    if !t.starts_with("---") {
        return (None, None);
    }
    let mut lines = t.lines();
    lines.next(); // skip opening ---
    let mut yaml_buf = String::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        if !yaml_buf.is_empty() {
            yaml_buf.push('\n');
        }
        yaml_buf.push_str(line);
    }
    if !closed || yaml_buf.trim().is_empty() {
        return (None, None);
    }

    let yaml: serde_yaml::Value = match serde_yaml::from_str(&yaml_buf) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };

    let name = yaml
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let description = yaml
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    (name, description)
}

// ─── zipball 下载（不走 GitHub API，无 token 依赖）──────────────

/// 创建临时目录
fn create_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).map_err(|e| format!("创建临时目录失败: {e}"))?;
    Ok(path)
}

fn github_cache_key(owner: &str, repo: &str, git_ref: Option<&str>) -> String {
    let ref_key = git_ref
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("__default__");
    let raw = format!("{owner}/{repo}@{ref_key}");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    format!(
        "{}-{}-{}",
        sanitize_id(owner),
        sanitize_id(repo),
        &hash[..12]
    )
}

fn github_import_cache_dir(
    cache_root: &Path,
    owner: &str,
    repo: &str,
    git_ref: Option<&str>,
) -> PathBuf {
    cache_root
        .join("github-import")
        .join(github_cache_key(owner, repo, git_ref))
}

struct CacheLock {
    path: PathBuf,
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn acquire_cache_lock(cache_dir: &Path) -> Result<CacheLock, String> {
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("创建 GitHub 导入缓存失败: {e}"))?;
    let lock_path = cache_dir.join(CACHE_LOCK_DIR);
    for _ in 0..600 {
        match std::fs::create_dir(&lock_path) {
            Ok(()) => return Ok(CacheLock { path: lock_path }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = std::fs::metadata(&lock_path)
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                    .is_some_and(|age| age > CACHE_LOCK_STALE_AFTER);
                if stale {
                    let _ = std::fs::remove_dir_all(&lock_path);
                    continue;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(error) => return Err(format!("创建 GitHub 导入缓存锁失败: {error}")),
        }
    }
    Err("等待 GitHub 导入缓存锁超时，请稍后重试".to_string())
}

fn extracted_repo_root(extract_dir: &Path) -> Result<PathBuf, String> {
    let entries: Vec<_> = std::fs::read_dir(extract_dir)
        .map_err(|e| format!("读取解压目录失败: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
        .collect();

    if entries.len() == 1 {
        Ok(entries[0].path())
    } else {
        Ok(extract_dir.to_path_buf())
    }
}

fn cached_extracted_repo(cache_dir: &Path) -> Result<Option<PathBuf>, String> {
    if !cache_dir.join(CACHE_READY_FILE).is_file() {
        return Ok(None);
    }
    let extract_dir = cache_dir.join("extracted");
    if !extract_dir.is_dir() {
        return Ok(None);
    }
    let extracted = extracted_repo_root(&extract_dir)?;
    if extracted.is_dir() && !find_skill_md_files(&extracted, &extracted).is_empty() {
        return Ok(Some(extracted));
    }
    Ok(None)
}

async fn cached_or_downloaded_repo(
    owner: &str,
    repo: &str,
    git_ref: Option<&str>,
    cache_root: Option<&Path>,
    temp_prefix: &str,
) -> Result<(Option<PathBuf>, PathBuf), String> {
    if let Some(cache_root) = cache_root {
        let cache_dir = github_import_cache_dir(cache_root, owner, repo, git_ref);
        let _lock = acquire_cache_lock(&cache_dir)?;
        if let Some(extracted) = cached_extracted_repo(&cache_dir)? {
            return Ok((None, extracted));
        }

        let zip_path = cache_dir.join("repo.zip");
        if !zip_path.is_file() || validate_zip_file(&zip_path).is_err() {
            let _ = std::fs::remove_file(&zip_path);
            let downloaded = download_zipball(owner, repo, git_ref, &cache_dir).await?;
            if downloaded != zip_path {
                std::fs::rename(&downloaded, &zip_path)
                    .map_err(|e| format!("保存 GitHub zip 缓存失败: {e}"))?;
            }
        }

        let stage_dir = cache_dir.join(format!(".extracted-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&stage_dir).map_err(|e| format!("创建解压目录失败: {e}"))?;
        if let Err(error) = extract_zip(&zip_path, &stage_dir) {
            let _ = std::fs::remove_dir_all(&stage_dir);
            let _ = std::fs::remove_file(&zip_path);
            return Err(error);
        }

        let final_extract_dir = cache_dir.join("extracted");
        let ready_path = cache_dir.join(CACHE_READY_FILE);
        let _ = std::fs::remove_file(&ready_path);
        if final_extract_dir.exists() {
            let _ = std::fs::remove_dir_all(&final_extract_dir);
        }
        std::fs::rename(&stage_dir, &final_extract_dir)
            .map_err(|e| format!("保存解压缓存失败: {e}"))?;
        std::fs::write(&ready_path, b"ready\n")
            .map_err(|e| format!("写入 GitHub 导入缓存标记失败: {e}"))?;
        let extracted = cached_extracted_repo(&cache_dir)?
            .ok_or_else(|| "GitHub 导入缓存写入后无法读取".to_string())?;
        return Ok((None, extracted));
    }

    let tmp = create_temp_dir(temp_prefix)?;
    let zip_path = download_zipball(owner, repo, git_ref, &tmp).await?;
    let extract_dir = tmp.join("extracted");
    std::fs::create_dir_all(&extract_dir).map_err(|e| format!("创建解压目录失败: {e}"))?;
    let extracted = extract_zip(&zip_path, &extract_dir)?;
    Ok((Some(tmp), extracted))
}

/// 下载 GitHub zipball（不走 GitHub REST API，无需 token/git）
/// URL: https://github.com/{owner}/{repo}/archive/refs/heads/{branch}.zip
async fn download_zipball(
    owner: &str,
    repo: &str,
    git_ref: Option<&str>,
    dest: &Path,
) -> Result<PathBuf, String> {
    let client = http_client()?;
    let branches: Vec<String> = if let Some(branch) = git_ref {
        vec![branch.to_string()]
    } else {
        vec!["main".to_string(), "master".to_string()]
    };

    let mut last_error = String::new();
    for branch in branches {
        let url = format!("https://github.com/{owner}/{repo}/archive/refs/heads/{branch}.zip");
        let response = match client
            .get(&url)
            .header(ACCEPT, "application/zip, application/octet-stream")
            .header(ACCEPT_ENCODING, "identity")
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                last_error = format!("下载仓库 zip 失败: {error}");
                continue;
            }
        };
        match write_zip_response(response, dest).await {
            Ok(zip_path) => return Ok(zip_path),
            Err(error) => last_error = error,
        }
    }

    if last_error.is_empty() {
        Err("GitHub 仓库下载失败: 未找到可用分支".to_string())
    } else {
        Err(last_error)
    }
}

/// 解压 zip 到目标目录，返回解压后的根目录
fn extract_zip(zip_path: &Path, dest: &Path) -> Result<PathBuf, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("打开 zip 文件失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("解析 zip 文件失败: {e}"))?;

    archive
        .extract(dest)
        .map_err(|e| format!("解压 zip 失败: {e}"))?;

    // GitHub zipball 解压后只有一个顶层目录，如 "repo-branch/"
    extracted_repo_root(dest)
}

/// 递归查找目录下所有 SKILL.md，返回相对于根目录的路径
fn find_skill_md_files(dir: &Path, base: &Path) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // 跳过隐藏目录和 node_modules
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.') || name == "node_modules" || name == ".git" {
                    continue;
                }
                results.extend(find_skill_md_files(&path, base));
            } else if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                if let Ok(relative) = path.strip_prefix(base) {
                    results.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }
    results
}

/// 从解压目录读取单个 SKILL.md 的内容
fn read_skill_content(extracted: &Path, skill_md_path: &str) -> Result<String, String> {
    let path = extracted.join(validate_skill_md_relative_path(skill_md_path)?);
    std::fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败: {e}", skill_md_path))
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("创建目录失败 ({}): {e}", dest.display()))?;
    for entry in
        std::fs::read_dir(src).map_err(|e| format!("读取目录失败 ({}): {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败 ({}): {e}", src_path.display()))?;
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dest_path).map_err(|e| {
                format!(
                    "复制技能文件失败 ({} -> {}): {e}",
                    src_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

/// 从 zip 解压目录安装单个技能到 root skills 目录
fn install_skill_from_zip(
    extracted: &Path,
    skill_md_path: &str,
    root_skills_dir: &Path,
    repo_url: &str,
) -> Result<GithubDownloadResult, String> {
    let skill_id = skill_id_from_path(skill_md_path);
    let relative_skill_path = validate_skill_md_relative_path(skill_md_path)?;
    let skill_md = extracted.join(&relative_skill_path);
    if !skill_md.is_file() {
        return Err(format!("技能入口不存在: {skill_md_path}"));
    }
    let canonical_extracted = extracted
        .canonicalize()
        .map_err(|e| format!("读取解压目录失败 ({}): {e}", extracted.display()))?;
    let canonical_skill_md = skill_md
        .canonicalize()
        .map_err(|e| format!("读取技能入口失败 ({}): {e}", skill_md.display()))?;
    if !canonical_skill_md.starts_with(&canonical_extracted) {
        return Err(format!("技能路径越界: {skill_md_path}"));
    }
    let source_skill_dir = skill_md
        .parent()
        .ok_or_else(|| format!("技能路径缺少父目录: {skill_md_path}"))?;

    let skill_dir = root_skills_dir.join(&skill_id);
    std::fs::create_dir_all(root_skills_dir)
        .map_err(|e| format!("创建 Root skills 目录失败: {e}"))?;
    let temp_skill_dir = root_skills_dir.join(format!(
        ".{skill_id}.github-import-{}",
        uuid::Uuid::new_v4()
    ));
    if temp_skill_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_skill_dir);
    }
    copy_dir_recursive(source_skill_dir, &temp_skill_dir)?;

    // 写入 origin 文件标记来源
    let (owner, _) = parse_github_url(repo_url).unwrap_or_default();
    let origin = serde_json::json!({
        "kind": "skill",
        "hub_url": "github",
        "hub_id": format!("github--{}--{}", owner, skill_id),
        "local_id": skill_id,
        "content_hash": "",
        "hub_updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "installed_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "skill_hub_ids": [],
        "source": {
            "type": "github",
            "repo": repo_url,
            "path": skill_md_path
        }
    });
    let origin_path = temp_skill_dir.join(".hub_origin.json");
    std::fs::write(
        &origin_path,
        serde_json::to_string_pretty(&origin).unwrap_or_default(),
    )
    .map_err(|e| format!("写入 .hub_origin.json 失败: {e}"))?;

    let backup_skill_dir = root_skills_dir.join(format!(
        ".{skill_id}.github-backup-{}",
        uuid::Uuid::new_v4()
    ));
    let had_existing = skill_dir.exists();
    if had_existing {
        std::fs::rename(&skill_dir, &backup_skill_dir)
            .map_err(|e| format!("备份旧技能失败: {e}"))?;
    }
    if let Err(error) = std::fs::rename(&temp_skill_dir, &skill_dir) {
        if had_existing {
            let _ = std::fs::rename(&backup_skill_dir, &skill_dir);
        }
        let _ = std::fs::remove_dir_all(&temp_skill_dir);
        return Err(format!("安装技能失败: {error}"));
    }
    if had_existing {
        let _ = std::fs::remove_dir_all(&backup_skill_dir);
    }

    Ok(GithubDownloadResult {
        skill_id,
        installed: true,
        error: None,
    })
}

// ─── 核心功能 ────────────────────────────────────────────────────

/// 扫描 GitHub 仓库中的所有技能
/// 通过下载 zipball（1 次 HTTP 请求，不走 GitHub API，无需 git/token）
pub async fn scan_github_repo(
    url: &str,
    git_ref: Option<&str>,
    cache_root: Option<&Path>,
) -> Result<Vec<GithubSkillPreview>, String> {
    let (owner, repo) = parse_github_url(url)?;

    // 1. 准备仓库归档。预览阶段写入 root cache，后续导入复用同一份解压目录。
    let (tmp, extracted) =
        cached_or_downloaded_repo(&owner, &repo, git_ref, cache_root, "chawork-zip-scan").await?;

    // 2. 查找所有 SKILL.md 并读取内容
    let mut skill_paths = find_skill_md_files(&extracted, &extracted);
    skill_paths.sort();

    if skill_paths.is_empty() {
        if let Some(tmp) = tmp {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        return Err(
            "未在仓库中找到 SKILL.md 文件。请确认仓库中包含以 SKILL.md 为入口的技能目录。"
                .to_string(),
        );
    }

    let mut previews = Vec::new();
    for path in &skill_paths {
        if let Ok(content) = read_skill_content(&extracted, path) {
            let (name, description) = parse_frontmatter(&content);
            let skill_id = skill_id_from_path(path);
            previews.push(GithubSkillPreview {
                path: path.clone(),
                name: name.unwrap_or_else(|| skill_id.clone()),
                description: description.unwrap_or_default(),
            });
        }
    }

    if let Some(tmp) = tmp {
        let _ = std::fs::remove_dir_all(&tmp);
    }

    Ok(previews)
}

/// 从 GitHub 下载并安装技能到 root skills 目录
pub async fn download_and_install_skills(
    url: &str,
    skill_paths: &[String],
    git_ref: Option<&str>,
    root_skills_dir: &Path,
    cache_root: Option<&Path>,
) -> Result<GithubBulkDownloadResult, String> {
    let (owner, repo) = parse_github_url(url)?;

    // 1. 使用预览阶段缓存；缓存缺失时才下载并解压。
    let (tmp, extracted) =
        cached_or_downloaded_repo(&owner, &repo, git_ref, cache_root, "chawork-zip-install")
            .await?;

    // 2. 安装选中的技能
    let mut installed_count = 0u32;
    let mut skill_ids = Vec::new();
    let mut failed = Vec::new();

    for path in skill_paths {
        match install_skill_from_zip(&extracted, path, root_skills_dir, url) {
            Ok(result) => {
                skill_ids.push(result.skill_id.clone());
                installed_count += 1;
            }
            Err(e) => {
                failed.push(GithubDownloadResult {
                    skill_id: skill_id_from_path(path),
                    installed: false,
                    error: Some(e),
                });
            }
        }
    }

    if let Some(tmp) = tmp {
        let _ = std::fs::remove_dir_all(&tmp);
    }

    Ok(GithubBulkDownloadResult {
        installed_count,
        skill_ids,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        cached_extracted_repo, github_cache_key, install_skill_from_zip,
        validate_skill_md_relative_path, validate_zip_bytes, CACHE_READY_FILE,
    };

    #[test]
    fn validate_zip_bytes_accepts_zip_magic() {
        validate_zip_bytes(b"PK\x03\x04").expect("zip magic should pass");
    }

    #[test]
    fn validate_zip_bytes_rejects_html_error_page() {
        let err = validate_zip_bytes(b"<!DOCTYPE html><html>").expect_err("html should fail");
        assert!(err.contains("不是有效 zip 文件"));
    }

    #[test]
    fn github_cache_key_distinguishes_refs() {
        let main = github_cache_key("nexu-io", "open-design", Some("main"));
        let master = github_cache_key("nexu-io", "open-design", Some("master"));
        assert_ne!(main, master);
        assert!(main.starts_with("nexu-io-open-design-"));
    }

    #[test]
    fn cached_extracted_repo_reuses_preview_extract_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp
            .path()
            .join("extracted")
            .join("open-design-main")
            .join("plugins/spec/examples/demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), "---\nname: demo\n---\n").expect("write skill");
        std::fs::write(temp.path().join(CACHE_READY_FILE), "ready").expect("write ready marker");

        let cached = cached_extracted_repo(temp.path())
            .expect("cache lookup")
            .expect("cache should be reusable");
        assert_eq!(
            cached.file_name().and_then(|v| v.to_str()),
            Some("open-design-main")
        );
    }

    #[test]
    fn cached_extracted_repo_ignores_unmarked_partial_extract() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join("extracted").join("repo").join("demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), "---\nname: demo\n---\n").expect("write skill");

        let cached = cached_extracted_repo(temp.path()).expect("cache lookup");
        assert!(cached.is_none());
    }

    #[test]
    fn validate_skill_md_relative_path_rejects_escape_paths() {
        assert!(validate_skill_md_relative_path("plugins/demo/SKILL.md").is_ok());
        assert!(validate_skill_md_relative_path("../demo/SKILL.md").is_err());
        assert!(validate_skill_md_relative_path("/tmp/demo/SKILL.md").is_err());
        assert!(validate_skill_md_relative_path("plugins/demo/README.md").is_err());
    }

    #[test]
    fn install_skill_from_zip_copies_entire_skill_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let extracted = temp.path().join("repo");
        let source = extracted.join("plugins/spec/examples/demo");
        std::fs::create_dir_all(source.join("assets")).expect("create asset dir");
        std::fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").expect("write skill");
        std::fs::write(source.join("assets/seed-brief.md"), "brief").expect("write asset");
        let root_skills = temp.path().join("root-skills");
        std::fs::create_dir_all(&root_skills).expect("create root skills");

        install_skill_from_zip(
            &extracted,
            "plugins/spec/examples/demo/SKILL.md",
            &root_skills,
            "https://github.com/nexu-io/open-design",
        )
        .expect("install skill");

        assert!(root_skills.join("demo/SKILL.md").is_file());
        assert!(root_skills.join("demo/assets/seed-brief.md").is_file());
        assert!(root_skills.join("demo/.hub_origin.json").is_file());
    }

    #[test]
    fn install_skill_from_zip_rejects_escape_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let extracted = temp.path().join("repo");
        std::fs::create_dir_all(&extracted).expect("create extracted");
        let root_skills = temp.path().join("root-skills");

        let err = install_skill_from_zip(
            &extracted,
            "../outside/SKILL.md",
            &root_skills,
            "https://github.com/nexu-io/open-design",
        )
        .expect_err("escape path should fail");

        assert!(err.contains("越界"));
    }
}
