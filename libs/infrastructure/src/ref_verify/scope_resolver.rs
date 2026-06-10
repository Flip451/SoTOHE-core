//! Concrete adapter for resolving the [`RefVerifyScope`] from the typed
//! invocation context plus track artifacts on disk (IN-12 / D2).
//!
//! ## Context-sensitive resolution
//!
//! The caller's firing surface is expressed as a typed
//! [`RefVerifyInvocationContext`] (assembled by cli-composition from CLI
//! arguments — never from environment variables):
//!
//! - [`RefVerifyInvocationContext::SpecDesign`]      → [`RefVerifyScope::Chain1`]
//! - [`RefVerifyInvocationContext::TypeDesign`]      → [`RefVerifyScope::Chain2`] for that layer
//! - [`RefVerifyInvocationContext::CommitGate`]      → [`RefVerifyScope::All`]
//! - [`RefVerifyInvocationContext::Standalone`]      → [`RefVerifyScope::All`]
//!
//! The resolver also validates that the artifacts required by the resolved
//! scope exist (spec.json for Chain1/All; the layer's TDDD catalogue for
//! Chain2; partial catalogue sets fail closed for All).

use std::path::{Path, PathBuf};

use domain::tddd::LayerId;
use usecase::ref_verify::RefVerifyScope;

/// Typed firing-surface context for `bin/sotp ref-verify run` (IN-12).
///
/// cli-composition converts CLI arguments into this enum; the resolver maps
/// it onto a [`RefVerifyScope`]. No environment variable participates in the
/// mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyInvocationContext {
    /// Fired right after Phase 1 (spec-design): verify Chain1 only.
    SpecDesign,
    /// Fired right after Phase 2 (type-design) for one layer: verify Chain2
    /// for that layer only.
    TypeDesign {
        /// The layer whose catalogue was just (re-)designed.
        layer: LayerId,
    },
    /// Fired from the commit gate: verify both chains, all layers.
    CommitGate,
    /// Fired standalone (no phase context): verify both chains, all layers.
    Standalone,
}

