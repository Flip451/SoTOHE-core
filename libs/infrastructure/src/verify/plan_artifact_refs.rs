//! Verify structured-ref fields introduced in T002 / T003 / T005.
//!
//! This module validates plan artifact references in a track directory:
//! - `spec.json` ref fields: `adr_refs`, `convention_refs`, `informal_grounds`,
//!   `related_conventions`, and per-catalogue-entry `spec_refs` / `informal_grounds`
//! - File existence for file-based refs (`AdrRef.file`, `ConventionRef.file`, `SpecRef.file`)
//! - `SpecRef.anchor` must resolve to a real element id inside the target `spec.json`
//! - `SpecRef.hash` must match the SHA-256 of the canonical JSON subtree
//! - `AdrAnchor` / `ConventionAnchor` loose non-empty validation (already enforced by newtypes)
//! - `InformalGroundRef` newtype validation (kind variant + non-empty summary)
//!
//! Per ADR 2026-04-19-1242 §D2.3.

use std::collections::HashMap;
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

    /// A task-coverage constraint was violated (reserved for T011).
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
}
