use super::*;
use crate::runtime::process::RuntimeConfig;
use crate::runtime::session::connection::{read_line, route_runtime_line, RuntimeMessage};
use crate::services::session as session_svc;
use crate::state::RuntimeSlotStatus;
use serde_json::json;
use std::process::Stdio;
use std::sync::Mutex;
use tauri::Listener;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::Duration;

static RUNTIME_ENV_LOCK: Mutex<()> = Mutex::new(());

fn text_turn_input(text: &str) -> RuntimeTurnInput {
    RuntimeTurnInput {
        text: text.to_string(),
        local_images: Vec::new(),
    }
}

#[test]
fn default_runtime_cli_uses_runtime_env_name() {
    let _env_guard = RUNTIME_ENV_LOCK.lock().expect("runtime env lock");
    std::env::set_var("CHAWORK_RUNTIME_CLI", "/tmp/new-runtime");
    assert_eq!(default_chawork_runtime_cli(), "/tmp/new-runtime");
    std::env::remove_var("CHAWORK_RUNTIME_CLI");
}

#[test]
fn required_runtime_capabilities_cover_backend_bridge_paths() {
    let caps = required_runtime_capabilities();
    assert!(caps.contains(&"thread.start"));
    assert!(caps.contains(&"thread.resume"));
    assert!(caps.contains(&"thread.compact.start"));
    assert!(caps.contains(&"turn.start.text"));
    assert!(caps.contains(&"turn.start.image"));
    assert!(caps.contains(&"turn.start.local_image"));
    assert!(caps.contains(&"turn.start.skill"));
    assert!(caps.contains(&"turn.start.mention"));
    assert!(caps.contains(&"turn.steer"));
    assert!(caps.contains(&"turn.interrupt"));
    assert!(caps.contains(&"assistant.delta"));
    assert!(caps.contains(&"assistant.done"));
    assert!(caps.contains(&"reasoning.delta"));
    assert!(caps.contains(&"reasoning.done"));
    assert!(caps.contains(&"item.started"));
    assert!(caps.contains(&"tool.call_delta"));
    assert!(caps.contains(&"tool.call_completed"));
    assert!(caps.contains(&"file_change.updated"));
    assert!(caps.contains(&"file_change.delta"));
    assert!(caps.contains(&"file_change.completed"));
    assert!(caps.contains(&"codex.notification.mcp_tool_call_progress"));
    assert!(caps.contains(&"codex.notification.mcp_server_oauth_login_completed"));
    assert!(caps.contains(&"codex.notification.mcp_server_status_updated"));
    assert!(caps.contains(&"plan.updated"));
    assert!(caps.contains(&"plan.delta"));
    assert!(caps.contains(&"plan.done"));
    assert!(caps.contains(&"turn.completed"));
    assert!(caps.contains(&"token_usage.updated"));
    assert!(caps.contains(&"server_request.command_approval"));
    assert!(caps.contains(&"server_request.file_change_approval"));
    assert!(caps.contains(&"server_request.permissions"));
    assert!(caps.contains(&"server_request.user_input"));
    assert!(caps.contains(&"server_request.mcp_elicitation"));
}

#[tokio::test]
async fn drive_turn_bridges_runtime_child_events_and_persists_thread_id() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_bridge".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id: workspace_id.clone(),
        session_id: session.id.clone(),
    };

    let script = r#"
read _line
printf '%s\n' '{"id":1,"result":{"threadId":"thread_bridge"}}'
read _line
printf '%s\n' '{"id":2,"result":{"turnId":"turn_bridge"}}'
printf '%s\n' '{"method":"assistant/delta","params":{"content":"bridge "}}'
printf '%s\n' '{"method":"assistant/done","params":{"content":"bridge ok"}}'
printf '%s\n' '{"method":"thread/token_usage/updated","params":{"last":{"totalTokens":7,"inputTokens":3,"cachedInputTokens":1,"outputTokens":4,"reasoningOutputTokens":0},"modelContextWindow":128000}}'
printf '%s\n' '{"method":"turn/completed","params":{"turnId":"turn_bridge"}}'
"#;
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();

    let assistant = runtime
        .drive_turn(
            &RuntimeTurnInput {
                text: "bridge test".to_string(),
                local_images: Vec::new(),
            },
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("drive turn");

    assert_eq!(assistant, "bridge ok");
    assert_eq!(conn.active_thread_id.as_deref(), Some("thread_bridge"));
    assert!(conn.active_turn_id.is_none());
    assert_eq!(
        *runtime.thread_id.lock().await,
        Some("thread_bridge".to_string())
    );
    assert_eq!(
        session_svc::load_runtime_thread_id(&workspace, &session.id).expect("runtime id"),
        Some("thread_bridge".to_string())
    );
    assert_eq!(*slot_status.lock().await, RuntimeSlotStatus::Running);
    let _ = conn.child.kill().await;
}

