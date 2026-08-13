//! Selected-chain I/O adaptation for `ref-verify check-approved`.
//!
//! This path intentionally bypasses the aggregate scope resolver: a request for
//! one chain must not inspect or validate artifacts belonging to the other.

use std::path::Path;
use std::sync::Arc;

use usecase::ref_verify::{
    CheckApprovedOutcome, RefVerifyChainFilter, RefVerifyCheckApprovedInteractor,
    RefVerifyCheckApprovedOutcome, RefVerifyCheckApprovedService as _, RefVerifyCommand,
    RefVerifyConfig, RefVerifyDriverError, RefVerifyError, RefVerifyPair, RefVerifyPairSourcePort,
    RefVerifyScope,
};

use super::driver_adapter_results::{
    check_partial_catalogue_set, check_track_dir_exists, inspect_chain2_catalogue_set,
    resolve_results_chain2_target_layers,
};
use super::guarded_io::read_guarded_text;
use super::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};

/// Adapt selected-chain filesystem enumeration to the use-case approval policy.
///
/// Unlike the aggregate approval path, this function enumerates the requested
/// chain directly.  Chain1 therefore neither loads nor validates Chain2 rules,
/// catalogues, or caches.
pub(crate) fn check_selected_chain_approved(
    project_root: &Path,
    track_id: domain::TrackId,
    chain: RefVerifyChainFilter,
    current_branch: String,
) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError> {
    let command = RefVerifyCommand {
        track_id,
        // Cache routing comes from each production pair's cache scope.
        scope: RefVerifyScope::All,
        current_branch,
    };
    // Chain2 can otherwise have no declared catalogue and return NoPairs before
    // the pair source checks that the requested track actually exists.  Keep
    // selected approval fail-closed for both chains before any vacant result.
    check_track_dir_exists(project_root, command.track_id.as_ref())?;
    let pair_source = RefVerifyPairSourceAdapter::new(project_root.to_path_buf());
    let config = RefVerifyConfig::default();
    let pairs = load_selected_pairs(project_root, &pair_source, &command, &config, chain)?;
    let pair_source = Arc::new(SelectedPairSource { pairs })
        as Arc<dyn usecase::ref_verify::RefVerifyPairSourcePort>;
    let cache = Arc::new(RefVerifyCacheAdapter::new(project_root.to_path_buf()))
        as Arc<dyn usecase::ref_verify::RefVerifyCachePort>;
    let interactor = RefVerifyCheckApprovedInteractor::new(pair_source, cache);

    let outcome = interactor.check_approved(&command).map_err(|error| {
        RefVerifyDriverError::Unavailable(format!(
            "ref-verify selected check-approved infrastructure error: {error}"
        ))
    })?;
    Ok(match outcome {
        CheckApprovedOutcome::NoPairs => RefVerifyCheckApprovedOutcome::NoPairs,
        CheckApprovedOutcome::AllApproved => RefVerifyCheckApprovedOutcome::AllApproved,
        CheckApprovedOutcome::NotApproved { missing_or_non_pass } => {
            RefVerifyCheckApprovedOutcome::NotApproved { missing_or_non_pass }
        }
    })
}

fn load_selected_pairs(
    project_root: &Path,
    pair_source: &RefVerifyPairSourceAdapter,
    command: &RefVerifyCommand,
    config: &RefVerifyConfig,
    chain: RefVerifyChainFilter,
) -> Result<Vec<RefVerifyPair>, RefVerifyDriverError> {
    match chain {
        RefVerifyChainFilter::Chain1 => {
            load_pairs_for_scope(pair_source, command, config, RefVerifyScope::Chain1)
        }
        RefVerifyChainFilter::Chain2 => {
            load_chain2_pairs(project_root, pair_source, command, config)
        }
        RefVerifyChainFilter::All => Err(RefVerifyDriverError::Wiring(
            "selected-chain approval requires Chain1 or Chain2".to_owned(),
        )),
    }
}

fn load_chain2_pairs(
    project_root: &Path,
    pair_source: &RefVerifyPairSourceAdapter,
    command: &RefVerifyCommand,
    config: &RefVerifyConfig,
) -> Result<Vec<RefVerifyPair>, RefVerifyDriverError> {
    let bindings = load_selected_chain2_tddd_bindings(project_root)?;
    check_selected_chain2_catalogues_require_spec(
        project_root,
        command.track_id.as_ref(),
        &bindings,
    )?;
    let layers = resolve_results_chain2_target_layers(
        &bindings,
        &usecase::ref_verify::RefVerifyLayerFilter::All,
    )?;
    let (present, absent) =
        inspect_chain2_catalogue_set(project_root, command.track_id.as_ref(), &bindings)?;
    check_partial_catalogue_set(&present, &absent)?;

    let mut pairs = Vec::new();
    for layer in layers {
        if !present.contains(&layer) {
            continue;
        }
        pairs.extend(load_pairs_for_scope(
            pair_source,
            command,
            config,
            RefVerifyScope::Chain2 { layer },
        )?);
    }
    Ok(pairs)
}

