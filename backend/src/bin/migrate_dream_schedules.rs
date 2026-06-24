//! Migrate existing employees' dream.yaml schedule to daily.
//!
//! Usage:
//!   cargo run --manifest-path backend/Cargo.toml --bin migrate_dream_schedules

use chawork_lib::services::{dream, root_workspace};

fn install_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.chawork.app")
}

fn main() {
    let install = install_dir();
    let root = match root_workspace::init_or_open(&install) {
        Ok(root) => root,
        Err(err) => {
            eprintln!("打开 ChaWork 根工作区失败: {err}");
            std::process::exit(1);
        }
    };

    match dream::migrate_dream_schedules_to_daily(&root) {
        Ok(updated) => {
            if updated.is_empty() {
                println!("所有员工的 dream.yaml 已是每日定时，无需迁移。");
            } else {
                println!("已迁移 {} 名员工为每日定时:", updated.len());
                for id in updated {
                    println!("  - {id}");
                }
            }
        }
        Err(err) => {
            eprintln!("迁移失败: {err}");
            std::process::exit(1);
        }
    }
}
