//! Verify structured-ref fields introduced in T002 / T003 / T005, plus
//! task-coverage enforcement and canonical-block suspicion detection (T011).
//!
//! This module validates plan artifact references in a track directory:
//! - `spec.json` ref fields: `adr_refs`, `convention_refs`, `informal_grounds`,
//!   `related_conventions`, and per-catalogue-entry `spec_refs` / `informal_grounds`
//! - File existence for file-based refs (`AdrRef.file`, `ConventionRef.file`, `SpecRef.file`)
//! - `SpecRef.anchor` must resolve to a real element id inside the target `spec.json`
//! - `SpecRef.hash` must match the SHA-256 of the canonical JSON subtree
//! - `AdrAnchor` / `ConventionAnchor` loose non-empty validation (already enforced by newtypes)
//! - `InformalGroundRef` newtype validation (kind variant + non-empty summary)
//! - `task-coverage.json` coverage enforcement (in_scope + acceptance_criteria must have task_refs)
//! - `task-coverage.json` referential integrity (all 4 sections: element ids in spec, task ids in impl-plan)
//! - Canonical-block suspicion detection in `plan.md` + `verification.md` (warning only)
//!
//! Per ADR 2026-04-19-1242 §D2.3 / §D3.3.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};
use thiserror::Error;

use crate::spec::codec as spec_codec;
use crate::tddd::catalogue_codec;
use crate::track::symlink_guard;

/// Errors specific to the `plan-artifact-refs` verifier.
///
/// These are surfaced as `VerifyFinding::error` entries in the outcome,
/// not propagated directly to callers.
#[derive(Debug, Error)]
pub enum PlanArtifactRefsError {
    /// JSON parse failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O failure while reading a file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A `SpecRef.anchor` does not resolve to any element in the target spec.
    #[error("unresolved SpecRef anchor '{anchor}' in file '{}'", file.display())]
    UnresolvedSpecRef { file: PathBuf, anchor: String },

