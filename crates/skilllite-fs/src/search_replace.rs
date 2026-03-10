//! search_replace 与 insert_lines：精确/模糊替换、行插入

use anyhow::Result;

/// 纯内存：精确 search_replace
pub fn apply_search_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<(String, usize)> {
    if old_string.is_empty() {
        anyhow::bail!("old_string cannot be empty");
    }
    let count = content.matches(old_string).count();
    if count == 0 {
        anyhow::bail!("old_string not found in content");
    }
    let new_content = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    let replaced = if replace_all { count } else { 1 };
    Ok((new_content, replaced))
}

/// 模糊匹配结果
#[derive(Debug, Clone)]
pub struct FuzzyReplaceResult {
    pub match_type: String,
    pub total_occurrences: usize,
    pub replaced_count: usize,
    pub first_match_start: usize,
    pub first_match_len: usize,
    pub new_content: String,
}

/// 精确或模糊替换（单次替换时启用 fuzzy fallback）
pub fn apply_replace_fuzzy(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<FuzzyReplaceResult> {
    if old_string.is_empty() {
        anyhow::bail!("old_string cannot be empty");
    }
    let exact_count = content.matches(old_string).count();
    if exact_count > 0 {
        if !replace_all && exact_count > 1 {
            anyhow::bail!(
                "Found {} occurrences of old_string. search_replace requires a unique match by default; add more context or set replace_all=true.",
                exact_count
            );
        }
        let first_start = content.find(old_string).unwrap_or(0);
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        return Ok(FuzzyReplaceResult {
            match_type: "exact".to_string(),
            total_occurrences: exact_count,
            replaced_count: if replace_all { exact_count } else { 1 },
            first_match_start: first_start,
            first_match_len: old_string.len(),
            new_content,
        });
    }
    if replace_all {
        anyhow::bail!("old_string not found in content");
    }
    match fuzzy_find(content, old_string) {
        Some(fm) => {
            let new_content = format!(
                "{}{}{}",
                &content[..fm.start],
                new_string,
                &content[fm.end..],
            );
            Ok(FuzzyReplaceResult {
                match_type: fm.match_type,
                total_occurrences: 1,
                replaced_count: 1,
                first_match_start: fm.start,
                first_match_len: fm.end - fm.start,
                new_content,
            })
        }
        None => {
            let hint = build_failure_hint(content, old_string);
            anyhow::bail!(
                "old_string not found in file (tried exact + fuzzy matching).\n\n{}\n\nTip: Copy the exact text from above into old_string, or use insert_lines with line number.",
                hint
            )
        }
    }
}

/// normalize_whitespace 模式：忽略行尾空白
pub fn apply_replace_normalize_whitespace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<FuzzyReplaceResult> {
    let escaped = regex::escape(old_string);
    let pattern = format!(r"({})([ \t]*)(\r?\n|$)", escaped);
    let re = regex::Regex::new(&pattern)
        .map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;
    let matches: Vec<_> = re.find_iter(content).collect();
    let count = matches.len();
    if count == 0 {
        anyhow::bail!("old_string not found (with normalize_whitespace)");
    }
    if !replace_all && count > 1 {
        anyhow::bail!("Found {} occurrences. Add more context or set replace_all=true.", count);
    }
    let first = matches[0];
    let new_content = if replace_all {
        re.replace_all(content, |caps: &regex::Captures| {
            let newline = caps.get(3).map_or("", |m| m.as_str());
            format!("{}{}", new_string, newline)
        })
        .into_owned()
    } else {
        re.replacen(content, 1, |caps: &regex::Captures| {
            let newline = caps.get(3).map_or("", |m| m.as_str());
            format!("{}{}", new_string, newline)
        })
        .into_owned()
    };
    Ok(FuzzyReplaceResult {
        match_type: "exact".to_string(),
        total_occurrences: count,
        replaced_count: if replace_all { count } else { 1 },
        first_match_start: first.start(),
        first_match_len: first.end() - first.start(),
        new_content,
    })
}

/// 在指定行后插入内容，支持 auto-indent
pub fn insert_lines_at(
    content: &str,
    line_num: usize,
    insert_content: &str,
) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if line_num > total {
        anyhow::bail!("Line {} is beyond end of file ({} lines)", line_num, total);
    }
    let offsets = line_byte_offsets(content);
    let insert_at = if line_num == 0 {
        0
    } else {
        offsets.get(line_num).copied().unwrap_or(content.len())
    };
    let needs_preceding_newline = line_num > 0
        && insert_at == content.len()
        && !content.is_empty()
        && !content.ends_with('\n');
    let indented = auto_indent(insert_content, &lines, line_num);
    let effective = indented.as_deref().unwrap_or(insert_content);
    let with_newline = if effective.ends_with('\n') {
        effective.to_string()
    } else {
        format!("{}\n", effective)
    };
    let new_content = if needs_preceding_newline {
        format!("{}\n{}{}", &content[..insert_at], with_newline, &content[insert_at..])
    } else {
        format!("{}{}{}", &content[..insert_at], with_newline, &content[insert_at..])
    };
    Ok(new_content)
}

