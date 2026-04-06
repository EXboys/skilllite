//! Optional Bearer authentication for the swarm HTTP API (`SKILLLITE_SWARM_TOKEN`).

use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};

/// Read non-empty `SKILLLITE_SWARM_TOKEN` from the environment.
///
/// Call after `load_dotenv` / `load_dotenv_from_dir` if the token may live in `.env`.
pub fn swarm_token_from_env() -> Option<String> {
    let v = std::env::var(skilllite_core::config::env_keys::swarm::SKILLLITE_SWARM_TOKEN).ok()?;
    let t = v.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn ct_eq_str(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn extract_bearer(headers: &HeaderMap) -> Option<&str> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let mut parts = raw.split_whitespace();
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if token.is_empty() {
        return None;
    }
    Some(token)
}

/// Returns `Some(response)` when the request must be rejected (missing or invalid auth).
pub fn reject_if_unauthorized(swarm_token: Option<&str>, headers: &HeaderMap) -> Option<Response> {
    let expected = swarm_token.filter(|s| !s.is_empty())?;
    let ok = extract_bearer(headers)
        .map(|got| ct_eq_str(got, expected))
        .unwrap_or(false);
    if ok {
        return None;
    }
    Some(
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "unauthorized",
                "message": "Missing or invalid Authorization: Bearer token (set SKILLLITE_SWARM_TOKEN on server and clients to the same secret)"
            })),
        )
            .into_response(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_extract_ok() {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            "Bearer secret-token".parse().unwrap(),
        );
        assert_eq!(extract_bearer(&h), Some("secret-token"));
    }

    #[test]
    fn bearer_scheme_case_insensitive() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "bearer abc".parse().unwrap());
        assert_eq!(extract_bearer(&h), Some("abc"));
    }

    #[test]
    fn ct_eq_rejects_wrong_len() {
        assert!(!ct_eq_str("a", "ab"));
        assert!(ct_eq_str("x", "x"));
    }
}