    /// `SpecRef.hash` does not match the actual SHA-256 of the element subtree.
    #[error(
        "SpecRef hash mismatch for anchor '{anchor}' in '{}': expected {expected}, actual {actual}",
        file.display()
    )]
    SpecHashMismatch { file: PathBuf, anchor: String, expected: String, actual: String },

    /// An anchor value failed validation (non-empty check).
    #[error("invalid anchor '{anchor}' in file '{}'", file.display())]
    InvalidAnchor { file: PathBuf, anchor: String },

    /// A task-coverage coverage or referential-integrity constraint was violated (T011).
    #[error("coverage violation: {0}")]
    CoverageViolation(String),
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Verify all structured-ref fields in a track directory.
///
/// Reads `spec.json` plus all layer type catalogues
/// (`domain-types.json`, `usecase-types.json`, `infrastructure-types.json`)
/// when present. Validates:
/// - Schema decode of every ref field (malformed refs → error finding)
/// - File existence for file-based refs
/// - `SpecRef.anchor` resolution in the target spec.json
/// - `SpecRef.hash` matching the canonical JSON subtree SHA-256
/// - `AdrAnchor` / `ConventionAnchor` loose validation (non-empty, enforced by newtype)
/// - `InformalGroundRef` newtype validation (kind + non-empty summary)
///
/// Skips silently when `spec.json` is absent (track not yet past Phase 1).
///
/// # Errors
///
/// Returns error-level findings when any ref field fails the checks above.
pub fn verify(track_dir: &Path) -> VerifyOutcome {
    // Absolutize `track_dir` without following symlinks (CWD join, no resolution).
    // All subsequent file paths are built from this absolute, lexical path so that
    // `reject_symlinks_below` can correctly compare ancestor chains.
    let abs_track_dir = super::trusted_root::absolutize(track_dir);

    // Resolve the trusted_root via the shared resolver, which uses git discovery
    // (`SystemGitRepo::discover()`) anchored at the actual repository root and
    // verifies the result is not itself a symlink. This is more robust than
    // manually walking N parent directories (which depends on `track_dir` being
    // exactly 3 levels deep) and catches symlinked intermediate path components
    // that a parent-walk would miss.
    let spec_json_path_for_discovery = abs_track_dir.join("spec.json");
    let trusted_root =
        match super::trusted_root::resolve_trusted_root(&spec_json_path_for_discovery) {
            Ok(r) => r,
            Err(e) => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "Refusing to verify: trusted_root resolution failed for track '{}': {e}",
                    track_dir.display()
                ))]);
            }
        };

    // Build catalogue-local paths from the absolutized track dir.
    // Repo-relative ref paths are resolved against `trusted_root` directly (inside
    // `resolve_path`), not against a derived parent walk, so both the containment
    // check and the symlink guard use the same root.
    let spec_json_path = abs_track_dir.join("spec.json");

    // -----------------------------------------------------------------------
    // 1. Load and validate spec.json (symlink-guarded)
    //
    // Run the symlink guard BEFORE the existence check so that a symlinked
    // (non-regular) `spec.json` is rejected rather than being treated as
    // absent. `Path::is_file()` follows symlinks and would return `false` for
    // a symlink-to-directory, silently skipping all artifact checks. By
    // calling `reject_symlinks_below` first, any symlink at or above the
    // path returns an error finding, and only a missing (non-existent) file
    // produces `pass()`.
    // -----------------------------------------------------------------------
    match symlink_guard::reject_symlinks_below(&spec_json_path, &trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            // spec.json absent — track not yet past Phase 1; skip silently.
            return VerifyOutcome::pass();
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "spec.json symlink guard: {e}"
            ))]);
        }
    }

    let mut findings: Vec<VerifyFinding> = Vec::new();

    let spec_content = match std::fs::read_to_string(&spec_json_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read spec.json: {e}"
            ))]);
        }
    };

    // The spec codec already validates all ref newtypes (AdrAnchor, ConventionAnchor,
    // InformalGroundKind, InformalGroundSummary, SpecElementId).
    // Codec errors here surface malformed ref JSON.
    let spec_doc = match spec_codec::decode(&spec_content) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse spec.json: {e}"
            ))]);
        }
    };

    // -----------------------------------------------------------------------
    // 2. Build a look-up map from spec element id → raw JSON subtree
    //    for SpecRef.anchor resolution and hash verification.
    //    This is loaded lazily per referenced spec file.
    // -----------------------------------------------------------------------
    let mut spec_element_cache: HashMap<PathBuf, SpecElementMap> = HashMap::new();

    // -----------------------------------------------------------------------
    // 3. Validate file-based refs in spec.json requirement sections
    // -----------------------------------------------------------------------
    let all_requirements: Vec<&domain::SpecRequirement> = spec_doc
        .goal()
        .iter()
        .chain(spec_doc.scope().in_scope().iter())
        .chain(spec_doc.scope().out_of_scope().iter())
        .chain(spec_doc.constraints().iter())
        .chain(spec_doc.acceptance_criteria().iter())
        .collect();

    for req in &all_requirements {
        for adr_ref in req.adr_refs() {
            check_ref_file(&adr_ref.file, &trusted_root, "spec.json adr_ref", &mut findings);
        }
        for conv_ref in req.convention_refs() {
            check_ref_file(
                &conv_ref.file,
                &trusted_root,
                "spec.json convention_ref",
                &mut findings,
            );
        }
        // InformalGroundRef: no file path; kind+summary already validated by newtype constructor.
    }

    // Validate top-level related_conventions
    for conv_ref in spec_doc.related_conventions() {
        check_ref_file(
            &conv_ref.file,
            &trusted_root,
            "spec.json related_conventions",
            &mut findings,
        );
    }

    // -----------------------------------------------------------------------
    // 4. Validate SpecRef fields in type catalogue entries
    //    Walk domain-, usecase-, infrastructure-types.json
    // -----------------------------------------------------------------------
    let catalogue_files = ["domain-types.json", "usecase-types.json", "infrastructure-types.json"];

    for catalogue_name in &catalogue_files {
        let catalogue_path = abs_track_dir.join(catalogue_name);

        // Symlink-guard catalogue file before reading.
        match symlink_guard::reject_symlinks_below(&catalogue_path, &trusted_root) {
            Ok(false) => continue, // catalogue absent — layer not active
            Ok(true) => {}
            Err(e) => {
                findings.push(VerifyFinding::error(format!("{catalogue_name} symlink guard: {e}")));
                continue;
            }
        }

        let catalogue_content = match std::fs::read_to_string(&catalogue_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(VerifyFinding::error(format!("Cannot read {catalogue_name}: {e}")));
                continue;
            }
        };

        // The catalogue codec already validates SpecRef newtypes (SpecElementId, ContentHash)
        // and InformalGroundRef (kind variant + non-empty summary).
        let catalogue_doc = match catalogue_codec::decode(&catalogue_content) {
            Ok(d) => d,
            Err(e) => {
                findings.push(VerifyFinding::error(format!("Cannot parse {catalogue_name}: {e}")));
                continue;
            }
        };

        for entry in catalogue_doc.entries() {
            for spec_ref in entry.spec_refs() {
                // Path-traversal guard: resolve the ref path and verify containment.
                let resolved = match resolve_path(&trusted_root, &spec_ref.file) {
                    None => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{}': spec_ref has invalid path \
                             (absolute or path-traversal): {}",
                            entry.name(),
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                    Some(p) => p,
                };

                // Symlink-guard and existence check for the referenced spec file.
                match symlink_guard::reject_symlinks_below(&resolved, &trusted_root) {
                    Ok(false) => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{}': spec_ref file not found: {}",
                            entry.name(),
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                    Ok(true) => {}
                    Err(e) => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{}': spec_ref symlink guard for '{}': {e}",
                            entry.name(),
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                }

                // Load and cache the referenced spec.json element map
                let element_map = match load_spec_element_map(
                    &mut spec_element_cache,
                    &resolved,
                    &trusted_root,
                ) {
                    Ok(m) => m,
                    Err(e) => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{}': cannot load spec file '{}': {e}",
                            entry.name(),
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                };

                // Anchor resolution: SpecRef.anchor must map to an element in the spec
                let anchor_str = spec_ref.anchor.as_ref();
                match element_map.get(anchor_str) {
                    None => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{}': unresolved SpecRef anchor '{}' in '{}'",
                            entry.name(),
                            anchor_str,
                            spec_ref.file.display()
                        )));
                    }
                    Some(subtree_json) => {
                        // Hash verification: compare stored hash to actual SHA-256
                        let actual_hash = canonical_json_sha256(subtree_json);
                        let expected_hash = spec_ref.hash.to_hex();
                        if actual_hash != expected_hash {
                            findings.push(VerifyFinding::error(format!(
                                "{catalogue_name} entry '{}': SpecRef hash mismatch for anchor '{}' \
                                 in '{}': expected {expected_hash}, actual {actual_hash}",
                                entry.name(),
                                anchor_str,
                                spec_ref.file.display()
                            )));
                        }
                    }
                }
            }
            // InformalGroundRef: kind+summary already validated by codec
        }
    }

    // -----------------------------------------------------------------------
    // 5. task-coverage.json coverage enforcement + referential integrity
    //    Migrated from spec_coverage::verify (T011).
    //    When task-coverage.json is absent: warn (soft failure for now).
    //    When present: enforce coverage + referential integrity.
    // -----------------------------------------------------------------------
    let task_coverage_path = abs_track_dir.join("task-coverage.json");
    match symlink_guard::reject_symlinks_below(&task_coverage_path, &trusted_root) {
        Ok(false) => {
            // task-coverage.json absent — emit warning, not error.
            // T012 will make it required; T011 keeps existing tracks functional.
            // impl-plan.json integrity is only enforced when task-coverage.json is present
            // (integrity validates task refs that appear in coverage; no coverage → no refs
            // to validate).
            findings.push(VerifyFinding::warning(
                "task-coverage.json absent — coverage enforcement deferred until T012 makes it required",
            ));
        }
        Ok(true) => {
            // task-coverage.json present — run coverage + referential integrity.
            verify_task_coverage(
                &abs_track_dir,
                &task_coverage_path,
                &spec_doc,
                &trusted_root,
                &mut findings,
            );
        }
        Err(e) => {
            findings.push(VerifyFinding::error(format!("task-coverage.json symlink guard: {e}")));
        }
    }

    // -----------------------------------------------------------------------
    // 6. Canonical-block suspicion detection in plan.md + verification.md
    //    Warning-only: long fenced code blocks >10 lines that lack an
    //    "example" marker may be canonical blocks leaking into rendered docs.
    // -----------------------------------------------------------------------
    for doc_file in &["plan.md", "verification.md"] {
        let doc_path = abs_track_dir.join(doc_file);
        match symlink_guard::reject_symlinks_below(&doc_path, &trusted_root) {
            Ok(false) => {} // absent — skip silently
            Ok(true) => {
                scan_canonical_block_suspicion(&doc_path, doc_file, &mut findings);
            }
            Err(e) => {
                // Symlink-guard failures are security controls: fail at error level,
                // consistent with all other symlink-guard checks in this verifier.
                findings.push(VerifyFinding::error(format!(
                    "{doc_file} symlink guard (canonical-block scan): {e}"
                )));
            }
        }
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Mapping from spec element id string → canonical JSON representation of the element subtree.
type SpecElementMap = HashMap<String, String>;

/// Resolve a file-based ref path, guard against symlinks, and push a finding if the
/// file is absent or unsafe.
///
/// Used for `AdrRef.file` and `ConventionRef.file` in spec.json.
/// `InformalGroundRef` has no file path and is not validated here.
///
/// Steps:
/// 1. `resolve_path` — reject absolute paths and `../` escapes outside the repo root.
/// 2. `symlink_guard::reject_symlinks_below` — reject symlinks anywhere on the path.
/// 3. Absence of the resolved file → push a "file not found" finding.
fn check_ref_file(
    file: &Path,
    trusted_root: &Path,
    context: &str,
    findings: &mut Vec<VerifyFinding>,
) {
    match resolve_path(trusted_root, file) {
        None => {
            findings.push(VerifyFinding::error(format!(
                "{context}: invalid path (absolute or path-traversal): {}",
                file.display()
            )));
        }
        Some(p) => match symlink_guard::reject_symlinks_below(&p, trusted_root) {
            Ok(true) => {}
            Ok(false) => {
                findings.push(VerifyFinding::error(format!(
                    "{context}: file not found: {}",
                    file.display()
                )));
            }
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "{context}: symlink guard for '{}': {e}",
                    file.display()
                )));
            }
        },
    }
}

