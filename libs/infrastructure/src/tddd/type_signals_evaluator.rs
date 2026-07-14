//! `execute_type_signals_for_layer` — wires pre-commit type-signal recomputation
//! to `SignalEvaluatorV2` (three-way diff evaluator: catalogue A, baseline B,
//! live rustdoc C). Output uses the existing schema_version 1 format so the
//! merge-gate reader (`type_signals_codec`) and the pre-commit classifier in
//! `make.rs` continue to work unchanged. `EvaluateSignalsError` is the public
//! error type.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
#[cfg(feature = "test-helpers")]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[path = "type_signals_evaluator/signal_tags.rs"]
pub(crate) mod signal_tags;

#[path = "type_signals_evaluator/build_inputs.rs"]
mod build_inputs;
#[path = "type_signals_evaluator/freshness.rs"]
mod freshness;
#[path = "type_signals_evaluator/inputs.rs"]
mod inputs;
#[path = "type_signals_evaluator/signal_builder.rs"]
mod signal_builder;

use domain::schema::SchemaExportError;
use domain::tddd::type_signals_doc::{
    BaselineHash, ImplementationInputHash, LiveRustdocSnapshotHash, TypeSignalsCurrentInputs,
    TypeSignalsDocument, TypeSignalsFreshness, TypeSignalsReuseDecision,
};
use domain::{ConfidenceSignal, Timestamp, TrackId, TypeSignal};
use freshness::{
    RustdocJsonPathProvider, reuse_decision_for_recorded_document, snapshot_status_and_content,
};
use inputs::{
    digest_identity, evaluator_contract_hash, hash_workspace_inputs, read_utf8_file_limited,
    rustdoc_extraction_contract_hash, verify_evaluation_inputs_unchanged,
};
use signal_builder::build_type_signals_from_report;
use signal_tags::{contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag};

#[cfg(feature = "test-helpers")]
pub use freshness::RustdocLaunchObserver;

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
use crate::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use crate::tddd::type_signals_codec;
use crate::tddd::{
    CatalogueToExtendedCratePort, SignalEvaluatorPort, ThreeWaySignal, ThreeWaySignalKind,
};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

const MAX_TYPE_SIGNALS_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUSTDOC_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

/// Error type for the signal evaluator.
///
/// Wraps any failure that occurs during the three-way evaluation pipeline for
/// a single layer: catalogue load, baseline load, rustdoc export, evaluation,
/// codec encode, or file write.
#[derive(Debug)]
pub struct EvaluateSignalsError(pub String);

impl std::fmt::Display for EvaluateSignalsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub(crate) fn type_signals_track_dir(items_dir: &Path, track_id: &domain::TrackId) -> PathBuf {
    items_dir.join(track_id.as_ref())
}

pub(crate) fn reject_symlinked_type_signals_anchor(path: &Path, label: &str) -> Result<(), String> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(path).map_err(|error| {
        format!(
            "symlink guard: refusing to use symlinked {label} or ancestor '{}': {error}",
            path.display()
        )
    })?;
    path.symlink_metadata().map(|_| ()).map_err(|error| {
        format!("symlink guard: cannot stat {label} '{}': {error}", path.display())
    })
}

/// Internal observation point immediately before semantic signal evaluation.
trait SignalEvaluationObserver {
    fn signal_evaluation_started(&self);
}

struct NoopSignalEvaluationObserver;

impl SignalEvaluationObserver for NoopSignalEvaluationObserver {
    fn signal_evaluation_started(&self) {}
}

/// Evaluates type signals for a single TDDD-enabled layer using `SignalEvaluatorV2`
/// (three-way diff: catalogue A, baseline B, live rustdoc C) and writes the
/// result to `<layer>-type-signals.json`.
///
/// This replaces the old TypeGraph-based evaluator removed in T008. The output
/// format is identical to the old evaluator (schema_version 1) so the merge-gate
/// reader and pre-commit classifier continue to work without changes.
///
/// # Steps
///
/// 1. Load `<layer>-types.json` (catalogue document A) via `CatalogueDocumentCodec`.
/// 2. Load `<layer>-types-baseline.json` (baseline B) via `BaselineRustdocCodec`.
/// 3. Export current code via `cargo +nightly rustdoc` → TypeGraph C.
/// 4. Convert A → `ExtendedCrate` via `CatalogueToExtendedCrateCodec`.
/// 5. Run `SignalEvaluatorV2::evaluate(extended_a, b, c)`.
/// 6. Convert `ThreeWayEvaluationReport` → `TypeSignalsDocument` (schema_version 1).
/// 7. Compute `declaration_hash` from the catalogue file bytes as read from disk.
/// 8. Encode and write `<layer>-type-signals.json` atomically.
///
/// # Errors
///
/// Returns `EvaluateSignalsError` when any step fails.
pub fn execute_type_signals_for_layer(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
) -> Result<ExitCode, EvaluateSignalsError> {
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    execute_type_signals_for_layer_with_dependencies(
        items_dir,
        track_id,
        workspace_root,
        binding,
        &exporter,
        &NoopSignalEvaluationObserver,
    )
}

/// Execute a layer using a test-only rustdoc launch observer.
///
/// This is a composition seam, not a production extension: it is compiled
/// only with the `test-helpers` feature and retains the production evaluator,
/// codec, and adapter paths unchanged.
#[cfg(feature = "test-helpers")]
pub fn execute_type_signals_for_layer_with_launch_observer(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    observer: &RustdocLaunchObserver,
) -> Result<ExitCode, EvaluateSignalsError> {
    execute_type_signals_for_layer_with_dependencies(
        items_dir,
        track_id,
        workspace_root,
        binding,
        observer,
        &NoopSignalEvaluationObserver,
    )
}