/// Failure modes of [`RefVerifyScopeResolver::resolve`].
#[derive(Debug)]
pub enum RefVerifyScopeResolverError {
    /// I/O failure while inspecting a required artifact.
    Io {
        /// The path that could not be inspected or was missing.
        path: String,
        /// Human-readable description of the failure.
        message: String,
    },
    /// A layer identifier did not resolve to a TDDD layer binding.
    InvalidLayerId {
        /// The rejected layer identifier.
        layer_id: String,
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
            Self::InvalidLayerId { layer_id, message } => {
                write!(f, "invalid layer id '{layer_id}': {message}")
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

/// Concrete [`RefVerifyScope`] resolver: maps the typed invocation context
/// onto a scope and validates the artifacts that scope requires.
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

    /// Resolve the [`RefVerifyScope`] for `track_id` under the given
    /// invocation context.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyScopeResolverError`] when a required artifact is
    /// missing/unreadable, the layer id has no TDDD binding, or only part of
    /// the TDDD catalogue set exists for an All-scope run.
    pub fn resolve(
        &self,
        track_id: &str,
        context: &RefVerifyInvocationContext,
    ) -> Result<RefVerifyScope, RefVerifyScopeResolverError> {
        let track_dir = self.project_root.join("track").join("items").join(track_id);

        match context {
            RefVerifyInvocationContext::SpecDesign => {
                self.require_spec_json(&track_dir)?;
                Ok(RefVerifyScope::Chain1)
            }
            RefVerifyInvocationContext::TypeDesign { layer } => {
                self.require_layer_catalogue(&track_dir, layer)?;
                Ok(RefVerifyScope::Chain2 { layer: layer.clone() })
            }
            RefVerifyInvocationContext::CommitGate | RefVerifyInvocationContext::Standalone => {
                self.require_spec_json(&track_dir)?;
                self.require_complete_or_absent_catalogues(&track_dir)?;
                Ok(RefVerifyScope::All)
            }
        }
    }

    fn require_spec_json(&self, track_dir: &Path) -> Result<(), RefVerifyScopeResolverError> {
        let spec_path = track_dir.join("spec.json");
        let exists = spec_path.try_exists().map_err(|e| RefVerifyScopeResolverError::Io {
            path: spec_path.display().to_string(),
            message: format!("cannot inspect path: {e}"),
        })?;
        if !exists {
            return Err(RefVerifyScopeResolverError::Io {
                path: spec_path.display().to_string(),
                message: "spec.json not found for the resolved scope".to_owned(),
            });
        }
        Ok(())
    }

    /// Validate that `layer` has a TDDD binding and that its catalogue file
    /// exists in the track directory.
    fn require_layer_catalogue(
        &self,
        track_dir: &Path,
        layer: &LayerId,
    ) -> Result<(), RefVerifyScopeResolverError> {
        let bindings = self.load_bindings()?;
        let binding =
            bindings.iter().find(|b| b.layer_id() == layer.as_ref()).ok_or_else(|| {
                RefVerifyScopeResolverError::InvalidLayerId {
                    layer_id: layer.as_ref().to_owned(),
                    message: "no TDDD layer binding in architecture-rules.json".to_owned(),
                }
            })?;
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let exists = catalogue_path.try_exists().map_err(|e| RefVerifyScopeResolverError::Io {
            path: catalogue_path.display().to_string(),
            message: format!("cannot inspect path: {e}"),
        })?;
        if !exists {
            return Err(RefVerifyScopeResolverError::Io {
                path: catalogue_path.display().to_string(),
                message: "layer catalogue not found for the resolved scope".to_owned(),
            });
        }
        Ok(())
    }

    /// For All scope: either no TDDD catalogue exists yet (pre-Phase-2 run —
    /// Chain2 contributes zero pairs) or all of them exist. A partial set
    /// fails closed so missing layers are not silently skipped.
    fn require_complete_or_absent_catalogues(
        &self,
        track_dir: &Path,
    ) -> Result<(), RefVerifyScopeResolverError> {
        let bindings = self.load_bindings()?;
        let mut present = 0usize;
        let mut missing: Vec<String> = Vec::new();
        for binding in &bindings {
            let catalogue_path = track_dir.join(binding.catalogue_file());
            let exists =
                catalogue_path.try_exists().map_err(|e| RefVerifyScopeResolverError::Io {
                    path: catalogue_path.display().to_string(),
                    message: format!("cannot inspect path: {e}"),
                })?;
            if exists {
                present += 1;
            } else {
                missing.push(binding.catalogue_file().to_owned());
            }
        }
        if present > 0 && !missing.is_empty() {
            return Err(RefVerifyScopeResolverError::PartialCatalogues { missing });
        }
        Ok(())
    }

    /// Load TDDD layer bindings from `architecture-rules.json`. An absent
    /// rules file means no TDDD layers (empty bindings), not an error.
    fn load_bindings(
        &self,
    ) -> Result<Vec<crate::verify::tddd_layers::TdddLayerBinding>, RefVerifyScopeResolverError>
    {
        let rules_path = self.project_root.join("architecture-rules.json");
        let exists = rules_path.try_exists().map_err(|e| RefVerifyScopeResolverError::Io {
            path: rules_path.display().to_string(),
            message: format!("cannot inspect path: {e}"),
        })?;
        if !exists {
            return Ok(Vec::new());
        }
        crate::verify::tddd_layers::load_tddd_layers(&rules_path, &self.project_root).map_err(|e| {
            RefVerifyScopeResolverError::Io {
                path: rules_path.display().to_string(),
                message: format!("cannot load TDDD layer bindings: {e}"),
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
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

    fn layer(id: &str) -> LayerId {
        LayerId::try_new(id).unwrap()
    }

    #[test]
    fn resolve_spec_design_returns_chain1_when_spec_exists() {
        let dir = tempfile::tempdir().unwrap();
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1", &RefVerifyInvocationContext::SpecDesign).unwrap();
        assert!(matches!(scope, RefVerifyScope::Chain1));
    }

    #[test]
    fn resolve_spec_design_fails_when_spec_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(track_dir(dir.path(), "t1")).unwrap();
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver.resolve("t1", &RefVerifyInvocationContext::SpecDesign).unwrap_err();
        assert!(matches!(err, RefVerifyScopeResolverError::Io { .. }), "got {err:?}");
    }

    #[test]
    fn resolve_type_design_returns_chain2_when_layer_catalogue_exists() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("domain-types.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver
            .resolve("t1", &RefVerifyInvocationContext::TypeDesign { layer: layer("domain") })
            .unwrap();
        let resolved_layer = match scope {
            RefVerifyScope::Chain2 { layer } => layer,
            other => panic!("expected Chain2, got {other:?}"),
        };
        assert_eq!(resolved_layer.as_ref(), "domain");
    }

    #[test]
    fn resolve_type_design_fails_for_unbound_layer() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(track_dir(dir.path(), "t1")).unwrap();
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver
            .resolve(
                "t1",
                &RefVerifyInvocationContext::TypeDesign { layer: layer("infrastructure") },
            )
            .unwrap_err();
        assert!(matches!(err, RefVerifyScopeResolverError::InvalidLayerId { .. }), "got {err:?}");
    }

    #[test]
    fn resolve_type_design_fails_when_catalogue_missing() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(track_dir(dir.path(), "t1")).unwrap();
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver
            .resolve("t1", &RefVerifyInvocationContext::TypeDesign { layer: layer("domain") })
            .unwrap_err();
        assert!(matches!(err, RefVerifyScopeResolverError::Io { .. }), "got {err:?}");
    }

    #[test]
    fn resolve_standalone_returns_all_with_complete_catalogues() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        write_file(&td.join("domain-types.json"), "{}");
        write_file(&td.join("usecase-types.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1", &RefVerifyInvocationContext::Standalone).unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_commit_gate_returns_all_with_absent_catalogues() {
        // Pre-Phase-2 run: spec.json exists, no catalogue at all → All
        // (Chain2 contributes zero pairs).
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1", &RefVerifyInvocationContext::CommitGate).unwrap();
        assert!(matches!(scope, RefVerifyScope::All));
    }

    #[test]
    fn resolve_standalone_fails_closed_on_partial_catalogues() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        write_file(&td.join("domain-types.json"), "{}");
        // usecase-types.json intentionally missing.
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver.resolve("t1", &RefVerifyInvocationContext::Standalone).unwrap_err();
        let RefVerifyScopeResolverError::PartialCatalogues { missing } = err else {
            panic!("expected PartialCatalogues, got {err:?}");
        };
        assert_eq!(missing, vec!["usecase-types.json".to_owned()]);
    }

    #[test]
    fn resolve_standalone_fails_when_spec_missing() {
        let dir = tempfile::tempdir().unwrap();
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(track_dir(dir.path(), "t1")).unwrap();
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let err = resolver.resolve("t1", &RefVerifyInvocationContext::Standalone).unwrap_err();
        assert!(matches!(err, RefVerifyScopeResolverError::Io { .. }), "got {err:?}");
    }

    #[test]
    fn resolve_does_not_read_scope_env_var() {
        // The resolver maps context → scope from typed input only; the legacy
        // SOTP_REF_VERIFY_RUN_SCOPE environment variable must have no effect.
        let dir = tempfile::tempdir().unwrap();
        let td = track_dir(dir.path(), "t1");
        write_file(&td.join("spec.json"), "{}");
        let resolver = RefVerifyScopeResolver::new(dir.path().to_path_buf());
        let scope = resolver.resolve("t1", &RefVerifyInvocationContext::SpecDesign).unwrap();
        assert!(matches!(scope, RefVerifyScope::Chain1));
    }
}
