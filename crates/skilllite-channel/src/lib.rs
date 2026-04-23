//! 出站消息渠道：企业微信、钉钉、Discord Webhook、WhatsApp Cloud API。
//!
//! 默认启用 `http` feature（基于 `reqwest::blocking`）。上层（CLI、Desktop、Agent 扩展）
//! 可依赖本 crate 做告警推送，而不反向依赖 `skilllite-agent`。
//!
//! ## 平台能力（当前实现）
//!
//! | 平台 | 模块 | 说明 |
//! |------|------|------|
//! | 企业微信群机器人 | [`wechat_work::WeChatWorkRobot`] | `text` / `markdown` |
//! | 钉钉自定义机器人 | [`dingtalk::DingTalkRobot`] | `text` / `markdown`，可选加签 |
//! | Discord Incoming Webhook | [`discord::DiscordWebhook`] | 纯文本 `content` |
//! | WhatsApp Cloud API | [`whatsapp::WhatsAppCloud`] | Graph `messages` 文本 |
//!
//! 凭证与 webhook URL 由调用方配置并传入；本 crate 不读取环境变量（避免与 `skilllite-core` 配置层耦合）。

pub mod error;
pub mod types;

#[cfg(feature = "http")]
mod http_common;

#[cfg(feature = "http")]
pub mod dingtalk;
#[cfg(feature = "http")]
pub mod discord;
#[cfg(feature = "http")]
pub mod wechat_work;
#[cfg(feature = "http")]
pub mod whatsapp;

pub use error::{Error, Result};
pub use types::ChannelKind;

#[cfg(feature = "http")]
pub use dingtalk::DingTalkRobot;
#[cfg(feature = "http")]
pub use discord::DiscordWebhook;
#[cfg(feature = "http")]
pub use wechat_work::WeChatWorkRobot;
#[cfg(feature = "http")]
pub use whatsapp::{WhatsAppCloud, WHATSAPP_GRAPH_API_VERSION};

#[cfg(all(test, feature = "http"))]
mod tests {
    use super::dingtalk::dingtalk_sign;
    use super::whatsapp::WhatsAppCloud;

    #[test]
    fn dingtalk_sign_matches_known_vector() {
        let secret = "SEC1";
        let ts: u128 = 1_600_000_000_000;
        let sign = dingtalk_sign(secret, ts).expect("sign");
        assert!(!sign.is_empty());
        let again = dingtalk_sign(secret, ts).expect("sign");
        assert_eq!(sign, again, "HMAC-SHA256 signature must be stable");
    }

    #[test]
    fn dingtalk_sign_rejects_empty_hmac_key() {
        let r = dingtalk_sign("", 1);
        assert!(r.is_err(), "empty secret must not build HMAC");
    }

    #[test]
    fn whatsapp_send_text_rejects_empty_or_blank_recipient中文() {
        let c = WhatsAppCloud::new("123", "tok")
            .expect("client")
            .with_graph_api_version("v22.0");
        let e = c.send_text("", "hi").expect_err("empty to");
        assert!(e.to_string().contains("must not be empty"), "{e}");
        let c2 = WhatsAppCloud::new("pid", "token").expect("client");
        let e2 = c2.send_text("   ", "body").expect_err("blank to");
        let s = e2.to_string();
        assert!(
            s.contains("must not be empty") || s.contains("empty"),
            "unexpected error: {s}"
        );
    }

    #[test]
    fn wechat_work_rejects_http_url() {
        let r =
            super::WeChatWorkRobot::new("http://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=x");
        assert!(r.is_err());
    }

    #[test]
    fn discord_rejects_http_url() {
        let r = super::DiscordWebhook::new("http://discord.com/api/webhooks/x/y");
        assert!(r.is_err());
    }
}
