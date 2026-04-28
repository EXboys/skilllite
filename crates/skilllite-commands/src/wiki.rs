//! Markdown-only project Repo Wiki commands.

use anyhow::Context;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;

const WIKI_DIRS: &[&str] = &["raw", "wiki", "lessons", "output"];

pub fn cmd_wiki_init(workspace: &str) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;
    rebuild_indexes(&wiki)?;
    append_log(&wiki, "init", "Initialized or repaired Repo Wiki structure")?;
    println!("Repo Wiki ready at {}", wiki.display());
    Ok(())
}

pub fn cmd_wiki_ingest(workspace: &str, source: &Path) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;

    if !source.is_file() {
        crate::error::bail!("source must be a local file: {}", source.display());
    }

    let content = fs::read_to_string(source)
        .with_context(|| format!("Failed to read source file: {}", source.display()))?;
    let title = source
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("source");
    let slug = unique_raw_slug(&wiki, title);
    let dest = wiki.join("raw").join(format!("{slug}.md"));
    let now = Utc::now().format("%Y-%m-%d").to_string();
    let source_display = source.to_string_lossy();
    let body = if source.extension().and_then(|e| e.to_str()) == Some("md") {
        content
    } else {
        format!("```text\n{}\n```\n", content.trim_end())
    };
    let doc = format!(
        "---\ntitle: \"{}\"\nsource: \"{}\"\ntype: notes\ningested: {}\ntags: []\nsummary: \"\"\n---\n\n# {}\n\n{}\n",
        yaml_escape(title),
        yaml_escape(&source_display),
        now,
        title,
        body.trim_end()
    );
    fs::write(&dest, doc).with_context(|| format!("Failed to write {}", dest.display()))?;

    rebuild_indexes(&wiki)?;
    append_log(
        &wiki,
        "ingest",
        &format!("Ingested `{}` into `raw/{slug}.md`", source.display()),
    )?;
    println!("Ingested {} -> {}", source.display(), dest.display());
    Ok(())
}

pub fn cmd_wiki_query(workspace: &str, question: &str) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;
    let hits = query_wiki(&wiki, question)?;
    if hits.is_empty() {
        println!(
            "No Repo Wiki matches for '{}'. Add sources with `skilllite wiki ingest <path>` or update wiki articles.",
            question
        );
        return Ok(());
    }

    println!("Repo Wiki matches for '{}':\n", question);
    for (idx, hit) in hits.iter().enumerate() {
        println!(
            "{}. {} (score {})\n{}\n",
            idx + 1,
            hit.path,
            hit.score,
            hit.snippet
        );
    }
    Ok(())
}

pub fn cmd_wiki_lint(workspace: &str) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = skilllite_core::paths::project_wiki_root(&root);
    let findings = lint_wiki(&wiki)?;
    if findings.is_empty() {
        println!("Repo Wiki lint passed: {}", wiki.display());
        return Ok(());
    }

    println!("Repo Wiki lint found {} issue(s):", findings.len());
    for finding in findings {
        println!("- {finding}");
    }
    crate::error::bail!("Repo Wiki lint failed");
}

fn workspace_root(workspace: &str) -> PathBuf {
    skilllite_core::paths::resolve_workspace_filesystem_root(workspace)
}

fn ensure_wiki(workspace: &Path) -> Result<PathBuf> {
    crate::init::ensure_project_wiki(workspace)
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if path.exists() {
        let existing = fs::read_to_string(path).unwrap_or_default();
        if existing == content {
            return Ok(());
        }
    }
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn rebuild_indexes(wiki: &Path) -> Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    for dir in WIKI_DIRS {
        fs::create_dir_all(wiki.join(dir))
            .with_context(|| format!("Failed to create {}", wiki.join(dir).display()))?;
        let rows = index_rows(wiki, dir)?;
        write_if_changed(
            &wiki.join(dir).join("_index.md"),
            &format!(
                "# {} Index\n\nLast updated: {}\n\n## Contents\n\n{}\n",
                title_case(dir),
                today,
                rows
            ),
        )?;
    }

    let mut root_rows = Vec::new();
    for dir in WIKI_DIRS {
        let count = markdown_files(&wiki.join(dir))?
            .into_iter()
            .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .count();
        root_rows.push(format!(
            "| [{}]({}/_index.md) | {} Markdown file(s) |",
            title_case(dir),
            dir,
            count
        ));
    }
    write_if_changed(
        &wiki.join("_index.md"),
        &format!(
            "# SkillLite Repo Wiki\n\nProject-local Markdown knowledge base for this repository.\n\nLast updated: {}\n\n## Contents\n\n| Section | Summary |\n|---|---|\n{}\n",
            today,
            root_rows.join("\n")
        ),
    )?;
    Ok(())
}

fn index_rows(wiki: &Path, dir: &str) -> Result<String> {
    let mut rows = Vec::new();
    for path in markdown_files(&wiki.join(dir))? {
        if path.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
            continue;
        }
        let rel = path
            .strip_prefix(wiki.join(dir))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        let content = fs::read_to_string(&path).unwrap_or_default();
        let title = frontmatter_value(&content, "title").unwrap_or_else(|| rel.clone());
        let summary = frontmatter_value(&content, "summary").unwrap_or_default();
        rows.push(format!("| [{}]({}) | {} |", title, rel, summary));
    }
    if rows.is_empty() {
        Ok("| File | Summary |\n|---|---|\n| - | No files yet. |".to_string())
    } else {
        Ok(format!(
            "| File | Summary |\n|---|---|\n{}",
            rows.join("\n")
        ))
    }
}

