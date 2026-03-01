//! EVO-6: External knowledge learning.
//!
//! Fetches tech articles from external sources (CN-priority), extracts
//! planning rules via LLM, and evolves the source registry itself
//! (pause/retire low-quality sources, update accessibility scores).
//!
//! Gated by env var: `SKILLLITE_EXTERNAL_LEARNING=1` (default OFF).
//! Daily cap: max 3 external fetch runs per day.
//! Network: CN sources use 5s timeout, global sources use 15s timeout.

use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use super::super::llm::LlmClient;
use super::super::types::{ChatMessage, PlanningRule, SourceEntry, SourceRegistry};
use super::{atomic_write, gatekeeper_l3_content, log_evolution_event};

// ─── Configuration constants ─────────────────────────────────────────────────

const EMA_ALPHA: f32 = 0.3;
const CN_TIMEOUT_SECS: u64 = 5;
const GLOBAL_TIMEOUT_SECS: u64 = 15;
/// Max sources to fetch per run (keep total time bounded).
const MAX_FETCHES_PER_RUN: usize = 3;
/// Max external learning runs per calendar day.
const MAX_RUNS_PER_DAY: i64 = 3;
/// Accessibility threshold below which a source is paused.
const PAUSE_ACCESSIBILITY_THRESHOLD: f32 = 0.15;
/// Minimum fail count before pausing (avoid reacting to transient errors).
const PAUSE_MIN_FAIL_COUNT: u32 = 7;
/// Quality threshold below which a mutable source may be retired.
const RETIRE_QUALITY_THRESHOLD: f32 = 0.20;
/// Minimum fetch attempts before retirement eligibility.
const RETIRE_MIN_FETCHES: u32 = 30;

const EXTERNAL_KNOWLEDGE_PROMPT: &str =
    include_str!("../seed/evolution_prompts/external_knowledge.seed.md");

// ─── Guard: should we run? ────────────────────────────────────────────────────

/// Check whether external learning is enabled and under the daily cap.
pub fn should_run_external_learning(conn: &Connection) -> bool {
    // Env guard (opt-in)
    let enabled = std::env::var("SKILLLITE_EXTERNAL_LEARNING")
        .ok()
        .as_deref()
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if !enabled {
        return false;
    }

    // Daily cap
    let runs_today: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log
             WHERE type = 'external_fetch_run' AND date(ts) = date('now')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if runs_today >= MAX_RUNS_PER_DAY {
        tracing::debug!(
            "External learning daily cap reached ({}/{})",
            runs_today,
            MAX_RUNS_PER_DAY
        );
        return false;
    }

    true
}

// ─── Source prioritization ────────────────────────────────────────────────────

/// Sort sources: CN region first, then by accessibility_score × quality_score descending.
fn prioritize_sources(sources: &[SourceEntry]) -> Vec<&SourceEntry> {
    let mut enabled: Vec<&SourceEntry> = sources
        .iter()
        .filter(|s| s.enabled)
        .collect();

    enabled.sort_by(|a, b| {
        // CN sources always before global
        let region_ord = match (a.region.as_str(), b.region.as_str()) {
            ("cn", "cn") | ("global", "global") => std::cmp::Ordering::Equal,
            ("cn", _) => std::cmp::Ordering::Less,
            (_, "cn") => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        };
        if region_ord != std::cmp::Ordering::Equal {
            return region_ord;
        }
        // Within same region: sort by composite score descending
        let score_a = a.accessibility_score * a.quality_score;
        let score_b = b.accessibility_score * b.quality_score;
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    enabled
}

// ─── EMA accessibility update ─────────────────────────────────────────────────

/// Update accessibility score with EMA: new = α×result + (1-α)×old
fn update_accessibility(source: &mut SourceEntry, success: bool) {
    let result = if success { 1.0_f32 } else { 0.0_f32 };
    source.accessibility_score = EMA_ALPHA * result + (1.0 - EMA_ALPHA) * source.accessibility_score;
    if success {
        source.fetch_success_count += 1;
    } else {
        source.fetch_fail_count += 1;
    }
    source.last_fetched = Some(chrono::Utc::now().to_rfc3339());
}

// ─── HTTP fetch ───────────────────────────────────────────────────────────────

/// Fetch raw content from a source. Returns Ok(raw_bytes) or Err.
async fn fetch_source(source: &SourceEntry) -> Result<String> {
    let timeout_secs = if source.region == "cn" {
        CN_TIMEOUT_SECS
    } else {
        GLOBAL_TIMEOUT_SECS
    };
    let timeout = std::time::Duration::from_secs(timeout_secs);

    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent("SkillLite/1.0 (external-learning)")
        .build()?;

    // Special handling for sources that require POST
    let response = if source.parser == "juejin" {
        let body = serde_json::json!({
            "id_type": 2,
            "client_type": 2608,
            "cursor": "0",
            "limit": 20
        });
        client.post(&source.url).json(&body).send().await?
    } else {
        client.get(&source.url).send().await?
    };

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} from {}", response.status(), source.url);
    }

    Ok(response.text().await?)
}

