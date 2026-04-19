//! `sotp track type-signals` — evaluate type signals via rustdoc schema export.
//!
//! Reads `<layer>-types.json` from the track directory, exports the target crate's
//! public API via rustdoc JSON, evaluates signals for each declared type, and writes
//! the updated document back to `<layer>-types.json`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use domain::TrackStatus;
use domain::TypeSignalsDocument;
use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::{catalogue_codec, type_signals_codec};
use infrastructure::timestamp_now;
use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::track::fs_store::read_track_metadata;
use infrastructure::track::symlink_guard::reject_symlinks_below;
use infrastructure::verify::tddd_layers::{
    LoadTdddLayersError, TdddLayerBinding, load_tddd_layers_from_path,
};

use crate::CliError;

/// Resolves the set of TDDD-enabled layers for this invocation.
///
/// - Reads `architecture-rules.json` from `workspace_root`.
/// - When `layer_filter` is `None`, returns every `tddd.enabled` layer in
///   `layers[]` order.
/// - When `layer_filter` is `Some(id)`, returns only the matching enabled
///   binding. An unknown or disabled layer id is fail-closed.
/// - When `architecture-rules.json` is absent, falls back to a single
///   synthetic `domain` binding so legacy tracks continue to work.
pub(crate) fn resolve_layers(
    workspace_root: &Path,
    layer_filter: Option<&str>,
) -> Result<Vec<TdddLayerBinding>, CliError> {
    let rules_path = workspace_root.join("architecture-rules.json");
    // Delegate symlink handling + legacy-fallback policy to the shared
    // infrastructure helper. CLI stays a thin composition layer; it only
    // maps the infra error variants into `CliError` and applies the
    // CLI-level layer filter.
    let bindings =
        load_tddd_layers_from_path(&rules_path, workspace_root).map_err(|e| match e {
            LoadTdddLayersError::Io { path, source } => {
                CliError::Message(format!("{}: {source}", path.display()))
            }
            LoadTdddLayersError::Parse(err) => {
                CliError::Message(format!("{}: {err}", rules_path.display()))
            }
        })?;

    if let Some(filter) = layer_filter {
        let Some(binding) = bindings.iter().find(|b| b.layer_id() == filter) else {
            return Err(CliError::Message(format!(
                "layer '{filter}' is not tddd.enabled in architecture-rules.json"
            )));
        };
        Ok(vec![binding.clone()])
    } else {
        Ok(bindings)
    }
}

/// Fail-closed active-track guard: rejects `Done` / `Archived` tracks.
///
/// Uses an exhaustive `match` on `TrackStatus` (6 variants) — adding a new variant
/// triggers a compile error, forcing explicit classification (frozen vs active).
/// This is the structural guarantee for fail-closed behavior; `matches!` would
/// silently pass new variants through (fail-open).
///
/// Mirrors the intent of the existing `is_done_or_archived` guard in
/// `libs/infrastructure/src/track/render.rs::sync_rendered_views` (which uses a
/// string-based `matches!` and is scope-deferred per ADR
/// `knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md` Consequences Neutral).
pub(crate) fn ensure_active_track(status: TrackStatus, track_id: &str) -> Result<(), CliError> {
    match status {
        TrackStatus::Done | TrackStatus::Archived => Err(CliError::Message(format!(
            "cannot run type-signals on '{track_id}' (status={status}). \
             Completed tracks are frozen — run on an active track instead.",
        ))),
        TrackStatus::Planned
        | TrackStatus::InProgress
        | TrackStatus::Blocked
        | TrackStatus::Cancelled => Ok(()),
    }
}

/// Evaluate type signals via rustdoc schema export and write back to `<layer>-types.json`.
///
/// Steps:
/// 1. Resolve the set of TDDD-enabled layers to process (all enabled, or just the
///    specified `--layer`).
/// 2. For each layer binding, read its catalogue file, export the target crate's
///    public API using `RustdocSchemaExporter`, evaluate signals, and write back.
/// 3. Print a signal summary per layer to stdout.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, the file cannot be read or
/// decoded, rustdoc export fails (e.g., nightly not installed), or the write fails.
pub fn execute_type_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id to prevent path traversal.
    let valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Active-track guard: reject type-signals on completed/archived tracks.
    // `metadata.status()` is derived from task states and can never return
    // `TrackStatus::Archived` (archived is a workflow-level state stored only in
    // `DocumentMeta::original_status`). Check `original_status` first so that an
    // archived track with unresolved tasks is not misclassified as `InProgress` and
    // allowed through.
    let (metadata, doc_meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;
    let effective_status = if doc_meta.original_status.as_deref() == Some("archived") {
        domain::TrackStatus::Archived
    } else {
        metadata.status()
    };
    ensure_active_track(effective_status, &track_id)?;

    // Resolve the set of TDDD-enabled layers to process. When
    // `architecture-rules.json` is absent we fall back to the legacy
    // single-`domain` binding so older tracks keep working. When `--layer`
    // is supplied we fail-closed on an unknown or disabled layer id.
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;

    // Fail-closed when no layers are enabled: returning SUCCESS with no
    // work done would silently mask a misconfigured `architecture-rules.json`
    // (e.g. all layers have `tddd.enabled = false`).
    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; \
             nothing to evaluate"
                .to_owned(),
        ));
    }

    for binding in &bindings {
        execute_type_signals_for_layer(&items_dir, &track_id, &workspace_root, binding)?;
    }

    Ok(ExitCode::SUCCESS)
}

