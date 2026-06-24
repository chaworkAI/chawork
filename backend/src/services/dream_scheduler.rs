//! Background scheduler for automatic Dream runs.
//!
//! Strategy: daily timed + startup catch-up.
//! On app startup, performs a one-time catch-up check for any missed runs.
//! Then enters an hourly loop that executes Dream Phase 1 for employees whose
//! scheduled time has passed today without a recorded run.

use std::sync::Arc;

use tauri::AppHandle;
use tokio::time::{interval, sleep, Duration, MissedTickBehavior};

use super::dream;
use super::dream_phase1::{self, DreamPhase1Mode};
use crate::state::AppState;

/// Main scheduler entry point — spawned once during `setup()`.
pub async fn start_dream_scheduler(state: Arc<AppState>, app: AppHandle) {
    sleep(Duration::from_secs(10)).await;

    run_scan(state.clone(), app.clone(), "startup catch-up").await;

    let mut ticker = interval(Duration::from_secs(3600));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        run_scan(state.clone(), app.clone(), "hourly check").await;
    }
}

/// Offload the filesystem-heavy due scan to a blocking thread, then execute
/// Dream Phase 1 asynchronously and serially for each due employee.
async fn run_scan(state: Arc<AppState>, app: AppHandle, label: &'static str) {
    let st = state.clone();
    let due = tokio::task::spawn_blocking(move || {
        println!("[dream-scheduler] {label}");
        let due = dream::scan_due_employees(&st.root);
        if !due.is_empty() {
            println!("[dream-scheduler] {label}: {} employees due", due.len());
        }
        due
    })
    .await
    .unwrap_or_default();

    for employee_id in due {
        match dream_phase1::run_phase1_with_runtime(
            &state,
            &app,
            &employee_id,
            DreamPhase1Mode::Scheduled,
        )
        .await
        {
            Ok(Some(result)) => {
                println!(
                    "[dream-scheduler] completed dream run {} for {}",
                    result.dream_run_id, employee_id
                );
            }
            Ok(None) => {
                println!("[dream-scheduler] skipped {employee_id}");
            }
            Err(e) => {
                eprintln!("[dream-scheduler] failed for {employee_id}: {e}");
            }
        }
    }
}
