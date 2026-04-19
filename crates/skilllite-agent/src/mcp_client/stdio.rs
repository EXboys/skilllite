//! MCP stdio client: JSON-RPC 2.0 one-object-per-line over child stdin/stdout.

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::error::Error;
use crate::Result;

use crate::types::McpServerEntry;

pub(crate) struct McpStdioSession {
    inner: Mutex<McpStdioSessionInner>,
}

struct McpStdioSessionInner {
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    #[allow(dead_code)]
    child: Child,
    next_id: u64,
}

impl McpStdioSession {
    pub(crate) async fn spawn(entry: &McpServerEntry, workspace: &str) -> Result<Self> {
        let cwd = entry
            .cwd
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(workspace);

        let mut cmd = Command::new(&entry.command);
        cmd.args(&entry.args);
        cmd.current_dir(cwd);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        for (k, v) in &entry.env {
            cmd.env(k, v);
        }
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            Error::validation(format!("MCP spawn failed for '{}': {}", entry.id.trim(), e))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::validation("MCP stdin missing".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::validation("MCP stdout missing".to_string()))?;

        Ok(Self {
            inner: Mutex::new(McpStdioSessionInner {
                stdin,
                stdout: BufReader::new(stdout),
                child,
                next_id: 1,
            }),
        })
    }

    /// Full MCP handshake: initialize → notifications/initialized → optional ping.
    pub(crate) async fn handshake(&self) -> Result<()> {
        let _ = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "skilllite-agent",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
            )
            .await?;
        self.notify("notifications/initialized", json!({})).await?;
        Ok(())
    }

    async fn notify(&self, method: &str, params: Value) -> Result<()> {
        let line = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        })
        .to_string();
        let mut g = self.inner.lock().await;
        g.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(Error::Io)?;
        g.stdin.write_all(b"\n").await.map_err(Error::Io)?;
        g.stdin.flush().await.map_err(Error::Io)?;
        Ok(())
    }

    pub(crate) async fn tools_list(&self) -> Result<Value> {
        self.request("tools/list", json!({})).await
    }

    pub(crate) async fn tools_call(&self, name: &str, arguments: Value) -> Result<Value> {
        self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )
        .await
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let mut g = self.inner.lock().await;
        let id = g.next_id;
        g.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = req.to_string();
        g.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(Error::Io)?;
        g.stdin.write_all(b"\n").await.map_err(Error::Io)?;
        g.stdin.flush().await.map_err(Error::Io)?;

        loop {
            let mut buf = String::new();
            let n = g.stdout.read_line(&mut buf).await.map_err(Error::Io)?;
            if n == 0 {
                return Err(Error::validation(format!(
                    "MCP EOF waiting for response to {}",
                    method
                )));
            }
            let trimmed = buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: Value =
                serde_json::from_str(trimmed).map_err(|e| Error::validation(e.to_string()))?;
            if msg.get("method").is_some() && msg.get("id").is_none() {
                continue;
            }
            let rid = msg.get("id");
            if rid.is_none() {
                continue;
            }
            let matches = match rid {
                Some(Value::Number(n)) => n.as_u64() == Some(id),
                Some(Value::String(s)) => *s == id.to_string(),
                _ => false,
            };
            if !matches {
                continue;
            }
            if let Some(err) = msg.get("error") {
                return Err(Error::validation(format!("MCP error: {}", err)));
            }
            return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}
