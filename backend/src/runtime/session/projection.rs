use serde_json::Value;

use crate::runtime::events::{ChaWorkEvent, PlanStep, TokenUsage};

#[derive(Debug)]
pub(super) enum RuntimeProjection {
    Events(Vec<ChaWorkEvent>),
    TurnCompleted { usage: Option<TokenUsage> },
    TurnInterrupted,
    TurnFailed { message: String },
    RuntimeError { message: String, recoverable: bool },
    BlockingRequest,
    RawServerRequest,
    Ignored,
}

#[derive(Default)]
pub(super) struct RuntimeProjectionState {
    assistant_accum: String,
    assistant_done_emitted: bool,
    last_tool_error: Option<String>,
    latest_usage: Option<TokenUsage>,
    has_image_input: bool,
}

impl RuntimeProjectionState {
    pub(super) fn for_input(has_image_input: bool) -> Self {
        Self {
            has_image_input,
            ..Self::default()
        }
    }

    fn final_assistant_text(&self, terminal_error: Option<&str>) -> String {
        if !self.assistant_accum.trim().is_empty() {
            return self.assistant_accum.clone();
        }
        if let Some(message) = terminal_error.filter(|message| !message.trim().is_empty()) {
            return format!("本轮执行失败：{message}");
        }
        if let Some(message) = self
            .last_tool_error
            .as_ref()
            .filter(|message| !message.trim().is_empty())
        {
            return format!("工具调用失败：{message}");
        }
        String::new()
    }

    pub(super) fn synthetic_assistant_done(
        &mut self,
        terminal_error: Option<&str>,
    ) -> Option<ChaWorkEvent> {
        if self.assistant_done_emitted {
            return None;
        }
        let content = self.final_assistant_text(terminal_error);
        if content.trim().is_empty() {
            return None;
        }
        self.assistant_done_emitted = true;
        Some(ChaWorkEvent::AssistantDone { content })
    }

    pub(super) fn into_final_assistant_text(self, terminal_error: Option<String>) -> String {
        self.final_assistant_text(terminal_error.as_deref())
    }
}

pub(super) fn runtime_debug_event(method: &str, params: Value) -> ChaWorkEvent {
    let category = match method {
        "runtime/audit" => "audit",
        "codex/notification" | "codex/serverRequest" => "raw",
        _ => "runtime",
    };
    ChaWorkEvent::RuntimeDebug {
        method: method.to_string(),
        category: category.to_string(),
        params,
    }
}

fn token_usage_u64(value: &Value, camel: &str, snake: &str) -> Option<u64> {
    value
        .get(camel)
        .or_else(|| value.get(snake))
        .and_then(Value::as_u64)
}

fn token_usage_u64_any(value: &Value, fields: &[(&str, &str)]) -> Option<u64> {
    fields
        .iter()
        .find_map(|(camel, snake)| token_usage_u64(value, camel, snake))
}

fn tool_error_message(error_payload: Option<&Value>) -> Option<String> {
    let value = error_payload?;
    if value.is_null() {
        return None;
    }
    value
        .get("message")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| value.as_str().map(ToString::to_string))
        .or_else(|| serde_json::to_string(value).ok())
}

fn is_response_stream_disconnect(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("stream disconnected before completion")
        || normalized.contains("stream closed before response.completed")
        || normalized.contains("websocket closed by server before response.completed")
}

fn is_likely_image_model_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    let mentions_image = normalized.contains("image")
        || normalized.contains("input_image")
        || normalized.contains("vision")
        || normalized.contains("visual")
        || normalized.contains("modalit")
        || message.contains("图片")
        || message.contains("图像")
        || message.contains("视觉")
        || message.contains("多模态");
    let mentions_unsupported = normalized.contains("does not support")
        || normalized.contains("not support")
        || normalized.contains("unsupported")
        || normalized.contains("invalid content")
        || normalized.contains("content type")
        || normalized.contains("only supports text")
        || message.contains("不支持")
        || message.contains("无法处理")
        || message.contains("不能处理")
        || message.contains("只支持文本")
        || message.contains("仅支持文本")
        || message.contains("文本模型");
    mentions_image && mentions_unsupported
}