#[tokio::test]
async fn drive_turn_emits_mcp_status_buffered_before_thread_response() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_mcp_buffered".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id: workspace_id.clone(),
        session_id: session.id.clone(),
    };

    let script = r#"
read _thread_line
printf '%s\n' '{"method":"mcp/server_status_updated","params":{"serverName":"chawork_workspace","status":"ready"}}'
printf '%s\n' '{"id":1,"result":{"threadId":"thread_mcp_buffered"}}'
read _turn_line
printf '%s\n' '{"id":2,"result":{"turnId":"turn_mcp_buffered"}}'
printf '%s\n' '{"method":"assistant/done","params":{"content":"ok"}}'
printf '%s\n' '{"method":"turn/completed","params":{"turnId":"turn_mcp_buffered"}}'
"#;
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<String>();
    let _listener = app.listen("codex-event", move |event| {
        let _ = event_tx.send(event.payload().to_string());
    });

    let assistant = runtime
        .drive_turn(
            &text_turn_input("mcp status test"),
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("drive turn");

    assert_eq!(assistant, "ok");
    let mut saw_ready = false;
    while let Ok(Some(payload)) =
        tokio::time::timeout(Duration::from_millis(50), event_rx.recv()).await
    {
        let event: serde_json::Value = serde_json::from_str(&payload).expect("event json");
        if event["type"].as_str() == Some("mcp_server_status_updated") {
            assert_eq!(event["workspace_id"].as_str(), Some(workspace_id.as_str()));
            assert_eq!(event["session_id"].as_str(), Some(session.id.as_str()));
            assert_eq!(event["server_name"].as_str(), Some("chawork_workspace"));
            assert_eq!(event["status"].as_str(), Some("ready"));
            saw_ready = true;
            break;
        }
    }
    assert!(
        saw_ready,
        "MCP status notification emitted before thread/start response must reach codex-event"
    );
    let _ = conn.child.kill().await;
}

