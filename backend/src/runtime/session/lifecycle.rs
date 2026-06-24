use tokio::sync::Mutex as AsyncMutex;

use crate::state::RuntimeSlotStatus;

fn mark_slot_pending_for_server_request(status: &mut RuntimeSlotStatus) {
    if matches!(status, RuntimeSlotStatus::Running) {
        *status = RuntimeSlotStatus::Pending;
    }
}

fn mark_slot_running_after_server_request(status: &mut RuntimeSlotStatus) {
    if matches!(status, RuntimeSlotStatus::Pending) {
        *status = RuntimeSlotStatus::Running;
    }
}

pub(super) async fn mark_status_pending(status: Option<&AsyncMutex<RuntimeSlotStatus>>) {
    if let Some(status) = status {
        let mut guard = status.lock().await;
        mark_slot_pending_for_server_request(&mut guard);
    }
}

pub(super) async fn mark_status_running_after_pending(
    status: Option<&AsyncMutex<RuntimeSlotStatus>>,
) {
    if let Some(status) = status {
        let mut guard = status.lock().await;
        mark_slot_running_after_server_request(&mut guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_request_status_transitions_preserve_terminal_states() {
        let mut status = RuntimeSlotStatus::Running;
        mark_slot_pending_for_server_request(&mut status);
        assert_eq!(status, RuntimeSlotStatus::Pending);

        mark_slot_running_after_server_request(&mut status);
        assert_eq!(status, RuntimeSlotStatus::Running);

        let mut cancelling = RuntimeSlotStatus::Cancelling;
        mark_slot_running_after_server_request(&mut cancelling);
        assert_eq!(cancelling, RuntimeSlotStatus::Cancelling);

        let mut error = RuntimeSlotStatus::Error;
        mark_slot_pending_for_server_request(&mut error);
        assert_eq!(error, RuntimeSlotStatus::Error);
    }
}
