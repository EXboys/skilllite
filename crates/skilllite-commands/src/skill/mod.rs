//! Skill management commands: add, remove, list, show.
//!
//! Migrated from Python `python-sdk/skilllite/cli/add.py` and `repo.py`.
//! Depends ONLY on skill/ and env/ layers (Layer 1-2), NOT on agent/ (Layer 3).

mod add;
mod common;
mod import_openclaw;
mod list;
mod remove;
mod show;
mod verify;

pub use add::{cmd_add, update_skill_from_source};
pub(crate) use common::resolve_skills_dir;
pub use import_openclaw::cmd_import_openclaw_skills;
pub(crate) use import_openclaw::{
    collect_openclaw_import_candidates, openclaw_workspace_candidates, SkillConflictPolicy,
};
pub use list::cmd_list;
pub use remove::cmd_remove;
pub use show::cmd_show;
pub use verify::cmd_verify;
