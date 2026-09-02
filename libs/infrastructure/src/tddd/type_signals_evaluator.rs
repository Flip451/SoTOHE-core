//! Per-layer type-signal evaluation with conservative rustdoc reuse.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[path = "type_signals_evaluator/feature_selection.rs"]
mod feature_selection;
#[path = "type_signals_evaluator/freshness.rs"]
pub(crate) mod freshness;
#[allow(dead_code)]
#[path = "type_signals_evaluator/inputs.rs"]
pub(crate) mod inputs;
#[path = "type_signals_evaluator/rustdoc_contexts.rs"]
mod rustdoc_contexts;
#[path = "type_signals_evaluator/signal_builder.rs"]
mod signal_builder;
#[path = "type_signals_evaluator/signal_tags.rs"]
pub(crate) mod signal_tags;

#[cfg(all(test, feature = "test-helpers"))]
use domain::tddd::AuthoritativeRustdocContext;
use domain::tddd::catalogue_v2::CrateName;
#[cfg(all(test, feature = "test-helpers"))]
use domain::tddd::type_signals_doc::BaselineHash;
#[cfg(all(test, feature = "test-helpers"))]
use domain::tddd::type_signals_doc::{
    AttestedRustdocSnapshot, CargoProfileName, ExpectedRustdocJsonPath,
    ResolvedCargoTargetDirectory, RustdocExecutionIdentity, construct_attested_rustdoc_snapshot,
};
use domain::tddd::type_signals_doc::{ResolutionFingerprint, TypeSignalsCacheKey};
use domain::tddd::{CargoFeatureName, LayerId};
use domain::{FreeText, TrackId};
use feature_selection::{load_layer_feature_selections, resolve_execution_identities};
use freshness::{RustdocProvider, decide_reuse_for_recorded_document};
use inputs::{read_head_commit, read_utf8_file_limited_under_root, worktree_is_clean};
pub(crate) use rustdoc_contexts::RustdocContextCache;
use rustdoc_contexts::resolution_input_fingerprint;
use rustdoc_contexts::{
    RustdocContextCacheKey, assemble_rustdoc_contexts_from_snapshot,
    current_implementation_fingerprint, evaluate_and_write_with_contexts,
    load_authoritative_inputs, map_rustdoc_capture_error, read_configured_catalogue,
    reject_type_signals_path, required_context_bindings, validate_context_export_count,
};
use signal_builder::{build_type_signal_identity_index, build_type_signals_from_report};
use signal_tags::{contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag};

#[cfg(feature = "test-helpers")]
pub use crate::tddd::type_signals_executor_adapter::RustdocLaunchObserver;

#[cfg(test)]
static PROCESS_ENVIRONMENT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(all(test, unix))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
fn write_test_executable(path: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::write(path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(all(test, unix))]
fn write_test_rustup(commands: &Path) {
    write_test_executable(
        &commands.join("rustup"),
        r#"#!/bin/sh
if [ "$1" = "run" ] && [ "$2" = "nightly" ] && [ "$3" = "rustc" ]; then
    exit 0
fi
if [ "$1" = "which" ] && [ "$2" = "--toolchain" ] && [ "$3" = "nightly" ]; then
    case "$4" in
        cargo|rustc|rustdoc)
            toolchain="${SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR:-$(dirname "$0")}"
            printf '%s/%s\n' "$toolchain" "$4"
            exit 0
            ;;
    esac
fi
exit 1
"#,
    );
}

#[cfg(all(test, unix))]
fn write_metadata_test_toolchain(commands: &Path) {
    write_test_executable(
        &commands.join("cargo"),
        r#"#!/bin/sh
if [ -n "$SOTOHE_TEST_CARGO_METADATA" ]; then
    exec /bin/cat "$SOTOHE_TEST_CARGO_METADATA"
fi
target_directory="${CARGO_TARGET_DIR:-$PWD/target}"
printf '{"packages":[],"target_directory":"%s"}\n' "$target_directory"
"#,
    );
    write_test_executable(&commands.join("rustc"), "#!/bin/sh\nexit 0\n");
    write_test_executable(&commands.join("rustdoc"), "#!/bin/sh\nexit 0\n");
    write_test_rustup(commands);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]
pub(crate) fn with_process_environment_lock<T>(action: impl FnOnce() -> T) -> T {
    let _environment_guard = match PROCESS_ENVIRONMENT_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    #[cfg(unix)]
    {
        let commands = tempfile::tempdir().unwrap();
        let nightly = commands.path().join("nightly");
        std::fs::create_dir_all(&nightly).unwrap();
        for tool in ["cargo", "rustc", "rustdoc"] {
            std::fs::write(nightly.join(tool), format!("fixture nightly {tool}\n")).unwrap();
        }
        write_metadata_test_toolchain(commands.path());
        let mut path_entries = vec![commands.path().to_path_buf()];
        path_entries.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        let path = std::env::join_paths(path_entries).unwrap();
        temp_env::with_vars(
            [
                ("CARGO_HOME", None::<&std::ffi::OsStr>),
                ("RUSTC", None::<&std::ffi::OsStr>),
                ("RUSTDOC", None::<&std::ffi::OsStr>),
                ("RUSTC_WRAPPER", None::<&std::ffi::OsStr>),
                ("RUSTC_WORKSPACE_WRAPPER", None::<&std::ffi::OsStr>),
                ("RUSTUP_HOME", None::<&std::ffi::OsStr>),
                ("RUSTUP_TOOLCHAIN", None::<&std::ffi::OsStr>),
                ("SOTOHE_TEST_CARGO_METADATA", None::<&std::ffi::OsStr>),
                ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(nightly.as_os_str())),
                ("PATH", Some(path.as_os_str())),
            ],
            action,
        )
    }
    #[cfg(not(unix))]
    {
        temp_env::with_vars(
            [
                ("CARGO_HOME", None::<&str>),
                ("RUSTC", None::<&str>),
                ("RUSTDOC", None::<&str>),
                ("RUSTC_WRAPPER", None::<&str>),
                ("RUSTC_WORKSPACE_WRAPPER", None::<&str>),
                ("RUSTUP_TOOLCHAIN", None::<&str>),
            ],
            action,
        )
    }
}

#[cfg(all(test, feature = "test-helpers"))]
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::rustdoc_crate_adapter::RustdocCrateAdapter;
use crate::tddd::type_signals_codec;
use crate::verify::tddd_layers::TdddLayerBinding;

#[cfg(all(test, feature = "test-helpers"))]
use rustdoc_contexts::{BaselineSnapshot, load_track_catalogues, read_actual_baseline};

const MAX_TYPE_SIGNALS_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUSTDOC_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_CATALOGUE_BYTES: usize = 16 * 1024 * 1024;
const TDDD_FEATURE_DECLARATION_FILE: &str = "tddd-features.json";
const TDDD_FEATURE_DECLARATION_SNAPSHOT_FILE: &str = "tddd-features-baseline.json";

type ResolutionPathsObserver<'a> =
    &'a dyn Fn(&HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>);
type EncodedCrateObserver<'a> = &'a dyn Fn(&domain::tddd::ExtendedCrate);
type ReuseObserver<'a> = &'a dyn Fn();

#[cfg(test)]
#[cfg(feature = "test-helpers")]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_arguments)]
mod evaluator_test_support {
    use super::*;
    use domain::tddd::type_signals_doc::{ImplementationFingerprint, Sha256Digest};

    pub(super) fn snapshot_for_context_test(
        crate_name: &str,
        crate_data: &rustdoc_types::Crate,
    ) -> AttestedRustdocSnapshot {
        fn decode(
            bytes: &[u8],
        ) -> Result<rustdoc_types::Crate, domain::tddd::catalogue_v2::RustdocCratePortError>
        {
            serde_json::from_slice(bytes).map_err(|error| {
                domain::tddd::catalogue_v2::RustdocCratePortError::ParseFailed {
                    crate_name: CrateName::new("test").unwrap(),
                    reason: FreeText::new(error.to_string()),
                }
            })
        }
        let target = ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(
            "/tmp/sotohe-evaluator-test-target",
        ))
        .unwrap();
        let expected = ExpectedRustdocJsonPath::try_new(
            target.as_path().join(format!("{crate_name}.json")),
            &target,
        )
        .unwrap();
        let identity = RustdocExecutionIdentity::new(
            target,
            domain::tddd::catalogue_v2::CrateName::new(crate_name).unwrap(),
            vec![],
            CargoProfileName::try_new("dev".to_owned()).unwrap(),
            expected,
        )
        .unwrap();
        let bytes = serde_json::to_vec(crate_data).unwrap();
        construct_attested_rustdoc_snapshot(
            ImplementationFingerprint::new(Sha256Digest::try_new("a".repeat(64)).unwrap()),
            identity,
            &bytes,
            decode,
        )
        .unwrap()
    }

    #[cfg(all(test, feature = "test-helpers"))]
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn legacy_cache_key(
        declaration_hash: domain::CatalogueDeclarationHash,
        head_commit: domain::CommitHash,
        baseline_hash: domain::BaselineHash,
    ) -> TypeSignalsCacheKey {
        let target = ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(
            "/tmp/sotohe-evaluator-test-target",
        ))
        .unwrap();
        let expected =
            ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/legacy.json"), &target)
                .unwrap();
        let identity = RustdocExecutionIdentity::new(
            target,
            CrateName::new("legacy").unwrap(),
            vec![],
            CargoProfileName::try_new("dev".to_owned()).unwrap(),
            expected,
        )
        .unwrap();
        let zero = Sha256Digest::try_new("0".repeat(64)).unwrap();
        TypeSignalsCacheKey::new(
            declaration_hash,
            head_commit,
            baseline_hash,
            ImplementationFingerprint::new(zero.clone()),
            ResolutionFingerprint::new(zero),
            identity,
        )
    }

    #[cfg(all(test, feature = "test-helpers"))]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_and_write(
        catalogue_bytes: &[u8],
        catalogue_path: &Path,
        track_dir: &Path,
        workspace_root: &Path,
        trusted_items_root: &Path,
        binding: &TdddLayerBinding,
        rustdoc_json: String,
        head_commit: domain::CommitHash,
        baseline_path: &Path,
        baseline_json: &str,
        baseline_hash: BaselineHash,
    ) -> Result<ExitCode, EvaluateSignalsError> {
        super::with_process_environment_lock(|| {
            let baseline = BaselineRustdocCodec::from_json(baseline_json).map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!(
                    "cannot decode baseline: {error}"
                ))
            })?;
            let current = BaselineRustdocCodec::from_json(&rustdoc_json).map_err(|error| {
                EvaluateSignalsError::evaluation(format!("cannot decode rustdoc JSON: {error}"))
            })?;
            let target_layer =
                LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
                    EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
                })?;
            let configured_layers =
                crate::verify::tddd_layers::load_tddd_layers_from_workspace(workspace_root)
                    .map_err(|error| {
                        EvaluateSignalsError::authoritative_input(format!(
                            "cannot load TDDD layer bindings: {error}"
                        ))
                    })?;
            let mut rustdoc_contexts = std::collections::BTreeMap::new();
            let mut baseline_snapshots = std::collections::BTreeMap::new();
            for configured_binding in &configured_layers {
                let layer = LayerId::try_new(configured_binding.layer_id().to_owned()).map_err(
                    |error| {
                        EvaluateSignalsError::authoritative_input(format!(
                            "invalid layer id: {error}"
                        ))
                    },
                )?;
                if layer == target_layer {
                    rustdoc_contexts.insert(
                        layer.clone(),
                        AuthoritativeRustdocContext::new(
                            layer.clone(),
                            baseline.clone(),
                            snapshot_for_context_test(binding.layer_id(), &current),
                        ),
                    );
                    baseline_snapshots.insert(
                        layer,
                        BaselineSnapshot {
                            path: baseline_path.to_path_buf(),
                            hash: baseline_hash.clone(),
                        },
                    );
                    continue;
                }
                let configured_baseline_path = track_dir.join(configured_binding.baseline_file());
                let (configured_baseline_json, configured_baseline_hash) =
                    read_actual_baseline(&configured_baseline_path, trusted_items_root)?;
                let configured_baseline = BaselineRustdocCodec::from_json(
                    &configured_baseline_json,
                )
                .map_err(|error| {
                    EvaluateSignalsError::authoritative_input(format!(
                        "cannot decode baseline for layer '{layer}': {error}"
                    ))
                })?;
                rustdoc_contexts.insert(
                    layer.clone(),
                    AuthoritativeRustdocContext::new(
                        layer.clone(),
                        configured_baseline.clone(),
                        snapshot_for_context_test(layer.as_ref(), &configured_baseline),
                    ),
                );
                baseline_snapshots.insert(
                    layer,
                    BaselineSnapshot {
                        path: configured_baseline_path,
                        hash: configured_baseline_hash,
                    },
                );
            }
            let start_resolution =
                resolution_input_fingerprint(workspace_root, track_dir, trusted_items_root)?;
            let start_implementation = current_implementation_fingerprint(workspace_root)?;
            let loaded_catalogues =
                load_track_catalogues(workspace_root, track_dir, trusted_items_root)?;
            evaluate_and_write_with_contexts(
                catalogue_bytes,
                catalogue_path,
                track_dir,
                workspace_root,
                trusted_items_root,
                binding,
                &rustdoc_contexts,
                &baseline_snapshots,
                &loaded_catalogues,
                start_resolution,
                start_implementation,
                head_commit,
                baseline_path,
                baseline_hash,
                legacy_cache_key(
                    type_signals_codec::declaration_hash(catalogue_bytes),
                    read_head_commit(workspace_root)?,
                    type_signals_codec::baseline_hash(baseline_json.as_bytes()),
                ),
                EvaluationObservers::none(),
            )
        })
    }
}

#[derive(Clone, Copy)]
struct EvaluationObservers<'a> {
    resolution_paths: Option<ResolutionPathsObserver<'a>>,
    encoded_crate: Option<EncodedCrateObserver<'a>>,
    before_reuse: Option<ReuseObserver<'a>>,
}

impl EvaluationObservers<'_> {
    fn none() -> Self {
        Self { resolution_paths: None, encoded_crate: None, before_reuse: None }
    }
}

/// Error returned when a layer's type signals cannot be evaluated safely.
#[derive(Debug)]
pub enum EvaluateSignalsError {
    /// An authoritative input could not be acquired or verified.
    AuthoritativeInput(FreeText),
    /// Rustdoc extraction or signal evaluation could not complete.
    Evaluation(FreeText),
    /// A freshly evaluated cache document could not be persisted.
    CacheWrite(FreeText),
}

impl EvaluateSignalsError {
    fn authoritative_input(message: impl Into<String>) -> Self {
        Self::AuthoritativeInput(FreeText::new(message))
    }

    fn evaluation(message: impl Into<String>) -> Self {
        Self::Evaluation(FreeText::new(message))
    }

    fn cache_write(message: impl Into<String>) -> Self {
        Self::CacheWrite(FreeText::new(message))
    }
}

impl std::fmt::Display for EvaluateSignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthoritativeInput(message)
            | Self::Evaluation(message)
            | Self::CacheWrite(message) => formatter.write_str(message.as_str()),
        }
    }
}

/// Returns the active track's signal-artifact directory.
pub(crate) fn type_signals_track_dir(items_dir: &Path, track_id: &TrackId) -> PathBuf {
    items_dir.join(track_id.as_ref())
}

