//! Spec element hash computation — infrastructure entry point for
//! `sotp track spec-element-hash`.
//!
//! Moves all raw I/O (including the `symlink_metadata` guard on `items_dir`)
//! into the infrastructure layer so the CLI command is a thin wrapper that
//! does only wiring plus exit-code mapping.
//!
//! Reuses `infrastructure::verify::plan_artifact_refs::build_element_map` +
//! `canonical_json_sha256` so the digests match the format consumed by
//! `sotp verify catalogue-spec-refs`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::track::fs_spec_file_loader::{FsSpecFileLoader, SpecFileLoaderPort};

/// Error returned by [`compute_spec_element_hashes`].
#[derive(Debug)]
pub struct SpecElementHashError(pub String);

impl std::fmt::Display for SpecElementHashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Compute canonical SHA-256 hashes for every element in `spec.json` and
/// return a sorted map from element id to 64-char lowercase hex digest.
///
/// Applies a `symlink_metadata` guard on `items_dir` before using it as the
/// trusted root for [`FsSpecFileLoader`], so the CLI layer never calls
/// `std::fs` or `symlink_metadata` directly.
///
/// When `anchor` is `Some`, only a single entry is returned (or an error when
/// the anchor is absent).  When `anchor` is `None`, the full map is returned.
///
/// # Errors
///
/// Returns [`SpecElementHashError`] when:
/// - `items_dir` itself is a symlink,
/// - `items_dir` cannot be stat-ed,
/// - `track_id` is structurally invalid (path traversal attempt),
/// - `spec.json` is absent, fails the symlink guard, or cannot be read,
/// - `spec.json` does not satisfy the schema (version, duplicate ids, etc.), or
/// - the requested `anchor` is not present in `spec.json`.
pub fn compute_spec_element_hashes(
    items_dir: PathBuf,
    track_id: &str,
    anchor: Option<&str>,
) -> Result<BTreeMap<String, String>, SpecElementHashError> {
    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor for `reject_symlinks_below`. That helper only checks components
    // *below* the trusted_root, so a symlinked items_dir would bypass all path guards.
    // Mirrors `execute_catalogue_spec_signals` (catalogue_spec_signals.rs).
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(SpecElementHashError(format!(
                "symlink guard: refusing to follow symlink at items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(SpecElementHashError(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            )));
        }
    }

    // Security: validate track_id via domain::TrackId before joining onto items_dir.
    // `Path::join` resolves `..`, `/`, and multi-segment paths (`foo/bar`) at the OS
    // level. Using the domain type enforces the slug rules (single-segment, no `..`,
    // no path separators) and makes this function safe when called directly without
    // upstream CLI validation.
    let valid_track_id = domain::TrackId::try_new(track_id)
        .map_err(|e| SpecElementHashError(format!("invalid track_id '{track_id}': {e}")))?;

    // `FsSpecFileLoader::load` applies `reject_symlinks_below(&spec_path, &items_dir)`
    // which checks every ancestor of spec_path down to items_dir — including the
    // track directory itself.  No additional track-dir symlink guard is required here;
    // a symlinked track_dir is caught by the loader before the file is read.
    let track_dir = items_dir.join(valid_track_id.as_ref());
    let spec_path = track_dir.join("spec.json");

    let loader = FsSpecFileLoader::new(items_dir);
    let text = loader.load(&spec_path).map_err(|e| SpecElementHashError(e.0))?;

    // Validate schema first so malformed spec.json fails closed instead of
    // emitting a partial hash map (mirrors verify_catalogue_spec_refs.rs).
    crate::spec::codec::decode(&text).map_err(|e| {
        SpecElementHashError(format!("spec.json schema error at '{}': {e}", spec_path.display()))
    })?;

    let raw: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| SpecElementHashError(format!("spec.json JSON parse error: {e}")))?;

    let element_map = crate::verify::plan_artifact_refs::build_element_map(&raw);
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    for (id, canonical) in element_map {
        let hash = crate::verify::plan_artifact_refs::canonical_json_sha256(&canonical);
        hashes.insert(id, hash);
    }

    match anchor {
        None => Ok(hashes),
        Some(anchor_id) => match hashes.remove(anchor_id) {
            Some(hash) => {
                let mut single = BTreeMap::new();
                single.insert(anchor_id.to_owned(), hash);
                Ok(single)
            }
            None => {
                let available = hashes.keys().cloned().collect::<Vec<_>>().join(", ");
                Err(SpecElementHashError(format!(
                    "anchor '{anchor_id}' not found in spec.json (available: {available})"
                )))
            }
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use super::*;

    /// Minimal valid spec.json (schema version 2) with one goal element and one
    /// in-scope element. Used across multiple test cases.
    const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Spec",
  "goal": [
    {"id": "GL-01", "text": "First goal"}
  ],
  "scope": {
    "in_scope": [
      {"id": "IN-01", "text": "In scope item"}
    ],
    "out_of_scope": []
  }
}"#;

    fn setup_track(base: &std::path::Path, track_id: &str) -> (PathBuf, PathBuf) {
        let items_dir = base.join("track/items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        (items_dir, track_dir)
    }

    #[test]
    fn test_compute_spec_element_hashes_all_returns_64_char_hex_per_element() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).unwrap();

        let hashes = compute_spec_element_hashes(items_dir, "my-track-2026-04-26", None).unwrap();
        assert_eq!(hashes.len(), 2, "expected GL-01 and IN-01");
        for (id, hex) in &hashes {
            assert_eq!(hex.len(), 64, "hash for '{id}' must be 64 chars");
            assert!(
                hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
                "hash for '{id}' must be lowercase hex: {hex}"
            );
        }
    }

    #[test]
    fn test_compute_spec_element_hashes_single_anchor_returns_one_entry() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).unwrap();

        let hashes =
            compute_spec_element_hashes(items_dir, "my-track-2026-04-26", Some("GL-01")).unwrap();
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains_key("GL-01"));
    }

    #[test]
    fn test_compute_spec_element_hashes_missing_anchor_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).unwrap();

        let err =
            compute_spec_element_hashes(items_dir, "my-track-2026-04-26", Some("NONEXISTENT"))
                .unwrap_err();
        assert!(err.0.contains("NONEXISTENT"), "error should mention missing anchor: {}", err.0);
    }

    #[test]
    fn test_compute_spec_element_hashes_missing_spec_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, _track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        // No spec.json written.

        let err = compute_spec_element_hashes(items_dir, "my-track-2026-04-26", None).unwrap_err();
        assert!(
            err.0.contains("spec.json"),
            "error should mention spec.json read failure: {}",
            err.0
        );
    }

    #[test]
    fn test_compute_spec_element_hashes_schema_invalid_spec_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_dir) = setup_track(dir.path(), "my-track-2026-04-26");
        // schema_version 1 is not accepted by spec::codec::decode.
        fs::write(
            track_dir.join("spec.json"),
            r#"{"schema_version": 1, "version": "1.0", "title": "X",
                "scope": {"in_scope": [], "out_of_scope": []}}"#,
        )
        .unwrap();

        let err = compute_spec_element_hashes(items_dir, "my-track-2026-04-26", None).unwrap_err();
        assert!(
            err.0.contains("spec.json schema error"),
            "error should mention schema failure: {}",
            err.0
        );
    }
}