#[tokio::test]
async fn drive_turn_sends_text_and_local_images_as_structured_turn_input() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    let turn_capture = tmp.path().join("turn-start.json");
    let image_path = tmp.path().join("image.png");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");
    std::fs::write(&image_path, b"image").expect("image");

    let workspace_id = "workspace_bridge".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id: workspace_id.clone(),
        session_id: session.id.clone(),
    };

    let script = format!(
        r#"
read _thread_line
printf '%s\n' '{{"id":1,"result":{{"threadId":"thread_images"}}}}'
read turn_line
printf '%s\n' "$turn_line" > '{}'
printf '%s\n' '{{"id":2,"result":{{"turnId":"turn_images"}}}}'
printf '%s\n' '{{"method":"turn/completed","params":{{"turnId":"turn_images"}}}}'
"#,
        turn_capture.to_string_lossy()
    );
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();

    runtime
        .drive_turn(
            &RuntimeTurnInput {
                text: "describe it".to_string(),
                local_images: vec![RuntimeLocalImage {
                    path: image_path.to_string_lossy().into_owned(),
                }],
            },
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("drive turn");

    let captured = std::fs::read_to_string(&turn_capture).expect("captured turn start");
    let request: serde_json::Value = serde_json::from_str(captured.trim()).expect("turn json");
    assert_eq!(request["method"].as_str(), Some("turn/start"));
    assert_eq!(request["params"]["input"][0]["type"].as_str(), Some("text"));
    assert_eq!(
        request["params"]["input"][0]["text"].as_str(),
        Some("describe it")
    );
    assert_eq!(
        request["params"]["input"][1]["type"].as_str(),
        Some("local_image")
    );
    assert_eq!(
        request["params"]["input"][1]["path"].as_str(),
        Some(image_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        request["params"]["input"][1]["detail"].as_str(),
        Some("high")
    );
    let _ = conn.child.kill().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn ensure_session_started_spawns_runtime_and_sends_required_capabilities() {
    use std::os::unix::fs::PermissionsExt;

    let _env_guard = RUNTIME_ENV_LOCK.lock().expect("runtime env lock");
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    let fake_runtime = tmp.path().join("fake-chawork-runtime");
    let initialize_capture = tmp.path().join("initialize.json");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    std::fs::write(
            &fake_runtime,
            format!(
                r#"#!/bin/sh
read init_line
printf '%s\n' "$init_line" > '{}'
printf '%s\n' '{{"id":1,"result":{{"contractVersion":1,"runtimeVersion":"0.1.0-test","codexVersion":"codex-test","capabilityMatrixVersion":"matrix-v1","releaseUnitId":"release-test","capabilities":{{"thread.start":"normalized","turn.start.text":"normalized"}},"unsupportedCapabilities":["mcpServer/tool/call"],"dream":{{"capabilityVersion":1,"promptVersion":"dream-v1","supportedPhases":["phase1","phase2"]}}}}}}'
read _shutdown_line
"#,
                initialize_capture.to_string_lossy()
            ),
        )
        .expect("write fake runtime");
    let mut perms = std::fs::metadata(&fake_runtime)
        .expect("fake runtime metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake_runtime, perms).expect("chmod fake runtime");

    let previous_cli = std::env::var_os("CHAWORK_RUNTIME_CLI");
    std::env::set_var("CHAWORK_RUNTIME_CLI", &fake_runtime);

    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });

    runtime
        .ensure_session_started()
        .await
        .expect("ensure session started");
    {
        let guard = runtime.session_connection.lock().await;
        assert!(guard.as_ref().is_some_and(|conn| conn.initialized));
    }
    let metadata = runtime
        .runtime_metadata()
        .await
        .expect("runtime initialize metadata");
    assert_eq!(metadata.release_unit_id.as_deref(), Some("release-test"));
    assert_eq!(metadata.runtime_version.as_deref(), Some("0.1.0-test"));
    assert_eq!(
        metadata.capabilities["thread.start"].as_str(),
        Some("normalized")
    );
    assert_eq!(
        metadata.unsupported_capabilities,
        vec!["mcpServer/tool/call".to_string()]
    );
    runtime
        .shutdown_session()
        .await
        .expect("shutdown fake runtime");

    match previous_cli {
        Some(value) => std::env::set_var("CHAWORK_RUNTIME_CLI", value),
        None => std::env::remove_var("CHAWORK_RUNTIME_CLI"),
    }

    let initialize_raw = std::fs::read_to_string(&initialize_capture).expect("initialize json");
    let initialize: serde_json::Value =
        serde_json::from_str(initialize_raw.trim()).expect("initialize parses");
    assert_eq!(initialize["method"].as_str(), Some("runtime/initialize"));
    assert_eq!(initialize["params"]["contractVersion"].as_i64(), Some(1));
    assert_eq!(
        initialize["params"]["workspacePath"].as_str(),
        Some(workspace.to_string_lossy().as_ref())
    );
    let caps = initialize["params"]["requiredCapabilities"]
        .as_array()
        .expect("required capabilities");
    for cap in [
        "thread.start",
        "turn.start.text",
        "turn.start.skill",
        "turn.start.mention",
        "server_request.command_approval",
        "server_request.user_input",
        "server_request.mcp_elicitation",
    ] {
        assert!(
            caps.iter().any(|value| value.as_str() == Some(cap)),
            "missing required capability {cap}"
        );
    }
}

#[tokio::test]
async fn drive_turn_responds_to_user_input_with_real_owner() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    let response_capture = tmp.path().join("server-request-response.json");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_user_input".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id,
        session_id: session.id.clone(),
    };

    let script = format!(
        r#"
read _line
printf '%s\n' '{{"id":1,"result":{{"threadId":"thread_user_input"}}}}'
read _line
printf '%s\n' '{{"id":2,"result":{{"turnId":"turn_user_input"}}}}'
printf '%s\n' '{{"method":"user_input/requested","params":{{"requestId":"req_user_input","questions":[{{"id":"q1","label":"Question","question":"Continue?"}}]}}}}'
read response_line
printf '%s\n' "$response_line" > '{}'
printf '%s\n' '{{"method":"assistant/done","params":{{"content":"after input"}}}}'
printf '%s\n' '{{"method":"turn/completed","params":{{"turnId":"turn_user_input"}}}}'
"#,
        response_capture.to_string_lossy()
    );
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let answer_tx = runtime.user_input_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        answer_tx
            .send((
                "req_user_input".to_string(),
                json!({"q1": {"answers": ["yes"]}}),
            ))
            .await
            .expect("send user input answer");
    });

    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();
    let assistant = runtime
        .drive_turn(
            &text_turn_input("need input"),
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("drive turn");

    assert_eq!(assistant, "after input");
    assert_eq!(*slot_status.lock().await, RuntimeSlotStatus::Running);
    assert!(
        runtime.pending_request_owners.lock().await.is_empty(),
        "terminal turn must clear pending requests"
    );

    let response_raw = std::fs::read_to_string(&response_capture).expect("response capture");
    let response: serde_json::Value =
        serde_json::from_str(response_raw.trim()).expect("response json");
    assert_eq!(response["method"].as_str(), Some("serverRequest/respond"));
    let params = &response["params"];
    assert_eq!(params["kind"].as_str(), Some("user_input"));
    assert_eq!(params["requestId"].as_str(), Some("req_user_input"));
    assert_eq!(
        params["owner"]["workspaceId"].as_str(),
        Some("workspace_user_input")
    );
    assert_eq!(
        params["owner"]["sessionId"].as_str(),
        Some(session.id.as_str())
    );
    assert_eq!(
        params["owner"]["threadId"].as_str(),
        Some("thread_user_input")
    );
    assert_eq!(params["owner"]["turnId"].as_str(), Some("turn_user_input"));
    assert_eq!(
        params["owner"]["requestId"].as_str(),
        Some("req_user_input")
    );
    assert_eq!(params["answers"]["q1"]["answers"][0].as_str(), Some("yes"));
    let _ = conn.child.kill().await;
}