/// Load the TDDD bindings required by the selected Chain-2 approval gate.
///
/// Unlike informational `results`, this gate cannot treat an absent
/// architecture configuration as an empty selection: doing so would turn an
/// unknown set of required pairs into a vacuous approval.
fn load_selected_chain2_tddd_bindings(
    project_root: &Path,
) -> Result<Vec<crate::verify::tddd_layers::TdddLayerBinding>, RefVerifyDriverError> {
    let rules_path = project_root.join("architecture-rules.json");
    crate::verify::tddd_layers::load_tddd_layers(&rules_path, project_root).map_err(|error| {
        RefVerifyDriverError::Wiring(format!(
            "cannot load TDDD layer bindings for selected Chain-2 approval: {error}"
        ))
    })
}

/// Reject a selected Chain-2 catalogue that exists before a valid required spec.
///
/// The existence probes and guarded read keep the selected gate within the
/// trusted-root boundary.  A present spec must be a regular, decodable file;
/// otherwise an empty catalogue could make the gate approve vacuously.
fn check_selected_chain2_catalogues_require_spec(
    project_root: &Path,
    track_id: &str,
    bindings: &[crate::verify::tddd_layers::TdddLayerBinding],
) -> Result<(), RefVerifyDriverError> {
    let track_dir = project_root.join("track").join("items").join(track_id);
    let spec_path = track_dir.join("spec.json");
    let spec_exists = crate::track::symlink_guard::reject_symlinks_below(&spec_path, project_root)
        .map_err(|error| {
            RefVerifyDriverError::Wiring(format!(
                "cannot inspect selected Chain-2 spec path '{}': {error}",
                spec_path.display()
            ))
        })?;
    if spec_exists {
        let metadata = std::fs::symlink_metadata(&spec_path).map_err(|error| {
            RefVerifyDriverError::Wiring(format!(
                "cannot inspect selected Chain-2 spec file '{}': {error}",
                spec_path.display()
            ))
        })?;
        if !metadata.file_type().is_file() {
            return Err(RefVerifyDriverError::Wiring(format!(
                "selected Chain-2 spec path '{}' is not a regular file",
                spec_path.display()
            )));
        }
        let spec_text = read_guarded_text(&spec_path, project_root).map_err(|error| {
            RefVerifyDriverError::Wiring(format!(
                "cannot read selected Chain-2 spec '{}': {error}",
                spec_path.display()
            ))
        })?;
        crate::spec::codec::decode(&spec_text).map_err(|error| {
            RefVerifyDriverError::Wiring(format!(
                "cannot decode selected Chain-2 spec '{}': {error}",
                spec_path.display()
            ))
        })?;
        return Ok(());
    }

    for binding in bindings {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let catalogue_exists =
            crate::track::symlink_guard::reject_symlinks_below(&catalogue_path, project_root)
                .map_err(|error| {
                    RefVerifyDriverError::Wiring(format!(
                        "cannot inspect selected Chain-2 catalogue path '{}': {error}",
                        catalogue_path.display()
                    ))
                })?;
        if catalogue_exists {
            return Err(RefVerifyDriverError::Wiring(format!(
                "spec.json not found while selected Chain-2 catalogue '{}' exists — SoT Chain ordering violation",
                catalogue_path.display()
            )));
        }
    }

    Ok(())
}

fn load_pairs_for_scope(
    pair_source: &RefVerifyPairSourceAdapter,
    command: &RefVerifyCommand,
    config: &RefVerifyConfig,
    scope: RefVerifyScope,
) -> Result<Vec<RefVerifyPair>, RefVerifyDriverError> {
    let scoped_command = RefVerifyCommand {
        track_id: command.track_id.clone(),
        scope,
        current_branch: command.current_branch.clone(),
    };
    pair_source
        .load_pairs(&scoped_command, config)
        .map_err(|error| RefVerifyDriverError::Usecase(format!("pair source enumeration: {error}")))
}

struct SelectedPairSource {
    pairs: Vec<RefVerifyPair>,
}

impl RefVerifyPairSourcePort for SelectedPairSource {
    fn load_pairs(
        &self,
        _command: &RefVerifyCommand,
        _config: &RefVerifyConfig,
    ) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
        Ok(self.pairs.clone())
    }
}
