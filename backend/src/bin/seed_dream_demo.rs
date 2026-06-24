//! Seed a Dream manual-test fixture into the local ChaWork data directory.
//!
//! Usage:
//!   cargo run --manifest-path backend/Cargo.toml --bin seed_dream_demo
//!   cargo run --manifest-path backend/Cargo.toml --bin seed_dream_demo -- --with-pending
//!   cargo run --manifest-path backend/Cargo.toml --bin seed_dream_demo -- --workspace ~/my-ws

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use chawork_lib::services::dream::{
    self, DreamDecision, DreamPrepareInput, DreamResult, PromptUpdate, SourceSessionRef,
};
use chawork_lib::services::employee::{self, CreateEmployeeInput};
use chawork_lib::services::root_workspace;
use chawork_lib::services::session;
use chawork_lib::services::workspace::{self, WorkspaceState};

const EMPLOYEE_ID: &str = "dream-demo";
const EMPLOYEE_NAME: &str = "Dream 演示助手";
const WS_NAME: &str = "dream-demo-ws";

const BASELINE_PROMPT: &str = r#"# Dream 演示助手

你是 ChaWork 里的演示用员工，负责帮用户整理资料与回答问题。

## 职责
- 理解用户问题并给出可行建议
- 必要时引用工作区 wiki / 文档

## 当前限制
- 尚未定义统一的回复长度与格式规范
"#;

fn default_workspace_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("chawork-dream-demo")
}

fn install_dir() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.chawork.app")
}

fn parse_args() -> (PathBuf, bool) {
    let mut workspace = default_workspace_path();
    let mut with_pending = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--with-pending" => with_pending = true,
            "--workspace" => {
                if let Some(p) = args.next() {
                    workspace = PathBuf::from(p);
                }
            }
            "--help" | "-h" => {
                eprintln!(
                    "用法: seed_dream_demo [--with-pending] [--workspace PATH]\n\
                     \n\
                     --with-pending  额外注入一条 update_required，便于直接测 Review Queue\n\
                     --workspace     工作区路径（默认 ~/Documents/chawork-dream-demo）"
                );
                std::process::exit(0);
            }
            other => eprintln!("未知参数: {other}（可用 --help）"),
        }
    }
    (workspace, with_pending)
}