#[tokio::test]
async fn drive_turn_rejects_raw_codex_server_request_without_product_handler() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    let reject_capture = tmp.path().join("server-request-reject.json");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_raw_request".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id,
        session_id: session.id.clone(),
    };

    let script = format!(
        r#"
read _line
printf '%s\n' '{{"id":1,"result":{{"threadId":"thread_raw_request"}}}}'
read _line
printf '%s\n' '{{"id":2,"result":{{"turnId":"turn_raw_request"}}}}'
printf '%s\n' '{{"method":"codex/serverRequest","params":{{"type":"codex/serverRequest","requestId":"req_raw_request","owner":{{"requestId":"req_raw_request"}},"codexMethod":"server_request/dynamic_tool_call","payload":{{"codexVariant":"DynamicToolCall"}}}}}}'
read reject_line
printf '%s\n' "$reject_line" > '{}'
printf '%s\n' '{{"method":"assistant/done","params":{{"content":"after raw request"}}}}'
printf '%s\n' '{{"method":"turn/completed","params":{{"turnId":"turn_raw_request"}}}}'
"#,
        reject_capture.to_string_lossy()
    );
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();

    let assistant = runtime
        .drive_turn(
            &text_turn_input("raw request"),
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("drive turn");

    assert_eq!(assistant, "after raw request");
    assert_eq!(*slot_status.lock().await, RuntimeSlotStatus::Running);
    assert!(
        runtime.pending_request_owners.lock().await.is_empty(),
        "raw debug requests must not enter the product review queue"
    );

    let reject_raw = std::fs::read_to_string(&reject_capture).expect("reject capture");
    let reject: serde_json::Value = serde_json::from_str(reject_raw.trim()).expect("reject json");
    assert_eq!(reject["method"].as_str(), Some("serverRequest/reject"));
    let params = &reject["params"];
    assert_eq!(params["requestId"].as_str(), Some("req_raw_request"));
    assert_eq!(
        params["reason"].as_str(),
        Some("raw Codex ServerRequest is not handled by ChaWork app")
    );
    assert_eq!(
        params["owner"]["workspaceId"].as_str(),
        Some("workspace_raw_request")
    );
    assert_eq!(
        params["owner"]["sessionId"].as_str(),
        Some(session.id.as_str())
    );
    assert_eq!(
        params["owner"]["threadId"].as_str(),
        Some("thread_raw_request")
    );
    assert_eq!(params["owner"]["turnId"].as_str(), Some("turn_raw_request"));
    assert_eq!(
        params["owner"]["requestId"].as_str(),
        Some("req_raw_request")
    );
    let _ = conn.child.kill().await;
}

