//! Optional UI locale hint via `SKILLLITE_UI_LOCALE` (e.g. desktop app settings).

use skilllite_core::config::env_keys::agent::SKILLLITE_UI_LOCALE;

const DESKTOP_LOCALE_HEADING: &str = "### Desktop UI language (from app settings)";

/// Markdown block appended to the system prompt when locale is recognized.
pub fn block_for_locale(locale: &str) -> Option<String> {
    let loc = locale.trim();
    if loc.is_empty() {
        return None;
    }
    let body = match loc {
        "en" => "The user selected **English** in the desktop app settings. Use clear, natural English by default for explanations and wording aimed at the user. If the user writes mainly in another language in their message, prefer replying in that language.",
        "zh" => "The user selected **Chinese (Simplified)** in the desktop app settings. Use clear, natural Simplified Chinese by default for explanations and wording aimed at the user. If the user writes mainly in another language in their message, prefer replying in that language.",
        _ => return None,
    };
    Some(format!("{DESKTOP_LOCALE_HEADING}\n\n{body}"))
}

/// Reads `SKILLLITE_UI_LOCALE` and returns the prompt block, if any.
pub fn context_append_from_ui_locale_env() -> Option<String> {
    std::env::var(SKILLLITE_UI_LOCALE)
        .ok()
        .and_then(|v| block_for_locale(&v))
}

/// Prepends the locale block from env when set. Skips if `existing` already contains the heading
/// (avoids duplicate when `context_append` was already built from the same env).
pub fn merge_ui_locale_env_into_context_append(existing: Option<String>) -> Option<String> {
    let Some(loc_block) = context_append_from_ui_locale_env() else {
        return existing;
    };
    let Some(ex) = existing else {
        return Some(loc_block);
    };
    if ex.contains(DESKTOP_LOCALE_HEADING) {
        return Some(ex);
    }
    Some(format!("{loc_block}\n\n{ex}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_for_locale_en_zh_only() {
        assert!(block_for_locale("en").is_some());
        assert!(block_for_locale("zh").is_some());
        assert!(block_for_locale("fr").is_none());
    }
}