pub(super) fn user_facing_turn_error_for_input(message: &str, has_image_input: bool) -> String {
    let trimmed = message.trim();
    if has_image_input && is_likely_image_model_error(trimmed) {
        return "当前模型无法处理图片输入。请在设置中切换到支持图片的多模态模型后重试。"
            .to_string();
    }
    if has_image_input && is_response_stream_disconnect(trimmed) {
        return format!(
            "图片请求已发送到当前模型，但 AI 服务在返回完成信号前关闭了流式连接。请检查当前 provider 的 Responses 图片流式接口兼容性后重试。底层错误：{trimmed}"
        );
    }
    if is_response_stream_disconnect(trimmed) {
        return format!(
            "模型流式连接中断，AI 服务在返回完成信号前关闭了连接。请重试；如果持续出现，切换模型或减少本轮启用的员工技能。底层错误：{trimmed}"
        );
    }
    trimmed.to_string()
}

pub(super) fn runtime_token_usage_from_params(params: &Value) -> Option<TokenUsage> {
    let source = params
        .get("last")
        .or_else(|| params.get("usage"))
        .unwrap_or(params);
    let input_tokens = token_usage_u64_any(
        source,
        &[
            ("inputTokens", "input_tokens"),
            ("promptTokens", "prompt_tokens"),
        ],
    )?;
    let cached_input_tokens =
        token_usage_u64(source, "cachedInputTokens", "cached_input_tokens").unwrap_or(0);
    let output_tokens = token_usage_u64_any(
        source,
        &[
            ("outputTokens", "output_tokens"),
            ("completionTokens", "completion_tokens"),
        ],
    )?;
    let reasoning_output_tokens =
        token_usage_u64(source, "reasoningOutputTokens", "reasoning_output_tokens").unwrap_or(0);
    let completion_tokens = token_usage_u64(source, "completionTokens", "completion_tokens")
        .or_else(|| output_tokens.checked_add(reasoning_output_tokens))?;
    let total_tokens = token_usage_u64(source, "totalTokens", "total_tokens").or_else(|| {
        input_tokens
            .checked_add(output_tokens)
            .and_then(|v| v.checked_add(reasoning_output_tokens))
    })?;
    let model_context_window =
        token_usage_u64(params, "modelContextWindow", "model_context_window")
            .or_else(|| token_usage_u64(source, "modelContextWindow", "model_context_window"));

    Some(TokenUsage {
        prompt_tokens: input_tokens,
        completion_tokens,
        total_tokens,
        input_tokens,
        cached_input_tokens,
        output_tokens,
        reasoning_output_tokens,
        model_context_window,
    })
}