#[tokio::test]
async fn drive_turn_returns_tool_error_text_when_turn_completes_without_assistant() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_tool_error".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id,
        session_id: session.id.clone(),
    };

    let script = r#"
read _line
printf '%s\n' '{"id":1,"result":{"threadId":"thread_tool_error"}}'
read _line
printf '%s\n' '{"id":2,"result":{"turnId":"turn_tool_error"}}'
printf '%s\n' '{"method":"tool/call_completed","params":{"itemId":"tool_mcp_1","tool":"mcp:calendar:create","args":{"status":"failed"},"error":{"message":"calendar MCP failed"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"turnId":"turn_tool_error"}}'
"#;
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<String>();
    let _listener = app.listen("codex-event", move |event| {
        let _ = event_tx.send(event.payload().to_string());
    });

    let assistant = runtime
        .drive_turn(
            &text_turn_input("tool error"),
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("tool error turn should still return a transcript fallback");

    assert_eq!(assistant, "工具调用失败：calendar MCP failed");
    let mut saw_assistant_done = false;
    while let Ok(Some(payload)) =
        tokio::time::timeout(Duration::from_millis(50), event_rx.recv()).await
    {
        let event: serde_json::Value = serde_json::from_str(&payload).expect("event json");
        if event["type"].as_str() == Some("assistant_done") {
            assert_eq!(
                event["content"].as_str(),
                Some("工具调用失败：calendar MCP failed")
            );
            saw_assistant_done = true;
        }
    }
    assert!(
        saw_assistant_done,
        "transcript fallback for tool failures must be visible in the live chat stream"
    );
    assert_eq!(*slot_status.lock().await, RuntimeSlotStatus::Running);
    let _ = conn.child.kill().await;
}

#[tokio::test]
async fn drive_turn_returns_and_emits_turn_failed_text_without_assistant() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_turn_failed".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id,
        session_id: session.id.clone(),
    };

    let script = r#"
read _line
printf '%s\n' '{"id":1,"result":{"threadId":"thread_turn_failed"}}'
read _line
printf '%s\n' '{"id":2,"result":{"turnId":"turn_failed"}}'
printf '%s\n' '{"method":"turn/failed","params":{"turnId":"turn_failed","error":{"message":"MCP tool aborted the turn"}}}'
"#;
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<String>();
    let _listener = app.listen("codex-event", move |event| {
        let _ = event_tx.send(event.payload().to_string());
    });

    let assistant = runtime
        .drive_turn(
            &text_turn_input("turn failed"),
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect("turn failure should still return a transcript fallback");

    assert_eq!(assistant, "本轮执行失败：MCP tool aborted the turn");
    let mut saw_assistant_done = false;
    let mut saw_error = false;
    while let Ok(Some(payload)) =
        tokio::time::timeout(Duration::from_millis(50), event_rx.recv()).await
    {
        let event: serde_json::Value = serde_json::from_str(&payload).expect("event json");
        match event["type"].as_str() {
            Some("assistant_done") => {
                assert_eq!(
                    event["content"].as_str(),
                    Some("本轮执行失败：MCP tool aborted the turn")
                );
                saw_assistant_done = true;
            }
            Some("error") => {
                assert_eq!(event["message"].as_str(), Some("MCP tool aborted the turn"));
                saw_error = true;
            }
            _ => {}
        }
    }
    assert!(
        saw_assistant_done,
        "turn/failed transcript fallback must be visible in the live chat stream"
    );
    assert!(
        saw_error,
        "turn/failed must still surface as a runtime error event"
    );
    assert_eq!(*slot_status.lock().await, RuntimeSlotStatus::Running);
    let _ = conn.child.kill().await;
}

