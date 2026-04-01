//! 进化 Memory：从执行反馈中沉淀**事实与经历**，供检索与类比。
//!
//! 设计参考：MemGPT 层级记忆与检索、AriGraph/MemoriesDB 语义+情节图、
//! 认知架构中语义/情节记忆划分。与规则进化（应然）、技能进化（可执行）明确分工：
//! 本模块只产出**实然**（实体、关系、情节、倾向、模式），不产出「何时做什么」类规则。
//! 详见 seed/evolution_prompts/memory_knowledge_extraction.seed.md 顶部设计说明。

use std::path::Path;

use crate::Result;
use rusqlite::Connection;
use tokio::task::block_in_place;

use crate::feedback::open_evolution_db;
use crate::gatekeeper_l1_path;
use crate::gatekeeper_l3_content;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

const MEMORY_KNOWLEDGE_PROMPT: &str =
    include_str!("seed/evolution_prompts/memory_knowledge_extraction.seed.md");

const RECENT_DAYS: &str = "-7 days";
const DECISION_LIMIT: i64 = 15;
/// 已有知识摘要最大字符数，供 LLM 去重参考，避免重复抽取
const EXISTING_KNOWLEDGE_CAP: usize = 3500;

/// 单次进化中各类知识条数上限，避免单次写入过长
const MAX_ENTITIES: usize = 12;
const MAX_RELATIONS: usize = 10;
const MAX_EPISODES: usize = 8;
const MAX_PREFERENCES: usize = 8;
const MAX_PATTERNS: usize = 5;

/// 运行 memory 进化：从近期 decisions 抽取实体、关系、情节、倾向、模式，追加到 memory/evolution/knowledge.md。
/// 返回 changelog 用 (change_type, target_id)，无变更时返回空 Vec。
pub async fn evolve_memory<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    model: &str,
    _txn_id: &str,
) -> Result<Vec<(String, String)>> {
    let summary = block_in_place(|| {
        let conn = open_evolution_db(chat_root)?;
        query_decisions_for_memory(&conn)
    })?;

    if summary.is_empty() {
        tracing::debug!("Memory evolution: no recent decisions with task_description, skipping");
        return Ok(Vec::new());
    }

    let knowledge_path = chat_root
        .join("memory")
        .join("evolution")
        .join("knowledge.md");
    let existing_summary = if knowledge_path.exists() {
        let full = skilllite_fs::read_file(&knowledge_path).unwrap_or_default();
        if full.len() <= EXISTING_KNOWLEDGE_CAP {
            full
        } else {
            // 取末尾一段（最近写入的），便于去重
            full.chars()
                .skip(full.len().saturating_sub(EXISTING_KNOWLEDGE_CAP))
                .collect::<String>()
        }
    } else {
        String::new()
    };

    let prompt = MEMORY_KNOWLEDGE_PROMPT
        .replace("{{decisions_summary}}", &summary)
        .replace("{{existing_knowledge_summary}}", existing_summary.trim());
    let messages = vec![EvolutionMessage::user(&prompt)];
    let content = llm
        .complete(&messages, model, 0.3)
        .await?
        .trim()
        .to_string();

    let parsed = match parse_knowledge_response(&content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "Memory knowledge extraction parse failed: {} — raw: {:.300}",
                e,
                content
            );
            let _ = block_in_place(|| {
                let conn = open_evolution_db(chat_root)?;
                let _ = crate::log_evolution_event(
                    &conn,
                    chat_root,
                    "memory_extraction_parse_failed",
                    "",
                    &format!("{}", e),
                    "",
                );
                Ok::<_, anyhow::Error>(())
            });
            return Ok(Vec::new());
        }
    };

    let has_any = !parsed.entities.is_empty()
        || !parsed.relations.is_empty()
        || !parsed.episodes.is_empty()
        || !parsed.preferences.is_empty()
        || !parsed.patterns.is_empty();
    if parsed.skip_reason.is_some() && !has_any {
        tracing::debug!(
            "Memory evolution: LLM skipped extraction — {}",
            parsed.skip_reason.as_deref().unwrap_or("")
        );
        return Ok(Vec::new());
    }

    let entities = parsed
        .entities
        .into_iter()
        .take(MAX_ENTITIES)
        .collect::<Vec<_>>();
    let relations = parsed
        .relations
        .into_iter()
        .take(MAX_RELATIONS)
        .collect::<Vec<_>>();
    let episodes = parsed
        .episodes
        .into_iter()
        .take(MAX_EPISODES)
        .collect::<Vec<_>>();
    let preferences = parsed
        .preferences
        .into_iter()
        .take(MAX_PREFERENCES)
        .collect::<Vec<_>>();
    let patterns = parsed
        .patterns
        .into_iter()
        .take(MAX_PATTERNS)
        .collect::<Vec<_>>();
    if entities.is_empty()
        && relations.is_empty()
        && episodes.is_empty()
        && preferences.is_empty()
        && patterns.is_empty()
    {
        return Ok(Vec::new());
    }

    let entity_block: String = entities
        .iter()
        .map(|e| format!("- **{}** ({}) {}", e.name, e.entity_type, e.note))
        .collect::<Vec<_>>()
        .join("\n");
    let relation_block: String = relations
        .iter()
        .map(|r| format!("- {} → {}: {}", r.from, r.to, r.relation))
        .collect::<Vec<_>>()
        .join("\n");
    let episode_block: String = episodes
        .iter()
        .map(|e| format!("- [{}] {} → 教训：{}", e.outcome, e.summary, e.lesson))
        .collect::<Vec<_>>()
        .join("\n");
    let preference_block: String = preferences
        .iter()
        .map(|p| format!("- {}（情境：{}）", p.description, p.context))
        .collect::<Vec<_>>()
        .join("\n");
    let pattern_block: String = patterns
        .iter()
        .map(|p| format!("- {}（{}）", p.description, p.evidence))
        .collect::<Vec<_>>()
        .join("\n");

    let full_content =
        format!(
        "## {}\n\n### 实体\n{}\n\n### 关系\n{}\n\n### 情节\n{}\n\n### 倾向\n{}\n\n### 模式\n{}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M"),
        if entity_block.is_empty() { "*无*".to_string() } else { entity_block },
        if relation_block.is_empty() { "*无*".to_string() } else { relation_block },
        if episode_block.is_empty() { "*无*".to_string() } else { episode_block },
        if preference_block.is_empty() { "*无*".to_string() } else { preference_block },
        if pattern_block.is_empty() { "*无*".to_string() } else { pattern_block }
    );

    if let Err(e) = gatekeeper_l3_content(&full_content) {
        tracing::warn!("Memory evolution L3 rejected content: {}", e);
        return Ok(Vec::new());
    }

    let memory_dir = chat_root.join("memory").join("evolution");
    let knowledge_path = memory_dir.join("knowledge.md");
    if !gatekeeper_l1_path(chat_root, &knowledge_path, None) {
        tracing::warn!(
            "Memory evolution L1 path rejected: {}",
            knowledge_path.display()
        );
        return Ok(Vec::new());
    }

    skilllite_fs::create_dir_all(&memory_dir)?;
    let to_append = full_content;
    let final_content = if knowledge_path.exists() {
        let existing = skilllite_fs::read_file(&knowledge_path).unwrap_or_default();
        format!("{}\n\n---\n\n{}", existing.trim_end(), to_append.trim())
    } else {
        format!(
            "# 进化知识库（实体·关系·情节·倾向·模式）\n\n由 Memory 进化从任务执行记录中自动抽取，仅沉淀事实与经历供检索，不与规则/技能重复。\n\n---\n\n{}",
            to_append.trim()
        )
    };
    skilllite_fs::write_file(&knowledge_path, &final_content)?;

    tracing::info!(
        "Memory evolution: wrote {} entities, {} relations, {} episodes, {} preferences, {} patterns to knowledge.md",
        entities.len(),
        relations.len(),
        episodes.len(),
        preferences.len(),
        patterns.len()
    );

    Ok(vec![(
        "memory_knowledge_added".to_string(),
        "knowledge".to_string(),
    )])
}

