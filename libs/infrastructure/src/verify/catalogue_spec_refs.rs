//! Per-layer catalogue-spec-ref integrity verification helpers.
//!
//! Moved from the CLI layer so that the CLI composition root never imports
//! `domain::tddd::LayerId`, `domain::ContentHash`, `domain::SpecElementId`,
//! `domain::SpecRefFinding`, `domain::SpecRefFindingKind`, or
//! `domain::check_catalogue_spec_ref_integrity` directly (CN-01 / AC-03).
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D1.5 / §D3.2 / §D3.6 / IN-10.

use std::collections::BTreeMap;
use std::path::Path;

use domain::tddd::LayerId;
use domain::{
    ContentHash, SpecElementId, SpecRefFinding, SpecRefFindingKind,
    check_catalogue_spec_ref_integrity,
};

use crate::tddd::{catalogue_codec, catalogue_spec_signals_codec, type_signals_codec};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

/// Detect whether any TDDD-enabled layer with `catalogue_spec_signal` opt-in
/// has its catalogue file present under `track_dir`.
///
/// Used as the Phase 0/1 gate (ADR D2.3): when no enabled catalogue exists,
/// the verifier returns silent PASS without consulting `spec.json`. Once at
/// least one catalogue is present, the SoT Chain ② contract activates and
/// `spec.json` becomes a hard requirement.
///
/// `items_dir` is the symlink-guard trusted root — every path component
/// between `items_dir` and the catalogue leaf is verified to be a real
/// (non-symlink) entry before existence is reported. Symlinks that escape
/// the workspace are rejected with `Err`; dangling symlinks below `items_dir`
/// are reported as absent (mirrors `verify_one_layer`'s lenient absent path).
///
/// # Errors
///
/// Returns a human-readable error string when a symlink that escapes the
/// trusted root is detected for any candidate catalogue path.
pub fn any_enabled_catalogue_present(
    bindings: &[TdddLayerBinding],
    track_dir: &Path,
    items_dir: &Path,
) -> Result<bool, String> {
    // Fail-closed: if `track_dir` is not a real directory (e.g. it is a regular
    // file), `reject_symlinks_below` will report all child catalogue paths as absent
    // and the caller would take the silent-pass path. Validate that `track_dir` is
    // an actual directory before probing catalogues.
    match track_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(format!(
                "symlink guard: refusing to use symlinked track_dir: {}",
                track_dir.display()
            ));
        }
        Ok(meta) if !meta.file_type().is_dir() => {
            return Err(format!(
                "expected a directory at track path '{}', found a non-directory entry. \
                 Check --items-dir and --track-id.",
                track_dir.display()
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // track_dir absent → no catalogues present.
            return Ok(false);
        }
        Err(e) => {
            return Err(format!(
                "symlink guard: cannot stat track_dir '{}': {e}",
                track_dir.display()
            ));
        }
    }

    for binding in bindings {
        if !binding.catalogue_spec_signal_enabled() {
            continue;
        }
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let present = reject_symlinks_below(&catalogue_path, items_dir).map_err(|e| {
            format!(
                "symlink guard: refusing to read catalogue '{}' for layer '{}': {e}",
                catalogue_path.display(),
                binding.layer_id()
            )
        })?;
        if present {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Read spec.json from the local workspace and build a map of per-element
/// canonical SHA-256 hashes keyed by `SpecElementId`.
///
/// `items_dir` is the symlink-guard trusted root — every path component
/// between `items_dir` and the spec.json leaf is verified to be a real
/// (non-symlink) path entry before the file is read.
///
/// # Errors
///
/// Returns a human-readable error string when the spec.json is absent, is a
/// symlink, fails to parse as JSON, or contains an element id that does not
/// satisfy `SpecElementId::try_new` (fail-closed: malformed ids abort the run
/// rather than silently producing an incomplete map that could hide findings).
pub fn read_spec_element_hashes(
    track_dir: &Path,
    items_dir: &Path,
) -> Result<BTreeMap<SpecElementId, ContentHash>, String> {
    let spec_path = track_dir.join("spec.json");

    // Symlink guard: reject symlinks at spec.json or any ancestor below items_dir.
    reject_symlinks_below(&spec_path, items_dir)
        .map_err(|e| format!("symlink guard: spec.json at '{}': {e}", spec_path.display()))?;

    let text = std::fs::read_to_string(&spec_path)
        .map_err(|e| format!("cannot read spec.json at '{}': {e}", spec_path.display()))?;
    // Validate schema first (mirrors `load_spec_element_map` in plan_artifact_refs.rs):
    // catches duplicate IDs, malformed required fields, and unsupported schema_version before
    // the element map is built.
    crate::spec::codec::decode(&text)
        .map_err(|e| format!("spec.json schema error at '{}': {e}", spec_path.display()))?;
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("spec.json JSON parse error: {e}"))?;
    let element_map = crate::verify::plan_artifact_refs::build_element_map(&raw);
    let mut out: BTreeMap<SpecElementId, ContentHash> = BTreeMap::new();
    for (id_str, canonical_json) in element_map {
        let anchor = SpecElementId::try_new(id_str.clone())
            .map_err(|e| format!("spec.json contains element with invalid id '{id_str}': {e}"))?;
        let hash_hex = crate::verify::plan_artifact_refs::canonical_json_sha256(&canonical_json);
        let hash = ContentHash::try_from_hex(hash_hex)
            .map_err(|e| format!("internal: canonical hash parse error: {e}"))?;
        out.insert(anchor, hash);
    }
    Ok(out)
}

/// Verify one layer — returns the list of formatted finding strings emitted by
/// the domain pure function.
///
/// All inputs (catalogue, spec hashes, signals) are resolved under `track_dir`
/// (= `items_dir/<track_id>`). `items_dir` is the symlink-guard trusted root for all reads.
///
/// Returns formatted strings instead of `SpecRefFinding` so the CLI never needs
/// to import `domain::SpecRefFinding` or `domain::SpecRefFindingKind` (CN-01 / AC-03).
///
/// # Errors
///
/// Returns a human-readable error string on I/O or decode failures.
pub fn verify_one_layer_formatted(
    track_dir: &Path,
    items_dir: &Path,
    binding: &TdddLayerBinding,
    spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>,
    skip_stale: bool,
) -> Result<Vec<String>, String> {
    let findings =
        verify_one_layer(track_dir, items_dir, binding, spec_element_hashes, skip_stale)?;
    Ok(findings.into_iter().map(|f| format_finding(&f)).collect())
}

/// Verify one layer — returns the raw `SpecRefFinding` list.
///
/// Internal helper; callers in the CLI layer should prefer
/// `verify_one_layer_formatted` to avoid importing domain types.
fn verify_one_layer(
    track_dir: &Path,
    items_dir: &Path,
    binding: &TdddLayerBinding,
    spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>,
    skip_stale: bool,
) -> Result<Vec<SpecRefFinding>, String> {
    let layer_id = binding.layer_id();
    let layer_id_newtype =
        LayerId::try_new(layer_id).map_err(|e| format!("invalid layer id '{layer_id}': {e}"))?;
    let catalogue_path = track_dir.join(binding.catalogue_file());

    let catalogue_present = reject_symlinks_below(&catalogue_path, items_dir).map_err(|e| {
        format!(
            "symlink guard: refusing to read catalogue '{}' for layer '{layer_id}': {e}",
            catalogue_path.display()
        )
    })?;

    if !catalogue_present {
        // Skip layers whose catalogue file is absent — consistent with the
        // `sotp track type-signals` lenient CI path.
        return Ok(Vec::new());
    }

    let bytes = std::fs::read(&catalogue_path)
        .map_err(|e| format!("cannot read catalogue '{}': {e}", catalogue_path.display()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        format!("catalogue '{}' contains non-UTF-8 bytes: {e}", catalogue_path.display())
    })?;
    let catalogue = catalogue_codec::decode(text)
        .map_err(|e| format!("cannot decode catalogue '{}': {e}", catalogue_path.display()))?;

    let catalogue_hash_hex = type_signals_codec::declaration_hash(&bytes);
    let catalogue_hash = ContentHash::try_from_hex(&catalogue_hash_hex).map_err(|e| {
        format!("internal: catalogue hash for layer '{layer_id}' is not canonical hex: {e}")
    })?;

    let (current_hash_opt, signals_opt) = if skip_stale {
        (None, None)
    } else {
        let signals_path = track_dir.join(format!("{layer_id}-catalogue-spec-signals.json"));
        let signals_present = reject_symlinks_below(&signals_path, items_dir).map_err(|e| {
            format!(
                "symlink guard: refusing to read signals '{}' for layer '{layer_id}': {e}",
                signals_path.display()
            )
        })?;
        if signals_present {
            let signals_text = std::fs::read_to_string(&signals_path)
                .map_err(|e| format!("cannot read signals '{}': {e}", signals_path.display()))?;
            let signals = catalogue_spec_signals_codec::decode(&signals_text)
                .map_err(|e| format!("cannot decode signals '{}': {e}", signals_path.display()))?;
            (Some(catalogue_hash.clone()), Some(signals))
        } else {
            (None, None)
        }
    };

    let findings = check_catalogue_spec_ref_integrity(
        &layer_id_newtype,
        &catalogue,
        spec_element_hashes,
        current_hash_opt.as_ref(),
        signals_opt.as_ref(),
    );
    Ok(findings)
}

/// Remove newline and carriage-return characters from a string so that a
/// malicious or malformed catalogue entry cannot inject extra lines into the
/// one-finding-per-line stderr format.
fn sanitize_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

/// Format a single `SpecRefFinding` for stderr (one line per finding).
///
/// `catalogue_entry` and `spec_file` are sanitized (newlines replaced with
/// space) before interpolation to prevent log-injection via malformed catalogue
/// content.
///
/// Output format follows ADR §D1.5:
/// - `DanglingAnchor`: `[layer=<L>] <entry>[ref=<i>] <file>: dangling anchor '<anchor>'`
/// - `HashMismatch`: `[layer=<L>] <entry>[ref=<i>] <file>: hash mismatch for '<anchor>' (declared=<hex>, actual=<hex>)`
/// - `StaleSignals`: `[layer=<L>] stale catalogue-spec-signals (declared=<hex>, actual=<hex>)`
pub fn format_finding(finding: &SpecRefFinding) -> String {
    let layer = finding.layer.as_ref();
    match &finding.kind {
        SpecRefFindingKind::DanglingAnchor { catalogue_entry, ref_index, spec_file, anchor } => {
            format!(
                "[layer={layer}] {}[ref={ref_index}] {}: dangling anchor '{}'",
                sanitize_line(catalogue_entry),
                sanitize_line(&spec_file.display().to_string()),
                anchor.as_ref()
            )
        }
        SpecRefFindingKind::HashMismatch {
            catalogue_entry,
            ref_index,
            spec_file,
            anchor,
            declared,
            actual,
        } => format!(
            "[layer={layer}] {}[ref={ref_index}] {}: hash mismatch for '{}' (declared={}, actual={})",
            sanitize_line(catalogue_entry),
            sanitize_line(&spec_file.display().to_string()),
            anchor.as_ref(),
            declared.to_hex(),
            actual.to_hex()
        ),
        SpecRefFindingKind::StaleSignals { declared_catalogue_hash, actual_catalogue_hash } => {
            format!(
                "[layer={layer}] stale catalogue-spec-signals (declared={}, actual={})",
                declared_catalogue_hash.to_hex(),
                actual_catalogue_hash.to_hex()
            )
        }
    }
}
