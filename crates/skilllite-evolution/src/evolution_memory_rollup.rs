//! 按月分卷的去重汇总（`YYYY-MM.rollup.md`）：从原始分卷解析条目、合并重复键，供阅读与抽取去重摘要。

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::bail;
use crate::gatekeeper_l1_path;
use crate::gatekeeper_l3_content;
use crate::Result;

/// 五维子目录名，与 `memory_learner` 一致。
const ROLLUP_CATEGORY_DIRS: &[&str] = &[
    "entities",
    "relations",
    "episodes",
    "preferences",
    "patterns",
];

fn month_key_valid(month: &str) -> bool {
    let b = month.as_bytes();
    if b.len() != 7 || b[4] != b'-' {
        return false;
    }
    for (i, ch) in month.chars().enumerate() {
        if i == 4 {
            if ch != '-' {
                return false;
            }
        } else if !ch.is_ascii_digit() {
            return false;
        }
    }
    true
}

fn category_title_zh(dir_name: &str) -> &'static str {
    match dir_name {
        "entities" => "实体",
        "relations" => "关系",
        "episodes" => "情节",
        "preferences" => "倾向",
        "patterns" => "模式",
        _ => "其他",
    }
}

/// 从按月分卷正文收集「运行块」内的行（`## ...` 标题之后、下一个一级/二级标题之前）。
fn for_each_run_line(content: &str, mut f: impl FnMut(&str)) {
    let mut in_run = false;
    for line in content.lines() {
        if line.starts_with("## ") && !line.starts_with("###") {
            in_run = true;
            continue;
        }
        if line.starts_with('#') && !line.starts_with("##") {
            in_run = false;
            continue;
        }
        if in_run {
            f(line);
        }
    }
}

fn parse_entity_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("- **")?;
    let (name, after) = rest.split_once("**")?;
    let after = after.trim_start().strip_prefix('(')?;
    let (etype, note_with_paren) = after.split_once(')')?;
    let note = note_with_paren.trim_start();
    Some((
        name.trim().to_string(),
        etype.trim().to_string(),
        note.to_string(),
    ))
}

fn parse_relation_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("- ")?;
    let pos = rest.find('→')?;
    let from = rest[..pos].trim();
    let rest = rest[pos + '→'.len_utf8()..].trim_start();
    let colon_pos = rest.find(':')?;
    let to = rest[..colon_pos].trim();
    let relation = rest[colon_pos + 1..].trim();
    Some((from.to_string(), to.to_string(), relation.to_string()))
}

fn parse_episode_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("- [")?;
    let (outcome, rest) = rest.split_once(']')?;
    let rest = rest.trim_start();
    let marker = "→ 教训：";
    let idx = rest.find(marker)?;
    let summary = rest[..idx].trim();
    let lesson = rest[idx + marker.len()..].trim();
    Some((
        outcome.trim().to_string(),
        summary.to_string(),
        lesson.to_string(),
    ))
}

fn parse_preference_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("- ")?;
    let key = "（情境：";
    let idx = rest.rfind(key)?;
    let desc = rest[..idx].trim_end();
    let inner = &rest[idx + key.len()..];
    let ctx = inner.strip_suffix('）')?;
    Some((desc.to_string(), ctx.to_string()))
}

fn parse_pattern_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("- ")?;
    if rest.contains("（情境：") {
        return None;
    }
    let open = rest.rfind('（')?;
    let desc = rest[..open].trim_end();
    let inner = &rest[open + '（'.len_utf8()..];
    let ev = inner.strip_suffix('）')?;
    Some((desc.to_string(), ev.to_string()))
}

fn merge_note_chosen(existing: &str, incoming: &str) -> String {
    let a = existing.trim();
    let b = incoming.trim();
    if b.len() > a.len() {
        return b.to_string();
    }
    if a.len() > b.len() {
        return a.to_string();
    }
    b.to_string()
}

