use serde_json::{json, Value};

use crate::runtime::process::{PendingRequestOwner, ThreadPersistCtx};

pub(super) fn runtime_owner_json(
    persist: Option<&ThreadPersistCtx>,
    thread_id: &str,
    turn_id: &str,
    request_id: &str,
) -> Value {
    json!({
        "workspaceId": persist.map(|ctx| ctx.workspace_id.clone()),
        "sessionId": persist.map(|ctx| ctx.session_id.clone()),
        "threadId": thread_id,
        "turnId": turn_id,
        "requestId": request_id,
    })
}

pub(super) fn pending_request_owner(
    persist: Option<&ThreadPersistCtx>,
    thread_id: &str,
    turn_id: &str,
    request_id: &str,
) -> Option<PendingRequestOwner> {
    let ctx = persist?;
    let owner = PendingRequestOwner {
        workspace_id: ctx.workspace_id.clone(),
        session_id: ctx.session_id.clone(),
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        request_id: request_id.to_string(),
    };
    owner.is_complete().then_some(owner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pending_request_owner_requires_complete_owner() {
        let ctx = ThreadPersistCtx {
            workspace: PathBuf::from("/tmp/workspace"),
            workspace_id: "workspace_1".to_string(),
            session_id: "session_1".to_string(),
        };

        let owner =
            pending_request_owner(Some(&ctx), "thread_1", "turn_1", "request_1").expect("owner");
        assert_eq!(owner.workspace_id, "workspace_1");
        assert_eq!(owner.session_id, "session_1");
        assert_eq!(owner.thread_id, "thread_1");
        assert_eq!(owner.turn_id, "turn_1");
        assert_eq!(owner.request_id, "request_1");

        assert!(pending_request_owner(None, "thread_1", "turn_1", "request_1").is_none());
        assert!(pending_request_owner(Some(&ctx), "", "turn_1", "request_1").is_none());
        assert!(pending_request_owner(Some(&ctx), "thread_1", "", "request_1").is_none());
        assert!(pending_request_owner(Some(&ctx), "thread_1", "turn_1", "").is_none());
    }

    #[test]
    fn runtime_owner_json_preserves_all_owner_fields() {
        let ctx = ThreadPersistCtx {
            workspace: PathBuf::from("/tmp/workspace"),
            workspace_id: "workspace_1".to_string(),
            session_id: "session_1".to_string(),
        };

        let owner = runtime_owner_json(Some(&ctx), "thread_1", "turn_1", "request_1");

        assert_eq!(owner["workspaceId"], "workspace_1");
        assert_eq!(owner["sessionId"], "session_1");
        assert_eq!(owner["threadId"], "thread_1");
        assert_eq!(owner["turnId"], "turn_1");
        assert_eq!(owner["requestId"], "request_1");
    }
}
