//! SkillLite bridge：与 `skilllite` CLI 子进程、聊天根目录文件、会话与集成能力交互。
//!
//! 子模块划分：`protocol`（agent-rpc 行协议）、`paths`（工作区与路径校验）、`chat`（对话子进程）、
//! `transcript`（历史记录）、`sessions`（sessions.json）、`workspace`（最近文件与按路径读文件）、
//! `integrations`（技能/进化/引导/运行时/Ollama/日程）。

mod bundled_skills_sync;
mod chat;
mod integrations;
mod paths;
mod protocol;
mod sessions;
mod transcript;
mod workspace;

pub use bundled_skills_sync::sync_bundled_skills_from_resources;
pub use chat::{
    chat_stream, merge_dotenv_with_chat_overrides, stop_chat, ChatConfigOverrides,
    ChatProcessState, ClarificationState, ClarifyResponse, ConfirmationState,
};
pub use integrations::*;
pub use paths::resolve_skilllite_path_app;
pub(crate) use paths::load_dotenv_for_child;
pub use sessions::*;
pub use transcript::{clear_transcript, load_transcript, TranscriptMessage};
pub use workspace::*;