fn rollup_entities(content: &str) -> Vec<String> {
    #[derive(Clone)]
    struct Entry {
        name: String,
        etype: String,
        note: String,
    }
    type Key = (String, String);
    let mut map: BTreeMap<Key, Entry> = BTreeMap::new();
    for_each_run_line(content, |line| {
        if let Some((name, etype, note)) = parse_entity_line(line) {
            let key = (name.trim().to_lowercase(), etype.trim().to_lowercase());
            let incoming = note.trim();
            map.entry(key)
                .and_modify(|e| {
                    let cur = e.note.trim();
                    if incoming.len() > cur.len()
                        || (incoming.len() == cur.len() && incoming != cur)
                    {
                        e.name = name.clone();
                        e.etype = etype.clone();
                        e.note = note.clone();
                    }
                })
                .or_insert(Entry { name, etype, note });
        }
    });
    map.into_values()
        .map(|e| format!("- **{}** ({}) {}", e.name, e.etype, e.note))
        .collect()
}

fn rollup_relations(content: &str) -> Vec<String> {
    type Key = (String, String, String);
    let mut map: BTreeMap<Key, String> = BTreeMap::new();
    for_each_run_line(content, |line| {
        if let Some((from, to, relation)) = parse_relation_line(line) {
            let key = (
                from.trim().to_lowercase(),
                to.trim().to_lowercase(),
                relation.trim().to_lowercase(),
            );
            map.entry(key).or_insert_with(|| {
                format!("- {} → {}: {}", from.trim(), to.trim(), relation.trim())
            });
        }
    });
    map.into_values().collect()
}

fn rollup_episodes(content: &str) -> Vec<String> {
    type Key = (String, String);
    let mut map: BTreeMap<Key, (String, String, String)> = BTreeMap::new();
    for_each_run_line(content, |line| {
        if let Some((outcome, summary, lesson)) = parse_episode_line(line) {
            let key = (outcome.trim().to_lowercase(), summary.trim().to_lowercase());
            map.entry(key)
                .and_modify(|(_, _, l)| {
                    *l = merge_note_chosen(l, &lesson);
                })
                .or_insert((outcome, summary, lesson));
        }
    });
    map.into_values()
        .map(|(o, s, l)| format!("- [{}] {} → 教训：{}", o, s, l))
        .collect()
}

fn rollup_preferences(content: &str) -> Vec<String> {
    let mut map: BTreeMap<String, (String, String)> = BTreeMap::new();
    for_each_run_line(content, |line| {
        if let Some((desc, ctx)) = parse_preference_line(line) {
            let key = desc.trim().to_lowercase();
            map.entry(key)
                .and_modify(|(_, c)| {
                    *c = merge_note_chosen(c, &ctx);
                })
                .or_insert((desc, ctx));
        }
    });
    map.into_values()
        .map(|(d, c)| format!("- {}（情境：{}）", d, c))
        .collect()
}

fn rollup_patterns(content: &str) -> Vec<String> {
    let mut map: BTreeMap<String, (String, String)> = BTreeMap::new();
    for_each_run_line(content, |line| {
        if let Some((desc, ev)) = parse_pattern_line(line) {
            let key = desc.trim().to_lowercase();
            map.entry(key)
                .and_modify(|(_, e)| {
                    *e = merge_note_chosen(e, &ev);
                })
                .or_insert((desc, ev));
        }
    });
    map.into_values()
        .map(|(d, e)| format!("- {}（{}）", d, e))
        .collect()
}

fn build_rollup_body(dir_name: &str, content: &str) -> String {
    let lines: Vec<String> = match dir_name {
        "entities" => rollup_entities(content),
        "relations" => rollup_relations(content),
        "episodes" => rollup_episodes(content),
        "preferences" => rollup_preferences(content),
        "patterns" => rollup_patterns(content),
        _ => Vec::new(),
    };
    lines.join("\n")
}