/// Pre-commit-flavored variant of [`execute_type_signals`] that treats a
/// missing per-layer catalogue file as "layer not yet initialized for this
/// track" and skips it silently, matching the CI
/// (`spec_states::evaluate_layer_catalogue`) and merge-gate semantics. This
/// resolves the asymmetry where the user-invoked `sotp track type-signals`
/// hard-fails on a missing catalogue (correct UX — the user explicitly asked
/// to evaluate), but the automated pre-commit hook must behave like the
/// verification gates (skip inactive layers, pass).
///
/// Same guards as `execute_type_signals`: active-track guard, `architecture-rules.json`
/// fail-closed via `resolve_layers`, empty-bindings fail-closed. Only the
/// per-layer NotFound handling differs.
///
/// # Errors
///
/// Returns `CliError` on the same paths as `execute_type_signals` EXCEPT the
/// per-layer catalogue NotFound, which is silently skipped here.
pub fn execute_type_signals_lenient(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;
    let (metadata, doc_meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;
    let effective_status = if doc_meta.original_status.as_deref() == Some("archived") {
        domain::TrackStatus::Archived
    } else {
        metadata.status()
    };
    ensure_active_track(effective_status, &track_id)?;

    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;
    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json; nothing to evaluate"
                .to_owned(),
        ));
    }

    let track_dir = items_dir.join(&track_id);
    for binding in &bindings {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        // Use `symlink_metadata` + explicit NotFound match so only a truly
        // absent declaration file is treated as "layer inactive". Symlinks,
        // directories, permission errors, and other `std::fs` failures
        // propagate as errors — matching the CI
        // (`evaluate_layer_catalogue`) fail-closed posture and preventing
        // the "pre-commit passes, verification fails later" divergence.
        match std::fs::symlink_metadata(&catalogue_path) {
            Ok(meta) if meta.file_type().is_file() => {
                // Skip recompute when the companion signal file is already
                // current for this layer. This matters for layers whose
                // `schema_export.targets` has more than one entry — the
                // strict `execute_type_signals_for_layer` hard-fails on
                // multi-target configs, but the merge gate / CI paths
                // already validate the signal file's `declaration_hash`
                // directly, so no recompute is needed when the signals
                // are already fresh.
                if signal_file_is_current(&track_dir, binding, &catalogue_path) {
                    continue;
                }
                execute_type_signals_for_layer(&items_dir, &track_id, &workspace_root, binding)?;
            }
            Ok(_) => {
                // Regular-file check failed: symlink, directory, block
                // device, etc. Delegate to the strict evaluator so the
                // caller sees the same error as the CI / merge-gate path
                // (`reject_symlinks_below` / non-regular-file rejection).
                execute_type_signals_for_layer(&items_dir, &track_id, &workspace_root, binding)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Declaration file is genuinely absent — layer not
                // TDDD-active for this track. Skip silently (symmetric with
                // `spec_states::evaluate_layer_catalogue` NotFound branch).
            }
            Err(e) => {
                return Err(CliError::Message(format!(
                    "pre-commit: cannot stat {}: {e}",
                    catalogue_path.display()
                )));
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Returns `true` when the companion `<layer>-type-signals.json` exists as a
/// regular file and its `declaration_hash` matches the current declaration
/// file bytes — i.e. the evaluation result is already fresh and no
/// `execute_type_signals_for_layer` recompute is needed.
///
/// Returns `false` on any missing / stale / unreadable / symlinked / decode
/// failure so the caller falls through to the strict recompute path (which
/// will emit a helpful diagnostic).
///
/// Used by [`execute_type_signals_lenient`] to avoid unnecessary recomputes
/// on the pre-commit path — especially for layers with multi-target
/// `schema_export.targets`, where the strict evaluator hard-fails but the
/// signal file may already be current from a prior run.
fn signal_file_is_current(
    track_dir: &std::path::Path,
    binding: &TdddLayerBinding,
    catalogue_path: &std::path::Path,
) -> bool {
    let signal_path = track_dir.join(binding.signal_file());
    let signal_is_file =
        signal_path.symlink_metadata().map(|m| m.file_type().is_file()).unwrap_or(false);
    if !signal_is_file {
        return false;
    }
    let Ok(signal_text) = std::fs::read_to_string(&signal_path) else {
        return false;
    };
    let Ok(signals_doc) = type_signals_codec::decode(&signal_text) else {
        return false;
    };
    let Ok(decl_bytes) = std::fs::read(catalogue_path) else {
        return false;
    };
    let current_hash = type_signals_codec::declaration_hash(&decl_bytes);
    signals_doc.declaration_hash() == current_hash
}

/// Evaluate type signals for a single TDDD layer binding and write back to the
/// configured catalogue file.
///
/// `binding` provides the configured catalogue / baseline filenames and the
/// `schema_export.targets` crate name so that explicit `tddd.catalogue_file`
/// and `tddd.schema_export.targets` overrides are honored consistently.
fn execute_type_signals_for_layer(
    items_dir: &std::path::Path,
    track_id: &str,
    workspace_root: &std::path::Path,
    binding: &TdddLayerBinding,
) -> Result<ExitCode, CliError> {
    let layer_id = binding.layer_id();
    let track_dir = items_dir.join(track_id);
    let catalogue_file = binding.catalogue_file();
    let catalogue_path = track_dir.join(catalogue_file);

    // Read and decode the configured catalogue file.
    // If not found, instruct the user to run /track:design first (TDDD requirement).
    let json = std::fs::read_to_string(&catalogue_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CliError::Message(format!(
                "{catalogue_file} not found for track '{track_id}'. \
                 Run /track:design first to create it (TDDD: type definitions must be written before implementation)."
            ))
        } else {
            CliError::Message(format!("cannot read {}: {e}", catalogue_path.display()))
        }
    })?;

    let mut doc = catalogue_codec::decode(&json)
        .map_err(|e| CliError::Message(format!("{catalogue_file} decode error: {e}")))?;

    // Resolve the target crate for schema export from the binding. Multi-target
    // layers are modeled in `architecture-rules.json` (`schema_export.targets`)
    // but full merge of multiple per-crate schema exports is not yet
    // implemented. Fail-closed when more than one target is configured so that
    // the caller is not silently given baseline/signal data computed from only
    // the first crate — that would drop types/traits from the remaining crates
    // and produce false undeclared/Red results on later signal evaluation.
    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(CliError::Message(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(CliError::Message(format!(
                "layer '{layer_id}' has {} schema_export.targets ({:?}), but multi-target export is not yet implemented. Use a single-target layer or wait for multi-target merge support.",
                multi.len(),
                multi
            )));
        }
    };

    // Export the target crate's public API via rustdoc JSON.
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let schema = exporter.export(target_crate).map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        CliError::Message(format!("failed to export schema: {e}{hint}"))
    })?;

    // Collect typestate names for outgoing transition filtering in build_type_graph.
    let typestate_names = doc.typestate_names();

    // Build a pre-indexed TypeGraph from the flat schema export.
    let profile = build_type_graph(&schema, &typestate_names);

    // Load baseline for 4-group evaluation. The baseline filename is
    // derived from the binding's catalogue stem (e.g.
    // `domain-types-baseline.json` for the default `domain-types.json`),
    // so an override via `tddd.catalogue_file` is honored automatically.
    let baseline_filename = binding.baseline_file();
    let baseline_path = track_dir.join(&baseline_filename);
    let baseline = match std::fs::read_to_string(&baseline_path) {
        Ok(bl_json) => infrastructure::tddd::baseline_codec::decode(&bl_json)
            .map_err(|e| CliError::Message(format!("baseline decode error: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CliError::Message(format!(
                "{baseline_filename} not found for track '{track_id}'. \
                 Run `sotp track baseline-capture {track_id} --layer {layer_id}` first."
            )));
        }
        Err(e) => {
            return Err(CliError::Message(format!("cannot read {}: {e}", baseline_path.display())));
        }
    };

    // Delegate to the core evaluator (nightly-free, directly testable).
    let rendered_stem = binding.rendered_file();
    let signal_file_name = binding.signal_file();
    evaluate_and_write_signals(
        &mut doc,
        &profile,
        &baseline,
        &catalogue_path,
        &track_dir,
        &rendered_stem,
        &signal_file_name,
    )
}

