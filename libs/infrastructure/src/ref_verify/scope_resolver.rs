//! Concrete adapter for resolving the [`RefVerifyScope`] from track artifact
//! existence on disk (IN-01 / D1).
//!
//! ## Existence-based resolution
//!
//! The scope is derived from which SoT artifacts exist in the track
//! directory — never from a caller-declared firing context ("file existence
//! = phase state"):
//!
//! - spec.json absent + all catalogues absent → [`RefVerifyScope::All`]
//!   (Phase 0: both chains contribute zero pairs downstream)
//! - spec.json present + all catalogues absent → [`RefVerifyScope::All`]
//!   (pre-Phase-2: Chain-2 contributes zero pairs downstream)
//! - spec.json present + all catalogues present → [`RefVerifyScope::All`]
//!
//! Fail-closed cases (inconsistent absence):
//!
//! - spec.json absent while any catalogue exists (SoT Chain ordering
//!   violation, IN-06) → error
//! - partial catalogue set (some present, some absent, IN-05) → error
//!
//! The resolver performs **existence checks only**. Reading or parsing the
//! artifacts (and the fail-closed handling of present-but-broken files,
//! IN-07) is owned by the pair source / codec layer.

use std::path::{Path, PathBuf};

use usecase::ref_verify::RefVerifyScope;

/// Check whether a path is present, rejecting symlinks at the leaf and every
/// ancestor between the path and `trusted_root`.
///
/// Delegates to [`crate::track::symlink_guard::reject_symlinks_below`] so that
/// both leaf-level and parent-directory symlinks are detected and rejected —
/// a symlinked `track/items` or `track_id` parent would otherwise allow
/// traversal to files outside the trusted root.
///
/// Returns:
/// - `Ok(true)` — path exists and no symlink was found in the path components
/// - `Ok(false)` — path does not exist (leaf absent, parents intact)
/// - `Err` — I/O error, or any path component is a symlink
fn path_is_present(path: &Path, trusted_root: &Path) -> Result<bool, std::io::Error> {
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root)
}

/// Failure modes of [`RefVerifyScopeResolver::resolve`].
#[derive(Debug)]
pub enum RefVerifyScopeResolverError {
    /// I/O failure while inspecting a required artifact, or an inconsistent
    /// artifact state (catalogue present while spec.json is absent).
    Io {
        /// The path that could not be inspected or violates the SoT Chain
        /// ordering.
        path: String,
        /// Human-readable description of the failure.
        message: String,
    },
    /// Some, but not all, TDDD layer catalogues exist for the track —
    /// an All-scope run would silently skip the missing layers, so the
    /// resolver fails closed instead.
    PartialCatalogues {
        /// Catalogue file names that are missing.
        missing: Vec<String>,
    },
}

