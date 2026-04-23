//! 企业微信群机器人（webhook）文本 / Markdown 推送。
//!
//! 文档：<https://developer.work.weixin.qq.com/document/path/91770>

#[cfg(feature = "http")]
use serde::Serialize;

#[cfg(feature = "http")]
use crate::error::{bail, Error, Result};
#[cfg(feature = "http")]
use crate::http_common::http_client;

/// 企业微信机器人客户端（完整 webhook URL，含 `key=` 查询参数）。
#[cfg(feature = "http")]
#[derive(Debug, Clone)]
pub struct WeChatWorkRobot {
    webhook_url: String,
}

#[cfg(feature = "http")]
impl WeChatWorkRobot {
    /// `webhook_url` 必须为 `https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=...` 形式。
    pub fn new(webhook_url: impl Into<String>) -> Result<Self> {
        let webhook_url = webhook_url.into();
        let webhook_url = webhook_url.trim().to_string();
        if webhook_url.is_empty() {
            bail!("WeChat Work webhook URL must not be empty");
        }
        if !webhook_url.to_ascii_lowercase().starts_with("https://") {
            return Err(Error::validation("WeChat Work webhook URL must use HTTPS"));
        }
        Ok(Self { webhook_url })
    }

    /// 发送 `text` 类型消息（UTF-8 全文发送，由服务端长度限制校验）。
    pub fn send_text(&self, content: &str) -> Result<()> {
        let body = WeChatTextMsg {
            msgtype: "text",
            text: WeChatTextBody {
                content: content.to_string(),
            },
        };
        self.send_json(&body)
    }

    /// 发送 `markdown` 类型消息。
    pub fn send_markdown(&self, content: &str) -> Result<()> {
        let body = WeChatMarkdownMsg {
            msgtype: "markdown",
            markdown: WeChatMarkdownBody {
                content: content.to_string(),
            },
        };
        self.send_json(&body)
    }

    fn send_json<T: Serialize + ?Sized>(&self, body: &T) -> Result<()> {
        let client = http_client()?;
        let resp = client
            .post(&self.webhook_url)
            .json(body)
            .send()
            .map_err(|e| Error::http(e.to_string()))?;
        map_wechat_response(resp)
    }
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct WeChatTextMsg {
    msgtype: &'static str,
    text: WeChatTextBody,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct WeChatMarkdownMsg {
    msgtype: &'static str,
    markdown: WeChatMarkdownBody,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct WeChatTextBody {
    content: String,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct WeChatMarkdownBody {
    content: String,
}

#[cfg(feature = "http")]
fn map_wechat_response(resp: reqwest::blocking::Response) -> Result<()> {
    let status = resp.status();
    let bytes = resp.bytes().map_err(|e| Error::http(e.to_string()))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let errcode = v.get("errcode").and_then(|x| x.as_i64()).unwrap_or(0);
    if !status.is_success() {
        return Err(Error::http(format!(
            "WeChat Work HTTP {}: {}",
            status.as_u16(),
            String::from_utf8_lossy(&bytes)
        )));
    }
    if errcode != 0 {
        let msg = v
            .get("errmsg")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown error");
        return Err(Error::http(format!("WeChat Work errcode={errcode}: {msg}")));
    }
    Ok(())
}
