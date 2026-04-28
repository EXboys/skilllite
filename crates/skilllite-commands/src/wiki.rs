//! Markdown-only project Repo Wiki commands.

use anyhow::Context;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;

const WIKI_DIRS: &[&str] = &["raw", "wiki", "lessons", "output"];
const ARTICLE_REQUIRED_FRONTMATTER: &[&str] = &[
    "title",
    "category",
    "sources",
    "created",
    "updated",
    "tags",
    "aliases",
    "confidence",
    "summary",
    "source_fingerprints",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WikiQueryDepth {
    Quick,
    Standard,
    Deep,
}

#[derive(Debug, Default)]
struct WikiFreshness {
    up_to_date: Vec<String>,
    stale: Vec<String>,
    uncompiled: Vec<String>,
    missing_sources: Vec<String>,
}

impl WikiFreshness {
    fn needs_refresh(&self) -> bool {
        !self.stale.is_empty() || !self.uncompiled.is_empty()
    }

    fn is_clean(&self) -> bool {
        self.stale.is_empty() && self.uncompiled.is_empty() && self.missing_sources.is_empty()
    }
}

pub fn cmd_wiki_init(workspace: &str) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;
    rebuild_indexes(&wiki)?;
    append_log(&wiki, "init", "Initialized or repaired Repo Wiki structure")?;
    println!("Repo Wiki ready at {}", wiki.display());
    Ok(())
}

pub fn cmd_wiki_ingest(workspace: &str, source: &Path, no_compile: bool) -> Result<()> {
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

    let compiled = if no_compile {
        rebuild_indexes(&wiki)?;
        0
    } else {
        let compiled = compile_raw_sources(&wiki)?;
        rebuild_indexes(&wiki)?;
        compiled
    };
    append_log(
        &wiki,
        "ingest",
        &format!(
            "Ingested `{}` into `raw/{slug}.md`{}",
            source.display(),
            if no_compile {
                ""
            } else {
                " and auto-compiled wiki articles"
            }
        ),
    )?;
    println!("Ingested {} -> {}", source.display(), dest.display());
    if !no_compile {
        println!("Auto-compiled {} raw source(s)", compiled);
    }
    Ok(())
}

pub fn cmd_wiki_compile(workspace: &str) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;
    let compiled = compile_raw_sources(&wiki)?;
    rebuild_indexes(&wiki)?;
    append_log(
        &wiki,
        "compile",
        &format!("Compiled {} raw source(s) into wiki articles", compiled),
    )?;
    println!(
        "Compiled {} raw source(s) into {}",
        compiled,
        wiki.join("wiki").display()
    );
    Ok(())
}

pub fn cmd_wiki_status(workspace: &str) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = skilllite_core::paths::project_wiki_root(&root);
    let freshness = wiki_freshness(&wiki)?;
    if !wiki.exists() {
        println!("Repo Wiki status: missing ({})", wiki.display());
        return Ok(());
    }

    if freshness.is_clean() {
        println!(
            "Repo Wiki status: up-to-date ({} source(s))",
            freshness.up_to_date.len()
        );
    } else {
        println!("Repo Wiki status: stale");
    }
    print_freshness(&freshness);
    Ok(())
}

pub fn cmd_wiki_record_lesson(
    workspace: &str,
    title: &str,
    trigger: &str,
    summary: &str,
    body: &str,
) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;
    let slug = unique_raw_slug(&wiki, title);
    let dest = wiki.join("raw").join(format!("{slug}.md"));
    let now = Utc::now().format("%Y-%m-%d").to_string();
    let body = lesson_body_with_template(summary, trigger, body);
    let doc = format!(
        "---\ntitle: \"{}\"\nsource: \"chat-confirmation\"\ntype: lesson\ntrigger: \"{}\"\ningested: {}\ntags: [agent, tools, repo-wiki]\nsummary: \"{}\"\n---\n\n# {}\n\n{}\n",
        yaml_escape(title),
        yaml_escape(trigger),
        now,
        yaml_escape(summary),
        title,
        body.trim()
    );
    fs::write(&dest, doc).with_context(|| format!("Failed to write {}", dest.display()))?;
    let compiled = compile_raw_sources(&wiki)?;
    rebuild_indexes(&wiki)?;
    append_log(
        &wiki,
        "record-lesson",
        &format!("Recorded chat lesson into `raw/{slug}.md` and compiled {compiled} source(s)"),
    )?;
    println!("Recorded lesson {} -> {}", title, dest.display());
    println!("Auto-compiled {} raw source(s)", compiled);
    Ok(())
}

