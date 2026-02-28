//! Tests for the builtin tools module.

use super::*;

#[test]
fn test_search_replace_first_occurrence() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "hello world\nhello again\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "hello world",
        "new_string": "hi world",
        "replace_all": false
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("Successfully replaced 1 occurrence"));
    assert!(result.content.contains("\"first_changed_line\": 1"));
    assert!(result.content.contains("\"changed\": true"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hi world\nhello again\n");
}

#[test]
fn test_search_replace_requires_unique_match_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "hello world\nhello again\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "hello",
        "new_string": "hi"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("requires a unique match by default"));
}

#[test]
fn test_search_replace_all() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "foo bar foo baz foo\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "foo",
        "new_string": "qux",
        "replace_all": true
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("Successfully replaced 3 occurrence"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "qux bar qux baz qux\n");
}

#[test]
fn test_search_replace_old_string_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "hello world\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "xyz",
        "new_string": "abc"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("old_string not found"));
}

#[test]
fn test_search_replace_blocks_sensitive_path() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let env_path = workspace.join(".env");
    std::fs::write(&env_path, "KEY=value\n").unwrap();

    let args = serde_json::json!({
        "path": ".env",
        "old_string": "KEY=value",
        "new_string": "KEY=modified"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("Blocked"));
}

#[test]
fn test_search_replace_normalize_whitespace_trailing() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "hello world  \nnext line\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "hello world",
        "new_string": "hi",
        "normalize_whitespace": true
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hi\nnext line\n");
}

#[test]
fn test_search_replace_normalize_whitespace_replace_all() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "foo \nbar \nbaz\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "bar",
        "new_string": "qux",
        "replace_all": true,
        "normalize_whitespace": true
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "foo \nqux\nbaz\n");
}

#[test]
fn test_search_replace_normalize_whitespace_literal_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "price: 100\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "price: 100",
        "new_string": "price: $200",
        "normalize_whitespace": true
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "price: $200\n");
}

#[test]
fn test_search_replace_output_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let output_dir = workspace.join("output");
    std::fs::create_dir_all(&output_dir).unwrap();
    let file_path = output_dir.join("index.html");
    std::fs::write(&file_path, "<title>Old Title</title>").unwrap();

    let args = serde_json::json!({
        "path": "output/index.html",
        "old_string": "Old Title",
        "new_string": "New Title"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "<title>New Title</title>");
}

#[test]
fn test_preview_edit_does_not_write_file() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "alpha beta\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "alpha",
        "new_string": "gamma"
    });
    let result = execute_builtin_tool("preview_edit", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("Preview edit"));
    assert!(result.content.contains("\"changed\": true"));
    assert!(result.content.contains("\"diff_excerpt\""));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "alpha beta\n");
}

// ─── P0: read_file line numbers + range ─────────────────────────────

#[test]
fn test_read_file_with_line_numbers() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

    let args = serde_json::json!({ "path": "test.txt" });
    let result = execute_builtin_tool("read_file", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("1|line1"));
    assert!(result.content.contains("2|line2"));
    assert!(result.content.contains("3|line3"));
}

#[test]
fn test_read_file_with_range() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "aaa\nbbb\nccc\nddd\neee\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "start_line": 2,
        "end_line": 4
    });
    let result = execute_builtin_tool("read_file", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("2|bbb"));
    assert!(result.content.contains("3|ccc"));
    assert!(result.content.contains("4|ddd"));
    assert!(!result.content.contains("1|aaa"));
    assert!(!result.content.contains("5|eee"));
    assert!(result.content.contains("[Showing lines 2-4 of 5 total]"));
}

#[test]
fn test_read_file_range_beyond_end() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "only\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "start_line": 100
    });
    let result = execute_builtin_tool("read_file", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("File has 1 lines"));
}

// ─── P0: insert_lines ───────────────────────────────────────────────

#[test]
fn test_insert_lines_at_beginning() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\nline2\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 0,
        "content": "inserted"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("Successfully inserted"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "inserted\nline1\nline2\n");
}

#[test]
fn test_insert_lines_in_middle() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 1,
        "content": "new_line"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "line1\nnew_line\nline2\nline3\n");
}

#[test]
fn test_insert_lines_at_end() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\nline2\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 2,
        "content": "last_line"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "line1\nline2\nlast_line\n");
}