/// 行首字节偏移
pub fn line_byte_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// 安全截取片段（避免在字符中间切断）
pub fn safe_excerpt(content: &str, start: usize, span_len: usize, max_len: usize) -> String {
    let prefix = 80usize;
    let suffix = 80usize;
    let begin = floor_char_boundary(content, start.saturating_sub(prefix));
    let end = ceil_char_boundary(content, (start + span_len + suffix).min(content.len()));
    let mut excerpt = content[begin..end].replace('\n', "\\n");
    if excerpt.len() > max_len {
        let safe_len = floor_char_boundary(&excerpt, max_len);
        excerpt.truncate(safe_len);
        excerpt.push_str("...");
    }
    excerpt
}

fn floor_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

pub fn build_failure_hint(content: &str, old_string: &str) -> String {
    let old_lines: Vec<&str> = old_string.lines().collect();
    if old_lines.is_empty() || content.is_empty() {
        return "File is empty or old_string is empty.".to_string();
    }
    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.is_empty() {
        return "File is empty.".to_string();
    }
    let mut best_score = 0.0_f64;
    let mut best_pos = 0_usize;
    let window = old_lines.len().min(content_lines.len());
    for i in 0..=(content_lines.len().saturating_sub(window)) {
        let mut total = 0.0;
        for j in 0..window {
            total += levenshtein_similarity(
                old_lines.get(j).unwrap_or(&"").trim(),
                content_lines[i + j].trim(),
            );
        }
        let avg = total / window as f64;
        if avg > best_score {
            best_score = avg;
            best_pos = i;
        }
    }
    let ctx = 5;
    let start = best_pos.saturating_sub(ctx);
    let end = (best_pos + window + ctx).min(content_lines.len());
    let mut hint = format!(
        "Closest match found at lines {}-{} (similarity: {:.2}):\n",
        best_pos + 1,
        best_pos + window,
        best_score
    );
    for i in start..end {
        hint.push_str(&format!("{:>6}|{}\n", i + 1, content_lines[i]));
    }
    hint
}

fn auto_indent(content: &str, lines: &[&str], after_line: usize) -> Option<String> {
    let ref_line = if after_line < lines.len() {
        lines[after_line]
    } else if after_line > 0 {
        lines[after_line - 1]
    } else if !lines.is_empty() {
        lines[0]
    } else {
        return None;
    };
    let indent = detect_indentation(ref_line);
    if indent.is_empty() {
        return None;
    }
    let has_indent = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .any(|l| l.starts_with(' ') || l.starts_with('\t'));
    if has_indent {
        return None;
    }
    let indented: Vec<String> = content
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else {
                format!("{}{}", indent, l)
            }
        })
        .collect();
    Some(indented.join("\n"))
}

fn detect_indentation(line: &str) -> &str {
    let trimmed_len = line.trim_start().len();
    &line[..line.len().saturating_sub(trimmed_len)]
}

// ─── Fuzzy match ───────────────────────────────────────────────────────────

struct FuzzyMatch {
    start: usize,
    end: usize,
    match_type: String,
}

const FUZZY_THRESHOLD: f64 = 0.85;