pub fn cmd_wiki_query(
    workspace: &str,
    question: &str,
    depth: WikiQueryDepth,
    no_compile: bool,
) -> Result<()> {
    let root = workspace_root(workspace);
    let wiki = ensure_wiki(&root)?;
    if !no_compile {
        let freshness = wiki_freshness(&wiki)?;
        if freshness.needs_refresh() {
            let compiled = compile_raw_sources(&wiki)?;
            rebuild_indexes(&wiki)?;
            append_log(
                &wiki,
                "auto-compile",
                &format!("Refreshed {} raw source(s) before query", compiled),
            )?;
            println!(
                "Repo Wiki refreshed before query: {} raw source(s)",
                compiled
            );
        }
    }
    let hits = query_wiki(&wiki, question, depth)?;
    if hits.is_empty() {
        println!(
            "No Repo Wiki matches for '{}'. Add sources with `skilllite wiki ingest <path>` or update wiki articles.",
            question
        );
        return Ok(());
    }

    println!("Repo Wiki {:?} matches for '{}':\n", depth, question);
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

fn lesson_body_with_template(summary: &str, trigger: &str, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.contains("## What Happened")
        && trimmed.contains("## Root Cause")
        && trimmed.contains("## Optimization")
        && trimmed.contains("## Next Time")
    {
        return trimmed.to_string();
    }
    let what_happened = if trimmed.is_empty() {
        summary.trim()
    } else {
        trimmed
    };
    format!(
        "## What Happened\n\n{}\n\nTrigger: `{}`.\n\n## Root Cause\n\nDescribe the confirmed reason this run needed replanning or hit repeated tool failures.\n\n## Optimization\n\nDocument the improved approach that avoids repeating the same failed path.\n\n## Next Time\n\n- Check the relevant file path, command output, schema, or dependency first.\n- Change the approach before retrying the same tool call.\n- Keep this lesson updated after the successful fix is confirmed.\n",
        what_happened,
        trigger.trim()
    )
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

fn compile_raw_sources(wiki: &Path) -> Result<usize> {
    let mut compiled = 0;
    for raw in markdown_files(&wiki.join("raw"))? {
        if raw.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
            continue;
        }
        let raw_content = fs::read_to_string(&raw).unwrap_or_default();
        if raw_content.trim().is_empty() {
            continue;
        }
        let raw_rel = rel_path(wiki, &raw);
        let raw_fingerprint = content_fingerprint(&raw_content);
        let title = frontmatter_value(&raw_content, "title").unwrap_or_else(|| {
            raw.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("source")
                .to_string()
        });
        let summary = frontmatter_value(&raw_content, "summary")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| first_meaningful_line(&raw_content));
        let article = article_path_for_raw(wiki, &raw);
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let created = fs::read_to_string(&article)
            .ok()
            .and_then(|content| frontmatter_value(&content, "created"))
            .unwrap_or_else(|| today.clone());
        let excerpt = excerpt_without_frontmatter(&raw_content, 900);
        let doc = format!(
            "---\ntitle: \"{}\"\ncategory: reference\nsources: [{}]\ncreated: {}\nupdated: {}\ntags: []\naliases: []\nconfidence: medium\nsummary: \"{}\"\nsource_fingerprints: [{}={}]\n---\n\n# {}\n\n## Abstract\n\n{}\n\n## Source Notes\n\n{}\n\n## Sources\n\n- [{}](../{})\n",
            yaml_escape(&title),
            raw_rel,
            created,
            today,
            yaml_escape(&summary),
            raw_rel,
            raw_fingerprint,
            title,
            summary,
            excerpt,
            raw_rel,
            raw_rel
        );
        write_if_changed(&article, &doc)?;
        compiled += 1;
    }
    Ok(compiled)
}

