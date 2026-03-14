//! Lightweight replay runner for JSONL evaluation sets.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplayCase {
    pub id: String,
    pub prompt: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub validation_focus: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayCaseResult {
    pub id: String,
    pub success: bool,
    pub first_success: bool,
    pub replans: usize,
    pub total_tools: usize,
    pub elapsed_ms: u64,
    pub response_preview: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplaySummary {
    pub dataset: String,
    pub total_cases: usize,
    pub completed_cases: usize,
    pub first_success_rate: f64,
    pub completion_rate: f64,
    pub avg_replans: f64,
    pub avg_tool_calls: f64,
    pub total_elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayReport {
    pub summary: ReplaySummary,
    pub results: Vec<ReplayCaseResult>,
}

fn truncate_preview(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.trim().to_string();
    }
    text.chars().take(max_chars).collect::<String>().trim().to_string() + "..."
}

pub fn load_replay_cases(dataset_path: &Path) -> Result<Vec<ReplayCase>> {
    let file = File::open(dataset_path)
        .with_context(|| format!("Failed to open replay dataset: {}", dataset_path.display()))?;
    let reader = BufReader::new(file);
    let mut cases = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| {
            format!(
                "Failed to read replay dataset line {} from {}",
                idx + 1,
                dataset_path.display()
            )
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let case: ReplayCase = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "Invalid replay JSONL at line {} in {}",
                idx + 1,
                dataset_path.display()
            )
        })?;
        cases.push(case);
    }

    if cases.is_empty() {
        anyhow::bail!("Replay dataset is empty: {}", dataset_path.display());
    }
    Ok(cases)
}

