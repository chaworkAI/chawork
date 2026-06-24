use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::VecDeque;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

pub(super) async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &Value,
) -> Result<()> {
    let encoded = serde_json::to_string(message)?;
    writer.write_all(encoded.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

/// Read the next non-empty line, or `None` at EOF.
pub(super) async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<String>> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return Ok(Some(trimmed.to_string()));
    }
}

#[derive(Debug)]
pub(super) enum RuntimeMessage {
    Response(Value),
    Notification { method: String, params: Value },
    Eof,
}

pub struct RuntimeConnection {
    pub(super) child: Child,
    pub(super) stdin: ChildStdin,
    pub(super) events_rx: mpsc::Receiver<RuntimeMessage>,
    pending_notifications: VecDeque<(String, Value)>,
    next_request_id: i64,
    pub(super) initialized: bool,
    pub(super) active_thread_id: Option<String>,
    pub(super) active_turn_id: Option<String>,
}

impl RuntimeConnection {
    pub(super) fn new(
        child: Child,
        stdin: ChildStdin,
        events_rx: mpsc::Receiver<RuntimeMessage>,
    ) -> Self {
        Self {
            child,
            stdin,
            events_rx,
            pending_notifications: VecDeque::new(),
            next_request_id: 0,
            initialized: false,
            active_thread_id: None,
            active_turn_id: None,
        }
    }

    pub(super) fn next_request_id(&mut self) -> i64 {
        self.next_request_id += 1;
        self.next_request_id
    }

    pub(super) async fn send_rpc(&mut self, method: &str, params: Option<Value>) -> Result<i64> {
        let id = self.next_request_id();
        let mut message = json!({
            "id": id,
            "method": method,
        });
        if let Some(params) = params {
            message["params"] = params;
        }
        write_message(&mut self.stdin, &message).await?;
        Ok(id)
    }

    async fn wait_response(&mut self, id: i64) -> Result<Value> {
        while let Some(message) = self.events_rx.recv().await {
            match message {
                RuntimeMessage::Response(value) => {
                    if value.get("id").and_then(Value::as_i64) == Some(id) {
                        if let Some(err) = value.get("error") {
                            bail!("runtime error for request {id}: {err}");
                        }
                        return Ok(value);
                    }
                }
                RuntimeMessage::Notification { method, params } => {
                    self.pending_notifications.push_back((method, params));
                }
                RuntimeMessage::Eof => break,
            }
        }
        bail!("runtime closed before responding to request {id}")
    }

    pub(super) async fn send_rpc_wait(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value> {
        let id = self.send_rpc(method, params).await?;
        self.wait_response(id).await
    }

    pub(super) fn take_pending_notifications(&mut self) -> Vec<(String, Value)> {
        self.pending_notifications.drain(..).collect()
    }
}

pub(super) fn route_runtime_line(line: &str) -> Option<RuntimeMessage> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("id").is_some() {
        return Some(RuntimeMessage::Response(value));
    }
    let method = value.get("method").and_then(Value::as_str)?.to_string();
    Some(RuntimeMessage::Notification {
        method,
        params: value.get("params").cloned().unwrap_or(Value::Null),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tokio::process::Command;

    #[test]
    fn route_runtime_line_splits_responses_and_notifications() {
        let response =
            route_runtime_line(r#"{"id":7,"result":{"ok":true}}"#).expect("response routed");
        match response {
            RuntimeMessage::Response(value) => {
                assert_eq!(value["id"].as_i64(), Some(7));
            }
            other => panic!("expected response, got {other:?}"),
        }

        let notification =
            route_runtime_line(r#"{"method":"turn/completed","params":{"turnId":"t1"}}"#)
                .expect("notification routed");
        match notification {
            RuntimeMessage::Notification { method, params } => {
                assert_eq!(method, "turn/completed");
                assert_eq!(params["turnId"].as_str(), Some("t1"));
            }
            other => panic!("expected notification, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn runtime_connection_allocates_monotonic_request_ids() {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg("cat >/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().expect("spawn");
        let stdin = child.stdin.take().expect("stdin");
        let (_tx, rx) = mpsc::channel(1);
        let mut conn = RuntimeConnection::new(child, stdin, rx);
        assert_eq!(conn.next_request_id(), 1);
        assert_eq!(conn.next_request_id(), 2);
        let _ = conn.child.kill().await;
    }

    #[tokio::test]
    async fn runtime_connection_buffers_notifications_while_waiting_for_response() {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg("cat >/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().expect("spawn");
        let stdin = child.stdin.take().expect("stdin");
        let (tx, rx) = mpsc::channel(4);
        let mut conn = RuntimeConnection::new(child, stdin, rx);

        tokio::spawn(async move {
            tx.send(RuntimeMessage::Notification {
                method: "mcp/server_status_updated".to_string(),
                params: json!({"serverName":"chawork_workspace","status":"ready"}),
            })
            .await
            .expect("send notification");
            tx.send(RuntimeMessage::Response(
                json!({"id":1,"result":{"ok":true}}),
            ))
            .await
            .expect("send response");
        });

        let response = conn
            .send_rpc_wait("thread/start", Some(json!({})))
            .await
            .expect("response");
        assert_eq!(response["result"]["ok"].as_bool(), Some(true));
        let pending = conn.take_pending_notifications();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "mcp/server_status_updated");
        assert_eq!(pending[0].1["status"].as_str(), Some("ready"));
        assert!(conn.take_pending_notifications().is_empty());
        let _ = conn.child.kill().await;
    }
}