fn wiki_freshness(wiki: &Path) -> Result<WikiFreshness> {
    let mut freshness = WikiFreshness::default();
    if !wiki.exists() {
        return Ok(freshness);
    }

    for raw in markdown_files(&wiki.join("raw"))? {
        if raw.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
            continue;
        }
        let raw_content = fs::read_to_string(&raw).unwrap_or_default();
        if raw_content.trim().is_empty() {
            continue;
        }
        let raw_rel = rel_path(wiki, &raw);
        let article = article_path_for_raw(wiki, &raw);
        if !article.is_file() {
            freshness.uncompiled.push(raw_rel);
            continue;
        }
        let article_content = fs::read_to_string(&article).unwrap_or_default();
        let fingerprints = frontmatter_list(&article_content, "source_fingerprints");
        let expected = format!("{}={}", raw_rel, content_fingerprint(&raw_content));
        if fingerprints.iter().any(|value| value == &expected) {
            freshness.up_to_date.push(raw_rel);
        } else {
            freshness.stale.push(raw_rel);
        }
    }

    for article in markdown_files(&wiki.join("wiki"))? {
        if article.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
            continue;
        }
        let article_content = fs::read_to_string(&article).unwrap_or_default();
        let article_rel = rel_path(wiki, &article);
        for source in frontmatter_list(&article_content, "sources") {
            if !wiki.join(&source).is_file() {
                freshness
                    .missing_sources
                    .push(format!("{} -> {}", article_rel, source));
            }
        }
    }

    freshness.up_to_date.sort();
    freshness.stale.sort();
    freshness.uncompiled.sort();
    freshness.missing_sources.sort();
    Ok(freshness)
}

fn print_freshness(freshness: &WikiFreshness) {
    for source in &freshness.uncompiled {
        println!("- uncompiled: {}", source);
    }
    for source in &freshness.stale {
        println!("- stale: {}", source);
    }
    for source in &freshness.missing_sources {
        println!("- missing source: {}", source);
    }
    for source in &freshness.up_to_date {
        println!("- up-to-date: {}", source);
    }
}

fn article_path_for_raw(wiki: &Path, raw: &Path) -> PathBuf {
    let article_slug = raw
        .file_stem()
        .and_then(|s| s.to_str())
        .map(slugify)
        .unwrap_or_else(|| "source".to_string());
    wiki.join("wiki").join(format!("{article_slug}.md"))
}

fn content_fingerprint(content: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
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

fn query_wiki(wiki: &Path, question: &str, depth: WikiQueryDepth) -> Result<Vec<QueryHit>> {
    let terms = query_terms(question);
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let mut hits = Vec::new();
    for path in query_scope_files(wiki, depth)? {
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
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    hits.truncate(5);
    Ok(hits)
}

fn query_scope_files(wiki: &Path, depth: WikiQueryDepth) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    match depth {
        WikiQueryDepth::Quick => {
            files.push(wiki.join("_index.md"));
            for dir in WIKI_DIRS {
                files.push(wiki.join(dir).join("_index.md"));
            }
        }
        WikiQueryDepth::Standard => {
            for dir in ["wiki", "lessons", "raw"] {
                files.extend(
                    markdown_files(&wiki.join(dir))?
                        .into_iter()
                        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md")),
                );
            }
        }
        WikiQueryDepth::Deep => {
            files = markdown_files(wiki)?;
        }
    }
    files.retain(|p| p.is_file());
    files.sort();
    files.dedup();
    Ok(files)
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
            let rel = rel_path(wiki, &md);
            if !has_frontmatter(&content) {
                findings.push(format!("missing YAML frontmatter: {}", rel));
                continue;
            }
            if *dir == "wiki" {
                validate_article_frontmatter(wiki, &rel, &content, &mut findings);
                validate_markdown_links(wiki, &rel, &content, &mut findings);
            } else if *dir == "lessons" {
                validate_markdown_links(wiki, &rel, &content, &mut findings);
            }
        }
    }
    Ok(findings)
}

fn validate_article_frontmatter(wiki: &Path, rel: &str, content: &str, findings: &mut Vec<String>) {
    for key in ARTICLE_REQUIRED_FRONTMATTER {
        if frontmatter_value(content, key).is_none() {
            findings.push(format!("missing article frontmatter `{}`: {}", key, rel));
        }
    }
    let sources = frontmatter_list(content, "sources");
    if sources.is_empty() {
        findings.push(format!("article has no sources: {}", rel));
    }
    for source in sources {
        if !wiki.join(&source).is_file() {
            findings.push(format!("dangling article source `{}` in {}", source, rel));
        }
    }
}

fn validate_markdown_links(wiki: &Path, rel: &str, content: &str, findings: &mut Vec<String>) {
    let base = wiki
        .join(rel)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| wiki.to_path_buf());
    for target in markdown_link_targets(content) {
        if target.starts_with("http://")
            || target.starts_with("https://")
            || target.starts_with("mailto:")
            || target.starts_with('#')
        {
            continue;
        }
        let clean = target.split('#').next().unwrap_or("").trim();
        if clean.is_empty() || !clean.ends_with(".md") {
            continue;
        }
        if !base.join(clean).is_file() {
            findings.push(format!("dangling markdown link `{}` in {}", target, rel));
        }
    }
}

