use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::runtime::dream_session::{DreamRuntimeClient, DreamRuntimeConfig};
use crate::services::context_builder;
use crate::services::dream as dream_svc;
use crate::services::ui_locale;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DreamPhase1Mode {
    Manual,
    Scheduled,
}

#[derive(Debug, Clone)]
pub struct DreamPhase1ExecutionInput {
    pub employee_id: String,
    pub dream_run_id: String,
    pub run_workspace_path: PathBuf,
    pub codex_home: String,
    pub runtime_home: String,
    pub model: String,
    pub api_key: String,
    pub output_language: String,
    pub mode: DreamPhase1Mode,
}

pub async fn run_phase1_with_runtime(
    app_state: &Arc<AppState>,
    app: &AppHandle,
    employee_id: &str,
    mode: DreamPhase1Mode,
) -> Result<Option<dream_svc::RecentDreamResult>, String> {
    let recent = run_phase1_with_executor(app_state, employee_id, mode, |input| {
        let state = Arc::clone(app_state);
        let app = app.clone();
        async move { execute_phase1_runtime(&state, &app, input).await }
    })
    .await?;

    if let Some(ref result) = recent {
        emit_dream_run_ready(app, &app_state.root, employee_id, result);
    }

    Ok(recent)
}

fn emit_dream_run_ready(
    app: &AppHandle,
    root: &super::root_workspace::RootWorkspace,
    employee_id: &str,
    result: &dream_svc::RecentDreamResult,
) {
    use serde::Serialize;

    #[derive(Serialize, Clone)]
    struct DreamReadyPayload {
        employee_id: String,
        employee_name: String,
        dream_run_id: String,
        selected_session_count: usize,
    }

    let employee_name = super::employee::list(root)
        .ok()
        .and_then(|entries| {
            entries
                .into_iter()
                .find(|entry| entry.id == employee_id)
                .map(|entry| entry.name)
        })
        .unwrap_or_else(|| employee_id.to_string());

    let _ = app.emit(
        "dream-run-ready",
        DreamReadyPayload {
            employee_id: employee_id.to_string(),
            employee_name,
            dream_run_id: result.dream_run_id.clone(),
            selected_session_count: result.source_sessions.len(),
        },
    );
}

pub async fn run_phase1_with_executor<F, Fut>(
    app_state: &Arc<AppState>,
    employee_id: &str,
    mode: DreamPhase1Mode,
    executor: F,
) -> Result<Option<dream_svc::RecentDreamResult>, String>
where
    F: FnOnce(DreamPhase1ExecutionInput) -> Fut,
    Fut: Future<Output = Result<dream_svc::DreamResult, String>>,
{
    let input = dream_svc::DreamPrepareInput {
        target_employee_id: employee_id.to_string(),
        workspace_filter: None,
    };

    let prepare_result = {
        let _lock = app_state.lock_employee_write();
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
        dream_svc::recover_stranded_review_requests(
            &app_state.root,
            employee_id,
            dream_phase2_active,
        )?;

        if mode == DreamPhase1Mode::Scheduled
            && (!dream_svc::should_run_dream(&app_state.root, employee_id)
                || !dream_svc::has_missed_dream(&app_state.root, employee_id)
                || dream_svc::has_active_review_request(&app_state.root, employee_id))
        {
            return Ok(None);
        }

        if mode == DreamPhase1Mode::Manual
            && dream_svc::has_active_review_request(&app_state.root, employee_id)
        {
            return Err(
                "已有待审或进行中的 Dream 更新请求，请先处理后再手动运行 Dream".to_string(),
            );
        }

        dream_svc::prepare_dream_run(&app_state.root, input)?
    };

    if let Some(reason) = &prepare_result.skipped_reason {
        return match mode {
            DreamPhase1Mode::Manual => Err(reason.clone()),
            DreamPhase1Mode::Scheduled => Ok(None),
        };
    }

    let run_workspace = PathBuf::from(&prepare_result.run_workspace_path);
    let dream_run_id = prepare_result.dream_run_id.clone();
    let output_language = ui_locale::read_ui_locale(&app_state.root);

    let prepared = context_builder::prepare_dream_codex_home(
        &run_workspace,
        &app_state.root,
        employee_id,
        &dream_run_id,
        1,
    )?;

    let runtime_result = executor(DreamPhase1ExecutionInput {
        employee_id: employee_id.to_string(),
        dream_run_id: dream_run_id.clone(),
        run_workspace_path: run_workspace,
        codex_home: prepared.codex_home,
        runtime_home: prepared.runtime_home,
        model: prepared.model,
        api_key: prepared.api_key,
        output_language,
        mode,
    })
    .await;

    match runtime_result {
        Ok(result) => {
            let _lock = app_state.lock_employee_write();
            dream_svc::process_dream_result(&app_state.root, &result)?;
            Ok(dream_svc::read_recent_dream_result(
                &app_state.root,
                employee_id,
            ))
        }
        Err(e) => {
            dream_svc::append_dream_log(
                &app_state.root,
                "phase1_error",
                &format!("Dream Phase 1 执行失败: {e}"),
            );
            Err(format!("Dream Phase 1 执行失败: {e}"))
        }
    }
}

