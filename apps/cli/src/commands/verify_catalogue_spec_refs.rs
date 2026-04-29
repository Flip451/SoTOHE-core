//! `sotp verify catalogue-spec-refs` — binary gate for SoT Chain ② integrity
//! (dangling anchor / hash mismatch / stale signals).
//!
//! Reads the LOCAL `<layer>-types.json`, `spec.json`, and optionally
//! `<layer>-catalogue-spec-signals.json` and delegates to the domain pure
//! function `check_catalogue_spec_ref_integrity`. Emits one `SpecRefFinding`
//! per violation per layer to stderr, exits 0 on empty findings and non-zero
//! when any finding is reported.
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D1.5 / §D3.2 / §D3.6 / IN-10.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use domain::tddd::LayerId;
use domain::{
    ContentHash, SpecElementId, SpecRefFinding, SpecRefFindingKind,
    check_catalogue_spec_ref_integrity,
};
use infrastructure::tddd::{catalogue_codec, catalogue_spec_signals_codec, type_signals_codec};
use infrastructure::track::symlink_guard::reject_symlinks_below;
use infrastructure::verify::tddd_layers::{TdddLayerBinding, parse_tddd_layers};

use crate::CliError;

/// Entry point for `sotp verify catalogue-spec-refs`.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid, the layer filter is
/// unknown, or any I/O error occurs. Integrity violations are NOT reported
/// via `Err` — they are printed to stderr and reflected in the exit code
/// (non-zero on any finding).
#[allow(clippy::too_many_lines)]
pub fn execute_verify_catalogue_spec_refs(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    skip_stale: bool,
) -> Result<ExitCode, CliError> {
    let valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Security: `reject_symlinks_below` treats its second argument as the trusted
    // root and only guards components *below* it.  A symlinked `items_dir` would
    // therefore bypass all downstream symlink guards (spec.json, catalogue, signals).
    // Guard `items_dir` itself with `symlink_metadata` before using it as the root.
    // Mirrors `execute_catalogue_spec_signals` (catalogue_spec_signals.rs).
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

    // Security: guard `workspace_root` against symlinks at the leaf.
    // `reject_symlinks_below` cannot check `workspace_root` itself because it needs a
    // trusted root above the path — `workspace_root` IS the root.  Checking the leaf
    // with `symlink_metadata()` (mirrors `items_dir` guard above and the pattern in
    // `execute_catalogue_spec_signals`) catches the most common attack: a directly
    // symlinked root directory.  Symlinked parent directories above `workspace_root`
    // (e.g. `/tmp → /private/tmp` on macOS) are a systemic OS-level concern that
    // applies to all CLI commands with root-path arguments and is outside the scope
    // of this guard (same limitation accepted in `execute_catalogue_spec_signals`).
    match workspace_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at workspace_root: {}",
                workspace_root.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat workspace_root {}: {e}",
                workspace_root.display()
            )));
        }
    }

    // Security: guard the track directory itself against symlinks. The `items_dir`
    // guard above only covers `items_dir`; a symlinked `items_dir/<track_id>`
    // directory would escape the trusted tree before `reject_symlinks_below`
    // (anchored at `items_dir`) can catch it. Mirrors `execute_catalogue_spec_signals`
    // (verify.rs).
    let track_dir = items_dir.join(&track_id);
    match track_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at track directory: {}",
                track_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Track directory absent — fail-closed. ADR D2.3's "catalogue absent
            // → silent PASS" gate is meant for Phase 0/1 *real* tracks (the
            // directory exists, no catalogue yet), not for typos or stale CI
            // variables. Without this explicit check, a non-existent `track_id`
            // would resolve every catalogue as absent under
            // `any_enabled_catalogue_present` and silently pass, regressing
            // fail-closed behavior (the previous code reached
            // `read_spec_element_hashes` and surfaced the missing artifact via
            // a clear I/O error).
            return Err(CliError::Message(format!(
                "track directory does not exist: {} (check the track_id)",
                track_dir.display()
            )));
        }
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat track directory {}: {e}",
                track_dir.display()
            )));
        }
    }

    // Binary-gate fail-closed: `resolve_layers` (shared with `sotp track type-signals`)
    // falls back to a synthetic `domain` binding when `architecture-rules.json` is absent.
    // That legacy-compat fallback is correct for a write command but wrong for a *verify*
    // gate — a missing rules file means we cannot know which catalogues to check.
    //
    // We perform a single atomic read instead of calling resolve_layers:
    // 1. `reject_symlinks_below` checks for a symlink at the leaf or any ancestor.
    // 2. If `Ok(true)` → the file is present; read and parse it.
    // 3. If `Ok(false)` → the file is absent → fail-closed error (no fallback).
    // 4. `Err` → symlink or I/O error → propagate.
    // This eliminates the TOCTOU that a separate exists()-check + read() pair would have.
    let rules_path = workspace_root.join("architecture-rules.json");
    let bindings = match reject_symlinks_below(&rules_path, &workspace_root) {
        Ok(true) => {
            let content = std::fs::read_to_string(&rules_path).map_err(|e| {
                CliError::Message(format!(
                    "cannot read architecture-rules.json at '{}': {e}",
                    rules_path.display()
                ))
            })?;
            parse_tddd_layers(&content).map_err(|e| {
                CliError::Message(format!(
                    "architecture-rules.json parse error at '{}': {e}",
                    rules_path.display()
                ))
            })?
        }
        Ok(false) => {
            return Err(CliError::Message(format!(
                "architecture-rules.json not found at '{}'; \
                 cannot enumerate TDDD layers for verification",
                rules_path.display()
            )));
        }
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: architecture-rules.json at '{}': {e}",
                rules_path.display()
            )));
        }
    };
    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json".to_owned(),
        ));
    }

    // ADR D2.3 (file existence = phase status): silent PASS when no enabled
    // layer's catalogue file exists. Phase 0/1 tracks have no catalogue yet,
    // so spec.json is not a SoT Chain ② requirement. As soon as at least one
    // catalogue exists, spec.json becomes required and a missing spec.json
    // continues to fail (handled by `read_spec_element_hashes`).
    if !any_enabled_catalogue_present(&bindings, &track_dir, &items_dir)? {
        println!("[OK] catalogue-spec-refs: no findings");
        return Ok(ExitCode::SUCCESS);
    }

    let spec_element_hashes = read_spec_element_hashes(&track_dir, &items_dir)?;

    let mut all_findings: Vec<SpecRefFinding> = Vec::new();
    for binding in &bindings {
        if !binding.catalogue_spec_signal_enabled() {
            // ADR §D5.4 phased activation — skip layers that have not opted in.
            continue;
        }
        // Signals are read from `track_dir` (= `items_dir/<track_id>`) so that catalogue,
        // spec, and signals all come from the same directory tree. The CLI contract
        // (per `CatalogueSpecRefsArgs`) treats `--items-dir` as the root for all local
        // track artifacts including signals.  In the normal case `items_dir` equals
        // `workspace_root/track/items` (the path `FsCatalogueSpecSignalsStore` writes to),
        // so reader and writer agree. When `--items-dir` is overridden, the caller is
        // responsible for placing signals in `<items_dir>/<track_id>/`.
        let layer_findings = verify_one_layer(
            &track_dir,
            &items_dir,
            &valid_id,
            binding,
            &spec_element_hashes,
            skip_stale,
        )?;
        all_findings.extend(layer_findings);
    }

    if all_findings.is_empty() {
        println!("[OK] catalogue-spec-refs: no findings");
        Ok(ExitCode::SUCCESS)
    } else {
        for finding in &all_findings {
            eprintln!("{}", format_finding(finding));
        }
        eprintln!("[FAIL] catalogue-spec-refs: {} finding(s)", all_findings.len());
        Ok(ExitCode::FAILURE)
    }
}

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
/// Returns `CliError` when a symlink that escapes the trusted root is
/// detected for any candidate catalogue path.
fn any_enabled_catalogue_present(
    bindings: &[TdddLayerBinding],
    track_dir: &Path,
    items_dir: &Path,
) -> Result<bool, CliError> {
    for binding in bindings {
        if !binding.catalogue_spec_signal_enabled() {
            continue;
        }
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let present = reject_symlinks_below(&catalogue_path, items_dir).map_err(|e| {
            CliError::Message(format!(
                "symlink guard: refusing to read catalogue '{}' for layer '{}': {e}",
                catalogue_path.display(),
                binding.layer_id()
            ))
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
/// Returns `CliError` when the spec.json is absent, is a symlink, fails to
/// parse as JSON, or contains an element id that does not satisfy
/// `SpecElementId::try_new` (fail-closed: malformed ids abort the run rather
/// than silently producing an incomplete map that could hide findings).
fn read_spec_element_hashes(
    track_dir: &Path,
    items_dir: &Path,
) -> Result<BTreeMap<SpecElementId, ContentHash>, CliError> {
    let spec_path = track_dir.join("spec.json");

    // Symlink guard: reject symlinks at spec.json or any ancestor below items_dir.
    // This must run before the read so a dangling symlink is not silently treated
    // as "file missing" (mirrors the plan_artifact_refs.rs pattern).
    reject_symlinks_below(&spec_path, items_dir).map_err(|e| {
        CliError::Message(format!("symlink guard: spec.json at '{}': {e}", spec_path.display()))
    })?;

    let text = std::fs::read_to_string(&spec_path).map_err(|e| {
        CliError::Message(format!("cannot read spec.json at '{}': {e}", spec_path.display()))
    })?;
    // Validate schema first (mirrors `load_spec_element_map` in plan_artifact_refs.rs):
    // catches duplicate IDs, malformed required fields, and unsupported schema_version before
    // the element map is built.  Without this, an invalid spec.json silently produces an
    // empty or partial map, which causes valid catalogue refs to appear as dangling anchors.
    infrastructure::spec::codec::decode(&text).map_err(|e| {
        CliError::Message(format!("spec.json schema error at '{}': {e}", spec_path.display()))
    })?;
    let raw: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CliError::Message(format!("spec.json JSON parse error: {e}")))?;
    let element_map = infrastructure::verify::plan_artifact_refs::build_element_map(&raw);
    let mut out: BTreeMap<SpecElementId, ContentHash> = BTreeMap::new();
    for (id_str, canonical_json) in element_map {
        // Fail-closed: an element id that fails SpecElementId validation indicates a
        // malformed spec.json. Silently skipping it would produce an incomplete hash
        // map and could cause valid catalogue refs to appear as dangling anchors.
        let anchor = SpecElementId::try_new(id_str.clone()).map_err(|e| {
            CliError::Message(format!("spec.json contains element with invalid id '{id_str}': {e}"))
        })?;
        let hash_hex =
            infrastructure::verify::plan_artifact_refs::canonical_json_sha256(&canonical_json);
        let hash = ContentHash::try_from_hex(hash_hex)
            .map_err(|e| CliError::Message(format!("internal: canonical hash parse error: {e}")))?;
        out.insert(anchor, hash);
    }
    Ok(out)
}

/// Verify one layer — returns the list of `SpecRefFinding`s emitted by the
/// domain pure function.
///
/// All inputs (catalogue, spec hashes, signals) are resolved under `track_dir`
/// (= `items_dir/<track_id>`). `items_dir` is the symlink-guard trusted root for all reads.
/// The CLI contract (`CatalogueSpecRefsArgs`) treats `--items-dir` as the root for all
/// local track artifacts, so signals are co-located with the catalogue they describe.
fn verify_one_layer(
    track_dir: &Path,
    items_dir: &Path,
    _track_id: &domain::TrackId,
    binding: &TdddLayerBinding,
    spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>,
    skip_stale: bool,
) -> Result<Vec<SpecRefFinding>, CliError> {
    let layer_id = binding.layer_id();
    let layer_id_newtype = LayerId::try_new(layer_id)
        .map_err(|e| CliError::Message(format!("invalid layer id '{layer_id}': {e}")))?;
    let catalogue_path = track_dir.join(binding.catalogue_file());

    // Symlink guard (fail-closed): reject symlinks at the catalogue leaf and
    // every ancestor below items_dir. A dangling symlink returns Ok(false) via
    // `reject_symlinks_below`, which we map to the lenient "absent" skip case
    // (consistent with the `sotp track type-signals` CI path). A valid symlink
    // that resolves outside the workspace is rejected with Err — it is never
    // silently followed.
    let catalogue_present = reject_symlinks_below(&catalogue_path, items_dir).map_err(|e| {
        CliError::Message(format!(
            "symlink guard: refusing to read catalogue '{}' for layer '{layer_id}': {e}",
            catalogue_path.display()
        ))
    })?;

    if !catalogue_present {
        // Skip layers whose catalogue file is absent — consistent with the
        // `sotp track type-signals` lenient CI path.
        return Ok(Vec::new());
    }

    let bytes = std::fs::read(&catalogue_path).map_err(|e| {
        CliError::Message(format!("cannot read catalogue '{}': {e}", catalogue_path.display()))
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        CliError::Message(format!(
            "catalogue '{}' contains non-UTF-8 bytes: {e}",
            catalogue_path.display()
        ))
    })?;
    let catalogue = catalogue_codec::decode(text).map_err(|e| {
        CliError::Message(format!("cannot decode catalogue '{}': {e}", catalogue_path.display()))
    })?;

    let catalogue_hash_hex = type_signals_codec::declaration_hash(&bytes);
    let catalogue_hash = ContentHash::try_from_hex(&catalogue_hash_hex).map_err(|e| {
        CliError::Message(format!(
            "internal: catalogue hash for layer '{layer_id}' is not canonical hex: {e}"
        ))
    })?;

    let (current_hash_opt, signals_opt) = if skip_stale {
        (None, None)
    } else {
        // Signals are read from `track_dir` so catalogue, spec, and signals all come
        // from the same `items_dir`-rooted tree. `items_dir` is the trusted root.
        let signals_path = track_dir.join(format!("{layer_id}-catalogue-spec-signals.json"));
        let signals_present = reject_symlinks_below(&signals_path, items_dir).map_err(|e| {
            CliError::Message(format!(
                "symlink guard: refusing to read signals '{}' for layer '{layer_id}': {e}",
                signals_path.display()
            ))
        })?;
        if signals_present {
            let signals_text = std::fs::read_to_string(&signals_path).map_err(|e| {
                CliError::Message(format!("cannot read signals '{}': {e}", signals_path.display()))
            })?;
            let signals = catalogue_spec_signals_codec::decode(&signals_text).map_err(|e| {
                CliError::Message(format!(
                    "cannot decode signals '{}': {e}",
                    signals_path.display()
                ))
            })?;
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
fn format_finding(finding: &SpecRefFinding) -> String {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use super::*;

    fn write_architecture_rules(root: &Path) {
        let rules = serde_json::json!({
            "schema_version": 2,
            "layers": [
                {
                    "crate": "test_layer",
                    "path": "libs/test_layer",
                    "dependencies": [],
                    "deny_reason": "",
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "test_layer-types.json",
                        "catalogue_spec_signal": {
                            "enabled": true
                        }
                    }
                }
            ]
        });
        fs::write(
            root.join("architecture-rules.json"),
            serde_json::to_string_pretty(&rules).unwrap(),
        )
        .unwrap();
    }

    fn write_spec_json(track_dir: &Path) {
        let spec = serde_json::json!({
            "schema_version": 2,
            "version": "1.0",
            "title": "Test",
            "scope": {
                "in_scope": [{"id": "IN-01", "text": "Requirement A"}],
                "out_of_scope": []
            }
        });
        fs::write(track_dir.join("spec.json"), serde_json::to_string_pretty(&spec).unwrap())
            .unwrap();
    }

    fn write_catalogue_with_dangling(track_dir: &Path) {
        let cat = serde_json::json!({
            "schema_version": 2,
            "type_definitions": [
                {
                    "name": "BadType",
                    "description": "dangling anchor",
                    "approved": true,
                    "kind": "value_object",
                    "expected_methods": [],
                    "spec_refs": [
                        {
                            "file": "track/items/x/spec.json",
                            "anchor": "IN-99",
                            "hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        }
                    ]
                }
            ]
        });
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn verify_exits_0_when_no_catalogue_entries() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);
        // Catalogue has no entries → no findings.
        let cat = serde_json::json!({"schema_version": 2, "type_definitions": []});
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn verify_exits_failure_when_dangling_anchor_present() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);
        write_catalogue_with_dangling(&track_dir);

        let result = execute_verify_catalogue_spec_refs(
            items_dir,
            track_id.to_owned(),
            ws,
            true, // skip stale to isolate dangling detection
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::FAILURE);
    }

    #[test]
    fn verify_rejects_path_traversal_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let items_dir = ws.join("track/items");
        fs::create_dir_all(&items_dir).unwrap();
        write_architecture_rules(&ws);

        let result = execute_verify_catalogue_spec_refs(items_dir, "../evil".to_owned(), ws, true);
        assert!(result.is_err());
    }

    // Fail-closed regression guard: a non-existent track directory (typo or
    // stale CI variable) must NOT be silently swallowed by the Phase 0/1
    // catalogue-absent gate. Without an explicit existence check, every
    // catalogue path under the missing directory would resolve as absent and
    // `any_enabled_catalogue_present` would return false, producing a false
    // PASS. The verifier must surface a clear error instead.
    #[test]
    fn verify_fails_when_track_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let items_dir = ws.join("track/items");
        fs::create_dir_all(&items_dir).unwrap();
        write_architecture_rules(&ws);
        // Deliberately do NOT create the track directory.

        let result =
            execute_verify_catalogue_spec_refs(items_dir, "no-such-track".to_owned(), ws, true);
        assert!(result.is_err(), "non-existent track directory must fail-closed: {result:?}");
    }

    // ADR D2.3: catalogue absent + spec.json absent → silent PASS (Phase 0/1).
    // No catalogue means SoT Chain ② is not yet active, so the missing
    // spec.json is not a violation. Mirrors the `validate_track_snapshots`
    // file-existence-driven phase model.
    #[test]
    fn verify_passes_when_catalogue_absent_and_spec_absent() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        // No spec.json AND no catalogue → Phase 0/1 state.

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(
            result.is_ok(),
            "Phase 0/1 (no catalogue, no spec.json) must produce silent PASS: {result:?}"
        );
        assert_eq!(result.unwrap(), ExitCode::SUCCESS, "Phase 0/1 must produce zero findings");
    }

    // ADR D2.3: catalogue present + spec.json absent → FAIL (SoT Chain ②).
    // The catalogue's spec_refs[] cite anchor ids in spec.json — without
    // spec.json, ref integrity cannot be verified. Treat as a hard error to
    // surface the contract violation rather than silently bypassing.
    #[test]
    fn verify_fails_when_catalogue_present_and_spec_absent() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        // Catalogue present (any non-empty entry forces the spec.json read path).
        write_catalogue_with_dangling(&track_dir);
        // Deliberately no spec.json.

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(
            result.is_err(),
            "catalogue present + spec.json absent must FAIL (SoT Chain ② violation)"
        );
    }

    // Absent catalogue file for a layer must be silently skipped (lenient CI path).
    // This is distinct from an empty catalogue: the file does not exist at all.
    #[test]
    fn verify_exits_0_when_catalogue_file_absent_for_layer() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);
        // Deliberately do NOT write `test_layer-types.json`.

        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(result.is_ok(), "absent catalogue file must not be an error: {result:?}");
        assert_eq!(
            result.unwrap(),
            ExitCode::SUCCESS,
            "absent catalogue file must produce zero findings"
        );
    }

    // `--skip-stale` must prevent reading `<layer>-catalogue-spec-signals.json`
    // even when that file exists.  A stale-signals finding from the domain layer
    // would be the only finding if the signals file were read — so EXIT_SUCCESS
    // with skip_stale=true proves the signals file was not consulted.
    #[test]
    fn verify_skip_stale_bypasses_signals_read() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);

        // Empty catalogue (no spec_refs) → no dangling findings regardless of signals.
        let cat = serde_json::json!({"schema_version": 2, "type_definitions": []});
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();

        // Write a signals file with a mismatched catalogue_declaration_hash so that if it were
        // read it would produce a StaleSignals finding.
        let stale_signals = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "signals": []
        });
        fs::write(
            track_dir.join("test_layer-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&stale_signals).unwrap(),
        )
        .unwrap();

        // With skip_stale=true, the signals file must NOT be read → no stale finding.
        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, true);
        assert!(result.is_ok(), "skip_stale must not error: {result:?}");
        assert_eq!(
            result.unwrap(),
            ExitCode::SUCCESS,
            "skip_stale=true must bypass signals read and produce zero findings"
        );
    }

    // Catalogue with a valid spec element but wrong declared hash must produce a
    // HashMismatch finding (exit FAILURE).
    #[test]
    fn verify_exits_failure_when_hash_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);

        // Catalogue references valid anchor IN-01 but with a deliberately wrong hash.
        let cat = serde_json::json!({
            "schema_version": 2,
            "type_definitions": [
                {
                    "name": "GoodType",
                    "description": "valid anchor, wrong hash",
                    "approved": true,
                    "kind": "value_object",
                    "expected_methods": [],
                    "spec_refs": [
                        {
                            "file": "track/items/test-track/spec.json",
                            "anchor": "IN-01",
                            "hash": "0000000000000000000000000000000000000000000000000000000000000000"
                        }
                    ]
                }
            ]
        });
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();

        let result = execute_verify_catalogue_spec_refs(
            items_dir,
            track_id.to_owned(),
            ws,
            true, // skip stale to isolate hash-mismatch detection
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            ExitCode::FAILURE,
            "wrong declared hash must produce a hash-mismatch finding"
        );
    }

    // When skip_stale=false and the signals file exists with a mismatched
    // declaration_hash, a StaleSignals finding must be produced (exit FAILURE).
    #[test]
    fn verify_exits_failure_when_stale_signals_detected() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_architecture_rules(&ws);
        write_spec_json(&track_dir);

        // Empty catalogue — no spec_refs → no dangling or hash-mismatch findings.
        let cat = serde_json::json!({"schema_version": 2, "type_definitions": []});
        fs::write(
            track_dir.join("test_layer-types.json"),
            serde_json::to_string_pretty(&cat).unwrap(),
        )
        .unwrap();

        // Write a signals file with an obviously wrong catalogue_declaration_hash (all zeros).
        // The actual catalogue hash will differ → StaleSignals finding.
        let stale_signals = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "signals": []
        });
        fs::write(
            track_dir.join("test_layer-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&stale_signals).unwrap(),
        )
        .unwrap();

        // With skip_stale=false the signals file IS read → stale hash → FAILURE.
        let result = execute_verify_catalogue_spec_refs(items_dir, track_id.to_owned(), ws, false);
        assert!(result.is_ok(), "stale signals must not error, just return FAILURE: {result:?}");
        assert_eq!(
            result.unwrap(),
            ExitCode::FAILURE,
            "stale catalogue-spec-signals must produce a finding and exit FAILURE"
        );
    }

    #[test]
    fn format_finding_dangling_anchor_has_expected_format() {
        let finding = SpecRefFinding::new(
            LayerId::try_new("domain").unwrap(),
            SpecRefFindingKind::DanglingAnchor {
                catalogue_entry: "Foo".to_owned(),
                ref_index: 0,
                spec_file: PathBuf::from("track/items/x/spec.json"),
                anchor: SpecElementId::try_new("IN-99").unwrap(),
            },
        );
        let formatted = format_finding(&finding);
        assert!(formatted.contains("[layer=domain]"));
        assert!(formatted.contains("Foo[ref=0]"));
        assert!(formatted.contains("dangling anchor 'IN-99'"));
    }

    #[test]
    fn format_finding_hash_mismatch_has_expected_format() {
        let finding = SpecRefFinding::new(
            LayerId::try_new("usecase").unwrap(),
            SpecRefFindingKind::HashMismatch {
                catalogue_entry: "Bar".to_owned(),
                ref_index: 2,
                spec_file: PathBuf::from("track/items/x/spec.json"),
                anchor: SpecElementId::try_new("IN-01").unwrap(),
                declared: ContentHash::from_bytes([0xaa; 32]),
                actual: ContentHash::from_bytes([0xbb; 32]),
            },
        );
        let formatted = format_finding(&finding);
        assert!(formatted.contains("[layer=usecase]"));
        assert!(formatted.contains("Bar[ref=2]"));
        assert!(formatted.contains("hash mismatch for 'IN-01'"));
        assert!(formatted.contains("declared="));
        assert!(formatted.contains("actual="));
    }

    #[test]
    fn format_finding_stale_signals_has_expected_format() {
        let finding = SpecRefFinding::new(
            LayerId::try_new("domain").unwrap(),
            SpecRefFindingKind::StaleSignals {
                declared_catalogue_hash: ContentHash::from_bytes([0x11; 32]),
                actual_catalogue_hash: ContentHash::from_bytes([0x22; 32]),
            },
        );
        let formatted = format_finding(&finding);
        assert!(formatted.contains("[layer=domain]"));
        assert!(formatted.contains("stale catalogue-spec-signals"));
    }
}
