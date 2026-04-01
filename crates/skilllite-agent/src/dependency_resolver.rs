//! LLM-based dependency resolution for skilllite-agent.
//!
//! This module extends skilllite-core's dependency_resolver with LLM-based package extraction.
//! Implements the LlmProvider trait to enable async resolution.

#[cfg(feature = "async-resolve")]
use skilllite_core::skill::dependency_resolver::{
    resolve_packages as core_resolve_packages, LlmProvider,
};

use crate::llm::LlmClient;
use std::path::Path;

#[cfg(feature = "async-resolve")]
mod llm_resolver {
    use super::*;
    use crate::types::ChatMessage;
    use async_trait::async_trait;

    /// Implement LlmProvider for skilllite-agent's LlmClient.
    #[async_trait]
    impl LlmProvider for LlmClient {
        async fn extract_packages(&self, model: &str, prompt: &str) -> Option<String> {
            let messages = vec![ChatMessage::user(prompt)];
            let resp = self
                .chat_completion(model, &messages, None, Some(0.0))
                .await
                .ok()?;
            let text = resp.choices.first()?.message.content.as_ref()?.clone();
            Some(text)
        }
    }

    /// Full async resolution: Lock → LLM → Whitelist.
    pub async fn resolve_packages(
        skill_dir: &Path,
        compatibility: Option<&str>,
        language: &str,
        llm_client: Option<&LlmClient>,
        model: Option<&str>,
        allow_unknown: bool,
    ) -> crate::Result<skilllite_core::skill::dependency_resolver::ResolvedDependencies> {
        Ok(core_resolve_packages(
            skill_dir,
            compatibility,
            language,
            llm_client,
            model,
            allow_unknown,
        )
        .await?)
    }
}

#[cfg(not(feature = "async-resolve"))]
mod llm_resolver {
    use super::*;

    /// Stub for non-async builds.
    pub async fn resolve_packages(
        _skill_dir: &Path,
        _compatibility: Option<&str>,
        _language: &str,
        _llm_client: Option<&LlmClient>,
        _model: Option<&str>,
        _allow_unknown: bool,
    ) -> crate::Result<skilllite_core::skill::dependency_resolver::ResolvedDependencies> {
        crate::error::bail!("async-resolve feature not enabled")
    }
}

pub use llm_resolver::resolve_packages;
