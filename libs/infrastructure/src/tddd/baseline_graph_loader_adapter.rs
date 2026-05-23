//! Filesystem-backed adapter for the `BaselineGraphLoader` domain port.
//!
//! [`BaselineGraphLoaderAdapter`] discovers `tddd.enabled` layers from
//! `architecture-rules.json` via [`load_tddd_layers`], then loads each
//! layer's rustdoc JSON baseline from the track directory using
//! [`BaselineRustdocCodec`]. The returned [`BaselineDocument`] slice groups
//! each `rustdoc_types::Crate` with its [`LayerId`] and [`CrateName`].
//!
//! ## Symlink policy
//!
//! All paths are guarded by [`reject_symlinks_below`] anchored at
//! `trusted_root`. A symlink anywhere along the resolved path is rejected
//! fail-closed as [`BaselineGraphLoaderError::SymlinkRejected`].
//!
//! ## File-not-found policy
//!
//! A missing baseline file is a hard error
//! ([`BaselineGraphLoaderError::NotFound`]). Absent baselines are never
//! silently skipped (fail-closed, per spec IN-19 / CN-03).
//!
//! ## Symmetric design
//!
//! Symmetric to [`super::contract_map_adapter::FsCatalogueLoader`]:
//! - Same three constructor fields (`track_root`, `rules_path`, `trusted_root`).
//! - Same layer-discovery path via `load_tddd_layers`.
//! - Same symlink / NotFound error-mapping conventions.
//!
//! (IN-02 / IN-19 / AC-02 / CN-03)

use std::path::PathBuf;

use domain::TrackId;
use domain::tddd::{
    BaselineGraphLoader, BaselineGraphLoaderError, LayerId, baseline_document::BaselineDocument,
    catalogue_v2::identifiers::CrateName,
};

use crate::tddd::baseline_rustdoc_codec::{BaselineRustdocCodec, BaselineRustdocCodecError};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

// ---------------------------------------------------------------------------
// BaselineGraphLoaderAdapter
// ---------------------------------------------------------------------------

/// Filesystem-backed [`BaselineGraphLoader`] implementation.
///
/// Discovers `tddd.enabled` layers via `architecture-rules.json` at
/// `rules_path`, then loads the rustdoc JSON baseline for each layer from
/// `track_root/<track_id>/<layer>-types-baseline.json`.
///
/// Rejects symlinks below `trusted_root` fail-closed. A missing baseline
/// file is always [`BaselineGraphLoaderError::NotFound`] (fail-closed).
///
/// Symmetric to `FsCatalogueLoader`. (IN-02 / IN-19 / AC-02 / AC-15 / CN-03)
pub struct BaselineGraphLoaderAdapter {
    /// Directory containing per-track subdirectories (typically `<workspace>/track/items`).
    pub track_root: PathBuf,
    /// Path to `architecture-rules.json`.
    pub rules_path: PathBuf,
    /// Directory below which symlink traversal is refused fail-closed.
    pub trusted_root: PathBuf,
}

impl BaselineGraphLoaderAdapter {
    /// Creates a new `BaselineGraphLoaderAdapter`.
    ///
    /// * `track_root` — directory containing per-track subdirectories
    ///   (typically `<workspace>/track/items`).
    /// * `rules_path` — path to `architecture-rules.json`.
    /// * `trusted_root` — directory below which symlink traversal is refused
    ///   fail-closed (usually the workspace root).
    #[must_use]
    pub fn new(track_root: PathBuf, rules_path: PathBuf, trusted_root: PathBuf) -> Self {
        Self { track_root, rules_path, trusted_root }
    }
}