/// 根据 `memory/evolution/<dir>/<month>.md` 重算同目录下的 `<month>.rollup.md`。
/// 若分卷不存在则删除已陈旧的 rollup 文件。
pub fn rebuild_rollups_for_month(chat_root: &Path, month: &str) -> Result<()> {
    if !month_key_valid(month) {
        bail!("Invalid evolution month key (expected YYYY-MM): {}", month);
    }
    let evolution = chat_root.join("memory").join("evolution");
    let generated = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

    for dir_name in ROLLUP_CATEGORY_DIRS {
        let shard_dir = evolution.join(dir_name);
        let shard = shard_dir.join(format!("{month}.md"));
        let rollup_path = shard_dir.join(format!("{month}.rollup.md"));

        if !gatekeeper_l1_path(chat_root, &rollup_path, None) {
            bail!(
                "Path escapes allowed evolution memory tree: {}",
                rollup_path.display()
            );
        }

        if !shard.exists() {
            if rollup_path.exists() {
                skilllite_fs::remove_file(&rollup_path)?;
            }
            continue;
        }

        let raw = skilllite_fs::read_file(&shard).unwrap_or_default();
        let body = build_rollup_body(dir_name, &raw);
        let zh = category_title_zh(dir_name);
        let doc = format!(
            "# {zh} — {month}（当月去重汇总）\n\n\
             由 Memory 进化根据 [`{dir_name}/{month}.md`]({month}.md) 自动合并重复条目；**按次原始记录**仍在分卷正文中。\n\n\
             **生成时间**: {generated}\n\n\
             ---\n\n\
             {body}\n",
            zh = zh,
            month = month,
            dir_name = dir_name,
            generated = generated,
            body = body.trim_end(),
        );
        gatekeeper_l3_content(&doc)?;
        skilllite_fs::create_dir_all(&shard_dir)?;
        skilllite_fs::write_file(&rollup_path, &doc)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ENTITIES: &str = r#"# 实体 — 2026-04

由 Memory 进化自动写入；本分卷位于 `entities/`。

---

## 2026-04-04 01:00 UTC

- **read_file** (工具) 用于读取文件。
- **read_file** (工具) 用于读取文件内容，常用于代码。

---

## 2026-04-04 02:00 UTC

- **深圳** (概念) 城市名。
"#;

    #[test]
    fn rollup_entities_dedups_by_name_and_type_prefers_longer_note() {
        let lines = rollup_entities(SAMPLE_ENTITIES);
        assert_eq!(lines.len(), 2);
        assert!(lines
            .iter()
            .any(|l| l.contains("read_file") && l.contains("常用于代码")));
        assert!(lines.iter().any(|l| l.contains("深圳")));
    }

    #[test]
    fn parse_entity_line_handles_unicode_note() {
        let (n, t, note) =
            parse_entity_line("- **工具甲** (概念) 说明里有中文与 emoji 🌧").expect("parse");
        assert_eq!(n, "工具甲");
        assert_eq!(t, "概念");
        assert!(note.contains("emoji"));
    }

    #[test]
    fn rollup_relations_dedups() {
        let md = r"## A
- a → b: x
---
## B
- a → b: x
";
        let lines = rollup_relations(md);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn month_key_valid_rejects_bad() {
        assert!(month_key_valid("2026-04"));
        assert!(!month_key_valid("2026-4"));
        assert!(!month_key_valid("../x"));
    }

    #[test]
    fn rebuild_rollups_for_month_writes_entities_rollup_under_chat_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let entities = root.join("memory").join("evolution").join("entities");
        std::fs::create_dir_all(&entities).expect("mkdir");
        std::fs::write(entities.join("2026-04.md"), SAMPLE_ENTITIES).expect("write shard");
        rebuild_rollups_for_month(root, "2026-04").expect("rollup");
        let rollup =
            std::fs::read_to_string(entities.join("2026-04.rollup.md")).expect("read rollup");
        assert!(
            rollup.contains("当月去重汇总"),
            "rollup header missing: {:.120}",
            rollup
        );
        assert!(
            rollup.contains("read_file"),
            "expected dedup entity retained"
        );
    }

    #[test]
    fn rebuild_rollups_for_month_invalid_month_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = rebuild_rollups_for_month(tmp.path(), "bad").expect_err("expected invalid month");
        let msg = format!("{}", err);
        assert!(
            msg.contains("YYYY-MM") || msg.contains("Invalid"),
            "unexpected error: {}",
            msg
        );
    }
}
