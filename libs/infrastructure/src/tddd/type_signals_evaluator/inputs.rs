//! Freshness-input helpers for the type-signal evaluator.

use std::io;
use std::path::Path;

use domain::tddd::CargoFeatureName;
use domain::tddd::type_signals_doc::{ImplementationInputHash, Sha256Digest, TypeSignalsCacheKey};
use sha2::Digest as _;

use super::{EvaluateSignalsError, build_inputs};
use crate::tddd::type_signals_codec;

pub(super) fn read_bytes_file_limited(
    path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, io::Error> {
    use std::io::Read as _;

    let metadata = std::fs::metadata(path)?;
    if metadata.len() > maximum_bytes as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file exceeds maximum size"));
    }
    // The take-bound caps the allocation even if the file grows between the
    // stat above and this read; reading one extra byte detects that race.
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((maximum_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file exceeds maximum size"));
    }
    Ok(bytes)
}

pub(super) fn read_utf8_file_limited(
    path: &Path,
    maximum_bytes: usize,
) -> Result<String, io::Error> {
    let bytes = read_bytes_file_limited(path, maximum_bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Computes the current implementation-side input hash for one layer.
pub(crate) fn hash_workspace_inputs(
    workspace_root: &Path,
    target_crate: &str,
    features: &[CargoFeatureName],
) -> Result<ImplementationInputHash, EvaluateSignalsError> {
    let toolchain_identifier = build_inputs::nightly_toolchain_identifier(workspace_root)?;
    hash_workspace_inputs_with_toolchain_identifier(
        workspace_root,
        target_crate,
        features,
        &toolchain_identifier,
    )
}

pub(crate) fn hash_workspace_inputs_with_toolchain_identifier(
    workspace_root: &Path,
    target_crate: &str,
    features: &[CargoFeatureName],
    toolchain_identifier: &[u8],
) -> Result<ImplementationInputHash, EvaluateSignalsError> {
    let implementation_hash = build_inputs::hash_implementation_inputs_with_toolchain_identifier(
        workspace_root,
        target_crate,
        toolchain_identifier,
    )?;
    implementation_hash_with_feature_selection(implementation_hash, features)
}

pub(crate) fn implementation_hash_with_feature_selection(
    implementation_hash: Sha256Digest,
    features: &[CargoFeatureName],
) -> Result<ImplementationInputHash, EvaluateSignalsError> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"type-signals-feature-selection-v1\0");
    hasher.update(implementation_hash.as_str().as_bytes());
    for feature in features {
        hasher.update(feature.as_str().as_bytes());
        hasher.update([0]);
    }
    let digest = Sha256Digest::try_new(format!("{:x}", hasher.finalize())).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "failed to construct implementation-input digest: {error}"
        ))
    })?;
    Ok(ImplementationInputHash::new(digest))
}

/// Rejects persistence when inputs change while rustdoc or evaluation is running.
pub(super) fn verify_evaluation_inputs_unchanged(
    workspace_root: &Path,
    target_crate: &str,
    features: &[CargoFeatureName],
    catalogue_path: &Path,
    baseline_path: &Path,
    initial_key: &TypeSignalsCacheKey,
) -> Result<(), EvaluateSignalsError> {
    if hash_workspace_inputs(workspace_root, target_crate, features)?
        != *initial_key.implementation_input_hash()
    {
        return Err(EvaluateSignalsError::authoritative_input(
            "implementation inputs changed during type-signal evaluation".to_owned(),
        ));
    }
    let catalogue =
        read_bytes_file_limited(catalogue_path, super::MAX_CATALOGUE_BYTES).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot re-read catalogue '{}': {error}",
                catalogue_path.display()
            ))
        })?;
    if type_signals_codec::declaration_hash(&catalogue) != *initial_key.declaration_hash() {
        return Err(EvaluateSignalsError::authoritative_input(
            "catalogue changed during type-signal evaluation".to_owned(),
        ));
    }
    let baseline =
        read_bytes_file_limited(baseline_path, super::MAX_RUSTDOC_JSON_BYTES).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot re-read baseline '{}': {error}",
                baseline_path.display()
            ))
        })?;
    if type_signals_codec::baseline_hash(&baseline) != *initial_key.baseline_hash() {
        return Err(EvaluateSignalsError::authoritative_input(
            "baseline changed during type-signal evaluation".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{implementation_hash_with_feature_selection, read_utf8_file_limited};
    use domain::tddd::CargoFeatureName;
    use domain::tddd::type_signals_doc::Sha256Digest;

    #[test]
    fn test_read_utf8_file_limited_rejects_missing_file() {
        assert!(read_utf8_file_limited(std::path::Path::new("missing"), 1).is_err());
    }

    #[test]
    fn test_implementation_hash_changes_when_feature_selection_changes() {
        let base = Sha256Digest::try_new("0".repeat(64)).unwrap();
        let no_features = implementation_hash_with_feature_selection(base.clone(), &[]).unwrap();
        let semantic_dup = implementation_hash_with_feature_selection(
            base,
            &[CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()],
        )
        .unwrap();

        assert_ne!(
            no_features, semantic_dup,
            "a changed rustdoc feature selection must force a fresh type-signal extraction"
        );
    }
}
