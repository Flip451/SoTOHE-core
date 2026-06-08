//! Helpers for validating file-based and structured spec refs.
//!
//! Covers:
//! - `AdrRef.file` and `ConventionRef.file` existence checks
//! - Path-traversal and symlink guards (`resolve_path`, `check_ref_file`)
//! - `SpecRef.anchor` resolution and hash verification helpers
//! - Canonical JSON construction and SHA-256 hashing

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::verify::VerifyFinding;

use crate::track::symlink_guard;

/// Mapping from spec element id string → canonical JSON representation of the element subtree.
pub type SpecElementMap = HashMap<String, String>;

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
pub(crate) fn check_ref_file(
    file: &Path,
    trusted_root: &Path,
    context: &str,
    findings: &mut Vec<VerifyFinding>,
) {
    check_ref_file_returning(file, trusted_root, context, findings);
}

/// Variant of [`check_ref_file`] that returns `true` when the file exists and passed
/// all guards, `false` otherwise.
///
/// Callers that need to perform additional checks only when the file is present
/// (e.g. anchor existence verification) should use this variant.
pub(crate) fn check_ref_file_returning(
    file: &Path,
    trusted_root: &Path,
    context: &str,
    findings: &mut Vec<VerifyFinding>,
) -> bool {
    match resolve_path(trusted_root, file) {
        None => {
            findings.push(VerifyFinding::error(format!(
                "{context}: invalid path (absolute or path-traversal): {}",
                file.display()
            )));
            false
        }
        Some(p) => match symlink_guard::reject_symlinks_below(&p, trusted_root) {
            Ok(true) => true,
            Ok(false) => {
                findings.push(VerifyFinding::error(format!(
                    "{context}: file not found: {}",
                    file.display()
                )));
                false
            }
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "{context}: symlink guard for '{}': {e}",
                    file.display()
                )));
                false
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
pub(crate) fn resolve_path(repo_root: &Path, file: &Path) -> Option<PathBuf> {
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
pub(crate) fn load_spec_element_map<'c>(
    cache: &'c mut HashMap<PathBuf, SpecElementMap>,
    spec_path: &Path,
    _trusted_root: &Path,
) -> Result<&'c SpecElementMap, String> {
    use crate::spec::codec as spec_codec;

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
pub fn build_element_map(raw: &serde_json::Value) -> HashMap<String, String> {
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
pub fn canonical_json(value: &serde_json::Value) -> String {
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
pub fn canonical_json_sha256(json: &str) -> String {
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
