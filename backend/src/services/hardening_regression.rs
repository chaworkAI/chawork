//! Service-layer regression tests mirroring hardening plan checklist B.
//! These are not GUI WebDriver tests; they validate disk + service invariants
//! for Provider configuration and Dream approval failure paths.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::services::{
        dream as dream_svc, employee as emp, global_provider, root_workspace, session, workspace,
    };

    fn setup() -> (tempfile::TempDir, root_workspace::RootWorkspace) {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = root_workspace::init_or_open(tmp.path()).expect("init root");
        (tmp, root)
    }

    fn write_global_provider(root: &root_workspace::RootWorkspace) {
        global_provider::write_global_field(root, "model", "gpt-test").expect("model");
        global_provider::write_global_field(root, "openai_base_url", "https://api.example.com/v1")
            .expect("base url");
        global_provider::write_global_field(root, "openai_api_key", "sk-test-key")
            .expect("api key");
    }

    fn request_dir(
        root: &root_workspace::RootWorkspace,
        employee_id: &str,
        status: &str,
    ) -> PathBuf {
        root.employees_dir()
            .join(employee_id)
            .join("prompt-update-requests")
            .join(status)
    }

    fn open_workspace(tmp: &tempfile::TempDir, name: &str) -> PathBuf {
        let ws_path = tmp.path().join(name);
        fs::create_dir_all(ws_path.join(".chawork/state")).expect("workspace dirs");
        let mut ws = workspace::open_or_create(ws_path.as_path()).expect("open workspace");
        workspace::persist_workspace(ws_path.as_path(), &mut ws).expect("persist workspace");
        ws_path
    }

    #[test]
    fn configured_global_provider_enables_effective_resolution() {
        let (_tmp, root) = setup();
        write_global_provider(&root);
        let ws = open_workspace(&_tmp, "provider-ws");

        let effective = global_provider::effective_provider(&root, &ws).expect("effective");
        assert_eq!(effective.model, "gpt-test");
    }

    #[test]
    fn dream_approve_prepare_failure_leaves_failed_not_stranded_in_applying() {
        let (_tmp, root) = setup();
        emp::create(
            &root,
            emp::CreateEmployeeInput::basic("fail-emp", "fail-emp"),
        )
        .expect("create employee");

        let result = dream_svc::DreamResult {
            decision: dream_svc::DreamDecision::UpdateRequired,
            target_employee_id: "fail-emp".to_string(),
            dream_run_id: "run-fail".to_string(),
            summary: "Needs update".to_string(),
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

        dream_svc::move_request_to_approved(&root, "fail-emp").expect("approve");
        dream_svc::move_request_to_status_pub(&root, "fail-emp", "approved", "failed")
            .expect("mark failed");

        assert!(
            !request_dir(&root, "fail-emp", "approved")
                .join("current.json")
                .is_file(),
            "approved should be empty after failed transition"
        );
        assert!(request_dir(&root, "fail-emp", "failed")
            .join("current.json")
            .is_file());
        let failed_json =
            fs::read_to_string(request_dir(&root, "fail-emp", "failed").join("current.json"))
                .unwrap();
        assert!(failed_json.contains("\"status\": \"failed\""));
        assert!(!dream_svc::has_pending_request(&root, "fail-emp"));
    }

    #[test]
    fn dream_apply_marker_recovery_completes_applied_not_failed() {
        let (_tmp, root) = setup();
        emp::create(
            &root,
            emp::CreateEmployeeInput::basic("marker-emp", "marker-emp"),
        )
        .expect("create employee");

        let initial_prompt = "# Initial Prompt\n\n## Operating Rules\n\nStart here with enough baseline content for validation.\n";
        fs::write(
            root.employees_dir().join("marker-emp").join("prompt.md"),
            initial_prompt,
        )
        .expect("seed prompt");

        let result = dream_svc::DreamResult {
            decision: dream_svc::DreamDecision::UpdateRequired,
            target_employee_id: "marker-emp".to_string(),
            dream_run_id: "run-marker".to_string(),
            summary: "Needs update".to_string(),
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
        dream_svc::move_request_to_approved(&root, "marker-emp").expect("approve");
        dream_svc::move_request_to_status_pub(&root, "marker-emp", "approved", "applying")
            .expect("applying");

        let candidate = "# Updated Employee Prompt\n\n## Operating Rules\n\nRecovered after marker.\n\n## Runtime Evidence\n\nGenerated by Dream Phase 2 runtime candidate.\n";
        let prompt_path = root.employees_dir().join("marker-emp").join("prompt.md");
        dream_svc::apply_prompt_from_runtime(&root, "marker-emp", "run-marker", &candidate)
            .expect("write prompt");
        fs::write(
            request_dir(&root, "marker-emp", "applying").join("prompt_written.marker"),
            "run-marker",
        )
        .expect("marker");

        dream_svc::recover_stranded_review_requests(&root, "marker-emp", false).expect("recover");

        assert!(
            request_dir(&root, "marker-emp", "applied")
                .join("current.json")
                .is_file(),
            "stranded applying with marker should recover to applied"
        );
        assert!(!request_dir(&root, "marker-emp", "applying")
            .join("current.json")
            .is_file());
        assert!(!request_dir(&root, "marker-emp", "failed")
            .join("current.json")
            .is_file());
        let applied_prompt = fs::read_to_string(prompt_path).expect("read prompt");
        assert!(applied_prompt.contains("Recovered after marker"));
    }

    #[test]
    fn dream_badge_lists_active_review_states_beyond_pending() {
        let (_tmp, root) = setup();
        emp::create(
            &root,
            emp::CreateEmployeeInput::basic("badge-emp", "badge-emp"),
        )
        .expect("create employee");

        let result = dream_svc::DreamResult {
            decision: dream_svc::DreamDecision::UpdateRequired,
            target_employee_id: "badge-emp".to_string(),
            dream_run_id: "run-badge".to_string(),
            summary: "Needs update".to_string(),
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
        dream_svc::move_request_to_approved(&root, "badge-emp").expect("approve");
        dream_svc::move_request_to_status_pub(&root, "badge-emp", "approved", "applying")
            .expect("applying");

        let ids = dream_svc::list_employees_with_pending_requests(&root).expect("list");
        assert!(ids.contains(&"badge-emp".to_string()));
    }

    #[test]
    fn workspace_switch_and_session_create_use_active_state_helpers() {
        let (_tmp, root) = setup();
        let ws_path = open_workspace(&_tmp, "active-ws");
        let state = crate::state::AppState {
            root: std::sync::Arc::new(root),
            active_workspace_path: std::sync::Mutex::new(Some(ws_path.clone())),
            active_session_id: std::sync::Mutex::new(None),
            known_workspaces_file: _tmp.path().join("known.json"),
            runtime_pool: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            codex_status: tokio::sync::Mutex::new("idle".to_string()),
            turn_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            transcript_write_lock: std::sync::Mutex::new(()),
            employee_write_lock: std::sync::Mutex::new(()),
            http_server_port: std::sync::atomic::AtomicU16::new(0),
            import_queues: std::sync::Mutex::new(std::collections::HashMap::new()),
            dream_runtime: tokio::sync::Mutex::new(None),
            dream_status: tokio::sync::Mutex::new("idle".to_string()),
        };

        let ws = workspace::open_or_create(ws_path.as_path()).expect("read ws");
        let meta = session::create(ws_path.as_path(), &ws.id).expect("create session");
        *state.lock_active_session_id() = Some(meta.id.clone());

        assert_eq!(
            state.require_active_workspace().expect("workspace"),
            ws_path
        );
        assert_eq!(
            state.lock_active_session_id().as_deref(),
            Some(meta.id.as_str())
        );
    }
}
