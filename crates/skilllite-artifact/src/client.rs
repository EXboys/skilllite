//! Blocking HTTP client implementing [`ArtifactStore`](skilllite_core::artifact_store::ArtifactStore).

use reqwest::header::{HeaderValue, CONTENT_TYPE};
use reqwest::StatusCode as ReqStatus;
use reqwest::Url;
use skilllite_core::artifact_store::{validate_artifact_key, ArtifactStore, StoreError};

use crate::validation::validate_run_id;

/// [`ArtifactStore`] backed by the HTTP API served by [`crate::artifact_router`].
pub struct HttpArtifactStore {
    client: reqwest::blocking::Client,
    base: Url,
    bearer: Option<String>,
}

impl HttpArtifactStore {
    /// `base_url` is the origin only (e.g. `http://127.0.0.1:8080`); trailing slashes are stripped.
    pub fn try_new(base_url: &str, bearer_token: Option<&str>) -> crate::Result<Self> {
        let base = Url::parse(base_url.trim_end_matches('/'))
            .map_err(|e| crate::Error::InvalidClientConfig(format!("invalid base URL: {e}")))?;
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| crate::Error::InvalidClientConfig(format!("reqwest client build: {e}")))?;
        Ok(Self {
            client,
            base,
            bearer: bearer_token.map(String::from),
        })
    }

    fn url(&self, run_id: &str, key: &str) -> std::result::Result<Url, StoreError> {
        let path = format!("v1/runs/{}/artifacts", run_id);
        let mut u = self.base.join(&path).map_err(|e| StoreError::Backend {
            message: format!("url join: {}", e),
            retryable: false,
            source: None,
        })?;
        u.query_pairs_mut().append_pair("key", key);
        Ok(u)
    }

    fn apply_auth(
        &self,
        rb: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match &self.bearer {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        }
    }
}

impl ArtifactStore for HttpArtifactStore {
    fn get(&self, run_id: &str, key: &str) -> std::result::Result<Option<Vec<u8>>, StoreError> {
        validate_run_id(run_id)?;
        validate_artifact_key(key)?;
        let url = self.url(run_id, key)?;
        let resp =
            self.apply_auth(self.client.get(url))
                .send()
                .map_err(|e| StoreError::Backend {
                    message: format!("http request: {}", e),
                    retryable: true,
                    source: Some(Box::new(e)),
                })?;
        let status = resp.status();
        if status == ReqStatus::NOT_FOUND {
            return Ok(None);
        }
        if status == ReqStatus::OK {
            let bytes = resp.bytes().map_err(|e| StoreError::Backend {
                message: format!("read body: {}", e),
                retryable: true,
                source: Some(Box::new(e)),
            })?;
            return Ok(Some(bytes.to_vec()));
        }
        Err(StoreError::Backend {
            message: format!("http GET status {}", status),
            retryable: status.is_server_error(),
            source: None,
        })
    }

    fn put(&self, run_id: &str, key: &str, data: &[u8]) -> std::result::Result<(), StoreError> {
        validate_run_id(run_id)?;
        validate_artifact_key(key)?;
        let url = self.url(run_id, key)?;
        let rb = self
            .client
            .put(url)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            )
            .body(data.to_vec());
        let resp = self
            .apply_auth(rb)
            .send()
            .map_err(|e| StoreError::Backend {
                message: format!("http request: {}", e),
                retryable: true,
                source: Some(Box::new(e)),
            })?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        Err(StoreError::Backend {
            message: format!("http PUT status {}", status),
            retryable: status.is_server_error(),
            source: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// `reqwest::blocking` must not run on the async test runtime thread (wiremock + tokio).
    #[tokio::test]
    async fn client_get_maps_200_and_404() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/runs/r1/artifacts"))
            .and(query_param("key", "k"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8, 8]))
            .mount(&mock)
            .await;

        let uri = mock.uri();
        let uri_miss = uri.clone();
        let v = tokio::task::spawn_blocking(move || {
            let store = HttpArtifactStore::try_new(&uri, None).expect("client build");
            store.get("r1", "k").expect("get")
        })
        .await
        .expect("join");
        assert_eq!(v, Some(vec![7, 8]));

        Mock::given(method("GET"))
            .and(path("/v1/runs/r1/artifacts"))
            .and(query_param("key", "missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock)
            .await;

        let v = tokio::task::spawn_blocking(move || {
            let store = HttpArtifactStore::try_new(&uri_miss, None).expect("client build");
            store.get("r1", "missing").expect("get 404")
        })
        .await
        .expect("join");
        assert_eq!(v, None);
    }

    #[tokio::test]
    async fn client_put_sends_octet_stream() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v1/runs/r2/artifacts"))
            .and(query_param("key", "f.bin"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock)
            .await;

        let uri = mock.uri();
        tokio::task::spawn_blocking(move || {
            let store = HttpArtifactStore::try_new(&uri, None).unwrap();
            store.put("r2", "f.bin", b"abc").expect("put");
        })
        .await
        .expect("join");
    }
}
