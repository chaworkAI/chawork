use std::path::PathBuf;

use reqwest::Client;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

/// 返回当前平台信息供前端 OTA 检查使用
#[tauri::command]
pub async fn get_platform_info() -> Result<serde_json::Value, String> {
    let target = current_target();
    let arch = current_arch();
    Ok(serde_json::json!({ "target": target, "arch": arch }))
}

/// 下载 OTA 文件到临时目录，可选 SHA-256 校验
#[tauri::command]
pub async fn download_ota_file(
    app: AppHandle,
    url: String,
    expected_hash: Option<String>,
) -> Result<String, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    let download_dir = app_data.join("hot-update").join("patches");
    std::fs::create_dir_all(&download_dir)
        .map_err(|e| format!("Failed to create download dir: {e}"))?;

    let filename = url
        .split('/')
        .last()
        .unwrap_or("ota_download")
        .split('?')
        .next()
        .unwrap_or("ota_download");
    let dest_path = download_dir.join(filename);

    let client = Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with status: {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    if let Some(ref expected) = expected_hash {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());
        if actual != *expected {
            return Err(format!(
                "Hash mismatch: expected {expected}, got {actual}"
            ));
        }
    }

    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create file: {e}"))?;
    file.write_all(&bytes)
        .await
        .map_err(|e| format!("Failed to write file: {e}"))?;

    Ok(dest_path.to_string_lossy().to_string())
}

/// 应用 bsdiff 热更新补丁
#[tauri::command]
pub async fn apply_hot_patch(
    app: AppHandle,
    patch_path: String,
    current_version: String,
) -> Result<String, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    let hot_update_dir = app_data.join("hot-update");
    let current_dir = hot_update_dir.join("current");
    let backup_dir = hot_update_dir.join("backup");

    let old_bundle_path = resolve_current_bundle(&hot_update_dir, &app)?;

    let old_bundle =
        std::fs::read(&old_bundle_path).map_err(|e| format!("Failed to read old bundle: {e}"))?;
    let patch =
        std::fs::read(&patch_path).map_err(|e| format!("Failed to read patch file: {e}"))?;

    let mut new_bundle = Vec::new();
    qbsdiff::Bspatch::new(&patch)
        .map_err(|e| format!("Invalid patch format: {e}"))?
        .apply(&old_bundle, &mut new_bundle)
        .map_err(|e| format!("Patch apply failed: {e}"))?;

    // 备份当前版本
    if current_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
        std::fs::rename(&current_dir, &backup_dir)
            .map_err(|e| format!("Backup failed: {e}"))?;
    }

    // 解压新 bundle 到 current/
    std::fs::create_dir_all(&current_dir)
        .map_err(|e| format!("Failed to create current dir: {e}"))?;

    extract_tar_gz(&new_bundle, &current_dir)?;

    // 写入版本标记
    std::fs::write(
        hot_update_dir.join("version.txt"),
        current_version.as_bytes(),
    )
    .map_err(|e| format!("Failed to write version marker: {e}"))?;

    // 清理补丁文件
    let _ = std::fs::remove_file(&patch_path);

    Ok("success".to_string())
}

/// 全量前端包替换（热更新 fallback）
#[tauri::command]
pub async fn apply_full_frontend_bundle(app: AppHandle, bundle_path: String) -> Result<String, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    let hot_update_dir = app_data.join("hot-update");
    let current_dir = hot_update_dir.join("current");
    let backup_dir = hot_update_dir.join("backup");

    // 备份
    if current_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
        let _ = std::fs::rename(&current_dir, &backup_dir);
    }

    std::fs::create_dir_all(&current_dir)
        .map_err(|e| format!("Failed to create current dir: {e}"))?;

    let bundle_bytes =
        std::fs::read(&bundle_path).map_err(|e| format!("Failed to read bundle: {e}"))?;

    extract_tar_gz(&bundle_bytes, &current_dir)?;

    let _ = std::fs::remove_file(&bundle_path);
    Ok("success".to_string())
}

/// 重置热更新崩溃计数（前端加载成功后调用）
#[tauri::command]
pub async fn ota_mark_healthy(app: AppHandle) -> Result<(), String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    let crash_file = app_data.join("hot-update").join(".crash_count");
    let _ = std::fs::write(&crash_file, "0");
    Ok(())
}

// ─── Internal helpers ────────────────────────────────────────

fn current_target() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let arch = current_arch();
    format!("{os}-{arch}")
}

fn current_arch() -> String {
    if cfg!(target_arch = "aarch64") {
        "aarch64".to_string()
    } else if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else {
        std::env::consts::ARCH.to_string()
    }
}

/// 定位当前生效的前端资源包（tar.gz 格式）
fn resolve_current_bundle(hot_update_dir: &PathBuf, _app: &AppHandle) -> Result<PathBuf, String> {
    let bundle_path = hot_update_dir.join("current.tar.gz");
    if bundle_path.exists() {
        return Ok(bundle_path);
    }

    // 如果没有 hot-update 包，尝试从已解压的 current/ 目录重新打包
    let current_dir = hot_update_dir.join("current");
    if current_dir.exists() {
        create_tar_gz(&current_dir, &bundle_path)?;
        return Ok(bundle_path);
    }

    Err("No existing frontend bundle found for patching. First hot-update requires full download.".to_string())
}

fn extract_tar_gz(data: &[u8], dest: &PathBuf) -> Result<(), String> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(dest)
        .map_err(|e| format!("Failed to extract tar.gz: {e}"))
}

fn create_tar_gz(source_dir: &PathBuf, dest: &PathBuf) -> Result<(), String> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tar::Builder;

    let file = std::fs::File::create(dest).map_err(|e| format!("Failed to create tar.gz: {e}"))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);
    tar.append_dir_all(".", source_dir)
        .map_err(|e| format!("Failed to build tar: {e}"))?;
    tar.finish()
        .map_err(|e| format!("Failed to finish tar: {e}"))?;
    Ok(())
}