/// Resolve a repo-relative path against the repository root.
///
/// Paths in ref fields must be repo-relative (e.g.
/// `"knowledge/adr/2026-04-19-1242.md"`). `repo_root` is the authoritative
/// repository root produced by `resolve_trusted_root()` in `verify()` — the same
/// root used by the symlink guard.  Passing `repo_root` directly avoids the
/// split that would arise from a separate 3-parent walk inside this helper.
///
/// Returns `None` when:
/// - `file` is absolute (absolute paths are not valid ref paths).
/// - `file` contains `..` components (symlink-hiding path-traversal).
/// - `file` has no `Normal` components (empty or `.`-only path).
/// - The resolved path would escape `repo_root` (path-traversal guard).
///
/// Lexical normalization collapses `.` components without requiring the target
/// file to exist.  Using `canonicalize()` here would follow symlinks inside the
/// path, which could shift the containment boundary to a symlink target.
fn resolve_path(repo_root: &Path, file: &Path) -> Option<PathBuf> {
    use std::path::Component;

    if file.is_absolute() {
        return None;
    }

    // Reject any `..` components in the ref path before normalization.
    //
    // Lexical normalization (below) collapses `..` components without resolving
    // symlinks. A crafted ref like `dir/symlink/../target.md` would normalize
    // to `dir/target.md`, removing `symlink` from the path before
    // `reject_symlinks_below` runs — hiding a potential symlink traversal.
    //
    // Legitimate ref paths (e.g., `knowledge/adr/2026-04-19-1242.md`) never
    // contain `..` components. Rejecting them here is safe and prevents the
    // intermediate-symlink bypass entirely.
    if file.components().any(|c| c == Component::ParentDir) {
        return None;
    }

    // Reject empty paths or paths composed entirely of `.` (CurDir) components.
    //
    // An empty path or `"."` joined to `repo_root` yields the repository root
    // itself, which is never a valid ref target.  After the `..` guard above,
    // a valid ref must contain at least one Normal component.
    if !file.components().any(|c| matches!(c, Component::Normal(_))) {
        return None;
    }

    // `repo_root` is already the absolute, verified repository root (produced by
    // `resolve_trusted_root()` in `verify()`).  Join the ref path directly rather
    // than deriving the root from `track_dir` via a hard-coded parent walk.  Both
    // the containment check below and the symlink guard at the call site therefore
    // operate against the same root, eliminating the split that a separate parent
    // walk would introduce.
    let joined = repo_root.join(file);

    // Lexically normalize the joined path by processing each component.
    // This collapses `.` entries without requiring the file to exist.
    // `..` components cannot appear here: they are rejected above.
    let mut normalized: Vec<std::ffi::OsString> = Vec::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                if normalized.is_empty() {
                    // `..` would escape the root of the normalized path — reject.
                    return None;
                }
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str().to_os_string()),
        }
    }

    let normalized_path: PathBuf = normalized.into_iter().collect();

    // Containment check: normalized path must start with repo_root (which is
    // the same root used by the symlink guard at the call site).
    if normalized_path.starts_with(repo_root) { Some(normalized_path) } else { None }
}

/// Load a spec.json file and build a `SpecElementMap` (id → canonical JSON subtree).
///
/// Results are cached in `cache` by the absolute path of the spec file to
/// avoid re-reading the same file for every catalogue entry that references it.
///
/// The function validates the referenced spec.json through `spec_codec::decode()`
/// to catch schema violations (e.g. duplicate element IDs, malformed required
/// fields) before building the element map. The element map is still built from
/// the raw `serde_json::Value` so that hash values match the canonical JSON of
/// the bytes on disk — not a re-encoded form.
fn load_spec_element_map<'c>(
    cache: &'c mut HashMap<PathBuf, SpecElementMap>,
    spec_path: &Path,
    _trusted_root: &Path,
) -> Result<&'c SpecElementMap, String> {
    // Normalise to absolute path for cache key.
    let key = spec_path.canonicalize().unwrap_or_else(|_| spec_path.to_path_buf());

    // Use entry API to avoid the double-lookup / borrow checker conflict.
    if !cache.contains_key(&key) {
        let content = std::fs::read_to_string(spec_path).map_err(|e| format!("I/O error: {e}"))?;
        // Validate schema first: catches duplicate IDs, malformed fields, etc.
        spec_codec::decode(&content)
            .map_err(|e| format!("spec.json schema error in '{}': {e}", spec_path.display()))?;
        // Build element map from raw JSON so hashes reflect the actual on-disk bytes.
        let raw: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("JSON error: {e}"))?;
        let map = build_element_map(&raw);
        cache.insert(key.clone(), map);
    }

    // Safety: we just inserted if absent; the map always exists at this point.
    // Use .ok_or_else to avoid any possibility of panic in non-test code.
    cache.get(&key).ok_or_else(|| "internal cache error: key missing after insert".to_owned())
}

/// Extract every `id`-bearing requirement from a spec.json `serde_json::Value`
/// and return a map from id-string → canonical JSON of the containing requirement
/// object.
///
/// Walks: `goal[]`, `scope.in_scope[]`, `scope.out_of_scope[]`, `constraints[]`,
/// `acceptance_criteria[]`.
fn build_element_map(raw: &serde_json::Value) -> SpecElementMap {
    let mut map = HashMap::new();

    let sections: &[&str] = &["goal", "constraints", "acceptance_criteria"];

    for &section in sections {
        if let Some(arr) = raw.get(section).and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    let canonical = canonical_json(item);
                    map.insert(id.to_owned(), canonical);
                }
            }
        }
    }

    // scope.in_scope and scope.out_of_scope
    if let Some(scope) = raw.get("scope") {
        for &sub in &["in_scope", "out_of_scope"] {
            if let Some(arr) = scope.get(sub).and_then(|v| v.as_array()) {
                for item in arr {
                    if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                        let canonical = canonical_json(item);
                        map.insert(id.to_owned(), canonical);
                    }
                }
            }
        }
    }

    map
}

/// Produce a deterministic canonical JSON string for a `serde_json::Value`.
///
/// Object keys are sorted recursively; all other value types are encoded
/// as-is. The output is compact (no extra whitespace) so the SHA-256 digest
/// is stable across reformatting.
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut pairs: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{}:{}", escape_json_string(k), canonical_json(v)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// Escape a JSON key string (minimal — only handles chars that serde would quote).
fn escape_json_string(s: &str) -> String {
    // Re-serialize via serde_json to get correct escaping.
    serde_json::Value::String(s.to_owned()).to_string()
}

