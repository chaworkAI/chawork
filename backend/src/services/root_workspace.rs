//! Root workspace: ChaWork 自己的全局运行环境（DESIGN §4.2 / §5.2）。
//!
//! 默认位置 `<tauri_app_data_dir>/root/`。保存全局 provider、全局 skills、全局 templates、
//! 唯一 MCP server 与应用状态。Workspace 提供局部覆盖与 tool policy。

use std::fs;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RootWorkspace {
    base: PathBuf,
}

impl RootWorkspace {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    pub fn path(&self) -> &Path {
        &self.base
    }

    pub fn codex_home_dir(&self) -> PathBuf {
        self.base.join("codex-home")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.base.join("runtime")
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.base.join("skills")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.base.join("templates")
    }

    pub fn mcp_dir(&self) -> PathBuf {
        self.base.join("mcp")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.base.join("state")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.base.join("cache")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base.join("logs")
    }

    pub fn provider_path(&self) -> PathBuf {
        self.runtime_dir().join("provider.json")
    }

    pub fn env_path(&self) -> PathBuf {
        self.runtime_dir().join("env.json")
    }

    pub fn runtime_state_path(&self) -> PathBuf {
        self.runtime_dir().join("runtime-state.json")
    }

    pub fn known_workspaces_path(&self) -> PathBuf {
        self.state_dir().join("known-workspaces.json")
    }

    pub fn employees_dir(&self) -> PathBuf {
        self.base.join("employees")
    }

    pub fn dream_employee_dir(&self) -> PathBuf {
        self.employees_dir().join("__dream__")
    }

    pub fn employee_registry_path(&self) -> PathBuf {
        self.state_dir().join("employee-registry.json")
    }

    pub fn config_dir(&self) -> PathBuf {
        self.base.join("config")
    }

    pub fn ensure_directories(&self) -> Result<(), String> {
        ensure_root_is_safe_to_own(&self.base)?;
        for d in [
            self.codex_home_dir(),
            self.runtime_dir(),
            self.skills_dir(),
            self.templates_dir(),
            self.mcp_dir(),
            self.state_dir(),
            self.cache_dir(),
            self.logs_dir(),
            self.employees_dir(),
            self.config_dir(),
        ] {
            fs::create_dir_all(&d).map_err(|e| format!("创建 {} 失败: {e}", d.display()))?;
        }
        fs::write(self.base.join(".chawork-root"), b"ChaWork root workspace\n").map_err(|e| {
            format!(
                "写入 root workspace 标记失败 ({}): {e}",
                self.base.display()
            )
        })?;
        Ok(())
    }
}

fn ensure_root_is_safe_to_own(root: &Path) -> Result<(), String> {
    let marker = root.join(".chawork-root");
    if marker.is_file() || !root.exists() {
        return Ok(());
    }
    if !root.is_dir() {
        return Err(format!("root workspace 路径不是目录: {}", root.display()));
    }

    let mut entries = fs::read_dir(root)
        .map_err(|e| format!("读取 root workspace 目录失败 ({}): {e}", root.display()))?;
    if entries.next().is_none() {
        return Ok(());
    }

    if root.join("state").is_dir()
        && root.join("employees").is_dir()
        && root.join("runtime").is_dir()
    {
        return Ok(());
    }
    if root.join("state/known-workspaces.json").is_file() {
        return Ok(());
    }

    Err(format!(
        "root workspace 路径不是空目录，也不像已有 ChaWork root: {}",
        root.display()
    ))
}

/// 在指定安装数据目录下初始化或打开根工作区。会执行一次性迁移。
pub fn init_or_open(install_data_dir: &Path) -> Result<RootWorkspace, String> {
    let root = RootWorkspace::new(resolve_root_base(install_data_dir)?);
    root.ensure_directories()?;
    migrate_known_workspaces(install_data_dir, &root)?;
    super::employee::ensure_employee_infrastructure(&root)?;
    super::dream::migrate_dream_schedules_to_daily_once(&root)?;
    Ok(root)
}

/// Resolve the persisted root workspace base.
///
/// The default stays under Tauri's app data directory. Windows installers can
/// write `root-dir.txt` next to it to move the root workspace to a user-chosen
/// location without depending on the process working directory.
pub fn resolve_root_base(install_data_dir: &Path) -> Result<PathBuf, String> {
    if let Ok(raw) = std::env::var("CHAWORK_ROOT_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return resolve_absolute_root_dir(trimmed);
        }
    }

    #[cfg(windows)]
    {
        if let Some(trimmed) = read_root_dir_from_registry() {
            if !trimmed.is_empty() {
                return resolve_absolute_root_dir(&trimmed);
            }
        }
    }

    // Legacy: root-dir.txt written by old installers (kept as last-resort fallback).
    let config_path = install_data_dir.join("root-dir.txt");
    if config_path.is_file() {
        if let Ok(raw) = fs::read_to_string(&config_path) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return resolve_absolute_root_dir(trimmed);
            }
        }
    }

    Ok(install_data_dir.join("root"))
}

