use skilllite_core::artifact_store::StoreError;

pub(crate) fn validate_run_id(run_id: &str) -> Result<(), StoreError> {
    if run_id.is_empty() || run_id.contains("..") || run_id.contains('/') {
        return Err(StoreError::InvalidKey {
            key: run_id.to_string(),
            reason: "run_id must be non-empty and must not contain '..' or '/'".to_string(),
        });
    }
    Ok(())
}
