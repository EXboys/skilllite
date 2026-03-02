//! 命令注册表：通过 `register_command` 注册新命令，减少 lib.rs 的 match 修改。
//!
//! 新增命令时：
//! 1. 在 cli.rs 的 Commands 枚举中添加变体
//! 2. 在对应 dispatch 模块中调用 `reg.register(...)` 注册处理逻辑
//! 3. lib.rs 无需修改

use anyhow::Result;
use std::sync::Arc;

use crate::cli::Commands;

/// 命令处理器：接收解析后的 Commands，若匹配则返回 Some(result)，否则返回 None
pub type CommandHandler = Arc<dyn Fn(&Commands) -> Option<Result<()>> + Send + Sync>;

/// 命令注册表：按注册顺序尝试分发，第一个返回 Some 的处理器负责执行
pub struct CommandRegistry {
    handlers: Vec<CommandHandler>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// 注册命令处理器。处理器应只处理其关心的 Commands 变体，不匹配时返回 None
    pub fn register<F>(&mut self, f: F)
    where
        F: Fn(&Commands) -> Option<Result<()>> + Send + Sync + 'static,
    {
        self.handlers.push(Arc::new(f));
    }

    /// 分发命令：依次调用已注册的处理器，返回第一个匹配的结果
    pub fn dispatch(&self, cmd: &Commands) -> Result<()> {
        for h in &self.handlers {
            if let Some(r) = h(cmd) {
                return r;
            }
        }
        unreachable!("every Commands variant must be handled by a registered handler")
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
