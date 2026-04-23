//! Discord Incoming Webhook（执行 webhook）文本消息。
//!
//! 文档：<https://discord.com/developers/docs/resources/webhook#execute-webhook>

#[cfg(feature = "http")]
use serde::Serialize;

#[cfg(feature = "http")]
use crate::error::{bail, Error, Result};
#[cfg(feature = "http")]
use crate::http_common::http_client;

/// Discord webhook 客户端（`https://discord.com/api/webhooks/...` 或 `https://discordapp.com/...`）。
#[cfg(feature = "http")]
#[derive(Debug, Clone)]
pub struct DiscordWebhook {
    webhook_url: String,
}

#[cfg(feature = "http")]
impl DiscordWebhook {
    pub fn new(webhook_url: impl Into<String>) -> Result<Self> {
        let webhook_url = webhook_url.into();
        let webhook_url = webhook_url.trim().to_string();
        if webhook_url.is_empty() {
            bail!("Discord webhook URL must not be empty");
        }
        if !webhook_url.to_ascii_lowercase().starts_with("https://") {
            return Err(Error::validation("Discord webhook URL must use HTTPS"));
        }
        Ok(Self { webhook_url })
    }

    /// 发送纯文本内容（受 Discord 2000 字符等限制，由服务端校验）。
    pub fn send_content(&self, content: &str) -> Result<()> {
        let body = DiscordExecuteBody {
            content: content.to_string(),
        };
        let client = http_client()?;
        let resp = client
            .post(&self.webhook_url)
            .json(&body)
            .send()
            .map_err(|e| Error::http(e.to_string()))?;
        map_discord_response(resp)
    }
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct DiscordExecuteBody {
    content: String,
}

#[cfg(feature = "http")]
fn map_discord_response(resp: reqwest::blocking::Response) -> Result<()> {
    let status = resp.status();
    let bytes = resp.bytes().map_err(|e| Error::http(e.to_string()))?;
    if status.is_success() {
        return Ok(());
    }
    Err(Error::http(format!(
        "Discord HTTP {}: {}",
        status.as_u16(),
        String::from_utf8_lossy(&bytes)
    )))
}