fn execute_type_signals_for_layer_with_dependencies(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    rustdoc_paths: &impl RustdocJsonPathProvider,
    evaluation_observer: &impl SignalEvaluationObserver,
) -> Result<ExitCode, EvaluateSignalsError> {
    // Security: the workspace root is a trust anchor for Cargo metadata,
    // build-input traversal, and rustdoc snapshot resolution. Reject a
    // symlinked anchor before any of those operations can follow it outside
    // the intended workspace.
    reject_symlinked_type_signals_anchor(workspace_root, "workspace_root")
        .map_err(EvaluateSignalsError)?;

    let track_dir = type_signals_track_dir(items_dir, track_id);

    // Security: reject symlinked items_dir root before using it as a trusted anchor.
    // Following a symlinked root would allow reading/writing outside the intended workspace.
    reject_symlinked_type_signals_anchor(items_dir, "items_dir").map_err(EvaluateSignalsError)?;

    // Security: verify track_dir is contained within items_dir and reject symlinks.
    // `items_dir` is the trusted root; anything outside it is not authorised.
    let canonical_items = items_dir.canonicalize().map_err(|e| {
        EvaluateSignalsError(format!(
            "cannot canonicalize items_dir '{}': {e}",
            items_dir.display()
        ))
    })?;

    // Security: ensure `items_dir` resolves within `workspace_root`.
    // The CLI accepts `--items-dir` as a user-supplied path; without this check a
    // caller could point the evaluator at an arbitrary directory (e.g.
    // `--items-dir /etc`) and have it read catalogue files and write
    // `<layer>-type-signals.json` outside the workspace while rustdoc still runs
    // against the trusted `workspace_root`.
    let canonical_workspace = workspace_root.canonicalize().map_err(|e| {
        EvaluateSignalsError(format!(
            "cannot canonicalize workspace_root '{}': {e}",
            workspace_root.display()
        ))
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(EvaluateSignalsError(format!(
            "security: items_dir '{}' resolves to '{}' which is outside workspace_root '{}'",
            items_dir.display(),
            canonical_items.display(),
            canonical_workspace.display()
        )));
    }

    match reject_symlinks_below(&track_dir, &canonical_items) {
        Ok(true) | Ok(false) => {
            // Directory present (or absent) and not a symlink — OK.
        }
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard rejected track directory '{}': {e}",
                track_dir.display()
            )));
        }
    }

    // --- Step 1: Load catalogue document (TypeGraph A source) ---
    // Read the raw bytes first so we can compute `declaration_hash` from the
    // exact on-disk bytes (post-encode) without reading the file twice.
    let catalogue_path = track_dir.join(binding.catalogue_file());
    // Security: individual file-level symlink guard so a symlinked catalogue
    // inside a real track directory does not escape items_dir.
    match reject_symlinks_below(&catalogue_path, &canonical_items) {
        Ok(true) | Ok(false) => {}
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard rejected catalogue '{}': {e}",
                catalogue_path.display()
            )));
        }
    }
    let catalogue_bytes = std::fs::read(&catalogue_path).map_err(|e| {
        EvaluateSignalsError(format!(
            "failed to read catalogue '{}': {e}",
            catalogue_path.display()
        ))
    })?;
    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(EvaluateSignalsError(format!(
                "schema_export.targets is empty for layer '{}'",
                binding.layer_id()
            )));
        }
        multi => {
            return Err(EvaluateSignalsError(format!(
                "layer '{}' has {} schema_export.targets — multi-target not yet supported",
                binding.layer_id(),
                multi.len()
            )));
        }
    };

    // Resolve every non-snapshot freshness input before considering an existing
    // artifact. Any failure here prevents reuse and is surfaced when the fresh
    // evaluation needs the same input; no unchecked value can open a skip path.
    let baseline_path = track_dir.join(binding.baseline_file());
    // Guard the baseline before hashing it: a full freshness match returns
    // before the baseline decoder below, so a late guard could otherwise let a
    // symlinked baseline participate in SkipEvaluation.
    match reject_symlinks_below(&baseline_path, &canonical_items) {
        Ok(true) | Ok(false) => {}
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard rejected baseline '{}': {e}",
                baseline_path.display()
            )));
        }
    }
    let current_declaration_hash = type_signals_codec::declaration_hash(catalogue_bytes.as_slice());
    let baseline_content = read_utf8_file_limited(&baseline_path, MAX_RUSTDOC_SNAPSHOT_BYTES)
        .map_err(|error| {
            EvaluateSignalsError(format!(
                "failed to read baseline freshness input '{}': {error}",
                baseline_path.display()
            ))
        })?;
    let current_baseline_hash = digest_identity(baseline_content.as_bytes(), BaselineHash::new)?;
    let current_evaluator_contract_hash = evaluator_contract_hash()?;
    let current_rustdoc_extraction_contract_hash = rustdoc_extraction_contract_hash()?;
    // An unresolvable build-input closure is unsafe for reuse, but it does not
    // prohibit a fresh rustdoc launch. The final persistence step recomputes
    // this identity and returns its diagnostic if it remains unavailable.
    let implementation_input =
        hash_workspace_inputs(workspace_root, target_crate, ImplementationInputHash::new);
    let current_inputs = implementation_input.as_ref().ok().map(|implementation_input_hash| {
        TypeSignalsCurrentInputs::new(
            current_declaration_hash.clone(),
            implementation_input_hash.clone(),
            current_baseline_hash.clone(),
            current_evaluator_contract_hash.clone(),
            current_rustdoc_extraction_contract_hash.clone(),
        )
    });
    let signal_path = track_dir.join(binding.signal_file());
    let recorded = match reject_symlinks_below(&signal_path, &canonical_items) {
        Ok(false) => None,
        Ok(true) => match read_utf8_file_limited(&signal_path, MAX_TYPE_SIGNALS_BYTES) {
            Ok(json) => type_signals_codec::decode(&json).ok(),
            // An unreadable, over-limit, or malformed artifact must not open a
            // reuse path; a fresh evaluation safely replaces it atomically.
            Err(_) => None,
        },
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard rejected signal artifact '{}': {e}",
                signal_path.display()
            )));
        }
    };
    let (snapshot_status, reusable_snapshot) =
        snapshot_status_and_content(rustdoc_paths, target_crate);
    let reuse_decision =
        current_inputs.as_ref().map_or(TypeSignalsReuseDecision::ReextractAndEvaluate, |inputs| {
            reuse_decision_for_recorded_document(recorded.as_ref(), inputs, snapshot_status)
        });
    let reusable_snapshot = match reuse_decision {
        TypeSignalsReuseDecision::SkipEvaluation => return Ok(ExitCode::SUCCESS),
        TypeSignalsReuseDecision::ReevaluateWithSnapshot => reusable_snapshot,
        TypeSignalsReuseDecision::ReextractAndEvaluate => None,
    };

    // Derive the filename stem (e.g. `"domain"` from `"domain-types.json"`) for
    // `CatalogueDocumentCodec::decode`, which validates `crate_name` against it.
    let filename_stem = catalogue_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .strip_suffix("-types.json")
        .unwrap_or_else(|| catalogue_path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))
        .to_owned();

    let catalogue_str = std::str::from_utf8(&catalogue_bytes).map_err(|e| {
        EvaluateSignalsError(format!(
            "catalogue '{}' is not valid UTF-8: {e}",
            catalogue_path.display()
        ))
    })?;

    use crate::tddd::catalogue_document_codec::CatalogueDocumentCodecError;
    let doc = CatalogueDocumentCodec::decode(catalogue_str, &filename_stem).map_err(|e| {
        // Provide specific actionable messages for schema_version mismatches so
        // that tracks still using an old catalogue get a clear migration prompt
        // rather than a generic decode failure.
        match &e {
            CatalogueDocumentCodecError::SchemaVersionRequiresMigration { from, to, reason } => {
                return EvaluateSignalsError(format!(
                    "catalogue '{}' uses schema_version {from} which requires migration to \
                     schema_version {to}: {reason}. \
                     Migrate the catalogue using the type-designer agent before running \
                     `sotp signal calc-impl-catalog`.",
                    catalogue_path.display()
                ));
            }
            CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual, .. } => {
                return EvaluateSignalsError(format!(
                    "catalogue '{}' uses schema_version {actual} — \
                     SignalEvaluatorV2 requires a v5 catalogue (schema_version=5). \
                     Migrate the catalogue using the type-designer agent before running \
                     `sotp signal calc-impl-catalog`.",
                    catalogue_path.display()
                ));
            }
            _ => {}
        }
        EvaluateSignalsError(format!(
            "failed to decode catalogue '{}': {e}",
            catalogue_path.display()
        ))
    })?;

    // Build item_name → kind_tag(s) map from the catalogue before `doc` is
    // consumed by `CatalogueToExtendedCrateCodec::encode`.  The signal converter
    // uses this map so that each `TypeSignal.kind_tag` is derived directly from
    // the v3 entry's role and kind fields (see `data_role_kind_tag`,
    // `contract_role_kind_tag`, `function_role_kind_tag`).
    //
    // ## Multi-kind_tag support (name collision)
    //
    // The catalogue's `types`, `traits`, and `functions` maps use separate
    // namespaces (distinct `BTreeMap` keys: `TypeName`, `TraitName`,
    // `FunctionPath`).  When a layer declares both a type and a trait with the
    // same short name (e.g. `Foo` type + `Foo` trait), `check_type_signals`
    // expects TWO signal entries — one for `("Foo", "value_object")` and one
    // for `("Foo", "secondary_port")`.  Collapsing them to a single entry
    // (first-wins) would leave one declaration permanently uncovered.
    //
    // Therefore `kind_tag_map` stores `Vec<&'static str>` per name.  Types are
    // pushed first, traits second; functions use fully-qualified `FunctionPath`
    // keys and never collide with short-name type/trait entries.
    //
    // ## BTreeMap for deterministic output
    //
    // Using `BTreeMap` (sorted by name) instead of `HashMap` ensures that the
    // synthetic Blue entries synthesized below for `SIntersectC_Match_Reference`
    // skip-bucket items are appended to `order` in a stable, reproducible order.
    // This prevents spurious diffs and flaky pre-commit output whenever the
    // report omits reference items.
    let kind_tag_map: BTreeMap<String, Vec<&'static str>> = {
        let mut m: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
        for (name, entry) in doc.types() {
            m.entry(name.as_str().to_owned())
                .or_default()
                .push(data_role_kind_tag(entry.role(), entry.kind()));
        }
        for (name, entry) in doc.traits() {
            m.entry(name.as_str().to_owned())
                .or_default()
                .push(contract_role_kind_tag(entry.role()));
        }
        for (path, entry) in doc.functions() {
            // T012 ensures that CatalogueDocumentCodec rejects cross-crate function
            // paths at decode time (CrossCrateFunctionPath error), so all function
            // paths here already carry the catalogue's own crate_name prefix.
            // No cross-crate filtering is needed.

            // FunctionPath keys are fully qualified (e.g. "crate::fn_name") and
            // never collide with short-name type/trait keys.
            m.entry(path.to_string()).or_default().push(function_role_kind_tag(entry.role()));
        }
        m
    };

    // --- Step 2: Convert CatalogueDocument → ExtendedCrate (A) ---
    let ext_crate_codec = CatalogueToExtendedCrateCodec::new();
    let extended_a = ext_crate_codec.encode(doc).map_err(|e| {
        EvaluateSignalsError(format!(
            "CatalogueToExtendedCrateCodec error for layer '{}': {e}",
            binding.layer_id()
        ))
    })?;

    // --- Step 3: Load baseline (TypeGraph B) ---
    if !baseline_path.is_file() {
        return Err(EvaluateSignalsError(format!(
            "baseline file not found: {} — run `sotp track baseline-capture {}` first \
             (rustdoc format; delete old TypeBaseline JSON if present and re-capture)",
            baseline_path.display(),
            track_id,
        )));
    }
    let baseline_b = BaselineRustdocCodec::from_json(&baseline_content).map_err(|e| {
        EvaluateSignalsError(format!("failed to load baseline '{}': {e}", baseline_path.display()))
    })?;

    // --- Step 4: Capture current TypeGraph (C) via rustdoc ---
    // Security: reject symlinked workspace_root before invoking rustdoc.
    // A symlinked workspace root could redirect the build to an arbitrary
    // directory outside the trusted workspace tree.
    reject_symlinked_type_signals_anchor(workspace_root, "workspace_root")
        .map_err(EvaluateSignalsError)?;

    let json_content = match reusable_snapshot {
        Some(content) => content,
        None => {
            let json_path = rustdoc_paths.export_rustdoc_json_path(target_crate).map_err(|e| {
                EvaluateSignalsError(format!(
                    "rustdoc export failed for crate '{target_crate}' (layer '{}'): {e}",
                    binding.layer_id()
                ))
            })?;
            read_utf8_file_limited(&json_path, MAX_RUSTDOC_SNAPSHOT_BYTES).map_err(|e| {
                EvaluateSignalsError(format!(
                    "failed to read rustdoc JSON '{}': {e}",
                    json_path.display()
                ))
            })?
        }
    };
    let current_c = BaselineRustdocCodec::from_json(&json_content).map_err(|e| {
        EvaluateSignalsError(format!(
            "failed to parse rustdoc JSON for crate '{target_crate}': {e}"
        ))
    })?;

    // --- Step 5: Evaluate ---
    let evaluator = SignalEvaluatorV2::with_workspace_root(workspace_root.to_path_buf());
    evaluation_observer.signal_evaluation_started();
    let report = evaluator.evaluate(extended_a, baseline_b, current_c).map_err(|e| {
        EvaluateSignalsError(format!(
            "signal evaluation error for layer '{}': {e:?}",
            binding.layer_id()
        ))
    })?;

    // --- Step 6: Convert ThreeWayEvaluationReport → TypeSignalsDocument ---
    let signals: Vec<TypeSignal> = build_type_signals_from_report(report.iter(), &kind_tag_map);

    // --- Step 7: Verify and retain the identities from before evaluation ---
    let implementation_input_hash = implementation_input?;
    let freshness_inputs = TypeSignalsCurrentInputs::new(
        current_declaration_hash,
        implementation_input_hash,
        current_baseline_hash,
        current_evaluator_contract_hash,
        current_rustdoc_extraction_contract_hash,
    );
    verify_evaluation_inputs_unchanged(
        workspace_root,
        target_crate,
        &catalogue_path,
        &baseline_path,
        &freshness_inputs,
    )?;
    let live_rustdoc_snapshot_hash =
        digest_identity(json_content.as_bytes(), LiveRustdocSnapshotHash::new)?;

    // --- Build the generated_at timestamp (UTC, Z suffix required by codec) ---
    let now_str = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let generated_at = Timestamp::new(&now_str).map_err(|e| {
        EvaluateSignalsError(format!("failed to construct generated_at timestamp: {e}"))
    })?;

    let freshness = TypeSignalsFreshness::new(
        freshness_inputs.declaration_hash().clone(),
        freshness_inputs.implementation_input_hash().clone(),
        freshness_inputs.baseline_hash().clone(),
        live_rustdoc_snapshot_hash,
        freshness_inputs.evaluator_contract_hash().clone(),
        freshness_inputs.rustdoc_extraction_contract_hash().clone(),
    );
    let doc = TypeSignalsDocument::new(generated_at, freshness, signals);

    // --- Step 8: Encode and write <layer>-type-signals.json ---
    let signal_json = type_signals_codec::encode(&doc).map_err(|e| {
        EvaluateSignalsError(format!(
            "failed to encode type-signals for layer '{}': {e}",
            binding.layer_id()
        ))
    })?;

    let signal_path = track_dir.join(binding.signal_file());
    // Write the JSON with a trailing newline so the file matches the codec's
    // pretty-print format. `serde_json::to_string_pretty` does not add a
    // trailing newline; we append one for consistency with `git diff`.
    //
    // Use atomic_write_file (tmp + fsync + rename) to:
    // (a) refuse to follow pre-existing symlinks (create_new guard), and
    // (b) leave the old file intact on crash rather than a truncated one.
    let signal_json_with_newline = format!("{signal_json}\n");
    atomic_write_file(&signal_path, signal_json_with_newline.as_bytes()).map_err(|e| {
        EvaluateSignalsError(format!(
            "failed to write signal file '{}': {e}",
            signal_path.display()
        ))
    })?;

    // Print a summary so the pre-commit output is informative.
    let layer_id = binding.layer_id();
    let blue = report.iter().filter(|s| s.signal().is_blue()).count();
    let yellow = report.iter().filter(|s| s.signal().is_yellow()).count();
    let red = report.iter().filter(|s| s.signal().is_red()).count();
    eprintln!(
        "[type-signals] {layer_id}: 🔵 {blue} Blue | 🟡 {yellow} Yellow | 🔴 {red} Red \
         → {signal_path}",
        signal_path = signal_path.display()
    );

    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use sha2::Digest;

    use domain::tddd::catalogue_v2::composite::{
        StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions,
    };
    use domain::tddd::catalogue_v2::identifiers::{MethodName, TypeName, TypeRef};
    use domain::tddd::catalogue_v2::roles::{DataRole, FunctionRole};
    use domain::tddd::type_signals_doc::{
        EvaluatorContractHash, LiveRustdocSnapshotStatus, RustdocExtractionContractHash,
        Sha256Digest,
    };

    use super::*;

    struct RustdocLaunchSpy {
        existing_snapshot_path: PathBuf,
        exported_snapshot_path: PathBuf,
        launch_count: AtomicUsize,
    }

    impl RustdocLaunchSpy {
        fn using_snapshot(snapshot_path: PathBuf) -> Self {
            Self {
                existing_snapshot_path: snapshot_path.clone(),
                exported_snapshot_path: snapshot_path,
                launch_count: AtomicUsize::new(0),
            }
        }

        fn launches(&self) -> usize {
            self.launch_count.load(Ordering::SeqCst)
        }
    }

    impl RustdocJsonPathProvider for RustdocLaunchSpy {
        fn export_rustdoc_json_path(
            &self,
            _crate_name: &str,
        ) -> Result<PathBuf, SchemaExportError> {
            self.launch_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.exported_snapshot_path.clone())
        }

        fn existing_rustdoc_json_path(
            &self,
            _crate_name: &str,
        ) -> Result<PathBuf, SchemaExportError> {
            Ok(self.existing_snapshot_path.clone())
        }
    }

    struct SignalEvaluationSpy(AtomicUsize);

    impl SignalEvaluationSpy {
        fn evaluations(&self) -> usize {
            self.0.load(Ordering::SeqCst)
        }
    }

    impl SignalEvaluationObserver for SignalEvaluationSpy {
        fn signal_evaluation_started(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct InputMutationObserver {
        path: PathBuf,
        replacement: String,
        mutation_failed: AtomicBool,
    }

    impl SignalEvaluationObserver for InputMutationObserver {
        fn signal_evaluation_started(&self) {
            if fs::write(&self.path, &self.replacement).is_err() {
                self.mutation_failed.store(true, Ordering::SeqCst);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinked_type_signals_anchor_rejects_workspace_root() {
        let parent = tempfile::tempdir().unwrap();
        let workspace = parent.path().join("workspace");
        let workspace_link = parent.path().join("workspace-link");
        fs::create_dir(&workspace).unwrap();
        std::os::unix::fs::symlink(&workspace, &workspace_link).unwrap();

        let error =
            reject_symlinked_type_signals_anchor(&workspace_link, "workspace_root").unwrap_err();

        assert!(error.contains("symlinked workspace_root"));
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinked_type_signals_anchor_rejects_symlinked_parent() {
        let parent = tempfile::tempdir().unwrap();
        let real_parent = parent.path().join("real-parent");
        let workspace = real_parent.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let linked_parent = parent.path().join("linked-parent");
        std::os::unix::fs::symlink(&real_parent, &linked_parent).unwrap();

        let error = reject_symlinked_type_signals_anchor(
            &linked_parent.join("workspace"),
            "workspace_root",
        )
        .unwrap_err();

        assert!(error.contains("symlinked workspace_root"));
        assert!(error.contains("refusing to follow symlink"));
    }

    /// T015: verify that `function_role_kind_tag` maps all `FunctionRole` variants to
    /// `"free_function"` uniformly (same tag for all, no cross-crate filtering needed).
    ///
    /// Since T012 rejects cross-crate function paths at decode time, the kind_tag_map
    /// building loop in `execute_type_signals_for_layer` no longer needs to skip any
    /// function entries — all entries present in the document are own-crate functions.
    #[test]
    fn test_function_role_kind_tag_returns_free_function_for_all_variants() {
        assert_eq!(function_role_kind_tag(FunctionRole::FreeFunction), "free_function");
        assert_eq!(function_role_kind_tag(FunctionRole::UseCaseFunction), "free_function");
    }

    // -----------------------------------------------------------------------
    // T005 / AC-01: unit struct + typestate → kind_tag "typestate"
    // -----------------------------------------------------------------------

    #[test]
    fn test_data_role_kind_tag_unit_struct_with_typestate_returns_typestate() {
        // AC-01: a unit struct carrying a typestate marker must be classified as "typestate".
        let marker = TypestateMarker::new(
            TypeName::new("LockMachine").unwrap(),
            TypestateTransitions::new(vec![MethodName::new("unlock").unwrap()]),
        );
        let kind = TypeKindV2::Struct(StructKind::new(StructShape::Unit, Some(marker)));
        assert_eq!(data_role_kind_tag(&DataRole::value_object(), &kind), "typestate");
    }

    // -----------------------------------------------------------------------
    // T005 / AC-02: tuple struct + typestate → kind_tag "typestate"
    // -----------------------------------------------------------------------

    #[test]
    fn test_data_role_kind_tag_tuple_struct_with_typestate_returns_typestate() {
        // AC-02: a tuple struct carrying a typestate marker must be classified as "typestate".
        let marker = TypestateMarker::new(
            TypeName::new("ApprovalMachine").unwrap(),
            TypestateTransitions::new(vec![MethodName::new("approve").unwrap()]),
        );
        let kind = TypeKindV2::Struct(StructKind::new(
            StructShape::Tuple {
                fields: vec![TypeRef::new("Uuid").unwrap()],
                has_stripped_fields: false,
            },
            Some(marker),
        ));
        assert_eq!(data_role_kind_tag(&DataRole::value_object(), &kind), "typestate");
    }

    // -----------------------------------------------------------------------
    // T005 / AC-07 regression: plain struct + typestate → kind_tag "typestate"
    // -----------------------------------------------------------------------

    #[test]
    fn test_data_role_kind_tag_plain_struct_with_typestate_returns_typestate() {
        // AC-07: existing plain struct + typestate must still be classified as "typestate".
        let marker = TypestateMarker::new(
            TypeName::new("ReviewMachine").unwrap(),
            TypestateTransitions::new(vec![]),
        );
        let kind = TypeKindV2::Struct(StructKind::new(
            StructShape::Plain { fields: vec![], has_stripped_fields: false },
            Some(marker),
        ));
        assert_eq!(data_role_kind_tag(&DataRole::value_object(), &kind), "typestate");
    }

    #[test]
    fn test_data_role_kind_tag_unit_struct_without_typestate_returns_role_tag() {
        // Without typestate, a unit struct falls through to role-based mapping.
        let kind = TypeKindV2::Struct(StructKind::new(StructShape::Unit, None));
        assert_eq!(data_role_kind_tag(&DataRole::value_object(), &kind), "value_object");
    }

    #[test]
    fn test_execute_type_signals_for_layer_missing_items_dir_returns_error() {
        let mut bindings = crate::verify::tddd_layers::parse_tddd_layers(
            r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"schema_export":{"method":"rustdoc","targets":["domain"]}}}]}"#,
        )
        .unwrap();
        let binding = bindings.pop().unwrap();
        let directory = tempfile::tempdir().unwrap();

        let error = execute_type_signals_for_layer(
            &directory.path().join("track/items"),
            &domain::TrackId::try_new("test-track").unwrap(),
            directory.path(),
            &binding,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("symlink guard: cannot stat items_dir"),
            "a missing items directory must fail before rustdoc work: {error}"
        );
    }

    const FRESHNESS_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const FRESHNESS_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn test_embedded_contract_digests_are_valid_and_distinct() {
        let evaluator = evaluator_contract_hash().unwrap();
        let extraction = rustdoc_extraction_contract_hash().unwrap();

        assert_eq!(evaluator.as_digest().as_str().len(), 64);
        assert_eq!(extraction.as_digest().as_str().len(), 64);
        assert_ne!(evaluator, EvaluatorContractHash::new(extraction.as_digest().clone()));
    }

    fn freshness_digest(value: &str) -> Sha256Digest {
        Sha256Digest::try_new(value.to_owned()).unwrap()
    }

    fn nightly_toolchain_available() -> bool {
        std::process::Command::new("rustup")
            .args(["run", "nightly", "rustc", "-Vv"])
            .status()
            .is_ok_and(|status| status.success())
    }

    fn recorded_type_signals_document() -> TypeSignalsDocument {
        let digest = freshness_digest(FRESHNESS_A);
        TypeSignalsDocument::new(
            Timestamp::new("2026-07-14T00:00:00Z").unwrap(),
            TypeSignalsFreshness::new(
                type_signals_codec::declaration_hash(b"catalogue"),
                ImplementationInputHash::new(digest.clone()),
                BaselineHash::new(digest.clone()),
                LiveRustdocSnapshotHash::new(digest.clone()),
                EvaluatorContractHash::new(digest.clone()),
                RustdocExtractionContractHash::new(digest),
            ),
            vec![],
        )
    }

    fn current_inputs_for(recorded: &TypeSignalsDocument) -> TypeSignalsCurrentInputs {
        TypeSignalsCurrentInputs::new(
            recorded.declaration_hash().clone(),
            recorded.freshness().implementation_input_hash().clone(),
            recorded.freshness().baseline_hash().clone(),
            recorded.freshness().evaluator_contract_hash().clone(),
            recorded.freshness().rustdoc_extraction_contract_hash().clone(),
        )
    }

    struct FreshnessFixture {
        workspace: tempfile::TempDir,
        items_dir: std::path::PathBuf,
        track_id: TrackId,
        binding: TdddLayerBinding,
        catalogue_path: std::path::PathBuf,
        signal_path: std::path::PathBuf,
        snapshot_path: std::path::PathBuf,
    }

    impl FreshnessFixture {
        fn new() -> Self {
            let workspace = tempfile::tempdir().unwrap();
            let workspace_root = workspace.path();
            fs::create_dir_all(workspace_root.join("crates/domain/src")).unwrap();
            fs::create_dir_all(workspace_root.join("target/doc")).unwrap();
            fs::write(
                workspace_root.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/domain\"]\nresolver = \"2\"\n",
            )
            .unwrap();
            fs::write(
                workspace_root.join("Cargo.lock"),
                "version = 4\n\n[[package]]\nname = \"domain\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
            fs::write(
                workspace_root.join("crates/domain/Cargo.toml"),
                "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            )
            .unwrap();
            fs::write(workspace_root.join("crates/domain/src/lib.rs"), "pub struct Fixture;\n")
                .unwrap();

            let items_dir = workspace_root.join("track/items");
            let track_id = TrackId::try_new("freshness-fixture").unwrap();
            let track_dir = type_signals_track_dir(&items_dir, &track_id);
            fs::create_dir_all(&track_dir).unwrap();
            let catalogue_path = track_dir.join("domain-types.json");
            fs::write(
                &catalogue_path,
                "{\n  \"schema_version\": 5,\n  \"crate_name\": \"domain\",\n  \"layer\": \"domain\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
            )
            .unwrap();
            let snapshot_path = workspace_root.join("target/doc/domain.json");
            let snapshot = minimal_rustdoc_json();
            fs::write(&snapshot_path, &snapshot).unwrap();
            fs::write(track_dir.join("domain-types-baseline.json"), snapshot).unwrap();

            Self {
                workspace,
                items_dir,
                track_id,
                binding: crate::verify::tddd_layers::parse_tddd_layers(
                    r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"schema_export":{"method":"rustdoc","targets":["domain"]}}}]}"#,
                )
                .unwrap()
                .pop()
                .unwrap(),
                catalogue_path,
                signal_path: track_dir.join("domain-type-signals.json"),
                snapshot_path,
            }
        }

        fn seed_current_artifact(&self) {
            let catalogue = fs::read(&self.catalogue_path).unwrap();
            let snapshot = fs::read(&self.snapshot_path).unwrap();
            let freshness = TypeSignalsFreshness::new(
                type_signals_codec::declaration_hash(&catalogue),
                hash_workspace_inputs(
                    self.workspace.path(),
                    "domain",
                    ImplementationInputHash::new,
                )
                .unwrap(),
                digest_identity(
                    &fs::read(
                        type_signals_track_dir(&self.items_dir, &self.track_id)
                            .join("domain-types-baseline.json"),
                    )
                    .unwrap(),
                    BaselineHash::new,
                )
                .unwrap(),
                digest_identity(&snapshot, LiveRustdocSnapshotHash::new).unwrap(),
                evaluator_contract_hash().unwrap(),
                rustdoc_extraction_contract_hash().unwrap(),
            );
            let document = TypeSignalsDocument::new(
                Timestamp::new("2026-07-14T00:00:00Z").unwrap(),
                freshness,
                vec![],
            );
            fs::write(&self.signal_path, type_signals_codec::encode(&document).unwrap()).unwrap();
        }

        fn evaluate_with(
            &self,
            rustdoc_paths: &impl RustdocJsonPathProvider,
            evaluation_observer: &impl SignalEvaluationObserver,
        ) -> Result<ExitCode, EvaluateSignalsError> {
            execute_type_signals_for_layer_with_dependencies(
                &self.items_dir,
                &self.track_id,
                self.workspace.path(),
                &self.binding,
                rustdoc_paths,
                evaluation_observer,
            )
        }

        fn evaluation_spies(&self) -> (RustdocLaunchSpy, SignalEvaluationSpy) {
            (
                RustdocLaunchSpy::using_snapshot(self.snapshot_path.clone()),
                SignalEvaluationSpy(AtomicUsize::new(0)),
            )
        }
    }

    fn minimal_rustdoc_json() -> String {
        format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        )
    }

    #[test]
    fn test_hash_workspace_inputs_source_change_reextracts_only_affected_layer() {
        if !nightly_toolchain_available() {
            eprintln!("skipping build-input closure lane: nightly toolchain is unavailable");
            return;
        }

        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        fs::create_dir_all(root.join("crates/alpha/src")).unwrap();
        fs::create_dir_all(root.join("crates/beta/src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/alpha\", \"crates/beta\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        fs::write(
            root.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"alpha\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"beta\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        for crate_name in ["alpha", "beta"] {
            fs::write(
                root.join("crates").join(crate_name).join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
                ),
            )
            .unwrap();
            fs::write(
                root.join("crates").join(crate_name).join("src/lib.rs"),
                format!("pub struct {} ;\n", crate_name.to_uppercase()),
            )
            .unwrap();
        }

        let alpha_before =
            hash_workspace_inputs(root, "alpha", ImplementationInputHash::new).unwrap();
        let beta_before =
            hash_workspace_inputs(root, "beta", ImplementationInputHash::new).unwrap();
        fs::write(root.join("crates/alpha/src/lib.rs"), "pub struct ChangedAlpha;\n").unwrap();
        let alpha_after =
            hash_workspace_inputs(root, "alpha", ImplementationInputHash::new).unwrap();
        let beta_after = hash_workspace_inputs(root, "beta", ImplementationInputHash::new).unwrap();

        assert_ne!(alpha_before, alpha_after, "the changed crate must be re-extracted");
        assert_eq!(beta_before, beta_after, "an unrelated layer must retain its closure hash");

        let template = recorded_type_signals_document();
        let beta_recorded = TypeSignalsDocument::new(
            template.generated_at().clone(),
            TypeSignalsFreshness::new(
                template.declaration_hash().clone(),
                beta_before,
                template.freshness().baseline_hash().clone(),
                template.freshness().live_rustdoc_snapshot_hash().clone(),
                template.freshness().evaluator_contract_hash().clone(),
                template.freshness().rustdoc_extraction_contract_hash().clone(),
            ),
            template.signals().to_vec(),
        );
        let alpha_recorded = TypeSignalsDocument::new(
            template.generated_at().clone(),
            TypeSignalsFreshness::new(
                template.declaration_hash().clone(),
                alpha_before,
                template.freshness().baseline_hash().clone(),
                template.freshness().live_rustdoc_snapshot_hash().clone(),
                template.freshness().evaluator_contract_hash().clone(),
                template.freshness().rustdoc_extraction_contract_hash().clone(),
            ),
            template.signals().to_vec(),
        );
        let unaffected = TypeSignalsCurrentInputs::new(
            beta_recorded.declaration_hash().clone(),
            beta_after,
            beta_recorded.freshness().baseline_hash().clone(),
            beta_recorded.freshness().evaluator_contract_hash().clone(),
            beta_recorded.freshness().rustdoc_extraction_contract_hash().clone(),
        );
        let affected = TypeSignalsCurrentInputs::new(
            alpha_recorded.declaration_hash().clone(),
            alpha_after,
            alpha_recorded.freshness().baseline_hash().clone(),
            alpha_recorded.freshness().evaluator_contract_hash().clone(),
            alpha_recorded.freshness().rustdoc_extraction_contract_hash().clone(),
        );
        assert_eq!(
            reuse_decision_for_recorded_document(
                Some(&beta_recorded),
                &unaffected,
                LiveRustdocSnapshotStatus::Verified(
                    beta_recorded.freshness().live_rustdoc_snapshot_hash().clone(),
                ),
            ),
            TypeSignalsReuseDecision::SkipEvaluation,
            "the unaffected layer must retain its skip path"
        );
        assert_eq!(
            reuse_decision_for_recorded_document(
                Some(&alpha_recorded),
                &affected,
                LiveRustdocSnapshotStatus::Verified(
                    alpha_recorded.freshness().live_rustdoc_snapshot_hash().clone(),
                ),
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
    }

    #[test]
    fn test_execute_type_signals_for_layer_full_match_skips_evaluation_without_rustdoc() {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        let fixture = FreshnessFixture::new();
        fixture.seed_current_artifact();
        let before = fs::read(&fixture.signal_path).unwrap();
        let persisted = type_signals_codec::decode(std::str::from_utf8(&before).unwrap()).unwrap();
        let freshness = persisted.freshness();
        let catalogue = fs::read(&fixture.catalogue_path).unwrap();
        let baseline = fs::read(
            type_signals_track_dir(&fixture.items_dir, &fixture.track_id)
                .join("domain-types-baseline.json"),
        )
        .unwrap();
        let snapshot = fs::read(&fixture.snapshot_path).unwrap();
        assert_eq!(
            persisted.declaration_hash(),
            &type_signals_codec::declaration_hash(&catalogue),
            "the persisted declaration identity must match the current catalogue"
        );
        assert_eq!(
            freshness.implementation_input_hash(),
            &hash_workspace_inputs(
                fixture.workspace.path(),
                "domain",
                ImplementationInputHash::new,
            )
            .unwrap(),
            "the persisted implementation identity must match the resolved build inputs"
        );
        assert_eq!(
            freshness.baseline_hash(),
            &BaselineHash::new(
                Sha256Digest::try_new(format!("{:x}", sha2::Sha256::digest(&baseline))).unwrap(),
            ),
            "the persisted baseline identity must match the baseline bytes"
        );
        assert_eq!(
            freshness.live_rustdoc_snapshot_hash(),
            &LiveRustdocSnapshotHash::new(
                Sha256Digest::try_new(format!("{:x}", sha2::Sha256::digest(&snapshot))).unwrap(),
            ),
            "the persisted snapshot hash must match the parseable live rustdoc JSON"
        );
        assert_eq!(
            freshness.evaluator_contract_hash(),
            &evaluator_contract_hash().unwrap(),
            "the persisted evaluator-contract identity must match its complete source closure"
        );
        assert_eq!(
            freshness.rustdoc_extraction_contract_hash(),
            &rustdoc_extraction_contract_hash().unwrap(),
            "the persisted extraction-contract identity must match its complete source closure"
        );
        for (label, digest) in [
            ("declaration", persisted.declaration_hash().as_digest()),
            ("implementation", freshness.implementation_input_hash().as_digest()),
            ("baseline", freshness.baseline_hash().as_digest()),
            ("snapshot", freshness.live_rustdoc_snapshot_hash().as_digest()),
            ("evaluator contract", freshness.evaluator_contract_hash().as_digest()),
            (
                "rustdoc extraction contract",
                freshness.rustdoc_extraction_contract_hash().as_digest(),
            ),
        ] {
            assert_eq!(
                digest.as_str(),
                digest.as_str().to_ascii_lowercase(),
                "the persisted {label} identity must be lowercase SHA-256 hex"
            );
        }
        let (rustdoc, evaluation) = fixture.evaluation_spies();

        let result = fixture.evaluate_with(&rustdoc, &evaluation);

        assert!(result.is_ok(), "a verified full match must skip rustdoc: {result:?}");
        assert_eq!(rustdoc.launches(), 0, "a full match must not launch rustdoc");
        assert_eq!(evaluation.evaluations(), 0, "a full match must not evaluate signals");
        assert_eq!(
            fs::read(&fixture.signal_path).unwrap(),
            before,
            "a skip must not rewrite signals"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_for_layer_symlinked_baseline_rejects_before_skip() {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        let fixture = FreshnessFixture::new();
        fixture.seed_current_artifact();
        let baseline = type_signals_track_dir(&fixture.items_dir, &fixture.track_id)
            .join("domain-types-baseline.json");
        let outside = fixture.workspace.path().join("outside-baseline.json");
        fs::rename(&baseline, &outside).unwrap();
        std::os::unix::fs::symlink(&outside, &baseline).unwrap();

        let (rustdoc, evaluation) = fixture.evaluation_spies();
        let error = fixture.evaluate_with(&rustdoc, &evaluation).unwrap_err();

        assert!(error.to_string().contains("symlink guard rejected baseline"));
        assert_eq!(rustdoc.launches(), 0, "baseline guard must run before snapshot reuse");
        assert_eq!(evaluation.evaluations(), 0, "baseline guard must run before evaluation");
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_for_layer_symlinked_signal_artifact_rejects_before_skip() {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        let fixture = FreshnessFixture::new();
        fixture.seed_current_artifact();
        let outside = fixture.workspace.path().join("outside-type-signals.json");
        fs::rename(&fixture.signal_path, &outside).unwrap();
        std::os::unix::fs::symlink(&outside, &fixture.signal_path).unwrap();

        let (rustdoc, evaluation) = fixture.evaluation_spies();
        let error = fixture.evaluate_with(&rustdoc, &evaluation).unwrap_err();

        assert!(error.to_string().contains("symlink guard rejected signal artifact"));
        assert_eq!(rustdoc.launches(), 0, "artifact guard must run before snapshot reuse");
        assert_eq!(evaluation.evaluations(), 0, "artifact guard must run before evaluation");
    }

    #[test]
    fn test_read_utf8_file_limited_oversized_file_returns_invalid_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oversized.json");
        let file = fs::File::create(&path).unwrap();
        file.set_len((MAX_TYPE_SIGNALS_BYTES + 1) as u64).unwrap();

        let error = read_utf8_file_limited(&path, MAX_TYPE_SIGNALS_BYTES).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds maximum size"));
    }

    #[test]
    fn test_execute_type_signals_for_layer_catalogue_baseline_or_evaluator_change_reevaluates_verified_snapshot_without_rustdoc()
     {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        for lane in ["catalogue", "baseline", "evaluator"] {
            let fixture = FreshnessFixture::new();
            fixture.seed_current_artifact();
            if lane == "catalogue" {
                fs::write(
                    &fixture.catalogue_path,
                    "{\n  \"schema_version\": 5,\n  \"crate_name\": \"domain\",\n  \"layer\": \"domain\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n\n",
                )
                .unwrap();
            } else if lane == "baseline" {
                let baseline = fs::read_to_string(
                    type_signals_track_dir(&fixture.items_dir, &fixture.track_id)
                        .join("domain-types-baseline.json"),
                )
                .unwrap();
                fs::write(
                    type_signals_track_dir(&fixture.items_dir, &fixture.track_id)
                        .join("domain-types-baseline.json"),
                    format!("{baseline}\n"),
                )
                .unwrap();
            } else {
                let document =
                    type_signals_codec::decode(&fs::read_to_string(&fixture.signal_path).unwrap())
                        .unwrap();
                let freshness = document.freshness();
                let changed = TypeSignalsFreshness::new(
                    freshness.declaration_hash().clone(),
                    freshness.implementation_input_hash().clone(),
                    freshness.baseline_hash().clone(),
                    freshness.live_rustdoc_snapshot_hash().clone(),
                    EvaluatorContractHash::new(freshness_digest(FRESHNESS_B)),
                    freshness.rustdoc_extraction_contract_hash().clone(),
                );
                let changed_document = TypeSignalsDocument::new(
                    document.generated_at().clone(),
                    changed,
                    document.signals().to_vec(),
                );
                fs::write(
                    &fixture.signal_path,
                    type_signals_codec::encode(&changed_document).unwrap(),
                )
                .unwrap();
            }

            let (rustdoc, evaluation) = fixture.evaluation_spies();
            let result = fixture.evaluate_with(&rustdoc, &evaluation);

            assert!(
                result.is_ok(),
                "{lane} change must evaluate from the verified snapshot: {result:?}"
            );
            assert_eq!(rustdoc.launches(), 0, "{lane} change must not launch rustdoc");
            assert_eq!(evaluation.evaluations(), 1, "{lane} change must reevaluate signals");
        }
    }

    #[test]
    fn test_execute_type_signals_for_layer_implementation_or_extraction_contract_change_launches_rustdoc()
     {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        for lane in ["implementation", "extraction-contract"] {
            let fixture = FreshnessFixture::new();
            fixture.seed_current_artifact();
            if lane == "implementation" {
                fs::write(
                    fixture.workspace.path().join("Cargo.lock"),
                    "# changed lockfile\nversion = 4\n\n[[package]]\nname = \"domain\"\nversion = \"0.1.0\"\n",
                )
                .unwrap();
            } else {
                let document =
                    type_signals_codec::decode(&fs::read_to_string(&fixture.signal_path).unwrap())
                        .unwrap();
                let freshness = document.freshness();
                let changed = TypeSignalsFreshness::new(
                    freshness.declaration_hash().clone(),
                    freshness.implementation_input_hash().clone(),
                    freshness.baseline_hash().clone(),
                    freshness.live_rustdoc_snapshot_hash().clone(),
                    freshness.evaluator_contract_hash().clone(),
                    RustdocExtractionContractHash::new(freshness_digest(FRESHNESS_B)),
                );
                let changed_document = TypeSignalsDocument::new(
                    document.generated_at().clone(),
                    changed,
                    document.signals().to_vec(),
                );
                fs::write(
                    &fixture.signal_path,
                    type_signals_codec::encode(&changed_document).unwrap(),
                )
                .unwrap();
            }

            let (rustdoc, evaluation) = fixture.evaluation_spies();
            let result = fixture.evaluate_with(&rustdoc, &evaluation);

            assert!(result.is_ok(), "{lane} change must re-extract: {result:?}");
            assert_eq!(rustdoc.launches(), 1, "{lane} change must launch rustdoc");
            assert_eq!(evaluation.evaluations(), 1, "{lane} change must evaluate signals");
        }
    }

    #[test]
    fn test_execute_type_signals_for_layer_snapshot_failure_or_incomplete_artifact_launches_rustdoc()
     {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        for lane in [
            "missing",
            "read-failed",
            "parse-fail",
            "hash-mismatch",
            "legacy",
            "implementation_input_hash",
            "baseline_hash",
            "live_rustdoc_snapshot_hash",
            "evaluator_contract_hash",
            "rustdoc_extraction_contract_hash",
        ] {
            let fixture = FreshnessFixture::new();
            fixture.seed_current_artifact();
            let mut rustdoc = RustdocLaunchSpy::using_snapshot(fixture.snapshot_path.clone());
            if lane == "missing" {
                rustdoc.existing_snapshot_path =
                    fixture.workspace.path().join("target/doc/missing.json");
            } else if lane == "read-failed" {
                let unreadable = fixture.workspace.path().join("target/doc/unreadable.json");
                fs::create_dir(&unreadable).unwrap();
                rustdoc.existing_snapshot_path = unreadable;
            } else if lane == "parse-fail" {
                let invalid = fixture.workspace.path().join("target/doc/invalid.json");
                fs::write(&invalid, "not rustdoc JSON").unwrap();
                rustdoc.existing_snapshot_path = invalid;
            } else if lane == "hash-mismatch" {
                let mismatched = fixture.workspace.path().join("target/doc/mismatched.json");
                fs::write(&mismatched, format!("{}\n", minimal_rustdoc_json())).unwrap();
                rustdoc.existing_snapshot_path = mismatched;
            } else if lane == "legacy" {
                let legacy = fs::read_to_string(&fixture.signal_path).unwrap().replacen(
                    "\"schema_version\": 2",
                    "\"schema_version\": 1",
                    1,
                );
                fs::write(&fixture.signal_path, legacy).unwrap();
            } else {
                let mut incomplete: serde_json::Value =
                    serde_json::from_str(&fs::read_to_string(&fixture.signal_path).unwrap())
                        .unwrap();
                incomplete.as_object_mut().unwrap().remove(lane).unwrap();
                fs::write(&fixture.signal_path, serde_json::to_string(&incomplete).unwrap())
                    .unwrap();
            }
            let evaluation = SignalEvaluationSpy(AtomicUsize::new(0));

            let result = fixture.evaluate_with(&rustdoc, &evaluation);

            assert!(
                result.is_ok(),
                "{lane} must re-extract from the exported snapshot: {result:?}"
            );
            assert_eq!(rustdoc.launches(), 1, "{lane} must launch rustdoc");
            assert_eq!(
                evaluation.evaluations(),
                1,
                "{lane} must evaluate signals after re-extraction"
            );
        }
    }

    #[test]
    fn test_execute_type_signals_for_layer_malformed_persisted_digest_launches_rustdoc() {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        for (lane, malformed_digest) in
            [("invalid-length", "short".to_owned()), ("invalid-hex", FRESHNESS_A.to_uppercase())]
        {
            let fixture = FreshnessFixture::new();
            fixture.seed_current_artifact();
            let mut artifact: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&fixture.signal_path).unwrap()).unwrap();
            artifact.as_object_mut().unwrap().insert(
                "implementation_input_hash".to_owned(),
                serde_json::Value::String(malformed_digest),
            );
            fs::write(&fixture.signal_path, serde_json::to_string(&artifact).unwrap()).unwrap();

            let (rustdoc, evaluation) = fixture.evaluation_spies();
            let result = fixture.evaluate_with(&rustdoc, &evaluation);

            assert!(
                result.is_ok(),
                "{lane} persisted digest must fail closed and re-extract: {result:?}"
            );
            assert_eq!(rustdoc.launches(), 1, "{lane} must launch rustdoc");
            assert_eq!(
                evaluation.evaluations(),
                1,
                "{lane} must evaluate signals after re-extraction"
            );
        }
    }

    #[test]
    fn test_snapshot_status_and_content_classifies_verified_and_unreadable_targets() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("valid.json");
        fs::write(&valid, minimal_rustdoc_json()).unwrap();
        let valid_paths = RustdocLaunchSpy::using_snapshot(valid);
        assert!(matches!(
            snapshot_status_and_content(&valid_paths, "domain"),
            (LiveRustdocSnapshotStatus::Verified(_), Some(_))
        ));

        let unreadable = directory.path().join("unreadable.json");
        fs::create_dir(&unreadable).unwrap();
        let unreadable_paths = RustdocLaunchSpy::using_snapshot(unreadable);
        assert_eq!(
            snapshot_status_and_content(&unreadable_paths, "domain"),
            (LiveRustdocSnapshotStatus::ReadFailed, None),
            "a target path that cannot be read as JSON must never be reused"
        );
    }

    #[test]
    fn test_execute_type_signals_for_layer_indeterminate_closure_launches_rustdoc() {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        let fixture = FreshnessFixture::new();
        fixture.seed_current_artifact();
        fs::remove_file(fixture.workspace.path().join("Cargo.lock")).unwrap();
        let (rustdoc, evaluation) = fixture.evaluation_spies();

        let error = fixture.evaluate_with(&rustdoc, &evaluation).unwrap_err();

        assert!(
            error.to_string().contains("Cargo.lock"),
            "closure error must be retained: {error}"
        );
        assert_eq!(rustdoc.launches(), 1, "an indeterminate closure must launch rustdoc");
        assert_eq!(evaluation.evaluations(), 1, "an indeterminate closure must evaluate signals");
    }

    #[test]
    fn test_execute_type_signals_for_layer_input_change_during_evaluation_refuses_persistence() {
        if !nightly_toolchain_available() {
            eprintln!("skipping freshness lane: nightly toolchain is unavailable");
            return;
        }

        let fixture = FreshnessFixture::new();
        fixture.seed_current_artifact();
        let before = fs::read(&fixture.signal_path).unwrap();
        fs::write(
            &fixture.catalogue_path,
            "{\n  \"schema_version\": 5,\n  \"crate_name\": \"domain\",\n  \"layer\": \"domain\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n\n",
        )
        .unwrap();
        let rustdoc = RustdocLaunchSpy::using_snapshot(fixture.snapshot_path.clone());
        let observer = InputMutationObserver {
            path: fixture.workspace.path().join("Cargo.lock"),
            replacement: "# changed during evaluation\nversion = 4\n\n[[package]]\nname = \"domain\"\nversion = \"0.1.0\"\n".to_owned(),
            mutation_failed: AtomicBool::new(false),
        };

        let result = fixture.evaluate_with(&rustdoc, &observer);

        assert!(
            !observer.mutation_failed.load(Ordering::SeqCst),
            "test input mutation must succeed"
        );
        let error = result.unwrap_err();
        assert!(error.to_string().contains("freshness inputs changed during evaluation"));
        assert_eq!(rustdoc.launches(), 0, "the verified snapshot remains reusable for evaluation");
        assert_eq!(
            fs::read(&fixture.signal_path).unwrap(),
            before,
            "input drift must leave the prior signal artifact intact"
        );
    }

    #[test]
    fn test_execute_type_signals_for_layer_unchanged_verified_inputs_skips_evaluation() {
        let recorded = recorded_type_signals_document();
        let current = current_inputs_for(&recorded);
        let decision = reuse_decision_for_recorded_document(
            Some(&recorded),
            &current,
            LiveRustdocSnapshotStatus::Verified(
                recorded.freshness().live_rustdoc_snapshot_hash().clone(),
            ),
        );

        assert_eq!(decision, TypeSignalsReuseDecision::SkipEvaluation);
    }

    #[test]
    fn test_execute_type_signals_for_layer_catalogue_or_baseline_change_reevaluates_snapshot() {
        let recorded = recorded_type_signals_document();
        let original = current_inputs_for(&recorded);
        let changed_catalogue = TypeSignalsCurrentInputs::new(
            type_signals_codec::declaration_hash(b"changed catalogue"),
            original.implementation_input_hash().clone(),
            original.baseline_hash().clone(),
            original.evaluator_contract_hash().clone(),
            original.rustdoc_extraction_contract_hash().clone(),
        );
        let changed_baseline = TypeSignalsCurrentInputs::new(
            original.declaration_hash().clone(),
            original.implementation_input_hash().clone(),
            BaselineHash::new(freshness_digest(FRESHNESS_B)),
            original.evaluator_contract_hash().clone(),
            original.rustdoc_extraction_contract_hash().clone(),
        );
        for current in [changed_catalogue, changed_baseline] {
            assert_eq!(
                reuse_decision_for_recorded_document(
                    Some(&recorded),
                    &current,
                    LiveRustdocSnapshotStatus::Verified(
                        recorded.freshness().live_rustdoc_snapshot_hash().clone(),
                    ),
                ),
                TypeSignalsReuseDecision::ReevaluateWithSnapshot
            );
        }
    }

    #[test]
    fn test_execute_type_signals_for_layer_unverifiable_snapshot_or_input_change_reextracts() {
        let recorded = recorded_type_signals_document();
        let current = current_inputs_for(&recorded);
        for status in [
            LiveRustdocSnapshotStatus::Missing,
            LiveRustdocSnapshotStatus::ParseFailed,
            LiveRustdocSnapshotStatus::HashMismatch,
        ] {
            assert_eq!(
                reuse_decision_for_recorded_document(Some(&recorded), &current, status),
                TypeSignalsReuseDecision::ReextractAndEvaluate
            );
        }
        let changed_input = TypeSignalsCurrentInputs::new(
            current.declaration_hash().clone(),
            ImplementationInputHash::new(freshness_digest(FRESHNESS_B)),
            current.baseline_hash().clone(),
            current.evaluator_contract_hash().clone(),
            current.rustdoc_extraction_contract_hash().clone(),
        );
        assert_eq!(
            reuse_decision_for_recorded_document(
                Some(&recorded),
                &changed_input,
                LiveRustdocSnapshotStatus::Verified(
                    recorded.freshness().live_rustdoc_snapshot_hash().clone(),
                ),
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
    }
}