/// Compute SHA-256 of a canonical JSON string and return a 64-char lowercase hex.
fn canonical_json_sha256(json: &str) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(json.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// T011: task-coverage enforcement helper
// ---------------------------------------------------------------------------

/// Run coverage enforcement + referential integrity checks for `task-coverage.json`.
///
/// Called from `verify()` when `task-coverage.json` is confirmed present and
/// passes the symlink guard.
///
/// Checks:
/// 1. Coverage: every `in_scope` and `acceptance_criteria` requirement in `spec.json`
///    must have at least one task_ref entry in `task-coverage.json`.
/// 2. Referential integrity (spec elements): every SpecElementId key present in any
///    section of `task-coverage.json` must resolve to an element in `spec.json`.
/// 3. Referential integrity (task ids): every TaskId value in any section must exist
///    in `impl-plan.json` (skipped when `impl-plan.json` is absent).
fn verify_task_coverage(
    track_dir: &Path,
    task_coverage_path: &Path,
    spec_doc: &domain::SpecDocument,
    trusted_root: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    // Load task-coverage.json
    let task_coverage_content = match std::fs::read_to_string(task_coverage_path) {
        Ok(c) => c,
        Err(e) => {
            findings.push(VerifyFinding::error(format!("Cannot read task-coverage.json: {e}")));
            return;
        }
    };
    let task_coverage_doc = match crate::task_coverage_codec::decode(&task_coverage_content) {
        Ok(d) => d,
        Err(e) => {
            findings.push(VerifyFinding::error(format!("Cannot parse task-coverage.json: {e}")));
            return;
        }
    };

    // -----------------------------------------------------------------------
    // 5a. Coverage enforcement: in_scope requirements
    // -----------------------------------------------------------------------
    for req in spec_doc.scope().in_scope() {
        let covered =
            task_coverage_doc.in_scope().get(req.id()).is_some_and(|refs| !refs.is_empty());
        if !covered {
            findings.push(VerifyFinding::error(format!(
                "coverage violation: in_scope requirement \"{}\" (id: {}) has no task_refs in task-coverage.json",
                req.text(),
                req.id()
            )));
        }
    }

    // -----------------------------------------------------------------------
    // 5b. Coverage enforcement: acceptance_criteria requirements
    // -----------------------------------------------------------------------
    for req in spec_doc.acceptance_criteria() {
        let covered = task_coverage_doc
            .acceptance_criteria()
            .get(req.id())
            .is_some_and(|refs| !refs.is_empty());
        if !covered {
            findings.push(VerifyFinding::error(format!(
                "coverage violation: acceptance_criteria requirement \"{}\" (id: {}) has no task_refs in task-coverage.json",
                req.text(),
                req.id()
            )));
        }
    }

    // -----------------------------------------------------------------------
    // 5c. Referential integrity: spec element ids
    //
    // Each task-coverage section is validated against its matching spec section.
    // `goal` is intentionally excluded because task-coverage.json has no `goal`
    // section — goal requirements are not tracked at the task level.
    // Cross-section mappings (e.g., a constraints ID in the in_scope map) are
    // flagged as referential integrity errors.
    // -----------------------------------------------------------------------
    let in_scope_ids: HashSet<String> =
        spec_doc.scope().in_scope().iter().map(|r| r.id().as_ref().to_owned()).collect();
    let out_of_scope_ids: HashSet<String> =
        spec_doc.scope().out_of_scope().iter().map(|r| r.id().as_ref().to_owned()).collect();
    let constraints_ids: HashSet<String> =
        spec_doc.constraints().iter().map(|r| r.id().as_ref().to_owned()).collect();
    let acceptance_criteria_ids: HashSet<String> =
        spec_doc.acceptance_criteria().iter().map(|r| r.id().as_ref().to_owned()).collect();

    // Validate referential integrity for each section against its matching spec section.
    check_section_integrity("in_scope", task_coverage_doc.in_scope(), &in_scope_ids, findings);
    check_section_integrity(
        "acceptance_criteria",
        task_coverage_doc.acceptance_criteria(),
        &acceptance_criteria_ids,
        findings,
    );
    check_section_integrity(
        "out_of_scope",
        task_coverage_doc.out_of_scope(),
        &out_of_scope_ids,
        findings,
    );
    check_section_integrity(
        "constraints",
        task_coverage_doc.constraints(),
        &constraints_ids,
        findings,
    );

    // -----------------------------------------------------------------------
    // 5d. Referential integrity: task ids against impl-plan.json
    //
    // impl-plan.json is optional; when absent, skip entirely.
    // When present but unreadable/malformed, fail closed.
    // -----------------------------------------------------------------------
    let impl_plan_path = track_dir.join("impl-plan.json");
    match symlink_guard::reject_symlinks_below(&impl_plan_path, trusted_root) {
        Ok(false) => {} // impl-plan.json absent — skip task-id integrity
        Ok(true) => match load_impl_plan_task_ids_from_path(&impl_plan_path) {
            Ok(valid_task_ids) => {
                check_task_id_integrity(&task_coverage_doc, &valid_task_ids, findings);
            }
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "Cannot load impl-plan.json for referential-integrity check: {e}"
                )));
            }
        },
        Err(e) => {
            findings.push(VerifyFinding::error(format!("impl-plan.json symlink guard: {e}")));
        }
    }
}

/// Validate that every SpecElementId key in a single task-coverage section matches
/// an element in the corresponding spec section.
///
/// Emits error findings for any key not found in `valid_ids`.
fn check_section_integrity(
    section_name: &str,
    section_map: &std::collections::BTreeMap<domain::SpecElementId, Vec<domain::TaskId>>,
    valid_ids: &HashSet<String>,
    findings: &mut Vec<VerifyFinding>,
) {
    for req_id in section_map.keys() {
        let id_str = req_id.as_ref();
        if !valid_ids.contains(id_str) {
            findings.push(VerifyFinding::error(format!(
                "coverage violation: task-coverage.json \
                 {section_name}[\"{id_str}\"] references an element id that does not exist \
                 in spec.json {section_name} section"
            )));
        }
    }
}

/// Validate that every TaskId in all four task-coverage sections exists in `valid_task_ids`.
///
/// Emits error findings for any task_ref not found in the impl-plan task set.
fn check_task_id_integrity(
    task_coverage_doc: &domain::TaskCoverageDocument,
    valid_task_ids: &HashSet<domain::TaskId>,
    findings: &mut Vec<VerifyFinding>,
) {
    let sections: [(&str, &std::collections::BTreeMap<domain::SpecElementId, Vec<domain::TaskId>>);
        4] = [
        ("in_scope", task_coverage_doc.in_scope()),
        ("acceptance_criteria", task_coverage_doc.acceptance_criteria()),
        ("out_of_scope", task_coverage_doc.out_of_scope()),
        ("constraints", task_coverage_doc.constraints()),
    ];
    for (section_name, section_map) in &sections {
        for (req_id, task_refs) in *section_map {
            let id_str = req_id.as_ref();
            for task_ref in task_refs {
                if !valid_task_ids.contains(task_ref) {
                    findings.push(VerifyFinding::error(format!(
                        "coverage violation: task_ref \"{task_ref}\" in \
                         {section_name}[\"{id_str}\"] does not exist in impl-plan.json"
                    )));
                }
            }
        }
    }
}

/// Load task IDs from `impl-plan.json` at the given path.
///
/// Returns `Ok(ids)` when decoded successfully (may be empty — an empty plan
/// means every task_ref is invalid).
/// Returns `Err(message)` when the file is unreadable or malformed.
fn load_impl_plan_task_ids_from_path(path: &Path) -> Result<HashSet<domain::TaskId>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc = crate::impl_plan_codec::decode(&content)
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
    Ok(doc.tasks().iter().map(|t| t.id().clone()).collect())
}

// ---------------------------------------------------------------------------
// T011: canonical-block suspicion detection helper
// ---------------------------------------------------------------------------