pub fn cmd_replay(
    api_base: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    workspace: Option<String>,
    skill_dirs: Vec<String>,
    dataset: String,
    max_iterations: usize,
    max_failures: Option<usize>,
    limit: Option<usize>,
    json_output: bool,
    verbose: bool,
) -> Result<()> {
    let mut config = skilllite_agent::types::AgentConfig::from_env();
    if let Some(base) = api_base {
        config.api_base = base;
    }
    if let Some(key) = api_key {
        config.api_key = key;
    }
    if let Some(model) = model {
        config.model = model;
    }
    if let Some(workspace) = workspace {
        config.workspace = workspace;
    }
    config.max_iterations = max_iterations;
    config.verbose = verbose;
    config.enable_task_planning = true;
    config.enable_memory = true;
    config.max_consecutive_failures = match max_failures {
        Some(0) => None,
        Some(n) => Some(n),
        None => Some(5),
    };

    if config.api_key.is_empty() {
        anyhow::bail!("API key required. Set OPENAI_API_KEY env var or use --api-key.");
    }

    let dataset_path = PathBuf::from(&dataset);
    let mut cases = load_replay_cases(&dataset_path)?;
    if let Some(limit) = limit {
        cases.truncate(limit.min(cases.len()));
    }

    let effective_skill_dirs = if skill_dirs.is_empty() {
        skilllite_core::skill::discovery::discover_skill_dirs_for_loading(
            Path::new(&config.workspace),
            Some(&[".skills", "skills"]),
        )
    } else {
        skill_dirs
    };
    let loaded_skills = skilllite_agent::skills::load_skills(&effective_skill_dirs);
    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

    let total_cases = cases.len();
    if !json_output {
        eprintln!("┌─ Replay ─────────────────────────────────────────────────");
        eprintln!("│  数据集: {}", dataset_path.display());
        eprintln!("│  样本数: {}", total_cases);
        eprintln!("│  模型: {}", config.model);
        eprintln!("│  工作区: {}", config.workspace);
        if !loaded_skills.is_empty() {
            eprintln!("│  已加载技能: {}", loaded_skills.len());
        }
        eprintln!("└──────────────────────────────────────────────────────────\n");
    }

    let mut results = Vec::with_capacity(total_cases);
    for (idx, case) in cases.iter().enumerate() {
        if !json_output {
            eprintln!(
                "[{}/{}] {}  {}",
                idx + 1,
                total_cases,
                case.id,
                truncate_preview(&case.prompt, 72)
            );
        }

        let started = Instant::now();
        let case_result = rt.block_on(async {
            let mut sink: Box<dyn skilllite_agent::types::EventSink> = if verbose {
                Box::new(skilllite_agent::types::RunModeEventSink::new(true))
            } else {
                Box::new(skilllite_agent::types::SilentEventSink)
            };

            match skilllite_agent::agent_loop::run_agent_loop(
                &config,
                Vec::new(),
                &case.prompt,
                &loaded_skills,
                sink.as_mut(),
                None,
            )
            .await
            {
                Ok(result) => ReplayCaseResult {
                    id: case.id.clone(),
                    success: result.feedback.task_completed,
                    first_success: result.feedback.task_completed && result.feedback.replans == 0,
                    replans: result.feedback.replans,
                    total_tools: result.feedback.total_tools,
                    elapsed_ms: result.feedback.elapsed_ms,
                    response_preview: truncate_preview(&result.response, 160),
                    error: None,
                },
                Err(err) => ReplayCaseResult {
                    id: case.id.clone(),
                    success: false,
                    first_success: false,
                    replans: 0,
                    total_tools: 0,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    response_preview: String::new(),
                    error: Some(err.to_string()),
                },
            }
        });

        if !json_output {
            if let Some(err) = &case_result.error {
                eprintln!("  ✗ 失败: {}", truncate_preview(err, 160));
            } else {
                eprintln!(
                    "  {} success={} first_success={} replans={} tools={} elapsed={}ms",
                    if case_result.success { "✓" } else { "!" },
                    case_result.success,
                    case_result.first_success,
                    case_result.replans,
                    case_result.total_tools,
                    case_result.elapsed_ms
                );
            }
        }

        results.push(case_result);
    }

    let completed_cases = results.iter().filter(|r| r.success).count();
    let first_success_cases = results.iter().filter(|r| r.first_success).count();
    let total_elapsed_ms: u64 = results.iter().map(|r| r.elapsed_ms).sum();
    let avg_replans = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.replans as f64).sum::<f64>() / results.len() as f64
    };
    let avg_tool_calls = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.total_tools as f64).sum::<f64>() / results.len() as f64
    };
    let summary = ReplaySummary {
        dataset: dataset_path.display().to_string(),
        total_cases: results.len(),
        completed_cases,
        first_success_rate: if results.is_empty() {
            0.0
        } else {
            first_success_cases as f64 / results.len() as f64
        },
        completion_rate: if results.is_empty() {
            0.0
        } else {
            completed_cases as f64 / results.len() as f64
        },
        avg_replans,
        avg_tool_calls,
        total_elapsed_ms,
    };
    let report = ReplayReport { summary, results };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        eprintln!("\nReplay summary");
        eprintln!("  完成率: {:.0}%", report.summary.completion_rate * 100.0);
        eprintln!("  首次成功率: {:.0}%", report.summary.first_success_rate * 100.0);
        eprintln!("  平均 replan: {:.2}", report.summary.avg_replans);
        eprintln!("  平均 tool calls: {:.2}", report.summary.avg_tool_calls);
        eprintln!("  总耗时: {} ms", report.summary.total_elapsed_ms);
        let failed_ids: Vec<&str> = report
            .results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.id.as_str())
            .collect();
        if !failed_ids.is_empty() {
            eprintln!("  失败样本: {}", failed_ids.join(", "));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_replay_cases_parses_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cases.jsonl");
        std::fs::write(
            &path,
            r#"{"id":"a","prompt":"first"}
{"id":"b","prompt":"second","tags":["x"]}"#,
        )
        .unwrap();

        let cases = load_replay_cases(&path).unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].id, "a");
        assert_eq!(cases[1].tags, vec!["x".to_string()]);
    }
}
