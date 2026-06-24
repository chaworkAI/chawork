mod commands;
pub mod constants;
mod http_server;
mod path_safety;
mod runtime;
pub mod services;
mod state;

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use state::AppState;
use std::io::Write;
use std::sync::Mutex;
use tauri::Manager;

fn startup_log_dir() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA").ok().map(|p| {
            std::path::PathBuf::from(p)
                .join("com.chawork.app")
                .join("logs")
        })
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME").ok().map(|p| {
            std::path::PathBuf::from(p)
                .join("Library")
                .join("Logs")
                .join("com.chawork.app")
        })
    } else {
        std::env::var("HOME").ok().map(|p| {
            std::path::PathBuf::from(p)
                .join(".local")
                .join("share")
                .join("com.chawork.app")
                .join("logs")
        })
    }
}

fn log_startup(message: &str) {
    if let Some(dir) = startup_log_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("startup.log"))
        {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = writeln!(f, "[{ts}] {message}");
        }
    }
}

fn build_app_state(root: Arc<services::root_workspace::RootWorkspace>) -> Arc<AppState> {
    let known_workspaces_file = root.known_workspaces_path();

    Arc::new(AppState {
        root,
        active_workspace_path: Mutex::new(None),
        active_session_id: Mutex::new(None),
        known_workspaces_file,
        runtime_pool: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        codex_status: tokio::sync::Mutex::new("idle".to_string()),
        turn_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        transcript_write_lock: Mutex::new(()),
        employee_write_lock: Mutex::new(()),
        http_server_port: AtomicU16::new(0),
        import_queues: Mutex::new(std::collections::HashMap::new()),
        dream_runtime: tokio::sync::Mutex::new(None),
        dream_status: tokio::sync::Mutex::new("idle".to_string()),
    })
}

fn create_main_window<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<(), String> {
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
    else {
        return Err("ChaWork: 未找到 main 窗口配置".to_string());
    };

    let builder = tauri::WebviewWindowBuilder::from_config(app, config)
        .map_err(|e| format!("ChaWork: 创建主窗口配置失败: {e}"))?;

    #[cfg(target_os = "macos")]
    let builder = builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    let window = builder
        .build()
        .map_err(|e| format!("ChaWork: 创建主窗口失败: {e}"))?;

    // 热更新：如果存在健康的 hot-update 资源，导航到本地文件
    if let Some(hot_url) = resolve_hot_update_url(app) {
        let _ = window.navigate(hot_url);
    }

    Ok(())
}

