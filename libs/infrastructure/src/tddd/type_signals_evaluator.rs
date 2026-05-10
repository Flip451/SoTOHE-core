//! `execute_type_signals_for_layer` — wires pre-commit type-signal recomputation
//! to `SignalEvaluatorV2` (three-way diff evaluator).
//!
//! T009: the old evaluator pipeline using `TypeGraph` + `TypeBaseline` is gone.
//! This module re-implements `execute_type_signals_for_layer` by delegating to
//! the new `SignalEvaluatorV2` (three-way diff: catalogue A, baseline B, live C).
//!
//! The output written to `<layer>-type-signals.json` follows the existing
//! schema_version 1 format so that the merge-gate reader (`type_signals_codec`)
//! and the pre-commit classifier in `make.rs` continue to work without changes.
//!
//! `EvaluateSignalsError` is kept as the public error type so that the CLI
//! composition root (`signals.rs`) does not need to be changed.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::ExitCode;

use domain::tddd::catalogue_v2::composite::{CompositePattern, TypeKindV2};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};
use domain::tddd::type_signals_doc::TypeSignalsDocument;
use domain::{ConfidenceSignal, Timestamp, TypeSignal};

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
    track_id: &str,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
) -> Result<ExitCode, EvaluateSignalsError> {
    // Security: validate track_id using the domain newtype. `TrackId::try_new`
    // enforces the lowercase slug format (no uppercase, no spaces, no underscores,
    // no leading/trailing hyphens, no path-traversal components), which is
    // strictly stronger than the Component::Normal check alone.
    let valid_track_id = domain::TrackId::try_new(track_id)
        .map_err(|e| EvaluateSignalsError(format!("invalid track_id '{track_id}': {e}")))?;

    let track_dir = items_dir.join(valid_track_id.as_ref());

    // Security: reject symlinked items_dir root before using it as a trusted anchor.
    // Following a symlinked root would allow reading/writing outside the intended workspace.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard: refusing to use symlinked items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard: cannot stat items_dir '{}': {e}",
                items_dir.display()
            )));
        }
    }

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
        // Provide a specific actionable message for schema_version mismatches so
        // that tracks still using a v2 catalogue get a clear migration prompt
        // rather than a generic decode failure.
        if let CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual, .. } = &e {
            return EvaluateSignalsError(format!(
                "catalogue '{}' uses schema_version {actual} — \
                 SignalEvaluatorV2 requires a v3 catalogue (schema_version=3). \
                 Migrate the catalogue using the type-designer agent before running \
                 `sotp track type-signals`.",
                catalogue_path.display()
            ));
        }
        EvaluateSignalsError(format!(
            "failed to decode catalogue '{}': {e}",
            catalogue_path.display()
        ))
    })?;

    // Build item_name → kind_tag(s) map from the catalogue before `doc` is
    // consumed by `CatalogueToExtendedCrateCodec::encode`.  The signal converter
    // uses this map so that each `TypeSignal.kind_tag` matches the v2-compat
    // kind tag used by `check_type_signals`.
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
        for (name, entry) in &doc.types {
            m.entry(name.as_str().to_owned())
                .or_default()
                .push(data_role_kind_tag_v2(entry.role, &entry.kind));
        }
        for (name, entry) in &doc.traits {
            m.entry(name.as_str().to_owned()).or_default().push(contract_role_kind_tag(entry.role));
        }
        for (path, entry) in &doc.functions {
            // Cross-crate functions (path.crate_name != doc.crate_name, ADR 2 D5) are
            // encoded into the ExtendedCrate with `crate_id != 0`, and `build_function_identity_map`
            // skips non-local items (`crate_id != 0`).  They therefore never receive a
            // `ThreeWaySignal` from the evaluator.  If we added them to `kind_tag_map`, the
            // synthetic Blue loop would mark them Blue without any evaluation — a false pass.
            // Exclude them here so `check_type_signals` is not asked to cover them.
            if path.crate_name != doc.crate_name {
                continue;
            }
            // FunctionPath keys are fully qualified (e.g. "crate::fn_name") and
            // never collide with short-name type/trait keys.
            m.entry(path.to_string()).or_default().push(function_role_kind_tag(entry.role));
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
    let baseline_path = track_dir.join(binding.baseline_file());
    // Security: individual file-level symlink guard for the baseline file.
    match reject_symlinks_below(&baseline_path, &canonical_items) {
        Ok(true) | Ok(false) => {}
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard rejected baseline '{}': {e}",
                baseline_path.display()
            )));
        }
    }
    if !baseline_path.is_file() {
        return Err(EvaluateSignalsError(format!(
            "baseline file not found: {} — run `sotp track baseline-capture {}` first \
             (rustdoc format; delete old TypeBaseline JSON if present and re-capture)",
            baseline_path.display(),
            track_id,
        )));
    }
    let baseline_b = BaselineRustdocCodec::load(&baseline_path).map_err(|e| {
        EvaluateSignalsError(format!("failed to load baseline '{}': {e}", baseline_path.display()))
    })?;

    // --- Step 4: Capture current TypeGraph (C) via rustdoc ---
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

    // Security: reject symlinked workspace_root before invoking rustdoc.
    // A symlinked workspace root could redirect the build to an arbitrary
    // directory outside the trusted workspace tree.
    match workspace_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard: refusing to use symlinked workspace_root: {}",
                workspace_root.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard: cannot stat workspace_root '{}': {e}",
                workspace_root.display()
            )));
        }
    }

    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let json_path = exporter.export_rustdoc_json_path(target_crate).map_err(|e| {
        EvaluateSignalsError(format!(
            "rustdoc export failed for crate '{target_crate}' (layer '{}'): {e}",
            binding.layer_id()
        ))
    })?;
    let json_content = std::fs::read_to_string(&json_path).map_err(|e| {
        EvaluateSignalsError(format!("failed to read rustdoc JSON '{}': {e}", json_path.display()))
    })?;
    let current_c = BaselineRustdocCodec::from_json(&json_content).map_err(|e| {
        EvaluateSignalsError(format!(
            "failed to parse rustdoc JSON for crate '{target_crate}': {e}"
        ))
    })?;

    // --- Step 5: Evaluate ---
    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(extended_a, baseline_b, current_c).map_err(|e| {
        EvaluateSignalsError(format!(
            "signal evaluation error for layer '{}': {e:?}",
            binding.layer_id()
        ))
    })?;

    // --- Step 6: Convert ThreeWayEvaluationReport → TypeSignalsDocument ---
    let signals: Vec<TypeSignal> = build_type_signals_from_report(report.iter(), &kind_tag_map);

    // --- Step 7: Compute declaration_hash from catalogue file bytes ---
    let declaration_hash = type_signals_codec::declaration_hash(catalogue_bytes.as_slice());

    // --- Build the generated_at timestamp (UTC, Z suffix required by codec) ---
    let now_str = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let generated_at = Timestamp::new(&now_str).map_err(|e| {
        EvaluateSignalsError(format!("failed to construct generated_at timestamp: {e}"))
    })?;

    let doc = TypeSignalsDocument::new(generated_at, declaration_hash, signals);

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

