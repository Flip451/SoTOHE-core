//! `TypeSignalsExecutorAdapter` — infrastructure adapter for `TypeSignalsExecutorPort`.
//!
//! Wraps `crate::tddd::type_signals_evaluator::execute_type_signals_for_layer`
//! and bridges the domain [`domain::tddd::catalogue_v2::TdddLayerBinding`] type
//! (public fields) to the infra [`crate::verify::tddd_layers::TdddLayerBinding`]
//! type (private getter methods).

use std::path::Path;
#[cfg(feature = "test-helpers")]
use std::path::PathBuf;

#[cfg(feature = "test-helpers")]
use std::collections::{BTreeMap, HashMap};
#[cfg(feature = "test-helpers")]
use std::sync::{Arc, Mutex};

use domain::TrackId;
use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::TdddLayerBinding as DomainTdddLayerBinding;
#[cfg(feature = "test-helpers")]
use domain::tddd::catalogue_v2::{RustdocCratePort, RustdocCratePortError};
#[cfg(feature = "test-helpers")]
use domain::tddd::type_signals_doc::CapturedRustdocJson;
#[cfg(feature = "test-helpers")]
use domain::tddd::type_signals_doc::{
    AttestedRustdocSnapshot, CargoProfileName, ExpectedRustdocJsonPath, ImplementationFingerprint,
    ResolvedCargoTargetDirectory, RustdocExecutionIdentity, construct_attested_rustdoc_snapshot,
    construct_captured_rustdoc_json,
};
#[cfg(feature = "test-helpers")]
use domain::tddd::{ExtendedCrate, catalogue_v2::CrateName};
use usecase::git_workflow::DiagnosticText;
use usecase::type_signals::{TypeSignalsExecutionError, TypeSignalsExecutorPort};

use crate::tddd::type_signals_evaluator::RustdocContextCache;
#[cfg(feature = "test-helpers")]
use crate::tddd::type_signals_evaluator::execute_type_signals_for_layer_with_launch_observer_and_context_cache;
#[cfg(feature = "test-helpers")]
use crate::tddd::type_signals_evaluator::freshness::RustdocProvider;
use crate::tddd::type_signals_evaluator::{
    EvaluateSignalsError, execute_type_signals_for_layer_with_context_cache,
    reject_symlinked_type_signals_anchor,
};
use crate::verify::tddd_layers::TdddLayerBinding as InfraTdddLayerBinding;
#[cfg(feature = "test-helpers")]
use usecase::catalogue_impl_signals::ports::{
    EvaluationStartCaptureError, EvaluationStartCapturePort,
};

#[path = "type_signals_executor_adapter/binding_conversion.rs"]
mod binding_conversion;

#[cfg(feature = "test-helpers")]
#[derive(Clone)]
pub struct RustdocLaunchObserver {
    json_paths: BTreeMap<String, PathBuf>,
    fallback_json_path: Option<PathBuf>,
    launches: Arc<Mutex<BTreeMap<String, usize>>>,
    feature_selections: Arc<Mutex<BTreeMap<String, Vec<Vec<String>>>>>,
    encoded_crates: Arc<Mutex<Vec<ExtendedCrate>>>,
    resolution_paths: Arc<Mutex<Vec<HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>>>>,
    before_export: Option<Arc<dyn Fn() + Send + Sync>>,
    after_export: Option<Arc<dyn Fn() + Send + Sync>>,
}

#[cfg(feature = "test-helpers")]
impl std::fmt::Debug for RustdocLaunchObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RustdocLaunchObserver")
            .field("json_paths", &self.json_paths)
            .field("launches", &self.launches())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "test-helpers")]
impl RustdocLaunchObserver {
    #[must_use]
    pub fn using_json_path(json_path: PathBuf) -> Self {
        Self {
            json_paths: BTreeMap::new(),
            fallback_json_path: Some(json_path),
            launches: Arc::new(Mutex::new(BTreeMap::new())),
            feature_selections: Arc::new(Mutex::new(BTreeMap::new())),
            encoded_crates: Arc::new(Mutex::new(Vec::new())),
            resolution_paths: Arc::new(Mutex::new(Vec::new())),
            before_export: None,
            after_export: None,
        }
    }

    /// Creates an observer that returns the matching rustdoc JSON file for
    /// each requested crate. This lets composition tests assert layer-scoped
    /// extraction without changing production rustdoc wiring.
    #[must_use]
    pub fn using_json_paths(json_paths: BTreeMap<String, PathBuf>) -> Self {
        Self {
            json_paths,
            fallback_json_path: None,
            launches: Arc::new(Mutex::new(BTreeMap::new())),
            feature_selections: Arc::new(Mutex::new(BTreeMap::new())),
            encoded_crates: Arc::new(Mutex::new(Vec::new())),
            resolution_paths: Arc::new(Mutex::new(Vec::new())),
            before_export: None,
            after_export: None,
        }
    }