/// Core signal evaluation and catalogue write given pre-built domain components.
///
/// Separated from `execute_type_signals` so the abort-before-write ordering
/// and signal assembly can be exercised in unit tests without requiring the nightly
/// toolchain that `RustdocSchemaExporter` depends on.
///
/// # Errors
///
/// Returns `CliError` if action diagnostics fail (delete errors), encoding fails,
/// either declaration-path / signal-path is a symlink (fail-closed per ADR §D7),
/// or any atomic write fails.
pub(crate) fn evaluate_and_write_signals(
    doc: &mut domain::TypeCatalogueDocument,
    profile: &domain::TypeGraph,
    baseline: &domain::TypeBaseline,
    domain_types_path: &std::path::Path,
    track_dir: &std::path::Path,
    rendered_file_stem: &str,
    signal_file_name: &str,
) -> Result<ExitCode, CliError> {
    // Bidirectional consistency check: forward (spec → code) + reverse (code → spec).
    let report = domain::check_consistency(doc.entries(), profile, baseline);

    // Convert undeclared types/traits (group 4) to Red TypeSignals.
    // Capture the count before appending group-3 baseline reds so the summary
    // WARN line only fires for truly new undeclared items (not baseline changes/deletions).
    let undeclared_signals =
        domain::undeclared_to_signals(report.undeclared_types(), report.undeclared_traits());
    let undeclared_count = undeclared_signals.len();
    let mut reverse_signals = undeclared_signals;

    // Baseline Red: structural changes or deletions (group 3).
    // `found_type` is true when the name still exists in the code (structural change),
    // false when the name is absent from the type graph (deletion).
    for name in report.baseline_red_types() {
        let found_type = profile.get_type(name).is_some();
        reverse_signals.push(domain::TypeSignal::new(
            name.clone(),
            "baseline_changed_type",
            domain::ConfidenceSignal::Red,
            found_type,
            vec![],
            vec![],
            vec![],
        ));
    }
    for name in report.baseline_red_traits() {
        let found_type = profile.get_trait(name).is_some();
        reverse_signals.push(domain::TypeSignal::new(
            name.clone(),
            "baseline_changed_trait",
            domain::ConfidenceSignal::Red,
            found_type,
            vec![],
            vec![],
            vec![],
        ));
    }

    let mut all_signals: Vec<_> = report.forward_signals().to_vec();
    all_signals.extend(reverse_signals);

    doc.set_signals(all_signals.clone());

    // Validate action diagnostics, then write only if validation succeeds.
    // This ordering guarantees no files are mutated on delete-error failure.
    //
    // `validate_and_write_catalogue` returns the declaration-file bytes it just
    // wrote to disk, so `declaration_hash` in the signal file matches the
    // on-disk declaration bytes verbatim (ADR §D5: hash is pinned to post-encode
    // disk bytes, not pre-encode in-memory state).
    let declaration_bytes = validate_and_write_catalogue(
        &report,
        doc,
        domain_types_path,
        track_dir,
        rendered_file_stem,
    )?;

    // T004: dual-write the signal file alongside the declaration file. Signals
    // stay co-located in `<layer>-types.json` during this transitional task
    // (ADR Migration §2); T007 strips them from the declaration codec in the
    // same commit that wires pre-commit auto-recomputation.
    write_signal_file(&all_signals, &declaration_bytes, track_dir, signal_file_name)?;

    let skipped = report.skipped_count();
    // Extract the catalogue filename from the path so the WARN message names the
    // correct file when running against a non-domain layer (e.g. usecase-types.json).
    let catalogue_file =
        domain_types_path.file_name().and_then(|n| n.to_str()).unwrap_or("types.json");
    print_signal_summary(&all_signals, undeclared_count, skipped, catalogue_file);

    Ok(ExitCode::SUCCESS)
}

/// Validate action-baseline contradiction warnings and delete validation errors,
/// then encode and write back the catalogue.
///
/// Calls [`print_action_diagnostics`] first.  If that returns `Err` (delete-error
/// abort), no files are written.  On success the catalogue JSON and the rendered
/// Markdown view are atomically written to disk.
///
/// Extracted so that the validate-before-write ordering can be tested without
/// requiring the nightly toolchain that `execute_type_signals` uses.
///
/// Returns the exact byte vector written to the declaration file (including the
/// trailing newline). Callers computing `declaration_hash` for the signal file
/// MUST use this return value so the hash is pinned to post-encode disk bytes
/// (ADR §D5).
///
/// # Errors
///
/// Returns `CliError` if action diagnostics fail, encoding fails, the declaration
/// file path is a symlink (ADR §D7 fail-closed write-path guard), or any atomic
/// write fails.
fn validate_and_write_catalogue(
    report: &domain::ConsistencyReport,
    doc: &domain::TypeCatalogueDocument,
    domain_types_path: &std::path::Path,
    track_dir: &std::path::Path,
    rendered_file_stem: &str,
) -> Result<Vec<u8>, CliError> {
    // Validate first: abort before any writes if delete errors are present.
    print_action_diagnostics(report)?;

    // Encode and write back.
    let encoded = catalogue_codec::encode(doc)
        .map_err(|e| CliError::Message(format!("catalogue encode error: {e}")))?;

    let declaration_bytes = format!("{encoded}\n").into_bytes();

    // ADR §D7 symlink guard on the write path: reject symlinks at the leaf or
    // at any ancestor below `track_dir`. `reject_symlinks_below` returns
    // `Ok(true)` when the target is a regular file, `Ok(false)` when the
    // target is absent, and `Err(_)` for any symlink or I/O error — both
    // `Ok` variants are safe to write to.
    reject_symlinks_below(domain_types_path, track_dir).map_err(|e| {
        CliError::Message(format!(
            "refusing to write declaration file {}: {e}",
            domain_types_path.display()
        ))
    })?;

    atomic_write_file(domain_types_path, &declaration_bytes).map_err(|e| {
        CliError::Message(format!("cannot write {}: {e}", domain_types_path.display()))
    })?;

    // Re-render the markdown view so it stays in sync with the catalogue.
    // The markdown filename is `<catalogue_stem>.md` where `<catalogue_stem>`
    // is derived from the binding's `catalogue_file`.
    let rendered_md_path = track_dir.join(rendered_file_stem);
    // Derive the catalogue JSON filename from the write path so the rendered view's
    // `Generated from ...` header reflects the actual source file (e.g.
    // `infrastructure-types.json`) — not the rendered `.md` path. `binding` is not
    // in this scope; `domain_types_path` already holds the exact catalogue file path
    // that was just written, so deriving from its file_name is authoritative.
    let catalogue_file =
        domain_types_path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            CliError::Message(format!(
                "cannot derive catalogue file name from {}",
                domain_types_path.display()
            ))
        })?;
    let rendered =
        infrastructure::type_catalogue_render::render_type_catalogue(doc, catalogue_file);
    atomic_write_file(&rendered_md_path, rendered.as_bytes()).map_err(|e| {
        CliError::Message(format!("cannot write {}: {e}", rendered_md_path.display()))
    })?;

    Ok(declaration_bytes)
}