#[tokio::test]
async fn drive_turn_rejects_normalized_request_without_complete_owner() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let workspace = tmp.path().join("workspace");
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&codex_home).expect("codex home");

    let workspace_id = "workspace_missing_owner".to_string();
    let session = session_svc::create(&workspace, &workspace_id).expect("session");
    let persist = ThreadPersistCtx {
        workspace: workspace.clone(),
        workspace_id,
        session_id: session.id.clone(),
    };

    let script = r#"
read _line
printf '%s\n' '{"id":1,"result":{"threadId":"thread_missing_owner"}}'
read _line
printf '%s\n' '{"id":2,"result":{"turnId":"turn_missing_owner"}}'
printf '%s\n' '{"method":"user_input/requested","params":{"type":"user_input/requested","questions":[]}}'
"#;
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().expect("spawn fake runtime");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let (events_tx, events_rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_line(&mut reader).await {
                Ok(Some(line)) => {
                    if let Some(message) = route_runtime_line(&line) {
                        if events_tx.send(message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events_tx.send(RuntimeMessage::Eof).await;
                    break;
                }
            }
        }
    });

    let mut conn = RuntimeConnection::new(child, stdin, events_rx);
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: workspace.to_string_lossy().into_owned(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        runtime_home: workspace
            .join(".runtime-home")
            .to_string_lossy()
            .into_owned(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec![workspace.to_string_lossy().into_owned()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });
    let slot_status = AsyncMutex::new(RuntimeSlotStatus::Running);
    let app = tauri::test::mock_app();

    let err = runtime
        .drive_turn(
            &text_turn_input("missing owner"),
            app.handle(),
            &mut conn,
            Some(&slot_status),
            Some(&persist),
        )
        .await
        .expect_err("incomplete normalized request owner must be fatal");

    assert!(err.to_string().contains("runtime reported fatal error"));
    assert_eq!(
        *slot_status.lock().await,
        RuntimeSlotStatus::Running,
        "incomplete owner must not move slot into Pending"
    );
    assert!(
        runtime.pending_request_owners.lock().await.is_empty(),
        "incomplete owner must not be registered"
    );
    let _ = conn.child.kill().await;
}

#[test]
fn missing_session_thread_id_starts_new_runtime_thread() {
    let selected = normalize_runtime_thread_id(None);

    assert_eq!(selected, None);
}

#[test]
fn existing_session_thread_id_is_used_for_resume() {
    let selected = normalize_runtime_thread_id(Some("session-thread".to_string()));

    assert_eq!(selected.as_deref(), Some("session-thread"));
}

#[test]
fn blank_session_thread_id_is_ignored() {
    let selected = normalize_runtime_thread_id(Some("   ".to_string()));

    assert_eq!(selected, None);
}

#[test]
fn thread_request_params_include_workspace_roots_and_execution_policy() {
    let runtime = CodexRuntime::new(RuntimeConfig {
        workspace_path: "/tmp/chawork-ws".to_string(),
        codex_home: "/tmp/chawork-codex-home".to_string(),
        runtime_home: "/tmp/chawork-runtime-home".to_string(),
        model: "mock-model".to_string(),
        api_key: String::new(),
        runtime_workspace_roots: vec!["/tmp/chawork-ws".to_string()],
        approval_policy: "on-request".to_string(),
        sandbox: "workspace-write".to_string(),
    });

    let params = thread_request_base_params(&runtime, "session_1");

    assert_eq!(params["workspacePath"], "/tmp/chawork-ws");
    assert_eq!(params["sessionId"], "session_1");
    assert_eq!(params["codexHome"], "/tmp/chawork-codex-home");
    assert_eq!(params["enableCodexApiKeyEnv"], false);
    assert_eq!(params["model"], "mock-model");
    assert_eq!(params["runtimeWorkspaceRoots"][0], "/tmp/chawork-ws");
    assert_eq!(params["approvalPolicy"], "on-request");
    assert_eq!(params["sandbox"], "workspace-write");
}
