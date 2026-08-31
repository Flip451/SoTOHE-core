//! Content and generation identity for the configured resolution inputs.

use std::path::Path;

use super::super::EvaluateSignalsError;

/// Hashes the exact rules, feature declarations, catalogues, and baselines
/// selected by the architecture configuration.
pub(crate) fn resolution_input_fingerprint(
    workspace_root: &Path,
    track_dir: &Path,
    trusted_items_root: &Path,
) -> Result<domain::CatalogueDeclarationHash, EvaluateSignalsError> {
    let mut input = Vec::new();
    let rules_path = workspace_root.join("architecture-rules.json");
    match crate::track::symlink_guard::reject_symlinks_below(&rules_path, workspace_root) {
        Ok(true) => {}
        Ok(false) => {
            return Err(EvaluateSignalsError::authoritative_input(
                "architecture-rules.json not found".to_owned(),
            ));
        }
        Err(error) => {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "symlink guard rejected architecture-rules.json '{}': {error}",
                rules_path.display()
            )));
        }
    }
    let rules = read_workspace_file(&rules_path, workspace_root, "architecture-rules.json")?
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(
                "architecture-rules.json not found".to_owned(),
            )
        })?;
    encode_snapshot_bytes(&mut input, &rules_path, Some(&rules))?;
    let rules_text = std::str::from_utf8(&rules).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "architecture-rules.json is not UTF-8: {error}"
        ))
    })?;
    let bindings = crate::verify::tddd_layers::parse_tddd_layers(rules_text).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot parse architecture-rules.json: {error}"
        ))
    })?;
    if bindings.len() > super::MAX_RUSTDOC_CONTEXT_EXPORTS {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "configured TDDD layer count {} exceeds the rustdoc context export budget of {}",
            bindings.len(),
            super::MAX_RUSTDOC_CONTEXT_EXPORTS
        )));
    }
    for file_name in [
        super::super::TDDD_FEATURE_DECLARATION_FILE,
        super::super::TDDD_FEATURE_DECLARATION_SNAPSHOT_FILE,
    ] {
        let path = track_dir.join(file_name);
        super::reject_type_signals_path(&path, trusted_items_root, file_name)?;
        match read_workspace_file(&path, trusted_items_root, file_name)? {
            Some(bytes) => encode_snapshot_bytes(&mut input, &path, Some(&bytes))?,
            None => encode_snapshot_bytes(&mut input, &path, None)?,
        }
    }
    for binding in bindings {
        input.extend_from_slice(binding.layer_id().as_bytes());
        let path = track_dir.join(binding.catalogue_file());
        super::reject_type_signals_path(&path, trusted_items_root, "catalogue")?;
        match read_configured_catalogue(&path, trusted_items_root)? {
            Some(bytes) => encode_snapshot_bytes(&mut input, &path, Some(&bytes))?,
            None => encode_snapshot_bytes(&mut input, &path, None)?,
        }
        let baseline_path = track_dir.join(binding.baseline_file());
        super::reject_type_signals_path(&baseline_path, trusted_items_root, "baseline")?;
        let (_, baseline_hash) = super::read_actual_baseline(&baseline_path, trusted_items_root)?;
        encode_snapshot_bytes(
            &mut input,
            &baseline_path,
            Some(baseline_hash.as_digest().as_str().as_bytes()),
        )?;
    }
    Ok(crate::tddd::type_signals_codec::declaration_hash(&input))
}

/// Reads one configured catalogue only when it is present.
pub(crate) fn read_configured_catalogue(
    path: &Path,
    trusted_items_root: &Path,
) -> Result<Option<Vec<u8>>, EvaluateSignalsError> {
    read_workspace_file(path, trusted_items_root, "catalogue")
}

pub(crate) fn read_workspace_file(
    path: &Path,
    trusted_root: &Path,
    label: &str,
) -> Result<Option<Vec<u8>>, EvaluateSignalsError> {
    crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
        path,
        Some(trusted_root),
        super::super::MAX_CATALOGUE_BYTES as u64,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read {label} '{}': {error}",
            path.display()
        ))
    })
}

pub(crate) fn encode_snapshot_bytes(
    input: &mut Vec<u8>,
    path: &Path,
    bytes: Option<&[u8]>,
) -> Result<(), EvaluateSignalsError> {
    match bytes {
        None => input.push(0),
        Some(bytes) => {
            let metadata = std::fs::symlink_metadata(path).map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!(
                    "cannot snapshot resolution input '{}': {error}",
                    path.display()
                ))
            })?;
            if !metadata.is_file() {
                return Err(EvaluateSignalsError::authoritative_input(format!(
                    "resolution input '{}' is not a regular file",
                    path.display()
                )));
            }
            input.push(1);
            input.extend_from_slice(&metadata.len().to_be_bytes());
            input.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            input.extend_from_slice(bytes);
        }
    }
    Ok(())
}
