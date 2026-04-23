//! Shared HTTP helpers (feature `http`).

use std::time::Duration;

use reqwest::blocking::Client;

use crate::error::{Error, Result};

/// Builds a blocking client with conservative defaults for webhook-style calls.
pub(crate) fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("skilllite-channel/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| Error::http(e.to_string()))
}