/// Rejects a symlinked trust anchor.
pub(crate) fn reject_symlinked_type_signals_anchor(path: &Path, label: &str) -> Result<(), String> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(path).map_err(|error| {
        format!("symlink guard: refusing to use {label} '{}': {error}", path.display())
    })
}

/// Evaluates and writes type signals for one TDDD-enabled layer.
///
/// # Errors
///
/// Returns an error when required files cannot be read or rustdoc cannot be
/// obtained safely.
pub fn execute_type_signals_for_layer(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
) -> Result<ExitCode, EvaluateSignalsError> {
    let context_cache = RustdocContextCache::default();
    execute_type_signals_for_layer_with_context_cache(
        items_dir,
        track_id,
        workspace_root,
        binding,
        features,
        &context_cache,
    )
}

/// Evaluates one layer while reusing the immutable multilayer rustdoc context
/// assembly held by the caller for the duration of a per-layer executor run.
pub(crate) fn execute_type_signals_for_layer_with_context_cache(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    context_cache: &RustdocContextCache,
) -> Result<ExitCode, EvaluateSignalsError> {
    let rustdoc = RustdocCrateAdapter::new(workspace_root.to_path_buf());
    execute_with_dependencies(
        items_dir,
        track_id,
        workspace_root,
        binding,
        features,
        &rustdoc,
        context_cache,
        EvaluationObservers::none(),
    )
}

#[cfg(feature = "test-helpers")]
#[allow(dead_code)]
pub(crate) fn execute_type_signals_for_layer_with_launch_observer(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    observer: &RustdocLaunchObserver,
) -> Result<ExitCode, EvaluateSignalsError> {
    let context_cache = RustdocContextCache::default();
    execute_type_signals_for_layer_with_launch_observer_and_context_cache(
        items_dir,
        track_id,
        workspace_root,
        binding,
        features,
        observer,
        &context_cache,
    )
}

#[cfg(feature = "test-helpers")]
pub(crate) fn execute_type_signals_for_layer_with_launch_observer_and_context_cache(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    observer: &RustdocLaunchObserver,
    context_cache: &RustdocContextCache,
) -> Result<ExitCode, EvaluateSignalsError> {
    let observe_resolution_paths =
        |paths: &std::collections::HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>| {
            observer.record_resolution_paths(paths);
        };
    let observe_encoded_crate = |encoded: &domain::tddd::ExtendedCrate| {
        observer.record_encoded_crate(encoded);
    };
    execute_with_dependencies(
        items_dir,
        track_id,
        workspace_root,
        binding,
        features,
        observer,
        context_cache,
        EvaluationObservers {
            resolution_paths: Some(&observe_resolution_paths),
            encoded_crate: Some(&observe_encoded_crate),
            before_reuse: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_with_dependencies(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    rustdoc: &impl RustdocProvider,
    context_cache: &RustdocContextCache,
    observers: EvaluationObservers<'_>,
) -> Result<ExitCode, EvaluateSignalsError> {
    reject_symlinked_type_signals_anchor(workspace_root, "workspace_root")
        .map_err(EvaluateSignalsError::authoritative_input)?;
    reject_symlinked_type_signals_anchor(items_dir, "items_dir")
        .map_err(EvaluateSignalsError::authoritative_input)?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot canonicalize items_dir '{}': {error}",
            items_dir.display()
        ))
    })?;
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot canonicalize workspace_root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "security: items_dir '{}' resolves outside workspace_root '{}'",
            items_dir.display(),
            workspace_root.display()
        )));
    }

    let track_dir = type_signals_track_dir(&canonical_items, track_id);
    reject_type_signals_path(&track_dir, &canonical_items, "track directory")?;
    let catalogue_path = track_dir.join(binding.catalogue_file());
    reject_type_signals_path(&catalogue_path, &canonical_items, "catalogue")?;
    let baseline_path = track_dir.join(binding.baseline_file());
    reject_type_signals_path(&baseline_path, &canonical_items, "baseline")?;
    let signal_path = track_dir.join(binding.signal_file());
    reject_type_signals_path(&signal_path, &canonical_items, "signal artifact")?;
    let catalogue_bytes = read_configured_catalogue(&catalogue_path, &canonical_items)?
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot read catalogue '{}': file not found",
                catalogue_path.display()
            ))
        })?;
    let target_crate = match binding.targets() {
        [target] => target.as_str(),
        _ => {
            return Err(EvaluateSignalsError::authoritative_input(
                "type-signal layers require exactly one rustdoc target".to_owned(),
            ));
        }
    };
    let declaration_hash = type_signals_codec::declaration_hash(&catalogue_bytes);
    let head_commit = read_head_commit(&canonical_workspace)?;
    let recorded_json = read_utf8_file_limited_under_root(
        &signal_path,
        &canonical_items,
        MAX_TYPE_SIGNALS_BYTES as u64,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read type-signals cache '{}': {error}",
            signal_path.display()
        ))
    })?;
    let cache_decision_start_implementation =
        current_implementation_fingerprint(&canonical_workspace)?;
    let loaded_catalogues =
        load_authoritative_inputs(&canonical_workspace, &track_dir, &canonical_items, rustdoc)?;
    validate_evaluator_binding(&loaded_catalogues.bindings, binding)?;
    let configured_layers = &loaded_catalogues.bindings;
    let cache_decision_start_resolution =
        loaded_catalogues.resolution_fingerprint.clone().ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(
                "authoritative resolution snapshot has no fingerprint".to_owned(),
            )
        })?;
    let target_layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
    })?;
    let target_catalogue = loaded_catalogues.catalogues.get(&target_layer).ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "target catalogue for layer '{target_layer}' is not present in the resolution snapshot"
        ))
    })?;
    if target_catalogue.declaration_hash() != &declaration_hash {
        return Err(EvaluateSignalsError::authoritative_input(
            "target catalogue changed while the authoritative resolution snapshot was assembled"
                .to_owned(),
        ));
    }
    let required_context_bindings =
        required_context_bindings(configured_layers, &target_layer, &loaded_catalogues)?;
    validate_context_export_count(&required_context_bindings)?;
    let baseline_hash = loaded_catalogues
        .baselines
        .get(&target_layer)
        .map(|baseline| {
            domain::tddd::type_signals_doc::BaselineHash::new(
                baseline.json_hash().as_digest().clone(),
            )
        })
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "target baseline for layer '{target_layer}' is not present in the resolution snapshot"
            ))
        })?;
    let feature_selections = load_layer_feature_selections(
        &track_dir,
        &canonical_workspace,
        configured_layers,
        &target_layer,
        features,
    )?;
    let target_features = feature_selections.get(&target_layer).ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "feature declaration has no selection for target layer '{target_layer}'"
        ))
    })?;
    let target_crate_name = CrateName::new(target_crate).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "invalid rustdoc target crate '{target_crate}': {error}"
        ))
    })?;
    let execution_identities = resolve_execution_identities(
        configured_layers,
        &target_layer,
        &loaded_catalogues.catalogues,
        &feature_selections,
        rustdoc,
    )?;
    let execution_identity = execution_identities.get(&target_layer).cloned().ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "rustdoc execution identity for target layer '{target_layer}' is unavailable"
        ))
    })?;
    let resolution_digest = cache_decision_start_resolution.as_digest().clone();
    let current_key = TypeSignalsCacheKey::new(
        declaration_hash.clone(),
        head_commit.clone(),
        baseline_hash.clone(),
        cache_decision_start_implementation.clone(),
        ResolutionFingerprint::new(resolution_digest),
        execution_identity,
    );
    let recorded = recorded_json.as_deref().and_then(|text| {
        type_signals_codec::decode_with_workspace_for_current(
            text,
            &canonical_workspace,
            current_key.rustdoc_execution_identity(),
        )
        .ok()
    });
    let worktree_clean = worktree_is_clean(&canonical_workspace)?;
    let reuse_decision =
        decide_reuse_for_recorded_document(recorded.as_ref(), &current_key, worktree_clean);
    match reuse_decision {
        domain::TypeSignalsReuseDecision::SkipEvaluation => {
            if let Some(observe) = observers.before_reuse {
                observe();
            }
            let end_implementation = current_implementation_fingerprint(&canonical_workspace)?;
            if end_implementation != cache_decision_start_implementation {
                return Err(EvaluateSignalsError::authoritative_input(
                    "workspace Rust implementation changed during type-signal evaluation",
                ));
            }
            let end_resolution =
                resolution_input_fingerprint(&canonical_workspace, &track_dir, &canonical_items)?;
            if end_resolution != cache_decision_start_resolution {
                return Err(EvaluateSignalsError::authoritative_input(
                    "architecture-rules, track catalogues, rustdoc baselines, or feature declarations changed during type-signal evaluation",
                ));
            }
            let end_head_commit = read_head_commit(&canonical_workspace)?;
            if end_head_commit != head_commit {
                return Err(EvaluateSignalsError::authoritative_input(
                    "HEAD changed during type-signal evaluation",
                ));
            }
            if !worktree_is_clean(&canonical_workspace)? {
                return Err(EvaluateSignalsError::authoritative_input(
                    "workspace worktree changed during type-signal evaluation",
                ));
            }
            return Ok(ExitCode::SUCCESS);
        }
        domain::TypeSignalsReuseDecision::ReextractAndEvaluate => {}
    }
    let context_cache_key = RustdocContextCacheKey {
        workspace_root: canonical_workspace.clone(),
        track_dir: track_dir.clone(),
        resolution_fingerprint: cache_decision_start_resolution.clone(),
        current_implementation_fingerprint: cache_decision_start_implementation.clone(),
        head_commit: head_commit.clone(),
        feature_selections: feature_selections.clone(),
        rustdoc_execution_identities: execution_identities.clone(),
    };
    let assembled_contexts = if let Some(cached) = context_cache.get(&context_cache_key)? {
        cached
    } else {
        let target_current = rustdoc
            .capture_current_with_implementation_fingerprint(
                &target_crate_name,
                target_features,
                &cache_decision_start_implementation,
            )
            .map_err(|error| {
                map_rustdoc_capture_error(
                    error,
                    format!("rustdoc export failed for '{target_crate}'"),
                )
            })?;
        if target_current.snapshot().execution_identity()
            != current_key.rustdoc_execution_identity()
        {
            return Err(EvaluateSignalsError::authoritative_input(
                "rustdoc execution identity changed between cache-key resolution and snapshot capture",
            ));
        }
        let assembled = assemble_rustdoc_contexts_from_snapshot(
            configured_layers,
            &target_layer,
            &loaded_catalogues,
            &target_current,
            &cache_decision_start_implementation,
            &feature_selections,
            rustdoc,
        )?;
        context_cache.insert_or_get(context_cache_key, assembled)?
    };
    for (layer, context) in &assembled_contexts.contexts {
        let expected = execution_identities.get(layer).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "rustdoc execution identity for configured layer '{layer}' is unavailable"
            ))
        })?;
        if context.current_snapshot().snapshot().execution_identity() != expected {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "rustdoc snapshot identity does not match configured layer '{layer}'"
            )));
        }
    }

    evaluate_and_write_with_contexts(
        &catalogue_bytes,
        &catalogue_path,
        &track_dir,
        &canonical_workspace,
        &canonical_items,
        binding,
        &assembled_contexts.contexts,
        &assembled_contexts.baseline_snapshots,
        &loaded_catalogues,
        cache_decision_start_resolution,
        cache_decision_start_implementation,
        head_commit,
        &baseline_path,
        baseline_hash,
        current_key,
        observers,
    )
}

fn evaluator_bindings_match(left: &TdddLayerBinding, right: &TdddLayerBinding) -> bool {
    left.layer_id() == right.layer_id()
        && left.catalogue_file() == right.catalogue_file()
        && left.targets() == right.targets()
}

fn validate_evaluator_binding(
    configured_bindings: &[TdddLayerBinding],
    binding: &TdddLayerBinding,
) -> Result<(), EvaluateSignalsError> {
    let configured_binding = configured_bindings
        .iter()
        .find(|candidate| candidate.layer_id() == binding.layer_id())
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "type-signal layer '{}' is not configured in architecture-rules.json",
                binding.layer_id()
            ))
        })?;
    if !evaluator_bindings_match(configured_binding, binding) {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "type-signal binding for layer '{}' does not match architecture-rules.json",
            binding.layer_id()
        )));
    }
    Ok(())
}

#[cfg(all(test, feature = "test-helpers"))]
#[path = "type_signals_evaluator_properties.rs"]
mod properties;

