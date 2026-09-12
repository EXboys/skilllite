use skilllite_core::artifact_store::StoreError;

fn has_windows_drive_prefix(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic()
    )
}

pub(crate) fn validate_run_id(run_id: &str) -> Result<(), StoreError> {
    if run_id.is_empty() {
        return Err(StoreError::InvalidKey {
            key: run_id.to_string(),
            reason: "run_id must be non-empty".to_string(),
        });
    }
    if run_id.contains("..") {
        return Err(StoreError::InvalidKey {
            key: run_id.to_string(),
            reason: "run_id must not contain '..'".to_string(),
        });
    }
    if run_id.contains('/') || run_id.contains('\\') {
        return Err(StoreError::InvalidKey {
            key: run_id.to_string(),
            reason: "run_id must not contain '/' or '\\'".to_string(),
        });
    }
    if has_windows_drive_prefix(run_id) {
        return Err(StoreError::InvalidKey {
            key: run_id.to_string(),
            reason: "run_id must not contain a Windows drive prefix".to_string(),
        });
    }
    if run_id.contains('\0') {
        return Err(StoreError::InvalidKey {
            key: run_id.to_string(),
            reason: "run_id must not contain null bytes".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_run_ids_pass() {
        assert!(validate_run_id("run-1").is_ok());
        assert!(validate_run_id("abc_def.123").is_ok());
    }

    #[test]
    fn separators_and_traversal_rejected() {
        for run_id in [
            "",
            "..",
            "a/b",
            r"a\b",
            r"\Windows\Temp",
            "C:evil",
            r"C:\evil",
        ] {
            let err = validate_run_id(run_id).unwrap_err();
            assert!(
                matches!(err, StoreError::InvalidKey { .. }),
                "expected rejection for {run_id:?}, got {err}"
            );
        }
    }
}
