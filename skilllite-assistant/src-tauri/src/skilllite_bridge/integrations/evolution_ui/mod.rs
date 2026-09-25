//! Evolution UI bridge — all data via `skilllite … --json` (L2).

mod authorize;
mod backlog;
mod growth;
mod pending;
mod status;
mod trigger;

pub use authorize::authorize_capability_evolution;
pub use backlog::{
    get_evolution_proposal_status, load_evolution_backlog, EvolutionBacklogRowDto,
    EvolutionProposalStatusDto,
};
pub use growth::evolution_growth_due;
pub use pending::{
    evolution_confirm_pending_skill, evolution_reject_pending_skill, list_evolution_pending_skills,
    read_evolution_pending_skill_md, PendingSkillDto,
};
pub use status::{load_evolution_status, EvolutionStatusPayload};
pub use trigger::trigger_evolution_run;
