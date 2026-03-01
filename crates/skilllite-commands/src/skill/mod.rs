//! Skill management commands: add, remove, list, show.
//!
//! Migrated from Python `python-sdk/skilllite/cli/add.py` and `repo.py`.
//! Depends ONLY on skill/ and env/ layers (Layer 1-2), NOT on agent/ (Layer 3).

mod add;
mod common;
mod list;
mod remove;
mod show;
mod verify;

pub use add::cmd_add;
pub use list::cmd_list;
pub use remove::cmd_remove;
pub use show::cmd_show;
pub use verify::cmd_verify;