fn markdown_link_targets(content: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = content;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        if let Some(close) = after.find(')') {
            targets.push(after[..close].trim().to_string());
            rest = &after[close + 1..];
        } else {
            break;
        }
    }
    targets
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

fn frontmatter_list(content: &str, key: &str) -> Vec<String> {
    if !has_frontmatter(content) {
        return Vec::new();
    }
    let prefix = format!("{key}:");
    let mut values = Vec::new();
    let mut in_list = false;
    for line in content.lines().skip(1) {
        if line == "---" {
            break;
        }
        if in_list {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("- ") {
                values.push(value.trim().trim_matches('"').to_string());
                continue;
            }
            break;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                in_list = true;
            } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
                values.extend(
                    trimmed[1..trimmed.len().saturating_sub(1)]
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string()),
                );
            }
        }
    }
    values.into_iter().filter(|s| !s.is_empty()).collect()
}

fn first_meaningful_line(content: &str) -> String {
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty()
            || t == "---"
            || t.starts_with("title:")
            || t.starts_with("source:")
            || t.starts_with("type:")
            || t.starts_with("ingested:")
            || t.starts_with("tags:")
            || t.starts_with("summary:")
            || t.starts_with('#')
        {
            continue;
        }
        return t.chars().take(160).collect();
    }
    "Compiled from raw source.".to_string()
}