#[test]
fn test_insert_lines_multiline_content() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "aaa\nbbb\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 1,
        "content": "x1\nx2\nx3"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"lines_inserted\": 3"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "aaa\nx1\nx2\nx3\nbbb\n");
}

#[test]
fn test_insert_lines_beyond_end_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 99,
        "content": "nope"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("beyond end of file"));
}

#[test]
fn test_insert_lines_no_trailing_newline() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "hello\nworld").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 2,
        "content": "end"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello\nworld\nend\n");
}

// ─── P0: search_replace dry_run ─────────────────────────────────────

#[test]
fn test_search_replace_dry_run_no_write() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "alpha beta\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "alpha",
        "new_string": "gamma",
        "dry_run": true
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("Preview edit"));
    assert!(result.content.contains("no changes written"));
    assert!(result.content.contains("\"match_type\": \"exact\""));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "alpha beta\n");
}

// ─── P0: match_type in result ───────────────────────────────────────

#[test]
fn test_search_replace_match_type_exact() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "hello world\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "hello world",
        "new_string": "hi world"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"match_type\": \"exact\""));
}

// ─── P0: fuzzy match — whitespace (Level 2) ─────────────────────────

#[test]
fn test_fuzzy_match_indent_difference() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.rs");
    std::fs::write(
        &file_path,
        "fn main() {\n    let x = 1;\n    let y = 2;\n}\n",
    )
    .unwrap();

    // old_string has 2-space indent instead of 4-space; multi-line prevents substring match
    let args = serde_json::json!({
        "path": "test.rs",
        "old_string": "  let x = 1;\n  let y = 2;",
        "new_string": "    let a = 10;\n    let b = 20;"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error, "Error: {}", result.content);
    assert!(result.content.contains("\"match_type\": \"whitespace_fuzzy\""));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("let a = 10"));
    assert!(content.contains("let b = 20"));
}

#[test]
fn test_fuzzy_match_trailing_whitespace_auto() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    // File has trailing spaces on the line
    std::fs::write(&file_path, "hello world   \nnext\n").unwrap();

    // old_string without trailing spaces — exact match fails because
    // "hello world" is a substring of "hello world   ", but let's
    // test the multi-line case
    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "hello world   \nnext",
        "new_string": "hi\nnext"
    });
    // Exact match succeeds here (substring match)
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"match_type\": \"exact\""));
}

#[test]
fn test_fuzzy_match_multiline_indent() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.py");
    std::fs::write(
        &file_path,
        "def foo():\n    x = 1\n    y = 2\n    return x + y\n",
    )
    .unwrap();

    // old_string has no indentation
    let args = serde_json::json!({
        "path": "test.py",
        "old_string": "x = 1\ny = 2",
        "new_string": "    a = 10\n    b = 20"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"match_type\": \"whitespace_fuzzy\""));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("    a = 10\n    b = 20"));
}

// ─── P0: fuzzy match — blank lines (Level 3) ────────────────────────

#[test]
fn test_fuzzy_match_blank_line_difference() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    // Content has an extra blank line between the two lines
    std::fs::write(&file_path, "aaa\n\nbbb\nccc\n").unwrap();

    // old_string without the blank line
    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "aaa\nbbb",
        "new_string": "xxx\nyyy"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"match_type\": \"blank_line_fuzzy\""));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.starts_with("xxx\nyyy"));
}

// ─── P0: fuzzy match — Levenshtein similarity (Level 4) ─────────────

#[test]
fn test_fuzzy_match_similarity() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(
        &file_path,
        "fn calculate_total(items: &[Item]) -> f64 {\n    items.iter().map(|i| i.price).sum()\n}\n",
    )
    .unwrap();

    // old_string has a minor typo / difference (calculate_totl instead of calculate_total)
    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "fn calculate_totl(items: &[Item]) -> f64 {\n    items.iter().map(|i| i.price).sum()\n}",
        "new_string": "fn calculate_total(items: &[Item]) -> u64 {\n    items.iter().map(|i| i.price as u64).sum()\n}"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("similarity("));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("-> u64"));
}

#[test]
fn test_fuzzy_match_low_similarity_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "completely different content here\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "nothing even close to matching this at all",
        "new_string": "replacement"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("old_string not found"));
}

// ─── P0: insert_lines blocks sensitive paths ────────────────────────

