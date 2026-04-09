//! 会话结束时由 LLM 生成「猜你想问」短列表（非 agent 循环，单次 chat completion）。

use std::collections::HashMap;

use skilllite_agent::llm::LlmClient;
use skilllite_agent::types::ChatMessage;
use skilllite_core::config::env_keys::llm as llm_keys;
use skilllite_core::config::LlmConfig;

use super::chat::{merge_dotenv_with_chat_overrides, ChatConfigOverrides};
use super::paths::load_dotenv_for_child;

fn env_lookup(map: &HashMap<String, String>, primary: &str, aliases: &[&str]) -> Option<String> {
    for k in std::iter::once(primary).chain(aliases.iter().copied()) {
        if let Some(v) = map.get(k) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn llm_from_merged_pairs(pairs: &[(String, String)]) -> Result<(String, String, String), String> {
    let map: HashMap<String, String> = pairs.iter().cloned().collect();
    let api_key = env_lookup(&map, llm_keys::API_KEY, llm_keys::API_KEY_ALIASES)
        .ok_or_else(|| "API key not configured".to_string())?;
    let api_base = env_lookup(&map, llm_keys::API_BASE, llm_keys::API_BASE_ALIASES)
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let model = env_lookup(&map, llm_keys::MODEL, llm_keys::MODEL_ALIASES)
        .unwrap_or_else(|| LlmConfig::default_model_for_base(&api_base).to_string());
    Ok((api_base, api_key, model))
}

/// Last non-empty `User:` block in the condensed transcript (`serializeChatMessagesForFollowup` shape).
fn last_user_message_text(transcript: &str) -> Option<&str> {
    let mut last: Option<&str> = None;
    for block in transcript.split("\n\n") {
        let b = block.trim();
        if let Some(rest) = b.strip_prefix("User:") {
            let t = rest.trim();
            if !t.is_empty() {
                last = Some(t);
            }
        }
    }
    last
}

fn contains_non_latin_script(s: &str) -> bool {
    s.chars().any(|c| {
        let u = u32::from(c);
        (0x4E00..=0x9FFF).contains(&u)
            || (0x3040..=0x30FF).contains(&u)
            || (0xAC00..=0xD7AF).contains(&u)
    })
}

/// Strong, model-specific language lock derived from the last user turn (transcript is often English-heavy).
fn followup_language_clause(last_user: Option<&str>) -> &'static str {
    let Some(s) = last_user else {
        return "";
    };
    if contains_non_latin_script(s) {
        "\n\n【Language — mandatory】The user's last message uses Chinese/Japanese/Korean (CJK). Write all 3 follow-up questions in the same language as that message (use Simplified Chinese if the user wrote Chinese). Do not answer in English.\n【语言 — 必须遵守】最后一条用户消息含中日韩文字。三条后续提问必须与该条用户消息使用同一种自然语言（用户用中文则用简体中文）。禁止仅用英文输出。"
    } else {
        let ascii_letters = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
        if ascii_letters >= 3 {
            "\n\n【Language — mandatory】The user's last message is in English (Latin script). Write all 3 follow-up questions in English.\n【语言 — 必须遵守】最后一条用户消息为英文。三条后续提问必须使用英文。"
        } else {
            "\n\n【Language — mandatory】Match the natural language of the user's last message in the transcript above (not the assistant's). If it is Chinese, use Chinese; if English, use English."
        }
    }
}

fn parse_suggestion_lines(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let mut t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("```") {
            t = rest.trim();
        }
        t = t.trim_end_matches('`').trim();
        let t = t
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches(['.', ')', '、'])
            .trim();
        let t = t
            .trim_start_matches("- ")
            .trim_start_matches("• ")
            .trim_start_matches("* ")
            .trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        if out.len() >= 3 {
            break;
        }
    }
    out
}

/// 根据本轮对话摘录生成最多 3 条后续提问建议；`transcript` 为空则返回空 Vec。
pub async fn followup_chat_suggestions(
    transcript: String,
    workspace: Option<String>,
    config_overrides: Option<ChatConfigOverrides>,
) -> Result<Vec<String>, String> {
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    let raw_workspace = workspace
        .or_else(|| config_overrides.as_ref().and_then(|c| c.workspace.clone()))
        .unwrap_or_else(|| ".".to_string());
    let pairs = merge_dotenv_with_chat_overrides(
        load_dotenv_for_child(&raw_workspace),
        config_overrides.as_ref(),
    );
    let (api_base, api_key, model) = llm_from_merged_pairs(&pairs)?;
    let client = LlmClient::new(&api_base, &api_key).map_err(|e| e.to_string())?;

    let body: String = trimmed.chars().take(12_000).collect();
    let last_user = last_user_message_text(trimmed);
    let lang = followup_language_clause(last_user);
    let user_msg = format!(
        "Here is a condensed transcript of a chat session that just ended (User / Assistant turns only):\n\n\"\"\"\n{body}\n\"\"\"\n\nGenerate exactly 3 short follow-up questions the user might want to ask next.\nRules:\n- The assistant's replies may be in English; ignore that for language choice. Use only the **user's** messages to decide the output language (especially the **last user message**).\n- Each question must suggest a new direction or refinement; do not repeat what was already settled.\n- Output exactly 3 lines: one question per line. No numbering, no bullets, no preamble or closing.{lang}"
    );

    let messages = vec![
        ChatMessage::system(
            "You suggest short follow-up questions after an agent chat. The transcript mixes languages; you MUST follow the mandatory Language section in the user message (based on the last user turn), not the assistant's language.",
        ),
        ChatMessage::user(&user_msg),
    ];

    let resp = client
        .chat_completion(&model, &messages, None, Some(0.4), None)
        .await
        .map_err(|e| e.to_string())?;

    let text = resp
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("")
        .trim();

    Ok(parse_suggestion_lines(text))
}

#[cfg(test)]
mod tests {
    use super::{followup_language_clause, last_user_message_text, parse_suggestion_lines};

    #[test]
    fn parses_numbered_and_bullets() {
        let raw = "1. First question?\n2. Second?\n- Third one?\n";
        let v = parse_suggestion_lines(raw);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], "First question?");
        assert_eq!(v[1], "Second?");
        assert_eq!(v[2], "Third one?");
    }

    #[test]
    fn last_user_picks_final_user_block() {
        let tr = "User: 你好\n\nAssistant: Hi\n\nUser: 请总结\n\nAssistant: Done.";
        assert_eq!(last_user_message_text(tr), Some("请总结"));
    }

    #[test]
    fn language_clause_cjk_mandates_chinese_block() {
        let s = followup_language_clause(Some("帮我写单元测试"));
        assert!(s.contains("简体中文") || s.contains("Chinese"));
        assert!(s.contains("禁止"));
    }

    #[test]
    fn language_clause_ascii_mandates_english() {
        let s = followup_language_clause(Some("Write unit tests for this module"));
        assert!(s.contains("English"));
    }
}
