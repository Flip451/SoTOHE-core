//! Freshness-input helpers for the type-signal evaluator.

use std::io;
use std::path::Path;

use sha2::Digest;

use domain::tddd::type_signals_doc::{
    BaselineHash, EvaluatorContractHash, ImplementationInputHash, RustdocExtractionContractHash,
    Sha256Digest, TypeSignalsCurrentInputs,
};

use super::{EvaluateSignalsError, MAX_RUSTDOC_SNAPSHOT_BYTES, build_inputs};
use crate::tddd::type_signals_codec;

/// Hashes bytes produced by a trusted local computation into a validated
/// freshness identity. Raw persisted values are validated by the codec; values
/// produced here are SHA-256 output and therefore canonical by construction.
pub(super) fn digest_identity<T>(
    bytes: &[u8],
    wrap: impl FnOnce(Sha256Digest) -> T,
) -> Result<T, EvaluateSignalsError> {
    let digest = sha2::Sha256::digest(bytes);
    Sha256Digest::try_new(format!("{digest:x}")).map(wrap).map_err(|error| {
        EvaluateSignalsError(format!("failed to construct SHA-256 digest: {error}"))
    })
}

pub(super) fn evaluator_contract_hash() -> Result<EvaluatorContractHash, EvaluateSignalsError> {
    embedded_contract_digest(
        "evaluator",
        env!("SOTP_EVALUATOR_CONTRACT_DIGEST"),
        EvaluatorContractHash::new,
    )
}

pub(super) fn rustdoc_extraction_contract_hash()
-> Result<RustdocExtractionContractHash, EvaluateSignalsError> {
    embedded_contract_digest(
        "rustdoc extraction",
        env!("SOTP_RUSTDOC_EXTRACTION_CONTRACT_DIGEST"),
        RustdocExtractionContractHash::new,
    )
}

fn embedded_contract_digest<T>(
    contract_name: &str,
    digest: &str,
    wrap: impl FnOnce(Sha256Digest) -> T,
) -> Result<T, EvaluateSignalsError> {
    Sha256Digest::try_new(digest.to_owned()).map(wrap).map_err(|error| {
        EvaluateSignalsError(format!(
            "build-time {contract_name} contract digest is invalid: {error}"
        ))
    })
}

pub(super) fn read_utf8_file_limited(
    path: &Path,
    maximum_bytes: usize,
) -> Result<String, io::Error> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > maximum_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("file exceeds maximum size of {maximum_bytes} bytes: {} bytes", metadata.len()),
        ));
    }

    let bytes = std::fs::read(path)?;
    if bytes.len() > maximum_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "file exceeds maximum size of {maximum_bytes} bytes after read: {} bytes",
                bytes.len()
            ),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("file is not valid UTF-8: {error}"))
    })
}

/// Hashes the complete resolved build-input closure for one rustdoc target.
///
/// # Errors
///
/// Returns an error whenever the closure cannot be resolved or normalized, so
/// callers take the re-extraction lane rather than reusing a stale snapshot.
pub(super) fn hash_workspace_inputs(
    workspace_root: &Path,
    target_crate: &str,
    wrap: impl FnOnce(Sha256Digest) -> ImplementationInputHash,
) -> Result<ImplementationInputHash, EvaluateSignalsError> {
    build_inputs::hash_resolved_build_inputs(workspace_root, target_crate).map(wrap)
}

/// Rejects persistence when an input changes while rustdoc or semantic
/// evaluation is in flight. Persisting post-evaluation hashes would associate
/// a snapshot produced from old inputs with new identities, allowing a later
/// stale snapshot skip.
pub(super) fn verify_evaluation_inputs_unchanged(
    workspace_root: &Path,
    target_crate: &str,
    catalogue_path: &Path,
    baseline_path: &Path,
    initial: &TypeSignalsCurrentInputs,
) -> Result<(), EvaluateSignalsError> {
    let current_catalogue = std::fs::read(catalogue_path).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot re-read catalogue freshness input '{}': {error}",
            catalogue_path.display()
        ))
    })?;
    let current_declaration_hash = type_signals_codec::declaration_hash(&current_catalogue);
    let current_baseline = read_utf8_file_limited(baseline_path, MAX_RUSTDOC_SNAPSHOT_BYTES)
        .map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot re-read baseline freshness input '{}': {error}",
                baseline_path.display()
            ))
        })?;
    let current_baseline_hash = digest_identity(current_baseline.as_bytes(), BaselineHash::new)?;
    let current_implementation_input_hash =
        hash_workspace_inputs(workspace_root, target_crate, ImplementationInputHash::new)?;

    let mut changed = Vec::new();
    if current_declaration_hash != *initial.declaration_hash() {
        changed.push("catalogue");
    }
    if current_baseline_hash != *initial.baseline_hash() {
        changed.push("baseline");
    }
    if current_implementation_input_hash != *initial.implementation_input_hash() {
        changed.push("workspace build inputs");
    }
    if changed.is_empty() {
        Ok(())
    } else {
        Err(EvaluateSignalsError(format!(
            "freshness inputs changed during evaluation ({}); refusing to persist signals",
            changed.join(", ")
        )))
    }
}