    /// Creates an observer that runs `before_export` immediately before the
    /// simulated rustdoc export. This permits a test to model an input that was
    /// unavailable for the reuse decision but is restored before persistence.
    #[must_use]
    pub fn using_json_path_with_before_export(
        json_path: PathBuf,
        before_export: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            json_paths: BTreeMap::new(),
            fallback_json_path: Some(json_path),
            launches: Arc::new(Mutex::new(BTreeMap::new())),
            feature_selections: Arc::new(Mutex::new(BTreeMap::new())),
            encoded_crates: Arc::new(Mutex::new(Vec::new())),
            resolution_paths: Arc::new(Mutex::new(Vec::new())),
            before_export: Some(before_export),
            after_export: None,
        }
    }

    /// Creates an observer with hooks around every simulated rustdoc export.
    /// The hooks let evaluator tests model a source generation that is changed
    /// between layer exports and restored before the enclosing evaluation ends.
    #[must_use]
    pub fn using_json_paths_with_before_and_after_export(
        json_paths: BTreeMap<String, PathBuf>,
        before_export: Arc<dyn Fn() + Send + Sync>,
        after_export: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            json_paths,
            fallback_json_path: None,
            launches: Arc::new(Mutex::new(BTreeMap::new())),
            feature_selections: Arc::new(Mutex::new(BTreeMap::new())),
            encoded_crates: Arc::new(Mutex::new(Vec::new())),
            resolution_paths: Arc::new(Mutex::new(Vec::new())),
            before_export: Some(before_export),
            after_export: Some(after_export),
        }
    }

    #[must_use]
    pub fn launches(&self) -> usize {
        self.launches.lock().map_or(0, |launches| launches.values().sum())
    }

    /// Returns the simulated rustdoc launch count for one crate.
    #[must_use]
    pub fn launches_for(&self, crate_name: &str) -> usize {
        self.launches.lock().map_or(0, |launches| launches.get(crate_name).copied().unwrap_or(0))
    }

    /// Returns every feature selection supplied to simulated rustdoc for one crate.
    #[must_use]
    pub fn feature_selections_for(&self, crate_name: &str) -> Vec<Vec<String>> {
        self.feature_selections.lock().map_or_else(
            |_| Vec::new(),
            |selections| selections.get(crate_name).cloned().unwrap_or_default(),
        )
    }

    /// Returns the encoded TypeGraph A values observed by the evaluator.
    #[must_use]
    pub fn encoded_crates(&self) -> Vec<ExtendedCrate> {
        self.encoded_crates.lock().map_or_else(|_| Vec::new(), |crates| crates.clone())
    }

    /// Returns the resolution-path maps observed by the evaluator.
    #[must_use]
    pub fn resolution_path_snapshots(
        &self,
    ) -> Vec<HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>> {
        self.resolution_paths.lock().map_or_else(|_| Vec::new(), |paths| paths.clone())
    }

    pub(crate) fn record_encoded_crate(&self, encoded: &ExtendedCrate) {
        if let Ok(mut crates) = self.encoded_crates.lock() {
            crates.push(encoded.clone());
        }
    }

    pub(crate) fn record_resolution_paths(
        &self,
        paths: &HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
    ) {
        if let Ok(mut snapshots) = self.resolution_paths.lock() {
            snapshots.push(paths.clone());
        }
    }

    fn json_path_for(&self, crate_name: &CrateName) -> Result<PathBuf, RustdocCratePortError> {
        self.json_paths
            .get(crate_name.as_str())
            .cloned()
            .or_else(|| self.fallback_json_path.clone())
            .ok_or_else(|| RustdocCratePortError::CaptureFailed {
                crate_name: crate_name.clone(),
                reason: domain::FreeText::new("test rustdoc path was not configured"),
            })
    }

    fn workspace_root_for_fingerprint(
        &self,
        crate_name: &CrateName,
    ) -> Result<PathBuf, RustdocCratePortError> {
        self.workspace_root_path().map_err(|reason| RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: domain::FreeText::new(reason),
        })
    }

    fn workspace_root_path(&self) -> Result<PathBuf, String> {
        let path = self
            .json_paths
            .values()
            .next()
            .or(self.fallback_json_path.as_ref())
            .ok_or_else(|| "test rustdoc path was not configured".to_owned())?;
        let mut candidate = path.as_path();
        while let Some(parent) = candidate.parent() {
            if candidate.file_name() == Some(std::ffi::OsStr::new(".sotp-rustdoc")) {
                return parent.canonicalize().map_err(|error| {
                    format!("cannot canonicalize test rustdoc workspace root: {error}")
                });
            }
            candidate = parent;
        }
        Err("test rustdoc path is not below a .sotp-rustdoc selection directory".to_owned())
    }

    fn workspace_root_for_evaluation_start(&self) -> Result<PathBuf, EvaluationStartCaptureError> {
        self.workspace_root_path().map_err(|reason| {
            EvaluationStartCaptureError::AuthoritativeInput {
                reason: domain::FreeText::new(reason),
            }
        })
    }

    fn run_before_export(&self) {
        if let Some(before_export) = &self.before_export {
            before_export();
        }
    }

    fn run_after_export(&self) {
        if let Some(after_export) = &self.after_export {
            after_export();
        }
    }

    fn capture_current_inner(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        if let Ok(mut launches) = self.launches.lock() {
            *launches.entry(crate_name.as_str().to_owned()).or_default() += 1;
        }
        if let Ok(mut selections) = self.feature_selections.lock() {
            selections
                .entry(crate_name.as_str().to_owned())
                .or_default()
                .push(features.iter().map(|feature| feature.as_str().to_owned()).collect());
        }
        let path = self.json_path_for(crate_name)?;
        let bytes = std::fs::read(&path).map_err(|error| RustdocCratePortError::Io {
            path: path.clone(),
            reason: domain::FreeText::new(error.to_string()),
        })?;
        let identity = observed_execution_identity(&path, crate_name, features)?;
        construct_attested_rustdoc_snapshot(
            evaluation_start.clone(),
            identity,
            &bytes,
            decode_observed_rustdoc,
        )
    }

    fn capture_current_attested_with_implementation_fingerprint(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        let workspace_root = self.workspace_root_for_fingerprint(crate_name)?;
        self.run_before_export();
        let before =
            crate::tddd::type_signals_evaluator::freshness::rustdoc_implementation_fingerprint(
                &workspace_root,
            )
            .map_err(|error| RustdocCratePortError::AuthoritativeInput {
                crate_name: crate_name.clone(),
                reason: domain::FreeText::new(format!(
                    "cannot fingerprint test rustdoc workspace before export: {error}"
                )),
            })?;
        let result = self.capture_current_inner(crate_name, features, evaluation_start);
        self.run_after_export();
        let after =
            crate::tddd::type_signals_evaluator::freshness::rustdoc_implementation_fingerprint(
                &workspace_root,
            )
            .map_err(|error| RustdocCratePortError::AuthoritativeInput {
                crate_name: crate_name.clone(),
                reason: domain::FreeText::new(format!(
                    "cannot fingerprint test rustdoc workspace after export: {error}"
                )),
            })?;
        let attested = result?;
        if before != *evaluation_start {
            return Err(RustdocCratePortError::AuthoritativeInput {
                crate_name: crate_name.clone(),
                reason: domain::FreeText::new(
                    "workspace implementation changed during type-signal evaluation: fingerprint before rustdoc export disagrees with evaluation-start snapshot",
                ),
            });
        }
        if after != *evaluation_start {
            return Err(RustdocCratePortError::AuthoritativeInput {
                crate_name: crate_name.clone(),
                reason: domain::FreeText::new(
                    "workspace implementation changed during type-signal evaluation: fingerprint after rustdoc export disagrees with evaluation-start snapshot",
                ),
            });
        }
        Ok(attested)
    }
}

