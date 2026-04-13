//! `sotp track baseline-capture` — capture TypeGraph snapshot as baseline.
//!
//! Generates `domain-types-baseline.json` from the current TypeGraph.
//! Skips if baseline already exists (idempotent). Use `--force` to regenerate.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::{baseline_builder, baseline_codec};
use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::track::symlink_guard::reject_symlinks_below;
use infrastructure::verify::tddd_layers::parse_tddd_layers;

use crate::CliError;

/// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
///
/// Steps:
/// 1. Check if `domain-types-baseline.json` already exists (skip unless `--force`).
/// 2. Export domain crate schema via rustdoc JSON.
/// 3. Build TypeGraph and convert to TypeBaseline.
/// 4. Encode and write to `domain-types-baseline.json`.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, rustdoc export fails,
/// or the file write fails.
pub fn execute_baseline_capture(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    force: bool,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // T007: Phase 1 wires only the `domain` layer. `--layer` is accepted on
    // the CLI surface so Phase 2 can extend it without another breaking
    // change, but any non-`domain` value is rejected fail-closed so that a
    // request like `baseline-capture --layer usecase` cannot silently
    // overwrite `domain-types-baseline.json` with the wrong target.
    if let Some(ref layer_id) = layer {
        if layer_id != "domain" {
            return Err(CliError::Message(format!(
                "layer '{layer_id}' is not yet supported by `baseline-capture` in Phase 1. \
                 Only `domain` is wired. Re-run with `--layer domain` (or omit `--layer`)."
            )));
        }
    }

    // T007 fail-closed: verify that `domain` is actually `tddd.enabled` in
    // the workspace's `architecture-rules.json`. If the caller disabled
    // domain (or deleted the rules file entirely), capturing a baseline for
    // an inactive layer would overwrite `domain-types-baseline.json` on a
    // layer the rest of the pipeline treats as opted out. Reject the run.
    let domain_binding = enforce_domain_tddd_enabled(&workspace_root, layer.as_deref())?;
    let catalogue_filename = domain_binding.catalogue_file().to_owned();
    let baseline_filename = domain_binding.baseline_file();

    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor. reject_symlinks_below only checks components *below* the trusted_root,
    // so a symlinked items_dir would pass through undetected.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            )));
        }
    }

    let track_dir = items_dir.join(&track_id);
    let baseline_path = track_dir.join(&baseline_filename);

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, &items_dir)
        .map_err(|e| CliError::Message(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file (unless --force).
    // Use is_file() rather than exists() so that a directory or other non-file node at
    // that path does not silently produce a spurious success — it falls through and will
    // fail at the write step with a meaningful error instead.
    if baseline_path.is_file() && !force {
        println!(
            "[OK] baseline-capture: {baseline_filename} already exists for '{track_id}' (use --force to regenerate)"
        );
        return Ok(ExitCode::SUCCESS);
    }

    // Fail fast if the track directory does not exist.
    if !track_dir.is_dir() {
        return Err(CliError::Message(format!(
            "track directory not found: {} (did you mean an existing track ID?)",
            track_dir.display()
        )));
    }

    // Read the configured catalogue file to extract typestate names for
    // `build_type_graph`. Security: guard the leaf path too — a symlinked
    // catalogue file inside the track directory could redirect reads
    // outside the trusted tree.
    let domain_types_path = track_dir.join(&catalogue_filename);
    reject_symlinks_below(&domain_types_path, &items_dir)
        .map_err(|e| CliError::Message(format!("symlink guard: {e}")))?;
    let typestate_names: std::collections::HashSet<String> =
        if let Ok(json) = std::fs::read_to_string(&domain_types_path) {
            if let Ok(doc) = infrastructure::tddd::catalogue_codec::decode(&json) {
                doc.typestate_names()
            } else {
                std::collections::HashSet::new()
            }
        } else {
            std::collections::HashSet::new()
        };

    // Export the domain crate's public API via rustdoc JSON.
    let exporter = RustdocSchemaExporter::new(workspace_root);
    let schema = exporter.export("domain").map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        CliError::Message(format!("failed to export schema: {e}{hint}"))
    })?;

    // Build TypeGraph and convert to TypeBaseline.
    let graph = build_type_graph(&schema, &typestate_names);
    let captured_at = infrastructure::timestamp_now()
        .map_err(|e| CliError::Message(format!("timestamp error: {e}")))?;
    let baseline = baseline_builder::build_baseline(&graph, captured_at);

    // Encode and write.
    let encoded = baseline_codec::encode(&baseline)
        .map_err(|e| CliError::Message(format!("baseline encode error: {e}")))?;

    atomic_write_file(&baseline_path, format!("{encoded}\n").as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", baseline_path.display())))?;

    let type_count = baseline.types().len();
    let trait_count = baseline.traits().len();
    println!(
        "[OK] baseline-capture: wrote {baseline_filename} ({type_count} types, {trait_count} traits)"
    );

    Ok(ExitCode::SUCCESS)
}