#[cfg(windows)]
fn read_root_dir_from_registry() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    extern "system" {
        fn RegGetValueW(
            hkey: isize,
            lpSubKey: *const u16,
            lpValue: *const u16,
            dwFlags: u32,
            pdwType: *mut u32,
            pvData: *mut std::ffi::c_void,
            pcbData: *mut u32,
        ) -> i32;
    }

    let sub_key: Vec<u16> = OsString::from("Software\\ChaWork")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = OsString::from("RootDir")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    const HKEY_CURRENT_USER: isize = -2147483647; // 0x80000001 sign-extended
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const ERROR_SUCCESS: i32 = 0;

    let mut data_type: u32 = 0;
    let mut data_size: u32 = 2048;
    let mut data: Vec<u16> = vec![0u16; data_size as usize / 2];

    unsafe {
        let ret = RegGetValueW(
            HKEY_CURRENT_USER,
            sub_key.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            &mut data_type,
            data.as_mut_ptr() as *mut std::ffi::c_void,
            &mut data_size,
        );
        if ret == ERROR_SUCCESS && data_size > 2 {
            // data_size is in bytes including null terminator
            let len = (data_size as usize / 2).saturating_sub(1);
            data.truncate(len);
            return OsString::from_wide(&data).into_string().ok();
        }
    }
    None
}

#[cfg(not(windows))]
fn read_root_dir_from_registry() -> Option<String> {
    None
}

fn resolve_absolute_root_dir(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(format!("root workspace 路径必须是绝对路径: {raw}"));
    }
    Ok(path)
}

/// 老布局 `<install>/workspaces.json` → `<root>/state/known-workspaces.json`。
/// 仅当源在且目标不在时执行，且失败时不视为致命错误（用户可手动迁移）。
fn migrate_known_workspaces(install: &Path, root: &RootWorkspace) -> Result<(), String> {
    let old = install.join("workspaces.json");
    let new = root.known_workspaces_path();
    if old.is_file() && !new.is_file() {
        if let Some(parent) = new.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("创建 known workspaces 目录失败 ({}): {e}", parent.display())
            })?;
        }
        fs::rename(&old, &new).map_err(|e| {
            format!(
                "迁移 known workspaces 失败 ({} → {}): {e}",
                old.display(),
                new.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_directories_creates_all_subdirs() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = init_or_open(tmp.path()).expect("init root");
        assert!(root.codex_home_dir().is_dir());
        assert!(root.runtime_dir().is_dir());
        assert!(root.skills_dir().is_dir());
        assert!(root.templates_dir().is_dir());
        assert!(root.mcp_dir().is_dir());
        assert!(root.state_dir().is_dir());
        assert!(root.cache_dir().is_dir());
        assert!(root.logs_dir().is_dir());
        assert!(root.path().join(".chawork-root").is_file());
    }

    #[test]
    fn resolve_root_base_defaults_to_app_data_root() {
        let tmp = tempfile::tempdir().expect("tmp");
        let resolved = resolve_root_base(tmp.path()).expect("resolve root base");
        assert_eq!(resolved, tmp.path().join("root"));
    }

    #[test]
    fn resolve_root_base_uses_installer_configured_root_dir() {
        let tmp = tempfile::tempdir().expect("tmp");
        let custom = tmp.path().join("custom root");
        fs::write(
            tmp.path().join("root-dir.txt"),
            custom.to_string_lossy().as_bytes(),
        )
        .unwrap();

        let resolved = resolve_root_base(tmp.path()).expect("resolve root base");

        assert_eq!(resolved, custom);
    }

    #[test]
    fn init_rejects_non_empty_non_chawork_custom_root() {
        let tmp = tempfile::tempdir().expect("tmp");
        let custom = tmp.path().join("existing");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("user-file.txt"), b"do not own").unwrap();
        fs::write(
            tmp.path().join("root-dir.txt"),
            custom.to_string_lossy().as_bytes(),
        )
        .unwrap();

        let err = init_or_open(tmp.path()).expect_err("must reject unsafe custom root");

        assert!(err.contains("不是空目录"));
    }

    #[test]
    fn migrates_legacy_known_workspaces() {
        let tmp = tempfile::tempdir().expect("tmp");
        let legacy = tmp.path().join("workspaces.json");
        fs::write(&legacy, b"[]").unwrap();

        let root = init_or_open(tmp.path()).expect("init root");
        assert!(!legacy.exists(), "legacy file should be moved");
        assert!(root.known_workspaces_path().is_file());
    }

    #[test]
    fn migration_skips_when_target_exists() {
        let tmp = tempfile::tempdir().expect("tmp");
        let legacy = tmp.path().join("workspaces.json");
        fs::write(&legacy, b"OLD").unwrap();

        // Pre-create target
        let root_path = tmp.path().join("root");
        fs::create_dir_all(root_path.join("state")).unwrap();
        fs::write(root_path.join("state/known-workspaces.json"), b"NEW").unwrap();

        let root = init_or_open(tmp.path()).expect("init root");
        let content = fs::read_to_string(root.known_workspaces_path()).unwrap();
        assert_eq!(content, "NEW", "should not overwrite existing target");
        assert!(
            legacy.exists(),
            "legacy preserved when target already present"
        );
    }
}
