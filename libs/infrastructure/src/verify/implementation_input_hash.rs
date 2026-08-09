//! Workspace implementation-input hash used by local signal freshness checks.

use std::io::Read as _;
use std::path::Path;

use domain::tddd::catalogue_v2::TdddLayerBindingsPort;
use domain::tddd::type_signals_doc::ImplementationInputHash;
use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter;
use crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use crate::tddd::type_signals_codec;
use crate::tddd::type_signals_evaluator::{build_inputs, inputs};
use crate::track::symlink_guard;

use usecase::tddd_feature_declaration::{
    TdddActualFeatureDeclarationPort, TdddActualFeatureDeclarationPortError,
};

pub(crate) const MAX_RUSTDOC_JSON_BYTES: usize = 64 * 1024 * 1024;

fn type_signal_file_name(catalogue_file: &str) -> String {
    let stem = catalogue_file.strip_suffix(".json").unwrap_or(catalogue_file);
    let signal_stem = stem
        .strip_suffix('s')
        .map_or_else(|| format!("{stem}-signals"), |trimmed| format!("{trimmed}-signals"));
    format!("{signal_stem}.json")
}

pub(crate) fn current_implementation_input_hash(
    workspace_root: &Path,
    signals_path: &Path,
    baseline_path: Option<&Path>,
) -> Result<Option<ImplementationInputHash>, String> {
    let Some(track_dir) = signals_path.parent() else {
        return Err("signals path has no track directory".to_owned());
    };
    let baseline_info = match baseline_path {
        Some(path) => {
            if path.parent() != Some(track_dir) {
                return Err(
                    "signals and baseline paths must belong to the same track directory".to_owned()
                );
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "baseline path has no valid filename".to_owned())?;
            Some((path, file_name))
        }
        None => None,
    };
    let signal_file = signals_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "signals path has no valid filename".to_owned())?;

    // The implementation-input comparison is the only axis that depends on
    // the local nightly authority. Probe it before collecting the committed
    // components so a successful enumeration with no installed nightly can
    // narrow exactly that comparison without turning the authority's absence
    // into a generic hash failure.
    let toolchain_identifier = match build_inputs::probe_nightly_toolchain(workspace_root)
        .map_err(|error| error.to_string())?
    {
        build_inputs::NightlyToolchainProbe::Installed(identity) => identity,
        build_inputs::NightlyToolchainProbe::Absent => return Ok(None),
    };

    let bindings = FsTdddLayerBindingsAdapter::new()
        .load(workspace_root, None)
        .map_err(|error| error.to_string())?;
    let mut matching_bindings = bindings
        .iter()
        .filter(|binding| type_signal_file_name(&binding.catalogue_file) == signal_file);
    let binding = match (matching_bindings.next(), matching_bindings.next()) {
        (Some(binding), None) => binding,
        (None, _) => {
            return Err(format!("no enabled TDDD layer matches signal file '{signal_file}'"));
        }
        (Some(_), Some(_)) => {
            return Err(format!("multiple enabled TDDD layers match signal file '{signal_file}'"));
        }
    };
    let required_layers = bindings
        .iter()
        .map(|binding| {
            domain::tddd::LayerId::try_new(binding.layer_id.clone())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some((path, baseline_file)) = baseline_info {
        let expected_baseline_file = &binding.baseline_file;
        if expected_baseline_file != baseline_file {
            return Err(format!(
                "baseline path '{}' does not match expected baseline file '{}' for signal file \
                 '{}'",
                path.display(),
                expected_baseline_file,
                signal_file
            ));
        }
    }
    let layer = domain::tddd::LayerId::try_new(binding.layer_id.clone())
        .map_err(|error| error.to_string())?;
    let feature_declaration = match FsTdddFeatureDeclarationAdapter::new().load_for_actual(
        track_dir,
        workspace_root,
        &bindings,
    ) {
        Ok(declaration) => declaration,
        Err(TdddActualFeatureDeclarationPortError::MissingBaselineSnapshot { .. }) => {
            // A fresh checkout has no local feature-selection snapshot. The
            // actual-declaration adapter has already validated the committed
            // declaration before reporting this structural absence, so reuse
            // that committed declaration without creating local state.
            let declaration_path = track_dir.join("tddd-features.json");
            match symlink_guard::reject_symlinks_below(&declaration_path, workspace_root) {
                Ok(true) => {}
                Ok(false) => return Err("feature declaration is absent".to_owned()),
                Err(error) => {
                    return Err(format!(
                        "cannot inspect feature declaration {}: {error}",
                        declaration_path.display()
                    ));
                }
            }
            let declaration_bytes = read_bytes_file_limited(
                &declaration_path,
                super::branch_implementation_inputs::MAX_DECLARATION_BYTES,
            )
            .map_err(|error| {
                format!("cannot read feature declaration {}: {error}", declaration_path.display())
            })?;
            let declaration_label = declaration_path.display().to_string();
            super::branch_implementation_inputs::parse_feature_declaration(
                &declaration_bytes,
                &declaration_label,
                &required_layers,
            )
            .map_err(|error| format!("cannot parse feature declaration: {error}"))?
        }
        Err(error) => return Err(format!("feature-selection baseline: {error}")),
    };
    let features = feature_declaration.features_for(&layer).map_err(|error| error.to_string())?;
    let target_crate = match binding.targets.as_slice() {
        [target] => target.as_str(),
        _ => return Err("type-signal layers require exactly one rustdoc target".to_owned()),
    };
    inputs::hash_workspace_inputs_with_toolchain_identifier(
        workspace_root,
        target_crate,
        features,
        &toolchain_identifier,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

fn read_bytes_file_limited(path: &Path, maximum_bytes: usize) -> Result<Vec<u8>, std::io::Error> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > maximum_bytes as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }

    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((maximum_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }
    Ok(bytes)
}

/// Verifies the cached document's baseline and implementation-input hashes
/// against the LOCAL authorities, when those authorities exist.
///
/// Authority availability is independent across the two axes: a structurally
/// absent type-baseline skips only the `baseline_hash` comparison. The
/// implementation-input hash is recomputed from the committed feature
/// declaration when the local nightly authority is installed; when its local
/// snapshot exists, the declaration adapter also verifies that snapshot before
/// returning the features. A missing snapshot is therefore a structural
/// downgrade, while any present but unreadable or mismatched authority remains
/// fail-closed. A successfully enumerated environment with no installed
/// nightly skips only the implementation-input comparison.
///
/// Returns `Some(outcome)` with error findings on a fail-closed violation,
/// `None` when every runnable check passed (or was skipped for absence).
pub(crate) fn verify_freshness_against_local_authorities(
    doc: &domain::TypeSignalsDocument,
    signals_path: &Path,
    normalized_baseline: Option<&Path>,
    workspace_root: &Path,
) -> Option<VerifyOutcome> {
    if let Some(baseline_path) = normalized_baseline {
        let baseline_bytes = match read_bytes_file_limited(baseline_path, MAX_RUSTDOC_JSON_BYTES) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Some(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "cannot read {}: {error}",
                    baseline_path.display()
                ))]));
            }
        };
        let current_baseline_hash = type_signals_codec::baseline_hash(&baseline_bytes);
        if *doc.cache_key().baseline_hash() != current_baseline_hash {
            return Some(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: baseline_hash mismatch (recorded={}, current={}) — \
                 re-run `sotp signal calc-impl-catalog` to refresh the evaluation result",
                signals_path.display(),
                doc.cache_key().baseline_hash().as_digest().as_str(),
                current_baseline_hash.as_digest().as_str()
            ))]));
        }
    }

    let current_implementation_hash = match current_implementation_input_hash(
        workspace_root,
        signals_path,
        normalized_baseline,
    ) {
        Ok(Some(hash)) => hash,
        Ok(None) => return None,
        Err(error) => {
            return Some(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot determine current implementation-input hash for {}: {error}",
                signals_path.display()
            ))]));
        }
    };
    if *doc.cache_key().implementation_input_hash() != current_implementation_hash {
        return Some(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: implementation_input_hash mismatch (recorded={}, current={}) — \
             re-run `sotp signal calc-impl-catalog` to refresh the evaluation result",
            signals_path.display(),
            doc.cache_key().implementation_input_hash().as_digest().as_str(),
            current_implementation_hash.as_digest().as_str()
        ))]));
    }
    None
}