async fn execute_phase1_runtime(
    app_state: &Arc<AppState>,
    app: &AppHandle,
    input: DreamPhase1ExecutionInput,
) -> Result<dream_svc::DreamResult, String> {
    let client = DreamRuntimeClient::new(DreamRuntimeConfig {
        run_workspace_path: input.run_workspace_path.clone(),
        codex_home: input.codex_home,
        runtime_home: input.runtime_home,
        model: input.model,
        api_key: input.api_key,
        output_language: input.output_language,
    });

    {
        let mut slot = app_state.dream_runtime.lock().await;
        match input.mode {
            DreamPhase1Mode::Manual => {
                if slot.is_some() {
                    return Err("Dream 正在运行中，请等待完成后再手动运行".to_string());
                }
            }
            DreamPhase1Mode::Scheduled => {
                if slot.is_some() {
                    return Err("Dream runtime 正在运行，跳过本次 scheduled Dream".to_string());
                }
            }
        }
        *slot = Some(Arc::clone(&client));
    }

    *app_state.dream_status.lock().await = "running".to_string();
    let result = client
        .phase1(app, &input.employee_id, &input.dream_run_id)
        .await
        .map(|runtime_result| runtime_result.result)
        .map_err(|e| e.to_string());

    {
        let mut slot = app_state.dream_runtime.lock().await;
        *slot = None;
    }
    *app_state.dream_status.lock().await = "idle".to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{employee, root_workspace, session, workspace};
    use crate::state::AppState;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU16};
    use std::sync::Mutex;
    use std::time::Duration;

    fn app_state(root: root_workspace::RootWorkspace) -> Arc<AppState> {
        Arc::new(AppState {
            known_workspaces_file: root.known_workspaces_path(),
            root: Arc::new(root),
            active_workspace_path: Mutex::new(None),
            active_session_id: Mutex::new(None),
            runtime_pool: tokio::sync::Mutex::new(HashMap::new()),
            codex_status: tokio::sync::Mutex::new("idle".to_string()),
            turn_cancel: Arc::new(AtomicBool::new(false)),
            transcript_write_lock: Mutex::new(()),
            employee_write_lock: Mutex::new(()),
            http_server_port: AtomicU16::new(0),
            import_queues: Mutex::new(HashMap::new()),
            dream_runtime: tokio::sync::Mutex::new(None),
            dream_status: tokio::sync::Mutex::new("idle".to_string()),
        })
    }

    fn create_due_employee_with_session(
        tmp: &tempfile::TempDir,
        root: &root_workspace::RootWorkspace,
        employee_id: &str,
    ) {
        employee::create(
            root,
            employee::CreateEmployeeInput::basic(employee_id, employee_id),
        )
        .expect("create employee");
        let mut cfg = dream_svc::DreamConfig::default();
        cfg.enabled = true;
        cfg.schedule.time = Some("00:00".to_string());
        dream_svc::write_dream_config(root, employee_id, &cfg).expect("write dream config");

        let ws_path = tmp.path().join(format!("{employee_id}-ws"));
        std::fs::create_dir_all(ws_path.join(".chawork/state")).expect("workspace dirs");
        let ws_id = uuid::Uuid::new_v4().to_string();
        let ws = workspace::WorkspaceState {
            id: ws_id.clone(),
            name: format!("{employee_id}-ws"),
            path: ws_path.to_string_lossy().into_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_active_at: chrono::Utc::now().to_rfc3339(),
            active_session_id: None,
            domain_pack_id: None,
            index_status: "stale".to_string(),
            pending_proposals_count: 0,
            bound_employee_name: None,
            bound_employee_id: None,
        };
        std::fs::write(
            ws_path.join(".chawork/state/workspace.json"),
            serde_json::to_string_pretty(&ws).expect("workspace json"),
        )
        .expect("write workspace state");
        employee::bind_workspace(root, employee_id, &ws_path, &ws_id, &ws.name)
            .expect("bind workspace");
        let meta = session::create(&ws_path, &ws_id).expect("create session");
        session::append_transcript(
            &ws_path,
            &meta.id,
            &serde_json::json!({
                "role": "user",
                "content": "Please remember this stable preference.",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        )
        .expect("append transcript");
        session::sync_meta_from_transcript(&ws_path, &meta.id).expect("sync meta");
    }

    #[tokio::test]
    async fn scheduled_phase1_executes_and_persists_recent_result() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        create_due_employee_with_session(&tmp, &root, "sched-emp");
        ui_locale::write_ui_locale(&root, "en-US").expect("write ui locale");
        let state = app_state(root);

        let recent = run_phase1_with_executor(
            &state,
            "sched-emp",
            DreamPhase1Mode::Scheduled,
            |input| async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                assert_eq!(input.output_language, "en-US");
                Ok(dream_svc::DreamResult {
                    decision: dream_svc::DreamDecision::NoUpdate,
                    target_employee_id: input.employee_id,
                    dream_run_id: input.dream_run_id,
                    summary: "No durable prompt update needed.".to_string(),
                    source_sessions: vec![dream_svc::SourceSessionRef {
                        workspace_id: "ws".to_string(),
                        session_id: "sess".to_string(),
                        last_updated_at: None,
                    }],
                    updates: None,
                    impact: None,
                    status: "pending".to_string(),
                    source_prompt_path: None,
                    created_at: Some(chrono::Utc::now().to_rfc3339()),
                })
            },
        )
        .await
        .expect("scheduled dream run")
        .expect("recent result");

        assert_eq!(recent.target_employee_id, "sched-emp");
        assert_eq!(recent.decision, dream_svc::DreamDecision::NoUpdate);
        assert!(dream_svc::read_recent_dream_result(&state.root, "sched-emp").is_some());
    }

    #[tokio::test]
    async fn manual_phase1_rejects_when_pending_request_exists() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        create_due_employee_with_session(&tmp, &root, "manual-emp");

        let result = dream_svc::DreamResult {
            decision: dream_svc::DreamDecision::UpdateRequired,
            target_employee_id: "manual-emp".to_string(),
            dream_run_id: "run-pending".to_string(),
            summary: "Needs review".to_string(),
            source_sessions: vec![dream_svc::SourceSessionRef {
                workspace_id: "ws".to_string(),
                session_id: "sess".to_string(),
                last_updated_at: None,
            }],
            updates: Some(vec![dream_svc::PromptUpdate {
                section: "Tone".to_string(),
                action: "add".to_string(),
                content: "Be concise.".to_string(),
                reason: "Test".to_string(),
            }]),
            impact: None,
            status: "pending".to_string(),
            source_prompt_path: None,
            created_at: None,
        };
        dream_svc::process_dream_result(&root, &result).expect("process result");
        let state = app_state(root);

        let err = run_phase1_with_executor(
            &state,
            "manual-emp",
            DreamPhase1Mode::Manual,
            |_input| async move { Err("executor should not run".to_string()) },
        )
        .await
        .expect_err("manual phase1 should fail");

        assert!(err.contains("待审或进行中"));
    }
}