#[cfg(test)]
#[cfg(feature = "test-helpers")]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::evaluator_test_support::{evaluate_and_write, snapshot_for_context_test};
    use super::*;
    use crate::tddd::ThreeWaySignal;
    use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
    use crate::tddd::catalogue_to_extended_crate_codec::{
        CatalogueToExtendedCrateCodec, normalized_paths_for_doc,
    };
    #[cfg(feature = "test-helpers")]
    use crate::tddd::catalogue_to_extended_crate_codec::{
        encode_document, resolution_paths_for_document,
    };
    use crate::verify::tddd_layers::parse_tddd_layers;
    use domain::FreeText;
    use domain::Timestamp;
    use domain::tddd::CatalogueToExtendedCratePort;
    use domain::tddd::catalogue_v2::{CatalogueDocument, RustdocCratePort, RustdocCratePortError};
    use domain::tddd::type_signals_doc::TypeSignalsDocument;
    use domain::tddd::type_signals_doc::{ImplementationFingerprint, Sha256Digest};
    use rustdoc_contexts::verify_baseline_snapshots_unchanged;
    use usecase::merge_gate::{BlobFetchResult, TrackBlobReader};

    #[test]
    fn test_rustdoc_fingerprint_failure_between_snapshots_is_authoritative_input() {
        let error = map_rustdoc_capture_error(
            RustdocCratePortError::AuthoritativeInput {
                crate_name: CrateName::new("infrastructure").unwrap(),
                reason: FreeText::new("workspace input fingerprint changed during rustdoc capture"),
            },
            "rustdoc export failed for 'infrastructure'",
        );

        assert!(
            matches!(&error, EvaluateSignalsError::AuthoritativeInput(_)),
            "a fingerprint failure must remain an authoritative-input error: {error}"
        );
        assert!(
            !matches!(&error, EvaluateSignalsError::Evaluation(_)),
            "a fingerprint failure must not be reported as evaluation: {error}"
        );
        assert!(
            error.to_string().contains("fingerprint changed"),
            "the fingerprint failure reason must be preserved: {error}"
        );
    }

    fn rustdoc_json() -> String {
        format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        )
    }

    fn rustdoc_crate_with_paths(
        paths: HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
    ) -> rustdoc_types::Crate {
        rustdoc_types::Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::new(),
            paths,
            external_crates: HashMap::new(),
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
            format_version: rustdoc_types::FORMAT_VERSION,
        }
    }

    fn rustdoc_crate_with_root_and_paths(
        root_name: &str,
        paths: HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
    ) -> rustdoc_types::Crate {
        let mut crate_ = rustdoc_crate_with_paths(paths);
        crate_.index.insert(
            rustdoc_types::Id(0),
            rustdoc_types::Item {
                id: rustdoc_types::Id(0),
                crate_id: 0,
                name: Some(root_name.to_owned()),
                span: None,
                visibility: rustdoc_types::Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: rustdoc_types::ItemEnum::Module(rustdoc_types::Module {
                    is_crate: true,
                    items: vec![],
                    is_stripped: false,
                }),
            },
        );
        crate_
    }

    fn cross_layer_handler_catalogue(
        field_type: Option<&str>,
        trait_ref: Option<&str>,
    ) -> CatalogueDocument {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{
            CatalogueEntryKey, CrateName, FieldDecl, FieldName, ModulePath, TypeRef,
        };

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        let fields = field_type
            .map(|field_type| {
                vec![FieldDecl::new(
                    FieldName::new("reference").unwrap(),
                    TypeRef::new(field_type).unwrap(),
                )]
            })
            .unwrap_or_default();
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Handler".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields, has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                Some(ModulePath::root()),
                None,
                vec![],
                vec![],
            ),
        );
        if let Some(trait_ref) = trait_ref {
            catalogue.push_trait_impl(TraitImplDeclV2::new(
                TypeRef::new(trait_ref).unwrap(),
                TypeRef::new("Handler").unwrap(),
            ));
        }
        catalogue
    }

    fn cross_layer_declaring_catalogue(
        type_key: &str,
        type_module_path: Option<domain::tddd::catalogue_v2::ModulePath>,
        trait_key: Option<&str>,
    ) -> CatalogueDocument {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
        use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{CatalogueEntryKey, CrateName, ModulePath};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new(type_key.to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                type_module_path,
                None,
                vec![],
                vec![],
            ),
        );
        if let Some(trait_key) = trait_key {
            catalogue.insert_trait(
                CatalogueEntryKey::try_new(trait_key.to_owned()).unwrap(),
                TraitEntry::new(
                    ItemAction::Add,
                    ContractRole::SecondaryPort,
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    Some(ModulePath::from_segments(vec!["ports".to_owned()]).unwrap()),
                    None,
                    vec![],
                    vec![],
                ),
            );
        }
        catalogue
    }

    fn setup_cross_layer_execute_workspace(
        target: &CatalogueDocument,
        declaring: Option<&CatalogueDocument>,
        domain_current: &rustdoc_types::Crate,
        infrastructure_current: &rustdoc_types::Crate,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        TrackId,
        TdddLayerBinding,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        setup_cross_layer_execute_workspace_with_domain_target(
            target,
            declaring,
            domain_current,
            infrastructure_current,
            "domain",
        )
    }

    fn setup_cross_layer_execute_workspace_with_domain_target(
        target: &CatalogueDocument,
        declaring: Option<&CatalogueDocument>,
        domain_current: &rustdoc_types::Crate,
        infrastructure_current: &rustdoc_types::Crate,
        domain_target: &str,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        TrackId,
        TdddLayerBinding,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let track_id = TrackId::try_new("cross-layer-execute-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub struct Fixture;\n").unwrap();
        std::fs::write(
            root.join(".gitignore"),
            "track/items/cross-layer-execute-track/infrastructure-type-signals.json\n",
        )
        .unwrap();
        let rules = serde_json::json!({
            "version": 2,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "method": "rustdoc", "targets": [domain_target] }
                    }
                },
                {
                    "crate": "infrastructure",
                    "path": "libs/infrastructure",
                    "may_depend_on": ["domain"],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "infrastructure-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                    }
                }
            ]
        })
        .to_string();
        std::fs::write(root.join("architecture-rules.json"), &rules).unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            CatalogueDocumentCodec::encode(target).unwrap(),
        )
        .unwrap();
        if let Some(declaring) = declaring {
            std::fs::write(
                track_dir.join("domain-types.json"),
                CatalogueDocumentCodec::encode(declaring).unwrap(),
            )
            .unwrap();
        }
        let baseline = rustdoc_json();
        if declaring.is_some() {
            std::fs::write(track_dir.join("domain-types-baseline.json"), &baseline).unwrap();
        }
        std::fs::write(track_dir.join("infrastructure-types-baseline.json"), &baseline).unwrap();
        let domain_current_path = exclusive_observer_json_path(root, "domain-current.json");
        let infrastructure_current_path =
            exclusive_observer_json_path(root, "infrastructure-current.json");
        std::fs::write(&domain_current_path, serde_json::to_string(domain_current).unwrap())
            .unwrap();
        std::fs::write(
            &infrastructure_current_path,
            serde_json::to_string(infrastructure_current).unwrap(),
        )
        .unwrap();
        crate::verify::test_support::git_init(root);
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(&rules)
            .unwrap()
            .into_iter()
            .find(|binding| binding.layer_id() == "infrastructure")
            .unwrap();

        (workspace, items_dir, track_id, binding, domain_current_path, infrastructure_current_path)
    }

    struct MissingCatalogueIdentityRustdoc {
        observer: RustdocLaunchObserver,
    }

    impl RustdocCratePort for MissingCatalogueIdentityRustdoc {
        fn load_from_path(
            &self,
            path: &Path,
        ) -> Result<domain::tddd::type_signals_doc::CapturedRustdocJson, RustdocCratePortError>
        {
            RustdocCratePort::load_from_path(&self.observer, path)
        }

        fn capture_current(
            &self,
            crate_name: &CrateName,
            features: &[CargoFeatureName],
            evaluation_start: &ImplementationFingerprint,
        ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
            RustdocCratePort::capture_current(
                &self.observer,
                crate_name,
                features,
                evaluation_start,
            )
        }
    }

    impl RustdocProvider for MissingCatalogueIdentityRustdoc {
        fn capture_current_with_implementation_fingerprint(
            &self,
            crate_name: &CrateName,
            features: &[CargoFeatureName],
            evaluation_start: &ImplementationFingerprint,
        ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
            RustdocProvider::capture_current_with_implementation_fingerprint(
                &self.observer,
                crate_name,
                features,
                evaluation_start,
            )
        }

        fn execution_identity(
            &self,
            crate_name: &CrateName,
            features: &[CargoFeatureName],
        ) -> Result<domain::tddd::type_signals_doc::RustdocExecutionIdentity, RustdocCratePortError>
        {
            if crate_name.as_str() == "domain" {
                return Err(RustdocCratePortError::AuthoritativeInput {
                    crate_name: crate_name.clone(),
                    reason: FreeText::new("package `domain` was not found in cargo metadata"),
                });
            }
            RustdocProvider::execution_identity(&self.observer, crate_name, features)
        }
    }

    #[test]
    fn test_type_signal_identity_index_uses_catalogue_adds_from_shared_resolution_set() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("AddedOnlyInCatalogue".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            ),
        );

        let baseline = rustdoc_crate_with_paths(HashMap::new());
        let current = baseline.clone();
        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &current)
            .expect("the shared resolution set must include catalogue add declarations");
        let index = build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("type-signal identity indexing must consume catalogue additions");

        let kinds = BTreeMap::from([("AddedOnlyInCatalogue".to_owned(), vec!["struct"])]);
        let signals = [ThreeWaySignal::catalogue_item(
            FreeText::new("domain::AddedOnlyInCatalogue"),
            CatalogueItemNamespace::Type,
            domain::tddd::signal_evaluator::region::SignalRegion::SIntersectC_Match_Add,
        )];
        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "AddedOnlyInCatalogue");
    }

    #[test]
    fn test_type_signal_identity_index_resolves_add_impl_owner_from_shared_resolution_set() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::CatalogueEntryKey;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
        use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{CrateName, TypeRef};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("NewTrait".to_owned()).unwrap(),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("NewTrait".to_owned()).unwrap(),
            TypeRef::new("Owner".to_owned()).unwrap(),
            vec![],
            vec![],
        ));

        let baseline = rustdoc_crate_with_paths(HashMap::new());
        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &baseline)
            .expect("the shared resolution set must include both add declarations");
        let index = build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("an add impl owner must resolve through the shared set");
        let kinds = BTreeMap::from([("Owner".to_owned(), vec!["struct"])]);
        let signals = [ThreeWaySignal::label(
            FreeText::new("Owner: NewTrait"),
            domain::tddd::signal_evaluator::region::SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "Owner");
        assert_eq!(built[0].found_items(), &["NewTrait"]);
    }

    #[test]
    fn test_type_signal_identity_index_rejects_impl_owner_absent_from_shared_resolution_set() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{CrateName, TypeRef};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            domain::tddd::catalogue_v2::ItemAction::Add,
            TypeRef::new("MissingTrait").unwrap(),
            TypeRef::new("MissingOwner").unwrap(),
            vec![],
            vec![],
        ));

        let empty = rustdoc_crate_with_paths(HashMap::new());
        let error = build_type_signal_identity_index(&catalogue, &empty.paths)
            .expect_err("an impl owner absent from both authorities must fail closed");
        assert!(error.contains("MissingOwner"));
    }

    #[test]
    fn test_execute_type_signals_evaluates_shared_catalogue_identity_and_function_path() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CatalogueEntryKey, CrateName, FunctionName, FunctionPath, ModulePath,
        };

        let make_rustdoc = |root_name: &str, include_items: bool, include_removed: bool| {
            let root_id = rustdoc_types::Id(0);
            let type_id = rustdoc_types::Id(1);
            let function_id = rustdoc_types::Id(2);
            let removed_id = rustdoc_types::Id(3);
            let mut index = HashMap::new();
            let mut paths = HashMap::new();
            let mut root_items = Vec::new();

            if include_items {
                index.insert(
                    type_id,
                    rustdoc_types::Item {
                        id: type_id,
                        crate_id: 0,
                        name: Some("Added".to_owned()),
                        span: None,
                        visibility: rustdoc_types::Visibility::Public,
                        docs: None,
                        links: HashMap::new(),
                        attrs: vec![],
                        deprecation: None,
                        inner: rustdoc_types::ItemEnum::Struct(rustdoc_types::Struct {
                            kind: rustdoc_types::StructKind::Unit,
                            generics: rustdoc_types::Generics {
                                params: vec![],
                                where_predicates: vec![],
                            },
                            impls: vec![],
                        }),
                    },
                );
                paths.insert(
                    type_id,
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec![
                            root_name.to_owned(),
                            "generated".to_owned(),
                            "Added".to_owned(),
                        ],
                        kind: rustdoc_types::ItemKind::Struct,
                    },
                );
                index.insert(
                    function_id,
                    rustdoc_types::Item {
                        id: function_id,
                        crate_id: 0,
                        name: Some("run".to_owned()),
                        span: None,
                        visibility: rustdoc_types::Visibility::Public,
                        docs: None,
                        links: HashMap::new(),
                        attrs: vec![],
                        deprecation: None,
                        inner: rustdoc_types::ItemEnum::Function(rustdoc_types::Function {
                            sig: rustdoc_types::FunctionSignature {
                                inputs: vec![],
                                output: None,
                                is_c_variadic: false,
                            },
                            generics: rustdoc_types::Generics {
                                params: vec![],
                                where_predicates: vec![],
                            },
                            has_body: true,
                            header: rustdoc_types::FunctionHeader {
                                is_async: false,
                                is_const: false,
                                is_unsafe: false,
                                abi: rustdoc_types::Abi::Rust,
                            },
                        }),
                    },
                );
                paths.insert(
                    function_id,
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec![root_name.to_owned(), "commands".to_owned(), "run".to_owned()],
                        kind: rustdoc_types::ItemKind::Function,
                    },
                );
                root_items.extend([type_id, function_id]);
            }

            if include_removed {
                index.insert(
                    removed_id,
                    rustdoc_types::Item {
                        id: removed_id,
                        crate_id: 0,
                        name: Some("Removed".to_owned()),
                        span: None,
                        visibility: rustdoc_types::Visibility::Public,
                        docs: None,
                        links: HashMap::new(),
                        attrs: vec![],
                        deprecation: None,
                        inner: rustdoc_types::ItemEnum::Struct(rustdoc_types::Struct {
                            kind: rustdoc_types::StructKind::Unit,
                            generics: rustdoc_types::Generics {
                                params: vec![],
                                where_predicates: vec![],
                            },
                            impls: vec![],
                        }),
                    },
                );
                paths.insert(
                    removed_id,
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec![root_name.to_owned(), "Removed".to_owned()],
                        kind: rustdoc_types::ItemKind::Struct,
                    },
                );
                root_items.push(removed_id);
            }

            index.insert(
                root_id,
                rustdoc_types::Item {
                    id: root_id,
                    crate_id: 0,
                    name: Some(root_name.to_owned()),
                    span: None,
                    visibility: rustdoc_types::Visibility::Public,
                    docs: None,
                    links: HashMap::new(),
                    attrs: vec![],
                    deprecation: None,
                    inner: rustdoc_types::ItemEnum::Module(rustdoc_types::Module {
                        is_crate: true,
                        items: root_items,
                        is_stripped: false,
                    }),
                },
            );

            rustdoc_types::Crate {
                root: root_id,
                crate_version: None,
                includes_private: false,
                index,
                paths,
                external_crates: HashMap::new(),
                target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
                format_version: rustdoc_types::FORMAT_VERSION,
            }
        };

        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        crate::verify::test_support::git_init(root);
        let track_id = TrackId::try_new("shared-identity-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/infrastructure\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(root.join("libs/infrastructure/src")).unwrap();
        std::fs::write(
            root.join("libs/infrastructure/Cargo.toml"),
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("libs/infrastructure/src/lib.rs"), "pub struct Fixture;\n")
            .unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(rules).unwrap().pop().unwrap();

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Added".to_owned()).unwrap(),
            domain::tddd::catalogue_v2::entries::TypeEntry::new(
                domain::tddd::catalogue_v2::ItemAction::Add,
                domain::tddd::catalogue_v2::roles::DataRole::value_object(),
                domain::tddd::catalogue_v2::composite::TypeKindV2::Struct(
                    domain::tddd::catalogue_v2::composite::StructKind::new(
                        domain::tddd::catalogue_v2::composite::StructShape::Unit,
                        None,
                    ),
                ),
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_deletion(domain::tddd::catalogue_v2::DeletionRecord::Type {
            name: CatalogueEntryKey::try_new("Removed".to_owned()).unwrap(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        catalogue.insert_function(
            FunctionPath::new(
                CrateName::new("infrastructure").unwrap(),
                ModulePath::from_segments(vec!["commands".to_owned()]).unwrap(),
                FunctionName::new("run").unwrap(),
            ),
            FunctionEntry::new(
                domain::tddd::catalogue_v2::ItemAction::Add,
                FunctionRole::FreeFunction,
                vec![],
                domain::tddd::catalogue_v2::TypeRef::new("()").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            ),
        );

        let catalogue_json = CatalogueDocumentCodec::encode(&catalogue).unwrap();
        let baseline = make_rustdoc("sotp", false, true);
        let current = make_rustdoc("sotp", true, false);
        let baseline_json = serde_json::to_string(&baseline).unwrap();
        let current_json = serde_json::to_string(&current).unwrap();
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let baseline_path = track_dir.join(binding.baseline_file());
        std::fs::write(&catalogue_path, &catalogue_json).unwrap();
        std::fs::write(&baseline_path, &baseline_json).unwrap();

        let result = evaluate_and_write(
            catalogue_json.as_bytes(),
            &catalogue_path,
            &track_dir,
            root,
            &items_dir.canonicalize().unwrap(),
            &binding,
            current_json,
            read_head_commit(root).unwrap(),
            &baseline_path,
            &baseline_json,
            type_signals_codec::baseline_hash(baseline_json.as_bytes()),
        )
        .unwrap();
        assert_eq!(result, ExitCode::SUCCESS);

        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(track_dir.join(binding.signal_file())).unwrap(),
        )
        .unwrap();
        assert!(
            persisted.signals().iter().any(|signal| signal.type_name() == "Added"),
            "catalogue additions must reach the production type-signal pipeline"
        );
        assert!(
            persisted
                .signals()
                .iter()
                .any(|signal| signal.type_name() == "infrastructure::commands::run"),
            "function identity must retain the canonical package path"
        );
        assert!(
            persisted.signals().iter().any(|signal| signal.type_name() == "Removed"),
            "deletion handling must retain the baseline identity through Phase 1"
        );
    }

    #[test]
    fn test_execute_type_signals_rejects_mismatched_cross_layer_candidate_fail_closed() {
        with_process_environment_lock(|| {
            let target = cross_layer_handler_catalogue(Some("domain::model::Name"), None);
            let declaring = cross_layer_declaring_catalogue("Name", None, None);
            let domain_current = rustdoc_crate_with_paths(HashMap::from([(
                rustdoc_types::Id(1),
                rustdoc_types::ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "wrong".to_owned(), "Name".to_owned()],
                    kind: rustdoc_types::ItemKind::Struct,
                },
            )]));
            let infrastructure_current = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(
                    &target,
                    Some(&declaring),
                    &domain_current,
                    &infrastructure_current,
                );
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));

            let error = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect_err("a mismatched external candidate must fail closed");
            assert!(
                error.to_string().contains("domain::model::Name"),
                "the diagnostic must retain the fully-qualified unresolved identity: {error}"
            );
            assert_eq!(observer.launches_for("domain"), 1);
            assert_eq!(observer.launches_for("infrastructure"), 1);
        });
    }

    #[test]
    fn test_execute_type_signals_resolves_cross_layer_type_and_trait_adds() {
        with_process_environment_lock(|| {
            let target = cross_layer_handler_catalogue(
                Some("domain::model::Name"),
                Some("domain::ports::Repository"),
            );
            let declaring = cross_layer_declaring_catalogue(
                "domain::model::Name",
                Some(
                    domain::tddd::catalogue_v2::ModulePath::from_segments(vec!["model".to_owned()])
                        .unwrap(),
                ),
                Some("domain::ports::Repository"),
            );
            let domain_current = rustdoc_crate_with_paths(HashMap::new());
            let infrastructure_current = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(
                    &target,
                    Some(&declaring),
                    &domain_current,
                    &infrastructure_current,
                );
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect("type and trait declarations from the other layer must resolve");

            let signals = type_signals_codec::decode(
                &std::fs::read_to_string(
                    items_dir.join(track_id.as_ref()).join(binding.signal_file()),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(
                signals.signals().iter().any(|signal| signal.type_name() == "Handler"),
                "the target type must survive evaluation after both external declarations resolve"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_mixed_layer_snapshot_after_source_aba_restore() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        with_process_environment_lock(|| {
            let target = cross_layer_handler_catalogue(None, None);
            let declaring = cross_layer_declaring_catalogue("Name", None, None);
            let empty = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(&target, Some(&declaring), &empty, &empty);
            let signal_path = signal_path(&items_dir, &track_id);
            let prior = b"prior-cache-generation";
            std::fs::write(&signal_path, prior).unwrap();

            let source_path = workspace.path().join("src/lib.rs");
            let original_source = std::fs::read(&source_path).unwrap();
            let before_count = std::sync::Arc::new(AtomicUsize::new(0));
            let before_count_for_hook = std::sync::Arc::clone(&before_count);
            let source_for_before = source_path.clone();
            let before_export = std::sync::Arc::new(move || {
                if before_count_for_hook.fetch_add(1, Ordering::SeqCst) == 1 {
                    std::fs::write(&source_for_before, b"pub struct FixtureB;\n").unwrap();
                }
            });
            let after_count = std::sync::Arc::new(AtomicUsize::new(0));
            let after_count_for_hook = std::sync::Arc::clone(&after_count);
            let source_for_after = source_path.clone();
            let original_for_after = original_source.clone();
            let after_export = std::sync::Arc::new(move || {
                if after_count_for_hook.fetch_add(1, Ordering::SeqCst) == 1 {
                    std::fs::write(&source_for_after, &original_for_after).unwrap();
                }
            });
            let observer = RustdocLaunchObserver::using_json_paths_with_before_and_after_export(
                BTreeMap::from([
                    ("domain".to_owned(), domain_path),
                    ("infrastructure".to_owned(), infrastructure_path),
                ]),
                before_export,
                after_export,
            );

            let error = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect_err("a mixed A/B layer snapshot must not be persisted");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("evaluation-start snapshot"),
                "the export must report the evaluation-start mismatch: {error}"
            );
            assert_eq!(std::fs::read(&source_path).unwrap(), original_source);
            assert_eq!(std::fs::read(&signal_path).unwrap(), prior);
            assert_eq!(observer.launches_for("infrastructure"), 1);
            assert_eq!(observer.launches_for("domain"), 1);
        });
    }

    #[test]
    fn test_execute_type_signals_canonicalizes_bin_target_for_synthesized_external_add() {
        use rustdoc_types::{ItemEnum, Type};

        with_process_environment_lock(|| {
            let target = cross_layer_handler_catalogue(Some("domain::generated::Name"), None);
            let declaring = cross_layer_declaring_catalogue("Name", None, None);
            let domain_current = rustdoc_crate_with_root_and_paths(
                "sotp",
                HashMap::from([(
                    rustdoc_types::Id(1),
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec!["sotp".to_owned(), "generated".to_owned(), "Name".to_owned()],
                        kind: rustdoc_types::ItemKind::Struct,
                    },
                )]),
            );
            let empty = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace_with_domain_target(
                    &target,
                    Some(&declaring),
                    &domain_current,
                    &empty,
                    "domain_bin",
                );
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain_bin".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect("the evaluator must place the external add through the bin-target alias");

            let resolution_snapshots = observer.resolution_path_snapshots();
            assert_eq!(resolution_snapshots.len(), 1);
            let resolved_paths = &resolution_snapshots[0];
            let resolved_name = resolved_paths
                .iter()
                .find(|(_, summary)| summary.path == ["domain", "generated", "Name"])
                .expect("the synthesized add must use the declaring crate root");
            assert_eq!(resolved_name.1.crate_id, u32::MAX - 1);
            assert!(
                !resolved_paths
                    .values()
                    .any(|summary| summary.path == ["sotp", "generated", "Name"]),
                "the bin target root must be canonicalized before the add is synthesized"
            );

            let encoded_crates = observer.encoded_crates();
            assert_eq!(encoded_crates.len(), 1);
            let encoded = &encoded_crates[0];
            let name_id = encoded
                .krate()
                .paths
                .iter()
                .find(|(_, summary)| summary.path == ["domain", "generated", "Name"])
                .map(|(id, _)| *id)
                .expect("the encoded synthesized item must retain its resolved placement");
            let name_summary = &encoded.krate().paths[&name_id];
            assert_ne!(name_summary.crate_id, 0);
            assert_eq!(encoded.krate().external_crates[&name_summary.crate_id].name, "domain");

            let handler_id = encoded
                .krate()
                .paths
                .iter()
                .find(|(_, summary)| summary.path == ["infrastructure", "Handler"])
                .map(|(id, _)| *id)
                .expect("the target Handler must be encoded");
            let ItemEnum::Struct(handler) = &encoded.krate().index[&handler_id].inner else {
                panic!("expected Handler struct");
            };
            let rustdoc_types::StructKind::Plain { fields, .. } = &handler.kind else {
                panic!("expected Handler named fields");
            };
            assert!(matches!(
                &encoded.krate().index[&fields[0]].inner,
                ItemEnum::StructField(Type::ResolvedPath(path)) if path.id == name_id
            ));
        });
    }

    #[test]
    fn test_execute_type_signals_accepts_more_than_64_configured_layers_when_only_target_is_exported()
     {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let layers = std::iter::once(serde_json::json!({
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": {
                        "method": "rustdoc",
                        "targets": ["infrastructure"]
                    }
                }
            }))
            .chain((0..64).map(|index| {
                serde_json::json!({
                    "crate": format!("layer_{index}"),
                    "path": format!("libs/layer_{index}"),
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": format!("layer_{index}-types.json"),
                        "schema_export": {
                            "method": "rustdoc",
                            "targets": [format!("layer_{index}")]
                        }
                    }
                })
            }))
            .collect::<Vec<_>>();
            std::fs::write(
                workspace.path().join("architecture-rules.json"),
                serde_json::json!({ "version": 2, "layers": layers }).to_string(),
            )
            .unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect("unexported configured layers must not consume the context budget");

            assert_eq!(observer.launches(), 1, "only the selected target should be exported");
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_65th_actual_export_before_export() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let recorded =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let layers = std::iter::once(serde_json::json!({
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": {
                        "method": "rustdoc",
                        "targets": ["infrastructure"]
                    }
                }
            }))
                .chain((0..64).map(|index| {
                    let layer = format!("layer_{index}");
                    let track_dir = items_dir.join(track_id.as_ref());
                    let catalogue = format!(
                        "{{\n  \"schema_version\": 5,\n  \"crate_name\": \"{layer}\",\n  \"layer\": \"{layer}\",\n  \"types\": {{}},\n  \"traits\": {{}},\n  \"functions\": {{}}\n}}\n"
                    );
                    std::fs::write(track_dir.join(format!("{layer}-types.json")), catalogue)
                        .unwrap();
                    std::fs::write(
                        track_dir.join(format!("{layer}-types-baseline.json")),
                        rustdoc_json(),
                    )
                    .unwrap();
                    serde_json::json!({
                        "crate": layer,
                        "path": format!("libs/layer_{index}"),
                        "may_depend_on": [],
                        "tddd": {
                            "enabled": true,
                            "catalogue_file": format!("layer_{index}-types.json"),
                            "schema_export": {
                                "method": "rustdoc",
                                "targets": [format!("layer_{index}")]
                            }
                        }
                    })
                }))
                .collect::<Vec<_>>();
            std::fs::write(
                workspace.path().join("architecture-rules.json"),
                serde_json::json!({ "version": 2, "layers": layers }).to_string(),
            )
            .unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            let error = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect_err("the 65th actual export must fail before rustdoc export");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("64"),
                "the error must identify the layer budget: {error}"
            );
            assert_eq!(observer.launches(), 0, "the layer-budget failure must precede export");
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                recorded,
                "a 65th required layer must not reuse or rewrite a recorded type-signals result"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_non_exclusive_target_directory() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, exclusive_path) = setup_workspace();
            let shared_path = workspace.path().join("infrastructure-rustdoc.json");
            std::fs::copy(&exclusive_path, &shared_path).unwrap();
            let observer = RustdocLaunchObserver::using_json_path(shared_path);

            let error = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect_err("a shared Cargo target must not be treated as authoritative");
            assert!(
                error.to_string().contains(".sotp-rustdoc"),
                "non-exclusive target ownership must fail closed: {error}"
            );
            assert_eq!(observer.launches(), 0, "export must not run without exclusive ownership");
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_lock_failure_does_not_reuse_or_retry() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, _observer_path) = setup_workspace();
            let commands = tempfile::tempdir().unwrap();
            let target_directory = workspace.path().join("cargo-target");
            let invocations = workspace.path().join("rustdoc-invocations");
            let metadata = serde_json::json!({
                "packages": [{
                    "name": "infrastructure",
                    "targets": [{"kind": ["lib"], "name": "infrastructure"}]
                }],
                "target_directory": target_directory,
            })
            .to_string();
            write_evaluator_test_toolchain(
                commands.path(),
                &metadata,
                "printf '%s\\n' invoked >> \"$SOTOHE_TEST_RUSTDOC_INVOCATIONS\"\nexit 1",
            );
            let path = minimal_test_command_path(commands.path());

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_RUSTDOC_INVOCATIONS", Some(invocations.as_os_str())),
                ],
                || {
                    let original = write_cache(
                        &items_dir,
                        &track_id,
                        read_head_commit(workspace.path()).unwrap(),
                    );
                    let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
                    let crate_name = CrateName::new("infrastructure").unwrap();
                    let identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let target = identity.target_directory().as_path();
                    std::fs::create_dir_all(target).unwrap();
                    std::fs::create_dir(target.join(".sotp-rustdoc-json.lock")).unwrap();

                    let error = execute_with_dependencies(
                        &items_dir,
                        &track_id,
                        workspace.path(),
                        &binding,
                        &[],
                        &adapter,
                        &RustdocContextCache::default(),
                        EvaluationObservers::none(),
                    )
                    .expect_err("a lock operation failure must stop evaluation");

                    assert!(
                        error.to_string().contains("lock"),
                        "the evaluator must preserve the lock failure: {error}"
                    );
                    assert!(
                        !invocations.exists(),
                        "a failed lock operation must not launch a lockless rustdoc export"
                    );
                    assert_eq!(
                        std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                        original,
                        "a lock failure must not reuse or replace the existing cache"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_waits_on_held_exclusive_lock() {
        use std::time::Duration;

        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, _observer_path) = setup_workspace();
            let commands = tempfile::tempdir().unwrap();
            let cargo_target = tempfile::tempdir().unwrap();
            let target_directory = cargo_target.path().to_path_buf();
            let invocations = target_directory.join("rustdoc-invocations");
            let metadata = serde_json::json!({
                "packages": [{
                    "name": "infrastructure",
                    "targets": [{"kind": ["lib"], "name": "infrastructure"}]
                }],
                "target_directory": target_directory,
            })
            .to_string();
            let rustdoc_json = rustdoc_json();
            write_evaluator_test_toolchain(
                commands.path(),
                &metadata,
                &format!(
                    "printf '%s\\n' invoked >> \"$SOTOHE_TEST_RUSTDOC_INVOCATIONS\"\nmkdir -p \"$CARGO_TARGET_DIR/doc\"\nprintf '%s\\n' '{rustdoc_json}' > \"$CARGO_TARGET_DIR/doc/infrastructure.json\"\nexit 0"
                ),
            );
            let path = minimal_test_command_path(commands.path());

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_RUSTDOC_INVOCATIONS", Some(invocations.as_os_str())),
                ],
                || {
                    assert_eq!(
                        crate::tddd::rustdoc_output_lock::RUSTDOC_OUTPUT_LOCK_TIMEOUT,
                        Duration::from_secs(120)
                    );
                    let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
                    let crate_name = CrateName::new("infrastructure").unwrap();
                    let identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let exclusive = identity.target_directory().as_path().to_path_buf();
                    let held =
                        crate::tddd::rustdoc_output_lock::RustdocOutputLock::acquire(&exclusive)
                            .unwrap();
                    let thread_items_dir = items_dir.clone();
                    let thread_track_id = track_id.clone();
                    let thread_workspace_root = workspace.path().to_path_buf();
                    let thread_binding = binding.clone();
                    let contender = std::thread::spawn(move || {
                        let adapter = RustdocCrateAdapter::new(thread_workspace_root.clone());
                        let context_cache = RustdocContextCache::default();
                        execute_with_dependencies(
                            &thread_items_dir,
                            &thread_track_id,
                            &thread_workspace_root,
                            &thread_binding,
                            &[],
                            &adapter,
                            &context_cache,
                            EvaluationObservers::none(),
                        )
                    });

                    std::thread::sleep(Duration::from_millis(80));
                    assert!(
                        !contender.is_finished(),
                        "type-signal evaluation must wait on the exclusive rustdoc lock"
                    );
                    assert!(
                        !invocations.exists(),
                        "evaluation must not launch a lockless export while the lock is held"
                    );
                    drop(held);
                    let _result = contender.join().unwrap();
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_rejects_symlinked_output_on_no_follow_read() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, _observer_path) = setup_workspace();
            let commands = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let outside_json = outside.path().join("infrastructure.json");
            let target_directory = workspace.path().join("cargo-target");
            std::fs::write(&outside_json, rustdoc_json()).unwrap();
            let metadata = serde_json::json!({
                "packages": [{
                    "name": "infrastructure",
                    "targets": [{"kind": ["lib"], "name": "infrastructure"}]
                }],
                "target_directory": target_directory,
            })
            .to_string();
            write_evaluator_test_toolchain(
                commands.path(),
                &metadata,
                "mkdir -p \"$CARGO_TARGET_DIR/doc\"\nln -s \"$SOTOHE_TEST_SYMLINK_TARGET\" \"$CARGO_TARGET_DIR/doc/infrastructure.json\"\nexit 0",
            );
            let path = minimal_test_command_path(commands.path());

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_SYMLINK_TARGET", Some(outside_json.as_os_str())),
                ],
                || {
                    let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
                    let error = execute_with_dependencies(
                        &items_dir,
                        &track_id,
                        workspace.path(),
                        &binding,
                        &[],
                        &adapter,
                        &RustdocContextCache::default(),
                        EvaluationObservers::none(),
                    )
                    .expect_err("a symlinked rustdoc output must fail closed");

                    assert!(
                        error.to_string().contains("symlink"),
                        "the evaluator must preserve the no-follow symlink rejection: {error}"
                    );
                    assert!(
                        !signal_path(&items_dir, &track_id).exists(),
                        "a symlinked output must not publish a signal artifact"
                    );
                },
            );
        });
    }

    #[cfg(not(unix))]
    #[test]
    fn test_execute_type_signals_rejects_non_unix_rustdoc_target_before_export() {
        let (workspace, items_dir, track_id, binding, _observer_path) = setup_workspace();
        let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());

        let error = execute_with_dependencies(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &adapter,
            &RustdocContextCache::default(),
            EvaluationObservers::none(),
        )
        .expect_err("unsupported platforms must fail closed before rustdoc export");

        assert!(
            error.to_string().contains("supported only on Unix"),
            "the evaluator must expose the unsupported-platform guard: {error}"
        );
    }

    #[test]
    fn test_execute_type_signals_treats_missing_enabled_catalogue_as_no_declarations() {
        with_process_environment_lock(|| {
            let target = CatalogueDocument::new(
                5,
                CrateName::new("infrastructure").unwrap(),
                LayerId::try_new("infrastructure").unwrap(),
            );
            let empty = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(&target, None, &empty, &empty);
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect("an enabled layer without a catalogue must contribute no declarations");

            let signals = type_signals_codec::decode(
                &std::fs::read_to_string(
                    items_dir.join(track_id.as_ref()).join(binding.signal_file()),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(signals.signals().is_empty());
            assert_eq!(observer.launches_for("domain"), 0);
            assert_eq!(observer.launches_for("infrastructure"), 1);
        });
    }

    #[test]
    fn test_execute_type_signals_skips_missing_catalogue_before_resolving_package_identity() {
        with_process_environment_lock(|| {
            let target = CatalogueDocument::new(
                5,
                CrateName::new("infrastructure").unwrap(),
                LayerId::try_new("infrastructure").unwrap(),
            );
            let empty = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(&target, None, &empty, &empty);
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));
            let rustdoc = MissingCatalogueIdentityRustdoc { observer: observer.clone() };

            execute_with_dependencies(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &rustdoc,
                &RustdocContextCache::default(),
                EvaluationObservers::none(),
            )
            .expect(
                "a missing enabled catalogue must be skipped before its unavailable package identity is resolved",
            );

            assert_eq!(observer.launches_for("domain"), 0);
            assert_eq!(observer.launches_for("infrastructure"), 1);
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_ungrounded_cross_layer_reference() {
        with_process_environment_lock(|| {
            let target = cross_layer_handler_catalogue(Some("domain::model::Unknown"), None);
            let declaring = cross_layer_declaring_catalogue(
                "domain::model::Name",
                Some(
                    domain::tddd::catalogue_v2::ModulePath::from_segments(vec!["model".to_owned()])
                        .unwrap(),
                ),
                None,
            );
            let empty = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(&target, Some(&declaring), &empty, &empty);
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));

            let error = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect_err("an ungrounded cross-crate reference must not use short-name fallback");
            assert!(
                error.to_string().contains("domain::model::Unknown"),
                "the diagnostic must identify the ungrounded reference: {error}"
            );
        });
    }

    #[test]
    fn test_adapter_evaluate_layer_rejects_ungrounded_cross_layer_reference() {
        use crate::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
        use domain::tddd::catalogue_v2::TdddLayerBinding as DomainTdddLayerBinding;
        use usecase::type_signals::TypeSignalsExecutorPort;

        with_process_environment_lock(|| {
            let target = cross_layer_handler_catalogue(Some("domain::model::Unknown"), None);
            let declaring = cross_layer_declaring_catalogue(
                "domain::model::Name",
                Some(
                    domain::tddd::catalogue_v2::ModulePath::from_segments(vec!["model".to_owned()])
                        .unwrap(),
                ),
                None,
            );
            let empty = rustdoc_crate_with_paths(HashMap::new());
            let (workspace, items_dir, track_id, _infra_binding, domain_path, infrastructure_path) =
                setup_cross_layer_execute_workspace(&target, Some(&declaring), &empty, &empty);
            let observer = RustdocLaunchObserver::using_json_paths(BTreeMap::from([
                ("domain".to_owned(), domain_path),
                ("infrastructure".to_owned(), infrastructure_path),
            ]));
            let binding = DomainTdddLayerBinding {
                layer_id: "infrastructure".to_owned(),
                catalogue_file: "infrastructure-types.json".to_owned(),
                baseline_file: "infrastructure-types-baseline.json".to_owned(),
                targets: vec!["infrastructure".to_owned()],
            };

            let error = TypeSignalsExecutorAdapter::with_rustdoc_launch_observer(observer)
                .evaluate_layer(&items_dir, &track_id, workspace.path(), &binding, &[])
                .expect_err("evaluate_layer must reject an ungrounded cross-crate reference");
            assert!(
                error.to_string().contains("domain::model::Unknown"),
                "the adapter diagnostic must identify the ungrounded reference: {error}"
            );
        });
    }

    #[test]
    fn test_evaluate_and_write_loads_all_track_catalogues_for_encode() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CatalogueEntryKey, CrateName, FieldDecl, FieldName, ModulePath,
            TypeRef,
        };

        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        crate::verify::test_support::git_init(root);
        let track_id = TrackId::try_new("cross-crate-handoff-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub struct Fixture;\n").unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
                    }
                },
                {
                    "crate": "infrastructure",
                    "path": "libs/infrastructure",
                    "may_depend_on": ["domain"],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "infrastructure-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                    }
                }
            ]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(rules)
            .unwrap()
            .into_iter()
            .find(|layer| layer.layer_id() == "infrastructure")
            .unwrap();

        let mut target = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        target.insert_type(
            CatalogueEntryKey::try_new("Handler".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![FieldDecl::new(
                            FieldName::new("id").unwrap(),
                            TypeRef::new("domain::model::UserId").unwrap(),
                        )],
                        has_stripped_fields: false,
                    },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                Some(ModulePath::root()),
                None,
                vec![],
                vec![],
            ),
        );

        let mut declaring = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        declaring.insert_type(
            CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(ModulePath::from_segments(vec!["model".to_owned()]).unwrap()),
                None,
                vec![],
                vec![],
            ),
        );

        let catalogue_json = CatalogueDocumentCodec::encode(&target).unwrap();
        let declaring_json = CatalogueDocumentCodec::encode(&declaring).unwrap();
        let baseline_json = rustdoc_json();
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let baseline_path = track_dir.join(binding.baseline_file());
        std::fs::write(&catalogue_path, &catalogue_json).unwrap();
        std::fs::write(&baseline_path, &baseline_json).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), declaring_json).unwrap();
        std::fs::write(track_dir.join("domain-types-baseline.json"), &baseline_json).unwrap();

        let trusted = items_dir.canonicalize().unwrap();
        let loaded = load_track_catalogues(root, &track_dir, &trusted)
            .expect("architecture-rules must select every TDDD-enabled catalogue");
        let loaded_documents = loaded
            .catalogues
            .iter()
            .map(|(layer, attested)| (layer.clone(), attested.document().clone()))
            .collect::<BTreeMap<_, _>>();
        let empty = rustdoc_crate_with_paths(HashMap::new());
        let target_layer = LayerId::try_new("infrastructure").unwrap();
        let declaring_layer = LayerId::try_new("domain").unwrap();
        let rustdoc_contexts = BTreeMap::from([
            (
                target_layer.clone(),
                AuthoritativeRustdocContext::new(
                    target_layer.clone(),
                    empty.clone(),
                    snapshot_for_context_test("infrastructure", &empty),
                ),
            ),
            (
                declaring_layer.clone(),
                AuthoritativeRustdocContext::new(
                    declaring_layer,
                    empty.clone(),
                    snapshot_for_context_test("domain", &empty),
                ),
            ),
        ]);
        let encoded = CatalogueToExtendedCrateCodec::new()
            .encode(&target_layer, &loaded_documents, &rustdoc_contexts)
            .expect("other-layer add declarations must join the evaluator encode map");
        let user_id = encoded
            .krate()
            .paths
            .iter()
            .find(|(_, summary)| summary.path == ["domain", "model", "UserId"])
            .map(|(id, summary)| (*id, summary.clone()))
            .expect("domain::model::UserId must be a declaring-crate external item");
        assert_ne!(user_id.1.crate_id, 0);
        assert_eq!(encoded.krate().external_crates[&user_id.1.crate_id].name, "domain");
        assert!(
            !encoded
                .krate()
                .paths
                .values()
                .any(|summary| summary.path == ["infrastructure", "model", "UserId"]),
            "the target catalogue must not duplicate the declaring-layer item"
        );

        let result = evaluate_and_write(
            catalogue_json.as_bytes(),
            &catalogue_path,
            &track_dir,
            root,
            &items_dir.canonicalize().unwrap(),
            &binding,
            baseline_json.clone(),
            read_head_commit(root).unwrap(),
            &baseline_path,
            &baseline_json,
            type_signals_codec::baseline_hash(baseline_json.as_bytes()),
        )
        .expect("evaluator encode must consume every track catalogue");
        assert_eq!(result, ExitCode::SUCCESS);

        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(track_dir.join(binding.signal_file())).unwrap(),
        )
        .unwrap();
        assert!(
            persisted.signals().iter().any(|signal| signal.type_name() == "Handler"),
            "other-layer add declarations must reach encode through the evaluator handoff"
        );
    }

    #[test]
    fn test_evaluate_and_write_rejects_stale_target_catalogue_input() {
        let (workspace, items_dir, track_id, binding, _rustdoc_path) = setup_workspace();
        let root = workspace.path();
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let baseline_path = track_dir.join(binding.baseline_file());
        let initial_catalogue = std::fs::read(&catalogue_path).unwrap();
        let changed_catalogue =
            format!("{} \n", std::str::from_utf8(&initial_catalogue).unwrap().trim_end());
        std::fs::write(&catalogue_path, changed_catalogue).unwrap();
        let baseline_json = std::fs::read_to_string(&baseline_path).unwrap();

        let error = evaluate_and_write(
            &initial_catalogue,
            &catalogue_path,
            &track_dir,
            root,
            &items_dir.canonicalize().unwrap(),
            &binding,
            rustdoc_json(),
            read_head_commit(root).unwrap(),
            &baseline_path,
            &baseline_json,
            type_signals_codec::baseline_hash(baseline_json.as_bytes()),
        )
        .expect_err("a stale caller catalogue must not mix with the attested target document");

        assert!(error.to_string().contains("target catalogue bytes changed"), "got: {error}");
    }

    #[test]
    fn test_load_track_catalogues_treats_enabled_layer_without_file_as_empty() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join("missing-layer-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
            "version": 2,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
                    }
                },
                {
                    "crate": "infrastructure",
                    "path": "libs/infrastructure",
                    "may_depend_on": ["domain"],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "infrastructure-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                    }
                }
            ]
        }"#,
        )
        .unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            CatalogueDocumentCodec::encode(&CatalogueDocument::new(
                5,
                CrateName::new("infrastructure").unwrap(),
                LayerId::try_new("infrastructure").unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();

        let loaded = load_track_catalogues(root, &track_dir, &items_dir.canonicalize().unwrap())
            .expect("missing enabled catalogues are empty, not errors");
        assert!(loaded.catalogues.contains_key(&LayerId::try_new("infrastructure").unwrap()));
        assert!(
            !loaded.catalogues.contains_key(&LayerId::try_new("domain").unwrap()),
            "a TDDD-enabled layer without a catalogue file must contribute no declarations"
        );
    }

    #[test]
    fn test_load_track_catalogues_uses_filename_stem_for_custom_catalogue_file() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join("custom-catalogue-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "custom.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        let document = CatalogueDocument::new(
            5,
            CrateName::new("custom").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        std::fs::write(
            track_dir.join("custom.json"),
            CatalogueDocumentCodec::encode(&document).unwrap(),
        )
        .unwrap();

        let loaded =
            load_track_catalogues(root, &track_dir, &items_dir.canonicalize().unwrap()).unwrap();
        assert_eq!(
            loaded
                .catalogues
                .get(&LayerId::try_new("infrastructure").unwrap())
                .unwrap()
                .document()
                .crate_name()
                .as_str(),
            "custom"
        );
    }

    #[test]
    fn test_load_track_catalogues_mismatched_layer_is_rejected() {
        use domain::tddd::catalogue_v2::{CatalogueDocument, CrateName};

        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let mismatched = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            CatalogueDocumentCodec::encode(&mismatched).unwrap(),
        )
        .unwrap();

        let error =
            load_track_catalogues(workspace.path(), &track_dir, &items_dir.canonicalize().unwrap())
                .expect_err("a catalogue layer must match its architecture binding");
        let message = error.to_string();
        assert!(message.contains("declares layer 'domain'"), "got: {message}");
        assert!(message.contains("binding selects 'infrastructure'"), "got: {message}");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_track_catalogues_dangling_symlink_is_rejected() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue_path = track_dir.join("infrastructure-types.json");
        std::fs::remove_file(&catalogue_path).unwrap();
        std::os::unix::fs::symlink(track_dir.join("missing-types.json"), &catalogue_path).unwrap();

        let error =
            load_track_catalogues(workspace.path(), &track_dir, &items_dir.canonicalize().unwrap())
                .expect_err("a dangling catalogue symlink must not be treated as absent");
        assert!(error.to_string().contains("symlink guard rejected catalogue"));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_track_catalogues_rejects_fifo_before_opening() {
        use std::time::Instant;

        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue_path = track_dir.join("infrastructure-types.json");
        std::fs::remove_file(&catalogue_path).unwrap();
        let status = std::process::Command::new("mkfifo").arg(&catalogue_path).status().unwrap();
        assert!(status.success(), "mkfifo must create the FIFO fixture");

        let started = Instant::now();
        let error =
            load_track_catalogues(workspace.path(), &track_dir, &items_dir.canonicalize().unwrap())
                .expect_err("a configured FIFO must fail closed");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "FIFO rejection must not block"
        );
        assert!(error.to_string().contains("not a regular file"), "got: {error}");
    }

    #[test]
    fn test_resolution_input_fingerprint_distinguishes_missing_from_empty_catalogue() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let trusted = items_dir.canonicalize().unwrap();
        let present = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        std::fs::remove_file(track_dir.join("infrastructure-types.json")).unwrap();
        let missing = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        std::fs::write(track_dir.join("infrastructure-types.json"), []).unwrap();
        let empty = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        assert_ne!(present, missing, "a missing catalogue must not hash as a present catalogue");
        assert_ne!(missing, empty, "a 0-byte catalogue must not hash as a missing catalogue");
        assert_ne!(present, empty);
    }

    #[test]
    fn test_resolution_input_fingerprint_includes_feature_declaration_bytes() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let trusted = items_dir.canonicalize().unwrap();
        let feature_path = track_dir.join(TDDD_FEATURE_DECLARATION_FILE);
        std::fs::write(&feature_path, r#"{"schema_version":1,"layers":{"infrastructure":[]}}"#)
            .unwrap();
        let first = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        std::fs::write(
            &feature_path,
            r#"{"schema_version":1,"layers":{"infrastructure":["serde"]}}"#,
        )
        .unwrap();
        let second = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        assert_ne!(first, second, "feature selection changes must invalidate resolution input");
    }

    #[test]
    fn test_verify_baseline_snapshots_rejects_changed_baseline() {
        let (workspace, items_dir, track_id, binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let baseline_path = track_dir.join(binding.baseline_file());
        let baseline = std::fs::read(&baseline_path).unwrap();
        let layer = LayerId::try_new(binding.layer_id().to_owned()).unwrap();
        let snapshots = BTreeMap::from([(
            layer,
            BaselineSnapshot {
                path: baseline_path.clone(),
                hash: type_signals_codec::baseline_hash(&baseline),
            },
        )]);

        std::fs::write(&baseline_path, format!("{}\n", rustdoc_json())).unwrap();
        let error =
            verify_baseline_snapshots_unchanged(&snapshots, &items_dir.canonicalize().unwrap())
                .expect_err("a changed authoritative baseline must block persistence");

        assert!(
            error.to_string().contains("baseline for layer 'infrastructure' changed"),
            "got: {error}"
        );
        let _ = workspace;
    }

    #[cfg(unix)]
    #[test]
    fn test_resolution_input_fingerprint_rejects_symlinked_architecture_rules() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let trusted = items_dir.canonicalize().unwrap();
        let rules_path = workspace.path().join("architecture-rules.json");
        let real_rules = workspace.path().join("architecture-rules.real.json");
        std::fs::rename(&rules_path, &real_rules).unwrap();
        std::os::unix::fs::symlink(&real_rules, &rules_path).unwrap();

        let error = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted)
            .expect_err("a symlinked architecture-rules.json must fail closed");
        assert!(
            error.to_string().contains("symlink guard rejected architecture-rules.json"),
            "got: {error}"
        );
    }

    #[test]
    fn test_execute_type_signals_resolves_mutual_add_and_modify_references_in_declaration_order() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueEntryKey, CrateName, FieldDecl, FieldName, ModulePath, TypeRef,
        };

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        // Insert the modify declaration before the add declarations. The production
        // pre-pass must still resolve every reference independently of declaration order.
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Modify,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![FieldDecl::new(
                            FieldName::new("first").unwrap(),
                            TypeRef::new("First").unwrap(),
                        )],
                        has_stripped_fields: false,
                    },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                Some(ModulePath::root()),
                None,
                vec![],
                vec![],
            ),
        );
        let first = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("second").unwrap(),
                        TypeRef::new("Second").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            None,
            None,
            vec![],
            vec![],
        );
        catalogue.insert_type(CatalogueEntryKey::try_new("First".to_owned()).unwrap(), first);
        let second = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("first").unwrap(),
                        TypeRef::new("First").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            None,
            None,
            vec![],
            vec![],
        );
        catalogue.insert_type(CatalogueEntryKey::try_new("Second".to_owned()).unwrap(), second);

        let baseline = rustdoc_crate_with_paths(HashMap::from([(
            rustdoc_types::Id(1),
            rustdoc_types::ItemSummary {
                crate_id: 0,
                path: vec!["infrastructure".to_owned(), "Holder".to_owned()],
                kind: rustdoc_types::ItemKind::Struct,
            },
        )]));
        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &baseline)
            .expect("the shared set must include both add declarations and the modify baseline");
        let identity_index = build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("all declared identities must resolve through the shared set");
        let _ = identity_index;
        let encoded = encode_document(catalogue, &baseline, &baseline)
            .expect("mutual add and modify-to-add references must encode");

        let id_for = |path: &[&str]| {
            let expected = path.iter().map(|segment| (*segment).to_owned()).collect::<Vec<_>>();
            encoded
                .krate()
                .paths
                .iter()
                .find(|(_, summary)| summary.path == expected)
                .map(|(id, _)| *id)
                .expect("encoded identity must have an authoritative path")
        };
        let first_id = id_for(&["infrastructure", "First"]);
        let second_id = id_for(&["infrastructure", "Second"]);
        let holder_id = id_for(&["infrastructure", "Holder"]);
        let field_target = |owner_id| {
            let rustdoc_types::ItemEnum::Struct(owner) = &encoded.krate().index[&owner_id].inner
            else {
                panic!("expected a struct owner")
            };
            let rustdoc_types::StructKind::Plain { fields, .. } = &owner.kind else {
                panic!("expected a plain struct owner")
            };
            let field_id = fields.first().expect("expected one reference-bearing field");
            let rustdoc_types::ItemEnum::StructField(rustdoc_types::Type::ResolvedPath(path)) =
                &encoded.krate().index[field_id].inner
            else {
                panic!("expected a resolved field reference")
            };
            path.id
        };

        assert_eq!(field_target(first_id), second_id);
        assert_eq!(field_target(second_id), first_id);
        assert_eq!(field_target(holder_id), first_id);
    }

    #[test]
    fn test_execute_type_signals_resolves_all_type_actions_and_modify_function_under_bin_root_alias()
     {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{FunctionEntry, TypeEntry};
        use domain::tddd::catalogue_v2::roles::{DataRole, FunctionRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueEntryKey, CrateName, FunctionName, FunctionPath, ModulePath, TypeRef,
        };

        let rooted_crate = |paths: Vec<(u32, Vec<&str>, rustdoc_types::ItemKind)>| {
            let path_map = paths
                .into_iter()
                .map(|(id, path, kind)| {
                    (
                        rustdoc_types::Id(id),
                        rustdoc_types::ItemSummary {
                            crate_id: 0,
                            path: path.into_iter().map(str::to_owned).collect(),
                            kind,
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            let mut krate = rustdoc_crate_with_paths(path_map);
            krate.index.insert(
                rustdoc_types::Id(0),
                rustdoc_types::Item {
                    id: rustdoc_types::Id(0),
                    crate_id: 0,
                    name: Some("sotp".to_owned()),
                    span: None,
                    visibility: rustdoc_types::Visibility::Public,
                    docs: None,
                    links: HashMap::new(),
                    attrs: vec![],
                    deprecation: None,
                    inner: rustdoc_types::ItemEnum::Module(rustdoc_types::Module {
                        is_crate: true,
                        items: vec![],
                        is_stripped: false,
                    }),
                },
            );
            krate
        };
        let type_entry = |action| {
            TypeEntry::new(
                action,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            )
        };

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("cli").unwrap(),
            LayerId::try_new("cli").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Added".to_owned()).unwrap(),
            type_entry(ItemAction::Add),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Modified".to_owned()).unwrap(),
            type_entry(ItemAction::Modify),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Referenced".to_owned()).unwrap(),
            type_entry(ItemAction::Reference),
        );
        catalogue.insert_function(
            FunctionPath::new(
                CrateName::new("cli").unwrap(),
                ModulePath::from_segments(vec!["commands".to_owned()]).unwrap(),
                FunctionName::new("run").unwrap(),
            ),
            FunctionEntry::new(
                ItemAction::Modify,
                FunctionRole::FreeFunction,
                vec![],
                TypeRef::new("()").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            ),
        );

        let baseline = rooted_crate(vec![
            (1, vec!["sotp", "commands", "Modified"], rustdoc_types::ItemKind::Struct),
            (2, vec!["sotp", "commands", "Referenced"], rustdoc_types::ItemKind::Struct),
            (3, vec!["sotp", "commands", "run"], rustdoc_types::ItemKind::Function),
        ]);
        let current = rooted_crate(vec![
            (1, vec!["sotp", "commands", "Modified"], rustdoc_types::ItemKind::Struct),
            (2, vec!["sotp", "commands", "Referenced"], rustdoc_types::ItemKind::Struct),
            (3, vec!["sotp", "commands", "run"], rustdoc_types::ItemKind::Function),
            (4, vec!["sotp", "generated", "Added"], rustdoc_types::ItemKind::Struct),
        ]);
        let package = CrateName::new("cli").unwrap();
        let normalized_current = normalized_paths_for_doc(&current, &package);
        for expected in [
            vec!["cli", "commands", "Modified"],
            vec!["cli", "commands", "Referenced"],
            vec!["cli", "commands", "run"],
            vec!["cli", "generated", "Added"],
        ] {
            assert!(
                normalized_current.values().any(|summary| summary.path == expected),
                "root alias must be normalized once for {expected:?}"
            );
        }

        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &current)
            .expect("all type actions must share one normalized resolution set");
        build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("type-signal indexing must use the same normalized identities");
        let encoded = encode_document(catalogue, &baseline, &current)
            .expect("add, modify, reference, and function identities must encode under alias");

        let id_for = |path: &[&str]| {
            let expected = path.iter().map(|segment| (*segment).to_owned()).collect::<Vec<_>>();
            encoded
                .krate()
                .paths
                .iter()
                .find(|(_, summary)| summary.path == expected)
                .map(|(id, _)| *id)
                .expect("encoded path must be present")
        };
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "generated", "Added"])),
            Some(ItemAction::Add)
        );
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "commands", "Modified"])),
            Some(ItemAction::Modify)
        );
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "commands", "Referenced"])),
            Some(ItemAction::Reference)
        );
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "commands", "run"])),
            Some(ItemAction::Modify)
        );
    }

    fn exclusive_observer_json_path(root: &Path, file_name: &str) -> PathBuf {
        let dir = root.join(".sotp-rustdoc").join("fixture");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(file_name)
    }

    #[cfg(unix)]
    fn prepend_test_command_path(directory: &Path) -> std::ffi::OsString {
        let mut entries = vec![directory.to_path_buf()];
        entries.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        std::env::join_paths(entries).unwrap()
    }

    #[cfg(unix)]
    fn minimal_test_command_path(directory: &Path) -> std::ffi::OsString {
        std::env::join_paths([directory, Path::new("/usr/bin"), Path::new("/bin")]).unwrap()
    }

    #[cfg(unix)]
    fn write_evaluator_test_toolchain(commands: &Path, metadata: &str, rustdoc_command: &str) {
        write_test_rustup(commands);
        write_test_executable(&commands.join("rustc"), "#!/bin/sh\nexit 0\n");
        write_test_executable(&commands.join("rustdoc"), "#!/bin/sh\nexit 0\n");
        let cargo = format!(
            "#!/bin/sh\nif [ \"$1\" = \"metadata\" ]; then\nprintf '%s\\n' '{metadata}'\nexit 0\nfi\n{rustdoc_command}\n"
        );
        write_test_executable(&commands.join("cargo"), &cargo);
    }

    #[cfg(unix)]
    fn metadata_test_output(target_directory: &Path, marker: &str) -> Vec<u8> {
        serde_json::json!({
            "packages": [],
            "target_directory": target_directory,
            "metadata_marker": marker,
        })
        .to_string()
        .into_bytes()
    }

    struct TestWorkspace {
        _temp_dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl TestWorkspace {
        fn path(&self) -> &Path {
            &self.root
        }
    }

    fn setup_workspace() -> (TestWorkspace, PathBuf, TrackId, TdddLayerBinding, PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        crate::verify::test_support::git_init(&root);
        let track_id = TrackId::try_new("feature-input-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(root.join("libs/infrastructure/src")).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/infrastructure\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"infrastructure\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("libs/infrastructure/Cargo.toml"),
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("libs/infrastructure/src/lib.rs"), "pub struct Fixture;\n")
            .unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            "{\n  \"schema_version\": 5,\n  \"crate_name\": \"infrastructure\",\n  \"layer\": \"infrastructure\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
        )
        .unwrap();
        let rustdoc_path = exclusive_observer_json_path(&root, "infrastructure-rustdoc.json");
        let json = rustdoc_json();
        std::fs::write(&rustdoc_path, &json).unwrap();
        std::fs::write(track_dir.join("infrastructure-types-baseline.json"), json).unwrap();
        std::fs::write(
            root.join(".gitignore"),
            "track/items/feature-input-track/infrastructure-type-signals.json\n",
        )
        .unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        std::fs::write(track_dir.join("plan.md"), "fixture plan\n").unwrap();
        crate::verify::test_support::run_git(&root, &["add", "."]);
        crate::verify::test_support::run_git(&root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(rules).unwrap().pop().unwrap();

        (TestWorkspace { _temp_dir: workspace, root }, items_dir, track_id, binding, rustdoc_path)
    }

    fn signal_path(items_dir: &Path, track_id: &TrackId) -> PathBuf {
        items_dir.join(track_id.as_ref()).join("infrastructure-type-signals.json")
    }

    fn cache_document(
        items_dir: &Path,
        track_id: &TrackId,
        head_commit: domain::CommitHash,
    ) -> TypeSignalsDocument {
        let track_dir = items_dir.join(track_id.as_ref());
        let workspace_root = items_dir.parent().and_then(Path::parent).unwrap();
        let catalogue = std::fs::read(track_dir.join("infrastructure-types.json")).unwrap();
        let (_, baseline_hash) =
            read_actual_baseline(&track_dir.join("infrastructure-types-baseline.json"), items_dir)
                .unwrap();
        let implementation = freshness::rustdoc_input_fingerprint(workspace_root).unwrap();
        let resolution =
            resolution_input_fingerprint(workspace_root, &track_dir, items_dir).unwrap();
        let rustdoc_path =
            exclusive_observer_json_path(workspace_root, "infrastructure-rustdoc.json");
        let rustdoc_path = std::fs::canonicalize(&rustdoc_path).unwrap();
        let target =
            ResolvedCargoTargetDirectory::try_new(rustdoc_path.parent().unwrap().to_path_buf())
                .unwrap();
        let expected = ExpectedRustdocJsonPath::try_new(rustdoc_path, &target).unwrap();
        let identity = RustdocExecutionIdentity::new(
            target,
            CrateName::new("infrastructure").unwrap(),
            vec![],
            CargoProfileName::try_new("dev".to_owned()).unwrap(),
            expected,
        )
        .unwrap();
        TypeSignalsDocument::new(
            Timestamp::new("2026-07-31T00:00:00Z").unwrap(),
            TypeSignalsCacheKey::new(
                type_signals_codec::declaration_hash(&catalogue),
                head_commit,
                baseline_hash,
                ImplementationFingerprint::new(Sha256Digest::try_new(implementation).unwrap()),
                ResolutionFingerprint::new(resolution.as_digest().clone()),
                identity,
            ),
            vec![],
        )
    }

    fn current_cache_document(
        workspace_root: &Path,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> TypeSignalsDocument {
        cache_document(items_dir, track_id, read_head_commit(workspace_root).unwrap())
    }

    fn write_cache(
        items_dir: &Path,
        track_id: &TrackId,
        head_commit: domain::CommitHash,
    ) -> String {
        let document = cache_document(items_dir, track_id, head_commit);
        let encoded = type_signals_codec::encode(&document).unwrap();
        std::fs::write(signal_path(items_dir, track_id), &encoded).unwrap();
        encoded
    }

    fn execute_with_encoded_crate_observer(
        items_dir: &Path,
        track_id: &TrackId,
        workspace_root: &Path,
        binding: &TdddLayerBinding,
        rustdoc: &RustdocLaunchObserver,
        encoded_observer: &dyn Fn(&domain::tddd::ExtendedCrate),
    ) -> Result<ExitCode, EvaluateSignalsError> {
        let context_cache = RustdocContextCache::default();
        execute_with_dependencies(
            items_dir,
            track_id,
            workspace_root,
            binding,
            &[],
            rustdoc,
            &context_cache,
            EvaluationObservers {
                resolution_paths: None,
                encoded_crate: Some(encoded_observer),
                before_reuse: None,
            },
        )
    }

    #[test]
    fn test_execute_type_signals_clean_worktree_and_matching_head_reuses_cache() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            assert_eq!(
                execute_type_signals_for_layer_with_launch_observer(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &observer,
                )
                .unwrap(),
                ExitCode::SUCCESS
            );
            assert_eq!(observer.launches(), 0);
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_metadata_output_change_invalidates_reuse() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let commands = tempfile::tempdir().unwrap();
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            let target_directory = workspace.path().join("cargo-target");
            std::fs::write(&metadata_path, metadata_test_output(&target_directory, "generation-a"))
                .unwrap();
            write_metadata_test_toolchain(commands.path());
            let path = prepend_test_command_path(commands.path());

            temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("CARGO_TARGET_DIR", None::<&std::ffi::OsStr>),
                ],
                || {
                    let original = write_cache(
                        &items_dir,
                        &track_id,
                        read_head_commit(workspace.path()).unwrap(),
                    );
                    std::fs::write(
                        &metadata_path,
                        metadata_test_output(&target_directory, "generation-b"),
                    )
                    .unwrap();
                    let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

                    execute_type_signals_for_layer_with_launch_observer(
                        &items_dir,
                        &track_id,
                        workspace.path(),
                        &binding,
                        &[],
                        &observer,
                    )
                    .expect("changed Cargo metadata must trigger a fresh evaluation");

                    assert_eq!(observer.launches(), 1);
                    let persisted = type_signals_codec::decode_with_workspace(
                        &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                        workspace.path(),
                    )
                    .unwrap();
                    assert_eq!(
                        persisted.cache_key(),
                        current_cache_document(workspace.path(), &items_dir, &track_id).cache_key()
                    );
                    assert_ne!(
                        persisted.cache_key().implementation_fingerprint(),
                        type_signals_codec::decode(&original)
                            .unwrap()
                            .cache_key()
                            .implementation_fingerprint(),
                        "metadata output must change the implementation cache identity"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_metadata_failure_does_not_reuse_existing_cache() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let commands = tempfile::tempdir().unwrap();
            write_test_executable(&commands.path().join("cargo"), "#!/bin/sh\nexit 1\n");
            let path = minimal_test_command_path(commands.path());
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            let error = temp_env::with_var("PATH", Some(path.as_os_str()), || {
                execute_type_signals_for_layer_with_launch_observer(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &observer,
                )
                .expect_err("Cargo metadata failure must fail before cache reuse")
            });

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("cargo metadata"),
                "metadata failure must remain an authoritative-input error: {error}"
            );
            assert_eq!(observer.launches(), 0, "metadata failure must not launch rustdoc");
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original,
                "metadata failure must not replace the old cache"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_path_tool_resolution_failure_does_not_reuse_existing_cache() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let commands = tempfile::tempdir().unwrap();
            write_test_executable(
                &commands.path().join("cargo"),
                "#!/bin/sh\nprintf '%s\\n' '{\"packages\":[],\"target_directory\":\"target\"}'\n",
            );
            let path = minimal_test_command_path(commands.path());
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            let error = temp_env::with_var("PATH", Some(path.as_os_str()), || {
                execute_type_signals_for_layer_with_launch_observer(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &observer,
                )
                .expect_err("PATH tool resolution failure must fail before cache reuse")
            });

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("rustc")
                    || error.to_string().contains("rustdoc")
                    || error.to_string().contains("PATH"),
                "PATH resolution failure must remain authoritative: {error}"
            );
            assert_eq!(observer.launches(), 0, "PATH failure must not launch rustdoc");
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original,
                "PATH failure must not replace the old cache"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_rustdoc_input_fingerprint_uses_nightly_selected_tools_not_path_proxies() {
        with_process_environment_lock(|| {
            let (workspace, _items_dir, _track_id, _binding, _rustdoc_path) = setup_workspace();
            let commands = tempfile::tempdir().unwrap();
            let nightly = tempfile::tempdir().unwrap();
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            std::fs::write(
                &metadata_path,
                metadata_test_output(&target_directory, "nightly-tool-selection"),
            )
            .unwrap();

            write_test_executable(
                &commands.path().join("cargo"),
                "#!/bin/sh\nexec /bin/cat \"$SOTOHE_TEST_CARGO_METADATA\"\n",
            );
            write_test_executable(&commands.path().join("rustc"), "proxy rustc generation-a\n");
            write_test_executable(&commands.path().join("rustdoc"), "proxy rustdoc generation-a\n");
            write_test_rustup(commands.path());
            for tool in ["cargo", "rustc", "rustdoc"] {
                write_test_executable(
                    &nightly.path().join(tool),
                    &format!("nightly {tool} generation-a\n"),
                );
            }

            let path = minimal_test_command_path(commands.path());
            temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(nightly.path().as_os_str())),
                    ("CARGO_TARGET_DIR", None::<&std::ffi::OsStr>),
                ],
                || {
                    let baseline = freshness::rustdoc_input_fingerprint(workspace.path()).unwrap();
                    for tool in ["cargo", "rustc", "rustdoc"] {
                        write_test_executable(
                            &nightly.path().join(tool),
                            &format!("nightly {tool} generation-b\n"),
                        );
                        let selected_changed =
                            freshness::rustdoc_input_fingerprint(workspace.path()).unwrap();
                        assert_ne!(
                            baseline, selected_changed,
                            "nightly-selected {tool} content must be part of the fingerprint"
                        );

                        write_test_executable(
                            &nightly.path().join(tool),
                            &format!("nightly {tool} generation-a\n"),
                        );
                        let restored =
                            freshness::rustdoc_input_fingerprint(workspace.path()).unwrap();
                        assert_eq!(
                            baseline, restored,
                            "restoring nightly-selected {tool} must restore the baseline fingerprint"
                        );
                    }

                    write_test_executable(
                        &commands.path().join("cargo"),
                        "#!/bin/sh\n# proxy generation-b\nexec /bin/cat \"$SOTOHE_TEST_CARGO_METADATA\"\n",
                    );
                    write_test_executable(
                        &commands.path().join("rustc"),
                        "proxy rustc generation-b\n",
                    );
                    write_test_executable(
                        &commands.path().join("rustdoc"),
                        "proxy rustdoc generation-b\n",
                    );
                    let proxy_changed =
                        freshness::rustdoc_input_fingerprint(workspace.path()).unwrap();
                    assert_eq!(
                        proxy_changed, baseline,
                        "changing PATH proxy contents must not change the nightly tool fingerprint"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_nightly_tool_snapshot_failure_is_authoritative() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let commands = tempfile::tempdir().unwrap();
            let nightly = tempfile::tempdir().unwrap();
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            std::fs::write(
                &metadata_path,
                metadata_test_output(&target_directory, "nightly-tool-failure"),
            )
            .unwrap();

            write_test_executable(
                &commands.path().join("cargo"),
                "#!/bin/sh\nexec /bin/cat \"$SOTOHE_TEST_CARGO_METADATA\"\n",
            );
            write_test_executable(&commands.path().join("rustc"), "proxy rustc\n");
            write_test_executable(&commands.path().join("rustdoc"), "proxy rustdoc\n");
            write_test_executable(&nightly.path().join("cargo"), "nightly cargo\n");
            write_test_executable(&nightly.path().join("rustc"), "nightly rustc\n");
            let missing_rustdoc = nightly.path().join("rustdoc-missing");
            write_test_executable(
                &commands.path().join("rustup"),
                r#"#!/bin/sh
if [ "$1" = "which" ] && [ "$2" = "--toolchain" ] && [ "$3" = "nightly" ]; then
    case "$4" in
        rustdoc)
            printf '%s\n' "$SOTOHE_TEST_MISSING_NIGHTLY_RUSTDOC"
            ;;
        cargo|rustc)
            printf '%s/%s\n' "$SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR" "$4"
            ;;
        *)
            exit 1
            ;;
    esac
    exit 0
fi
if [ "$1" = "run" ] && [ "$2" = "nightly" ] && [ "$3" = "rustc" ]; then
    exit 0
fi
exit 1
"#,
            );

            let path = minimal_test_command_path(commands.path());
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let error = temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(nightly.path().as_os_str())),
                    ("SOTOHE_TEST_MISSING_NIGHTLY_RUSTDOC", Some(missing_rustdoc.as_os_str())),
                    ("CARGO_TARGET_DIR", None::<&std::ffi::OsStr>),
                ],
                || {
                    execute_type_signals_for_layer_with_launch_observer(
                        &items_dir,
                        &track_id,
                        workspace.path(),
                        &binding,
                        &[],
                        &observer,
                    )
                    .expect_err("a missing nightly-selected tool must fail before cache reuse")
                },
            );

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("rustdoc"),
                "the selected-tool failure must identify the missing tool: {error}"
            );
            assert_eq!(observer.launches(), 0, "tool resolution failure must not launch rustdoc");
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original,
                "tool resolution failure must not replace the old cache"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_accepts_aba_generation_before_cache_reuse() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original_cache =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let rules_path = workspace.path().join("architecture-rules.json");
            let original_rules = std::fs::read(&rules_path).unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let rules_path_for_mutation = rules_path.clone();
            let original_for_mutation = original_rules.clone();
            let mutate_aba = move || {
                let replacement_b =
                    rules_path_for_mutation.with_file_name("architecture-rules.b.tmp");
                std::fs::write(&replacement_b, b"{\n  \"version\": 2,\n  \"layers\": []\n}\n")
                    .unwrap();
                std::fs::rename(&replacement_b, &rules_path_for_mutation).unwrap();
                let replacement_a =
                    rules_path_for_mutation.with_file_name("architecture-rules.a.tmp");
                std::fs::write(&replacement_a, &original_for_mutation).unwrap();
                std::fs::rename(&replacement_a, &rules_path_for_mutation).unwrap();
            };
            let context_cache = RustdocContextCache::default();

            let result = execute_with_dependencies(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
                &context_cache,
                EvaluationObservers {
                    resolution_paths: None,
                    encoded_crate: None,
                    before_reuse: Some(&mutate_aba),
                },
            )
            .expect("an A-B-A path replacement returning to the start bytes is reusable");

            assert_eq!(std::fs::read(&rules_path).unwrap(), original_rules);
            assert_eq!(result, ExitCode::SUCCESS);
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original_cache,
                "an ABA generation must not alter the existing cache"
            );
            assert_eq!(observer.launches(), 0, "cache reuse must not launch rustdoc");
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_rejects_head_change_before_cache_reuse() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original_cache =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let root = workspace.path().to_path_buf();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let mutate_head = move || {
                crate::verify::test_support::run_git(
                    &root,
                    &["commit", "--quiet", "--allow-empty", "-m", "cache reuse head change"],
                );
            };
            let context_cache = RustdocContextCache::default();

            let error = execute_with_dependencies(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
                &context_cache,
                EvaluationObservers {
                    resolution_paths: None,
                    encoded_crate: None,
                    before_reuse: Some(&mutate_head),
                },
            )
            .expect_err("a HEAD change must fail before cache reuse");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("HEAD changed"),
                "the cache-reuse path must report the changed HEAD: {error}"
            );
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original_cache,
                "a changed HEAD must not alter the existing cache"
            );
            assert_eq!(observer.launches(), 0, "cache reuse must not launch rustdoc");
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_rejects_dirty_worktree_before_cache_reuse() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original_cache =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            let operational_path =
                workspace.path().join("track/items").join(track_id.as_ref()).join("plan.md");
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let mutate_worktree = move || {
                std::fs::write(&operational_path, "changed fixture plan\n").unwrap();
            };
            let context_cache = RustdocContextCache::default();

            let error = execute_with_dependencies(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
                &context_cache,
                EvaluationObservers {
                    resolution_paths: None,
                    encoded_crate: None,
                    before_reuse: Some(&mutate_worktree),
                },
            )
            .expect_err("a dirty worktree must fail before cache reuse");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("worktree changed"),
                "the cache-reuse path must report the dirty worktree: {error}"
            );
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original_cache,
                "a dirty worktree must not alter the existing cache"
            );
            assert_eq!(observer.launches(), 0, "cache reuse must not launch rustdoc");
        });
    }

    #[test]
    fn test_execute_type_signals_dirty_worktree_recalculates_even_with_matching_cache() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let head_commit = read_head_commit(workspace.path()).unwrap();
            write_cache(&items_dir, &track_id, head_commit);
            std::fs::write(
                workspace.path().join("libs/infrastructure/src/lib.rs"),
                "pub struct ChangedFixture;\n",
            )
            .unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .unwrap();

            assert_eq!(observer.launches(), 1);
            let persisted = type_signals_codec::decode_with_workspace(
                &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                workspace.path(),
            )
            .unwrap();
            assert_eq!(
                persisted.cache_key(),
                current_cache_document(workspace.path(), &items_dir, &track_id).cache_key()
            );
        });
    }

    #[test]
    fn test_execute_type_signals_context_cache_reextracts_after_source_change() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let context_cache = RustdocContextCache::default();
            let first_observer = RustdocLaunchObserver::using_json_path(rustdoc_path.clone());
            execute_type_signals_for_layer_with_launch_observer_and_context_cache(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &first_observer,
                &context_cache,
            )
            .unwrap();
            assert_eq!(first_observer.launches(), 1);

            std::fs::write(
                workspace.path().join("libs/infrastructure/src/lib.rs"),
                "pub struct ChangedFixture;",
            )
            .unwrap();
            let second_observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            execute_type_signals_for_layer_with_launch_observer_and_context_cache(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &second_observer,
                &context_cache,
            )
            .unwrap();
            assert_eq!(
                second_observer.launches(),
                1,
                "a changed implementation must not reuse a run-local rustdoc context"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_implementation_change_before_persist() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let source_path = workspace.path().join("libs/infrastructure/src/lib.rs");
            let observer = RustdocLaunchObserver::using_json_path_with_before_export(
                rustdoc_path,
                std::sync::Arc::new(move || {
                    std::fs::write(&source_path, "pub struct ChangedDuringExport;\n").unwrap();
                }),
            );

            let error = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .expect_err("a source change during export must block persistence");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("changed during type-signal evaluation"),
                "the error must identify the changed implementation: {error}"
            );
            assert!(
                !signal_path(&items_dir, &track_id).exists(),
                "a changed implementation must not publish a signal artifact"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_changed_generation_before_persist() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let source_path = workspace.path().join("libs/infrastructure/src/lib.rs");
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let mutate_source = move |_: &domain::tddd::ExtendedCrate| {
                std::fs::write(&source_path, "pub struct ChangedGeneration;\n").unwrap();
            };

            let error = execute_with_encoded_crate_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &observer,
                &mutate_source,
            )
            .expect_err("a changed implementation generation must fail closed");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("changed during type-signal evaluation"),
                "the changed generation must be reported: {error}"
            );
            assert!(
                !signal_path(&items_dir, &track_id).exists(),
                "a changed generation must not publish a signal artifact"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_discards_result_before_persist_when_resolution_changes() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let signal = signal_path(&items_dir, &track_id);
            let prior = b"prior-cache-generation";
            std::fs::write(&signal, prior).unwrap();
            let rules_path = workspace.path().join("architecture-rules.json");
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let mutate_resolution = move |_: &domain::tddd::ExtendedCrate| {
                let mut changed = std::fs::read(&rules_path).unwrap();
                changed.extend_from_slice(b"\n ");
                std::fs::write(&rules_path, changed).unwrap();
            };

            let error = execute_with_encoded_crate_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &observer,
                &mutate_resolution,
            )
            .expect_err("a changed resolution generation must discard the result");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("architecture-rules")
                    || error.to_string().contains("changed during type-signal evaluation"),
                "the resolution change must be reported: {error}"
            );
            assert_eq!(
                std::fs::read(&signal).unwrap(),
                prior,
                "discard-before-persist must preserve the prior cache bytes"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_rejects_path_reread_generation_change() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let catalogue_path = items_dir.join(track_id.as_ref()).join(binding.catalogue_file());
            let mut changed = std::fs::read(&catalogue_path).unwrap();
            changed.extend_from_slice(b"\n ");
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let mutate_catalogue = move |_: &domain::tddd::ExtendedCrate| {
                std::fs::write(&catalogue_path, &changed).unwrap();
            };

            let error = execute_with_encoded_crate_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &observer,
                &mutate_catalogue,
            )
            .expect_err("a changed catalogue path must not be accepted on reread");

            assert!(matches!(error, EvaluateSignalsError::AuthoritativeInput(_)));
            assert!(
                error.to_string().contains("architecture-rules")
                    || error.to_string().contains("track catalogues"),
                "the path reread must detect the changed generation: {error}"
            );
            assert!(
                !signal_path(&items_dir, &track_id).exists(),
                "a changed catalogue generation must not publish a signal artifact"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_accepts_aba_generation_after_path_restoration() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let rules_path = workspace.path().join("architecture-rules.json");
            let original = std::fs::read(&rules_path).unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let rules_path_for_mutation = rules_path.clone();
            let original_for_mutation = original.clone();
            let mutate_aba = move |_: &domain::tddd::ExtendedCrate| {
                let replacement_b =
                    rules_path_for_mutation.with_file_name("architecture-rules.b.tmp");
                std::fs::write(&replacement_b, b"{\n  \"version\": 2,\n  \"layers\": []\n}\n")
                    .unwrap();
                std::fs::rename(&replacement_b, &rules_path_for_mutation).unwrap();
                let replacement_a =
                    rules_path_for_mutation.with_file_name("architecture-rules.a.tmp");
                std::fs::write(&replacement_a, &original_for_mutation).unwrap();
                std::fs::rename(&replacement_a, &rules_path_for_mutation).unwrap();
            };

            let result = execute_with_encoded_crate_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &observer,
                &mutate_aba,
            )
            .expect("an A-B-A path replacement returning to the start bytes must persist");

            assert_eq!(std::fs::read(&rules_path).unwrap(), original);
            assert_eq!(result, ExitCode::SUCCESS);
            assert!(
                signal_path(&items_dir, &track_id).exists(),
                "an A-B-A generation returning to the start bytes must publish the result"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_context_cache_invalidates_for_generated_semantic_inputs() {
        with_process_environment_lock(|| {
            let semantic_inputs = [
                ("Cargo.toml", "\n# generated manifest change\n"),
                ("Cargo.lock", "\n# generated lockfile change\n"),
                (".cargo/config.toml", "[build]\nrustflags = []\n"),
                ("rust-toolchain.toml", "[toolchain]\nchannel = \"stable\"\n"),
                ("build-input.json", "{\"generated\":true}\n"),
            ];

            for (relative_path, replacement) in semantic_inputs {
                let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
                let context_cache = RustdocContextCache::default();
                let first_observer = RustdocLaunchObserver::using_json_path(rustdoc_path.clone());
                execute_type_signals_for_layer_with_launch_observer_and_context_cache(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &first_observer,
                    &context_cache,
                )
                .unwrap_or_else(|error| {
                    panic!("initial evaluation must succeed for {relative_path}: {error}")
                });
                assert_eq!(first_observer.launches(), 1, "initial evaluation must build a context");

                let changed_path = workspace.path().join(relative_path);
                if let Some(parent) = changed_path.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                let mut changed = std::fs::read(&changed_path).unwrap_or_default();
                changed.extend_from_slice(replacement.as_bytes());
                std::fs::write(&changed_path, changed).unwrap();

                let second_observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
                execute_type_signals_for_layer_with_launch_observer_and_context_cache(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &second_observer,
                    &context_cache,
                )
                .unwrap_or_else(|error| {
                    panic!("reevaluation must succeed for {relative_path}: {error}")
                });
                assert_eq!(
                    second_observer.launches(),
                    1,
                    "a changed semantic input must invalidate the context cache: {relative_path}"
                );
            }
        });
    }

    #[test]
    fn test_rustdoc_input_fingerprint_oversized_file_returns_typed_limit_error() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"fingerprint-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(workspace.path().join("src")).unwrap();
        std::fs::write(workspace.path().join("src/lib.rs"), "pub struct Fixture;\n").unwrap();
        let oversized = workspace.path().join("oversized-input.bin");
        std::fs::File::create(&oversized).unwrap().set_len(64 * 1024 * 1024 + 1).unwrap();

        let error = freshness::rustdoc_input_fingerprint(workspace.path())
            .expect_err("an oversized rustdoc input must fail closed");
        assert!(matches!(error, freshness::RustdocInputFingerprintError::FileBytes { .. }));
    }

    #[test]
    fn test_rustdoc_input_fingerprint_includes_nested_track_source_directory() {
        with_process_environment_lock(|| {
            let (workspace, _items_dir, _track_id, _binding, _rustdoc_path) = setup_workspace();
            let nested_track = workspace.path().join("libs/infrastructure/src/track");
            std::fs::create_dir_all(&nested_track).unwrap();

            let before = freshness::rustdoc_input_fingerprint(workspace.path()).unwrap();
            std::fs::write(nested_track.join("generated.rs"), "pub struct Generated;\n").unwrap();
            let after = freshness::rustdoc_input_fingerprint(workspace.path()).unwrap();

            assert_ne!(
                before, after,
                "a nested production track directory must not be treated as operational state"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_clean_worktree_with_different_head_recalculates() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            write_cache(
                &items_dir,
                &track_id,
                domain::CommitHash::try_new("b".repeat(40)).unwrap(),
            );
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .unwrap();

            assert_eq!(observer.launches(), 1);
            let persisted = type_signals_codec::decode_with_workspace(
                &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                workspace.path(),
            )
            .unwrap();
            assert_eq!(
                persisted.cache_key(),
                current_cache_document(workspace.path(), &items_dir, &track_id).cache_key()
            );
        });
    }

    #[test]
    fn test_execute_type_signals_missing_or_invalid_cache_is_replaced_atomically() {
        with_process_environment_lock(|| {
            let invalid_cache_variants = [
                ("missing", None),
                ("malformed", Some("not-json".to_owned())),
                (
                    "schema-mismatch",
                    Some(
                        serde_json::json!({
                            "schema_version": 999,
                            "generated_at": "2026-07-31T00:00:00Z",
                            "declaration_hash": "a".repeat(64),
                            "head_commit": "b".repeat(40),
                            "baseline_hash": "a".repeat(64),
                            "signals": []
                        })
                        .to_string(),
                    ),
                ),
                (
                    "missing-required-field",
                    Some(
                        serde_json::json!({
                            "schema_version": 4,
                            "generated_at": "2026-07-31T00:00:00Z",
                            "declaration_hash": "a".repeat(64),
                            "baseline_hash": "a".repeat(64),
                            "signals": []
                        })
                        .to_string(),
                    ),
                ),
                (
                    "invalid-value",
                    Some(
                        serde_json::json!({
                            "schema_version": 4,
                            "generated_at": "2026-07-31T00:00:00Z",
                            "declaration_hash": "a".repeat(64),
                            "head_commit": "not-a-commit",
                            "baseline_hash": "a".repeat(64),
                            "signals": []
                        })
                        .to_string(),
                    ),
                ),
            ];

            for (label, invalid_cache) in invalid_cache_variants {
                let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
                if let Some(invalid_cache) = invalid_cache {
                    std::fs::write(signal_path(&items_dir, &track_id), invalid_cache).unwrap();
                }
                let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

                execute_type_signals_for_layer_with_launch_observer(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &observer,
                )
                .unwrap_or_else(|error| panic!("{label} cache must be treated as a miss: {error}"));

                assert_eq!(observer.launches(), 1, "{label} cache must trigger evaluation");
                let persisted = type_signals_codec::decode_with_workspace(
                    &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                    workspace.path(),
                )
                .unwrap();
                assert_eq!(
                    persisted.cache_key(),
                    current_cache_document(workspace.path(), &items_dir, &track_id).cache_key(),
                    "{label} cache must be atomically replaced with current identities"
                );
            }
        });
    }

    #[test]
    fn test_execute_type_signals_cache_write_failure_preserves_prior_cache() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original = write_cache(
                &items_dir,
                &track_id,
                domain::CommitHash::try_new("b".repeat(40)).unwrap(),
            );
            let signal_path = signal_path(&items_dir, &track_id);
            let temporary_path = signal_path.parent().unwrap().join(format!(
                ".tmp-{}-{}",
                signal_path.file_name().unwrap().to_string_lossy(),
                std::process::id()
            ));
            std::fs::create_dir(&temporary_path).unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            let error = match execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            ) {
                Ok(status) => panic!("a failed atomic cache write returned {status:?}"),
                Err(error) => error,
            };

            assert!(
                matches!(&error, EvaluateSignalsError::CacheWrite(_)),
                "signal write failure must be reported as a cache-write error: {error:?}"
            );
            assert_eq!(
                std::fs::read(&signal_path).unwrap(),
                original.as_bytes(),
                "failed replacement must preserve the prior cache bytes"
            );
        });
    }

    #[test]
    fn test_execute_type_signals_cache_replacement_is_read_by_track_blob_reader() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .unwrap();

            assert_eq!(observer.launches(), 1, "a missing cache must trigger evaluation");
            let persisted = type_signals_codec::decode_with_workspace(
                &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                workspace.path(),
            )
            .unwrap();

            let root = workspace.path();
            crate::verify::test_support::run_git(root, &["branch", "-M", "main"]);
            crate::verify::test_support::run_git(
                root,
                &["remote", "add", "origin", root.to_str().unwrap()],
            );
            crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);
            crate::verify::test_support::run_git(
                root,
                &["add", "-f", "track/items/feature-input-track/infrastructure-type-signals.json"],
            );
            crate::verify::test_support::run_git(
                root,
                &["commit", "--quiet", "-m", "replace cache"],
            );
            crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);

            let reader =
                crate::verify::merge_gate_adapter::GitShowTrackBlobReader::new(root.to_path_buf());
            match reader.read_type_signals("main", track_id.as_ref(), "infrastructure") {
                BlobFetchResult::Found(document) => {
                    assert_eq!(document.cache_key(), persisted.cache_key());
                    assert_eq!(document.signals(), persisted.signals());
                }
                other => {
                    panic!("the replaced cache must be readable through TrackBlobReader: {other:?}")
                }
            }
        });
    }

    #[test]
    fn test_unreadable_type_signals_reader_forces_cache_miss_and_reevaluation() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let root = workspace.path();
            let signal_file = signal_path(&items_dir, &track_id);
            let relative_signal_file =
                "track/items/feature-input-track/infrastructure-type-signals.json";

            // Start with a cache that would otherwise be eligible for reuse, then
            // publish an unreadable replacement to the branch the blob reader sees.
            write_cache(&items_dir, &track_id, read_head_commit(root).unwrap());
            crate::verify::test_support::run_git(root, &["branch", "-M", "main"]);
            crate::verify::test_support::run_git(
                root,
                &["remote", "add", "origin", root.to_str().unwrap()],
            );
            crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);
            std::fs::write(&signal_file, b"not valid json").unwrap();
            crate::verify::test_support::run_git(root, &["add", "-f", relative_signal_file]);
            crate::verify::test_support::run_git(
                root,
                &["commit", "--quiet", "-m", "unreadable cache"],
            );
            crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);

            let reader =
                crate::verify::merge_gate_adapter::GitShowTrackBlobReader::new(root.to_path_buf());
            match reader.read_type_signals("main", track_id.as_ref(), "infrastructure") {
                BlobFetchResult::FetchError(message) => {
                    assert!(message.contains("decode error"), "reader diagnostic: {message}");
                }
                other => {
                    panic!("an existing malformed cache must be reported as unreadable: {other:?}")
                }
            }

            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                root,
                &binding,
                &[],
                &observer,
            )
            .unwrap();

            assert_eq!(observer.launches(), 1, "unreadable cache must force reevaluation");
            let replacement = type_signals_codec::decode_with_workspace(
                &std::fs::read_to_string(&signal_file).unwrap(),
                root,
            )
            .unwrap();
            assert_eq!(
                replacement.cache_key(),
                current_cache_document(root, &items_dir, &track_id).cache_key()
            );

            crate::verify::test_support::run_git(root, &["add", "-f", relative_signal_file]);
            crate::verify::test_support::run_git(
                root,
                &["commit", "--quiet", "-m", "replace unreadable cache"],
            );
            crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);
            assert!(matches!(
                reader.read_type_signals("main", track_id.as_ref(), "infrastructure"),
                BlobFetchResult::Found(_)
            ));
        });
    }

    #[test]
    fn test_execute_type_signals_unreadable_authority_does_not_reuse_cache() {
        with_process_environment_lock(|| {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original =
                write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
            std::fs::remove_file(
                items_dir.join(track_id.as_ref()).join("infrastructure-types.json"),
            )
            .unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            let result = execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            );

            assert!(matches!(result, Err(EvaluateSignalsError::AuthoritativeInput(_))));
            assert_eq!(observer.launches(), 0, "unreadable authority must not reuse the cache");
            assert_eq!(
                std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
                original
            );
        });
    }
}