pub(super) fn project_runtime_notification(
    method: &str,
    params: &Value,
    state: &mut RuntimeProjectionState,
) -> RuntimeProjection {
    match method {
        "assistant/delta" => {
            let content = params["content"].as_str().unwrap_or("").to_string();
            state.assistant_accum.push_str(&content);
            RuntimeProjection::Events(vec![ChaWorkEvent::AssistantDelta { content }])
        }
        "assistant/done" => {
            let content = params["content"].as_str().unwrap_or("").to_string();
            if !content.is_empty() {
                state.assistant_accum = content.clone();
            }
            let done_content = if content.is_empty() {
                state.assistant_accum.clone()
            } else {
                content
            };
            if done_content.trim().is_empty() {
                RuntimeProjection::Ignored
            } else {
                state.assistant_done_emitted = true;
                RuntimeProjection::Events(vec![ChaWorkEvent::AssistantDone {
                    content: done_content,
                }])
            }
        }
        "reasoning/delta" => {
            let content = params["content"].as_str().unwrap_or("").to_string();
            RuntimeProjection::Events(vec![ChaWorkEvent::ThinkingDelta { content }])
        }
        "reasoning/done" => RuntimeProjection::Events(vec![ChaWorkEvent::ThinkingDone]),
        "tool/call_started" => {
            let id = params["itemId"]
                .as_str()
                .or_else(|| params["eventId"].as_str())
                .unwrap_or("")
                .to_string();
            let tool = params["tool"].as_str().unwrap_or("tool").to_string();
            let args = params["args"].clone();
            RuntimeProjection::Events(vec![ChaWorkEvent::ToolCall { tool, args, id }])
        }
        "tool/call_delta" => {
            let id = params["itemId"]
                .as_str()
                .or_else(|| params["eventId"].as_str())
                .unwrap_or("")
                .to_string();
            let tool = params["tool"].as_str().unwrap_or("tool").to_string();
            let content = params["content"].as_str().unwrap_or("").to_string();
            if content.is_empty() {
                RuntimeProjection::Ignored
            } else {
                RuntimeProjection::Events(vec![ChaWorkEvent::ToolDelta { id, tool, content }])
            }
        }
        "tool/call_completed" => {
            let id = params["itemId"]
                .as_str()
                .or_else(|| params["eventId"].as_str())
                .unwrap_or("")
                .to_string();
            let tool = params["tool"].as_str().unwrap_or("tool").to_string();
            let args = params["args"].clone();
            let error_payload = params.get("error").cloned();
            let result_payload = params.get("result").cloned();
            let mut events = vec![ChaWorkEvent::ToolCall {
                tool: tool.clone(),
                args,
                id: id.clone(),
            }];
            if result_payload.is_some() || error_payload.as_ref().is_some_and(|v| !v.is_null()) {
                let err_str = tool_error_message(error_payload.as_ref());
                if let Some(message) = err_str.as_ref() {
                    state.last_tool_error = Some(message.clone());
                }
                events.push(ChaWorkEvent::ToolResult {
                    id,
                    tool: Some(tool),
                    result: result_payload.unwrap_or(serde_json::Value::Null),
                    error: err_str,
                });
            }
            RuntimeProjection::Events(events)
        }
        "file_change/completed" | "file_change/updated" => {
            let events = params["changes"]
                .as_array()
                .map(|changes| {
                    changes
                        .iter()
                        .map(|change| ChaWorkEvent::FileChange {
                            path: change["path"].as_str().unwrap_or("").to_string(),
                            diff: change["diff"].as_str().unwrap_or("").to_string(),
                            action: change["action"].as_str().unwrap_or("modify").to_string(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if events.is_empty() {
                RuntimeProjection::Ignored
            } else {
                RuntimeProjection::Events(events)
            }
        }
        "file_change/delta" => {
            let id = params["itemId"]
                .as_str()
                .or_else(|| params["eventId"].as_str())
                .unwrap_or("")
                .to_string();
            let content = params["content"].as_str().unwrap_or("").to_string();
            if content.is_empty() {
                RuntimeProjection::Ignored
            } else {
                RuntimeProjection::Events(vec![ChaWorkEvent::FileChangeDelta { id, content }])
            }
        }
        "mcp/oauth_login_completed" => {
            let server_name = params["serverName"].as_str().unwrap_or("").to_string();
            let success = params["success"].as_bool().unwrap_or(false);
            let error = params["error"].as_str().map(ToString::to_string);
            RuntimeProjection::Events(vec![ChaWorkEvent::McpOauthLoginCompleted {
                server_name,
                success,
                error,
            }])
        }
        "mcp/server_status_updated" => {
            let server_name = params["serverName"].as_str().unwrap_or("").to_string();
            let status = params["status"].as_str().unwrap_or("unknown").to_string();
            let error = params["error"].as_str().map(ToString::to_string);
            RuntimeProjection::Events(vec![ChaWorkEvent::McpServerStatusUpdated {
                server_name,
                status,
                error,
            }])
        }
        "turn/plan/updated" => {
            let explanation = params["explanation"].as_str().map(ToString::to_string);
            let steps = params["steps"]
                .as_array()
                .map(|steps| {
                    steps
                        .iter()
                        .filter_map(|step| {
                            let text = step["step"].as_str()?.to_string();
                            let status = step["status"].as_str()?.to_string();
                            Some(PlanStep { step: text, status })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            RuntimeProjection::Events(vec![ChaWorkEvent::PlanUpdate { explanation, steps }])
        }
        "plan/delta" => {
            let content = params["content"].as_str().unwrap_or("").to_string();
            if content.is_empty() {
                RuntimeProjection::Ignored
            } else {
                RuntimeProjection::Events(vec![ChaWorkEvent::PlanDelta { content }])
            }
        }
        "plan/done" => {
            let content = params["content"].as_str().unwrap_or("").to_string();
            if content.is_empty() {
                RuntimeProjection::Ignored
            } else {
                RuntimeProjection::Events(vec![ChaWorkEvent::PlanDone { content }])
            }
        }
        "thread/token_usage/updated" => {
            if let Some(usage) = runtime_token_usage_from_params(params) {
                state.latest_usage = Some(usage);
            }
            RuntimeProjection::Ignored
        }
        "turn/completed" => {
            let usage =
                runtime_token_usage_from_params(params).or_else(|| state.latest_usage.take());
            RuntimeProjection::TurnCompleted { usage }
        }
        "turn/interrupted" => RuntimeProjection::TurnInterrupted,
        "turn/failed" => RuntimeProjection::TurnFailed {
            message: user_facing_turn_error_for_input(
                params["error"]["message"].as_str().unwrap_or("turn failed"),
                state.has_image_input,
            ),
        },
        "runtime/error" => RuntimeProjection::RuntimeError {
            message: user_facing_turn_error_for_input(
                params["error"]["message"]
                    .as_str()
                    .unwrap_or("runtime error"),
                state.has_image_input,
            ),
            recoverable: params["error"]["recoverable"].as_bool().unwrap_or(false),
        },
        "approval/requested" | "mcp_elicitation/requested" | "user_input/requested" => {
            RuntimeProjection::BlockingRequest
        }
        "codex/serverRequest" => RuntimeProjection::RawServerRequest,
        "codex/notification" | "runtime/audit" => {
            RuntimeProjection::Events(vec![runtime_debug_event(method, params.clone())])
        }
        "thread/started" | "turn/started" => RuntimeProjection::Ignored,
        other => RuntimeProjection::Events(vec![runtime_debug_event(other, params.clone())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_token_usage_from_update_uses_last_breakdown() {
        let usage = runtime_token_usage_from_params(&json!({
            "last": {
                "totalTokens": 11,
                "inputTokens": 8,
                "cachedInputTokens": 3,
                "outputTokens": 2,
                "reasoningOutputTokens": 1
            },
            "total": {
                "totalTokens": 31,
                "inputTokens": 20,
                "cachedInputTokens": 7,
                "outputTokens": 9,
                "reasoningOutputTokens": 2
            },
            "modelContextWindow": 128000
        }))
        .expect("usage");

        assert_eq!(usage.prompt_tokens, 8);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.total_tokens, 11);
        assert_eq!(usage.input_tokens, 8);
        assert_eq!(usage.cached_input_tokens, 3);
        assert_eq!(usage.output_tokens, 2);
        assert_eq!(usage.reasoning_output_tokens, 1);
        assert_eq!(usage.model_context_window, Some(128_000));
    }

    #[test]
    fn runtime_token_usage_accepts_turn_completed_usage_shape() {
        let usage = runtime_token_usage_from_params(&json!({
            "usage": {
                "total_tokens": 15,
                "input_tokens": 10,
                "cached_input_tokens": 4,
                "output_tokens": 3,
                "reasoning_output_tokens": 2
            }
        }))
        .expect("usage");

        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.cached_input_tokens, 4);
        assert_eq!(usage.output_tokens, 3);
        assert_eq!(usage.reasoning_output_tokens, 2);
    }

    #[test]
    fn runtime_projection_accumulates_assistant_text_and_turn_usage() {
        let mut state = RuntimeProjectionState::default();

        match project_runtime_notification(
            "assistant/delta",
            &json!({ "content": "hello " }),
            &mut state,
        ) {
            RuntimeProjection::Events(events) => {
                assert!(matches!(
                    &events[0],
                    ChaWorkEvent::AssistantDelta { content } if content == "hello "
                ));
            }
            other => panic!("expected assistant delta projection, got {other:?}"),
        }
        match project_runtime_notification(
            "assistant/done",
            &json!({ "content": "hello world" }),
            &mut state,
        ) {
            RuntimeProjection::Events(events) => {
                assert!(matches!(
                    &events[0],
                    ChaWorkEvent::AssistantDone { content } if content == "hello world"
                ));
            }
            other => panic!("expected assistant done projection, got {other:?}"),
        }
        assert_eq!(state.assistant_accum, "hello world");

        assert!(matches!(
            project_runtime_notification(
                "thread/token_usage/updated",
                &json!({
                    "last": {
                        "totalTokens": 7,
                        "inputTokens": 3,
                        "cachedInputTokens": 1,
                        "outputTokens": 4,
                        "reasoningOutputTokens": 0
                    }
                }),
                &mut state,
            ),
            RuntimeProjection::Ignored
        ));

        match project_runtime_notification("turn/completed", &json!({}), &mut state) {
            RuntimeProjection::TurnCompleted { usage: Some(usage) } => {
                assert_eq!(usage.total_tokens, 7);
                assert_eq!(usage.input_tokens, 3);
                assert_eq!(usage.output_tokens, 4);
            }
            other => panic!("expected turn completion with cached usage, got {other:?}"),
        }
    }

    #[test]
    fn runtime_projection_makes_response_stream_disconnect_user_facing() {
        let mut state = RuntimeProjectionState::default();

        match project_runtime_notification(
            "turn/failed",
            &json!({
                "error": {
                    "message": "stream disconnected before completion: stream closed before response.completed"
                }
            }),
            &mut state,
        ) {
            RuntimeProjection::TurnFailed { message } => {
                assert!(message.contains("模型流式连接中断"));
                assert!(message.contains("请重试"));
                assert!(message.contains("stream closed before response.completed"));
            }
            other => panic!("expected user-facing stream disconnect failure, got {other:?}"),
        }
    }

    #[test]
    fn image_turn_error_prompts_for_multimodal_model() {
        let message = user_facing_turn_error_for_input(
            "This model does not support input_image content.",
            true,
        );

        assert_eq!(
            message,
            "当前模型无法处理图片输入。请在设置中切换到支持图片的多模态模型后重试。"
        );
    }

    #[test]
    fn chinese_image_turn_error_prompts_for_multimodal_model() {
        let message = user_facing_turn_error_for_input("当前文本模型不支持图片输入。", true);

        assert_eq!(
            message,
            "当前模型无法处理图片输入。请在设置中切换到支持图片的多模态模型后重试。"
        );
    }

    #[test]
    fn image_stream_disconnect_prompts_for_provider_compatibility() {
        let message =
            user_facing_turn_error_for_input("stream closed before response.completed", true);

        assert!(message.contains("图片请求已发送到当前模型"));
        assert!(message.contains("Responses 图片流式接口兼容性"));
        assert!(!message.contains("多模态模型"));
        assert!(message.contains("stream closed before response.completed"));
    }

    #[test]
    fn runtime_projection_classifies_blocking_and_raw_requests() {
        let mut state = RuntimeProjectionState::default();

        assert!(matches!(
            project_runtime_notification(
                "user_input/requested",
                &json!({ "requestId": "req_1" }),
                &mut state,
            ),
            RuntimeProjection::BlockingRequest
        ));
        assert!(matches!(
            project_runtime_notification(
                "codex/serverRequest",
                &json!({ "requestId": "raw_1" }),
                &mut state,
            ),
            RuntimeProjection::RawServerRequest
        ));
    }

    #[test]
    fn runtime_projection_explicitly_routes_raw_audit_methods_to_debug_events() {
        let event = runtime_debug_event(
            "codex/notification",
            json!({
                "codexMethod": "codex/event_msg",
                "payload": { "kind": "example" },
            }),
        );

        match event {
            ChaWorkEvent::RuntimeDebug {
                method,
                category,
                params,
            } => {
                assert_eq!(method, "codex/notification");
                assert_eq!(category, "raw");
                assert_eq!(params["codexMethod"].as_str(), Some("codex/event_msg"));
            }
            other => panic!("expected runtime debug event, got {other:?}"),
        }

        let source = include_str!("projection.rs");
        assert!(source.contains("\"codex/notification\""));
        assert!(source.contains("\"codex/serverRequest\""));
        assert!(source.contains("\"runtime/audit\""));
    }
}
