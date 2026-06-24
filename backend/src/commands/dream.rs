use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::runtime::dream_session::{DreamRuntimeClient, DreamRuntimeConfig};
use crate::runtime::lifecycle::{
    invalidate_employee_chat_runtimes, MutationWithRuntimeInvalidation, RuntimeInvalidationReason,
};
use crate::services::context_builder;
use crate::services::dream as dream_svc;
use crate::services::dream_phase1::{self, DreamPhase1Mode};
use crate::state::AppState;

#[tauri::command]
pub fn get_dream_log(
    app_state: State<Arc<AppState>>,
    limit: usize,
) -> Result<Vec<dream_svc::DreamLogEntry>, String> {
    Ok(dream_svc::read_dream_log(&app_state.root, limit))
}

#[tauri::command]
pub fn get_dream_config(
    app_state: State<Arc<AppState>>,
    employee_id: String,
) -> Result<dream_svc::DreamConfig, String> {
    dream_svc::read_dream_config(&app_state.root, &employee_id)
}

#[tauri::command]
pub fn set_dream_config(
    app_state: State<Arc<AppState>>,
    employee_id: String,
    config: dream_svc::DreamConfig,
) -> Result<(), String> {
    let _lock = app_state.lock_employee_write();
    dream_svc::write_dream_config(&app_state.root, &employee_id, &config)
}

#[tauri::command]
pub fn get_recent_dream_result(
    app_state: State<Arc<AppState>>,
    employee_id: String,
) -> Result<Option<dream_svc::RecentDreamResult>, String> {
    Ok(dream_svc::read_recent_dream_result(
        &app_state.root,
        &employee_id,
    ))
}

#[tauri::command]
pub fn get_pending_request(
    app_state: State<Arc<AppState>>,
    employee_id: String,
) -> Result<Option<dream_svc::PendingUpdateRequest>, String> {
    let dream_phase2_active = app_state
        .dream_status
        .try_lock()
        .map(|status| status.as_str() != "idle")
        .unwrap_or(true)
        || app_state
            .dream_runtime
            .try_lock()
            .map(|slot| slot.is_some())
            .unwrap_or(true);
    let _lock = app_state.lock_employee_write();
    dream_svc::recover_stranded_review_requests(
        &app_state.root,
        &employee_id,
        dream_phase2_active,
    )?;
    Ok(dream_svc::read_active_review_request(
        &app_state.root,
        &employee_id,
    ))
}

#[tauri::command]
pub fn list_employees_with_pending_dream_requests(
    app_state: State<Arc<AppState>>,
) -> Result<Vec<String>, String> {
    dream_svc::list_employees_with_pending_requests(&app_state.root)
}

#[tauri::command]
pub fn reject_dream_request(
    app_state: State<Arc<AppState>>,
    employee_id: String,
) -> Result<(), String> {
    let _lock = app_state.lock_employee_write();
    dream_svc::reject_pending_request(&app_state.root, &employee_id)
}

