//! Evolution mode, skill action, and threshold env configuration.

use skilllite_core::config::env_keys::evolution as evo_keys;

// ─── EVO-5: Evolution mode ───────────────────────────────────────────────────

/// Which dimensions of evolution are enabled.
#[derive(Debug, Clone, PartialEq)]
pub enum EvolutionMode {
    All,
    PromptsOnly,
    MemoryOnly,
    SkillsOnly,
    Disabled,
}

impl EvolutionMode {
    pub fn from_env() -> Self {
        match std::env::var("SKILLLITE_EVOLUTION").ok().as_deref() {
            None | Some("1") | Some("true") | Some("") => Self::All,
            Some("0") | Some("false") => Self::Disabled,
            Some("prompts") => Self::PromptsOnly,
            Some("memory") => Self::MemoryOnly,
            Some("skills") => Self::SkillsOnly,
            Some(other) => {
                tracing::warn!(
                    "Unknown SKILLLITE_EVOLUTION value '{}', defaulting to all",
                    other
                );
                Self::All
            }
        }
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn prompts_enabled(&self) -> bool {
        matches!(self, Self::All | Self::PromptsOnly)
    }

    pub fn memory_enabled(&self) -> bool {
        matches!(self, Self::All | Self::MemoryOnly)
    }

    pub fn skills_enabled(&self) -> bool {
        matches!(self, Self::All | Self::SkillsOnly)
    }
}

// ─── SkillAction (used by should_evolve) ──────────────────────────────────────

/// Action type for skill evolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SkillAction {
    #[default]
    None,
    Generate,
    Refine,
}

impl SkillAction {
    /// When `true`, `skill_synth::evolve_skills` runs failure- and success-driven generation before
    /// any fallback refine. When `false` (`Refine`), only retire + `refine_weakest_skill` run.
    #[must_use]
    pub fn should_run_skill_generation_paths(self) -> bool {
        !matches!(self, Self::Refine)
    }
}

/// 进化触发阈值，均由环境变量配置，未设置时使用下列默认值。
#[derive(Debug, Clone)]
pub struct EvolutionThresholds {
    pub cooldown_hours: f64,
    pub recent_days: i64,
    pub recent_limit: i64,
    pub meaningful_min_tools: i64,
    pub meaningful_threshold_skills: i64,
    pub meaningful_threshold_memory: i64,
    pub meaningful_threshold_prompts: i64,
    pub failures_min_prompts: i64,
    pub replans_min_prompts: i64,
    pub repeated_pattern_min_count: i64,
    pub repeated_pattern_min_success_rate: f64,
}

impl Default for EvolutionThresholds {
    fn default() -> Self {
        Self {
            cooldown_hours: 1.0,
            recent_days: 7,
            recent_limit: 100,
            meaningful_min_tools: 2,
            meaningful_threshold_skills: 3,
            meaningful_threshold_memory: 3,
            meaningful_threshold_prompts: 5,
            failures_min_prompts: 2,
            replans_min_prompts: 2,
            repeated_pattern_min_count: 3,
            repeated_pattern_min_success_rate: 0.8,
        }
    }
}

/// 进化触发场景：不设或 default 时与原有默认行为完全一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionProfile {
    /// 与不设 EVO_PROFILE 时一致（当前默认阈值）
    Default,
    /// 演示/内测：冷却短、阈值低，进化更频繁
    Demo,
    /// 生产/省成本：冷却长、阈值高，进化更少
    Conservative,
}

impl EvolutionThresholds {
    /// 预设：演示场景，进化更频繁
    fn demo_preset() -> Self {
        Self {
            cooldown_hours: 0.25,
            recent_days: 3,
            recent_limit: 50,
            meaningful_min_tools: 1,
            meaningful_threshold_skills: 1,
            meaningful_threshold_memory: 1,
            meaningful_threshold_prompts: 2,
            failures_min_prompts: 1,
            replans_min_prompts: 1,
            repeated_pattern_min_count: 2,
            repeated_pattern_min_success_rate: 0.7,
        }
    }

    /// 预设：保守场景，进化更少、省成本
    fn conservative_preset() -> Self {
        Self {
            cooldown_hours: 4.0,
            recent_days: 14,
            recent_limit: 200,
            meaningful_min_tools: 2,
            meaningful_threshold_skills: 5,
            meaningful_threshold_memory: 5,
            meaningful_threshold_prompts: 8,
            failures_min_prompts: 3,
            replans_min_prompts: 3,
            repeated_pattern_min_count: 4,
            repeated_pattern_min_success_rate: 0.85,
        }
    }

    pub fn from_env() -> Self {
        let parse_i64 = |key: &str, default: i64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let parse_f64 = |key: &str, default: f64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let profile = match std::env::var(evo_keys::SKILLLITE_EVO_PROFILE)
            .ok()
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some("demo") => EvolutionProfile::Demo,
            Some("conservative") => EvolutionProfile::Conservative,
            _ => EvolutionProfile::Default,
        };
        let base = match profile {
            EvolutionProfile::Default => Self::default(),
            EvolutionProfile::Demo => Self::demo_preset(),
            EvolutionProfile::Conservative => Self::conservative_preset(),
        };
        Self {
            cooldown_hours: parse_f64(evo_keys::SKILLLITE_EVO_COOLDOWN_HOURS, base.cooldown_hours),
            recent_days: parse_i64(evo_keys::SKILLLITE_EVO_RECENT_DAYS, base.recent_days),
            recent_limit: parse_i64(evo_keys::SKILLLITE_EVO_RECENT_LIMIT, base.recent_limit),
            meaningful_min_tools: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_MIN_TOOLS,
                base.meaningful_min_tools,
            ),
            meaningful_threshold_skills: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_THRESHOLD_SKILLS,
                base.meaningful_threshold_skills,
            ),
            meaningful_threshold_memory: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_THRESHOLD_MEMORY,
                base.meaningful_threshold_memory,
            ),
            meaningful_threshold_prompts: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_THRESHOLD_PROMPTS,
                base.meaningful_threshold_prompts,
            ),
            failures_min_prompts: parse_i64(
                evo_keys::SKILLLITE_EVO_FAILURES_MIN_PROMPTS,
                base.failures_min_prompts,
            ),
            replans_min_prompts: parse_i64(
                evo_keys::SKILLLITE_EVO_REPLANS_MIN_PROMPTS,
                base.replans_min_prompts,
            ),
            repeated_pattern_min_count: parse_i64(
                evo_keys::SKILLLITE_EVO_REPEATED_PATTERN_MIN_COUNT,
                base.repeated_pattern_min_count,
            ),
            repeated_pattern_min_success_rate: parse_f64(
                evo_keys::SKILLLITE_EVO_REPEATED_PATTERN_MIN_SUCCESS_RATE,
                base.repeated_pattern_min_success_rate,
            ),
        }
    }
}