fn excerpt_without_frontmatter(content: &str, max_chars: usize) -> String {
    let body = if has_frontmatter(content) {
        let mut seen_close = false;
        let mut out = Vec::new();
        for line in content.lines().skip(1) {
            if !seen_close {
                if line == "---" {
                    seen_close = true;
                }
                continue;
            }
            out.push(line);
        }
        out.join("\n")
    } else {
        content.to_string()
    };
    body.trim().chars().take(max_chars).collect()
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
        cmd_wiki_ingest(workspace.to_str().expect("utf8 path"), &source, true).expect("ingest");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let hits = query_wiki(&wiki, "payment retry", WikiQueryDepth::Standard).expect("query");
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

    #[test]
    fn wiki_compile_creates_article_with_sources_and_query_modes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let source = workspace.join("adapter.md");
        fs::write(
            &source,
            "# Payment Adapter\n\nPayment retry backoff handles transient failures.",
        )
        .expect("write source");

        cmd_wiki_ingest(workspace.to_str().expect("utf8 path"), &source, true).expect("ingest");
        cmd_wiki_compile(workspace.to_str().expect("utf8 path")).expect("compile");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let article = markdown_files(&wiki.join("wiki"))
            .expect("wiki files")
            .into_iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .expect("compiled article");
        let content = fs::read_to_string(&article).expect("read article");
        assert!(content.contains("category: reference"));
        assert!(content.contains("sources: [raw/"));
        assert!(content.contains("confidence: medium"));

        let quick = query_wiki(&wiki, "payment", WikiQueryDepth::Quick).expect("quick query");
        assert!(!quick.is_empty());
        assert!(quick.iter().any(|h| h.path.ends_with("_index.md")));

        let standard =
            query_wiki(&wiki, "transient failures", WikiQueryDepth::Standard).expect("query");
        assert!(standard.iter().any(|h| h.path.starts_with("wiki/")));

        let findings = lint_wiki(&wiki).expect("lint");
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn ingest_auto_compiles_by_default_and_status_is_clean() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let source = workspace.join("dynamic.md");
        fs::write(&source, "# Dynamic Wiki\n\nFresh knowledge is searchable.")
            .expect("write source");

        cmd_wiki_ingest(workspace.to_str().expect("utf8 path"), &source, false).expect("ingest");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let freshness = wiki_freshness(&wiki).expect("freshness");
        assert!(freshness.is_clean(), "{freshness:?}");
        assert_eq!(freshness.up_to_date.len(), 1);
        let article = markdown_files(&wiki.join("wiki"))
            .expect("wiki files")
            .into_iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .expect("compiled article");
        let article_content = fs::read_to_string(article).expect("read article");
        assert!(article_content.contains("source_fingerprints: [raw/"));
        assert!(article_content.contains("fnv1a64:"));
    }

    #[test]
    fn freshness_detects_stale_raw_and_query_refreshes_it() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let source = workspace.join("stale.md");
        fs::write(&source, "# Stale Wiki\n\nOriginal payment behavior.").expect("write source");

        cmd_wiki_ingest(workspace.to_str().expect("utf8 path"), &source, false).expect("ingest");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let raw = markdown_files(&wiki.join("raw"))
            .expect("raw files")
            .into_iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .expect("raw source");
        let mut raw_content = fs::read_to_string(&raw).expect("read raw");
        raw_content.push_str("\n\nUpdated refund behavior.\n");
        fs::write(&raw, raw_content).expect("write raw");

        let stale = wiki_freshness(&wiki).expect("freshness");
        assert_eq!(stale.stale.len(), 1);

        cmd_wiki_query(
            workspace.to_str().expect("utf8 path"),
            "refund behavior",
            WikiQueryDepth::Standard,
            false,
        )
        .expect("query refresh");

        let fresh = wiki_freshness(&wiki).expect("freshness");
        assert!(fresh.is_clean(), "{fresh:?}");
    }

    #[test]
    fn no_compile_preserves_uncompiled_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let source = workspace.join("manual.md");
        fs::write(&source, "# Manual\n\nManual compile only.").expect("write source");

        cmd_wiki_ingest(workspace.to_str().expect("utf8 path"), &source, true).expect("ingest");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let freshness = wiki_freshness(&wiki).expect("freshness");
        assert_eq!(freshness.uncompiled.len(), 1);
        assert!(freshness.up_to_date.is_empty());
    }

    #[test]
    fn record_lesson_writes_raw_lesson_and_compiles() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();

        cmd_wiki_record_lesson(
            workspace.to_str().expect("utf8 path"),
            "Tool failure lesson",
            "consecutive_tool_failures",
            "Three read_file calls failed due to a wrong path.",
            "Root cause: the path was outside the workspace.\nNext time: verify the workspace root first.",
        )
        .expect("record lesson");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let raw = markdown_files(&wiki.join("raw"))
            .expect("raw files")
            .into_iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .expect("raw lesson");
        let raw_content = fs::read_to_string(raw).expect("read raw");
        assert!(raw_content.contains("type: lesson"));
        assert!(raw_content.contains("trigger: \"consecutive_tool_failures\""));
        assert!(raw_content.contains("## Root Cause"));
        assert!(raw_content.contains("## Optimization"));
        assert!(raw_content.contains("## Next Time"));

        let article = markdown_files(&wiki.join("wiki"))
            .expect("wiki files")
            .into_iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .expect("compiled article");
        let article_content = fs::read_to_string(article).expect("read article");
        assert!(article_content.contains("source_fingerprints: [raw/"));
    }

    #[test]
    fn record_lesson_defaults_to_optimization_template_when_body_empty() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();

        cmd_wiki_record_lesson(
            workspace.to_str().expect("utf8 path"),
            "Default lesson",
            "replan",
            "The run needed a smaller plan.",
            "",
        )
        .expect("record lesson");

        let wiki = skilllite_core::paths::project_wiki_root(workspace);
        let raw = markdown_files(&wiki.join("raw"))
            .expect("raw files")
            .into_iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) != Some("_index.md"))
            .expect("raw lesson");
        let raw_content = fs::read_to_string(raw).expect("read raw");
        assert!(raw_content.contains("## What Happened"));
        assert!(raw_content.contains("## Root Cause"));
        assert!(raw_content.contains("## Optimization"));
        assert!(raw_content.contains("## Next Time"));
    }

    #[test]
    fn lint_reports_dangling_article_source_and_markdown_link() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let wiki = ensure_wiki(workspace).expect("wiki");
        rebuild_indexes(&wiki).expect("indexes");
        let article = wiki.join("wiki").join("broken.md");
        fs::write(
            &article,
            "---\ntitle: \"Broken\"\ncategory: reference\nsources: [raw/missing.md]\ncreated: 2026-04-28\nupdated: 2026-04-28\ntags: []\naliases: []\nconfidence: low\nsummary: \"Broken\"\n---\n\n# Broken\n\nSee [missing](missing.md).\n",
        )
        .expect("write article");

        let findings = lint_wiki(&wiki).expect("lint");

        assert!(findings
            .iter()
            .any(|f| f.contains("dangling article source `raw/missing.md`")));
        assert!(findings
            .iter()
            .any(|f| f.contains("dangling markdown link `missing.md`")));
    }
}
