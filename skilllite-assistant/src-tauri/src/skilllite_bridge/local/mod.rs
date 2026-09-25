//! Local copies of engine constants/types so the desktop crate needs no path deps on engine crates.

pub mod dotenv;
pub mod engine_types;
pub mod env_keys;
pub mod mcp;
pub mod paths;
pub mod schedule;
pub mod skill_discovery;

pub use dotenv::{parse_dotenv_from_dir, parse_dotenv_walking_up};
pub use paths::{chat_root, data_root};
pub use schedule::{
    list_due_job_indices, load_schedule, load_state, prepare_state_for_today, save_schedule,
    ScheduleFile,
};
pub use skill_discovery::{
    discover_skill_instances_in_workspace, resolve_skills_dir_with_legacy_fallback, SkillInstance,
    SKILL_SEARCH_DIRS,
};