/// Scan a Markdown file for fenced code blocks longer than 10 lines.
///
/// Emits a `VerifyFinding::warning` for each suspicious block (file + line number).
/// Blocks with an ADR-style "example" marker are excluded. The marker is recognized in:
/// 1. The line immediately preceding the opening fence.
/// 2. The info string on the opening fence line itself (e.g., ` ```example `).
///
/// Inner-line scanning is intentionally excluded to avoid false negatives from block
/// content that happens to contain example-marker text.
///
/// Each line is normalized by `fence_line_normalize` before fence detection: blockquote
/// `> ` markers are stripped recursively and all leading whitespace is removed. The same
/// normalization is applied consistently to every line — opening fence, inner body lines,
/// and closing fence — so that fenced blocks at any container depth are detected and
/// their boundaries are correctly matched.
///
/// Known accepted deviation: `plan.md` and `verification.md` are flat rendered views
/// generated by `sotp`. They do not use CommonMark indented code blocks (4-space indent)
/// or embed literal fence delimiters (`` ``` ``, `~~~`) as body content inside fenced
/// blocks. Treating these edge cases as fence delimiters would result in spurious
/// warnings, but this is acceptable because: (a) the patterns never appear in practice
/// in the target files, and (b) the canonical-block warning is suppression-only and
/// never fails CI — false positives can always be suppressed with an example marker.
///
/// Recognized preceding-line markers (case-insensitive, matched on normalized text):
/// - `<!-- illustrative, non-canonical -->` — canonical ADR Q3 marker (ADR 2026-04-19-1242 §Q3)
/// - `<!-- example -->` or `<!-- example:` — HTML comment markers
/// - `// example` — C-style line comment (word-boundary match)
///
/// Fence-open inline marker (case-insensitive info string, word-boundary match):
/// - ` ```example ` / ` ~~~example ` — "example" as a whole word in the info string
fn scan_canonical_block_suspicion(
    doc_path: &Path,
    doc_label: &str,
    findings: &mut Vec<VerifyFinding>,
) {
    let content = match std::fs::read_to_string(doc_path) {
        Ok(c) => c,
        Err(e) => {
            // Symlink guard already confirmed the file is present; a read failure here
            // is unexpected and should fail at error level, consistent with other I/O
            // failures in this verifier.
            findings.push(VerifyFinding::error(format!(
                "canonical-block scan: cannot read {doc_label}: {e}"
            )));
            return;
        }
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        // Safe: loop invariant guarantees i < lines.len()
        let raw_line = lines.get(i).copied().unwrap_or("");

        // Opening-fence detection: strip blockquote prefixes and ALL leading whitespace
        // so that fenced blocks at any container depth are visible.
        let line = fence_line_normalize(raw_line);

        // Detect opening fence: line starting with ``` or ~~~ (CommonMark fenced code block).
        // Returns both the fence character and the opening fence length (>= 3) so that
        // the closing fence can be validated per CommonMark (must be >= opening length).
        if let Some((fence_kind, open_len)) = detect_fence_open(line.as_str()) {
            let fence_start = i;
            let preceding_line = if i > 0 { lines.get(i - 1).copied() } else { None };

            // Example markers are recognized in two places only, to avoid false negatives
            // from block content that happens to contain marker text:
            //   1. The line immediately preceding the opening fence.
            //   2. The info string on the opening fence line itself (e.g., "```example").
            // Inner-line scanning is intentionally excluded.
            let fence_open_has_example = fence_info_has_example(line.as_str());

            // Closing-fence detection: apply the same `fence_line_normalize` as opening.
            // This is consistent: blockquote-level fences find their closers correctly,
            // and the scanner applies the same transformation at every container depth.
            // `plan.md` and `verification.md` are generated flat views that do not
            // contain `` ``` `` as literal text inside fenced code block bodies.
            let mut j = i + 1;
            while j < lines.len() {
                // Safe: loop invariant guarantees j < lines.len()
                let inner_raw = lines.get(j).copied().unwrap_or("");
                let inner = fence_line_normalize(inner_raw);
                if is_fence_close(inner.as_str(), fence_kind, open_len) {
                    break;
                }
                j += 1;
            }

            // Count lines inside the fence (excluding opening and closing fence lines).
            let block_line_count = j.saturating_sub(fence_start + 1);

            if block_line_count > 10 {
                // Normalize the preceding line (strip container prefixes) before checking
                // for an example marker so that markers inside blockquotes are recognized.
                let preceding_has_example = preceding_line
                    .is_some_and(|l| is_example_marker(fence_line_normalize(l).as_str()));

                if !preceding_has_example && !fence_open_has_example {
                    findings.push(VerifyFinding::warning(format!(
                        "canonical-block suspicion: {doc_label} line {} has a fenced code block \
                         with {block_line_count} lines — may be a canonical block leaking into a \
                         rendered view (add an example marker to suppress)",
                        fence_start + 1
                    )));
                }
            }

            // Advance past the closing fence (or end of file if unclosed).
            i = if j < lines.len() { j + 1 } else { j };
        } else {
            i += 1;
        }
    }
}

/// Normalize a raw Markdown line for fence detection.
///
/// 1. Strips blockquote `> ` markers (0-3 leading spaces + `>` + optional space) recursively
///    so that fenced blocks inside blockquotes are detected.
/// 2. Strips all remaining leading and trailing whitespace.
///
/// Step 2 means that indented lines (e.g., 4-space indented) are also treated as fence
/// candidates after blockquote stripping. This is intentional for `plan.md` and
/// `verification.md`, which are flat rendered views generated by `sotp` and do not use
/// CommonMark indented code blocks in practice. The same normalization is applied to
/// every line — opening fence, inner body lines, and closing fence — so the scanner
/// is self-consistent across all container depths.
fn fence_line_normalize(line: &str) -> String {
    strip_container_prefix(line).trim().to_owned()
}

/// Strip CommonMark blockquote container prefixes from a line.
///
/// A blockquote prefix consists of 0-3 leading spaces followed by `>` and an optional
/// single space. This function strips as many such prefixes as are present (recursive
/// nesting), returning the innermost content.
///
/// Example:
/// - `"   > > ```rust"` → `"```rust"`  (two levels of blockquote stripped)
/// - `"> line"` → `"line"`
/// - `"normal line"` → `"normal line"`  (unchanged)
fn strip_container_prefix(line: &str) -> String {
    let mut s = line;
    loop {
        // Strip 0-3 optional leading spaces
        let trimmed = s.trim_start_matches(' ');
        let spaces_stripped = s.len() - trimmed.len();
        if spaces_stripped > 3 {
            // 4+ leading spaces = indented code block context; stop stripping
            break;
        }
        // Check for blockquote `>` marker
        if let Some(after_gt) = trimmed.strip_prefix('>') {
            // Optionally consume one trailing space after `>`
            s = after_gt.strip_prefix(' ').unwrap_or(after_gt);
        } else {
            break;
        }
    }
    s.to_owned()
}

/// Returns `Some((fence_char, fence_len))` if the trimmed line opens a fenced code block.
///
/// `fence_char` is `'`'` or `'~'`. `fence_len` is the number of consecutive fence
/// characters at the start of the line (>= 3). An info string (e.g. "rust") may follow
/// the fence prefix — that does not affect detection.
///
/// Per CommonMark, the closing fence must use the same character and be at least as long
/// as the opening fence. Returning the opening length allows `is_fence_close` to enforce
/// this requirement.
fn detect_fence_open(line: &str) -> Option<(char, usize)> {
    let fence_char = if line.starts_with("```") {
        '`'
    } else if line.starts_with("~~~") {
        '~'
    } else {
        return None;
    };
    let fence_len = line.chars().take_while(|&c| c == fence_char).count();
    Some((fence_char, fence_len))
}

