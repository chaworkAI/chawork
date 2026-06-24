//! ChaWork-facing events emitted over `codex-event`.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_context_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanStep {
    pub step: String,
    pub status: String,
}

/// ChaWork-level events (stable for React `useCodexEvents` and `types/events.ts`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChaWorkEvent {
    #[serde(rename = "assistant_delta")]
    AssistantDelta { content: String },
    #[serde(rename = "assistant_done")]
    AssistantDone { content: String },
    #[serde(rename = "thinking")]
    Thinking { summary: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { content: String },
    #[serde(rename = "thinking_done")]
    ThinkingDone,
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        args: serde_json::Value,
        id: String,
    },
    #[serde(rename = "tool_delta")]
    ToolDelta {
        id: String,
        tool: String,
        content: String,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
        result: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "file_change")]
    FileChange {
        path: String,
        diff: String,
        action: String,
    },
    #[serde(rename = "file_change_delta")]
    FileChangeDelta { id: String, content: String },
    #[serde(rename = "plan_update")]
    PlanUpdate {
        #[serde(skip_serializing_if = "Option::is_none")]
        explanation: Option<String>,
        steps: Vec<PlanStep>,
    },
    #[serde(rename = "plan_delta")]
    PlanDelta { content: String },
    #[serde(rename = "plan_done")]
    PlanDone { content: String },
    #[serde(rename = "approval_request")]
    ApprovalRequest {
        id: String,
        method: String,
        title: String,
        description: String,
        risk: String,
        params: serde_json::Value,
    },
    #[serde(rename = "user_input_request")]
    UserInputRequest {
        id: String,
        method: String,
        title: String,
        description: String,
        questions: serde_json::Value,
        params: serde_json::Value,
    },
    #[serde(rename = "mcp_elicitation_request")]
    McpElicitationRequest {
        id: String,
        server_name: String,
        mode: String,
        message: String,
        params: serde_json::Value,
    },
    #[serde(rename = "mcp_oauth_login_completed")]
    McpOauthLoginCompleted {
        server_name: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "mcp_server_status_updated")]
    McpServerStatusUpdated {
        server_name: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "runtime_debug")]
    RuntimeDebug {
        method: String,
        category: String,
        params: serde_json::Value,
    },
    #[serde(rename = "error")]
    Error { message: String, recoverable: bool },
    #[serde(rename = "turn_complete")]
    TurnComplete {
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
    },
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChaWorkEventEnvelope<'a> {
    pub workspace_id: &'a str,
    pub session_id: &'a str,
    #[serde(flatten)]
    pub event: &'a ChaWorkEvent,
}

#[cfg(test)]
mod tests {
    use super::{ChaWorkEvent, ChaWorkEventEnvelope};

    #[test]
    fn event_envelope_keeps_type_and_owner_fields_flat() {
        let event = ChaWorkEvent::AssistantDelta {
            content: "hello".to_string(),
        };
        let payload = serde_json::to_value(ChaWorkEventEnvelope {
            workspace_id: "workspace-a",
            session_id: "session-1",
            event: &event,
        })
        .expect("serialize event envelope");

        assert_eq!(payload["workspace_id"], "workspace-a");
        assert_eq!(payload["session_id"], "session-1");
        assert_eq!(payload["type"], "assistant_delta");
        assert_eq!(payload["content"], "hello");
    }

    #[test]
    fn plan_events_serialize_to_product_shapes() {
        let update = serde_json::to_value(ChaWorkEvent::PlanUpdate {
            explanation: Some("Need inspect then patch".to_string()),
            steps: vec![
                super::PlanStep {
                    step: "Inspect current mapper".to_string(),
                    status: "completed".to_string(),
                },
                super::PlanStep {
                    step: "Patch plan mapping".to_string(),
                    status: "inProgress".to_string(),
                },
            ],
        })
        .expect("serialize plan update");
        assert_eq!(update["type"], "plan_update");
        assert_eq!(update["steps"][0]["status"], "completed");

        let delta = serde_json::to_value(ChaWorkEvent::PlanDelta {
            content: "1. inspect".to_string(),
        })
        .expect("serialize plan delta");
        assert_eq!(delta["type"], "plan_delta");
        assert_eq!(delta["content"], "1. inspect");

        let done = serde_json::to_value(ChaWorkEvent::PlanDone {
            content: "1. inspect\n2. patch".to_string(),
        })
        .expect("serialize plan done");
        assert_eq!(done["type"], "plan_done");
        assert_eq!(done["content"], "1. inspect\n2. patch");
    }

    #[test]
    fn tool_and_file_delta_events_serialize_to_product_shapes() {
        let tool = serde_json::to_value(ChaWorkEvent::ToolDelta {
            id: "item_c".to_string(),
            tool: "shell".to_string(),
            content: "running tests\n".to_string(),
        })
        .expect("serialize tool delta");
        assert_eq!(tool["type"], "tool_delta");
        assert_eq!(tool["id"], "item_c");
        assert_eq!(tool["tool"], "shell");
        assert_eq!(tool["content"], "running tests\n");

        let file = serde_json::to_value(ChaWorkEvent::FileChangeDelta {
            id: "item_f".to_string(),
            content: "applying patch\n".to_string(),
        })
        .expect("serialize file delta");
        assert_eq!(file["type"], "file_change_delta");
        assert_eq!(file["id"], "item_f");
        assert_eq!(file["content"], "applying patch\n");
    }

    #[test]
    fn mcp_status_events_serialize_to_product_shapes() {
        let oauth = serde_json::to_value(ChaWorkEvent::McpOauthLoginCompleted {
            server_name: "ctx7".to_string(),
            success: false,
            error: Some("denied".to_string()),
        })
        .expect("serialize mcp oauth status");
        assert_eq!(oauth["type"], "mcp_oauth_login_completed");
        assert_eq!(oauth["server_name"], "ctx7");
        assert_eq!(oauth["success"], false);
        assert_eq!(oauth["error"], "denied");

        let startup = serde_json::to_value(ChaWorkEvent::McpServerStatusUpdated {
            server_name: "ctx7".to_string(),
            status: "failed".to_string(),
            error: Some("spawn failed".to_string()),
        })
        .expect("serialize mcp server status");
        assert_eq!(startup["type"], "mcp_server_status_updated");
        assert_eq!(startup["server_name"], "ctx7");
        assert_eq!(startup["status"], "failed");
        assert_eq!(startup["error"], "spawn failed");
    }
}