/// Converts a `ThreeWayEvaluationReport` iterator into a `Vec<TypeSignal>`.
///
/// ## Two-pass aggregation
///
/// Signals whose `item_name` contains `": "` are *impl-level* signals
/// (key format: `"TypeName: TraitPath"`).  These are not present in the
/// `kind_tag_map` (which only holds catalogue top-level type, trait, and
/// function short names), so emitting them as standalone `TypeSignal` entries
/// would produce `kind_tag = "unknown"` entries that the `check_type_signals`
/// gate ignores — allowing incomplete `trait_impls` declarations to silently
/// pass the merge gate.
///
/// Instead, impl signals are *aggregated* onto the owning type's `TypeSignal`:
///
/// - **Blue** impl → added to `found_items` (impl achieved; no signal downgrade).
/// - **Yellow** impl → added to `missing_items`; owning type signal downgraded to
///   at most Yellow.
/// - **Red** impl → added to `missing_items` or `extra_items` (see below); owning
///   type signal downgraded to Red.
///
/// For `CMinusSUnionD` (undeclared impl found in C) the trait is added to
/// `extra_items`.  For all other regions it is added to `missing_items`.
///
/// When the owning type has no top-level signal in the report (e.g. it falls in
/// the `SIntersectC_Match_Reference` skip bucket), a synthetic `TypeSignal` is
/// emitted with `found_type = true`, `kind_tag` from `kind_tag_map` (falling
/// back to `"unknown"`), and the impl-derived signal level.
/// Intermediate accumulator entry for a single top-level item.
///
/// Fields: `(signal, found_type, found_items, missing_items, extra_items)`.
type AccEntry = (ConfidenceSignal, bool, Vec<String>, Vec<String>, Vec<String>);