/// Return true if a trimmed line is a valid closing fence for the given opening fence.
///
/// Per CommonMark, a closing fence:
/// - consists entirely of the same fence character as the opening fence
/// - has length >= `open_len` (the opening fence length)
///
/// This prevents a shorter fence (e.g., 3 backticks) from incorrectly closing a
/// block opened with a longer fence (e.g., 4 or more backticks).
fn is_fence_close(line: &str, fence_char: char, open_len: usize) -> bool {
    if line.len() < open_len {
        return false;
    }
    line.chars().all(|c| c == fence_char)
}

/// Return true if a line contains an ADR-style example marker.
///
/// Markers (matched case-insensitively on the trimmed normalized line):
/// - `<!-- illustrative, non-canonical -->` — canonical ADR Q3 marker (ADR 2026-04-19-1242 §Q3)
/// - `<!-- example -->` or `<!-- example:` — HTML comment markers
/// - `// example` — C-style line comment (word-boundary: not `// examples` etc.)
///
/// Deliberately excluded patterns that are too broad for plan.md/verification.md:
/// - `# example` — would match Markdown section headings like `# Examples`
/// - `example:` — would match YAML content and other prose lines
fn is_example_marker(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("<!-- illustrative, non-canonical -->") {
        return true;
    }
    if lower.contains("<!-- example -->") || lower.contains("<!-- example:") {
        return true;
    }
    // `// example` with word-boundary: followed by end-of-string, space, or colon
    if let Some(rest) = lower.strip_prefix("// example") {
        if rest.is_empty() || rest.starts_with(':') || rest.starts_with(' ') {
            return true;
        }
    }
    false
}