fn query_decisions_for_memory(conn: &Connection) -> Result<String> {
    let sql = format!(
        "SELECT task_description, total_tools, failed_tools, replans, elapsed_ms, tools_detail, task_completed
         FROM decisions
         WHERE ts >= datetime('now', '{}') AND task_description IS NOT NULL
         ORDER BY ts DESC LIMIT {}",
        RECENT_DAYS, DECISION_LIMIT
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let desc: String = row.get(0)?;
            let total: i64 = row.get(1)?;
            let failed: i64 = row.get(2)?;
            let replans: i64 = row.get(3)?;
            let elapsed: i64 = row.get(4)?;
            let tools_json: Option<String> = row.get(5)?;
            let completed: bool = row.get(6)?;
            let tool_summary = tools_json
                .as_deref()
                .and_then(|s| {
                    let arr: Option<Vec<serde_json::Value>> = serde_json::from_str(s).ok()?;
                    let names: Vec<String> = arr?
                        .iter()
                        .filter_map(|v| v.get("tool").and_then(|t| t.as_str()).map(String::from))
                        .collect();
                    Some(names.join(", "))
                })
                .unwrap_or_else(|| "—".to_string());
            Ok(format!(
                "- 任务: {} | 完成: {} | 工具: {} (失败: {}) | replan: {} | 耗时: {}ms | 工具序列: {}",
                desc,
                if completed { "是" } else { "否" },
                total,
                failed,
                replans,
                elapsed,
                tool_summary
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows.join("\n"))
}

#[derive(Debug, Default, serde::Deserialize)]
struct KnowledgeResponse {
    #[serde(default)]
    entities: Vec<EntityEntry>,
    #[serde(default)]
    relations: Vec<RelationEntry>,
    #[serde(default)]
    episodes: Vec<EpisodeEntry>,
    #[serde(default)]
    preferences: Vec<PreferenceEntry>,
    #[serde(default)]
    patterns: Vec<PatternEntry>,
    #[serde(default)]
    skip_reason: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct EpisodeEntry {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    lesson: String,
}

#[derive(Debug, serde::Deserialize)]
struct PreferenceEntry {
    #[serde(default)]
    description: String,
    #[serde(default)]
    context: String,
}

#[derive(Debug, serde::Deserialize)]
struct PatternEntry {
    #[serde(default)]
    description: String,
    #[serde(default)]
    evidence: String,
}

#[derive(Debug, serde::Deserialize)]
struct EntityEntry {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
    note: String,
}

#[derive(Debug, serde::Deserialize)]
struct RelationEntry {
    from: String,
    to: String,
    relation: String,
}

fn parse_knowledge_response(content: &str) -> Result<KnowledgeResponse> {
    let cleaned = crate::strip_think_blocks(content.trim());
    let json_str = crate::prompt_learner::extract_json_block(cleaned);
    let parsed: KnowledgeResponse = serde_json::from_str(&json_str).map_err(|e| {
        crate::Error::validation(format!("memory knowledge JSON parse error: {}", e))
    })?;
    Ok(parsed)
}