fn build_type_signals_from_report<'a>(
    signals: impl Iterator<Item = &'a ThreeWaySignal>,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
) -> Vec<TypeSignal> {
    use domain::tddd::signal_evaluator::region::SignalRegion;

    // Intermediate accumulator per top-level item name.
    // (signal, found_type, found_items, missing_items, extra_items)
    //
    // Keyed by name only (not (name, kind_tag)) because the evaluator operates
    // at L1 (short-name) resolution and never emits two signals for the same
    // name — even when the catalogue declares both a type and a trait with the
    // same short name.  Multiple kind_tags per name are handled at the final
    // build step (below), where one TypeSignal is emitted per kind_tag in
    // `kind_tag_map[name]`.
    let mut acc: HashMap<String, AccEntry> = HashMap::new();
    // Preserve insertion order so the output is deterministic.
    let mut order: Vec<String> = Vec::new();

    for signal in signals {
        let name = signal.item_name();
        let confidence = signal_kind_to_confidence(signal.signal());
        let found_in_c = !matches!(
            signal.region(),
            SignalRegion::SMinusC_Add
                | SignalRegion::SMinusC_Modify
                | SignalRegion::SMinusC_Reference
                | SignalRegion::DMinusC
        );

        if let Some(sep) = name.find(": ") {
            // --- Impl-level signal: aggregate onto the owning type ---
            let owner = &name[..sep];
            let trait_part = &name[sep + 2..];

            let entry = acc.entry(owner.to_owned()).or_insert_with(|| {
                order.push(owner.to_owned());
                // Owning type was in the skip bucket (Match_Reference), so its
                // signal is not in the report.  Synthesise a Blue entry with
                // found_type = true (it was present in C, otherwise this impl
                // could not have been evaluated in the S ∩ C path).
                // kind_tag is resolved later in the final build pass using
                // `kind_tag_map`.
                (ConfidenceSignal::Blue, true, Vec::new(), Vec::new(), Vec::new())
            });

            // DMinusC (delete achieved) is a Blue impl signal: no downgrade needed
            // and the deleted impl does not appear in any sub-item list — the
            // deletion is simply a completed action.
            if signal.region() == SignalRegion::DMinusC {
                // Nothing to do: owning type is not downgraded; trait not listed.
            } else {
                // Downgrade the owning type's signal if this impl is worse.
                entry.0 = worse_signal(entry.0, confidence);

                // Classify the trait into found/missing/extra.
                match signal.region() {
                    // Blue: impl achieved (present in C and matches) — found_items.
                    SignalRegion::SIntersectC_Match_Add
                    | SignalRegion::SIntersectC_Match_Modify => {
                        entry.2.push(trait_part.to_owned());
                    }
                    // CMinusSUnionD: undeclared impl found in C — extra_items.
                    SignalRegion::CMinusSUnionD => {
                        entry.4.push(trait_part.to_owned());
                    }
                    // All other non-Blue regions: impl not satisfied — missing_items.
                    _ => {
                        entry.3.push(trait_part.to_owned());
                    }
                }
            }
        } else {
            // --- Top-level signal (type / trait / function) ---
            let entry = acc.entry(name.to_owned()).or_insert_with(|| {
                order.push(name.to_owned());
                (confidence, found_in_c, Vec::new(), Vec::new(), Vec::new())
            });
            // In practice a top-level name appears at most once in the report;
            // if it does appear twice (shouldn't happen), keep the worse signal.
            entry.0 = worse_signal(entry.0, confidence);
            // found_type: true if present in C in either occurrence.
            entry.1 = entry.1 || found_in_c;
        }
    }

    // Synthesize Blue entries for catalogue items absent from the report.
    //
    // `ThreeWayEvaluationReport::new` filters out `SIntersectC_Match_Reference`
    // (Skip) entries to reduce noise: a type with `action = Reference` that fully
    // matches the current code produces no `ThreeWaySignal` in the report.
    // However, `check_type_signals` requires a signal entry for every catalogue
    // item.  Without a signal, it reports "no signal evaluation" and blocks the
    // merge gate.
    //
    // For every catalogue item (key in `kind_tag_map`) that has no entry in `acc`
    // after processing the report, insert a synthetic Blue row with `found_type =
    // true` and empty sub-item lists.  This correctly represents a maintained
    // reference item (present in C, no declared change, structural match).
    //
    // Because `kind_tag_map` is a `BTreeMap`, iteration over its keys is sorted
    // (deterministic), so the order in which synthetic entries are appended to
    // `order` is stable across runs.
    for name in kind_tag_map.keys() {
        // `entry().or_insert_with` avoids overwriting existing entries from the
        // report (which may be Yellow/Red and must not be silenced).
        acc.entry(name.clone()).or_insert_with(|| {
            order.push(name.clone());
            (ConfidenceSignal::Blue, true, Vec::new(), Vec::new(), Vec::new())
        });
    }

    // Build the final Vec<TypeSignal> in insertion order.
    //
    // When a catalogue declares both a type and a trait with the same short
    // name, `kind_tag_map[name]` contains two entries (e.g.
    // `["value_object", "secondary_port"]`).  `check_type_signals` keys
    // coverage by `(type_name, kind_tag)`, so both must appear in the output.
    //
    // ## Signal assignment for collision names
    //
    // The evaluator operates at L1 (short-name resolution): when two items
    // share a short name, `build_type_trait_identity_map` keeps only one of
    // them (lexicographically-smallest full path).  Only that one item is
    // actually evaluated.  The other item is not independently checked.
    //
    // The winning item is non-deterministic when a type and a trait share the
    // same short name at the same module path (identical full paths).  Because
    // `kind_tag_map` always pushes types before traits, kind_tags[0] is always
    // a type's kind_tag, but the evaluator may have evaluated the trait.
    // Assigning the evaluated signal only to kind_tags[0] would give Blue to
    // the wrong declaration, causing false gate failures.
    //
    // Therefore, for ALL collision names (len > 1), every kind_tag entry is
    // degraded to at most Yellow:
    //
    // - `worse_signal(sig, Yellow)` = Yellow when `sig` == Blue (degrades to
    //   "ambiguous — cannot attribute signal to a specific declaration").
    // - `worse_signal(sig, Yellow)` = Yellow when `sig` == Yellow (no change).
    // - `worse_signal(sig, Yellow)` = Red when `sig` == Red (failure
    //   propagates — at least one item named `Foo` is known-broken).
    //
    // In the common case (no collision: kind_tags.len() == 1), the signal is
    // emitted unchanged, so there is no behaviour change for non-collision names.
    order
        .into_iter()
        .flat_map(|name| {
            let Some((sig, found_type, found_items, missing_items, extra_items)) =
                acc.remove(&name)
            else {
                return Vec::new();
            };
            let kind_tags = kind_tag_map.get(name.as_str()).map(Vec::as_slice).unwrap_or(&[]);
            if kind_tags.is_empty() {
                // Name came from the report but not from the catalogue (e.g. an
                // impl-level owner that was already in the skip bucket).  Emit a
                // single entry with kind_tag "unknown" so the signal is not lost.
                return vec![TypeSignal::new(
                    name,
                    "unknown",
                    sig,
                    found_type,
                    found_items,
                    missing_items,
                    extra_items,
                )];
            }
            // For collision names (len > 1), degrade ALL entries to at most Yellow.
            // `build_type_trait_identity_map` collapses the collision to one evaluated
            // signal but the winning item is non-deterministic (lexicographic full-path
            // ordering means a type and a trait at the same path can swap).  Assigning
            // the evaluated signal only to kind_tags[0] (always the type entry, because
            // types are pushed first into kind_tag_map) would give Blue to the wrong
            // declaration when the trait won the evaluation, causing false gate failures.
            // Degrading every collision entry to at most Yellow is the conservative
            // correct behaviour: the ambiguity is surfaced as Yellow rather than silently
            // mis-attributed as Blue.
            let is_collision = kind_tags.len() > 1;
            kind_tags
                .iter()
                .map(|&kt| {
                    let effective_sig = if is_collision {
                        worse_signal(sig, ConfidenceSignal::Yellow)
                    } else {
                        sig
                    };
                    TypeSignal::new(
                        name.clone(),
                        kt,
                        effective_sig,
                        found_type,
                        found_items.clone(),
                        missing_items.clone(),
                        extra_items.clone(),
                    )
                })
                .collect()
        })
        .collect()
}

