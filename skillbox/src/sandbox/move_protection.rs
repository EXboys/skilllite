//! Move Protection and LogTag Mechanism for Sandbox Security
//!
//! This module provides:
//! - Move blocking rules to prevent bypass via mv/rename (P0)
//! - LogTag mechanism for precise violation tracking (P1)
//! - Glob to regex conversion for flexible path matching

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// LogTag Mechanism for Precise Violation Tracking (P1)
// ============================================================================

/// Session suffix for this process instance, used to filter log events
static SESSION_SUFFIX: OnceLock<String> = OnceLock::new();

/// Generate a random alphanumeric suffix using system time and process id
fn generate_session_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    
    // Create a simple hash-like string from timestamp and pid
    let combined = timestamp ^ (pid as u128);
    let chars: Vec<char> = "0123456789abcdefghijklmnopqrstuvwxyz".chars().collect();
    let suffix: String = (0..9)
        .map(|i| {
            let idx = ((combined >> (i * 4)) & 0x1F) as usize % chars.len();
            chars[idx]
        })
        .collect();
    
    format!("_{}_SBX", suffix)
}

/// Get the session suffix for this process instance
pub fn get_session_suffix() -> &'static str {
    SESSION_SUFFIX.get_or_init(generate_session_suffix)
}

/// Simple base64 encoding for command strings (URL-safe variant)
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = input.as_bytes();
    let mut result = String::with_capacity((bytes.len() + 2) / 3 * 4);
    
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        
        result.push(ALPHABET[(b0 >> 2) & 0x3F] as char);
        result.push(ALPHABET[((b0 << 4) | (b1 >> 4)) & 0x3F] as char);
        
        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 << 2) | (b2 >> 6)) & 0x3F] as char);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3F] as char);
        }
    }
    
    result
}

/// Encode a command for embedding in sandbox log messages
pub fn encode_sandboxed_command(command: &str) -> String {
    base64_encode(command)
}

/// Generate a unique log tag for sandbox monitoring
/// 
/// The log tag is embedded in sandbox deny rules via `(with message "...")` 
/// and can be used to filter system logs for violations from this specific command.
/// 
/// Format: `CMD64_{base64_encoded_command}_END{session_suffix}`
pub fn generate_log_tag(command: &str) -> String {
    let encoded_command = encode_sandboxed_command(command);
    format!("CMD64_{}_END{}", encoded_command, get_session_suffix())
}

// ============================================================================
// Move Blocking Rules - Prevents Bypass via mv/rename (P0)
// ============================================================================

/// Get all ancestor directories for a path, up to (but not including) root
/// 
/// Example: `/private/tmp/test/file.txt` -> `["/private/tmp/test", "/private/tmp", "/private"]`
pub fn get_ancestor_directories(path_str: &str) -> Vec<String> {
    let mut ancestors = Vec::new();
    let path = PathBuf::from(path_str);
    let mut current = path.parent();
    
    while let Some(parent) = current {
        let parent_str = parent.to_string_lossy().to_string();
        if parent_str.is_empty() || parent_str == "/" {
            break;
        }
        ancestors.push(parent_str);
        current = parent.parent();
    }
    
    ancestors
}

/// Convert a glob pattern to a regular expression for macOS sandbox profiles
/// 
/// This implements gitignore-style pattern matching:
/// - `*` matches any characters except `/`
/// - `**` matches any characters including `/`
/// - `**/` matches zero or more directories
/// - `?` matches any single character except `/`
/// - `[abc]` matches any character in the set
pub fn glob_to_regex(glob_pattern: &str) -> String {
    let mut result = String::with_capacity(glob_pattern.len() * 2);
    result.push('^');
    
    let mut chars = glob_pattern.chars().peekable();
    
    while let Some(c) = chars.next() {
        match c {
            // Escape regex special characters (except glob chars)
            '.' => result.push_str("\\."),
            '^' => result.push_str("\\^"),
            '$' => result.push_str("\\$"),
            '+' => result.push_str("\\+"),
            '{' => result.push_str("\\{"),
            '}' => result.push_str("\\}"),
            '(' => result.push_str("\\("),
            ')' => result.push_str("\\)"),
            '|' => result.push_str("\\|"),
            '\\' => result.push_str("\\\\"),
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next(); // consume second *
                    if chars.peek() == Some(&'/') {
                        chars.next(); // consume /
                        // **/ matches zero or more directories
                        result.push_str("(.*/)?");
                    } else {
                        // ** matches anything including /
                        result.push_str(".*");
                    }
                } else {
                    // * matches anything except /
                    result.push_str("[^/]*");
                }
            }
            '?' => {
                // ? matches single character except /
                result.push_str("[^/]");
            }
            '[' => {
                // Character class - pass through
                result.push('[');
                while let Some(inner) = chars.next() {
                    if inner == ']' {
                        result.push(']');
                        break;
                    }
                    result.push(inner);
                }
            }
            _ => result.push(c),
        }
    }
    
    result.push('$');
    result
}