fn markdown_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(markdown_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

#[derive(Debug, Clone)]
struct QueryHit {
    path: String,
    score: usize,
    snippet: String,
}

fn query_wiki(wiki: &Path, question: &str) -> Result<Vec<QueryHit>> {
    let terms = query_terms(question);
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let mut hits = Vec::new();
    for dir in ["wiki", "lessons", "raw"] {
        for path in markdown_files(&wiki.join(dir))? {
            if path.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            let lower = content.to_lowercase();
            let score = terms.iter().map(|t| lower.matches(t).count()).sum();
            if score == 0 {
                continue;
            }
            let rel = path
                .strip_prefix(wiki)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            hits.push(QueryHit {
                path: rel,
                score,
                snippet: best_snippet(&content, &terms),
            });
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    hits.truncate(5);
    Ok(hits)
}

fn query_terms(question: &str) -> Vec<String> {
    let lower = question.to_lowercase();
    let mut terms: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() >= 2)
        .map(str::to_string)
        .collect();
    if terms.is_empty() {
        let trimmed = lower.trim();
        if !trimmed.is_empty() {
            terms.push(trimmed.to_string());
        }
    }
    terms.sort();
    terms.dedup();
    terms
}

fn best_snippet(content: &str, terms: &[String]) -> String {
    let mut best = "";
    let mut best_score = 0;
    for line in content.lines() {
        let lower = line.to_lowercase();
        let score: usize = terms.iter().map(|t| lower.matches(t).count()).sum();
        if score > best_score {
            best = line;
            best_score = score;
        }
    }
    let line = if best.trim().is_empty() {
        content.lines().find(|l| !l.trim().is_empty()).unwrap_or("")
    } else {
        best
    };
    line.chars().take(240).collect()
}

fn lint_wiki(wiki: &Path) -> Result<Vec<String>> {
    let mut findings = Vec::new();
    if !wiki.exists() {
        findings.push(format!("missing wiki root: {}", wiki.display()));
        return Ok(findings);
    }
    for file in ["_index.md", "config.md", "log.md"] {
        if !wiki.join(file).is_file() {
            findings.push(format!("missing required file: {file}"));
        }
    }
    for dir in WIKI_DIRS {
        let path = wiki.join(dir);
        if !path.is_dir() {
            findings.push(format!("missing required directory: {dir}/"));
            continue;
        }
        if !path.join("_index.md").is_file() {
            findings.push(format!("missing section index: {dir}/_index.md"));
        }
        for md in markdown_files(&path)? {
            if md.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
                continue;
            }
            let content = fs::read_to_string(&md).unwrap_or_default();
            if !has_frontmatter(&content) {
                findings.push(format!(
                    "missing YAML frontmatter: {}",
                    md.strip_prefix(wiki)
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_else(|_| md.to_string_lossy().to_string())
                ));
            }
        }
    }
    Ok(findings)
}

fn append_log(wiki: &Path, op: &str, description: &str) -> Result<()> {
    let path = wiki.join("log.md");
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let mut existing =
        fs::read_to_string(&path).unwrap_or_else(|_| "# Repo Wiki Log\n\n".to_string());
    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&format!("\n## [{date}] {op} | {description}\n"));
    fs::write(&path, existing).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn unique_raw_slug(wiki: &Path, title: &str) -> String {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let base = format!("{}-{}", date, slugify(title));
    let mut candidate = base.clone();
    let mut i = 2;
    while wiki.join("raw").join(format!("{candidate}.md")).exists() {
        candidate = format!("{base}-{i}");
        i += 1;
    }
    candidate
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "source".to_string()
    } else {
        trimmed.to_string()
    }
}

fn yaml_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn has_frontmatter(content: &str) -> bool {
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return false;
    }
    lines.any(|line| line == "---")
}

fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    if !has_frontmatter(content) {
        return None;
    }
    let prefix = format!("{key}:");
    for line in content.lines().skip(1) {
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn title_case(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_ingest_query_and_lint_happy_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let source = workspace.join("note.md");
        fs::write(
            &source,
            "# Architecture\n\nThe payment adapter uses retry backoff.",
        )
        .expect("write source");

        cmd_wiki_init(workspace.to_str().expect("utf8 path")).expect("init wiki");
        cmd_wiki_ingest(workspace.to_str().expect("utf8 path"), &source).expect("ingest");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let hits = query_wiki(&wiki, "payment retry").expect("query");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.starts_with("raw/"));
        assert!(hits[0].snippet.contains("payment adapter"));

        let findings = lint_wiki(&wiki).expect("lint");
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn lint_reports_missing_frontmatter() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let wiki = ensure_wiki(workspace).expect("wiki");
        rebuild_indexes(&wiki).expect("indexes");
        let bad = wiki.join("wiki").join("bad.md");
        fs::write(&bad, "# Missing frontmatter\n").expect("write bad");

        let findings = lint_wiki(&wiki).expect("lint");

        assert!(findings
            .iter()
            .any(|f| f.contains("missing YAML frontmatter: wiki/bad.md")));
    }
}