/// Maps a `ThreeWaySignalKind` to `ConfidenceSignal`.
///
/// `Skip` should be filtered by `ThreeWayEvaluationReport::new` before reaching
/// here; it is mapped to `Yellow` as a safe fallback.
fn signal_kind_to_confidence(kind: ThreeWaySignalKind) -> ConfidenceSignal {
    match kind {
        ThreeWaySignalKind::Blue => ConfidenceSignal::Blue,
        ThreeWaySignalKind::Yellow => ConfidenceSignal::Yellow,
        ThreeWaySignalKind::Red => ConfidenceSignal::Red,
        ThreeWaySignalKind::Skip => ConfidenceSignal::Yellow,
    }
}

/// Returns the worse of two `ConfidenceSignal`s (Red > Yellow > Blue).
fn worse_signal(a: ConfidenceSignal, b: ConfidenceSignal) -> ConfidenceSignal {
    match (a, b) {
        (ConfidenceSignal::Red, _) | (_, ConfidenceSignal::Red) => ConfidenceSignal::Red,
        (ConfidenceSignal::Yellow, _) | (_, ConfidenceSignal::Yellow) => ConfidenceSignal::Yellow,
        _ => ConfidenceSignal::Blue,
    }
}

// ---------------------------------------------------------------------------
// Role → kind_tag mapping helpers
// ---------------------------------------------------------------------------