/// Check if a path pattern contains glob characters
pub fn contains_glob_chars(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

/// Escape a path for use in Seatbelt sandbox profile
/// Uses JSON encoding for proper escaping of special characters
pub fn escape_path_for_seatbelt(path: &str) -> String {
    serde_json::to_string(path).unwrap_or_else(|_| format!("\"{}\"", path))
}

/// Generate deny rules for file movement (file-write-unlink) to protect paths
/// 
/// This prevents bypassing read or write restrictions by moving files/directories.
/// For example, an attacker could try:
/// ```text
/// mv /protected/parent /tmp/parent
/// # Now access /tmp/parent/sensitive_file
/// ```
/// 
/// These rules block:
/// 1. Moving/renaming files matching the protected pattern
/// 2. Moving ancestor directories of protected paths
pub fn generate_move_blocking_rules(path_patterns: &[String], log_tag: &str) -> Vec<String> {
    let mut rules = Vec::new();
    
    for path_pattern in path_patterns {
        if contains_glob_chars(path_pattern) {
            // Use regex matching for glob patterns
            let regex_pattern = glob_to_regex(path_pattern);
            
            // Block moving/renaming files matching this pattern
            rules.push(format!(
                "(deny file-write-unlink\n  (regex #\"{}\")\n  (with message \"{}\"))",
                regex_pattern, log_tag
            ));
            
            // For glob patterns, extract the static prefix and block ancestor moves
            let static_prefix: String = path_pattern
                .chars()
                .take_while(|c| !['*', '?', '['].contains(c))
                .collect();
            
            if !static_prefix.is_empty() && static_prefix != "/" {
                // Get the directory containing the glob pattern
                let base_dir = if static_prefix.ends_with('/') {
                    static_prefix[..static_prefix.len() - 1].to_string()
                } else {
                    PathBuf::from(&static_prefix)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                
                if !base_dir.is_empty() {
                    // Block moves of the base directory itself
                    rules.push(format!(
                        "(deny file-write-unlink\n  (literal {})\n  (with message \"{}\"))",
                        escape_path_for_seatbelt(&base_dir), log_tag
                    ));
                    
                    // Block moves of ancestor directories
                    for ancestor_dir in get_ancestor_directories(&base_dir) {
                        rules.push(format!(
                            "(deny file-write-unlink\n  (literal {})\n  (with message \"{}\"))",
                            escape_path_for_seatbelt(&ancestor_dir), log_tag
                        ));
                    }
                }
            }
        } else {
            // Use subpath matching for literal paths
            
            // Block moving/renaming the denied path itself
            rules.push(format!(
                "(deny file-write-unlink\n  (subpath {})\n  (with message \"{}\"))",
                escape_path_for_seatbelt(path_pattern), log_tag
            ));
            
            // Block moves of ancestor directories
            for ancestor_dir in get_ancestor_directories(path_pattern) {
                rules.push(format!(
                    "(deny file-write-unlink\n  (literal {})\n  (with message \"{}\"))",
                    escape_path_for_seatbelt(&ancestor_dir), log_tag
                ));
            }
        }
    }
    
    rules
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_suffix() {
        let suffix = get_session_suffix();
        assert!(suffix.starts_with('_'));
        assert!(suffix.ends_with("_SBX"));
        assert!(suffix.len() > 5);
    }

    #[test]
    fn test_generate_log_tag() {
        let command = "python3 script.py";
        let log_tag = generate_log_tag(command);
        assert!(log_tag.starts_with("CMD64_"));
        assert!(log_tag.contains("_END"));
        assert!(log_tag.ends_with("_SBX"));
    }

    #[test]
    fn test_get_ancestor_directories() {
        let ancestors = get_ancestor_directories("/private/tmp/test/file.txt");
        assert_eq!(ancestors, vec![
            "/private/tmp/test",
            "/private/tmp",
            "/private",
        ]);
    }

    #[test]
    fn test_get_ancestor_directories_short_path() {
        let ancestors = get_ancestor_directories("/tmp/file.txt");
        assert_eq!(ancestors, vec!["/tmp"]);
    }

    #[test]
    fn test_glob_to_regex_simple() {
        assert_eq!(glob_to_regex("*.txt"), "^[^/]*\\.txt$");
        assert_eq!(glob_to_regex("file?.txt"), "^file[^/]\\.txt$");
    }

    #[test]
    fn test_glob_to_regex_globstar() {
        assert_eq!(glob_to_regex("**/*.txt"), "^(.*/)?[^/]*\\.txt$");
        assert_eq!(glob_to_regex("src/**"), "^src/.*$");
    }

    #[test]
    fn test_contains_glob_chars() {
        assert!(contains_glob_chars("*.txt"));
        assert!(contains_glob_chars("file?.txt"));
        assert!(contains_glob_chars("[abc].txt"));
        assert!(!contains_glob_chars("file.txt"));
        assert!(!contains_glob_chars("/path/to/file"));
    }

    #[test]
    fn test_generate_move_blocking_rules() {
        let patterns = vec!["/home/user/.ssh".to_string()];
        let log_tag = "TEST_TAG";
        let rules = generate_move_blocking_rules(&patterns, log_tag);
        
        // Should have rule for the path itself
        assert!(rules.iter().any(|r| r.contains("subpath") && r.contains(".ssh")));
        // Should have rules for ancestors
        assert!(rules.iter().any(|r| r.contains("literal") && r.contains("/home/user")));
        assert!(rules.iter().any(|r| r.contains("literal") && r.contains("/home")));
    }

    #[test]
    fn test_generate_move_blocking_rules_glob() {
        let patterns = vec!["**/.git/hooks/**".to_string()];
        let log_tag = "TEST_TAG";
        let rules = generate_move_blocking_rules(&patterns, log_tag);
        
        // Should have regex rule for glob pattern
        assert!(rules.iter().any(|r| r.contains("regex")));
    }
}
