//! Shared message shapes for outbound notifications.

use serde::{Deserialize, Serialize};

/// High-level channel identifier (for routing / config, not wire format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    WeChatWork,
    DingTalk,
    Discord,
    WhatsApp,
}