#[test]
fn test_insert_lines_blocks_sensitive_path() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let env_path = workspace.join(".env");
    std::fs::write(&env_path, "KEY=value\n").unwrap();

    let args = serde_json::json!({
        "path": ".env",
        "line": 0,
        "content": "INJECTED=bad"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("Blocked"));
}

// ─── Phase II: grep_files ────────────────────────────────────────────

#[test]
fn test_grep_files_basic_match() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    std::fs::write(workspace.join("a.txt"), "hello world\nfoo bar\n").unwrap();
    std::fs::write(workspace.join("b.txt"), "hello rust\nbaz\n").unwrap();

    let args = serde_json::json!({ "pattern": "hello" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("a.txt:1:hello world"));
    assert!(result.content.contains("b.txt:1:hello rust"));
    assert!(result.content.contains("2 match(es) in 2 file(s)"));
}

#[test]
fn test_grep_files_regex_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    std::fs::write(workspace.join("code.rs"), "fn main() {\n    let x = 42;\n}\n").unwrap();

    let args = serde_json::json!({ "pattern": r"fn\s+\w+" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("code.rs:1:fn main()"));
}

#[test]
fn test_grep_files_include_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    std::fs::write(workspace.join("a.rs"), "match_me\n").unwrap();
    std::fs::write(workspace.join("b.py"), "match_me\n").unwrap();

    let args = serde_json::json!({ "pattern": "match_me", "include": "*.rs" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("a.rs:1:match_me"));
    assert!(!result.content.contains("b.py"));
}

#[test]
fn test_grep_files_no_match() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    std::fs::write(workspace.join("a.txt"), "hello\n").unwrap();

    let args = serde_json::json!({ "pattern": "xyz_not_here" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("No matches found"));
}

#[test]
fn test_grep_files_skips_git_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let git_dir = workspace.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("config"), "find_me\n").unwrap();
    std::fs::write(workspace.join("src.txt"), "find_me\n").unwrap();

    let args = serde_json::json!({ "pattern": "find_me" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("src.txt"));
    assert!(!result.content.contains(".git"));
}

#[test]
fn test_grep_files_recursive() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let sub = workspace.join("sub").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("nested.txt"), "deep_match\n").unwrap();

    let args = serde_json::json!({ "pattern": "deep_match" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("sub/deep/nested.txt:1:deep_match"));
}

#[test]
fn test_grep_files_invalid_regex() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();

    let args = serde_json::json!({ "pattern": "[invalid" });
    let result = execute_builtin_tool("grep_files", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("Invalid regex"));
}

// ─── Phase II: auto-backup ───────────────────────────────────────────

#[test]
fn test_search_replace_creates_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "original content\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "original",
        "new_string": "modified"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"backup\""));
    assert!(result.content.contains("edit-backups"));
}

#[test]
fn test_insert_lines_creates_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 0,
        "content": "prepended"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"backup\""));
    assert!(result.content.contains("edit-backups"));
}

#[test]
fn test_dry_run_no_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "alpha beta\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "old_string": "alpha",
        "new_string": "gamma",
        "dry_run": true
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"backup\": null"));
}

// ─── Phase II: syntax validation ─────────────────────────────────────

#[test]
fn test_validation_warns_on_invalid_json() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("data.json");
    std::fs::write(&file_path, "{\"key\": \"value\"}\n").unwrap();

    let _args = serde_json::json!({
        "path": "data.json",
        "old_string": "\"value\"",
        "new_string": "\"value\""
    });
    let args2 = serde_json::json!({
        "path": "data.json",
        "old_string": "{\"key\": \"value\"}",
        "new_string": "{\"key\": \"value\""
    });
    let result = execute_builtin_tool("search_replace", &args2.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("JSON syntax warning"));
}

