//! Redact subscription tokens before a line leaves the sandbox.

/// Replace each `sk-` token with `[redacted]`.
///
/// This covers Anthropic keys (`sk-ant-…`) and OpenAI keys (`sk-proj-…` and `sk-…`).
///
/// The marker and the token body are ASCII. Surrounding text, including
/// non-ASCII characters, is copied through on character boundaries.
pub fn redact_line(line: &str) -> String {
    const MARKER: &str = "sk-";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(index) = rest.find(MARKER) {
        out.push_str(&rest[..index]);
        out.push_str("[redacted]");
        let after = &rest[index + MARKER.len()..];
        let skip = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .map(char::len_utf8)
            .sum::<usize>();
        rest = &after[skip..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_token_and_keeps_non_ascii_text() {
        let line = "你好 sk-ant-abc_123-XYZ 世界";
        assert_eq!(redact_line(line), "你好 [redacted] 世界");
    }

    #[test]
    fn leaves_unrelated_text_unchanged() {
        assert_eq!(redact_line("no token here"), "no token here");
    }

    #[test]
    fn redacts_openai_project_key_and_keeps_non_ascii() {
        assert_eq!(
            redact_line("密钥 sk-proj-abc_1 中文"),
            "密钥 [redacted] 中文"
        );
    }

    #[test]
    fn redacts_marker_with_empty_body() {
        assert_eq!(
            redact_line("prefix sk-ant- suffix"),
            "prefix [redacted] suffix"
        );
    }
}