/// Fails closed unless `domain` is `tddd.enabled=true` in
/// `architecture-rules.json`, and returns the resolved domain binding
/// so the caller can use `binding.catalogue_file()` /
/// `binding.baseline_file()` for filename-driven I/O.
///
/// Non-domain enabled layers produce a stderr warning when `--layer` is
/// omitted, mirroring the `type-signals` behavior: Phase 1 wires only
/// domain, so extra layers are skipped with an explicit warning rather
/// than silently or fail-closed.
///
/// When the rules file does not exist we fall back to a synthetic
/// default-domain binding so legacy repos (without a `tddd` block) still
/// behave the same as before T007.
///
/// `layer_filter` is `Some("domain")` when the caller asked for domain
/// explicitly — in that case no warning is printed for other layers
/// (the caller already acknowledged the single-layer scope).
fn enforce_domain_tddd_enabled(
    workspace_root: &std::path::Path,
    layer_filter: Option<&str>,
) -> Result<infrastructure::verify::tddd_layers::TdddLayerBinding, CliError> {
    let rules_path = workspace_root.join("architecture-rules.json");
    if !rules_path.is_file() {
        // Legacy fallback: no rules file → synthetic default-domain
        // binding (`domain-types.json` + `domain-types-baseline.json`).
        return synthetic_domain_binding();
    }
    let content = std::fs::read_to_string(&rules_path)
        .map_err(|e| CliError::Message(format!("cannot read {}: {e}", rules_path.display())))?;
    let bindings = parse_tddd_layers(&content)
        .map_err(|e| CliError::Message(format!("{}: {e}", rules_path.display())))?;
    let Some(domain_binding) = bindings.iter().find(|b| b.layer_id() == "domain").cloned() else {
        return Err(CliError::Message(
            "`domain` is not tddd.enabled in architecture-rules.json. baseline-capture \
             refuses to write domain baseline for an opted-out layer. \
             Enable `domain.tddd.enabled = true` or run a Phase 2 command for the \
             active layers."
                .to_owned(),
        ));
    };
    // Only warn about non-domain layers when the caller did NOT explicitly
    // select `--layer domain` — an explicit domain filter is a conscious
    // choice to run only the domain baseline, and the warning would be
    // noise.
    if layer_filter != Some("domain") {
        let non_domain: Vec<&str> =
            bindings.iter().map(|b| b.layer_id()).filter(|id| *id != "domain").collect();
        for layer_id in &non_domain {
            eprintln!(
                "[WARN] layer '{layer_id}' is tddd.enabled in architecture-rules.json but \
                 is not yet supported by `baseline-capture` in Phase 1. \
                 Skipping this layer; Phase 2 will add per-layer baseline capture."
            );
        }
    }
    Ok(domain_binding)
}

/// Returns a synthetic default-domain binding for the case where
/// `architecture-rules.json` is absent entirely.
fn synthetic_domain_binding()
-> Result<infrastructure::verify::tddd_layers::TdddLayerBinding, CliError> {
    let json = r#"{
      "layers": [
        { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } }
      ]
    }"#;
    parse_tddd_layers(json)
        .map_err(|e| {
            CliError::Message(format!("internal: default domain binding failed to parse: {e}"))
        })?
        .into_iter()
        .next()
        .ok_or_else(|| {
            CliError::Message("internal: default domain binding produced empty list".to_owned())
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_capture_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "../evil".to_owned(),
            dir.path().into(),
            false,
            None,
        );
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_baseline_capture_skips_when_baseline_exists() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write a dummy baseline file.
        std::fs::write(track_dir.join("domain-types-baseline.json"), "{}").unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            false,
            None,
        );
        assert!(result.is_ok(), "should skip existing baseline without error");
    }

    /// `--force` bypasses the skip check and proceeds to rustdoc export.
    ///
    /// In a test environment nightly rustdoc is not available, so the call fails
    /// with a schema export error rather than returning `Ok(SUCCESS)`.
    #[test]
    fn test_baseline_capture_force_flag_bypasses_skip() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Pre-existing baseline — would trigger skip without --force.
        std::fs::write(track_dir.join("domain-types-baseline.json"), "{}").unwrap();

        let result = execute_baseline_capture(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            true,
            None,
        );
        assert!(result.is_err(), "--force must bypass skip and attempt export");
    }
}
