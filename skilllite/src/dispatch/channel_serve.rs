//! `skilllite channel serve` — inbound HTTP (see `crates/skilllite-commands/src/channel_serve.rs`).

use crate::cli::{ChannelAction, Commands};
use crate::command_registry::CommandRegistry;
use crate::Error;

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Channel { action } = cmd {
            match action {
                ChannelAction::Serve { bind, token } => Some(
                    skilllite_commands::channel_serve::cmd_channel_serve(bind, token.as_deref())
                        .map_err(Error::from),
                ),
            }
        } else {
            None
        }
    });
}