#[cfg(feature = "test-helpers")]
impl RustdocCratePort for RustdocLaunchObserver {
    fn load_from_path(&self, path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        let bytes = std::fs::read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RustdocCratePortError::NotFound { path: path.to_path_buf() }
            } else {
                RustdocCratePortError::Io {
                    path: path.to_path_buf(),
                    reason: domain::FreeText::new(error.to_string()),
                }
            }
        })?;
        construct_captured_rustdoc_json(&bytes, decode_observed_rustdoc)
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.capture_current_attested_with_implementation_fingerprint(
            crate_name,
            features,
            evaluation_start,
        )
    }
}

#[cfg(feature = "test-helpers")]
impl EvaluationStartCapturePort for RustdocLaunchObserver {
    fn capture_evaluation_start(
        &self,
    ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError> {
        let workspace_root = self.workspace_root_for_evaluation_start()?;
        crate::tddd::type_signals_evaluator::freshness::rustdoc_implementation_fingerprint(
            &workspace_root,
        )
        .map_err(|error| EvaluationStartCaptureError::AuthoritativeInput {
            reason: domain::FreeText::new(format!(
                "cannot fingerprint test rustdoc workspace: {error}"
            )),
        })
    }
}

#[cfg(feature = "test-helpers")]
impl RustdocProvider for RustdocLaunchObserver {
    fn capture_current_with_implementation_fingerprint(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.capture_current_attested_with_implementation_fingerprint(
            crate_name,
            features,
            evaluation_start,
        )
    }