fn fuzzy_find(content: &str, old_string: &str) -> Option<FuzzyMatch> {
    fuzzy_find_whitespace(content, old_string)
        .or_else(|| fuzzy_find_blank_lines(content, old_string))
        .or_else(|| {
            let threshold = std::env::var("SKILLLITE_FUZZY_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(FUZZY_THRESHOLD);
            fuzzy_find_similarity(content, old_string, threshold)
        })
}

fn fuzzy_find_whitespace(content: &str, old_string: &str) -> Option<FuzzyMatch> {
    let old_lines: Vec<&str> = old_string.lines().collect();
    if old_lines.is_empty() {
        return None;
    }
    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.len() < old_lines.len() {
        return None;
    }
    let trimmed_old: Vec<&str> = old_lines.iter().map(|l| l.trim()).collect();
    if trimmed_old.iter().all(|l| l.is_empty()) {
        return None;
    }
    let offsets = line_byte_offsets(content);
    for i in 0..=(content_lines.len() - old_lines.len()) {
        let all_match = (0..old_lines.len())
            .all(|j| content_lines[i + j].trim() == trimmed_old[j]);
        if all_match {
            let start = offsets[i];
            let end = fuzzy_match_end(content, &offsets, &content_lines, i, old_lines.len(), old_string.ends_with('\n'));
            return Some(FuzzyMatch {
                start,
                end,
                match_type: "whitespace_fuzzy".to_string(),
            });
        }
    }
    None
}

fn fuzzy_find_blank_lines(content: &str, old_string: &str) -> Option<FuzzyMatch> {
    let old_non_blank: Vec<&str> = old_string.lines().filter(|l| !l.trim().is_empty()).collect();
    if old_non_blank.is_empty() {
        return None;
    }
    let content_lines: Vec<&str> = content.lines().collect();
    let offsets = line_byte_offsets(content);
    for start_line in 0..content_lines.len() {
        if content_lines[start_line].trim().is_empty() {
            continue;
        }
        let mut old_idx = 0;
        let mut last_matched = start_line;
        for i in start_line..content_lines.len() {
            if content_lines[i].trim().is_empty() {
                continue;
            }
            if old_idx < old_non_blank.len() && content_lines[i] == old_non_blank[old_idx] {
                old_idx += 1;
                last_matched = i;
            } else {
                break;
            }
        }
        if old_idx == old_non_blank.len() {
            let start = offsets[start_line];
            let end = fuzzy_match_end(content, &offsets, &content_lines, last_matched, 1, old_string.ends_with('\n'));
            return Some(FuzzyMatch {
                start,
                end,
                match_type: "blank_line_fuzzy".to_string(),
            });
        }
    }
    None
}

fn fuzzy_find_similarity(content: &str, old_string: &str, threshold: f64) -> Option<FuzzyMatch> {
    let old_lines: Vec<&str> = old_string.lines().collect();
    if old_lines.is_empty() {
        return None;
    }
    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.len() < old_lines.len() {
        return None;
    }
    let offsets = line_byte_offsets(content);
    let mut best_score = 0.0_f64;
    let mut best_pos = 0_usize;
    for i in 0..=(content_lines.len() - old_lines.len()) {
        let mut total = 0.0;
        for j in 0..old_lines.len() {
            total += levenshtein_similarity(
                old_lines[j].trim(),
                content_lines[i + j].trim(),
            );
        }
        let avg = total / old_lines.len() as f64;
        if avg > best_score {
            best_score = avg;
            best_pos = i;
        }
    }
    if best_score >= threshold {
        let start = offsets[best_pos];
        let end = fuzzy_match_end(content, &offsets, &content_lines, best_pos, old_lines.len(), old_string.ends_with('\n'));
        Some(FuzzyMatch {
            start,
            end,
            match_type: format!("similarity({:.2})", best_score),
        })
    } else {
        None
    }
}

fn fuzzy_match_end(
    content: &str,
    offsets: &[usize],
    content_lines: &[&str],
    start_line: usize,
    num_lines: usize,
    old_ends_with_newline: bool,
) -> usize {
    let end_line_idx = start_line + num_lines;
    if old_ends_with_newline {
        offsets.get(end_line_idx).copied().unwrap_or(content.len())
    } else {
        let last = start_line + num_lines - 1;
        (offsets[last] + content_lines[last].len()).min(content.len())
    }
}

fn levenshtein_similarity(a: &str, b: &str) -> f64 {
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - levenshtein_distance(a, b) as f64 / max_len as f64
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (a_len, b_len) = (a.len(), b.len());
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];
    for i in 0..a_len {
        curr[0] = i + 1;
        for j in 0..b_len {
            let cost = if a[i] == b[j] { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(curr[j] + 1).min(prev[j + 1] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}
