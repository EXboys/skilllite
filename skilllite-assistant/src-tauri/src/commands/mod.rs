//! Tauri `invoke` 处理器，按领域分子模块（聊天、文件、技能、会话、进化、运行时、Gateway 等）。

pub mod chat;
pub mod desktop_health;
pub mod evolution;
pub mod files_and_dirs;
pub mod gateway;
pub mod life_pulse;
pub mod sessions;
pub mod skills_workspace;
pub mod uninstall;
pub mod workspace_editor;