/// Encode and write the per-layer evaluation-result file
/// (`<layer>-type-signals.json`, schema_version 1) alongside the declaration
/// file.
///
/// `declaration_bytes` MUST be the exact byte sequence just written to the
/// declaration file. The SHA-256 hash of these bytes is persisted as
/// `declaration_hash` so that T005's stale detection can compare against the
/// current on-disk declaration file (ADR §D5).
///
/// # Errors
///
/// Returns `CliError` when the system timestamp cannot be derived, the signal
/// file path is a symlink (ADR §D7 fail-closed write-path guard), signal
/// encoding fails, or the atomic write fails.
fn write_signal_file(
    signals: &[domain::TypeSignal],
    declaration_bytes: &[u8],
    track_dir: &std::path::Path,
    signal_file_name: &str,
) -> Result<(), CliError> {
    let generated_at = timestamp_now()
        .map_err(|e| CliError::Message(format!("cannot derive generation timestamp: {e}")))?;
    let declaration_hash = type_signals_codec::declaration_hash(declaration_bytes);
    let signals_doc = TypeSignalsDocument::new(generated_at, declaration_hash, signals.to_vec());

    let encoded = type_signals_codec::encode(&signals_doc)
        .map_err(|e| CliError::Message(format!("signal file encode error: {e}")))?;

    let signal_path = track_dir.join(signal_file_name);

    // ADR §D7 symlink guard on the write path: same policy as the declaration
    // file. `reject_symlinks_below` returns Ok(true | false) for regular files
    // and absent paths; any symlink or unexpected I/O error is Err.
    reject_symlinks_below(&signal_path, track_dir).map_err(|e| {
        CliError::Message(format!("refusing to write signal file {}: {e}", signal_path.display()))
    })?;

    atomic_write_file(&signal_path, format!("{encoded}\n").as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", signal_path.display())))?;

    Ok(())
}

/// Print action-baseline contradiction warnings and delete validation errors.
///
/// Contradictions are printed as `[WARN]` to stderr. Delete errors cause an
/// early return with `CliError`.
fn print_action_diagnostics(report: &domain::ConsistencyReport) -> Result<(), CliError> {
    for contradiction in report.contradictions() {
        eprintln!(
            "[WARN] {} (action={}): {:?}",
            contradiction.name(),
            contradiction.action().action_tag(),
            contradiction.kind(),
        );
    }

    if !report.delete_errors().is_empty() {
        for name in report.delete_errors() {
            eprintln!(
                "[ERROR] action=delete for `{name}` but type not in baseline — \
                 cannot delete non-existent type"
            );
        }
        return Err(CliError::Message(
            "delete action validation failed: one or more entries reference non-existent \
             baseline types"
                .to_owned(),
        ));
    }

    Ok(())
}

/// Format the signal summary line with baseline-aware counts.
///
/// Returns a `String` containing the full output (newline-terminated lines) so the
/// formatting logic is testable without requiring the nightly toolchain that the full
/// `execute_type_signals` path needs.
///
/// `total` equals `signals.len()` — it counts every emitted signal (forward + reverse),
/// not just the entries in `<layer>-types.json`, to keep `blue + yellow + red == total`.
///
/// `catalogue_file` is the filename (e.g. `domain-types.json` or `usecase-types.json`)
/// reported in the undeclared-types WARN line so users see the correct file to update.
fn format_signal_summary(
    signals: &[domain::TypeSignal],
    undeclared_count: usize,
    skipped_count: usize,
    catalogue_file: &str,
) -> String {
    let blue = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Blue).count();
    let yellow = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Yellow).count();
    let red = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Red).count();
    let total = signals.len();
    let mut out = format!(
        "[OK] type-signals: blue={blue} yellow={yellow} red={red} (total={total}, undeclared={undeclared_count}, skipped={skipped_count})\n",
    );

    if undeclared_count > 0 {
        out.push_str(&format!(
            "[WARN] {undeclared_count} undeclared type(s)/trait(s) found. Run /track:design to update {catalogue_file}.\n"
        ));
    }

    out
}