/// Return true if the info string of a fence-open line contains "example" (case-insensitive).
///
/// The info string is the portion of the trimmed fence-open line that follows the
/// 3-character fence prefix (` ``` ` or `~~~`). For example:
/// - ` ```example ` → info string is "example" → returns true
/// - ` ```example my-block ` → info string is "example my-block" → returns true
/// - ` ```rust ` → info string is "rust" → returns false
///
/// This handles the case where the author tags the opening fence line itself with
/// "example" to mark the entire block as an intentional example.
fn fence_info_has_example(line: &str) -> bool {
    // Determine the fence character (` or ~) and strip all leading fence characters
    // to get the info string. A fence may use 3+ characters, so we strip all of them.
    let fence_char = if line.starts_with("```") {
        '`'
    } else if line.starts_with("~~~") {
        '~'
    } else {
        return false;
    };
    let info = line.trim_start_matches(fence_char);
    // Require "example" as a standalone word: preceded by start-of-string or whitespace,
    // and followed by end-of-string or whitespace. This allows ` ```example ` and
    // ` ```my example ` but rejects ` ```non-example ` and ` ```notexample `.
    let lower_info = info.to_ascii_lowercase();
    let target = "example";
    let mut search = lower_info.as_str();
    while let Some(pos) = search.find(target) {
        let before_ok = pos == 0 || search.as_bytes().get(pos - 1).is_some_and(|b| *b == b' ');
        let after_pos = pos + target.len();
        let after_ok = after_pos >= search.len()
            || search.as_bytes().get(after_pos).is_some_and(|b| *b == b' ');
        if before_ok && after_ok {
            return true;
        }
        // Advance past this occurrence; stop if at end
        search = search.get(pos + 1..).unwrap_or("");
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    // -----------------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------------

    /// Minimal spec.json (v2) with no ref fields → all refs are empty.
    const MINIMAL_SPEC: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Track",
  "scope": {
    "in_scope": [
      {"id": "IN-01", "text": "requirement one"}
    ],
    "out_of_scope": []
  }
}"#;

    /// Writes a file at `dir / relative_path`, creating parent dirs as needed.
    fn write_file(dir: &Path, relative: &str, content: &str) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    /// Build a fake repo layout rooted at `tmp`:
    ///   tmp/.git/               ← git root marker (makes `fallback_trusted_root` return `tmp`)
    ///   tmp/track/items/<id>/   ← track_dir
    ///   tmp/knowledge/adr/      ← referenced by adr_refs
    fn setup_repo(tmp: &Path, track_id: &str) -> PathBuf {
        // Create a `.git` directory at the repo root so that `fallback_trusted_root`
        // (called inside `resolve_trusted_root` when the spec path is outside the
        // current working tree) returns `tmp` as the trusted_root.  This matches the
        // layout assumed by `resolve_path`, which now uses `trusted_root` directly as
        // the repo root instead of a hard-coded 3-parent walk.
        std::fs::create_dir_all(tmp.join(".git")).unwrap();
        let track_dir = tmp.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        track_dir
    }

    // -----------------------------------------------------------------------
    // Happy-path: all refs valid
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_spec_json_passes() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        // No spec.json at all → pass immediately.
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "absent spec.json must pass: {:?}", outcome);
    }

    #[test]
    fn test_minimal_spec_with_no_refs_passes() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "spec with no refs must pass: {:?}", outcome);
    }

    #[test]
    fn test_spec_with_valid_adr_ref_passes() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        // Create the ADR file that will be referenced.
        write_file(tmp.path(), "knowledge/adr/2026-04-19-1242.md", "# ADR\n## D2.1 Section\n");

        let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "adr_refs": [{"file": "knowledge/adr/2026-04-19-1242.md", "anchor": "D2.1"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
        write_file(&track_dir, "spec.json", spec);
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "valid adr_ref must pass: {:?}", outcome);
    }

    #[test]
    fn test_spec_with_missing_adr_file_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "adr_refs": [{"file": "knowledge/adr/missing.md", "anchor": "D2.1"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
        write_file(&track_dir, "spec.json", spec);
        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "missing ADR file must produce error: {:?}", outcome);
        assert!(
            outcome.findings()[0].message().contains("missing.md"),
            "error must mention the missing file"
        );
    }

    #[test]
    fn test_spec_with_missing_convention_file_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "convention_refs": [{"file": "knowledge/conventions/missing.md", "anchor": "intro"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
        write_file(&track_dir, "spec.json", spec);
        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "missing convention file must produce error");
    }

    #[test]
    fn test_spec_with_informal_grounds_only_passes() {
        // informal_grounds have no file path — no file existence check needed.
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "informal_grounds": [{"kind": "feedback", "summary": "user directive to defer"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
        write_file(&track_dir, "spec.json", spec);
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "informal_grounds only must pass: {:?}", outcome);
    }

    #[test]
    fn test_malformed_spec_json_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", "not valid json");
        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "malformed spec.json must produce error");
    }

    // -----------------------------------------------------------------------
    // SpecRef anchor resolution
    // -----------------------------------------------------------------------

    #[test]
    fn test_spec_ref_with_valid_anchor_passes() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        // Write spec.json with one in_scope element.
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // Compute the expected hash for IN-01 element.
        let spec_value: serde_json::Value = serde_json::from_str(MINIMAL_SPEC).unwrap();
        let element = &spec_value["scope"]["in_scope"][0];
        let canonical = canonical_json(element);
        let hash = canonical_json_sha256(&canonical);

        // Write a domain-types.json with a SpecRef pointing at IN-01.
        let catalogue = format!(
            r#"{{
  "schema_version": 2,
  "type_definitions": [
    {{
      "name": "MyType",
      "description": "desc",
      "kind": "value_object",
      "approved": true,
      "spec_refs": [
        {{
          "file": "track/items/test-track/spec.json",
          "anchor": "IN-01",
          "hash": "{hash}"
        }}
      ]
    }}
  ]
}}"#
        );
        write_file(&track_dir, "domain-types.json", &catalogue);

        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "valid SpecRef must pass: {:?}", outcome);
    }

    #[test]
    fn test_spec_ref_with_missing_spec_file_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        let catalogue = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "MyType",
      "description": "desc",
      "kind": "value_object",
      "approved": true,
      "spec_refs": [
        {
          "file": "track/items/nonexistent/spec.json",
          "anchor": "IN-01",
          "hash": "0000000000000000000000000000000000000000000000000000000000000000"
        }
      ]
    }
  ]
}"#;
        write_file(&track_dir, "domain-types.json", catalogue);

        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "missing spec file must produce error: {:?}", outcome);
    }

    #[test]
    fn test_spec_ref_with_unresolved_anchor_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        let catalogue = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "MyType",
      "description": "desc",
      "kind": "value_object",
      "approved": true,
      "spec_refs": [
        {
          "file": "track/items/test-track/spec.json",
          "anchor": "IN-99",
          "hash": "0000000000000000000000000000000000000000000000000000000000000000"
        }
      ]
    }
  ]
}"#;
        write_file(&track_dir, "domain-types.json", catalogue);

        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "unresolved anchor must produce error: {:?}", outcome);
        assert!(
            outcome.findings()[0].message().contains("IN-99"),
            "error must mention the missing anchor"
        );
    }

    #[test]
    fn test_spec_ref_with_hash_mismatch_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // Use a deliberately wrong hash.
        let wrong_hash = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let catalogue = format!(
            r#"{{
  "schema_version": 2,
  "type_definitions": [
    {{
      "name": "MyType",
      "description": "desc",
      "kind": "value_object",
      "approved": true,
      "spec_refs": [
        {{
          "file": "track/items/test-track/spec.json",
          "anchor": "IN-01",
          "hash": "{wrong_hash}"
        }}
      ]
    }}
  ]
}}"#
        );
        write_file(&track_dir, "domain-types.json", &catalogue);

        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "hash mismatch must produce error: {:?}", outcome);
        assert!(
            outcome.findings()[0].message().contains("mismatch"),
            "error must mention 'mismatch': {}",
            outcome.findings()[0].message()
        );
    }

    #[test]
    fn test_empty_spec_refs_on_catalogue_entry_passes() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // Catalogue entry with no spec_refs → nothing to validate.
        let catalogue = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "MyType",
      "description": "desc",
      "kind": "value_object",
      "approved": true
    }
  ]
}"#;
        write_file(&track_dir, "domain-types.json", catalogue);

        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "empty spec_refs must pass: {:?}", outcome);
    }

    #[test]
    fn test_malformed_catalogue_json_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        write_file(&track_dir, "domain-types.json", "not valid json");

        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "malformed catalogue must produce error: {:?}", outcome);
    }

    // -----------------------------------------------------------------------
    // canonical_json helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_canonical_json_sorts_object_keys() {
        let v: serde_json::Value = serde_json::from_str(r#"{"z": 1, "a": 2, "m": 3}"#).unwrap();
        let canonical = canonical_json(&v);
        // Keys must appear in alphabetical order: a, m, z
        let pos_a = canonical.find("\"a\"").unwrap();
        let pos_m = canonical.find("\"m\"").unwrap();
        let pos_z = canonical.find("\"z\"").unwrap();
        assert!(pos_a < pos_m, "a must come before m");
        assert!(pos_m < pos_z, "m must come before z");
    }

    #[test]
    fn test_canonical_json_is_compact() {
        let v: serde_json::Value = serde_json::from_str(r#"{"key": "value"}"#).unwrap();
        let canonical = canonical_json(&v);
        assert!(!canonical.contains('\n'), "canonical form must not contain newlines");
        assert!(!canonical.contains("  "), "canonical form must not contain extra spaces");
    }

    #[test]
    fn test_canonical_json_sha256_is_stable() {
        // Same input must produce the same hash every time.
        let json = r#"{"id":"IN-01","text":"requirement one"}"#;
        let h1 = canonical_json_sha256(json);
        let h2 = canonical_json_sha256(json);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_canonical_json_sha256_differs_for_different_input() {
        let h1 = canonical_json_sha256(r#"{"id":"IN-01"}"#);
        let h2 = canonical_json_sha256(r#"{"id":"IN-02"}"#);
        assert_ne!(h1, h2);
    }

    // -----------------------------------------------------------------------
    // Security: symlink guard and path-traversal rejection
    // -----------------------------------------------------------------------

    /// A ref path containing a `..` component must be rejected even when the
    /// normalized result would remain inside the repo root.
    ///
    /// Rationale: lexical normalization collapses `..` before the symlink guard
    /// runs. A crafted ref like `dir/symlink/../target.md` normalizes to
    /// `dir/target.md`, hiding the symlink component from `reject_symlinks_below`.
    /// Rejecting `..` components in the input prevents this bypass.
    #[test]
    fn test_spec_with_adr_ref_containing_parent_dir_reports_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        // Create a real ADR file so the ref would pass if `..` were accepted.
        write_file(tmp.path(), "knowledge/adr/2026-04-19-1242.md", "# ADR\n");

        let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "adr_refs": [{"file": "knowledge/../knowledge/adr/2026-04-19-1242.md", "anchor": "D2.1"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
        write_file(&track_dir, "spec.json", spec);
        let outcome = verify(&track_dir);
        assert!(!outcome.is_ok(), "ref path with `..` must be rejected as invalid: {:?}", outcome);
    }

    /// A symlinked spec.json (symlink to a regular file) must be rejected by
    /// the symlink guard rather than silently passing because `is_file()` returns
    /// true for symlinks.
    ///
    /// A symlink-to-directory would cause `is_file()` to return `false`, which
    /// (before the fix) triggered an early `pass()` return, skipping all checks.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_spec_json_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        // Write a real spec.json somewhere outside the track dir.
        let real_spec = tmp.path().join("real_spec.json");
        std::fs::write(&real_spec, MINIMAL_SPEC).unwrap();

        // Create spec.json as a symlink pointing to the real file.
        let symlink_path = track_dir.join("spec.json");
        std::os::unix::fs::symlink(&real_spec, &symlink_path).unwrap();

        let outcome = verify(&track_dir);
        assert!(!outcome.is_ok(), "symlinked spec.json must be rejected: {:?}", outcome);
    }

    /// A symlinked spec.json pointing to a directory must also be rejected
    /// (before the fix, `is_file()` → false would silently return `pass()`).
    #[cfg(unix)]
    #[test]
    fn test_symlinked_spec_json_pointing_to_dir_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");

        // Create a directory target for the symlink.
        let dir_target = tmp.path().join("some_dir");
        std::fs::create_dir_all(&dir_target).unwrap();

        // spec.json → directory: `is_file()` returns false here, but the
        // symlink guard must still catch it.
        let symlink_path = track_dir.join("spec.json");
        std::os::unix::fs::symlink(&dir_target, &symlink_path).unwrap();

        let outcome = verify(&track_dir);
        assert!(!outcome.is_ok(), "symlink-to-directory spec.json must be rejected: {:?}", outcome);
    }

    // -----------------------------------------------------------------------
    // T011: task-coverage enforcement tests
    // -----------------------------------------------------------------------

    /// When task-coverage.json is absent, emit a warning (not an error).
    /// The outcome is still ok() because warnings don't fail CI in T011.
    #[test]
    fn test_absent_task_coverage_emits_warning_not_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        // No task-coverage.json
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "absent task-coverage.json must not fail CI: {:?}", outcome);
        let has_warning =
            outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Warning);
        assert!(has_warning, "absent task-coverage.json must emit a warning: {:?}", outcome);
    }

    /// in_scope requirement without a task_ref entry in task-coverage.json → error.
    #[test]
    fn test_in_scope_missing_task_ref_reports_coverage_violation() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        // task-coverage.json with empty in_scope section
        write_file(
            &track_dir,
            "task-coverage.json",
            r#"{"schema_version": 1, "in_scope": {}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
        );
        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "uncovered in_scope must produce error: {:?}", outcome);
        let has_coverage_error = outcome
            .findings()
            .iter()
            .any(|f| f.message().contains("coverage violation") && f.message().contains("IN-01"));
        assert!(has_coverage_error, "error must mention IN-01: {:?}", outcome);
    }

    /// Stale element id in task-coverage (not in spec.json) → referential integrity error.
    #[test]
    fn test_stale_element_id_in_task_coverage_reports_integrity_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        // task-coverage.json references IN-99 which doesn't exist in spec.json
        write_file(
            &track_dir,
            "task-coverage.json",
            r#"{"schema_version": 1, "in_scope": {"IN-01": ["T001"], "IN-99": ["T001"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
        );
        // impl-plan.json with T001
        write_file(
            &track_dir,
            "impl-plan.json",
            r#"{"schema_version": 1, "tasks": [{"id": "T001", "description": "task", "status": "todo"}], "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T001"]}]}}"#,
        );
        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "stale element id must produce error: {:?}", outcome);
        let has_integrity_error = outcome.findings().iter().any(|f| f.message().contains("IN-99"));
        assert!(has_integrity_error, "error must mention IN-99: {:?}", outcome);
    }

    /// Stale TaskId in task-coverage (not in impl-plan.json) → impl-plan integrity error.
    #[test]
    fn test_stale_task_id_in_task_coverage_reports_implplan_integrity_error() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        // task-coverage.json references T999 which doesn't exist in impl-plan.json
        write_file(
            &track_dir,
            "task-coverage.json",
            r#"{"schema_version": 1, "in_scope": {"IN-01": ["T999"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
        );
        // impl-plan.json with T001 only (T999 absent)
        write_file(
            &track_dir,
            "impl-plan.json",
            r#"{"schema_version": 1, "tasks": [{"id": "T001", "description": "task", "status": "todo"}], "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T001"]}]}}"#,
        );
        let outcome = verify(&track_dir);
        assert!(outcome.has_errors(), "stale TaskId must produce error: {:?}", outcome);
        let has_task_error = outcome
            .findings()
            .iter()
            .any(|f| f.message().contains("T999") && f.message().contains("impl-plan.json"));
        assert!(has_task_error, "error must mention T999 and impl-plan.json: {:?}", outcome);
    }

    /// Fully covered track (IN-01 → T001 in both coverage and impl-plan) passes.
    #[test]
    fn test_fully_covered_track_passes() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);
        write_file(
            &track_dir,
            "task-coverage.json",
            r#"{"schema_version": 1, "in_scope": {"IN-01": ["T001"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
        );
        write_file(
            &track_dir,
            "impl-plan.json",
            r#"{"schema_version": 1, "tasks": [{"id": "T001", "description": "task", "status": "todo"}], "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T001"]}]}}"#,
        );
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "fully covered track must pass: {:?}", outcome);
    }

    // -----------------------------------------------------------------------
    // T011: canonical-block suspicion detection tests
    // -----------------------------------------------------------------------

    /// A fenced code block with >10 lines and no example marker → warning.
    #[test]
    fn test_canonical_block_over_10_lines_emits_warning() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // Create a plan.md with a long fenced code block (12 lines inside).
        let plan_md = "# Plan\n\n```rust\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
        write_file(&track_dir, "plan.md", plan_md);

        let outcome = verify(&track_dir);
        let has_block_warning = outcome.findings().iter().any(|f| {
            f.severity() == domain::verify::Severity::Warning
                && f.message().contains("canonical-block suspicion")
        });
        assert!(
            has_block_warning,
            "long code block must emit canonical-block warning: {:?}",
            outcome
        );
        // Must NOT be an error (warning only).
        assert!(outcome.is_ok(), "canonical-block warning must not fail CI: {:?}", outcome);
    }

    /// A fenced code block with an example marker in the preceding line → no warning.
    #[test]
    fn test_canonical_block_with_preceding_example_marker_no_warning() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // plan.md with example marker on the line before the fence
        let plan_md = "# Plan\n\n<!-- example: long block -->\n```rust\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
        write_file(&track_dir, "plan.md", plan_md);

        let outcome = verify(&track_dir);
        let has_block_warning =
            outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
        assert!(
            !has_block_warning,
            "example-marked block must not emit canonical-block warning: {:?}",
            outcome
        );
    }

    /// A fenced code block with an example marker only inside the body (no preceding-line or
    /// fence-open marker) → warning IS emitted. Inner-line scanning is intentionally excluded
    /// to avoid false negatives from block content that happens to contain `example:` text.
    #[test]
    fn test_canonical_block_with_inner_only_example_marker_still_warns() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // plan.md with example: marker inside the fence but NOT in preceding line or info string.
        // Inner markers alone do not suppress the canonical-block warning.
        let plan_md = "# Plan\n\n```rust\n// example\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\n```\n";
        write_file(&track_dir, "plan.md", plan_md);

        let outcome = verify(&track_dir);
        let has_block_warning =
            outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
        assert!(
            has_block_warning,
            "inner-only example marker must not suppress canonical-block warning: {:?}",
            outcome
        );
    }

    /// A fenced code block with an example marker in the opening fence info string → no warning.
    ///
    /// Example: "```example" or "```example my-block" — the info string contains "example".
    #[test]
    fn test_canonical_block_with_fence_open_example_marker_no_warning() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // plan.md with example marker inline on the opening fence line (info string)
        let plan_md = "# Plan\n\n```example\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
        write_file(&track_dir, "plan.md", plan_md);

        let outcome = verify(&track_dir);
        let has_block_warning =
            outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
        assert!(
            !has_block_warning,
            "block with fence-open example marker must not emit warning: {:?}",
            outcome
        );
    }

    /// A fenced code block preceded by the canonical ADR Q3 marker
    /// `<!-- illustrative, non-canonical -->` → no warning.
    #[test]
    fn test_canonical_block_with_adr_illustrative_marker_no_warning() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // plan.md with the canonical ADR Q3 marker on the line before the fence.
        let plan_md = "# Plan\n\n<!-- illustrative, non-canonical -->\n```rust\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
        write_file(&track_dir, "plan.md", plan_md);

        let outcome = verify(&track_dir);
        let has_block_warning =
            outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
        assert!(
            !has_block_warning,
            "ADR Q3 illustrative marker must suppress canonical-block warning: {:?}",
            outcome
        );
    }

    /// A fenced code block with exactly 10 lines (not >10) → no warning.
    #[test]
    fn test_canonical_block_exactly_10_lines_no_warning() {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), "test-track");
        write_file(&track_dir, "spec.json", MINIMAL_SPEC);

        // plan.md with exactly 10 lines inside the fence (boundary: not >10)
        let plan_md = "# Plan\n\n```\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n```\n";
        write_file(&track_dir, "plan.md", plan_md);

        let outcome = verify(&track_dir);
        let has_block_warning =
            outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
        assert!(
            !has_block_warning,
            "block with exactly 10 lines must not emit warning: {:?}",
            outcome
        );
    }
}
