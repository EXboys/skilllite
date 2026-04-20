//! SkillLite bridge：与 `skilllite` CLI 子进程、聊天根目录文件、会话与集成能力交互。
//!
//! 子模块划分：`protocol`（agent-rpc 行协议）、`paths`（工作区与路径校验）、`chat`（对话子进程）、
//! `transcript`（历史记录）、`sessions`（sessions.json）、`workspace`（最近文件与按路径读文件）、
//! `integrations`（按域拆分：`skill_rpc` / `prompt_artifact` / `evolution_ui` / `desktop_services`）。

mod bundled_skills_sync;
mod chat;
mod followup_suggestions;
mod integrations;
mod llm_routing_error;
mod paths;
mod protocol;
mod sessions;
mod transcript;
mod workspace;

pub use bundled_skills_sync::sync_bundled_skills_from_resources;
pub use chat::{
    chat_stream, merge_dotenv_with_chat_overrides, stop_chat, ChatConfigOverrides,
    ChatImageAttachment, ChatProcessState, ClarificationState, ClarifyResponse, ConfirmationState,
};
pub use followup_suggestions::followup_chat_suggestions;
pub use integrations::*;
pub use llm_routing_error::{
    classify_llm_routing_error_message, LlmInvokeResult, LlmRoutingError, LlmRoutingErrorKind,
};
pub(crate) use paths::load_dotenv_for_child;
pub use paths::{default_writable_workspace_dir, resolve_skilllite_path_app};
pub use sessions::*;
pub use transcript::{clear_transcript, load_transcript, TranscriptMessage};
pub use workspace::*;