/// Maps a v3 `DataRole` + `TypeKindV2` pair to the v2-compat kind_tag string
/// used by `TypeSignal` and expected by `check_type_signals`.
///
/// `check_type_signals` evaluates coverage against a `TypeCatalogueDocument`
/// produced by `v3_doc_to_stub` (in `catalogue_bulk_loader`), which maps v3
/// roles to v2 `TypeDefinitionKind` variants. The kind_tag string written in
/// `<layer>-type-signals.json` must match what `v3_doc_to_stub` would produce
/// for the same entry, or the coverage check will flag every v3 entry as
/// uncovered.
///
/// The mapping mirrors `data_role_to_kind` in `catalogue_bulk_loader.rs`:
/// - `TypeKindV2::Struct { pattern: TypestateState, .. }` → `"typestate"`
/// - `TypeKindV2::Enum { .. }` + `DataRole::ErrorType` → `"error_type"`
/// - `TypeKindV2::Enum { .. }` otherwise → `"enum"`
/// - Struct/TypeAlias: `Entity | AggregateRoot | Specification | ValueObject`
///   → `"value_object"` (v3 does not distinguish at the v2 shape level)
/// - All other struct/alias roles → their canonical v2 snake_case name.
fn data_role_kind_tag_v2(role: DataRole, kind: &TypeKindV2) -> &'static str {
    // Typestate detection: Struct with TypestateState pattern.
    if let TypeKindV2::Struct { pattern: CompositePattern::TypestateState { .. }, .. } = kind {
        return "typestate";
    }
    // Enum kind: role determines error_type vs enum.
    if matches!(kind, TypeKindV2::Enum { .. }) {
        return if matches!(role, DataRole::ErrorType) { "error_type" } else { "enum" };
    }
    // Struct / TypeAlias: role-based mapping (mirrors `data_role_to_kind`).
    match role {
        DataRole::ValueObject
        | DataRole::Entity
        | DataRole::AggregateRoot
        | DataRole::Specification => "value_object",
        DataRole::DomainService => "domain_service",
        DataRole::Factory => "factory",
        DataRole::UseCase => "use_case",
        DataRole::Interactor => "interactor",
        DataRole::Command => "command",
        DataRole::Query => "query",
        DataRole::Dto => "dto",
        DataRole::ErrorType => "error_type",
        DataRole::SecondaryAdapter => "secondary_adapter",
    }
}

/// Maps a `ContractRole` (v3 catalogue trait entry role) to the v2-compat kind_tag
/// string used in `TypeSignal` and `check_type_signals`.
///
/// The mapping mirrors `contract_role_to_kind` in `catalogue_bulk_loader.rs`:
/// - `SecondaryPort | SpecificationPort` → `TypeDefinitionKind::SecondaryPort`
///   → kind_tag `"secondary_port"` (v3 has no `SpecificationPort` v2 equivalent;
///   both collapse to `SecondaryPort` at the v2 shape level).
/// - `ApplicationService` → `TypeDefinitionKind::ApplicationService`
///   → kind_tag `"application_service"`.
const fn contract_role_kind_tag(role: ContractRole) -> &'static str {
    match role {
        // Both SpecificationPort and SecondaryPort collapse to SecondaryPort
        // at the v2 shape level (mirrors `contract_role_to_kind` in
        // `catalogue_bulk_loader.rs`). Using "secondary_port" here ensures
        // `check_type_signals` coverage comparison succeeds for both roles.
        ContractRole::SpecificationPort | ContractRole::SecondaryPort => "secondary_port",
        ContractRole::ApplicationService => "application_service",
    }
}

/// Maps a `FunctionRole` (v3 catalogue function entry role) to the v2-compat kind_tag
/// string used in `TypeSignal` and `check_type_signals`.
///
/// The mapping mirrors `function_role_to_kind` in `catalogue_bulk_loader.rs`:
/// all v3 `FunctionRole` variants collapse to `TypeDefinitionKind::FreeFunction`
/// → kind_tag `"free_function"`. Using the v3 semantic name ("use_case_function")
/// would produce a coverage mismatch against the v2 stub.
const fn function_role_kind_tag(_role: FunctionRole) -> &'static str {
    // All v3 FunctionRole variants collapse to FreeFunction at the v2 shape level
    // (mirrors `function_role_to_kind` in `catalogue_bulk_loader.rs`).
    "free_function"
}