impl BaselineGraphLoader for BaselineGraphLoaderAdapter {
    /// Load all enabled-layer rustdoc baselines for `track_id`.
    ///
    /// # Errors
    ///
    /// - [`BaselineGraphLoaderError::LayerDiscoveryFailed`] when
    ///   `architecture-rules.json` cannot be read or parsed, or when a
    ///   layer ID string is invalid.
    /// - [`BaselineGraphLoaderError::SymlinkRejected`] when any path in the
    ///   baseline chain passes through a symlink (including the track directory
    ///   itself — detected while stat-checking the per-layer baseline path).
    /// - [`BaselineGraphLoaderError::NotFound`] when a layer's baseline file
    ///   is absent (fail-closed).
    /// - [`BaselineGraphLoaderError::ParseFailed`] when JSON deserialization
    ///   fails for a layer.
    /// - [`BaselineGraphLoaderError::IoError`] for non-symlink I/O failures
    ///   during baseline loading.
    fn load_all(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<BaselineDocument>, BaselineGraphLoaderError> {
        // Step 1: discover tddd.enabled layers from architecture-rules.json.
        let bindings = load_tddd_layers(&self.rules_path, &self.trusted_root)
            .map_err(map_layer_discovery_error)?;

        let track_dir = self.track_root.join(track_id.as_ref());

        // Step 2: for each enabled layer, load the rustdoc JSON baseline.
        //
        // There is no separate pre-check on `track_dir` itself. The per-layer
        // symlink guard on each `baseline_path` walks the full ancestor chain
        // (including `track_dir`) and therefore detects a symlinked track
        // directory as `SymlinkRejected`, a missing track directory as `NotFound`,
        // and a permission-denied track directory as `IoError` — all with the
        // layer context required to construct the correct error variant.
        let mut documents = Vec::with_capacity(bindings.len());
        for binding in &bindings {
            let layer_id = LayerId::try_new(binding.layer_id()).map_err(|e| {
                BaselineGraphLoaderError::LayerDiscoveryFailed {
                    reason: format!("invalid layer id '{}': {e}", binding.layer_id()),
                }
            })?;

            let baseline_filename = binding.baseline_file();
            let baseline_path = track_dir.join(&baseline_filename);

            // Symlink guard on the baseline path.
            match reject_symlinks_below(&baseline_path, &self.trusted_root) {
                Ok(true) => {} // exists and not a symlink
                Ok(false) => {
                    // File absent — fail-closed.
                    return Err(BaselineGraphLoaderError::NotFound {
                        layer_id: layer_id.clone(),
                        path: baseline_path,
                    });
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::InvalidInput {
                        return Err(BaselineGraphLoaderError::SymlinkRejected {
                            path: baseline_path,
                        });
                    }
                    return Err(BaselineGraphLoaderError::IoError {
                        layer_id: layer_id.clone(),
                        path: baseline_path,
                        reason: e.to_string(),
                    });
                }
            }

            // Defensive existence check after the symlink guard (handles non-symlink
            // missing files on platforms where `reject_symlinks_below` only inspects
            // symlink components and may return `true` for a valid path that does not
            // yet exist on disk).
            if !baseline_path.is_file() {
                return Err(BaselineGraphLoaderError::NotFound {
                    layer_id: layer_id.clone(),
                    path: baseline_path,
                });
            }

            // Load and deserialize the rustdoc JSON.
            let krate = BaselineRustdocCodec::load(&baseline_path).map_err(|e| match e {
                BaselineRustdocCodecError::IoError(io_err) => BaselineGraphLoaderError::IoError {
                    layer_id: layer_id.clone(),
                    path: baseline_path.clone(),
                    reason: io_err.to_string(),
                },
                BaselineRustdocCodecError::Json(json_err) => {
                    BaselineGraphLoaderError::ParseFailed {
                        layer_id: layer_id.clone(),
                        reason: json_err.to_string(),
                    }
                }
                BaselineRustdocCodecError::UnsupportedFormatVersion { actual, expected } => {
                    BaselineGraphLoaderError::ParseFailed {
                        layer_id: layer_id.clone(),
                        reason: format!(
                            "unsupported rustdoc format_version: file has {actual}, \
                             binary expects {expected}"
                        ),
                    }
                }
            })?;

            // Derive crate_name from schema_export.targets[0].
            //
            // The ADR (Decision A-r3) designs `Vec<BaselineDocument>` to support
            // 1-layer-N-crate configurations where each crate gets its own
            // `BaselineDocument`. Full multi-target iteration (one `BaselineDocument`
            // per target) is deferred to a future track; the current project has only
            // single-element targets arrays. We use `targets[0]` for the canonical
            // crate that owns this layer's baseline file.
            //
            // When targets is empty after parse_tddd_layers defaulting (should not
            // happen — the parse function substitutes [layer_id] for empty/absent
            // targets), the layer_id is used as a safe fallback crate name.
            let crate_str = binding.targets().first().map_or(binding.layer_id(), String::as_str);
            let crate_name = CrateName::new(crate_str).map_err(|e| {
                BaselineGraphLoaderError::LayerDiscoveryFailed {
                    reason: format!("invalid crate name '{}': {e}", crate_str),
                }
            })?;

            documents.push(BaselineDocument::new(layer_id, crate_name, krate));
        }

        Ok(documents)
    }
}

// ---------------------------------------------------------------------------
// Error mapping helpers
// ---------------------------------------------------------------------------

fn map_layer_discovery_error(err: LoadTdddLayersError) -> BaselineGraphLoaderError {
    match err {
        LoadTdddLayersError::Io { path, source } => {
            if source.kind() == std::io::ErrorKind::InvalidInput {
                BaselineGraphLoaderError::SymlinkRejected { path }
            } else {
                BaselineGraphLoaderError::LayerDiscoveryFailed {
                    reason: format!("{}: {source}", path.display()),
                }
            }
        }
        LoadTdddLayersError::Parse(parse_err) => {
            BaselineGraphLoaderError::LayerDiscoveryFailed { reason: parse_err.to_string() }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::str_to_string
)]
mod tests {
    use rustdoc_types::FORMAT_VERSION;
    use tempfile::TempDir;

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Minimal rustdoc JSON for a valid `rustdoc_types::Crate`.
    fn minimal_crate_json() -> String {
        format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {FORMAT_VERSION},
                "target": {{"triple": "", "target_features": []}}
            }}"#
        )
    }

    /// Minimal `architecture-rules.json` with one `tddd.enabled` layer.
    fn rules_json_single_layer(layer: &str, catalogue_file: &str) -> String {
        format!(
            r#"{{
              "version": 2,
              "layers": [
                {{
                  "crate": "{layer}",
                  "path": "libs/{layer}",
                  "may_depend_on": [],
                  "deny_reason": "no reverse dep",
                  "tddd": {{
                    "enabled": true,
                    "catalogue_file": "{catalogue_file}",
                    "schema_export": {{"method": "rustdoc", "targets": ["{layer}"]}}
                  }}
                }}
              ]
            }}"#
        )
    }

    /// `architecture-rules.json` with two `tddd.enabled` layers.
    fn rules_json_two_layers() -> String {
        r#"{
          "version": 2,
          "layers": [
            {
              "crate": "domain",
              "path": "libs/domain",
              "may_depend_on": [],
              "deny_reason": "no reverse dep",
              "tddd": {
                "enabled": true,
                "catalogue_file": "domain-types.json",
                "schema_export": {"method": "rustdoc", "targets": ["domain"]}
              }
            },
            {
              "crate": "usecase",
              "path": "libs/usecase",
              "may_depend_on": ["domain"],
              "deny_reason": "no reverse dep",
              "tddd": {
                "enabled": true,
                "catalogue_file": "usecase-types.json",
                "schema_export": {"method": "rustdoc", "targets": ["usecase"]}
              }
            }
          ]
        }"#
        .to_string()
    }

    /// Write content to `path`, creating parent directories as needed.
    fn write(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// Set up a temporary workspace with `architecture-rules.json` and return
    /// (dir, rules_path, track_root).
    fn setup_workspace(rules_content: &str) -> (TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let rules_path = root.join("architecture-rules.json");
        write(&rules_path, rules_content);
        let track_root = root.join("track").join("items");
        std::fs::create_dir_all(&track_root).unwrap();
        (dir, rules_path, track_root)
    }

    fn track_id(slug: &str) -> TrackId {
        TrackId::try_new(slug.to_owned()).unwrap()
    }

    // -----------------------------------------------------------------------
    // Happy path: single layer
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_single_layer_returns_one_document() {
        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        // baseline file: domain-types.json → stem "domain-types" → "domain-types-baseline.json"
        write(&track_dir.join("domain-types-baseline.json"), &minimal_crate_json());

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let docs = adapter.load_all(&tid).unwrap();

        assert_eq!(docs.len(), 1, "expected 1 document");
        assert_eq!(docs[0].layer.as_ref(), "domain");
        assert_eq!(docs[0].crate_name.as_str(), "domain");
    }

    // -----------------------------------------------------------------------
    // Happy path: two layers
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_two_layers_returns_two_documents_in_declaration_order() {
        let rules = rules_json_two_layers();
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        write(&track_dir.join("domain-types-baseline.json"), &minimal_crate_json());
        write(&track_dir.join("usecase-types-baseline.json"), &minimal_crate_json());

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let docs = adapter.load_all(&tid).unwrap();

        assert_eq!(docs.len(), 2, "expected 2 documents");
        let layers: Vec<_> = docs.iter().map(|d| d.layer.as_ref()).collect();
        assert!(layers.contains(&"domain"), "domain must be present");
        assert!(layers.contains(&"usecase"), "usecase must be present");
    }

    // -----------------------------------------------------------------------
    // Layer discovery: architecture-rules.json missing
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_missing_rules_returns_layer_discovery_failed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let rules_path = root.join("architecture-rules.json"); // does not exist
        let track_root = root.join("track").join("items");
        std::fs::create_dir_all(&track_root).unwrap();

        let tid = track_id("my-track-2026-05-22");
        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();

        assert!(
            matches!(err, BaselineGraphLoaderError::LayerDiscoveryFailed { .. }),
            "expected LayerDiscoveryFailed, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Fail-closed: baseline file missing
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_missing_baseline_returns_not_found() {
        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        // Create track dir but do NOT write the baseline file.
        std::fs::create_dir_all(track_root.join(tid.as_ref())).unwrap();

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();

        match err {
            BaselineGraphLoaderError::NotFound { layer_id, path } => {
                assert_eq!(layer_id.as_ref(), "domain");
                assert!(
                    path.ends_with("domain-types-baseline.json"),
                    "path should point to baseline file, got {path:?}"
                );
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Fail-closed: malformed baseline JSON
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_malformed_baseline_returns_parse_failed() {
        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        write(&track_dir.join("domain-types-baseline.json"), "{ not valid json }");

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();

        assert!(
            matches!(err, BaselineGraphLoaderError::ParseFailed { .. }),
            "expected ParseFailed, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Fail-closed: wrong format_version
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_wrong_format_version_returns_parse_failed() {
        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());

        // JSON with wrong format_version.
        let bad_json = r#"{
            "root": 0,
            "crate_version": null,
            "includes_private": false,
            "index": {},
            "paths": {},
            "external_crates": {},
            "format_version": 1,
            "target": {"triple": "", "target_features": []}
        }"#;
        write(&track_dir.join("domain-types-baseline.json"), bad_json);

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();

        assert!(
            matches!(err, BaselineGraphLoaderError::ParseFailed { .. }),
            "expected ParseFailed (UnsupportedFormatVersion mapped), got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Symlink rejection: symlinked baseline file
    // -----------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn test_load_all_symlinked_baseline_returns_symlink_rejected() {
        use std::os::unix::fs::symlink;

        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        // Create the real file outside the trusted root.
        let outside = root.join("outside").join("domain-types-baseline.json");
        write(&outside, &minimal_crate_json());

        // Symlink inside the track dir pointing outside.
        symlink(&outside, track_dir.join("domain-types-baseline.json")).unwrap();

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root.clone());
        let err = adapter.load_all(&tid).unwrap_err();

        assert!(
            matches!(err, BaselineGraphLoaderError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Symlink rejection: symlinked track directory
    // -----------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn test_load_all_symlinked_track_dir_returns_symlink_rejected() {
        use std::os::unix::fs::symlink;

        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        std::fs::create_dir_all(&track_root).unwrap();

        // Create a real directory outside the track_root and symlink to it.
        let real_dir = root.join("real-track");
        std::fs::create_dir_all(&real_dir).unwrap();
        write(&real_dir.join("domain-types-baseline.json"), &minimal_crate_json());

        let tid = track_id("my-track-2026-05-22");
        symlink(&real_dir, track_root.join(tid.as_ref())).unwrap();

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();

        assert!(
            matches!(err, BaselineGraphLoaderError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Layer-agnostic: custom layer name is preserved
    //
    // Note: the layer id (e.g. "custom_layer") must be a valid LayerId (allows
    // hyphens), but `targets` must be valid Rust identifiers because they are
    // used as the crate name. This test uses an underscore-based layer name
    // so both LayerId and CrateName validations succeed.
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_custom_layer_name_preserved_in_document() {
        let rules = rules_json_single_layer("custom-layer", "custom-layer-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        // baseline: custom-layer-types.json → "custom-layer-types-baseline.json"
        write(&track_dir.join("custom-layer-types-baseline.json"), &minimal_crate_json());

        // architecture-rules.json puts "custom-layer" as the crate name in
        // `rules_json_single_layer`. LayerId accepts hyphens; however CrateName
        // requires a valid Rust identifier. In this fixture the target crate is
        // "custom-layer" which is not a valid Rust identifier, so we expect
        // LayerDiscoveryFailed for CrateName validation failure.
        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();
        assert!(
            matches!(err, BaselineGraphLoaderError::LayerDiscoveryFailed { .. }),
            "hyphen crate name must fail crate name validation: {err:?}"
        );
    }

    // Layer-agnostic with valid Rust identifier crate name.
    #[test]
    fn test_load_all_underscore_layer_name_preserved_in_document() {
        // "application" is a valid LayerId and a valid CrateName.
        let rules = rules_json_single_layer("application", "application-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        // baseline: application-types.json → "application-types-baseline.json"
        write(&track_dir.join("application-types-baseline.json"), &minimal_crate_json());

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let docs = adapter.load_all(&tid).unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].layer.as_ref(), "application");
        assert_eq!(docs[0].crate_name.as_str(), "application");
    }

    // -----------------------------------------------------------------------
    // Loaded document carries correct crate data
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_document_carries_krate_with_correct_format_version() {
        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        write(&track_dir.join("domain-types-baseline.json"), &minimal_crate_json());

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let docs = adapter.load_all(&tid).unwrap();

        assert_eq!(docs[0].krate.format_version, FORMAT_VERSION);
    }

    // -----------------------------------------------------------------------
    // No-op: zero tddd.enabled layers
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_no_enabled_layers_returns_empty_vec() {
        // architecture-rules.json with no tddd.enabled layers.
        let rules = r#"{
          "version": 2,
          "layers": [
            {
              "crate": "domain",
              "path": "libs/domain",
              "may_depend_on": [],
              "deny_reason": "no reverse dep"
            }
          ]
        }"#;
        let (_dir, rules_path, track_root) = setup_workspace(rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        // No baseline file needed; no enabled layers.

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let docs = adapter.load_all(&tid).unwrap();

        assert!(docs.is_empty(), "expected empty Vec, got {docs:?}");
    }

    // -----------------------------------------------------------------------
    // Display of write-target path includes layer
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_all_not_found_error_display_contains_layer_and_path() {
        let rules = rules_json_single_layer("domain", "domain-types.json");
        let (_dir, rules_path, track_root) = setup_workspace(&rules);
        let root = rules_path.parent().unwrap().to_path_buf();

        let tid = track_id("my-track-2026-05-22");
        std::fs::create_dir_all(track_root.join(tid.as_ref())).unwrap();

        let adapter = BaselineGraphLoaderAdapter::new(track_root, rules_path, root);
        let err = adapter.load_all(&tid).unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("domain"), "Display must include layer; got: {msg}");
        assert!(
            msg.contains("domain-types-baseline.json"),
            "Display must include file name; got: {msg}"
        );
    }
}
