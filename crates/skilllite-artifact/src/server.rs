//! Axum HTTP server mapping for [`ArtifactStore`](skilllite_core::artifact_store::ArtifactStore).

use std::sync::Arc;

use crate::validation::validate_run_id;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use skilllite_core::artifact_store::{validate_artifact_key, ArtifactStore, StoreError};

/// Maximum request body size for `PUT` (per artifact). Larger uploads receive HTTP 413.
pub const MAX_ARTIFACT_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Configuration for the HTTP artifact API (optional bearer token).
#[derive(Clone, Debug, Default)]
pub struct ArtifactHttpServerConfig {
    /// When set, requests must include `Authorization: Bearer <token>`.
    pub bearer_token: Option<String>,
}

/// Shared state for the artifact HTTP router.
#[derive(Clone)]
pub struct ArtifactHttpState {
    pub store: Arc<dyn ArtifactStore>,
    bearer_token: Option<String>,
}

impl ArtifactHttpState {
    /// Build state from a store and server config.
    pub fn new(store: Arc<dyn ArtifactStore>, config: ArtifactHttpServerConfig) -> Self {
        Self {
            store,
            bearer_token: config.bearer_token,
        }
    }
}

/// Router: `GET` / `PUT` `/v1/runs/{run_id}/artifacts?key=...`
pub fn artifact_router(state: ArtifactHttpState) -> Router {
    Router::new()
        .route(
            "/v1/runs/:run_id/artifacts",
            get(get_artifact).put(put_artifact),
        )
        .layer(DefaultBodyLimit::max(MAX_ARTIFACT_BODY_BYTES))
        .layer(from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct KeyQuery {
    key: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "missing or invalid Authorization bearer token".to_string(),
        }
    }

    fn from_store(err: StoreError) -> Self {
        match err {
            StoreError::InvalidKey { key, reason } => Self {
                status: StatusCode::BAD_REQUEST,
                code: "invalid_key",
                message: format!("key {:?}: {}", key, reason),
            },
            StoreError::NotFound { run_id, key } => Self {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: format!("run_id={} key={}", run_id, key),
            },
            StoreError::Backend {
                message, retryable, ..
            } => Self {
                status: if retryable {
                    StatusCode::BAD_GATEWAY
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                },
                code: "backend_error",
                message,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorBody {
            error: self.code,
            message: self.message,
        });
        (self.status, body).into_response()
    }
}

async fn auth_middleware(
    State(state): State<ArtifactHttpState>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    if let Some(expected) = &state.bearer_token {
        let ok = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|t| t.trim() == expected.as_str())
            .unwrap_or(false);
        if !ok {
            return Err(ApiError::unauthorized());
        }
    }
    Ok(next.run(req).await)
}

async fn get_artifact(
    State(state): State<ArtifactHttpState>,
    Path(run_id): Path<String>,
    Query(q): Query<KeyQuery>,
) -> Result<impl IntoResponse, ApiError> {
    validate_run_id(&run_id).map_err(ApiError::from_store)?;
    validate_artifact_key(&q.key).map_err(ApiError::from_store)?;

    let store = state.store.clone();
    let key = q.key.clone();
    let data = tokio::task::spawn_blocking(move || store.get(&run_id, &key))
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "join_error",
            message: format!("spawn_blocking join: {}", e),
        })?
        .map_err(ApiError::from_store)?;

    match data {
        Some(bytes) => Ok((
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "artifact not found".to_string(),
        }),
    }
}

async fn put_artifact(
    State(state): State<ArtifactHttpState>,
    Path(run_id): Path<String>,
    Query(q): Query<KeyQuery>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    validate_run_id(&run_id).map_err(ApiError::from_store)?;
    validate_artifact_key(&q.key).map_err(ApiError::from_store)?;

    let store = state.store.clone();
    let key = q.key.clone();
    let payload = body.to_vec();
    tokio::task::spawn_blocking(move || store.put(&run_id, &key, &payload))
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "join_error",
            message: format!("spawn_blocking join: {}", e),
        })?
        .map_err(ApiError::from_store)?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use skilllite_core::artifact_store::StoreError;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tower::ServiceExt;

    #[derive(Default)]
    struct MemoryStore(Mutex<HashMap<(String, String), Vec<u8>>>);

    impl ArtifactStore for MemoryStore {
        fn get(&self, run_id: &str, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
            Ok(self
                .0
                .lock()
                .map_err(|e| StoreError::Backend {
                    message: format!("mutex poisoned: {}", e),
                    retryable: false,
                    source: None,
                })?
                .get(&(run_id.to_string(), key.to_string()))
                .cloned())
        }

        fn put(&self, run_id: &str, key: &str, data: &[u8]) -> Result<(), StoreError> {
            self.0
                .lock()
                .map_err(|e| StoreError::Backend {
                    message: format!("mutex poisoned: {}", e),
                    retryable: false,
                    source: None,
                })?
                .insert((run_id.to_string(), key.to_string()), data.to_vec());
            Ok(())
        }
    }

    fn test_router(config: ArtifactHttpServerConfig) -> Router {
        let store: Arc<dyn ArtifactStore> = Arc::new(MemoryStore::default());
        artifact_router(ArtifactHttpState::new(store, config))
    }

    #[tokio::test]
    async fn get_put_roundtrip() {
        let app = test_router(ArtifactHttpServerConfig::default());
        let uri_put = "/v1/runs/run-a/artifacts?key=out.bin";
        let req_put = Request::builder()
            .method("PUT")
            .uri(uri_put)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(vec![1u8, 2, 3]))
            .unwrap();
        let res = app.clone().oneshot(req_put).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let req_get = Request::builder()
            .method("GET")
            .uri(uri_put)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req_get).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), &[1, 2, 3]);
    }

    #[tokio::test]
    async fn get_missing_returns_404_json() {
        let app = test_router(ArtifactHttpServerConfig::default());
        let req = Request::builder()
            .method("GET")
            .uri("/v1/runs/run-x/artifacts?key=nope")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn bearer_required_rejects_missing() {
        let app = test_router(ArtifactHttpServerConfig {
            bearer_token: Some("secret".to_string()),
        });
        let req = Request::builder()
            .method("GET")
            .uri("/v1/runs/run-a/artifacts?key=k")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn put_payload_over_limit_returns_413() {
        let app = test_router(ArtifactHttpServerConfig::default());
        let oversized = vec![b'x'; MAX_ARTIFACT_BODY_BYTES + 1];
        let uri = "/v1/runs/run-a/artifacts?key=big.bin";
        let req = Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(oversized))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn bearer_accepts_valid() {
        let app = test_router(ArtifactHttpServerConfig {
            bearer_token: Some("secret".to_string()),
        });
        let uri = "/v1/runs/run-a/artifacts?key=k";
        let req_put = Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::AUTHORIZATION, "Bearer secret")
            .body(Body::from(b"ok".as_slice()))
            .unwrap();
        let res = app.clone().oneshot(req_put).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }
}
