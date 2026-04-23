//! 钉钉自定义机器人（webhook + 可选加签）。
//!
//! 文档：<https://open.dingtalk.com/document/orgapp-server/customize-robot-security-settings>

#[cfg(feature = "http")]
use base64::{engine::general_purpose::STANDARD, Engine};
#[cfg(feature = "http")]
use hmac::{Hmac, Mac};
#[cfg(feature = "http")]
use reqwest::Url;
#[cfg(feature = "http")]
use serde::Serialize;
#[cfg(feature = "http")]
use sha2::Sha256;

#[cfg(feature = "http")]
use crate::error::{bail, Error, Result};
#[cfg(feature = "http")]
use crate::http_common::http_client;

type HmacSha256 = Hmac<Sha256>;

/// 钉钉机器人客户端。
#[cfg(feature = "http")]
#[derive(Debug, Clone)]
pub struct DingTalkRobot {
    /// 基础 webhook URL（含 `access_token=`，不含加签参数）。
    webhook_url: String,
    secret: Option<String>,
}

#[cfg(feature = "http")]
impl DingTalkRobot {
    /// `webhook_url` 示例：`https://oapi.dingtalk.com/robot/send?access_token=...`
    pub fn new(webhook_url: impl Into<String>, secret: Option<String>) -> Result<Self> {
        let webhook_url = webhook_url.into();
        let webhook_url = webhook_url.trim().to_string();
        if webhook_url.is_empty() {
            bail!("DingTalk webhook URL must not be empty");
        }
        if !webhook_url.to_ascii_lowercase().starts_with("https://") {
            return Err(Error::validation("DingTalk webhook URL must use HTTPS"));
        }
        Ok(Self {
            webhook_url,
            secret,
        })
    }

    /// 发送 Markdown（`markdown` 类型，`title` + `text`）。
    pub fn send_markdown(&self, title: &str, text: &str) -> Result<()> {
        let body = DingTalkMsg {
            msgtype: "markdown",
            markdown: DingMarkdown {
                title: title.to_string(),
                text: text.to_string(),
            },
        };
        self.send_json(&body)
    }

    /// 发送纯文本（`text` 类型，`content` 可含 `\n`）。
    pub fn send_text(&self, content: &str) -> Result<()> {
        let body = DingTalkTextMsg {
            msgtype: "text",
            text: DingText {
                content: content.to_string(),
            },
        };
        self.send_json(&body)
    }

    fn send_json<T: Serialize + ?Sized>(&self, body: &T) -> Result<()> {
        let url = self.signed_url()?;
        let client = http_client()?;
        let resp = client
            .post(url)
            .json(body)
            .header("Content-Type", "application/json; charset=utf-8")
            .send()
            .map_err(|e| Error::http(e.to_string()))?;
        map_dingtalk_response(resp)
    }

    fn signed_url(&self) -> Result<Url> {
        let mut url = Url::parse(&self.webhook_url)
            .map_err(|e| Error::validation(format!("invalid DingTalk webhook URL: {e}")))?;
        if let Some(secret) = &self.secret {
            let secret = secret.trim();
            if secret.is_empty() {
                bail!("DingTalk secret is set but empty");
            }
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| Error::validation(format!("system clock: {e}")))?
                .as_millis()
                .to_string();
            let string_to_sign = format!("{ts}\n{secret}");
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| {
                Error::validation(format!("invalid DingTalk secret (HMAC key): {e}"))
            })?;
            mac.update(string_to_sign.as_bytes());
            let sign = STANDARD.encode(mac.finalize().into_bytes());
            url.query_pairs_mut()
                .append_pair("timestamp", &ts)
                .append_pair("sign", &sign);
        }
        Ok(url)
    }
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct DingTalkMsg {
    msgtype: &'static str,
    markdown: DingMarkdown,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct DingMarkdown {
    title: String,
    text: String,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct DingTalkTextMsg {
    msgtype: &'static str,
    text: DingText,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct DingText {
    content: String,
}

#[cfg(feature = "http")]
fn map_dingtalk_response(resp: reqwest::blocking::Response) -> Result<()> {
    let status = resp.status();
    let bytes = resp.bytes().map_err(|e| Error::http(e.to_string()))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let errcode = v.get("errcode").and_then(|x| x.as_i64()).unwrap_or(0);
    if !status.is_success() {
        return Err(Error::http(format!(
            "DingTalk HTTP {}: {}",
            status.as_u16(),
            String::from_utf8_lossy(&bytes)
        )));
    }
    if errcode != 0 {
        let msg = v
            .get("errmsg")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown error");
        return Err(Error::http(format!("DingTalk errcode={errcode}: {msg}")));
    }
    Ok(())
}

/// 计算钉钉加签 query 参数（便于单测与外部拼装 URL）。
#[cfg(feature = "http")]
pub fn dingtalk_sign(secret: &str, timestamp_ms: u128) -> Result<String> {
    if secret.is_empty() {
        return Err(Error::validation("DingTalk secret must not be empty"));
    }
    let string_to_sign = format!("{timestamp_ms}\n{secret}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| Error::validation(format!("invalid DingTalk secret (HMAC key): {e}")))?;
    mac.update(string_to_sign.as_bytes());
    Ok(STANDARD.encode(mac.finalize().into_bytes()))
}
