//! UTF-8 safe string helpers.

/// Truncate a string at a safe UTF-8 char boundary (from the start).
/// Returns a &str of at most `max_bytes` bytes, never splitting a multi-byte character.
pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Get a &str starting from approximately `start_pos`, adjusted forward to a safe UTF-8 boundary.
pub fn safe_slice_from(s: &str, start_pos: usize) -> &str {
    if start_pos >= s.len() {
        return "";
    }
    let mut start = start_pos;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

/// Split a string into chunks of approximately `chunk_size` bytes,
/// ensuring each split occurs at a valid UTF-8 char boundary.
pub fn chunk_str(s: &str, chunk_size: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < s.len() {
        let target_end = (start + chunk_size).min(s.len());
        let mut safe_end = target_end;
        while safe_end > start && !s.is_char_boundary(safe_end) {
            safe_end -= 1;
        }
        if safe_end == start && start < s.len() {
            safe_end = start + 1;
            while safe_end < s.len() && !s.is_char_boundary(safe_end) {
                safe_end += 1;
            }
        }
        chunks.push(&s[start..safe_end]);
        start = safe_end;
    }
    chunks
}
