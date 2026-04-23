//! WhatsApp Cloud API（Meta）文本消息发送。
//!
//! 文档：<https://developers.facebook.com/docs/whatsapp/cloud-api/guides/send-messages>

#[cfg(feature = "http")]
use serde::Serialize;

#[cfg(feature = "http")]
use crate::error::{bail, Error, Result};
#[cfg(feature = "http")]
use crate::http_common::http_client;

/// Graph API 版本前缀（路径 `/v{VERSION}/`）。
pub const WHATSAPP_GRAPH_API_VERSION: &str = "v21.0";

/// WhatsApp Cloud API 客户端（永久访问令牌放在 `Authorization: Bearer`）。
#[cfg(feature = "http")]
#[derive(Debug, Clone)]
pub struct WhatsAppCloud {
    phone_number_id: String,
    access_token: String,
    graph_api_version: String,
}

#[cfg(feature = "http")]
impl WhatsAppCloud {
    pub fn new(
        phone_number_id: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Result<Self> {
        let phone_number_id = phone_number_id.into();
        let access_token = access_token.into();
        if phone_number_id.trim().is_empty() {
            bail!("WhatsApp phone_number_id must not be empty");
        }
        if access_token.trim().is_empty() {
            bail!("WhatsApp access_token must not be empty");
        }
        Ok(Self {
            phone_number_id,
            access_token,
            graph_api_version: WHATSAPP_GRAPH_API_VERSION.to_string(),
        })
    }

    /// 覆盖默认 Graph 版本（例如 `v22.0`）。
    pub fn with_graph_api_version(mut self, version: impl Into<String>) -> Self {
        self.graph_api_version = version.into();
        self
    }

    /// `to` 为 E.164；允许带或不带 `+` 前缀。
    pub fn send_text(&self, to: &str, body: &str) -> Result<()> {
        let to = to.trim().trim_start_matches('+');
        if to.is_empty() {
            bail!("WhatsApp recipient `to` must not be empty");
        }
        let url = self.messages_url();
        let payload = WaTextMessage {
            messaging_product: "whatsapp",
            to: to.to_string(),
            msg_type: "text",
            text: WaTextBody {
                body: body.to_string(),
            },
        };
        let client = http_client()?;
        let resp = client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .map_err(|e| Error::http(e.to_string()))?;
        map_whatsapp_response(resp)
    }

    fn messages_url(&self) -> String {
        let ver = self.graph_api_version.trim();
        let ver = if ver.starts_with('v') {
            ver.to_string()
        } else {
            format!("v{ver}")
        };
        format!(
            "https://graph.facebook.com/{ver}/{}/messages",
            self.phone_number_id
        )
    }
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct WaTextMessage {
    messaging_product: &'static str,
    to: String,
    #[serde(rename = "type")]
    msg_type: &'static str,
    text: WaTextBody,
}

#[cfg(feature = "http")]
#[derive(Serialize)]
struct WaTextBody {
    body: String,
}

#[cfg(feature = "http")]
fn map_whatsapp_response(resp: reqwest::blocking::Response) -> Result<()> {
    let status = resp.status();
    let bytes = resp.bytes().map_err(|e| Error::http(e.to_string()))?;
    if !status.is_success() {
        return Err(Error::http(format!(
            "WhatsApp HTTP {}: {}",
            status.as_u16(),
            String::from_utf8_lossy(&bytes)
        )));
    }
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown Graph error");
        return Err(Error::http(format!("WhatsApp Graph error: {msg}")));
    }
    Ok(())
}