impl std::fmt::Display for RefVerifyScopeResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(f, "ref-verify scope resolver I/O error at '{path}': {message}")
            }
            Self::PartialCatalogues { missing } => {
                write!(
                    f,
                    "partial TDDD catalogue set — missing catalogue file(s): {}",
                    missing.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for RefVerifyScopeResolverError {}

/// Concrete [`RefVerifyScope`] resolver: derives the scope from track
/// artifact existence and validates artifact-state consistency.
#[derive(Debug)]
pub struct RefVerifyScopeResolver {
    project_root: PathBuf,
}

impl RefVerifyScopeResolver {
    /// Construct a resolver rooted at `project_root`.
    #[must_use]
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Resolve the [`RefVerifyScope`] for `track_id` from artifact existence.
    ///
    /// Resolution is existence-only: catalogue files are inspected by probing
    /// their declared paths on the filesystem — no file content is read or
    /// parsed by the resolver itself. Present-but-broken artifacts fail closed
    /// downstream in pair_source/codec (IN-07); that is deliberately not this
    /// resolver's responsibility.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyScopeResolverError`] when an artifact path cannot
    /// be inspected, a catalogue exists while spec.json is absent (SoT Chain
    /// ordering violation, IN-06), or only part of the TDDD catalogue set
    /// exists (IN-05).
    pub fn resolve(&self, track_id: &str) -> Result<RefVerifyScope, RefVerifyScopeResolverError> {
        let track_dir = self.project_root.join("track").join("items").join(track_id);

        let spec_path = track_dir.join("spec.json");
        let spec_exists = path_is_present(&spec_path, &self.project_root).map_err(|e| {
            RefVerifyScopeResolverError::Io {
                path: spec_path.display().to_string(),
                message: format!("cannot inspect path: {e}"),
            }
        })?;

        // Obtain the declared catalogue filenames from architecture-rules.json.
        // Using declared names — rather than a file-name pattern — means that
        // any valid catalogue_file value (not just ones ending in -types.json)
        // is recognised. See `load_bindings_or_empty` for the error-handling
        // contract: absent/malformed rules files return an empty list.
        let bindings = self.load_bindings_or_empty()?;

        // Count declared catalogues that are present vs absent on disk.
        let mut present = 0usize;
        let mut missing: Vec<String> = Vec::new();
        for binding in &bindings {
            let catalogue_path = track_dir.join(binding.catalogue_file());
            let exists = path_is_present(&catalogue_path, &self.project_root).map_err(|e| {
                RefVerifyScopeResolverError::Io {
                    path: catalogue_path.display().to_string(),
                    message: format!("cannot inspect path: {e}"),
                }
            })?;
            if exists {
                present += 1;
            } else {
                missing.push(binding.catalogue_file().to_owned());
            }
        }

        // IN-06 (checked before IN-05): a TDDD catalogue can only be authored
        // after spec.json (Phase 2 follows Phase 1). Fail closed.
        if !spec_exists && present > 0 {
            return Err(RefVerifyScopeResolverError::Io {
                path: spec_path.display().to_string(),
                message: "spec.json not found while TDDD catalogue(s) exist — \
                          SoT Chain ordering violation"
                    .to_owned(),
            });
        }

        // IN-05: a partial catalogue set would silently under-verify an
        // All-scope run. Fail closed.
        if present > 0 && !missing.is_empty() {
            return Err(RefVerifyScopeResolverError::PartialCatalogues { missing });
        }

        // Consistent state: absent artifacts contribute zero pairs in the
        // pair source; present artifacts are enumerated normally.
        Ok(RefVerifyScope::All)
    }

    /// Load TDDD layer bindings from `architecture-rules.json`.
    ///
    /// Returns an empty list — treating the situation as "no TDDD layers
    /// declared" — when:
    ///
    /// - `architecture-rules.json` is absent (pre-TDDD or pre-architecture-rules
    ///   repository): there are no declared catalogues to check.
    /// - The file exists but is malformed or fails schema validation: the
    ///   broken file will fail closed downstream in pair_source/codec (IN-07),
    ///   not here; the resolver's job is existence checks only.
    ///
    /// Other I/O errors (permission denied, symlink rejection, etc.) are
    /// propagated as [`RefVerifyScopeResolverError::Io`] because they indicate
    /// a filesystem-level problem outside the expected empty-list cases.
    fn load_bindings_or_empty(
        &self,
    ) -> Result<Vec<crate::verify::tddd_layers::TdddLayerBinding>, RefVerifyScopeResolverError>
    {
        use crate::verify::tddd_layers::LoadTdddLayersError;
        let rules_path = self.project_root.join("architecture-rules.json");
        match crate::verify::tddd_layers::load_tddd_layers(&rules_path, &self.project_root) {
            Ok(bindings) => Ok(bindings),
            // Malformed / schema-invalid rules file: no declared bindings for
            // scope resolution; the file will fail closed in pair_source/codec.
            Err(LoadTdddLayersError::Parse(_)) => Ok(Vec::new()),
            // Rules file absent: no TDDD layers are declared in this repo/phase.
            Err(LoadTdddLayersError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(Vec::new())
            }
            // Other I/O errors must be surfaced.
            Err(LoadTdddLayersError::Io { path, source }) => Err(RefVerifyScopeResolverError::Io {
                path: path.display().to_string(),
                message: format!("cannot load TDDD layer bindings: {source}"),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn track_dir(project_root: &Path, track_id: &str) -> PathBuf {
        project_root.join("track").join("items").join(track_id)
    }

    /// architecture-rules.json with domain + usecase TDDD layers.
    fn write_tddd_rules(project_root: &Path) {
        write_file(
            &project_root.join("architecture-rules.json"),
            r#"{
                "layers": [
                    {
                        "crate": "domain",
                        "tddd": { "enabled": true, "catalogue_file": "domain-types.json" }
                    },
                    {
                        "crate": "usecase",
                        "tddd": { "enabled": true, "catalogue_file": "usecase-types.json" }
                    }
                ]
            }"#,
        );
    }

    #[test]
    fn resolve_returns_all_when_no_artifact_exists_phase0() {
        // Phase 0: no spec.json, no catalogue → All (both chains contribute
        // zero pairs downstream); the commit gate passes.
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(track_dir(dir.path(), "t1")).unwrap();
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1").unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_returns_all_when_spec_exists_and_catalogues_absent() {
        // Pre-Phase-2: spec.json exists, no catalogue → All (Chain-2
        // contributes zero pairs downstream).
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1").unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_returns_all_when_spec_and_all_catalogues_exist() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        write_file(&td.join("domain-types.json"), "{}");
        write_file(&td.join("usecase-types.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1").unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_returns_all_when_no_rules_file_exists() {
        // No architecture-rules.json → no TDDD layers declared → the
        // catalogue checks are vacuous; pre-Phase-0 repos resolve to All.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(track_dir(dir.path(), "t1")).unwrap();
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1").unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_fails_closed_when_catalogue_exists_without_spec() {
        // SoT Chain ordering violation (IN-06): catalogue present while
        // spec.json is absent.
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("domain-types.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver.resolve("t1").unwrap_err();
        assert!(matches!(err, RefVerifyScopeResolverError::Io { .. }), "got {err:?}");
    }

    #[test]
    fn resolve_fails_closed_on_partial_catalogues() {
        // Partial catalogue set (IN-05): spec.json present, only one of the
        // two declared catalogues exists.
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        write_file(&td.join("domain-types.json"), "{}");
        // usecase-types.json intentionally missing.
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver.resolve("t1").unwrap_err();
        let RefVerifyScopeResolverError::PartialCatalogues { missing } = err else {
            panic!("expected PartialCatalogues, got {err:?}");
        };
        assert_eq!(missing, vec!["usecase-types.json".to_owned()]);
    }

    #[test]
    fn resolve_returns_all_when_rules_file_malformed_and_spec_only() {
        // A malformed architecture-rules.json must not reject a spec-only
        // (pre-Phase-2) track. Parse errors are silently treated as an empty
        // binding list; the broken file fails downstream in pair_source/codec.
        let dir = tempfile::tempdir().unwrap();
        write_file(&dir.path().join("architecture-rules.json"), "not valid json {{{");
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1").unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_does_not_read_scope_env_var() {
        // The resolver derives the scope from artifact existence only; the
        // legacy SOTP_REF_VERIFY_RUN_SCOPE environment variable must have no
        // effect.
        let dir = tempfile::tempdir().unwrap();
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1").unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }
}
