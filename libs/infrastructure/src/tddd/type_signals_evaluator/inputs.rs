//! Freshness-input helpers for the type-signal evaluator.

use std::io;
use std::path::Path;

use domain::tddd::type_signals_doc::ImplementationInputHash;

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
pub(super) fn hash_workspace_inputs(
    workspace_root: &Path,
    target_crate: &str,
) -> Result<ImplementationInputHash, EvaluateSignalsError> {
    build_inputs::hash_implementation_inputs(workspace_root, target_crate)
        .map(ImplementationInputHash::new)
}

/// Rejects persistence when inputs change while rustdoc or evaluation is running.
pub(super) fn verify_evaluation_inputs_unchanged(
    workspace_root: &Path,
    target_crate: &str,
    catalogue_path: &Path,
    initial_declaration_hash: &domain::CatalogueDeclarationHash,
    initial_implementation_input_hash: &ImplementationInputHash,
) -> Result<(), EvaluateSignalsError> {
    let catalogue =
        read_bytes_file_limited(catalogue_path, super::MAX_CATALOGUE_BYTES).map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot re-read catalogue '{}': {error}",
                catalogue_path.display()
            ))
        })?;
    if type_signals_codec::declaration_hash(&catalogue) != *initial_declaration_hash {
        return Err(EvaluateSignalsError(
            "catalogue changed during type-signal evaluation".to_owned(),
        ));
    }
    if hash_workspace_inputs(workspace_root, target_crate)? != *initial_implementation_input_hash {
        return Err(EvaluateSignalsError(
            "implementation inputs changed during type-signal evaluation".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::read_utf8_file_limited;

    #[test]
    fn test_read_utf8_file_limited_rejects_missing_file() {
        assert!(read_utf8_file_limited(std::path::Path::new("missing"), 1).is_err());
    }
}
