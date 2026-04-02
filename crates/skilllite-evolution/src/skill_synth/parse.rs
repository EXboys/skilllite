//! LLM 响应解析：技能生成、精炼 JSON

use crate::Result;
use regex::Regex;

use crate::prompt_learner;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

use super::infer;
use super::MAX_PARSE_RETRIES;

/// Re-export for defense-in-depth usage in parsers.
pub(super) use crate::strip_think_blocks;

/// Fix LLM over-escaping: JSON may contain literal `\n` (backslash+n) instead of newlines.
pub(super) fn unescape_llm_newlines(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

pub(super) struct GeneratedSkill {
    pub name: String,
    pub description: String,
    pub entry_point: String,
    pub script_content: String,
    pub skill_md_content: String,
}

/// 修复类型：由 LLM 根据 error_trace 诊断得出
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FixType {
    Script,
    TestInput,
    SkillMd,
    /// 回滚到上轮开始前的状态，下一轮再尝试其他修复
    Rollback,
    #[allow(dead_code)]
    Unfixable,
}

pub(super) struct RefinedSkill {
    pub fix_type: FixType,
    pub fix_summary: String,
    pub fixed_script: Option<String>,
    pub fix_test_input: Option<String>,
    pub fix_skill_md: Option<String>,
    /// LLM 返回的跳过原因，解析时保留供日志/后续使用
    #[allow(dead_code)]
    pub skip_reason: Option<String>,
    /// 回复给用户的进度/说明，修复过程中可展示
    pub user_reply: Option<String>,
}

/// Try to repair JSON truncated mid-string (e.g. LLM hit token limit).
fn try_repair_truncated_skill_json(json_str: &str) -> Option<String> {
    let trimmed = json_str.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let open_braces = trimmed.matches('{').count();
    let close_braces = trimmed.matches('}').count();
    let to_close_braces = open_braces.saturating_sub(close_braces);

    let last_char = trimmed.chars().last().unwrap_or(' ');
    let in_string = !matches!(last_char, '"' | '}' | ']' | ',' | ' ' | '\n' | '\t');
    let ends_with_backslash = trimmed.ends_with('\\');

    let mut repaired = trimmed.to_string();
    if (in_string || ends_with_backslash) && !trimmed.ends_with('"') {
        repaired.push('"');
    }
    for _ in 0..to_close_braces {
        repaired.push('}');
    }
    if repaired != trimmed {
        tracing::debug!(
            "Attempted JSON repair: appended {} chars",
            repaired.len() - trimmed.len()
        );
        Some(repaired)
    } else {
        None
    }
}

/// LLM 常输出 JS 风格未加引号的 key，尝试修复为合法 JSON
fn try_repair_unquoted_keys(json_str: &str) -> Option<String> {
    let re1 = Regex::new(r"\{\s*fixed_script\s*:").ok()?;
    let re2 = Regex::new(r"\{\s*fix_summary\s*:").ok()?;
    let re3 = Regex::new(r"\{\s*skip_reason\s*:").ok()?;
    let re4 = Regex::new(r"\{\s*fix_type\s*:").ok()?;
    let re5 = Regex::new(r"\{\s*fix_test_input\s*:").ok()?;
    let re6 = Regex::new(r"\{\s*fix_skill_md\s*:").ok()?;
    let re7 = Regex::new(r"\{\s*user_reply\s*:").ok()?;
    let mut s = json_str.to_string();
    s = re1.replace_all(&s, r#"{"fixed_script":"#).into_owned();
    s = re2.replace_all(&s, r#"{"fix_summary":"#).into_owned();
    s = re3.replace_all(&s, r#"{"skip_reason":"#).into_owned();
    s = re4.replace_all(&s, r#"{"fix_type":"#).into_owned();
    s = re5.replace_all(&s, r#"{"fix_test_input":"#).into_owned();
    s = re6.replace_all(&s, r#"{"fix_skill_md":"#).into_owned();
    s = re7.replace_all(&s, r#"{"user_reply":"#).into_owned();
    let re_comma = Regex::new(r",\s*(fixed_script|fix_summary|skip_reason|fix_type|fix_test_input|fix_skill_md|user_reply)\s*:").ok()?;
    s = re_comma.replace_all(&s, r#","$1":"#).into_owned();
    if s != json_str {
        Some(s)
    } else {
        None
    }
}

pub(super) async fn parse_skill_generation_with_retry<L: EvolutionLlm>(
    llm: &L,
    model: &str,
    messages: &[EvolutionMessage],
) -> Result<Option<GeneratedSkill>> {
    let content = llm.complete(messages, model, 0.3).await?.trim().to_string();
    match parse_skill_generation_response(&content) {
        ok @ Ok(_) => ok,
        Err(e) => {
            for attempt in 0..MAX_PARSE_RETRIES {
                tracing::info!(
                    "Skill generation JSON parse failed (attempt {}), retrying with LLM feedback: {}",
                    attempt + 1,
                    e
                );
                let retry_msg = format!(
                    "你的上一次输出无法解析为 JSON。错误: {}。\n\n请重新输出完整、合法的 JSON。确保 script_content 和 skill_md_content 中的字符串正确转义：换行用 \\n，制表符用 \\t，双引号用 \\\"。",
                    e
                );
                let mut msgs = messages.to_vec();
                msgs.push(EvolutionMessage::user(&retry_msg));
                let content2 = llm.complete(&msgs, model, 0.3).await?.trim().to_string();
                match parse_skill_generation_response(&content2) {
                    ok @ Ok(_) => return ok,
                    Err(e2) => {
                        if attempt == MAX_PARSE_RETRIES - 1 {
                            return Err(crate::Error::validation(format!(
                                "Parse retry failed: {}",
                                e2
                            )));
                        }
                    }
                }
            }
            Err(e)
        }
    }
}

pub(super) fn parse_skill_generation_response(content: &str) -> Result<Option<GeneratedSkill>> {
    let content = strip_think_blocks(content);
    let json_str = prompt_learner::extract_json_block(content);

    let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("EOF") || err_str.contains("unexpected end of file") {
                if let Some(repaired) = try_repair_truncated_skill_json(&json_str) {
                    serde_json::from_str(&repaired).map_err(|e2| {
                        crate::Error::validation(format!(
                            "Failed to parse skill generation JSON (after repair): {}",
                            e2
                        ))
                    })?
                } else {
                    return Err(crate::Error::validation(format!(
                        "Failed to parse skill generation JSON: {}",
                        e
                    )));
                }
            } else {
                return Err(crate::Error::validation(format!(
                    "Failed to parse skill generation JSON: {}",
                    e
                )));
            }
        }
    };

    if let Some(skip) = parsed.get("skip_reason").and_then(|v| v.as_str()) {
        if !skip.is_empty() && skip != "null" {
            tracing::debug!("Skill generation skipped: {}", skip);
            return Ok(None);
        }
    }

    let skill = parsed
        .get("skill")
        .ok_or_else(|| crate::Error::validation("No 'skill' field in response"))?;

    let name = skill
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() || name.len() > 50 {
        return Ok(None);
    }

    let description = skill
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let entry_point = skill
        .get("entry_point")
        .and_then(|v| v.as_str())
        .unwrap_or("scripts/main.py")
        .to_string();
    let script_content = skill
        .get("script_content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let skill_md_content = unescape_llm_newlines(
        skill
            .get("skill_md_content")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );

    if script_content.is_empty() || skill_md_content.is_empty() {
        return Ok(None);
    }

    if script_content.lines().count() > 150 {
        tracing::warn!("Generated script exceeds 150 lines, rejecting");
        return Ok(None);
    }

    Ok(Some(GeneratedSkill {
        name,
        description,
        entry_point,
        script_content,
        skill_md_content,
    }))
}

pub(super) fn parse_refinement_response(content: &str) -> Result<Option<RefinedSkill>> {
    let content = strip_think_blocks(content);
    let mut json_str = prompt_learner::extract_json_block(content);
    if json_str.trim().is_empty() || !json_str.trim().starts_with('{') {
        json_str = infer::extract_first_json_object(content)
            .unwrap_or("{}")
            .to_string();
    }

    let parsed: serde_json::Value = match serde_json::from_str(json_str.trim()) {
        Ok(p) => p,
        Err(e) => {
            if let Some(repaired) = try_repair_unquoted_keys(&json_str) {
                if let Ok(p) = serde_json::from_str(&repaired) {
                    p
                } else {
                    let trimmed = content.trim();
                    let from_brace = trimmed
                        .find('{')
                        .and_then(|i| infer::extract_first_json_object(&trimmed[i..]))
                        .unwrap_or("{}");
                    let from_str = from_brace.to_string();
                    if let Some(repaired2) = try_repair_unquoted_keys(&from_str) {
                        serde_json::from_str(&repaired2).map_err(|e2| {
                            crate::Error::validation(format!(
                                "Failed to parse refinement JSON: {}",
                                e2
                            ))
                        })?
                    } else {
                        return Err(crate::Error::validation(format!(
                            "Failed to parse refinement JSON: {}",
                            e
                        )));
                    }
                }
            } else {
                let trimmed = content.trim();
                let from_brace = trimmed
                    .find('{')
                    .and_then(|i| infer::extract_first_json_object(&trimmed[i..]))
                    .unwrap_or("{}");
                let from_str = from_brace.to_string();
                serde_json::from_str(&from_str)
                    .or_else(|_| {
                        try_repair_unquoted_keys(&from_str)
                            .and_then(|r| serde_json::from_str(&r).ok())
                            .ok_or(e)
                    })
                    .map_err(|e2| {
                        crate::Error::validation(format!("Failed to parse refinement JSON: {}", e2))
                    })?
            }
        }
    };

    let fix_summary = parsed
        .get("fix_summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let fix_type_str = parsed
        .get("fix_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let fixed_script = parsed
        .get("fixed_script")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let fix_test_input = parsed
        .get("fix_test_input")
        .and_then(|v| {
            if v.is_null() {
                None
            } else if let Some(s) = v.as_str() {
                if s.is_empty() || s == "null" {
                    None
                } else {
                    Some(s.to_string())
                }
            } else {
                // 模型可能返回 JSON 对象而非字符串
                Some(serde_json::to_string(v).unwrap_or_default())
            }
        })
        .filter(|s| !s.is_empty());
    let fix_skill_md = parsed
        .get("fix_skill_md")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let skip_reason = parsed
        .get("skip_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty() && s != "null");
    let user_reply = parsed
        .get("user_reply")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty() && s != "null");

    // fix_type 不再是必填项；只要有任何非 null 的修复字段就视为有效
    let has_any_fix = fixed_script.is_some() || fix_test_input.is_some() || fix_skill_md.is_some();

    let fix_type = if fix_type_str == "rollback" {
        FixType::Rollback
    } else if (fix_type_str == "unfixable" || skip_reason.is_some()) && !has_any_fix {
        return Ok(None);
    } else if has_any_fix {
        // 有修复内容，按优先级推断 fix_type（仅用于兼容旧调用方）
        if fix_skill_md.is_some() {
            FixType::SkillMd
        } else if fixed_script.is_some() {
            FixType::Script
        } else {
            FixType::TestInput
        }
    } else {
        tracing::debug!("No valid fix content");
        return Ok(None);
    };

    Ok(Some(RefinedSkill {
        fix_type,
        fix_summary,
        fixed_script,
        fix_test_input,
        fix_skill_md,
        skip_reason,
        user_reply,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_refinement_response() {
        let json = serde_json::json!({
            "fix_type": "script",
            "fixed_script": "#!/usr/bin/env python3\nimport sys\nprint('fixed')",
            "fix_summary": "Removed unsafe eval call",
            "fix_test_input": null,
            "fix_skill_md": null,
            "skip_reason": null
        })
        .to_string();

        let result = parse_refinement_response(&json).expect("valid refinement JSON should parse");
        assert!(result.is_some());
        let refined = result.expect("test has fix_type script");
        assert_eq!(refined.fix_type, FixType::Script);
        assert!(refined
            .fixed_script
            .as_ref()
            .expect("test sets fixed_script")
            .contains("fixed"));
        assert_eq!(refined.fix_summary, "Removed unsafe eval call");
    }

    #[test]
    fn test_parse_refinement_test_input() {
        let json = serde_json::json!({
            "fix_type": "test_input",
            "fix_summary": "补全 base, exponent",
            "fix_test_input": "{\"base\": 2, \"exponent\": 3}",
            "fixed_script": null,
            "fix_skill_md": null,
            "skip_reason": null
        })
        .to_string();

        let result =
            parse_refinement_response(&json).expect("valid test_input refinement should parse");
        assert!(result.is_some());
        let refined = result.expect("test has fix_type test_input");
        assert_eq!(refined.fix_type, FixType::TestInput);
        assert_eq!(
            refined.fix_test_input.as_deref(),
            Some("{\"base\": 2, \"exponent\": 3}")
        );
    }

    #[test]
    fn test_parse_refinement_unquoted_keys() {
        let json =
            r##"{fix_type: "script", fixed_script: "a", fix_summary: "b", skip_reason: null}"##;
        let result = parse_refinement_response(json).expect("unquoted keys should be repaired");
        assert!(result.is_some(), "repair should fix unquoted keys");
        let refined = result.expect("repair yields refined");
        assert_eq!(refined.fixed_script.as_deref(), Some("a"));
        assert_eq!(refined.fix_summary, "b");
    }

    #[test]
    fn test_parse_refinement_unfixable() {
        let json = serde_json::json!({
            "fix_type": "unfixable",
            "fix_summary": "",
            "skip_reason": "CLI 型技能，需 argparse 参数"
        })
        .to_string();
        let result =
            parse_refinement_response(&json).expect("unfixable JSON should parse (returns None)");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_skill_generation() {
        let json = serde_json::json!({
            "skill": {
                "name": "daily-report",
                "description": "Generate daily work summary",
                "entry_point": "main.py",
                "input_schema": {"type": "object", "properties": {}},
                "script_content": "#!/usr/bin/env python3\nimport sys\nprint('hello')",
                "skill_md_content": "# Skill: daily-report\n\n## Description\nGenerate daily summary"
            },
            "skip_reason": null
        })
        .to_string();

        let result = parse_skill_generation_response(&json)
            .expect("valid skill generation JSON should parse");
        assert!(result.is_some());
        let skill = result.expect("test has skill");
        assert_eq!(skill.name, "daily-report");
        assert!(!skill.script_content.is_empty());
    }

    #[test]
    fn test_parse_skill_generation_skipped() {
        let json =
            serde_json::json!({"skill": null, "skip_reason": "no repeated pattern"}).to_string();
        let result = parse_skill_generation_response(&json)
            .expect("skipped skill JSON should parse (returns None)");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_refinement_with_think_block() {
        let content = r#"<redacted_thinking>
Let me analyze this task carefully.
The test input was `{}` (empty JSON object).
The script expects `base` and `exponent` parameters like {"base": 2}.
</redacted_thinking>
{"fix_summary": "补充测试输入", "fix_test_input": "{\"base\": 2, \"exponent\": 10}", "fixed_script": null, "fix_skill_md": null}"#;
        let result =
            parse_refinement_response(content).expect("content with think block should parse");
        assert!(
            result.is_some(),
            "should parse JSON after <redacted_thinking> block"
        );
        let refined = result.expect("test has refinement");
        assert_eq!(
            refined.fix_test_input.as_deref(),
            Some("{\"base\": 2, \"exponent\": 10}")
        );
    }

    #[test]
    fn test_parse_refinement_think_block_with_code_fence() {
        let content = "<redacted_thinking>\nAnalyzing the issue...\n</redacted_thinking>\n```json\n{\"fix_summary\": \"fix\", \"fix_test_input\": \"{\\\"start\\\": 1, \\\"end\\\": 10}\"}\n```";
        let result = parse_refinement_response(content)
            .expect("content with think block and code fence should parse");
        assert!(
            result.is_some(),
            "should parse JSON in code fence after <redacted_thinking>"
        );
        let refined = result.expect("test has refinement");
        assert!(refined.fix_test_input.is_some());
    }

    #[test]
    fn test_strip_think_blocks() {
        // Normal closed tags
        assert_eq!(
            strip_think_blocks("<redacted_thinking>foo</redacted_thinking>{\"a\":1}"),
            "{\"a\":1}"
        );
        assert_eq!(strip_think_blocks("no think here"), "no think here");
        assert_eq!(
            strip_think_blocks("<redacted_thinking>x{y}z</redacted_thinking>\n{\"b\":2}"),
            "{\"b\":2}"
        );
        assert_eq!(
            strip_think_blocks("<thinking>analysis</thinking>{\"c\":3}"),
            "{\"c\":3}"
        );
        assert_eq!(
            strip_think_blocks("<reasoning>step 1</reasoning>\n{\"d\":4}"),
            "{\"d\":4}"
        );
        // Nested / multiple think blocks
        assert_eq!(
            strip_think_blocks("<redacted_thinking>a</redacted_thinking>mid<redacted_thinking>b</redacted_thinking>{\"e\":5}"),
            "{\"e\":5}"
        );
        // Think block at end (content before it)
        assert_eq!(
            strip_think_blocks("{\"f\":6}<redacted_thinking>verify</redacted_thinking>"),
            "{\"f\":6}<redacted_thinking>verify</redacted_thinking>"
        );
        // Unclosed think tag — take content before the opening tag
        assert_eq!(
            strip_think_blocks("{\"g\":7}<redacted_thinking>still thinking..."),
            "{\"g\":7}"
        );
        // Unclosed think tag with nothing before — return original
        assert_eq!(
            strip_think_blocks("<redacted_thinking>thinking with no output"),
            "<redacted_thinking>thinking with no output"
        );
    }

    #[test]
    fn test_parse_skill_generation_too_long() {
        let long_script = (0..200)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let json = serde_json::json!({
            "skill": {
                "name": "test",
                "description": "t",
                "entry_point": "main.py",
                "script_content": long_script,
                "skill_md_content": "# test"
            },
            "skip_reason": null
        })
        .to_string();
        let result = parse_skill_generation_response(&json)
            .expect("long script JSON should parse (returns None)");
        assert!(result.is_none(), "should reject scripts > 150 lines");
    }
}
