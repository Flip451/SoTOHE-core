//! Content and generation identity for the configured resolution inputs.

use std::collections::BTreeSet;
use std::path::Path;

use super::super::EvaluateSignalsError;
use super::{LoadedTrackCatalogues, MAX_RUSTDOC_CONTEXT_EXPORTS, TdddLayerBinding};

/// Returns the distinct layers that will receive a rustdoc context for this
/// target. A selected target is always required; other layers participate only
/// when their catalogue was present in the authoritative resolution snapshot.
pub(super) fn required_context_bindings<'a>(
    configured_layers: &'a [TdddLayerBinding],
    target_layer: &domain::tddd::LayerId,
    loaded: &LoadedTrackCatalogues,
) -> Result<Vec<(domain::tddd::LayerId, &'a TdddLayerBinding)>, EvaluateSignalsError> {
    let mut required = Vec::new();
    let mut seen = BTreeSet::new();
    for configured_binding in configured_layers {
        let layer = domain::tddd::LayerId::try_new(configured_binding.layer_id().to_owned())
            .map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
            })?;
        if &layer != target_layer && !loaded.catalogues.contains_key(&layer) {
            continue;
        }
        if seen.insert(layer.clone()) {
            required.push((layer, configured_binding));
        }
    }
    Ok(required)
}

pub(super) fn validate_context_export_count(
    required: &[(domain::tddd::LayerId, &TdddLayerBinding)],
) -> Result<(), EvaluateSignalsError> {
    if required.len() > MAX_RUSTDOC_CONTEXT_EXPORTS {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "required rustdoc context export count {} exceeds the rustdoc context export budget of {}",
            required.len(),
            MAX_RUSTDOC_CONTEXT_EXPORTS
        )));
    }
    Ok(())
}

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
    let mut catalogue_bearing_layers = 0;
    for binding in bindings {
        input.extend_from_slice(binding.layer_id().as_bytes());
        let path = track_dir.join(binding.catalogue_file());
        super::reject_type_signals_path(&path, trusted_items_root, "catalogue")?;
        let catalogue_present = match read_configured_catalogue(&path, trusted_items_root)? {
            Some(bytes) => {
                encode_snapshot_bytes(&mut input, &path, Some(&bytes))?;
                true
            }
            None => {
                encode_snapshot_bytes(&mut input, &path, None)?;
                false
            }
        };
        if !catalogue_present {
            continue;
        }
        catalogue_bearing_layers += 1;
        if catalogue_bearing_layers > super::MAX_RUSTDOC_CONTEXT_EXPORTS {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "required rustdoc context export count {} exceeds the rustdoc context export budget of {}",
                catalogue_bearing_layers,
                super::MAX_RUSTDOC_CONTEXT_EXPORTS
            )));
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