/// Print the signal summary produced by [`format_signal_summary`].
fn print_signal_summary(
    signals: &[domain::TypeSignal],
    undeclared_count: usize,
    skipped_count: usize,
    catalogue_file: &str,
) {
    print!("{}", format_signal_summary(signals, undeclared_count, skipped_count, catalogue_file));
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::*;

    /// Minimal valid `metadata.json` for tests where the active-track guard should pass.
    /// All tasks are `todo` → derived `TrackStatus::Planned`.
    fn minimal_active_metadata_json(track_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": 3,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z",
  "tasks": [
    {{
      "id": "T001",
      "description": "A task",
      "status": "todo",
      "commit_hash": null
    }}
  ],
  "plan": {{
    "summary": ["Test"],
    "sections": [{{
      "id": "S001",
      "title": "Test",
      "description": ["Test"],
      "task_ids": ["T001"]
    }}]
  }}
}}
"#
        )
    }

    /// Sets up a minimal track directory with the given `domain-types.json` content
    /// and a valid `metadata.json` (status=planned) so the active-track guard passes.
    fn setup_track(dir: &std::path::Path, domain_types: &str) -> (PathBuf, String) {
        let items_dir = dir.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), domain_types).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json(track_id))
            .unwrap();
        (items_dir, track_id.to_owned())
    }

    #[test]
    fn test_execute_type_signals_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result = execute_type_signals(items_dir, "../evil".to_owned(), workspace_root, None);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_execute_type_signals_with_missing_domain_types_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result = execute_type_signals(items_dir, "test-track".to_owned(), workspace_root, None);
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("/track:design"), "error must suggest /track:design, got: {msg}");
    }

    #[test]
    fn test_execute_type_signals_with_malformed_domain_types_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_id) = setup_track(dir.path(), "{not valid json}");
        let workspace_root = dir.path().to_path_buf();

        let result = execute_type_signals(items_dir, track_id, workspace_root, None);
        assert!(result.is_err(), "malformed domain-types.json must return error");
    }

    #[test]
    fn test_execute_type_signals_with_unknown_layer_returns_error() {
        // When --layer is specified with a layer that is not tddd.enabled in
        // architecture-rules.json, the command must fail-closed with a clear error.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        let workspace_root = dir.path().to_path_buf();

        // No architecture-rules.json => fallback has only "domain"; "nonexistent" should fail.
        let result = execute_type_signals(
            items_dir,
            "test-track".to_owned(),
            workspace_root,
            Some("nonexistent".to_owned()),
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("nonexistent"),
            "error must mention the unknown layer name, got: {msg}"
        );
        assert!(msg.contains("not tddd.enabled"), "error must mention tddd.enabled, got: {msg}");
    }

    #[test]
    fn test_execute_type_signals_with_usecase_layer_dispatches_to_usecase_catalogue() {
        // Regression guard: when --layer usecase is specified and usecase is enabled in
        // architecture-rules.json, execute_type_signals must read usecase-types.json (not
        // domain-types.json). The absence of usecase-types.json causes a NotFound error
        // mentioning that file — proving the dispatch went to the usecase binding.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();

        // Write architecture-rules.json with usecase enabled.
        let rules_json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            {
              "crate": "usecase",
              "tddd": {
                "enabled": true,
                "catalogue_file": "usecase-types.json",
                "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
              }
            }
          ]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        // Do NOT write usecase-types.json — the error must mention it, proving
        // that execute_type_signals dispatched to the usecase binding.
        let result = execute_type_signals(
            items_dir,
            "test-track".to_owned(),
            dir.path().to_path_buf(),
            Some("usecase".to_owned()),
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        // The error must mention the usecase catalogue file (not domain-types.json),
        // proving the multi-layer dispatch reached the usecase binding.
        assert!(
            msg.contains("usecase-types.json"),
            "error must mention usecase-types.json (not domain), got: {msg}"
        );
        assert!(
            !msg.contains("domain-types.json"),
            "error must NOT mention domain-types.json for --layer usecase, got: {msg}"
        );
        // Must not carry any Phase 1 rejection message.
        assert!(!msg.contains("not yet supported"), "Phase 1 rejection must be gone, got: {msg}");
    }

    // --- format_signal_summary tests (pure, no nightly needed) ---

    fn make_signal(signal: domain::ConfidenceSignal) -> domain::TypeSignal {
        domain::TypeSignal::new("Foo", "value_object", signal, true, vec![], vec![], vec![])
    }

    #[test]
    fn test_format_signal_summary_with_no_signals_prints_zero_counts() {
        let out = format_signal_summary(&[], 0, 0, "domain-types.json");
        assert!(
            out.contains("blue=0 yellow=0 red=0 (total=0, undeclared=0, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(!out.contains("[WARN]"), "no WARN expected when undeclared=0");
    }

    #[test]
    fn test_format_signal_summary_with_mixed_signals_counts_correctly() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let out = format_signal_summary(&signals, 0, 0, "domain-types.json");
        assert!(
            out.contains("blue=2 yellow=1 red=1 (total=4, undeclared=0, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(!out.contains("[WARN]"), "no WARN expected when undeclared=0");
    }

    #[test]
    fn test_format_signal_summary_with_undeclared_shows_warn_and_track_design() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Red),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        // 2 undeclared signals are represented in the red count above; undeclared_count is passed
        // separately to distinguish reverse-Red from forward-Red in the WARN line.
        let out = format_signal_summary(&signals, 2, 0, "usecase-types.json");
        assert!(
            out.contains("blue=1 yellow=0 red=2 (total=3, undeclared=2, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(out.contains("[WARN]"), "WARN line expected when undeclared>0");
        assert!(out.contains("/track:design"), "WARN must mention /track:design, got: {out}");
        // Verify the catalogue_file parameter is reflected in the WARN message.
        assert!(
            out.contains("usecase-types.json"),
            "WARN must mention the specific catalogue file, got: {out}"
        );
    }

    #[test]
    fn test_format_signal_summary_blue_plus_yellow_plus_red_equals_total() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let out = format_signal_summary(&signals, 1, 0, "domain-types.json");
        // invariant: blue + yellow + red == total
        assert!(out.contains("blue=1 yellow=2 red=1 (total=4,"), "totals must sum: {out}");
    }

    // --- print_action_diagnostics tests (pure, no nightly needed) ---

    fn make_entry_d(name: &str, action: domain::TypeAction) -> domain::TypeCatalogueEntry {
        domain::TypeCatalogueEntry::new(
            name,
            "desc",
            domain::TypeDefinitionKind::ValueObject,
            action,
            true,
        )
        .unwrap()
    }

    fn empty_graph_d() -> domain::TypeGraph {
        domain::TypeGraph::new(std::collections::HashMap::new(), std::collections::HashMap::new())
    }

    fn empty_baseline_d() -> domain::TypeBaseline {
        domain::TypeBaseline::new(
            1,
            domain::Timestamp::new("2026-01-01T00:00:00Z").unwrap(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        )
    }

    #[test]
    fn test_print_action_diagnostics_with_clean_report_returns_ok() {
        // A report with no contradictions and no delete errors must return Ok.
        let report = domain::check_consistency(&[], &empty_graph_d(), &empty_baseline_d());
        let result = print_action_diagnostics(&report);
        assert!(result.is_ok(), "clean report must return Ok: {result:?}");
    }

    #[test]
    fn test_print_action_diagnostics_with_delete_error_returns_cli_error() {
        // action=delete on a type not in baseline → delete_errors non-empty → Err.
        let entry = make_entry_d("Ghost", domain::TypeAction::Delete);
        let report = domain::check_consistency(&[entry], &empty_graph_d(), &empty_baseline_d());
        assert!(!report.delete_errors().is_empty(), "delete_errors must be non-empty");
        let result = print_action_diagnostics(&report);
        assert!(result.is_err(), "delete error must return Err: {result:?}");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("delete action validation failed"),
            "error must mention delete validation, got: {err_msg}"
        );
    }

    #[test]
    fn test_print_action_diagnostics_does_not_write_files_on_failure() {
        // Verifies that print_action_diagnostics itself has no write side effects —
        // it only prints to stderr and returns Err on delete errors.
        // (File-write invariant for execute_type_signals is covered by the
        // fact that print_action_diagnostics is called before atomic_write_file.)
        let entry = make_entry_d("Phantom", domain::TypeAction::Delete);
        let report = domain::check_consistency(&[entry], &empty_graph_d(), &empty_baseline_d());
        // Calling the function must not panic and must return Err.
        let result = print_action_diagnostics(&report);
        assert!(result.is_err(), "must return Err for delete_errors");
    }

    #[test]
    fn test_validate_and_write_catalogue_does_not_write_on_delete_error() {
        // Verifies the validate-before-write ordering in validate_and_write_catalogue:
        // when delete errors are present, no files should be created or modified.
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let entry = make_entry_d("Ghost", domain::TypeAction::Delete);
        let doc = domain::TypeCatalogueDocument::new(1, vec![entry.clone()]);
        let report = domain::check_consistency(&[entry], &empty_graph_d(), &empty_baseline_d());

        assert!(
            !report.delete_errors().is_empty(),
            "precondition: delete_errors must be non-empty"
        );

        let result = validate_and_write_catalogue(
            &report,
            &doc,
            &domain_types_path,
            &track_dir,
            "domain-types.md",
        );

        assert!(result.is_err(), "delete errors must cause validate_and_write_catalogue to fail");
        assert!(
            !domain_types_path.exists(),
            "domain-types.json must NOT be written on delete-error abort"
        );
        assert!(
            !dir.path().join("domain-types.md").exists(),
            "domain-types.md must NOT be written on delete-error abort"
        );
    }

    #[test]
    fn test_validate_and_write_catalogue_writes_files_when_no_errors() {
        // Verifies that validate_and_write_catalogue writes both files when there
        // are no delete errors (contradiction warnings are advisory and do not block writes).
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        // A report with no errors: empty entries against empty baseline/graph.
        let doc = domain::TypeCatalogueDocument::new(1, vec![]);
        let report = domain::check_consistency(&[], &empty_graph_d(), &empty_baseline_d());

        assert!(report.delete_errors().is_empty(), "precondition: no delete errors");

        let result = validate_and_write_catalogue(
            &report,
            &doc,
            &domain_types_path,
            &track_dir,
            "domain-types.md",
        );

        assert!(result.is_ok(), "no-error report must succeed: {result:?}");
        assert!(domain_types_path.exists(), "domain-types.json must be written on success");
        assert!(
            dir.path().join("domain-types.md").exists(),
            "domain-types.md must be written on success"
        );
    }

    #[test]
    fn test_validate_and_write_catalogue_rendered_header_uses_json_catalogue_filename() {
        // Regression guard for the T004 caller-side header fix: the rendered `.md` file's
        // `Generated from ...` header must reference the catalogue JSON filename, not the
        // markdown output filename.  Using a non-domain path (`infrastructure-types.json`)
        // makes the distinction explicit — if the derivation regresses back to passing
        // `rendered_file_stem` the header would incorrectly say
        // `Generated from infrastructure-types.md`.
        let dir = tempfile::tempdir().unwrap();
        let catalogue_path = dir.path().join("infrastructure-types.json");
        let track_dir = dir.path().to_path_buf();

        let doc = domain::TypeCatalogueDocument::new(1, vec![]);
        let report = domain::check_consistency(&[], &empty_graph_d(), &empty_baseline_d());

        assert!(report.delete_errors().is_empty(), "precondition: no delete errors");

        let result = validate_and_write_catalogue(
            &report,
            &doc,
            &catalogue_path,
            &track_dir,
            "infrastructure-types.md",
        );

        assert!(result.is_ok(), "clean report must succeed: {result:?}");

        let rendered = std::fs::read_to_string(dir.path().join("infrastructure-types.md"))
            .expect("rendered md must exist");
        assert!(
            rendered.contains("Generated from infrastructure-types.json"),
            "rendered header must reference the JSON catalogue file, got:\n{rendered}"
        );
        assert!(
            !rendered.contains("Generated from infrastructure-types.md"),
            "rendered header must NOT reference the md output file, got:\n{rendered}"
        );
    }

    // --- evaluate_and_write_signals tests (core path, no nightly needed) ---

    #[test]
    fn test_evaluate_and_write_signals_with_delete_error_returns_err_and_leaves_files_untouched() {
        // End-to-end test for the abort-before-write path via the public core evaluator.
        // Proves that `execute_type_signals` cannot write files on delete errors
        // regardless of where the `validate_and_write_catalogue` call is placed.
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let entry = make_entry_d("Ghost", domain::TypeAction::Delete);
        let mut doc = domain::TypeCatalogueDocument::new(1, vec![entry.clone()]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-types.md",
            "domain-type-signals.json",
        );

        assert!(result.is_err(), "delete error must cause evaluate_and_write_signals to fail");
        assert!(
            !domain_types_path.exists(),
            "domain-types.json must NOT be written on delete-error abort"
        );
        assert!(
            !dir.path().join("domain-types.md").exists(),
            "domain-types.md must NOT be written on delete-error abort"
        );
        assert!(
            !dir.path().join("domain-type-signals.json").exists(),
            "domain-type-signals.json must NOT be written on delete-error abort"
        );
    }

    #[test]
    fn test_evaluate_and_write_signals_with_clean_report_returns_success_and_writes_files() {
        // End-to-end test for the success path via the public core evaluator.
        // Proves that `execute_type_signals` writes catalogue + rendered view + signal file
        // on a clean report, and that the signal file declaration_hash matches the
        // on-disk declaration bytes (ADR §D5 post-encode hash pin).
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let mut doc = domain::TypeCatalogueDocument::new(1, vec![]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-types.md",
            "domain-type-signals.json",
        );

        assert!(result.is_ok(), "clean report must succeed: {result:?}");
        assert_eq!(result.unwrap(), ExitCode::SUCCESS, "must return EXIT_SUCCESS");
        assert!(domain_types_path.exists(), "domain-types.json must be written on success");
        assert!(
            dir.path().join("domain-types.md").exists(),
            "domain-types.md must be written on success"
        );
        // T004: dual-write — signal file must be written alongside the declaration file.
        let signal_path = dir.path().join("domain-type-signals.json");
        assert!(signal_path.exists(), "domain-type-signals.json must be written on success");

        // declaration_hash in the signal file must equal SHA-256 of the on-disk
        // declaration bytes (ADR §D5: hash pinned to post-encode disk bytes).
        let declaration_bytes = std::fs::read(&domain_types_path).unwrap();
        let expected_hash =
            infrastructure::tddd::type_signals_codec::declaration_hash(&declaration_bytes);
        let signal_json = std::fs::read_to_string(&signal_path).unwrap();
        let signal_doc = infrastructure::tddd::type_signals_codec::decode(&signal_json).unwrap();
        assert_eq!(
            signal_doc.declaration_hash(),
            expected_hash,
            "declaration_hash in signal file must match SHA-256 of on-disk declaration bytes"
        );
        assert_eq!(signal_doc.schema_version(), 1, "signal file schema_version must be 1");
        assert!(
            signal_doc.signals().is_empty(),
            "empty report must yield empty signals in the signal file"
        );
    }

    #[test]
    fn test_evaluate_and_write_signals_signal_file_includes_forward_signals() {
        // The signal file must carry the forward-evaluation signals — not just
        // undeclared/baseline reds. A non-empty declared entry with a Blue
        // evaluation proves the forward signals are plumbed through.
        use std::collections::HashMap;

        use domain::schema::{TypeKind, TypeNode};

        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let entry = make_entry_d("TrackId", domain::TypeAction::Add);
        let mut doc = domain::TypeCatalogueDocument::new(1, vec![entry]);

        // Graph with TrackId present as a struct → Blue.
        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_owned(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], std::collections::HashSet::new()),
        );
        let profile = domain::TypeGraph::new(types, HashMap::new());

        let result = evaluate_and_write_signals(
            &mut doc,
            &profile,
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-types.md",
            "domain-type-signals.json",
        );

        assert!(result.is_ok(), "clean report must succeed: {result:?}");

        let signal_path = dir.path().join("domain-type-signals.json");
        let signal_json = std::fs::read_to_string(&signal_path).unwrap();
        let signal_doc = infrastructure::tddd::type_signals_codec::decode(&signal_json).unwrap();
        let signals = signal_doc.signals();
        assert!(!signals.is_empty(), "signal file must carry forward signals, got empty");
        let blue_track_id = signals
            .iter()
            .find(|s| s.type_name() == "TrackId" && s.signal() == domain::ConfidenceSignal::Blue);
        assert!(blue_track_id.is_some(), "TrackId must appear as Blue in the signal file");
    }

    #[cfg(unix)]
    #[test]
    fn test_evaluate_and_write_signals_rejects_signal_file_symlink() {
        // ADR §D7: writing the signal file to a symlink must fail-closed.
        // Setup: create a symlink at the signal-file path before invocation.
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let signal_path = dir.path().join("domain-type-signals.json");
        let track_dir = dir.path().to_path_buf();

        // Create a symlink at the signal file path pointing at a non-existent target.
        std::os::unix::fs::symlink(dir.path().join("nowhere.json"), &signal_path).unwrap();

        let mut doc = domain::TypeCatalogueDocument::new(1, vec![]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-types.md",
            "domain-type-signals.json",
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("refusing to write signal file"),
            "error must be from the signal-file symlink guard, got: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_evaluate_and_write_signals_rejects_declaration_file_symlink() {
        // ADR §D7: writing the declaration file to a symlink must fail-closed.
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        // Create a symlink at the declaration file path pointing nowhere.
        std::os::unix::fs::symlink(dir.path().join("nowhere.json"), &domain_types_path).unwrap();

        let mut doc = domain::TypeCatalogueDocument::new(1, vec![]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-types.md",
            "domain-type-signals.json",
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("refusing to write declaration file"),
            "error must be from the declaration-file symlink guard, got: {msg}"
        );
        assert!(
            !dir.path().join("domain-type-signals.json").exists(),
            "signal file must NOT be written when declaration-file guard aborts"
        );
    }

    #[test]
    fn test_execute_type_signals_no_layer_filter_iterates_all_enabled_bindings() {
        // Regression guard: when --layer is omitted, the loop iterates enabled bindings
        // in layers[] order. This test proves the domain binding is entered FIRST by
        // asserting the error comes from a step that is LATER in domain's execution
        // than where usecase would fail.
        //
        // Execution order for `execute_type_signals_for_layer`:
        //   1. catalogue read  (fail → "/track:design" message)
        //   2. export (nightly required; fail → "failed to export schema: … nightly")
        //   3. baseline read   (fail → "run baseline-capture first")
        //   4. evaluate + write
        //
        // Setup:
        //   - domain: catalogue present + valid → step 1 OK, step 2 fails (nightly).
        //   - usecase: catalogue absent → step 1 fails ("/track:design").
        //
        // Expected error: from domain's step 2 (nightly export), which mentions
        // "failed to export schema" or "nightly". It does NOT mention usecase-types.json.
        //
        // If the loop processed usecase FIRST (regression), it would fail at step 1
        // ("/track:design" + "usecase-types.json"), so the assertion `!msg.contains(
        // "usecase-types.json")` would fail — catching the bug.
        //
        // Note: because `execute_type_signals_for_layer` always reaches the nightly
        // export call before the baseline read (step 3), the first binding can never
        // return `Ok` in a unit test environment. A full "first succeeds, loop
        // continues to second" integration is covered by the `#[ignore]` test below
        // (requires nightly).
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();

        let rules_json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            {
              "crate": "usecase",
              "tddd": {
                "enabled": true,
                "catalogue_file": "usecase-types.json",
                "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
              }
            }
          ]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        // domain-types.json present + valid → domain binding passes catalogue read and
        // fails at rustdoc export (nightly unavailable in test env).
        // usecase-types.json absent → usecase would fail at catalogue read (step 1).
        // Domain fails LATER than usecase would, so the error distinguishes which
        // binding the loop entered first.
        let domain_types_json = r#"{"schema_version":2,"type_definitions":[]}"#;
        std::fs::write(track_dir.join("domain-types.json"), domain_types_json).unwrap();
        // do NOT write usecase-types.json

        let result = execute_type_signals(
            items_dir,
            "test-track".to_owned(),
            dir.path().to_path_buf(),
            None,
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        // Domain fails at rustdoc export (step 2), which does NOT mention usecase-types.json.
        // If a regression caused the loop to process usecase first (step 1 fail), the
        // error would mention "usecase-types.json" — caught by this assertion.
        assert!(
            !msg.contains("usecase-types.json"),
            "error must NOT mention usecase-types.json; domain binding must be processed first \
             (its export error comes from a later step than usecase's catalogue-not-found); \
             got: {msg}"
        );
        // Confirm the error is from domain's export step, not some unrelated path.
        assert!(
            msg.contains("export schema") || msg.contains("nightly") || msg.contains("failed"),
            "error must be from domain's rustdoc export step, got: {msg}"
        );
    }

    /// Success-path integration test.  Requires nightly toolchain for `cargo +nightly rustdoc`.
    /// Run with: `cargo test --package cli -- --ignored`
    #[test]
    #[ignore]
    fn test_execute_type_signals_success_path_writes_signals() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ]
}"#;
        let (items_dir, track_id) = setup_track(dir.path(), domain_types_json);
        // Write an empty baseline so the baseline-required code path succeeds.
        let baseline_json = r#"{
  "schema_version": 2,
  "captured_at": "2026-01-01T00:00:00Z",
  "types": {},
  "traits": {}
}"#;
        std::fs::write(items_dir.join(&track_id).join("domain-types-baseline.json"), baseline_json)
            .unwrap();
        // workspace_root must point to the real workspace so rustdoc can find the domain crate.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();

        let result =
            execute_type_signals(items_dir.clone(), track_id.clone(), workspace_root, None);
        assert!(result.is_ok(), "success path must return Ok: {result:?}");

        // Verify signals were written back
        let updated =
            std::fs::read_to_string(items_dir.join(&track_id).join("domain-types.json")).unwrap();
        assert!(updated.contains("\"signals\""), "signals must be written to domain-types.json");

        // Verify domain-types.md was generated
        let md_path = items_dir.join(&track_id).join("domain-types.md");
        assert!(md_path.exists(), "domain-types.md must be generated");
    }

    // --- ensure_active_track unit tests (exhaustive TrackStatus variant coverage) ---

    #[test]
    fn test_ensure_active_track_rejects_done() {
        let result = ensure_active_track(TrackStatus::Done, "test-track");
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=done"), "msg should mention status=done: {msg}");
        assert!(msg.contains("Completed tracks are frozen"), "msg: {msg}");
        assert!(msg.contains("test-track"), "msg should mention track_id: {msg}");
    }

    #[test]
    fn test_ensure_active_track_rejects_archived() {
        let result = ensure_active_track(TrackStatus::Archived, "test-track");
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=archived"), "msg should mention status=archived: {msg}");
        assert!(msg.contains("Completed tracks are frozen"), "msg: {msg}");
    }

    #[test]
    fn test_ensure_active_track_allows_planned() {
        assert!(ensure_active_track(TrackStatus::Planned, "test-track").is_ok());
    }

    #[test]
    fn test_ensure_active_track_allows_in_progress() {
        assert!(ensure_active_track(TrackStatus::InProgress, "test-track").is_ok());
    }

    #[test]
    fn test_ensure_active_track_allows_blocked() {
        assert!(ensure_active_track(TrackStatus::Blocked, "test-track").is_ok());
    }

    #[test]
    fn test_ensure_active_track_allows_cancelled() {
        assert!(ensure_active_track(TrackStatus::Cancelled, "test-track").is_ok());
    }

    // --- Integration test: execute_type_signals rejects done track via full path ---

    #[test]
    fn test_execute_type_signals_rejects_done_track() {
        // Integration: a track whose derived TrackStatus is Done (all tasks DoneTraced)
        // must be rejected by execute_type_signals before reaching catalogue read,
        // protecting merged-track data immutability.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-done-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let done_metadata = r#"{
  "schema_version": 3,
  "id": "test-done-track",
  "branch": "track/test-done-track",
  "title": "Test Done Track",
  "status": "done",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z",
  "tasks": [
    {
      "id": "T001",
      "description": "A completed task",
      "status": "done",
      "commit_hash": "0000000000000000000000000000000000000000"
    }
  ],
  "plan": {
    "summary": ["Test"],
    "sections": [{
      "id": "S001",
      "title": "Test",
      "description": ["Test"],
      "task_ids": ["T001"]
    }]
  }
}"#;
        std::fs::write(track_dir.join("metadata.json"), done_metadata).unwrap();

        let result = execute_type_signals(
            items_dir,
            "test-done-track".to_owned(),
            dir.path().to_path_buf(),
            None,
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=done"), "guard must mention status=done, got: {msg}");
        assert!(
            msg.contains("Completed tracks are frozen"),
            "guard must mention 'Completed tracks are frozen', got: {msg}"
        );
        assert!(msg.contains("test-done-track"), "guard must mention the track_id, got: {msg}");
    }

    #[test]
    fn test_execute_type_signals_rejects_archived_track_with_incomplete_tasks() {
        // Regression guard: archived tracks with unresolved tasks must still be
        // rejected. `metadata.status()` derives from task states and would return
        // `InProgress` for such a track (which `ensure_active_track` allows). The
        // guard must consult `DocumentMeta::original_status` to catch this case.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-archived-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // status="archived" in JSON, but task T001 is still "in_progress" →
        // metadata.status() would return InProgress (not Archived).
        let archived_incomplete_metadata = r#"{
  "schema_version": 3,
  "id": "test-archived-track",
  "branch": "track/test-archived-track",
  "title": "Test Archived Track",
  "status": "archived",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z",
  "tasks": [
    {
      "id": "T001",
      "description": "An incomplete task",
      "status": "in_progress",
      "commit_hash": null
    }
  ],
  "plan": {
    "summary": ["Test"],
    "sections": [{
      "id": "S001",
      "title": "Test",
      "description": ["Test"],
      "task_ids": ["T001"]
    }]
  }
}"#;
        std::fs::write(track_dir.join("metadata.json"), archived_incomplete_metadata).unwrap();

        let result = execute_type_signals(
            items_dir,
            "test-archived-track".to_owned(),
            dir.path().to_path_buf(),
            None,
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("status=archived"), "guard must mention status=archived, got: {msg}");
        assert!(
            msg.contains("Completed tracks are frozen"),
            "guard must mention 'Completed tracks are frozen', got: {msg}"
        );
        assert!(msg.contains("test-archived-track"), "guard must mention the track_id, got: {msg}");
    }
}