// ─── Content parsers ──────────────────────────────────────────────────────────

/// Parse raw content into a list of (title, snippet) pairs.
fn parse_content(source: &SourceEntry, raw: &str) -> Vec<(String, String)> {
    match source.parser.as_str() {
        "juejin" => parse_juejin_json(raw),
        "infoq_cn" => parse_infoq_json(raw),
        "hn_algolia" => parse_hn_algolia_json(raw),
        "rss_generic" => parse_rss(raw),
        "github_trending_html" => parse_github_trending(raw),
        _ => parse_rss(raw), // fallback
    }
}

fn parse_juejin_json(raw: &str) -> Vec<(String, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let items = v["data"].as_array().cloned().unwrap_or_default();
    items.iter().take(10).filter_map(|item| {
        let title = item["article_info"]["title"].as_str()?.to_string();
        let brief = item["article_info"]["brief_content"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect::<String>();
        Some((title, brief))
    }).collect()
}

fn parse_infoq_json(raw: &str) -> Vec<(String, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let items = v["data"].as_array().cloned().unwrap_or_default();
    items.iter().take(10).filter_map(|item| {
        let title = item["article"]["title"].as_str()?.to_string();
        let summary = item["article"]["summary"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect::<String>();
        Some((title, summary))
    }).collect()
}

fn parse_rss(raw: &str) -> Vec<(String, String)> {
    // Minimal RSS parser: extract <title> and <description> from <item> blocks.
    let mut results = Vec::new();
    let items: Vec<&str> = raw.split("<item>").skip(1).collect();
    for item in items.iter().take(10) {
        let title = extract_xml_tag(item, "title").unwrap_or_default();
        let desc = extract_xml_tag(item, "description").unwrap_or_default();
        // Strip basic HTML tags from description
        let desc_clean = strip_html_basic(&desc).chars().take(120).collect::<String>();
        if !title.is_empty() {
            results.push((title, desc_clean));
        }
    }
    results
}

fn parse_github_trending(raw: &str) -> Vec<(String, String)> {
    // Extract repo names from GitHub trending HTML: look for h2 class="h3 lh-condensed"
    let mut results = Vec::new();
    let mut search = raw;
    while let Some(start) = search.find("h2 class=\"h3 lh-condensed\"") {
        search = &search[start + 26..];
        if let Some(link_start) = search.find("<a href=\"/") {
            let after = &search[link_start + 9..];
            if let Some(end) = after.find('"') {
                let repo_path = after[..end].to_string();
                // The description is in a <p> tag nearby
                let desc = if let Some(p_start) = search.find("<p ") {
                    let p_content = &search[p_start..];
                    if let Some(close) = p_content.find("</p>") {
                        let inner = &p_content[..close];
                        strip_html_basic(inner).trim().chars().take(100).collect()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                results.push((repo_path, desc));
                if results.len() >= 10 {
                    break;
                }
            }
        }
    }
    results
}

fn extract_xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start = text.find(&open)?;
    let content_start = text[start..].find('>')? + start + 1;
    let end = text[content_start..].find(&close)? + content_start;
    let raw = &text[content_start..end];
    // Unescape common XML/HTML entities
    let unescaped = raw
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("<![CDATA[", "")
        .replace("]]>", "");
    Some(unescaped.trim().to_string())
}

fn strip_html_basic(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

// ─── LLM rule extraction ──────────────────────────────────────────────────────

/// Extract planning rules from article content using LLM.
async fn extract_rules_from_content(
    articles: &[(String, String)],
    domains: &[String],
    existing_summary: &str,
    api_base: &str,
    api_key: &str,
    model: &str,
) -> Result<Vec<PlanningRule>> {
    if articles.is_empty() {
        return Ok(Vec::new());
    }

    // Build article content block (titles + snippets)
    let article_content = articles
        .iter()
        .enumerate()
        .map(|(i, (title, snippet))| {
            if snippet.is_empty() {
                format!("{}. {}", i + 1, title)
            } else {
                format!("{}. {}\n   {}", i + 1, title, snippet)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let domains_str = domains.join(", ");

    let prompt = EXTERNAL_KNOWLEDGE_PROMPT
        .replace("{{domains}}", &domains_str)
        .replace("{{article_content}}", &article_content)
        .replace("{{existing_rules_summary}}", existing_summary);

    let llm = LlmClient::new(api_base, api_key);
    let messages = vec![ChatMessage::user(&prompt)];
    let response = llm.chat_completion(model, &messages, None, Some(0.3)).await?;

    let content = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Ok(Vec::new());
    }

    parse_external_rule_response(&content)
}

fn parse_external_rule_response(content: &str) -> Result<Vec<PlanningRule>> {
    // Strip markdown code fences if present
    let json_str = extract_json_array(content);

    let arr: Vec<serde_json::Value> = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse external rule JSON: {}: raw={:.200}", e, content))?;

    let mut rules = Vec::new();
    for val in arr {
        let id = val["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() || !id.starts_with("ext_") {
            tracing::warn!("External rule rejected: id '{}' must start with 'ext_'", id);
            continue;
        }
        let instruction = val["instruction"].as_str().unwrap_or("").to_string();
        if instruction.is_empty() || instruction.len() > 200 {
            continue;
        }
        // L3 content safety check
        if let Err(e) = gatekeeper_l3_content(&instruction) {
            tracing::warn!("L3 rejected external rule {}: {}", id, e);
            continue;
        }
        let priority = val["priority"].as_u64().unwrap_or(50).clamp(45, 55) as u32;
        let keywords: Vec<String> = val["keywords"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let context_keywords: Vec<String> = val["context_keywords"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let tool_hint = val["tool_hint"]
            .as_str()
            .filter(|s| !s.is_empty() && *s != "null")
            .map(String::from);

        rules.push(PlanningRule {
            id,
            priority,
            keywords,
            context_keywords,
            tool_hint,
            instruction,
            mutable: true,
            origin: "external".to_string(),
            reusable: false,
            effectiveness: None,
            trigger_count: None,
        });
    }

    Ok(rules)
}

fn extract_json_array(content: &str) -> String {
    // Strip ```json fences or ``` fences
    let stripped = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    // Find first '[' and last ']'
    if let (Some(start), Some(end)) = (stripped.find('['), stripped.rfind(']')) {
        stripped[start..=end].to_string()
    } else {
        stripped.to_string()
    }
}

// ─── Source registry evolution ────────────────────────────────────────────────

/// Apply pause/retire logic to sources based on accessibility + quality scores.
fn evolve_sources(sources: &mut Vec<SourceEntry>) -> Vec<(String, String)> {
    let mut changes = Vec::new();
    for source in sources.iter_mut() {
        // Only pause/retire mutable sources (seed sources can't be retired, only paused)
        let total_fetches = source.fetch_success_count + source.fetch_fail_count;

        // Pause: accessibility too low and fail count high enough
        if source.enabled
            && source.accessibility_score < PAUSE_ACCESSIBILITY_THRESHOLD
            && source.fetch_fail_count >= PAUSE_MIN_FAIL_COUNT
        {
            source.enabled = false;
            tracing::info!(
                "Pausing source {} (accessibility={:.2}, fails={})",
                source.id, source.accessibility_score, source.fetch_fail_count
            );
            changes.push(("source_paused".to_string(), source.id.clone()));
        }

        // Retire (mutable only): quality too low, no rules contributed, many fetches
        if source.mutable
            && source.quality_score < RETIRE_QUALITY_THRESHOLD
            && source.rules_contributed == 0
            && total_fetches >= RETIRE_MIN_FETCHES
        {
            source.enabled = false;
            tracing::info!("Retiring source {} (quality={:.2})", source.id, source.quality_score);
            changes.push(("source_retired".to_string(), source.id.clone()));
        }
    }
    changes
}

// ─── Persistence helpers ──────────────────────────────────────────────────────

/// Save source registry atomically to prompts/sources.json.
fn save_sources(chat_root: &Path, registry: &SourceRegistry) -> Result<()> {
    let path = chat_root.join("prompts").join("sources.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(registry)?;
    atomic_write(&path, &json)?;
    Ok(())
}

/// Merge new external rules into existing rules.json, skipping duplicates.
fn merge_external_rules(chat_root: &Path, new_rules: Vec<PlanningRule>) -> Result<Vec<(String, String)>> {
    if new_rules.is_empty() {
        return Ok(Vec::new());
    }

    let rules_path = chat_root.join("prompts").join("rules.json");
    let mut existing: Vec<PlanningRule> = if rules_path.exists() {
        std::fs::read_to_string(&rules_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut changes = Vec::new();
    // External rules share the 50-rule cap with internal evolved rules
    let available_slots = 50_usize.saturating_sub(existing.len());
    for rule in new_rules.into_iter().take(available_slots) {
        if existing.iter().any(|r| r.id == rule.id) {
            continue;
        }
        changes.push(("external_rule_added".to_string(), rule.id.clone()));
        existing.push(rule);
    }

    if !changes.is_empty() {
        let json = serde_json::to_string_pretty(&existing)?;
        atomic_write(&rules_path, &json)?;
    }

    Ok(changes)
}

// ─── Priority promotion ────────────────────────────────────────────────────────

/// Promote external rules with effectiveness ≥ 0.7 to priority 65.
/// Called from feedback.rs integration point (see promote_external_rules).
pub fn apply_external_rule_promotions(
    chat_root: &Path,
    promotions: &[String], // rule IDs to promote
) -> Result<Vec<(String, String)>> {
    if promotions.is_empty() {
        return Ok(Vec::new());
    }
    let rules_path = chat_root.join("prompts").join("rules.json");
    if !rules_path.exists() {
        return Ok(Vec::new());
    }
    let mut rules: Vec<PlanningRule> = serde_json::from_str(
        &std::fs::read_to_string(&rules_path)?
    )?;
    let mut changes = Vec::new();
    for rule in rules.iter_mut() {
        if promotions.contains(&rule.id) && rule.origin == "external" && rule.priority < 65 {
            rule.priority = 65;
            changes.push(("external_rule_promoted".to_string(), rule.id.clone()));
        }
    }
    if !changes.is_empty() {
        let json = serde_json::to_string_pretty(&rules)?;
        atomic_write(&rules_path, &json)?;
    }
    Ok(changes)
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Run external learning cycle. Returns (change_type, id) pairs for the changelog.
///
/// Gated by `SKILLLITE_EXTERNAL_LEARNING=1`. If not enabled, returns Ok(empty).
/// Opens its own SQLite connection so the future is `Send`.
pub async fn run_external_learning(
    chat_root: &Path,
    api_base: &str,
    api_key: &str,
    model: &str,
    txn_id: &str,
) -> Result<Vec<(String, String)>> {
    // Phase 1: sync DB check (drop before any await)
    let should_run = {
        let conn = super::feedback::open_evolution_db(chat_root)?;
        let run = should_run_external_learning(&conn);
        run // conn dropped here
    };
    if !should_run {
        return Ok(Vec::new());
    }

    tracing::info!("EVO-6: Starting external learning run (txn={})", txn_id);

    // Load sources and existing rules (sync, no await)
    let mut registry = super::seed::load_sources(chat_root);
    let existing_rules = super::seed::load_rules(chat_root);
    let existing_summary = existing_rules
        .iter()
        .map(|r| format!("- {}: {}", r.id, r.instruction))
        .collect::<Vec<_>>()
        .join("\n");

    let prioritized = prioritize_sources(&registry.sources);
    let to_fetch: Vec<SourceEntry> = prioritized
        .into_iter()
        .take(MAX_FETCHES_PER_RUN)
        .cloned()
        .collect();

    let mut all_changes: Vec<(String, String)> = Vec::new();
    let mut source_update_map: Vec<(String, bool, u32)> = Vec::new(); // (id, success, rules_added)

    // Phase 2: async fetch + LLM calls (no Connection held)
    for source in &to_fetch {
        tracing::debug!("EVO-6: Fetching source {} ({})", source.id, source.url);

        let fetch_result = fetch_source(source).await;
        let (success, raw) = match fetch_result {
            Ok(content) if !content.is_empty() => (true, content),
            Ok(_) => {
                tracing::warn!("EVO-6: Empty response from {}", source.id);
                (false, String::new())
            }
            Err(e) => {
                tracing::warn!("EVO-6: Fetch failed for {}: {}", source.id, e);
                (false, String::new())
            }
        };

        if !success || raw.is_empty() {
            source_update_map.push((source.id.clone(), false, 0));
            continue;
        }

        // Parse content
        let articles = parse_content(source, &raw);
        if articles.is_empty() {
            tracing::debug!("EVO-6: No articles parsed from {}", source.id);
            source_update_map.push((source.id.clone(), true, 0));
            continue;
        }

        // LLM rule extraction
        let new_rules = match extract_rules_from_content(
            &articles,
            &source.domains,
            &existing_summary,
            api_base,
            api_key,
            model,
        ).await {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("EVO-6: Rule extraction failed for {}: {}", source.id, e);
                Vec::new()
            }
        };

        tracing::info!(
            "EVO-6: Source {} → {} articles → {} candidate rules",
            source.id, articles.len(), new_rules.len()
        );

        // Merge rules into rules.json
        let rule_changes = merge_external_rules(chat_root, new_rules)?;
        let rules_added = rule_changes.len() as u32;
        all_changes.extend(rule_changes);
        source_update_map.push((source.id.clone(), true, rules_added));
    }

    // Phase 3: update registry and apply source evolution (sync)
    for (id, success, rules_added) in &source_update_map {
        if let Some(src) = registry.sources.iter_mut().find(|s| s.id == *id) {
            update_accessibility(src, *success);
            src.rules_contributed += rules_added;
        }
    }

    // Check promote external rules whose effectiveness has improved
    let promoted = {
        let conn = super::feedback::open_evolution_db(chat_root)?;
        let promotable = super::feedback::find_promotable_external_rules(&conn, chat_root)?;
        promotable
        // conn dropped here
    };
    let promotion_changes = apply_external_rule_promotions(chat_root, &promoted)?;
    all_changes.extend(promotion_changes);

    let source_changes = evolve_sources(&mut registry.sources);
    all_changes.extend(source_changes);

    // Save updated registry
    save_sources(chat_root, &registry)?;

    // Phase 4: log to DB (sync)
    {
        let conn = super::feedback::open_evolution_db(chat_root)?;
        // Log the run itself for daily-cap tracking
        log_evolution_event(
            &conn, chat_root,
            "external_fetch_run", "",
            &format!("{} sources fetched, {} changes", to_fetch.len(), all_changes.len()),
            txn_id,
        )?;
        // Log individual changes
        for (ctype, cid) in &all_changes {
            log_evolution_event(&conn, chat_root, ctype, cid, "external learning", txn_id)?;
        }
    }

    tracing::info!("EVO-6: External learning complete — {} changes", all_changes.len());
    Ok(all_changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SourceEntry, SourceRegistry};

    fn make_source(id: &str, region: &str, accessibility: f32, quality: f32) -> SourceEntry {
        SourceEntry {
            id: id.to_string(),
            name: id.to_string(),
            url: format!("https://example.com/{}", id),
            source_type: "rss".to_string(),
            parser: "rss_generic".to_string(),
            region: region.to_string(),
            language: "zh".to_string(),
            domains: vec!["programming".to_string()],
            quality_score: quality,
            accessibility_score: accessibility,
            rules_contributed: 0,
            fetch_success_count: 0,
            fetch_fail_count: 0,
            last_fetched: None,
            mutable: true,
            origin: "seed".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn test_prioritize_sources_cn_first() {
        let sources = vec![
            make_source("global_a", "global", 0.9, 0.9),
            make_source("cn_b", "cn", 0.5, 0.5),
            make_source("cn_a", "cn", 0.9, 0.9),
        ];
        let registry = SourceRegistry { version: 1, sources };
        let prioritized = prioritize_sources(&registry.sources);
        assert_eq!(prioritized[0].region, "cn");
        assert_eq!(prioritized[1].region, "cn");
        assert_eq!(prioritized[2].region, "global");
        // Among CN: higher score first
        assert_eq!(prioritized[0].id, "cn_a");
    }

    #[test]
    fn test_update_accessibility_ema() {
        let mut src = make_source("test", "cn", 0.8, 0.8);
        update_accessibility(&mut src, true);
        let expected = 0.3 * 1.0 + 0.7 * 0.8;
        assert!((src.accessibility_score - expected).abs() < 1e-5);
        assert_eq!(src.fetch_success_count, 1);

        update_accessibility(&mut src, false);
        let expected2 = 0.3 * 0.0 + 0.7 * expected;
        assert!((src.accessibility_score - expected2).abs() < 1e-5);
        assert_eq!(src.fetch_fail_count, 1);
    }

    #[test]
    fn test_evolve_sources_pause_low_accessibility() {
        let mut sources = vec![{
            let mut s = make_source("low_access", "cn", 0.10, 0.70);
            s.fetch_fail_count = 8;
            s
        }];
        let changes = evolve_sources(&mut sources);
        assert!(!sources[0].enabled, "source should be paused");
        assert!(changes.iter().any(|(t, _)| t == "source_paused"));
    }

    #[test]
    fn test_evolve_sources_retire_mutable() {
        let mut sources = vec![{
            let mut s = make_source("low_quality", "cn", 0.9, 0.10);
            s.fetch_success_count = 25;
            s.fetch_fail_count = 10;
            s.rules_contributed = 0;
            s.mutable = true;
            s
        }];
        let changes = evolve_sources(&mut sources);
        assert!(!sources[0].enabled, "source should be retired");
        assert!(changes.iter().any(|(t, _)| t == "source_retired"));
    }

    #[test]
    fn test_evolve_sources_no_retire_immutable() {
        let mut sources = vec![{
            let mut s = make_source("seed_src", "cn", 0.9, 0.10);
            s.fetch_success_count = 25;
            s.fetch_fail_count = 10;
            s.rules_contributed = 0;
            s.mutable = false; // seed = immutable
            s
        }];
        let changes = evolve_sources(&mut sources);
        // Should NOT be retired (only paused if accessibility is low)
        assert!(!changes.iter().any(|(t, _)| t == "source_retired"));
    }

    #[test]
    fn test_extract_json_array_with_fences() {
        let input = "```json\n[{\"id\": \"ext_test\"}]\n```";
        let result = extract_json_array(input);
        assert!(result.contains("ext_test"));
        let arr: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn test_parse_external_rule_response_valid() {
        let input = r#"[{"id":"ext_prefer_logging","priority":50,"keywords":["log","debug"],"context_keywords":[],"tool_hint":null,"instruction":"Always add structured logging before running commands."}]"#;
        let rules = parse_external_rule_response(input).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "ext_prefer_logging");
        assert_eq!(rules[0].origin, "external");
        assert!(rules[0].mutable);
        assert_eq!(rules[0].priority, 50);
    }

    #[test]
    fn test_parse_external_rule_response_bad_id_rejected() {
        // Rule ID doesn't start with ext_ — should be rejected
        let input = r#"[{"id":"bad_rule","priority":50,"keywords":["log"],"context_keywords":[],"tool_hint":null,"instruction":"Some instruction."}]"#;
        let rules = parse_external_rule_response(input).unwrap();
        assert!(rules.is_empty(), "non-ext_ id should be rejected");
    }

    #[test]
    fn test_parse_rss_basic() {
        let rss = r#"<?xml version="1.0"?>
<rss><channel>
<item><title>Test Article</title><description>Some content here</description></item>
<item><title>Another Article</title><description>More content</description></item>
</channel></rss>"#;
        let articles = parse_rss(rss);
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].0, "Test Article");
    }

    #[test]
    fn test_strip_html_basic() {
        let html = "<p>Hello <b>world</b>!</p>";
        assert_eq!(strip_html_basic(html), "Hello world!");
    }

    #[test]
    fn test_should_run_env_disabled_by_default() {
        // Without SKILLLITE_EXTERNAL_LEARNING=1, should return false
        std::env::remove_var("SKILLLITE_EXTERNAL_LEARNING");
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::evolution::feedback::ensure_evolution_tables(&conn).unwrap();
        assert!(!should_run_external_learning(&conn));
    }

    #[test]
    fn test_merge_external_rules_no_duplicates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let chat_root = tmp.path();
        crate::evolution::seed::ensure_seed_data(chat_root);

        let new_rule = PlanningRule {
            id: "ext_test_rule".to_string(),
            priority: 50,
            keywords: vec!["test".to_string()],
            context_keywords: vec![],
            tool_hint: None,
            instruction: "Test external rule.".to_string(),
            mutable: true,
            origin: "external".to_string(),
            reusable: false,
            effectiveness: None,
            trigger_count: None,
        };

        // First merge: should add the rule
        let changes1 = merge_external_rules(chat_root, vec![new_rule.clone()]).unwrap();
        assert_eq!(changes1.len(), 1);
        assert_eq!(changes1[0].0, "external_rule_added");

        // Second merge: duplicate — should not add again
        let changes2 = merge_external_rules(chat_root, vec![new_rule]).unwrap();
        assert!(changes2.is_empty(), "duplicate rule should not be added again");
    }
}

fn parse_hn_algolia_json(raw: &str) -> Vec<(String, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let hits = v["hits"].as_array().cloned().unwrap_or_default();
    hits.iter().take(10).filter_map(|hit| {
        let title = hit["title"].as_str()?.to_string();
        let url = hit["url"].as_str().unwrap_or("").to_string();
        Some((title, url))
    }).collect()
}