    fn execution_identity(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<RustdocExecutionIdentity, RustdocCratePortError> {
        let path = self.json_path_for(crate_name)?;
        observed_execution_identity(&path, crate_name, features)
    }
}

#[cfg(feature = "test-helpers")]
fn decode_observed_rustdoc(bytes: &[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError> {
    let text = std::str::from_utf8(bytes).map_err(|error| RustdocCratePortError::ParseFailed {
        crate_name: fixed_test_crate_name("test_observer"),
        reason: domain::FreeText::new(error.to_string()),
    })?;
    crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec::from_json(text).map_err(|error| {
        RustdocCratePortError::ParseFailed {
            crate_name: fixed_test_crate_name("test_observer"),
            reason: domain::FreeText::new(error.to_string()),
        }
    })
}

#[cfg(feature = "test-helpers")]
fn fixed_test_crate_name(value: &str) -> CrateName {
    CrateName::new(value).unwrap_or_else(|_| std::process::abort())
}

#[cfg(feature = "test-helpers")]
fn observed_execution_identity(
    path: &Path,
    crate_name: &CrateName,
    features: &[CargoFeatureName],
) -> Result<RustdocExecutionIdentity, RustdocCratePortError> {
    let path = std::fs::canonicalize(path).map_err(|error| RustdocCratePortError::Io {
        path: path.to_path_buf(),
        reason: domain::FreeText::new(error.to_string()),
    })?;
    let target_directory = path
        .parent()
        .and_then(|parent| {
            if parent.file_name() == Some(std::ffi::OsStr::new("doc")) {
                parent.parent()
            } else {
                Some(parent)
            }
        })
        .ok_or_else(|| RustdocCratePortError::Io {
            path: path.clone(),
            reason: domain::FreeText::new("test rustdoc path has no target directory"),
        })?;
    let target_directory = ResolvedCargoTargetDirectory::try_new(target_directory.to_path_buf())
        .map_err(|error| RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: domain::FreeText::new(error.to_string()),
        })?;
    let expected = ExpectedRustdocJsonPath::try_new(path, &target_directory).map_err(|error| {
        RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: domain::FreeText::new(error.to_string()),
        }
    })?;
    let profile = CargoProfileName::try_new("dev".to_owned()).map_err(|error| {
        RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: domain::FreeText::new(error.to_string()),
        }
    })?;
    RustdocExecutionIdentity::new(
        target_directory,
        crate_name.clone(),
        features.to_vec(),
        profile,
        expected,
    )
    .map_err(|error| RustdocCratePortError::CaptureFailed {
        crate_name: crate_name.clone(),
        reason: domain::FreeText::new(error.to_string()),
    })
}

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// Stateless adapter implementing [`TypeSignalsExecutorPort`].
///
/// Converts the domain [`DomainTdddLayerBinding`] (public fields) to the infra
/// [`InfraTdddLayerBinding`] (private getters + `signal_file()` method) and
/// delegates to the crate-private `execute_type_signals_for_layer` evaluator.
///
/// Every catalogue input is evaluated conservatively. Missing or unverifiable
/// inputs fail closed; multi-target catalogues return an error because they are
/// not yet supported.
#[derive(Debug, Default)]
pub struct TypeSignalsExecutorAdapter {
    context_cache: RustdocContextCache,
    #[cfg(feature = "test-helpers")]
    launch_observer: Option<RustdocLaunchObserver>,
}

impl TypeSignalsExecutorAdapter {
    /// Creates a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an adapter wired to a test-only rustdoc launch observer.
    #[cfg(feature = "test-helpers")]
    #[must_use]
    pub fn with_rustdoc_launch_observer(launch_observer: RustdocLaunchObserver) -> Self {
        Self {
            context_cache: RustdocContextCache::default(),
            launch_observer: Some(launch_observer),
        }
    }

    /// Convert a domain binding to an infra binding for use with the evaluator.
    ///
    /// The infra binding stores private fields only populated via
    /// `parse_tddd_layers`; `TypeSignalsExecutorAdapter` constructs a
    /// synthetic binding from the domain fields so the evaluator can be
    /// reused without changing its signature.
    ///
    /// # Errors
    ///
    /// Returns `TypeSignalsExecutionError` if the synthetic rules JSON fails to
    /// parse (should not happen in practice — indicates a logic error).
    fn to_infra_binding(
        b: &DomainTdddLayerBinding,
    ) -> Result<InfraTdddLayerBinding, TypeSignalsExecutionError> {
        binding_conversion::to_infra_binding(b)
    }

    fn map_evaluation_error(error: EvaluateSignalsError) -> TypeSignalsExecutionError {
        match error {
            EvaluateSignalsError::AuthoritativeInput(message) => {
                TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(message.as_str()))
            }
            EvaluateSignalsError::Evaluation(message) => {
                TypeSignalsExecutionError::Evaluation(DiagnosticText::new(message.as_str()))
            }
            EvaluateSignalsError::CacheWrite(message) => {
                TypeSignalsExecutionError::CacheWrite(DiagnosticText::new(message.as_str()))
            }
        }
    }
}

