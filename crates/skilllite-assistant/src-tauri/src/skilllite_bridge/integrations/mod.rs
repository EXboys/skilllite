//! 按协议域拆分：CLI 子进程桥、prompt 工件路径、进化 UI、引导/运行时。
//!
//! | 子模块 | 职责 |
//! |--------|------|
//! | [`skill_rpc`] | `skilllite` CLI 与技能目录发现（非 agent-rpc 行协议） |
//! | [`prompt_artifact`] | `chat_root/prompts` / `_versions` 白名单与读写；契约单测与 [`super::protocol`] 同级策略 |
//! | [`evolution_ui`] | 进化状态、待审核技能、Backlog、进程内/子进程触发 |
//! | [`desktop_services`] | 引导体检、运行时预取、Ollama、`schedule.json` |

mod desktop_services;
mod evolution_ui;
mod prompt_artifact;
mod shared;
mod skill_rpc;

pub use desktop_services::*;
pub use evolution_ui::*;
pub use prompt_artifact::*;
pub use skill_rpc::*;

pub use skilllite_sandbox::{ProvisionRuntimesResult, RuntimeUiLine, RuntimeUiSnapshot};