fn append_turn(ws: &Path, session_id: &str, role: &str, content: &str) {
    let entry = serde_json::json!({
        "role": role,
        "content": content,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    session::append_transcript(ws, session_id, &entry).expect("append transcript");
    session::sync_meta_from_transcript(ws, session_id).expect("sync meta");
}

fn seed_session(ws: &Path, ws_id: &str, turns: &[(&str, &str)]) -> String {
    let meta = session::create(ws, ws_id).expect("create session");
    for (role, content) in turns {
        append_turn(ws, &meta.id, role, content);
        thread::sleep(Duration::from_millis(50));
    }
    meta.id
}

fn ensure_employee(root: &root_workspace::RootWorkspace) {
    if employee::get_detail(root, EMPLOYEE_ID).is_ok() {
        println!("员工已存在，重置 prompt: {EMPLOYEE_ID}");
    } else {
        employee::create(
            root,
            CreateEmployeeInput {
                id: EMPLOYEE_ID.to_string(),
                name: EMPLOYEE_NAME.to_string(),
                description: "用于手动测试 Dream 闭环的演示员工".to_string(),
                initial_prompt: BASELINE_PROMPT.to_string(),
                root_skill_ids: vec![],
            },
        )
        .expect("create employee");
        println!("已创建员工: {EMPLOYEE_ID}");
    }

    let prompt_path = root.employees_dir().join(EMPLOYEE_ID).join("prompt.md");
    fs::write(&prompt_path, BASELINE_PROMPT).expect("write baseline prompt");
}

fn ensure_workspace(
    root: &root_workspace::RootWorkspace,
    ws_path: &Path,
) -> (WorkspaceState, Vec<String>) {
    fs::create_dir_all(ws_path.join(".chawork/state")).expect("workspace dirs");
    fs::create_dir_all(ws_path.join("sessions")).expect("sessions dir");

    let ws_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let ws = WorkspaceState {
        id: ws_id.clone(),
        name: WS_NAME.to_string(),
        path: fs::canonicalize(ws_path)
            .unwrap_or_else(|_| ws_path.to_path_buf())
            .to_string_lossy()
            .into_owned(),
        created_at: now.clone(),
        last_active_at: now,
        active_session_id: None,
        domain_pack_id: None,
        index_status: "stale".to_string(),
        pending_proposals_count: 0,
        bound_employee_name: None,
        bound_employee_id: None,
    };
    workspace::persist_workspace(ws_path, &ws).expect("persist workspace");

    employee::bind_workspace(root, EMPLOYEE_ID, ws_path, &ws_id, WS_NAME).expect("bind");
    workspace::add_known(&root.known_workspaces_path(), &ws).expect("add known");

    // 清空旧 sessions（若重复执行）
    if ws_path.join("sessions").is_dir() {
        for entry in fs::read_dir(ws_path.join("sessions"))
            .into_iter()
            .flatten()
            .flatten()
        {
            if entry.path().is_dir() {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    let s1 = seed_session(
        ws_path,
        &ws_id,
        &[
            ("user", "帮我总结一下这份资料的核心观点。"),
            (
                "assistant",
                "好的。首先我们需要从背景说起，这份资料涉及多个方面……（此处省略大段冗长说明）",
            ),
            ("user", "太长了，能不能简短一点？"),
        ],
    );
    thread::sleep(Duration::from_millis(200));

    let s2 = seed_session(
        ws_path,
        &ws_id,
        &[
            ("user", "把刚才的结论改成 3 条 bullet。"),
            (
                "assistant",
                "1. 第一点……\n2. 第二点……\n3. 第三点……\n另外补充说明：……",
            ),
            ("user", "补充说明可以不要，只要三条。"),
        ],
    );
    thread::sleep(Duration::from_millis(200));

    let s3 = seed_session(
        ws_path,
        &ws_id,
        &[
            ("user", "以后回复都用中文，且不超过 3 句话，可以吗？"),
            (
                "assistant",
                "明白，我会尽量简洁。如果内容确实复杂，我会先问你是否需要展开。",
            ),
            ("user", "对，默认简洁，除非我让你展开。"),
        ],
    );

    (ws, vec![s1, s2, s3])
}

fn sample_source_sessions(sessions: &[(String, String)]) -> Vec<SourceSessionRef> {
    sessions
        .iter()
        .map(|(workspace_id, session_id)| SourceSessionRef {
            workspace_id: workspace_id.clone(),
            session_id: session_id.clone(),
            last_updated_at: None,
        })
        .collect()
}

fn sample_update_result(dream_run_id: &str, sessions: &[(String, String)]) -> DreamResult {
    DreamResult {
        decision: DreamDecision::UpdateRequired,
        target_employee_id: EMPLOYEE_ID.to_string(),
        dream_run_id: dream_run_id.to_string(),
        summary: "用户在多次会话中反复要求：中文、简洁、默认不超过 3 句话，复杂内容先询问是否展开。".to_string(),
        source_sessions: sample_source_sessions(sessions),
        updates: Some(vec![PromptUpdate {
            section: "沟通风格".to_string(),
            action: "add".to_string(),
            content: "- 默认使用中文回复\n- 默认不超过 3 句话\n- 内容复杂时先询问用户是否需要展开\n- 用户明确要求要点时，只输出 bullet，不要附加「补充说明」段落"
                .to_string(),
            reason: "会话 s1-s3 中用户多次抱怨回复过长、要求 bullet 与 3 句上限".to_string(),
        }]),
        impact: Some("新会话将默认更短、更结构化，减少冗长回复".to_string()),
        status: "pending".to_string(),
        source_prompt_path: Some(format!("employees/{EMPLOYEE_ID}/prompt.md")),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    }
}

fn sample_update_json(dream_run_id: &str, sessions: &[(String, String)]) -> String {
    serde_json::to_string_pretty(&sample_update_result(dream_run_id, sessions))
        .expect("serialize sample update result")
}

fn sample_no_update_json(dream_run_id: &str, sessions: &[(String, String)]) -> String {
    let result = DreamResult {
        decision: DreamDecision::NoUpdate,
        target_employee_id: EMPLOYEE_ID.to_string(),
        dream_run_id: dream_run_id.to_string(),
        summary: "当前 prompt 已足够，无需变更（演示用 no_update）。".to_string(),
        source_sessions: sample_source_sessions(sessions),
        updates: None,
        impact: None,
        status: "pending".to_string(),
        source_prompt_path: Some(format!("employees/{EMPLOYEE_ID}/prompt.md")),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    };
    serde_json::to_string_pretty(&result).expect("serialize sample no-update result")
}

fn write_sample_files(repo_root: &Path, ws_id: &str, session_ids: &[String]) {
    let out_dir = repo_root.join("scripts/dream-demo/generated");
    fs::create_dir_all(&out_dir).ok();
    let sessions: Vec<(String, String)> = session_ids
        .iter()
        .map(|sid| (ws_id.to_string(), sid.clone()))
        .collect();

    fs::write(
        out_dir.join("update_required.sample.json"),
        sample_update_json("dream-run-REPLACE-ME", &sessions),
    )
    .ok();
    fs::write(
        out_dir.join("no_update.sample.json"),
        sample_no_update_json("dream-run-REPLACE-ME", &sessions),
    )
    .ok();
}

fn main() {
    let (ws_path, with_pending) = parse_args();
    let install = install_dir();
    let root = root_workspace::init_or_open(&install).expect("init root");

    ensure_employee(&root);
    let (ws, session_ids) = ensure_workspace(&root, &ws_path);

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    write_sample_files(&repo_root, &ws.id, &session_ids);

    println!();
    println!("=== Dream 演示数据已就绪 ===");
    println!("员工 ID:     {EMPLOYEE_ID}");
    println!("工作区:      {}", ws.path);
    println!("会话数量:    {}（Dream 会取最新 3 个）", session_ids.len());
    println!(
        "Prompt 基线: {}",
        root.employees_dir()
            .join(EMPLOYEE_ID)
            .join("prompt.md")
            .display()
    );
    println!();
    println!("【GUI 测试步骤】");
    println!("1. 启动 ChaWork（pnpm run tauri:dev）");
    println!("2. 切换到工作区「{WS_NAME}」（路径见上）");
    println!("3. 员工管理 → 选择「{EMPLOYEE_NAME}」→ Dream Tab");
    println!("4. 点击「运行 Dream」（需已配置全局 Provider + Codex）");
    println!("5. 期望 Dream 分析 3 个会话后建议增加「沟通风格」章节");
    println!("6. Review Queue → 批准 / 拒绝 → 查看 prompt.md 是否更新");
    println!();
    println!("【无 Codex 时】可先测 prepare + 结构化 pending request：");
    println!(
        "  cargo run --manifest-path backend/Cargo.toml --bin seed_dream_demo -- --with-pending"
    );
    println!();

    if with_pending {
        dream::reject_pending_request(&root, EMPLOYEE_ID).ok();
        let prepare = dream::prepare_dream_run(
            &root,
            DreamPrepareInput {
                target_employee_id: EMPLOYEE_ID.to_string(),
                workspace_filter: None,
            },
        )
        .expect("prepare dream run");

        let sessions: Vec<(String, String)> = prepare
            .selected_sessions
            .iter()
            .map(|s| (s.workspace_id.clone(), s.session_id.clone()))
            .collect();

        let result = sample_update_result(&prepare.dream_run_id, &sessions);
        dream::process_dream_result(&root, &result).expect("persist update_required");

        println!(
            "已注入 pending 审批请求（dream_run_id: {}）",
            prepare.dream_run_id
        );
        println!("→ 打开员工「{EMPLOYEE_NAME}」→ Review Queue 即可直接测批准/拒绝");
    }
}