#[test]
fn test_validation_warns_on_unmatched_bracket() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("code.rs");
    std::fs::write(&file_path, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

    let args = serde_json::json!({
        "path": "code.rs",
        "old_string": "fn main() {\n    println!(\"hi\");\n}",
        "new_string": "fn main() {\n    println!(\"hi\");\n"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("validation_warning"));
    assert!(result.content.contains("Unclosed"));
}

#[test]
fn test_validation_no_warning_on_valid_code() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("code.rs");
    std::fs::write(&file_path, "fn foo() {\n    1 + 2\n}\n").unwrap();

    let args = serde_json::json!({
        "path": "code.rs",
        "old_string": "1 + 2",
        "new_string": "3 + 4"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("\"validation_warning\": null"));
}

#[test]
fn test_search_replace_multibyte_content_no_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("readme.md");
    let chinese_content = "# 项目说明\n\n**轻量级 AI Agent 安全引擎**，内置原生系统级沙箱，零依赖，本地执行。\n\n## 功能特性\n\n- 🔒 安全沙箱\n- 🚀 高性能\n- 📦 零依赖\n";
    std::fs::write(&file_path, chinese_content).unwrap();

    let args = serde_json::json!({
        "path": "readme.md",
        "old_string": "**轻量级 AI Agent 安全引擎**，内置原生系统级沙箱，零依赖，本地执行。",
        "new_string": "**A lightweight AI Agent secure engine** with built-in sandbox, zero deps."
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error, "Error: {}", result.content);
    assert!(result.content.contains("\"match_type\": \"exact\""));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("lightweight AI Agent"));
}

#[test]
fn test_validation_warns_on_invalid_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("config.yaml");
    std::fs::write(&file_path, "key: value\nnested:\n  a: 1\n").unwrap();

    let args = serde_json::json!({
        "path": "config.yaml",
        "old_string": "nested:\n  a: 1",
        "new_string": "nested:\n  a: 1\n  b: [unclosed"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(!result.is_error);
    assert!(result.content.contains("YAML syntax warning") || result.content.contains("Unclosed"));
}

// ─── Phase III: edit failure smart hints ─────────────────────────────

#[test]
fn test_failure_hint_shows_closest_match_context() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.rs");
    std::fs::write(
        &file_path,
        "fn main() {\n    println!(\"hello world\");\n    let x = 42;\n}\n",
    )
    .unwrap();

    let args = serde_json::json!({
        "path": "test.rs",
        "old_string": "completely_unrelated_string_that_wont_match_anything_at_all_xyz"
    ,   "new_string": "replacement"
    });
    let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
    assert!(result.is_error);
    assert!(result.content.contains("Closest match found at line"));
    assert!(result.content.contains("similarity:"));
    assert!(result.content.contains("Tip:"));
}

// ─── Phase III: insert_lines indent awareness ───────────────────────

#[test]
fn test_insert_lines_auto_indent() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.rs");
    std::fs::write(
        &file_path,
        "fn main() {\n    let x = 1;\n    let y = 2;\n}\n",
    )
    .unwrap();

    let args = serde_json::json!({
        "path": "test.rs",
        "line": 2,
        "content": "let z = 3;"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        content.contains("    let z = 3;"),
        "Expected auto-indented line, got:\n{}",
        content
    );
}

#[test]
fn test_insert_lines_no_indent_when_content_already_indented() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.py");
    std::fs::write(&file_path, "def foo():\n    x = 1\n").unwrap();

    let args = serde_json::json!({
        "path": "test.py",
        "line": 1,
        "content": "    y = 2"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        content.contains("    y = 2") && !content.contains("        y = 2"),
        "Should NOT double-indent already-indented content, got:\n{}",
        content
    );
}

#[test]
fn test_insert_lines_no_indent_at_top_level() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.txt");
    std::fs::write(&file_path, "line1\nline2\n").unwrap();

    let args = serde_json::json!({
        "path": "test.txt",
        "line": 1,
        "content": "new_line"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "line1\nnew_line\nline2\n");
}

#[test]
fn test_insert_lines_multiline_auto_indent() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path();
    let file_path = workspace.join("test.rs");
    std::fs::write(
        &file_path,
        "fn main() {\n    let x = 1;\n}\n",
    )
    .unwrap();

    let args = serde_json::json!({
        "path": "test.rs",
        "line": 1,
        "content": "let y = 2;\nlet z = 3;"
    });
    let result = execute_builtin_tool("insert_lines", &args.to_string(), workspace);
    assert!(!result.is_error);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("    let y = 2;\n    let z = 3;"));
}

// ─── Phase III: run_command output truncation ───────────────────────

#[test]
fn test_run_command_truncation() {
    use super::run_command;
    let long_output = "x".repeat(10000);
    let truncated = run_command::truncate_command_output_for_test(&long_output);
    assert!(truncated.len() < long_output.len());
    assert!(truncated.contains("output truncated"));
    assert!(truncated.contains("10000 total chars"));
}

#[test]
fn test_run_command_no_truncation_short() {
    use super::run_command;
    let short = "hello world";
    let result = run_command::truncate_command_output_for_test(short);
    assert_eq!(result, short);
}