impl TypeSignalsExecutorPort for TypeSignalsExecutorAdapter {
    /// Evaluates type signals for one layer binding.
    ///
    /// Missing catalogue inputs are errors rather than reuse skips. A
    /// multi-target catalogue returns an error (not yet supported, CN-02).
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsExecutionError`] on any evaluation failure.
    fn evaluate_layer(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
        workspace_root: &Path,
        binding: &DomainTdddLayerBinding,
        features: &[CargoFeatureName],
    ) -> Result<(), TypeSignalsExecutionError> {
        // Security: validate items_dir first, before any binding-dependent
        // early-exit paths.  This ensures a symlinked items_dir is rejected
        // regardless of catalogue presence or binding contents.  Mirrors the
        // identical check in `execute_type_signals_for_layer`.
        reject_symlinked_type_signals_anchor(items_dir, "items_dir").map_err(|reason| {
            TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(reason))
        })?;

        // Perform input validation and binding conversion before any early-exit
        // paths so that malformed requests fail closed regardless of catalogue
        // presence.
        //
        // Validate track_id via the domain newtype before joining onto items_dir.
        // `Path::join` resolves `..`, `/`, and multi-segment paths at the OS level.
        // Using `TrackId::try_new` enforces the slug rules (single-segment, no `..`,
        // no path separators) so that the absent-catalogue skip cannot be bypassed
        // via path-traversal track IDs (e.g. `../bad`).
        let valid_track_id = track_id;

        // Reject empty targets: a binding with no targets is always malformed.
        if binding.targets.is_empty() {
            return Err(TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(
                format!(
                    "layer '{}': schema_export.targets is empty — at least one target is required",
                    binding.layer_id,
                ),
            )));
        }

        // Convert the domain binding to an infra binding before any early-exit
        // paths so that catalogue_file and layer_id are validated via
        // parse_tddd_layers (is_safe_path_component) regardless of the targets
        // count.  Without this call in the multi-target path, a malformed
        // catalogue_file (e.g. containing path traversal characters) would be
        // passed directly to track_dir.join() without validation.
        let infra_binding = Self::to_infra_binding(binding)?;

        // Verify that the infra binding derives the same baseline_file as the
        // domain binding.  The infra binding computes baseline_file from
        // catalogue_file (stem + "-baseline.json"); if the caller supplied a
        // non-standard baseline path, flag it rather than silently reading the
        // wrong file.
        let derived_baseline = infra_binding.baseline_file();
        if derived_baseline != binding.baseline_file {
            return Err(TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(
                format!(
                    "layer '{}': domain baseline_file '{}' differs from the infra-derived \
                 baseline_file '{}' (derived from catalogue_file '{}'); supply a standard \
                 baseline path or adjust catalogue_file",
                    binding.layer_id,
                    binding.baseline_file,
                    derived_baseline,
                    binding.catalogue_file,
                ),
            )));
        }

        // Multi-target bindings are not yet supported by the strict evaluator.
        if binding.targets.len() > 1 {
            return Err(TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(
                format!(
                    "layer '{}' has {} schema_export.targets — multi-target not yet supported",
                    binding.layer_id,
                    binding.targets.len()
                ),
            )));
        }

        #[cfg(feature = "test-helpers")]
        let execution = match &self.launch_observer {
            Some(observer) => {
                execute_type_signals_for_layer_with_launch_observer_and_context_cache(
                    items_dir,
                    valid_track_id,
                    workspace_root,
                    &infra_binding,
                    features,
                    observer,
                    &self.context_cache,
                )
            }
            None => execute_type_signals_for_layer_with_context_cache(
                items_dir,
                valid_track_id,
                workspace_root,
                &infra_binding,
                features,
                &self.context_cache,
            ),
        };
        #[cfg(not(feature = "test-helpers"))]
        let execution = execute_type_signals_for_layer_with_context_cache(
            items_dir,
            valid_track_id,
            workspace_root,
            &infra_binding,
            features,
            &self.context_cache,
        );

        execution.map(|_exit| ()).map_err(Self::map_evaluation_error)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn domain_binding(layer_id: &str) -> DomainTdddLayerBinding {
        DomainTdddLayerBinding {
            layer_id: layer_id.to_owned(),
            catalogue_file: format!("{layer_id}-types.json"),
            baseline_file: format!("{layer_id}-types-baseline.json"),
            targets: vec![layer_id.to_owned()],
        }
    }

    fn track_id() -> TrackId {
        TrackId::try_new("my-track").unwrap()
    }

    #[cfg(feature = "test-helpers")]
    fn minimal_rustdoc_json() -> String {
        format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        )
    }

    #[cfg(feature = "test-helpers")]
    fn rustdoc_json_with_paths(
        root_name: &str,
        entries: &[(&[&str], rustdoc_types::ItemKind)],
    ) -> String {
        let root_id = rustdoc_types::Id(0);
        let paths = entries
            .iter()
            .enumerate()
            .map(|(index, (path, kind))| {
                (
                    rustdoc_types::Id(index as u32 + 1),
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
                        kind: *kind,
                    },
                )
            })
            .collect();
        let index = std::collections::HashMap::from([(
            root_id,
            rustdoc_types::Item {
                id: root_id,
                crate_id: 0,
                name: Some(root_name.to_owned()),
                span: None,
                visibility: rustdoc_types::Visibility::Public,
                docs: None,
                links: std::collections::HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: rustdoc_types::ItemEnum::Module(rustdoc_types::Module {
                    is_crate: true,
                    items: vec![],
                    is_stripped: false,
                }),
            },
        )]);
        serde_json::to_string(&rustdoc_types::Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: std::collections::HashMap::new(),
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
            format_version: rustdoc_types::FORMAT_VERSION,
        })
        .unwrap()
    }

    #[cfg(feature = "test-helpers")]
    fn rustdoc_json_with_external_path(path: &[&str]) -> String {
        let mut crate_: rustdoc_types::Crate = serde_json::from_str(&rustdoc_json_with_paths(
            "infrastructure",
            &[(path, rustdoc_types::ItemKind::Struct)],
        ))
        .unwrap();
        let item_id = rustdoc_types::Id(1);
        let external_crate_id = rustdoc_types::Id(7);
        crate_.paths.get_mut(&item_id).unwrap().crate_id = external_crate_id.0;
        crate_.external_crates.insert(
            external_crate_id.0,
            rustdoc_types::ExternalCrate {
                name: "domain".to_owned(),
                html_root_url: None,
                path: std::path::PathBuf::new(),
            },
        );
        serde_json::to_string(&crate_).unwrap()
    }

    #[cfg(feature = "test-helpers")]
    fn setup_feature_aware_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join("my-track");
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
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nsemantic-dup = []\n",
        )
        .unwrap();
        std::fs::write(root.join("libs/infrastructure/src/lib.rs"), "pub struct Fixture;\n")
            .unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            "{\n  \"schema_version\": 5,\n  \"crate_name\": \"infrastructure\",\n  \"layer\": \"infrastructure\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
        )
        .unwrap();
        let exclusive = root.join(".sotp-rustdoc").join("fixture");
        std::fs::create_dir_all(&exclusive).unwrap();
        let rustdoc_path = exclusive.join("infrastructure-rustdoc.json");
        let rustdoc_json = minimal_rustdoc_json();
        std::fs::write(&rustdoc_path, &rustdoc_json).unwrap();
        std::fs::write(track_dir.join("infrastructure-types-baseline.json"), rustdoc_json).unwrap();
        std::fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .unwrap();
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
        crate::verify::test_support::git_init(root);
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);

        (workspace, rustdoc_path)
    }

    #[cfg(feature = "test-helpers")]
    fn nightly_toolchain_available() -> bool {
        std::process::Command::new("rustup")
            .args(["run", "nightly", "rustc", "-Vv"])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[test]
    fn test_to_infra_binding_preserves_layer_id() {
        let domain = domain_binding("domain");
        let infra = TypeSignalsExecutorAdapter::to_infra_binding(&domain).unwrap();
        assert_eq!(infra.layer_id(), "domain");
        assert_eq!(infra.catalogue_file(), "domain-types.json");
        assert_eq!(infra.baseline_file(), "domain-types-baseline.json");
        assert_eq!(infra.targets(), &["domain"]);
    }

    #[test]
    fn test_adapter_implements_type_signals_executor_port() {
        fn assert_port<T: TypeSignalsExecutorPort>() {}
        assert_port::<TypeSignalsExecutorAdapter>();
    }

    #[cfg(feature = "test-helpers")]
    #[test]
    fn test_evaluate_layer_loads_architecture_rules_catalogues_and_omits_absent_layer_files() {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, rustdoc_path) = setup_feature_aware_workspace();
            let items_dir = workspace.path().join("track/items");
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let adapter = TypeSignalsExecutorAdapter::with_rustdoc_launch_observer(observer);

            adapter
                .evaluate_layer(
                    &items_dir,
                    &track_id(),
                    workspace.path(),
                    &domain_binding("infrastructure"),
                    &[],
                )
                .expect("an enabled layer without a catalogue file contributes no declarations");
        });
    }

    #[cfg(feature = "test-helpers")]
    #[test]
    fn test_evaluate_layer_canonicalizes_declaring_bin_target_alias_and_prefers_rustdoc_item() {
        use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CatalogueEntryKey, CrateName, FieldDecl, FieldName, ModulePath,
            TypeRef,
        };

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            let root = workspace.path();
            let track_id = TrackId::try_new("cross-layer-type-signals").unwrap();
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
                "track/items/cross-layer-type-signals/infrastructure-type-signals.json\n",
            )
            .unwrap();
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
                            "schema_export": { "method": "rustdoc", "targets": ["domain_bin"] }
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

            let mut target = CatalogueDocument::new(
                5,
                CrateName::new("infrastructure").unwrap(),
                domain::tddd::LayerId::try_new("infrastructure").unwrap(),
            );
            target.insert_type(
                CatalogueEntryKey::try_new("Handler".to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain {
                            fields: vec![FieldDecl::new(
                                FieldName::new("name").unwrap(),
                                TypeRef::new("domain::model::Name").unwrap(),
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
                domain::tddd::LayerId::try_new("domain").unwrap(),
            );
            declaring.insert_type(
                CatalogueEntryKey::try_new("Name".to_owned()).unwrap(),
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

            std::fs::write(
                track_dir.join("infrastructure-types.json"),
                CatalogueDocumentCodec::encode(&target).unwrap(),
            )
            .unwrap();
            std::fs::write(
                track_dir.join("domain-types.json"),
                CatalogueDocumentCodec::encode(&declaring).unwrap(),
            )
            .unwrap();
            let baseline = minimal_rustdoc_json();
            std::fs::write(track_dir.join("infrastructure-types-baseline.json"), &baseline)
                .unwrap();
            std::fs::write(track_dir.join("domain-types-baseline.json"), &baseline).unwrap();

            let exclusive = root.join(".sotp-rustdoc").join("fixture");
            std::fs::create_dir_all(&exclusive).unwrap();
            let domain_current_path = exclusive.join("domain-current.json");
            let infrastructure_current_path = exclusive.join("infrastructure-current.json");
            std::fs::write(
                &domain_current_path,
                rustdoc_json_with_paths(
                    "sotp",
                    &[(&["sotp", "model", "Name"], rustdoc_types::ItemKind::Struct)],
                ),
            )
            .unwrap();
            std::fs::write(
                &infrastructure_current_path,
                rustdoc_json_with_external_path(&["domain", "model", "Name"]),
            )
            .unwrap();
            crate::verify::test_support::git_init(root);
            crate::verify::test_support::run_git(root, &["add", "."]);
            crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);

            let observer =
                RustdocLaunchObserver::using_json_paths(std::collections::BTreeMap::from([
                    ("domain_bin".to_owned(), domain_current_path),
                    ("infrastructure".to_owned(), infrastructure_current_path),
                ]));
            let observer_snapshot = observer.clone();
            TypeSignalsExecutorAdapter::with_rustdoc_launch_observer(observer)
                .evaluate_layer(&items_dir, &track_id, root, &domain_binding("infrastructure"), &[])
                .expect("evaluate_layer must use the declaring layer's current rustdoc");

            assert_eq!(observer_snapshot.launches_for("infrastructure"), 1);
            assert_eq!(observer_snapshot.launches_for("domain_bin"), 1);
            let signals =
                std::fs::read_to_string(track_dir.join("infrastructure-type-signals.json"))
                    .unwrap();
            let signals = crate::tddd::type_signals_codec::decode(&signals).unwrap();
            assert!(
                signals.signals().iter().any(|signal| signal.type_name() == "Handler"),
                "the target catalogue must be evaluated after external add placement succeeds"
            );

            let resolution_snapshots = observer_snapshot.resolution_path_snapshots();
            assert_eq!(resolution_snapshots.len(), 1);
            let resolved_paths = resolution_snapshots
                .first()
                .expect("the evaluator must expose one resolution-path snapshot");
            let (resolved_id, resolved_name) = resolved_paths
                .iter()
                .find(|(_, summary)| summary.path == ["domain", "model", "Name"])
                .expect("the referencing rustdoc identity must remain in the resolution set");
            assert_eq!(*resolved_id, rustdoc_types::Id(1));
            assert_eq!(resolved_name.crate_id, 7);
            assert!(
                !resolved_paths.values().any(|summary| {
                    summary.path == ["domain", "model", "Name"] && summary.crate_id == u32::MAX - 1
                }),
                "the rustdoc item must win instead of being synthesized a second time"
            );

            let encoded_crates = observer_snapshot.encoded_crates();
            assert_eq!(encoded_crates.len(), 1);
            let encoded = encoded_crates.first().expect("the evaluator must expose TypeGraph A");
            let encoded_name = encoded
                .krate()
                .paths
                .values()
                .find(|summary| summary.path == ["domain", "model", "Name"])
                .expect("the encoded Handler reference must retain the external identity");
            assert_ne!(encoded_name.crate_id, 0);
            assert_eq!(
                encoded
                    .krate()
                    .external_crates
                    .get(&encoded_name.crate_id)
                    .expect("the encoded identity must name its external crate")
                    .name,
                "domain"
            );
        });
    }

    #[test]
    fn test_map_evaluation_error_each_stage_preserves_execution_error_category() {
        let cases = [
            (
                EvaluateSignalsError::AuthoritativeInput(domain::FreeText::new(
                    "missing Cargo.lock",
                )),
                "AuthoritativeInput",
            ),
            (
                EvaluateSignalsError::Evaluation(domain::FreeText::new("cannot create timestamp")),
                "Evaluation",
            ),
            (
                EvaluateSignalsError::CacheWrite(domain::FreeText::new(
                    "cannot write type signals",
                )),
                "CacheWrite",
            ),
        ];

        for (evaluator_error, expected_stage) in cases {
            let execution_error = TypeSignalsExecutorAdapter::map_evaluation_error(evaluator_error);
            assert!(
                matches!(
                    (expected_stage, &execution_error),
                    ("AuthoritativeInput", TypeSignalsExecutionError::AuthoritativeInput(_))
                        | ("Evaluation", TypeSignalsExecutionError::Evaluation(_))
                        | ("CacheWrite", TypeSignalsExecutionError::CacheWrite(_))
                ),
                "expected {expected_stage} error stage, got {execution_error:?}"
            );
        }
    }

    #[cfg(feature = "test-helpers")]
    #[test]
    fn test_evaluate_layer_with_declared_features_forwards_them_to_rustdoc() {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            if !nightly_toolchain_available() {
                eprintln!(
                    "skipping feature-forwarding adapter test: nightly toolchain is unavailable"
                );
                return;
            }
            let (workspace, rustdoc_path) = setup_feature_aware_workspace();
            let items_dir = workspace.path().join("track/items");
            let declared_feature = CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap();
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
            let adapter =
                TypeSignalsExecutorAdapter::with_rustdoc_launch_observer(observer.clone());

            adapter
                .evaluate_layer(
                    &items_dir,
                    &track_id(),
                    workspace.path(),
                    &domain_binding("infrastructure"),
                    std::slice::from_ref(&declared_feature),
                )
                .unwrap();

            assert_eq!(observer.launches_for("infrastructure"), 1);
            assert_eq!(
                observer.feature_selections_for("infrastructure"),
                vec![vec!["semantic-dup".to_owned()]],
                "the executor adapter must pass the declaration-derived feature selection to rustdoc"
            );
        });
    }

    #[test]
    fn test_evaluate_layer_absent_catalogue_returns_error() {
        // Missing catalogue leaves a freshness input unknown and must fail closed.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // No catalogue file written

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(
            &items_dir,
            &track_id(),
            dir.path(),
            &domain_binding("domain"),
            &[],
        );
        assert!(result.is_err(), "absent catalogue must fail closed");
    }

    #[test]
    fn test_evaluate_layer_rejects_items_dir_outside_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let items_dir = external.path().join("track/items");
        std::fs::create_dir_all(items_dir.join("my-track")).unwrap();

        let result = TypeSignalsExecutorAdapter::new().evaluate_layer(
            &items_dir,
            &track_id(),
            workspace.path(),
            &domain_binding("domain"),
            &[],
        );

        assert!(
            matches!(&result, Err(error) if error.to_string().contains("resolves outside workspace_root")),
            "items_dir outside the workspace must be rejected before reads: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_evaluate_layer_rejects_symlinked_catalogue_before_read() {
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let outside_catalogue = workspace.path().join("outside-types.json");
        std::fs::write(&outside_catalogue, "{}").unwrap();
        std::os::unix::fs::symlink(&outside_catalogue, track_dir.join("domain-types.json"))
            .unwrap();

        let result = TypeSignalsExecutorAdapter::new().evaluate_layer(
            &items_dir,
            &track_id(),
            workspace.path(),
            &domain_binding("domain"),
            &[],
        );

        assert!(
            matches!(&result, Err(error) if error.to_string().contains("symlink guard rejected catalogue")),
            "a symlinked catalogue must be rejected before read: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_layer_multi_target_absent_catalogue_returns_error() {
        // Multi-target evaluation is unsupported even if the catalogue is absent.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // Catalogue file intentionally NOT created.

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result =
            adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &multi_binding, &[]);
        assert!(result.is_err(), "multi-target + absent catalogue must fail closed");
    }

    #[test]
    fn test_evaluate_layer_multi_target_present_catalogue_returns_error() {
        // Multi-target + present catalogue => fail-closed (CN-02: no fail-open).
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write a (dummy) catalogue file so the presence check detects it.
        std::fs::write(track_dir.join("my-layer-types.json"), b"{}").unwrap();

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result =
            adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &multi_binding, &[]);
        assert!(
            result.is_err(),
            "multi-target + present catalogue must return Err (fail-closed, CN-02)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("multi-target"), "error must mention multi-target, got: {msg}");
    }

    #[test]
    fn test_evaluate_layer_missing_track_dir_returns_error() {
        // ADR 2026-06-01-0406 D1: a missing track dir is a structural anomaly and
        // must fail-closed. Only an EXISTING track dir with an absent catalogue file
        // is a sanctioned skip scenario.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        // Create items_dir but do NOT create the track subdirectory.
        std::fs::create_dir_all(&items_dir).unwrap();
        // track_dir is intentionally absent.

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(
            &items_dir,
            &track_id(),
            dir.path(),
            &domain_binding("domain"),
            &[],
        );
        assert!(
            result.is_err(),
            "missing track dir must return Err (fail-closed per ADR 2026-06-01-0406 D1)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cannot read catalogue"),
            "error must identify the missing input, got: {msg}"
        );
    }

    #[test]
    fn test_evaluate_layer_multi_target_missing_track_dir_returns_error() {
        // ADR 2026-06-01-0406 D1: same fail-closed requirement for multi-target bindings.
        // A missing track dir must not be silently treated as "absent catalogue".
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        // Create items_dir but do NOT create the track subdirectory.
        std::fs::create_dir_all(&items_dir).unwrap();
        // track_dir is intentionally absent.

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result =
            adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &multi_binding, &[]);
        assert!(
            result.is_err(),
            "multi-target + missing track dir must return Err (fail-closed per ADR 2026-06-01-0406 D1)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("multi-target"),
            "error must preserve the unsupported-target cause, got: {msg}"
        );
    }
}