/// Approve a pending dream request and run Phase 2 via chawork-runtime to generate
/// the complete prompt candidate. Runtime failure never falls back to app-layer writes.
#[tauri::command]
pub async fn approve_dream_request(
    app_state: State<'_, Arc<AppState>>,
    app: AppHandle,
    employee_id: String,
) -> Result<MutationWithRuntimeInvalidation<dream_svc::ApplyResult>, String> {
    let req = {
        let _lock = app_state.lock_employee_write();
        dream_svc::take_request_for_phase2(&app_state.root, &employee_id)?
    };

    let dream_run_id = req.dream_run_id.clone();
    let run_workspace = dream_svc::dream_run_workspace(&app_state.root, &dream_run_id);

    let prepared = match context_builder::prepare_dream_codex_home(
        &run_workspace,
        &app_state.root,
        &employee_id,
        &dream_run_id,
        2,
    ) {
        Ok(prepared) => prepared,
        Err(e) => {
            mark_request_failed(&app_state, &employee_id, &dream_run_id, "approved", &e)?;
            return Err(e);
        }
    };
    let output_language = crate::services::ui_locale::read_ui_locale(&app_state.root);

    {
        let _lock = app_state.lock_employee_write();
        dream_svc::move_request_to_status_pub(
            &app_state.root,
            &employee_id,
            "approved",
            "applying",
        )?;
    }

    let client = DreamRuntimeClient::new(DreamRuntimeConfig {
        run_workspace_path: run_workspace.clone(),
        codex_home: prepared.codex_home,
        runtime_home: prepared.runtime_home,
        model: prepared.model,
        api_key: prepared.api_key,
        output_language,
    });

    {
        let mut slot = app_state.dream_runtime.lock().await;
        if slot.is_some() {
            return Err("Dream 正在运行中，请稍后再批准".to_string());
        }
        *slot = Some(Arc::clone(&client));
    }

    *app_state.dream_status.lock().await = "running".to_string();
    let target_prompt_path = format!("employees/{employee_id}/prompt.md");
    let turn_result = client
        .phase2(&app, "current", &req.result, &target_prompt_path)
        .await;

    {
        let mut slot = app_state.dream_runtime.lock().await;
        *slot = None;
    }
    *app_state.dream_status.lock().await = "idle".to_string();

    let outcome = match turn_result {
        Ok(runtime_result) => {
            if runtime_result.target_employee_id != employee_id
                || runtime_result.dream_run_id != dream_run_id
                || runtime_result.target_prompt_path != target_prompt_path
            {
                let err = "Dream Phase 2 返回的 target metadata 不匹配".to_string();
                mark_request_failed(&app_state, &employee_id, &dream_run_id, "applying", &err)?;
                return Err(err);
            }
            let _lock = app_state.lock_employee_write();
            let result = dream_svc::apply_prompt_and_complete_request(
                &app_state.root,
                &employee_id,
                &dream_run_id,
                &runtime_result.prompt_candidate,
            )?;
            dream_svc::append_dream_log(
                &app_state.root,
                "prompt_applied_phase2",
                &format!(
                    "Dream Phase 2 已完成并应用 prompt 更新 (run: {dream_run_id}, target: {employee_id})"
                ),
            );
            Ok(result)
        }
        Err(e) => {
            mark_request_failed(
                &app_state,
                &employee_id,
                &dream_run_id,
                "applying",
                &e.to_string(),
            )?;
            Err(format!("Dream Phase 2 执行失败: {e}"))
        }
    };

    let apply_result = outcome?;
    let runtime_invalidation = invalidate_employee_chat_runtimes(
        &app_state,
        &app,
        &employee_id,
        RuntimeInvalidationReason::DreamPromptApplied,
    )
    .await;
    Ok(MutationWithRuntimeInvalidation::success(
        apply_result,
        runtime_invalidation,
    ))
}

#[tauri::command]
pub fn get_dream_defaults(
    app_state: State<Arc<AppState>>,
) -> Result<dream_svc::DreamDefaults, String> {
    Ok(dream_svc::read_dream_defaults(&app_state.root))
}

#[tauri::command]
pub fn set_dream_defaults(
    app_state: State<Arc<AppState>>,
    defaults: dream_svc::DreamDefaults,
) -> Result<(), String> {
    let _lock = app_state.lock_employee_write();
    dream_svc::write_dream_defaults(&app_state.root, &defaults)
}

/// Run Dream Phase 1: prepare run, build runtime config, execute via Dream runtime,
/// and persist the structured result. Emits progress on `"dream-event"` channel.
#[tauri::command]
pub async fn run_dream_phase1(
    app_state: State<'_, Arc<AppState>>,
    app: AppHandle,
    employee_id: String,
) -> Result<dream_svc::RecentDreamResult, String> {
    dream_phase1::run_phase1_with_runtime(&app_state, &app, &employee_id, DreamPhase1Mode::Manual)
        .await?
        .ok_or_else(|| "Dream Phase 1 完成后未找到 recent result".to_string())
}

#[tauri::command]
pub async fn cancel_dream_run(app_state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut slot = app_state.dream_runtime.lock().await;
    if let Some(rt) = slot.take() {
        rt.cancel().await;
    }
    *app_state.dream_status.lock().await = "idle".to_string();
    Ok(())
}

fn mark_request_failed(
    app_state: &AppState,
    employee_id: &str,
    dream_run_id: &str,
    from_status: &str,
    err: &str,
) -> Result<(), String> {
    let _lock = app_state.lock_employee_write();
    dream_svc::move_request_to_status_pub(&app_state.root, employee_id, from_status, "failed")?;
    let failed_dir = app_state
        .root
        .employees_dir()
        .join(employee_id)
        .join("prompt-update-requests/failed");
    std::fs::create_dir_all(&failed_dir).map_err(|e| e.to_string())?;
    std::fs::write(failed_dir.join("error.txt"), err).map_err(|e| e.to_string())?;
    dream_svc::append_dream_log(
        &app_state.root,
        "phase2_error",
        &format!("Dream Phase 2 执行失败: {err} (run: {dream_run_id})"),
    );
    Ok(())
}