/// 检查是否存在健康的热更新资源，返回 file:// URL
fn resolve_hot_update_url<R: tauri::Runtime>(app: &tauri::App<R>) -> Option<url::Url> {
    let app_data = app.path().app_data_dir().ok()?;
    let hot_update_dir = app_data.join("hot-update");
    let current_dir = hot_update_dir.join("current");
    let index_html = current_dir.join("index.html");

    if !index_html.exists() {
        return None;
    }

    // 崩溃保护：连续崩溃超过 3 次则回退
    let crash_file = hot_update_dir.join(".crash_count");
    let crash_count = std::fs::read_to_string(&crash_file)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

    if crash_count >= 3 {
        log_startup("Hot update crash limit reached, falling back to embedded");
        let _ = std::fs::remove_dir_all(&current_dir);
        let _ = std::fs::write(&crash_file, "0");
        return None;
    }

    // 递增崩溃计数（前端成功加载后会通过 ota_mark_healthy 重置）
    let _ = std::fs::write(&crash_file, (crash_count + 1).to_string());

    url::Url::from_file_path(&index_html).ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            let install_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("ChaWork: 无法解析应用数据目录: {e}"))?;
            std::fs::create_dir_all(&install_dir).map_err(|e| {
                format!(
                    "ChaWork: 无法创建应用数据目录 {}: {e}",
                    install_dir.display()
                )
            })?;

            let root = services::root_workspace::init_or_open(&install_dir)
                .map(Arc::new)
                .map_err(|e| format!("ChaWork: 初始化根工作区失败: {e}"))?;
            let app_state = build_app_state(root);
            if !app.manage(app_state.clone()) {
                return Err("ChaWork: AppState 已被注册".into());
            }

            create_main_window(app)?;

            let recovery_state = app_state.clone();
            std::thread::spawn(move || {
                if let Err(e) =
                    services::dream::recover_all_stranded_review_requests(&recovery_state.root)
                {
                    let msg = format!("Dream review recovery failed at startup: {e}");
                    log_startup(&msg);
                    eprintln!("ChaWork: {msg}");
                }
            });

            let app_handle = app.handle().clone();
            let http_state = app_state.clone();
            let sched_state = app_state.clone();

            tauri::async_runtime::spawn(async move {
                match http_server::start_http_server(http_state.clone()).await {
                    Ok(port) => {
                        http_state.http_server_port.store(port, Ordering::Relaxed);
                        println!("ChaWork HTTP server started on port {port}");
                    }
                    Err(e) => {
                        let msg = format!("HTTP server failed to start: {e}");
                        log_startup(&msg);
                        eprintln!("ChaWork: {msg}");
                    }
                }
            });

            tauri::async_runtime::spawn(services::dream_scheduler::start_dream_scheduler(
                sched_state,
                app_handle,
            ));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::employee::list_employees,
            commands::employee::get_employee_detail,
            commands::employee::create_employee,
            commands::employee::update_employee_metadata,
            commands::employee::delete_employee,
            commands::employee::check_employee_integrity,
            commands::employee::read_employee_prompt,
            commands::employee::write_employee_prompt,
            commands::employee::list_employee_skills,
            commands::employee::copy_root_skill_to_employee,
            commands::employee::toggle_employee_skill,
            commands::employee::delete_employee_skill,
            commands::employee::bind_workspace_to_employee,
            commands::employee::unbind_workspace_from_employee,
            commands::employee::validate_workspace_binding,
            commands::employee::list_workspaces_for_employee,
            commands::dream::get_dream_log,
            commands::dream::get_dream_config,
            commands::dream::set_dream_config,
            commands::dream::get_recent_dream_result,
            commands::dream::get_pending_request,
            commands::dream::list_employees_with_pending_dream_requests,
            commands::dream::reject_dream_request,
            commands::dream::approve_dream_request,
            commands::dream::get_dream_defaults,
            commands::dream::set_dream_defaults,
            commands::dream::run_dream_phase1,
            commands::dream::cancel_dream_run,
            commands::workspace::list_workspaces,
            commands::workspace::create_workspace,
            commands::workspace::switch_workspace,
            commands::workspace::open_workspace_dialog,
            commands::session::list_sessions,
            commands::session::create_session,
            commands::session::switch_session,
            commands::session::rename_session,
            commands::session::delete_session,
            commands::session::send_chat_message,
            commands::session::get_active_session_transcript,
            commands::runtime::get_runtime_status,
            commands::runtime::get_runtime_metadata,
            commands::runtime::cancel_current_turn,
            commands::runtime::respond_runtime_approval,
            commands::runtime::respond_runtime_user_input,
            commands::runtime::respond_runtime_mcp_elicitation,
            commands::runtime::respond_runtime_permissions,
            commands::runtime::start_workspace_runtime,
            commands::runtime::refresh_runtime_context,
            commands::skill::list_skills,
            commands::skill::set_workspace_skill_selection,
            commands::skill::create_workspace_skill_override,
            commands::skill::delete_workspace_skill,
            commands::skill::promote_skill_to_global,
            commands::hub::hub_get_manifest,
            commands::hub::hub_list_professions,
            commands::hub::hub_list_skills,
            commands::hub::hub_get_skill_detail,
            commands::hub::hub_install_skill,
            commands::hub::hub_uninstall_skill,
            commands::hub::hub_list_employees,
            commands::hub::hub_get_employee_detail,
            commands::hub::hub_install_employee,
            commands::hub::hub_start_github_import,
            commands::hub::hub_get_github_import_job,
            commands::hub::hub_complete_github_import,
            commands::hub::github_scan_repo,
            commands::hub::github_download_all_skills,
            commands::hub::github_complete_import,
            commands::mcp_tool::list_mcp_tools,
            commands::mcp_tool::set_workspace_mcp_tool_policy,
            commands::mcp_tool::list_workspace_mcp_servers,
            commands::mcp_tool::upsert_workspace_mcp_server,
            commands::mcp_tool::import_workspace_mcp_servers_json,
            commands::mcp_tool::delete_workspace_mcp_server,
            commands::mcp_tool::test_workspace_mcp_server,
            commands::qmd::qmd_initialize,
            commands::qmd::qmd_refresh,
            commands::qmd::qmd_status,
            commands::qmd::qmd_search,
            commands::qmd::qmd_get_document,
            commands::qmd::qmd_refresh_if_stale,
            commands::import::import_file,
            commands::import::get_import_task,
            commands::import::list_import_tasks,
            commands::import::list_imports,
            commands::domain_pack::get_domain_pack,
            commands::proposal::create_proposal,
            commands::proposal::list_proposals,
            commands::proposal::get_proposal,
            commands::proposal::apply_proposal,
            commands::proposal::reject_proposal,
            commands::global_settings::get_global_provider,
            commands::global_settings::set_global_provider_model,
            commands::global_settings::set_global_provider_instructions,
            commands::global_settings::set_global_provider_connection,
            commands::global_settings::is_global_provider_configured,
            commands::global_settings::get_ui_preferences,
            commands::global_settings::set_ui_preferences,
            commands::global_settings::get_ui_locale,
            commands::global_settings::set_ui_locale,
            commands::global_settings::get_root_workspace_info,
            commands::global_settings::reveal_global_provider_config,
            commands::global_settings::set_global_provider,
            commands::global_settings::list_provider_models,
            commands::workspace_config::get_effective_provider,
            commands::workspace_config::get_tool_policy,
            commands::workspace_config::set_tool_policy,
            commands::http_server::get_http_server_port,
            commands::ota::get_platform_info,
            commands::ota::download_ota_file,
            commands::ota::apply_hot_patch,
            commands::ota::apply_full_frontend_bundle,
            commands::ota::ota_mark_healthy,
        ])
        .run(tauri::generate_context!())
    {
        let msg = format!("应用启动失败: {e}");
        log_startup(&msg);
        eprintln!("ChaWork: {msg}");
        std::process::exit(1);
    }
}
