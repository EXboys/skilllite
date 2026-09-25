//! Parse `.env` without linking `skilllite-core`.

pub fn parse_dotenv_content(content: &str) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let mut value = line[eq_pos + 1..].trim();
            if let Some(hash_pos) = value.find('#') {
                let before_hash = value[..hash_pos].trim_end();
                if !before_hash.contains('"') && !before_hash.contains('\'') {
                    value = before_hash;
                }
            }
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = &value[1..value.len() - 1];
            }
            if !key.is_empty() {
                vars.push((key, value.to_string()));
            }
        }
    }
    vars
}

pub fn parse_dotenv_from_dir(dir: &std::path::Path) -> Vec<(String, String)> {
    let path = dir.join(".env");
    if let Ok(content) = std::fs::read_to_string(&path) {
        parse_dotenv_content(&content)
    } else {
        vec![]
    }
}

pub fn parse_dotenv_walking_up(
    start: &std::path::Path,
    max_levels: usize,
) -> Vec<(String, String)> {
    let mut dir = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    for _ in 0..max_levels {
        let vars = parse_dotenv_from_dir(&dir);
        if !vars.is_empty() {
            return vars;
        }
        if !dir.pop() {
            break;
        }
    }
    vec![]
}
