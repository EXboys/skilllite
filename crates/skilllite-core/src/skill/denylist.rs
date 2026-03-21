//! Manual Skill denylist (P1): block execution by SKILL name before sandbox run.
//!
//! Sources (merged): `SKILLLITE_SKILL_DENYLIST` (comma-separated), `~/.skilllite/skill-denylist.txt`,
//! `{SKILLLITE_WORKSPACE or data_root}/.skilllite/skill-denylist.txt`, and `./.skilllite/skill-denylist.txt`.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::config::env_keys::observability;
use crate::config::loader::env_optional;
use crate::paths::data_root;

fn parse_denylist_text(content: &str) -> HashSet<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|s| s.to_string())
        .collect()
}

fn push_env_entries(out: &mut HashSet<String>) {
    if let Some(raw) = env_optional(observability::SKILLLITE_SKILL_DENYLIST, &[]) {
        for part in raw.split([',', ';']) {
            let s = part.trim();
            if !s.is_empty() {
                out.insert(s.to_string());
            }
        }
    }
}

fn read_file_if_exists(path: &std::path::Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// User-level file under data root: `~/.skilllite/skill-denylist.txt`
fn user_denylist_path() -> PathBuf {
    data_root().join("skill-denylist.txt")
}

/// Project-style file next to skills config: `{data_root}/.skilllite/skill-denylist.txt`
fn data_root_nested_denylist_path() -> PathBuf {
    data_root().join(".skilllite").join("skill-denylist.txt")
}

fn cwd_project_denylist_path() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|p| p.join(".skilllite").join("skill-denylist.txt"))
}

/// All denied SKILL names (from SKILL.md `name` / audit `skill_id`), merged from all sources.
pub fn load_denied_skill_names() -> HashSet<String> {
    let mut out = HashSet::new();
    push_env_entries(&mut out);

    if let Some(text) = read_file_if_exists(&user_denylist_path()) {
        out.extend(parse_denylist_text(&text));
    }
    if let Some(text) = read_file_if_exists(&data_root_nested_denylist_path()) {
        out.extend(parse_denylist_text(&text));
    }
    if let Some(p) = cwd_project_denylist_path() {
        if let Some(text) = read_file_if_exists(&p) {
            out.extend(parse_denylist_text(&text));
        }
    }
    out
}

/// If execution should be blocked, returns a human-readable reason (for errors/logs).
pub fn deny_reason_for_skill_name(name: &str) -> Option<String> {
    let denied = load_denied_skill_names();
    if denied.contains(name) {
        Some(format!(
            "Execution blocked: skill {:?} is on the denylist. \
Remove it from {}, {}, project .skilllite/skill-denylist.txt, or clear {}.",
            name,
            user_denylist_path().display(),
            data_root_nested_denylist_path().display(),
            observability::SKILLLITE_SKILL_DENYLIST
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skips_comments_and_blank() {
        let s = "  foo  \n# bar\nbaz\n";
        let h = parse_denylist_text(s);
        assert!(h.contains("foo"));
        assert!(h.contains("baz"));
        assert!(!h.contains("bar"));
    }
}
