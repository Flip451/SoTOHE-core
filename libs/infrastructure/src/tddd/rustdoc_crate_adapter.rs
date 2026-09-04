//! `RustdocCrateAdapter` — infrastructure adapter for `RustdocCratePort`.
//!
//! - `load_from_path`: reads and decodes one immutable baseline byte vector.
//! - `capture_current`: delegates export, locking, and output snapshot capture
//!   to `RustdocSchemaExporter`, then constructs the domain proof.
//!
//! `workspace_root` is passed to `RustdocSchemaExporter::new` so it knows
//! where to invoke `cargo +nightly rustdoc`.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::{Path, PathBuf};

use domain::FreeText;
use domain::tddd::catalogue_v2::{RustdocCratePort, RustdocCratePortError};
use domain::tddd::type_signals_doc::CapturedRustdocJson;
use domain::tddd::type_signals_doc::{
    AttestedRustdocSnapshot, ImplementationFingerprint, RustdocExecutionIdentity,
    construct_attested_rustdoc_snapshot, construct_captured_rustdoc_json,
};
#[cfg(test)]
use domain::tddd::type_signals_doc::{
    CargoProfileName, ExpectedRustdocJsonPath, ResolvedCargoTargetDirectory, RustdocSnapshot,
    construct_rustdoc_snapshot,
};
use domain::tddd::{CargoFeatureName, catalogue_v2::CrateName};
use usecase::catalogue_impl_signals::ports::{
    EvaluationStartCaptureError, EvaluationStartCapturePort,
};

use crate::schema_export::{RustdocCaptureError, RustdocSchemaExporter};
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
#[cfg(test)]
use crate::tddd::rustdoc_output_lock::RustdocOutputLock;
use crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes;
use crate::tddd::type_signals_evaluator::freshness::{self, RustdocProvider};
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

const MAX_RUSTDOC_JSON_BYTES: u64 = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// RustdocCrateAdapter
// ---------------------------------------------------------------------------

/// Adapter implementing [`RustdocCratePort`].
///
/// - `load_from_path` reads and decodes one immutable baseline byte vector.
/// - `capture_current` uses the common locked exporter and returns one
///   identity-bearing current snapshot.
///
/// `workspace_root` is passed to `RustdocSchemaExporter::new`. Injected into
/// `CatalogueImplSignalsInteractor` at the `apps/cli` composition root.
///
/// [source: ADR 2026-05-11-2330 D2]
pub struct RustdocCrateAdapter {
    workspace_root: PathBuf,
    #[cfg(test)]
    test_exporter: Option<RustdocSchemaExporter>,
    #[cfg(test)]
    test_capture_timeouts: Option<freshness::EvaluationStartTimeouts>,
}

impl RustdocCrateAdapter {
    /// Creates a new adapter for the given workspace root.
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            #[cfg(test)]
            test_exporter: None,
            #[cfg(test)]
            test_capture_timeouts: None,
        }
    }

    #[cfg(test)]
    fn with_test_exporter(workspace_root: PathBuf, exporter: RustdocSchemaExporter) -> Self {
        Self { workspace_root, test_exporter: Some(exporter), test_capture_timeouts: None }
    }

    #[cfg(test)]
    fn with_test_capture_timeouts(
        workspace_root: PathBuf,
        execution: std::time::Duration,
        drain: std::time::Duration,
    ) -> Self {
        Self {
            workspace_root,
            test_exporter: None,
            test_capture_timeouts: Some(freshness::EvaluationStartTimeouts::new(execution, drain)),
        }
    }

    fn capture_current_attested_with_start_fingerprint(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        #[cfg(test)]
        if let Some(exporter) = &self.test_exporter {
            return capture_current_attested_with_start_fingerprint(
                &self.workspace_root,
                crate_name,
                features,
                evaluation_start,
                exporter,
            );
        }

        let exporter = RustdocSchemaExporter::new(self.workspace_root.clone());
        capture_current_attested_with_start_fingerprint(
            &self.workspace_root,
            crate_name,
            features,
            evaluation_start,
            &exporter,
        )
    }
}

impl RustdocCratePort for RustdocCrateAdapter {
    /// Loads a `rustdoc_types::Crate` from the given JSON file path (B-side baseline).
    ///
    /// # Errors
    ///
    /// Returns [`RustdocCratePortError::NotFound`] if the file is absent.
    ///
    /// Returns [`RustdocCratePortError::Io`] if a non-symlink I/O error occurs.
    ///
    /// Returns [`RustdocCratePortError::ParseFailed`] if JSON deserialization or
    /// format-version validation fails.
    fn load_from_path(&self, path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        let diagnostic_crate_name = match path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(|value| CrateName::new(value.to_owned()).ok())
        {
            Some(crate_name) => crate_name,
            None => unknown_crate_name()?,
        };

        reject_symlinks_up_to_root(&self.workspace_root).map_err(|error| {
            RustdocCratePortError::Io {
                path: self.workspace_root.clone(),
                reason: FreeText::new(format!("symlink guard rejected workspace root: {error}")),
            }
        })?;

        // Security: fail-closed symlink guard before reading the baseline JSON.
        //
        // Use `self.workspace_root` as the trusted root so `reject_symlinks_below`
        // walks EVERY ancestor between workspace_root and the leaf.  Previously the
        // caller passed `path.parent()` as the trusted root, which only inspected
        // the leaf — a redirected grandparent (e.g. `track -> /outside` followed
        // by `track/items/id/...-baseline.json`) would bypass the guard because
        // `parent.symlink_metadata()` stats AFTER following the upstream symlink.
        //
        // Per the symlink_guard contract, only components STRICTLY BELOW
        // workspace_root are inspected; the workspace root itself is the caller's
        // composition-root responsibility to ensure is genuine.
        reject_symlinks_below(path, &self.workspace_root).map_err(|e| {
            RustdocCratePortError::Io {
                path: path.to_path_buf(),
                reason: FreeText::new(format!("symlink guard rejected baseline path: {e}")),
            }
        })?;

        // Security: enforce trusted-root containment on every platform. The
        // shared opened-file containment check is Linux-specific, so the
        // canonical path check is the fail-closed guard for other platforms.
        let canonical_root =
            self.workspace_root.canonicalize().map_err(|error| RustdocCratePortError::Io {
                path: self.workspace_root.clone(),
                reason: FreeText::new(format!(
                    "cannot canonicalize trusted workspace root: {error}"
                )),
            })?;
        if !canonical_root.is_dir() {
            return Err(RustdocCratePortError::Io {
                path: self.workspace_root.clone(),
                reason: FreeText::new("trusted workspace root is not a directory"),
            });
        }
        let canonical_path = path.canonicalize().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RustdocCratePortError::NotFound { path: path.to_path_buf() }
            } else {
                RustdocCratePortError::Io {
                    path: path.to_path_buf(),
                    reason: FreeText::new(format!("cannot canonicalize baseline path: {error}")),
                }
            }
        })?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(RustdocCratePortError::Io {
                path: path.to_path_buf(),
                reason: FreeText::new(format!(
                    "baseline path resolves outside trusted workspace root: {}",
                    path.display()
                )),
            });
        }

        let bytes = read_optional_regular_file_bytes(
            &canonical_path,
            Some(&canonical_root),
            MAX_RUSTDOC_JSON_BYTES,
        )
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RustdocCratePortError::NotFound { path: path.to_path_buf() }
            } else {
                RustdocCratePortError::Io {
                    path: path.to_path_buf(),
                    reason: FreeText::new(error.to_string()),
                }
            }
        })?
        .ok_or_else(|| RustdocCratePortError::NotFound { path: path.to_path_buf() })?;
        construct_captured_rustdoc_json(&bytes, decode_rustdoc_bytes).map_err(|error| match error {
            RustdocCratePortError::ParseFailed { reason, .. } => {
                RustdocCratePortError::ParseFailed {
                    crate_name: diagnostic_crate_name.clone(),
                    reason,
                }
            }
            other => RustdocCratePortError::ParseFailed {
                crate_name: diagnostic_crate_name.clone(),
                reason: FreeText::new(other.to_string()),
            },
        })
    }

    /// Captures the current rustdoc graph as one identity-bearing snapshot.
    fn capture_current(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.capture_current_attested_with_start_fingerprint(crate_name, features, evaluation_start)
    }
}

impl EvaluationStartCapturePort for RustdocCrateAdapter {
    /// Captures the run-wide implementation fingerprint before any current
    /// rustdoc export is started.
    fn capture_evaluation_start(
        &self,
    ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError> {
        let fingerprint =
            freshness::rustdoc_implementation_fingerprint_with_timeouts(&self.workspace_root, {
                #[cfg(test)]
                {
                    self.test_capture_timeouts.unwrap_or_default()
                }
                #[cfg(not(test))]
                {
                    freshness::EvaluationStartTimeouts::default()
                }
            });
        fingerprint.map_err(|error| EvaluationStartCaptureError::AuthoritativeInput {
            reason: FreeText::new(format!("cannot fingerprint rustdoc inputs: {error}")),
        })
    }
}

impl RustdocProvider for RustdocCrateAdapter {
    fn capture_current_with_implementation_fingerprint(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.capture_current_attested_with_start_fingerprint(crate_name, features, evaluation_start)
    }

    fn execution_identity(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<RustdocExecutionIdentity, RustdocCratePortError> {
        reject_workspace_root(&self.workspace_root, crate_name)?;
        let identity = RustdocSchemaExporter::new(self.workspace_root.clone())
            .rustdoc_execution_identity(crate_name, features)
            .map(|(identity, _)| identity)
            .map_err(|error| RustdocCratePortError::AuthoritativeInput {
                crate_name: crate_name.clone(),
                reason: FreeText::new(error.to_string()),
            })?;
        crate::schema_export::require_exclusive_rustdoc_target(
            identity.target_directory().as_path(),
        )
        .map_err(|error| RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        })?;
        Ok(identity)
    }
}

fn decode_rustdoc_bytes(bytes: &[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError> {
    let crate_name = unknown_crate_name()?;
    let text = std::str::from_utf8(bytes).map_err(|error| RustdocCratePortError::ParseFailed {
        crate_name: crate_name.clone(),
        reason: FreeText::new(format!("rustdoc JSON is not UTF-8: {error}")),
    })?;
    BaselineRustdocCodec::from_json(text).map_err(|error| RustdocCratePortError::ParseFailed {
        crate_name,
        reason: FreeText::new(error.to_string()),
    })
}

fn unknown_crate_name() -> Result<CrateName, RustdocCratePortError> {
    CrateName::new("unknown").map_err(|error| RustdocCratePortError::Io {
        path: PathBuf::from("<unknown-rustdoc-crate>"),
        reason: FreeText::new(format!("cannot construct diagnostic crate name: {error}")),
    })
}

fn map_capture_error(crate_name: &CrateName, error: RustdocCaptureError) -> RustdocCratePortError {
    match error {
        RustdocCaptureError::AuthoritativeInput(error) => {
            RustdocCratePortError::AuthoritativeInput {
                crate_name: crate_name.clone(),
                reason: FreeText::new(error.to_string()),
            }
        }
        RustdocCaptureError::CaptureFailed(error) => RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        },
        RustdocCaptureError::ParseFailed(error) => match error {
            RustdocCratePortError::ParseFailed { reason, .. } => {
                RustdocCratePortError::ParseFailed { crate_name: crate_name.clone(), reason }
            }
            other => RustdocCratePortError::ParseFailed {
                crate_name: crate_name.clone(),
                reason: FreeText::new(other.to_string()),
            },
        },
    }
}

fn workspace_input_fingerprint(
    workspace_root: &Path,
    crate_name: &CrateName,
) -> Result<ImplementationFingerprint, RustdocCratePortError> {
    freshness::rustdoc_implementation_fingerprint(workspace_root).map_err(|error| {
        RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(format!("cannot fingerprint rustdoc inputs: {error}")),
        }
    })
}

fn capture_current_attested_with_start_fingerprint(
    workspace_root: &Path,
    crate_name: &CrateName,
    features: &[CargoFeatureName],
    evaluation_start: &ImplementationFingerprint,
    exporter: &RustdocSchemaExporter,
) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
    reject_workspace_root(workspace_root, crate_name)?;
    let start_fingerprint = workspace_input_fingerprint(workspace_root, crate_name)?;
    reject_unexpected_workspace_fingerprint(
        crate_name,
        evaluation_start,
        &start_fingerprint,
        "before",
    )?;
    let (identity, bytes) = exporter
        .capture_rustdoc_json_classified(crate_name, features)
        .map_err(|error| map_capture_error(crate_name, error))?;
    require_exclusive_target(crate_name, identity.target_directory().as_path())?;
    let end_fingerprint = workspace_input_fingerprint(workspace_root, crate_name)?;
    reject_unexpected_workspace_fingerprint(
        crate_name,
        evaluation_start,
        &end_fingerprint,
        "after",
    )?;
    construct_attested_rustdoc_snapshot(
        evaluation_start.clone(),
        identity,
        &bytes,
        decode_rustdoc_bytes,
    )
    .map_err(|error| RustdocCratePortError::ParseFailed {
        crate_name: crate_name.clone(),
        reason: FreeText::new(error.to_string()),
    })
}

fn reject_unexpected_workspace_fingerprint(
    crate_name: &CrateName,
    evaluation_start: &ImplementationFingerprint,
    observed: &ImplementationFingerprint,
    phase: &str,
) -> Result<(), RustdocCratePortError> {
    if evaluation_start == observed {
        Ok(())
    } else {
        Err(RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(format!(
                "workspace implementation changed during type-signal evaluation: fingerprint {phase} rustdoc export disagrees with evaluation-start snapshot"
            )),
        })
    }
}

fn reject_workspace_root(
    workspace_root: &Path,
    crate_name: &CrateName,
) -> Result<(), RustdocCratePortError> {
    validate_workspace_root(workspace_root).map_err(|reason| {
        RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(reason),
        }
    })
}

fn validate_workspace_root(workspace_root: &Path) -> Result<(), String> {
    #[cfg(not(unix))]
    {
        return Err(format!(
            "descriptor-relative no-follow rustdoc locks are supported only on Unix (workspace root '{}')",
            workspace_root.display()
        ));
    }

    #[cfg(unix)]
    {
        reject_symlinks_up_to_root(workspace_root).map_err(|error| {
            format!(
                "symlink guard: refusing to use workspace root '{}': {error}",
                workspace_root.display()
            )
        })?;
        let metadata = workspace_root.symlink_metadata().map_err(|error| {
            format!("cannot inspect trusted workspace root '{}': {error}", workspace_root.display())
        })?;
        if !metadata.is_dir() {
            return Err(format!(
                "trusted workspace root '{}' is not a directory",
                workspace_root.display()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
fn capture_current_with_exporter<F>(
    crate_name: &CrateName,
    features: &[CargoFeatureName],
    export: F,
) -> Result<RustdocSnapshot, RustdocCratePortError>
where
    F: FnOnce(
        &CrateName,
        &[CargoFeatureName],
    ) -> Result<PathBuf, domain::schema::SchemaExportError>,
{
    let json_path =
        export(crate_name, features).map_err(|e| RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: FreeText::new(e.to_string()),
        })?;
    let target_directory =
        json_path.parent().and_then(Path::parent).ok_or_else(|| RustdocCratePortError::Io {
            path: json_path.clone(),
            reason: FreeText::new("rustdoc JSON path has no target directory"),
        })?;
    require_exclusive_target(crate_name, target_directory)?;
    let lock = RustdocOutputLock::acquire(target_directory).map_err(|error| {
        RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        }
    })?;
    let target_directory = ResolvedCargoTargetDirectory::try_new(target_directory.to_path_buf())
        .map_err(|error| RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        })?;
    let expected_json_path = ExpectedRustdocJsonPath::try_new(json_path.clone(), &target_directory)
        .map_err(|error| RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        })?;
    let bytes = lock.read_bytes(&json_path, MAX_RUSTDOC_JSON_BYTES).map_err(|error| {
        RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        }
    })?;
    let profile = CargoProfileName::try_new("dev".to_owned()).map_err(|error| {
        RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        }
    })?;
    let identity = RustdocExecutionIdentity::new(
        target_directory,
        crate_name.clone(),
        features.to_vec(),
        profile,
        expected_json_path,
    )
    .map_err(|error| RustdocCratePortError::AuthoritativeInput {
        crate_name: crate_name.clone(),
        reason: FreeText::new(error.to_string()),
    })?;
    construct_rustdoc_snapshot(identity, &bytes, decode_rustdoc_bytes).map_err(|error| {
        RustdocCratePortError::ParseFailed {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        }
    })
}

fn require_exclusive_target(
    crate_name: &CrateName,
    target_directory: &Path,
) -> Result<(), RustdocCratePortError> {
    crate::schema_export::require_exclusive_rustdoc_target(target_directory).map_err(|error| {
        RustdocCratePortError::AuthoritativeInput {
            crate_name: crate_name.clone(),
            reason: FreeText::new(error.to_string()),
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use domain::tddd::type_signals_doc::Sha256Digest;
    #[cfg(unix)]
    use std::time::{Duration, Instant};
    #[cfg(unix)]
    use temp_env;

    #[cfg(unix)]
    fn fixed_evaluation_start_fingerprint() -> ImplementationFingerprint {
        ImplementationFingerprint::new(Sha256Digest::try_new("a".repeat(64)).unwrap())
    }

    #[cfg(unix)]
    fn write_test_executable(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt as _;

        std::fs::write(path, contents).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    fn write_test_rustup(commands: &Path) {
        write_test_executable(
            &commands.join("rustup"),
            r#"#!/bin/sh
if [ "$1" = "run" ] && [ "$2" = "nightly" ] && [ "$3" = "rustc" ]; then
    exit 0
fi
if [ "$1" = "which" ] && [ "$2" = "--toolchain" ] && [ "$3" = "nightly" ]; then
    case "$4" in
        cargo|rustc|rustdoc)
            toolchain="${SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR:-$(dirname "$0")}"
            printf '%s/%s\n' "$toolchain" "$4"
            exit 0
            ;;
    esac
fi
exit 1
"#,
        );
    }

    #[cfg(unix)]
    fn write_metadata_test_toolchain(commands: &Path) {
        write_test_executable(
            &commands.join("cargo"),
            "#!/bin/sh\nexec /bin/cat \"$SOTOHE_TEST_CARGO_METADATA\"\n",
        );
        write_test_executable(&commands.join("rustc"), "#!/bin/sh\nexit 0\n");
        write_test_executable(&commands.join("rustdoc"), "#!/bin/sh\nexit 0\n");
        write_test_rustup(commands);
    }

    #[cfg(unix)]
    fn prepend_test_command_path(commands: &Path) -> std::ffi::OsString {
        let mut entries = vec![commands.to_path_buf()];
        entries.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        std::env::join_paths(entries).unwrap()
    }

    #[cfg(unix)]
    fn write_metadata_fixture(metadata_path: &Path, target_directory: &Path) {
        std::fs::write(
            metadata_path,
            serde_json::json!({
                "packages": [{
                    "name": "lockfail",
                    "targets": [{"kind": ["lib"], "name": "lockfail"}]
                }],
                "target_directory": target_directory,
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn test_load_from_path_nonexistent_file_returns_not_found() {
        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let path = Path::new("/nonexistent/path/does-not-exist.json");
        let err = adapter.load_from_path(path).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::NotFound { .. }),
            "expected NotFound, got: {err}"
        );
    }

    #[test]
    fn test_load_from_path_hashes_and_decodes_the_same_immutable_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let crate_data = rustdoc_types::Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: std::collections::HashMap::new(),
            paths: std::collections::HashMap::new(),
            external_crates: std::collections::HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        };
        let bytes = serde_json::to_vec(&crate_data).unwrap();
        let path = dir.path().join("domain-types-baseline.json");
        std::fs::write(&path, &bytes).unwrap();

        let adapter = RustdocCrateAdapter::new(dir.path().to_path_buf());
        let captured = adapter.load_from_path(&path).unwrap();
        let expected = construct_captured_rustdoc_json(&bytes, decode_rustdoc_bytes).unwrap();

        assert_eq!(captured.json_hash(), expected.json_hash());
        assert_eq!(captured.crate_data(), expected.crate_data());

        std::fs::write(&path, b"generation-b").unwrap();
        assert_eq!(captured.json_hash(), expected.json_hash());
        assert_eq!(captured.crate_data(), expected.crate_data());
    }

    #[test]
    fn test_load_from_path_invalid_json_returns_parse_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types-baseline.json");
        std::fs::write(&path, "{ not valid json }").unwrap();

        let adapter = RustdocCrateAdapter::new(dir.path().to_path_buf());
        let err = adapter.load_from_path(&path).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::ParseFailed { .. }),
            "expected ParseFailed, got: {err}"
        );
    }

    #[test]
    fn test_load_from_path_outside_workspace_root_returns_io_error() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let workspace_root = workspace.path().canonicalize().unwrap();
        let outside_root = outside.path().canonicalize().unwrap();
        let path = outside_root.join("domain-types-baseline.json");
        std::fs::write(&path, "{}").unwrap();

        let adapter = RustdocCrateAdapter::new(workspace_root);
        let err = adapter.load_from_path(&path).unwrap_err();

        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "expected Io for a baseline outside the trusted workspace root, got: {err}"
        );
    }

    #[test]
    fn test_load_from_path_relative_escape_returns_io_error() {
        let parent = tempfile::tempdir().unwrap();
        let parent_root = parent.path().canonicalize().unwrap();
        let workspace = parent_root.join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let outside = parent_root.join("outside-baseline.json");
        std::fs::write(&outside, "{}").unwrap();

        let escaped_path = workspace.join("../outside-baseline.json");
        let adapter = RustdocCrateAdapter::new(workspace);
        let err = adapter.load_from_path(&escaped_path).unwrap_err();

        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "a relative path escaping the trusted root must fail closed: {err}"
        );
        assert!(
            err.to_string().contains("outside trusted workspace root"),
            "the failure must identify the containment rejection: {err}"
        );
    }

    #[test]
    fn test_load_from_path_unverifiable_workspace_root_returns_io_error() {
        let parent = tempfile::tempdir().unwrap();
        let parent_root = parent.path().canonicalize().unwrap();
        let workspace = parent_root.join("missing-workspace");
        let path = workspace.join("domain-types-baseline.json");
        let adapter = RustdocCrateAdapter::new(workspace);

        let err = adapter.load_from_path(&path).unwrap_err();

        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "an unverifiable trusted root must fail closed: {err}"
        );
        assert!(
            err.to_string().contains("canonicalize trusted workspace root"),
            "the failure must identify the unavailable trusted root: {err}"
        );
    }

    #[test]
    fn test_load_from_path_non_directory_workspace_root_returns_io_error() {
        let parent = tempfile::tempdir().unwrap();
        let parent_root = parent.path().canonicalize().unwrap();
        let workspace = parent_root.join("workspace-file");
        std::fs::write(&workspace, b"not a directory").unwrap();
        let adapter = RustdocCrateAdapter::new(workspace.clone());

        let err = adapter.load_from_path(&workspace).unwrap_err();

        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "a non-directory trusted root must fail closed: {err}"
        );
        assert!(
            err.to_string().contains("not a directory"),
            "the failure must identify the invalid trusted root: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_from_path_symlinked_file_returns_io_error() {
        // Security: a symlinked baseline JSON (leaf) must be rejected before loading.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-baseline.json");
        std::fs::write(&real, "{}").unwrap();

        let link = dir.path().join("domain-types-baseline.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let err = adapter.load_from_path(&link).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "expected Io (symlink rejection), got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_from_path_symlinked_parent_dir_returns_io_error() {
        // Security: reading through a symlinked parent directory (e.g. symlinked track dir)
        // must be rejected before the leaf check.
        let dir = tempfile::tempdir().unwrap();
        let real_sub = dir.path().join("real-sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        std::fs::write(real_sub.join("domain-types-baseline.json"), "{}").unwrap();

        let link_sub = dir.path().join("link-sub");
        std::os::unix::fs::symlink(&real_sub, &link_sub).unwrap();

        let path = link_sub.join("domain-types-baseline.json");
        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let err = adapter.load_from_path(&path).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "expected Io (symlinked parent directory rejection), got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_symlinked_workspace_root_returns_authoritative_input() {
        // Security: a symlinked workspace root must be rejected before invoking the exporter.
        let dir = tempfile::tempdir().unwrap();
        let real_ws = dir.path().join("real-workspace");
        std::fs::create_dir_all(&real_ws).unwrap();

        let link_ws = dir.path().join("link-workspace");
        std::os::unix::fs::symlink(&real_ws, &link_ws).unwrap();

        let adapter = RustdocCrateAdapter::new(link_ws);
        let crate_name = CrateName::new("some_crate".to_owned()).unwrap();
        let evaluation_start = fixed_evaluation_start_fingerprint();
        let err = adapter.capture_current(&crate_name, &[], &evaluation_start).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::AuthoritativeInput { .. }),
            "expected AuthoritativeInput (symlink workspace_root rejection), got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_execution_identity_unverifiable_workspace_root_returns_authoritative_input() {
        let parent = tempfile::tempdir().unwrap();
        let parent_root = parent.path().canonicalize().unwrap();
        let workspace = parent_root.join("missing-workspace");
        let target = parent_root.join("cargo-target");
        let adapter = RustdocCrateAdapter::new(workspace);
        let crate_name = CrateName::new("some_crate").unwrap();

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            temp_env::with_vars([("CARGO_TARGET_DIR", Some(target.as_os_str()))], || {
                let err = adapter.execution_identity(&crate_name, &[]).unwrap_err();
                assert!(
                    matches!(err, RustdocCratePortError::AuthoritativeInput { .. }),
                    "an unverifiable trusted root must reject identity admission: {err}"
                );
                assert!(
                    err.to_string().contains("cannot inspect trusted workspace root"),
                    "the failure must identify the unavailable trusted root: {err}"
                );
            });
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_unverifiable_workspace_root_returns_authoritative_input() {
        let parent = tempfile::tempdir().unwrap();
        let workspace = parent.path().join("missing-workspace");
        let adapter = RustdocCrateAdapter::new(workspace);

        let error = adapter.capture_evaluation_start().unwrap_err();

        assert!(
            matches!(error, EvaluationStartCaptureError::AuthoritativeInput { .. }),
            "an unverifiable workspace root must reject the evaluation-start fingerprint: {error}"
        );
        assert!(
            error.to_string().contains("cannot inspect trusted workspace root"),
            "the failure must identify the unavailable trusted root: {error}"
        );
        assert!(
            !error.to_string().contains("crate"),
            "the run-level error must not invent a crate identity: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_once_returns_complete_crate_independent_fingerprint() {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(workspace.path().join("src")).unwrap();
            std::fs::write(workspace.path().join("Cargo.toml"), "[workspace]\nmembers = []\n")
                .unwrap();
            std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
            std::fs::write(workspace.path().join("src/lib.rs"), b"pub struct Fixture;\n").unwrap();

            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            let commands = tempfile::tempdir().unwrap();
            let nightly = tempfile::tempdir().unwrap();
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            write_metadata_test_toolchain(commands.path());
            for tool in ["cargo", "rustc", "rustdoc"] {
                write_test_executable(
                    &nightly.path().join(tool),
                    &format!("nightly {tool} generation-a\n"),
                );
            }
            std::fs::write(
                &metadata_path,
                serde_json::json!({
                    "packages": [],
                    "target_directory": target_directory,
                })
                .to_string(),
            )
            .unwrap();

            let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
            let path = prepend_test_command_path(commands.path());
            temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(nightly.path().as_os_str())),
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                ],
                || {
                    let fingerprint = match adapter.capture_evaluation_start() {
                        Ok(fingerprint) => fingerprint,
                        Err(error) => {
                            panic!(
                                "one complete run-wide fingerprint must be authoritative: {error}"
                            )
                        }
                    };
                    assert_eq!(fingerprint.as_digest().as_str().len(), 64);
                    assert!(
                        fingerprint
                            .as_digest()
                            .as_str()
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                        "the run-wide fingerprint must be a complete lowercase SHA-256 digest"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_fingerprint_tracks_each_authoritative_input() {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            let source_directory = workspace.path().join("src");
            std::fs::create_dir_all(&source_directory).unwrap();
            let source = source_directory.join("lib.rs");
            let source_a = b"pub struct Fixture;\n";
            let source_b = b"pub struct FixtureChanged;\n";
            std::fs::write(workspace.path().join("Cargo.toml"), "[workspace]\nmembers = []\n")
                .unwrap();
            std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
            std::fs::write(&source, source_a).unwrap();

            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            let commands = tempfile::tempdir().unwrap();
            let nightly = tempfile::tempdir().unwrap();
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            write_metadata_test_toolchain(commands.path());
            for tool in ["cargo", "rustc", "rustdoc"] {
                write_test_executable(
                    &nightly.path().join(tool),
                    &format!("nightly {tool} generation-a\n"),
                );
            }
            let metadata_a = serde_json::json!({
                "packages": [],
                "target_directory": target_directory,
                "metadata_marker": "generation-a",
            })
            .to_string();
            let metadata_b = serde_json::json!({
                "packages": [],
                "target_directory": target_directory,
                "metadata_marker": "generation-b",
            })
            .to_string();
            std::fs::write(&metadata_path, &metadata_a).unwrap();

            let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
            let path = prepend_test_command_path(commands.path());
            temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(nightly.path().as_os_str())),
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("CARGO_ENCODED_RUSTFLAGS", None::<&std::ffi::OsStr>),
                    ("RUSTDOCFLAGS", None::<&std::ffi::OsStr>),
                    ("RUSTFLAGS", None::<&std::ffi::OsStr>),
                ],
                || {
                    let baseline = adapter.capture_evaluation_start().unwrap();

                    std::fs::write(&source, source_b).unwrap();
                    let workspace_changed = adapter.capture_evaluation_start().unwrap();
                    assert_ne!(
                        baseline, workspace_changed,
                        "changing a workspace file must change the real adapter fingerprint"
                    );
                    std::fs::write(&source, source_a).unwrap();
                    assert_eq!(
                        baseline,
                        adapter.capture_evaluation_start().unwrap(),
                        "restoring the workspace file must restore the real adapter fingerprint"
                    );

                    for tool in ["cargo", "rustc", "rustdoc"] {
                        write_test_executable(
                            &nightly.path().join(tool),
                            &format!("nightly {tool} generation-b\n"),
                        );
                        let nightly_changed = adapter.capture_evaluation_start().unwrap();
                        assert_ne!(
                            baseline, nightly_changed,
                            "changing nightly-selected {tool} must change the real adapter fingerprint"
                        );
                        write_test_executable(
                            &nightly.path().join(tool),
                            &format!("nightly {tool} generation-a\n"),
                        );
                        assert_eq!(
                            baseline,
                            adapter.capture_evaluation_start().unwrap(),
                            "restoring nightly-selected {tool} must restore the real adapter fingerprint"
                        );
                    }

                    std::fs::write(&metadata_path, &metadata_b).unwrap();
                    let metadata_changed = adapter.capture_evaluation_start().unwrap();
                    assert_ne!(
                        baseline, metadata_changed,
                        "changing cargo metadata bytes must change the real adapter fingerprint"
                    );
                    std::fs::write(&metadata_path, &metadata_a).unwrap();
                    assert_eq!(
                        baseline,
                        adapter.capture_evaluation_start().unwrap(),
                        "restoring cargo metadata bytes must restore the real adapter fingerprint"
                    );

                    for (name, changed_value) in [
                        ("CARGO_ENCODED_RUSTFLAGS", "-C\x1fopt-level=1"),
                        ("RUSTDOCFLAGS", "--cfg=env_identity_changed"),
                        ("RUSTFLAGS", "-C opt-level=1"),
                    ] {
                        let environment_changed =
                            temp_env::with_var(name, Some(changed_value), || {
                                adapter.capture_evaluation_start().unwrap()
                            });
                        assert_ne!(
                            baseline, environment_changed,
                            "changing {name} must change the real adapter fingerprint"
                        );
                        assert_eq!(
                            baseline,
                            adapter.capture_evaluation_start().unwrap(),
                            "restoring {name} must restore the real adapter fingerprint"
                        );
                    }
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_uses_production_bounds_and_locked_metadata_without_export() {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, _, _) = lockfail_workspace();
            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            write_metadata_test_toolchain(&commands);

            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            write_metadata_fixture(&metadata_path, &target_directory);
            let observations = tempfile::tempdir().unwrap();
            let cargo_args = observations.path().join("cargo-args");
            write_test_executable(
                &commands.join("cargo"),
                r#"#!/bin/sh
printf '%s\n' "$@" >> "$SOTOHE_TEST_CARGO_ARGS"
if [ "$1" = "metadata" ]; then
    exec /bin/cat "$SOTOHE_TEST_CARGO_METADATA"
fi
exit 1
"#,
            );

            let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
            let path = prepend_test_command_path(&commands);
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_ARGS", Some(cargo_args.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                    ("SOTOHE_TEST_RUSTDOC_OUTPUT_LOCK_TIMEOUT_MS", Some(std::ffi::OsStr::new("0"))),
                ],
                || {
                    let defaults = freshness::EvaluationStartTimeouts::default();
                    assert_eq!(
                        freshness::EVALUATION_START_EXECUTION_TIMEOUT,
                        Duration::from_secs(120),
                        "evaluation-start execution bound must remain the D1 120-second bound"
                    );
                    assert_eq!(
                        freshness::EVALUATION_START_DRAIN_TIMEOUT,
                        Duration::from_secs(1),
                        "evaluation-start drain bound must remain the D1 one-second bound"
                    );
                    assert_eq!(defaults.execution, freshness::EVALUATION_START_EXECUTION_TIMEOUT);
                    assert_eq!(defaults.drain, freshness::EVALUATION_START_DRAIN_TIMEOUT);

                    let fingerprint = adapter.capture_evaluation_start().unwrap();
                    assert_eq!(fingerprint.as_digest().as_str().len(), 64);

                    let args = std::fs::read_to_string(&cargo_args)
                        .unwrap()
                        .lines()
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        args,
                        vec![
                            "metadata".to_owned(),
                            "--format-version".to_owned(),
                            "1".to_owned(),
                            "--no-deps".to_owned(),
                            "--locked".to_owned(),
                        ],
                        "evaluation-start metadata must use the locked no-dependencies command"
                    );
                    assert!(
                        !args.iter().any(|argument| argument == "rustdoc"),
                        "evaluation-start capture must not invoke the D6 rustdoc export path"
                    );
                    assert!(
                        !target_directory.join(".sotp-rustdoc").exists(),
                        "evaluation-start capture must not acquire a D6 selection-directory lock"
                    );
                    assert_eq!(
                        crate::tddd::rustdoc_output_lock::RUSTDOC_OUTPUT_LOCK_TIMEOUT,
                        Duration::from_secs(120),
                        "the separate D6 selection-directory lock bound must remain 120 seconds"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_metadata_execution_timeout_returns_authoritative_input_without_partial_fingerprint()
     {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, _, _) = lockfail_workspace();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let started_marker = workspace.path().join("metadata-started");
            write_test_executable(
                &commands.join("cargo"),
                r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
    printf '%s\n' '{"packages":[]}'
    : > "$SOTOHE_TEST_METADATA_STARTED"
    exec /bin/sleep 5
fi
exit 1
"#,
            );
            let adapter = RustdocCrateAdapter::with_test_capture_timeouts(
                workspace.path().to_path_buf(),
                Duration::from_millis(75),
                Duration::from_millis(250),
            );
            let path = prepend_test_command_path(&commands);

            temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_METADATA_STARTED", Some(started_marker.as_os_str())),
                ],
                || {
                    let started = Instant::now();
                    let error = adapter.capture_evaluation_start().unwrap_err();

                    assert!(
                        started.elapsed() < Duration::from_secs(2),
                        "metadata execution timeout must not wait for the child: {:?}",
                        started.elapsed()
                    );
                    assert!(
                        matches!(error, EvaluationStartCaptureError::AuthoritativeInput { .. }),
                        "metadata execution timeout must fail at the run-level port: {error}"
                    );
                    assert!(
                        error.to_string().contains("timed out"),
                        "the execution timeout must be preserved in the diagnostic: {error}"
                    );
                    assert!(
                        !error.to_string().contains("layer")
                            && !error.to_string().contains("crate"),
                        "a run-wide timeout must not fabricate layer or crate attribution: {error}"
                    );
                    assert!(
                        started_marker.exists(),
                        "the timeout fixture must reach cargo metadata before it is stopped"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_metadata_drain_timeout_returns_authoritative_input_without_fallback()
     {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, _, _) = lockfail_workspace();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let started_marker = workspace.path().join("metadata-started");
            write_test_executable(
                &commands.join("cargo"),
                r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
    printf '%s\n' '{"packages":[]}'
    : > "$SOTOHE_TEST_METADATA_STARTED"
    (exec /bin/sleep 5) &
    exit 0
fi
exit 1
"#,
            );
            let adapter = RustdocCrateAdapter::with_test_capture_timeouts(
                workspace.path().to_path_buf(),
                Duration::from_secs(2),
                Duration::from_millis(75),
            );
            let path = prepend_test_command_path(&commands);

            temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_METADATA_STARTED", Some(started_marker.as_os_str())),
                ],
                || {
                    let started = Instant::now();
                    let error = adapter.capture_evaluation_start().unwrap_err();

                    assert!(
                        started.elapsed() < Duration::from_secs(2),
                        "metadata drain timeout must not wait for the inherited pipe: {:?}",
                        started.elapsed()
                    );
                    assert!(
                        matches!(error, EvaluationStartCaptureError::AuthoritativeInput { .. }),
                        "metadata drain timeout must fail at the run-level port: {error}"
                    );
                    assert!(
                        error.to_string().contains("timed out"),
                        "the drain timeout must be preserved in the diagnostic: {error}"
                    );
                    assert!(
                        !error.to_string().contains("layer")
                            && !error.to_string().contains("crate"),
                        "a run-wide drain timeout must not fabricate layer or crate attribution: {error}"
                    );
                    assert!(
                        started_marker.exists(),
                        "the timeout fixture must reach cargo metadata before its pipe is held"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_nightly_resolution_timeout_returns_authoritative_input_without_fallback()
     {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, _, _) = lockfail_workspace();
            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            write_metadata_test_toolchain(&commands);
            let metadata_path = workspace.path().join("metadata.json");
            write_metadata_fixture(&metadata_path, &target_directory);
            let started_marker = workspace.path().join("nightly-resolution-started");
            write_test_executable(
                &commands.join("rustup"),
                r#"#!/bin/sh
if [ "$1" = "which" ]; then
    : > "$SOTOHE_TEST_NIGHTLY_STARTED"
    exec /bin/sleep 5
fi
exit 1
"#,
            );
            let adapter = RustdocCrateAdapter::with_test_capture_timeouts(
                workspace.path().to_path_buf(),
                Duration::from_millis(75),
                Duration::from_millis(250),
            );
            let path = prepend_test_command_path(&commands);

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_STARTED", Some(started_marker.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let started = Instant::now();
                    let error = adapter.capture_evaluation_start().unwrap_err();

                    assert!(
                        started.elapsed() < Duration::from_secs(2),
                        "nightly resolution timeout must not wait for the child: {:?}",
                        started.elapsed()
                    );
                    assert!(
                        matches!(error, EvaluationStartCaptureError::AuthoritativeInput { .. }),
                        "nightly resolution timeout must fail at the run-level port: {error}"
                    );
                    assert!(
                        error.to_string().contains("timed out"),
                        "the nightly execution timeout must be preserved in the diagnostic: {error}"
                    );
                    assert!(
                        !error.to_string().contains("layer")
                            && !error.to_string().contains("crate"),
                        "a run-wide nightly timeout must not fabricate layer or crate attribution: {error}"
                    );
                    assert!(
                        started_marker.exists(),
                        "the timeout fixture must reach nightly tool resolution"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_workspace_walk_timeout_returns_authoritative_input_without_partial_fingerprint()
     {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, _, _) = lockfail_workspace();
            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            write_metadata_test_toolchain(&commands);
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            write_metadata_fixture(&metadata_path, &target_directory);

            let inputs = workspace.path().join("workspace-inputs");
            std::fs::create_dir_all(&inputs).unwrap();
            for index in 0..32_000 {
                std::fs::write(inputs.join(format!("input-{index:05}.rs")), b"").unwrap();
            }

            let adapter = RustdocCrateAdapter::with_test_capture_timeouts(
                workspace.path().to_path_buf(),
                Duration::from_millis(250),
                Duration::from_millis(250),
            );
            let path = prepend_test_command_path(&commands);

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let started = Instant::now();
                    let error = adapter.capture_evaluation_start().unwrap_err();

                    assert!(
                        started.elapsed() < Duration::from_secs(2),
                        "workspace-walk timeout must not wait beyond the test bound: {:?}",
                        started.elapsed()
                    );
                    assert!(
                        matches!(error, EvaluationStartCaptureError::AuthoritativeInput { .. }),
                        "workspace-walk timeout must fail at the run-level port: {error}"
                    );
                    assert!(
                        error.to_string().contains("workspace walk")
                            && error.to_string().contains("timed out"),
                        "the workspace-walk timeout must be preserved in the diagnostic: {error}"
                    );
                    assert!(
                        !error.to_string().contains("layer")
                            && !error.to_string().contains("crate"),
                        "a run-wide timeout must not fabricate layer or crate attribution: {error}"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_evaluation_start_nightly_resolution_drain_timeout_returns_authoritative_input_without_fallback()
     {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, _, _) = lockfail_workspace();
            let target_directory = workspace.path().join("cargo-target");
            std::fs::create_dir_all(&target_directory).unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            write_metadata_test_toolchain(&commands);
            let metadata = tempfile::tempdir().unwrap();
            let metadata_path = metadata.path().join("cargo-metadata.json");
            write_metadata_fixture(&metadata_path, &target_directory);
            let started_marker = workspace.path().join("nightly-resolution-started");
            write_test_executable(
                &commands.join("rustup"),
                r#"#!/bin/sh
if [ "$1" = "which" ]; then
    : > "$SOTOHE_TEST_NIGHTLY_STARTED"
    (exec /bin/sleep 5) &
    exit 0
fi
exit 1
"#,
            );

            let adapter = RustdocCrateAdapter::with_test_capture_timeouts(
                workspace.path().to_path_buf(),
                Duration::from_secs(2),
                Duration::from_millis(75),
            );
            let path = prepend_test_command_path(&commands);

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_STARTED", Some(started_marker.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let started = Instant::now();
                    let error = adapter.capture_evaluation_start().unwrap_err();

                    assert!(
                        started.elapsed() < Duration::from_secs(2),
                        "nightly output-drain timeout must not wait for the inherited pipe: {:?}",
                        started.elapsed()
                    );
                    assert!(
                        matches!(error, EvaluationStartCaptureError::AuthoritativeInput { .. }),
                        "nightly output-drain timeout must fail at the run-level port: {error}"
                    );
                    assert!(
                        error.to_string().contains("output drain timed out"),
                        "the nightly drain timeout must be preserved in the diagnostic: {error}"
                    );
                    assert!(
                        !error.to_string().contains("layer")
                            && !error.to_string().contains("crate"),
                        "a run-wide drain timeout must not fabricate layer or crate attribution: {error}"
                    );
                    assert!(
                        started_marker.exists(),
                        "the timeout fixture must reach nightly tool resolution"
                    );
                },
            );
        });
    }

    #[cfg(not(unix))]
    #[test]
    fn test_execution_identity_non_unix_returns_authoritative_input() {
        let workspace = tempfile::tempdir().unwrap();
        let workspace_root = workspace.path().canonicalize().unwrap();
        let adapter = RustdocCrateAdapter::new(workspace_root);
        let crate_name = CrateName::new("some_crate").unwrap();

        let err = adapter.execution_identity(&crate_name, &[]).unwrap_err();

        assert!(
            matches!(err, RustdocCratePortError::AuthoritativeInput { .. }),
            "non-Unix rustdoc identity admission must fail closed: {err}"
        );
        assert!(
            err.to_string().contains("supported only on Unix"),
            "the failure must identify the unsupported platform: {err}"
        );
    }

    #[test]
    fn test_capture_current_with_exporter_forwards_declared_features() {
        let crate_name = CrateName::new("domain".to_owned()).unwrap();
        let features = [CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()];
        let observed_features = Arc::new(Mutex::new(Vec::new()));
        let observed_features_for_export = Arc::clone(&observed_features);

        let error =
            capture_current_with_exporter(&crate_name, &features, move |_target, selected| {
                *observed_features_for_export.lock().unwrap() =
                    selected.iter().map(|feature| feature.as_str().to_owned()).collect();
                Err(domain::schema::SchemaExportError::RustdocFailed(
                    "stub exporter failure".to_owned(),
                ))
            })
            .unwrap_err();

        assert!(matches!(error, RustdocCratePortError::CaptureFailed { .. }));
        assert_eq!(*observed_features.lock().unwrap(), vec!["semantic-dup"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_lock_operation_failure_does_not_retry_or_reuse_json() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"lockfail\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(workspace.path().join("src")).unwrap();
        std::fs::write(workspace.path().join("src/lib.rs"), "pub struct Fixture;\n").unwrap();

        let target = workspace.path().join("target-area");
        let commands = workspace.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        write_metadata_test_toolchain(&commands);
        let metadata_path = workspace.path().join("cargo-metadata.json");
        write_metadata_fixture(&metadata_path, &target);

        let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
        let crate_name = CrateName::new("lockfail").unwrap();

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let path = prepend_test_command_path(&commands);
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let evaluation_start = adapter.capture_evaluation_start().unwrap();
                    let exporter = RustdocSchemaExporter::new(workspace.path().to_path_buf());
                    let (identity, expected_path) =
                        exporter.rustdoc_execution_identity(&crate_name, &[]).unwrap();
                    let exclusive_target = identity.target_directory().as_path().to_path_buf();
                    std::fs::create_dir_all(expected_path.parent().unwrap()).unwrap();
                    let stale_json = format!(
                        r#"{{"root":0,"crate_version":"stale","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
                        rustdoc_types::FORMAT_VERSION
                    );
                    std::fs::write(&expected_path, stale_json.as_bytes()).unwrap();
                    std::fs::create_dir(exclusive_target.join(".sotp-rustdoc-json.lock")).unwrap();

                    let error =
                        adapter.capture_current(&crate_name, &[], &evaluation_start).unwrap_err();
                    assert!(
                        matches!(error, RustdocCratePortError::AuthoritativeInput { .. }),
                        "lock-operation failure must fail closed: {error}"
                    );
                    assert_eq!(std::fs::read(&expected_path).unwrap(), stale_json.as_bytes());
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_rustdoc_crate_adapter_lock_timeout_is_fail_closed_without_retry() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::time::Duration;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let (workspace, adapter, crate_name) = lockfail_workspace();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            write_metadata_test_toolchain(&commands);

            let target_directory = workspace.path().join("cargo-target");
            let metadata_path = workspace.path().join("metadata.json");
            std::fs::write(
                &metadata_path,
                serde_json::json!({
                    "packages": [{
                        "name": "lockfail",
                        "targets": [{"kind": ["lib"], "name": "lockfail"}]
                    }],
                    "target_directory": target_directory,
                })
                .to_string(),
            )
            .unwrap();
            let invocations = workspace.path().join("rustdoc-invocations");
            let cargo = commands.join("cargo");
            std::fs::write(
                &cargo,
                b"#!/bin/sh\nif [ \"$1\" = \"metadata\" ]; then cat \"$SOTOHE_TEST_CARGO_METADATA\"; exit 0; fi\n: > \"$SOTOHE_TEST_ADAPTER_INVOCATIONS\"\nexit 1\n",
            )
            .unwrap();
            std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
            let path = prepend_test_command_path(&commands);

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                    ("SOTOHE_TEST_ADAPTER_INVOCATIONS", Some(invocations.as_os_str())),
                    (
                        "SOTOHE_TEST_RUSTDOC_OUTPUT_LOCK_TIMEOUT_MS",
                        Some(std::ffi::OsStr::new("25")),
                    ),
                ],
                || {
                    let evaluation_start = adapter.capture_evaluation_start().unwrap();
                    let identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let selection = identity.target_directory().as_path().to_path_buf();
                    let held = RustdocOutputLock::acquire(&selection).unwrap();

                    assert_eq!(
                        crate::tddd::rustdoc_output_lock::RUSTDOC_OUTPUT_LOCK_TIMEOUT,
                        Duration::from_secs(120),
                        "the adapter lock policy must remain bounded at 120 seconds"
                    );

                    let error =
                        adapter.capture_current(&crate_name, &[], &evaluation_start).unwrap_err();
                    assert!(
                        matches!(error, RustdocCratePortError::AuthoritativeInput { .. }),
                        "a lock timeout must fail closed at the adapter boundary: {error}"
                    );
                    assert!(
                        error.to_string().contains("timed out"),
                        "the adapter must preserve the bounded lock timeout: {error}"
                    );
                    assert!(
                        !invocations.exists(),
                        "a timed-out lock must not retry into rustdoc export"
                    );
                    drop(held);
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_holds_one_selection_lock_through_path_check_and_byte_copy() {
        use std::sync::mpsc::channel;
        use std::time::{Duration, Instant};

        let (workspace, _, crate_name) = lockfail_workspace();
        let cargo_target = workspace.path().join("cargo-target");
        std::fs::create_dir_all(&cargo_target).unwrap();
        let commands = workspace.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        write_metadata_test_toolchain(&commands);
        let metadata_path = workspace.path().join("cargo-metadata.json");
        write_metadata_fixture(&metadata_path, &cargo_target);

        let export_started = cargo_target.join("capture-export-started");
        let release_export = cargo_target.join("capture-release-export");
        let rustdoc_json = format!(
            r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        );
        write_test_executable(
            &commands.join("cargo"),
            &format!(
                r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
  exec /bin/cat "$SOTOHE_TEST_CARGO_METADATA"
fi
: > "$SOTOHE_TEST_EXPORT_STARTED"
while [ ! -f "$SOTOHE_TEST_RELEASE_EXPORT" ]; do sleep 0.01; done
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s\n' '{rustdoc_json}' > "$CARGO_TARGET_DIR/doc/lockfail.json"
"#
            ),
        );

        let (selection_started_tx, selection_started_rx) = channel();
        let (release_selection_tx, release_selection_rx) = channel();
        let (copy_completed_tx, copy_completed_rx) = channel();
        let (release_read_tx, release_read_rx) = channel();
        let release_selection_rx = Arc::new(Mutex::new(release_selection_rx));
        let release_read_rx = Arc::new(Mutex::new(release_read_rx));
        let release_selection_rx_for_hook = Arc::clone(&release_selection_rx);
        let release_read_rx_for_hook = Arc::clone(&release_read_rx);
        let exporter = RustdocSchemaExporter::with_capture_hooks(
            workspace.path().to_path_buf(),
            std::sync::Arc::new(move || {
                selection_started_tx.send(()).unwrap();
                release_selection_rx_for_hook.lock().unwrap().recv().unwrap();
            }),
            std::sync::Arc::new(move || {
                copy_completed_tx.send(()).unwrap();
                release_read_rx_for_hook.lock().unwrap().recv().unwrap();
            }),
        );
        let adapter =
            RustdocCrateAdapter::with_test_exporter(workspace.path().to_path_buf(), exporter);
        let path = prepend_test_command_path(&commands);

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(cargo_target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                    ("SOTOHE_TEST_EXPORT_STARTED", Some(export_started.as_os_str())),
                    ("SOTOHE_TEST_RELEASE_EXPORT", Some(release_export.as_os_str())),
                ],
                || {
                    let evaluation_start = adapter.capture_evaluation_start().unwrap();
                    let identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let selection_directory = identity.target_directory().as_path().to_path_buf();

                    let first_crate_name = crate_name.clone();
                    let first = std::thread::spawn(move || {
                        adapter.capture_current(&first_crate_name, &[], &evaluation_start)
                    });

                    let cleanup = || {
                        let _ = release_selection_tx.send(());
                        let _ = std::fs::write(&release_export, b"release");
                        let _ = release_read_tx.send(());
                    };
                    let wait_for_file = |path: &Path| {
                        let deadline = Instant::now() + Duration::from_secs(2);
                        while !path.exists() && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        path.exists()
                    };

                    if selection_started_rx.recv_timeout(Duration::from_secs(2)).is_err() {
                        cleanup();
                        panic!(
                            "capture_current did not reach expected-path selection while its exporter lock was held"
                        );
                    }

                    let (contender_started_tx, contender_started_rx) = channel();
                    let (acquired_tx, acquired_rx) = channel();
                    let contender_directory = selection_directory.clone();
                    let contender = std::thread::spawn(move || {
                        contender_started_tx.send(()).unwrap();
                        let result = RustdocOutputLock::acquire_for_test(
                            &contender_directory,
                            Duration::from_secs(5),
                        );
                        if result.is_ok() {
                            acquired_tx.send(()).unwrap();
                        }
                        result
                    });
                    contender_started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
                    let assert_contender_blocked = |stage: &str| {
                        if acquired_rx.recv_timeout(Duration::from_millis(100)).is_ok() {
                            cleanup();
                            panic!("a competing writer acquired the selection lock during {stage}");
                        }
                    };

                    assert_contender_blocked("expected-path selection");
                    release_selection_tx.send(()).unwrap();

                    assert!(
                        wait_for_file(&export_started),
                        "capture_current did not reach the fake rustdoc export barrier"
                    );
                    assert_contender_blocked("rustdoc export");
                    std::fs::write(&release_export, b"release").unwrap();

                    copy_completed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                    assert_contender_blocked("returned-path validation and completed byte copy");
                    release_read_tx.send(()).unwrap();

                    let snapshot = first.join().unwrap().unwrap();
                    let contender_lock = contender.join().unwrap().unwrap();
                    drop(contender_lock);
                    assert_eq!(
                        snapshot.snapshot().crate_data().crate_version.as_deref(),
                        Some("generation-a")
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_constructs_identity_bearing_snapshot_from_locked_bytes() {
        let workspace = tempfile::tempdir().unwrap();
        let output = workspace.path().join(".sotp-rustdoc").join("fixture/doc/domain.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        let json = format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        );
        std::fs::write(&output, json.as_bytes()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let snapshot =
            capture_current_with_exporter(
                &crate_name,
                &[],
                |_target, _features| Ok(output.clone()),
            )
            .unwrap();

        assert_eq!(snapshot.execution_identity().crate_name(), &crate_name);
        assert_eq!(snapshot.crate_data().format_version, rustdoc_types::FORMAT_VERSION);
        assert_eq!(snapshot.json_hash().as_digest().as_str().len(), 64);
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_pairs_execution_identity_components() {
        let workspace = tempfile::tempdir().unwrap();
        let output = workspace.path().join(".sotp-rustdoc/fixture/doc/domain.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        let json = format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        );
        std::fs::write(&output, json.as_bytes()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let features = [CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()];
        let expected_features = features.clone();
        let expected_path = output.clone();

        let snapshot = capture_current_with_exporter(&crate_name, &features, move |_, selected| {
            assert_eq!(selected, expected_features.as_slice());
            Ok(expected_path)
        })
        .unwrap();
        let identity = snapshot.execution_identity();

        assert_eq!(identity.crate_name(), &crate_name);
        assert_eq!(identity.features(), features.as_slice());
        assert_eq!(identity.profile().as_str(), "dev");
        assert_eq!(
            identity.target_directory().as_path(),
            output.parent().unwrap().parent().unwrap()
        );
        assert_eq!(identity.expected_json_path().as_path(), output);
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_rejects_oversized_json_without_fallback() {
        let workspace = tempfile::tempdir().unwrap();
        let output = workspace.path().join(".sotp-rustdoc/fixture/doc/domain.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::File::create(&output).unwrap().set_len(MAX_RUSTDOC_JSON_BYTES + 1).unwrap();
        let crate_name = CrateName::new("domain").unwrap();

        let error =
            capture_current_with_exporter(&crate_name, &[], move |_, _| Ok(output)).unwrap_err();

        assert!(matches!(error, RustdocCratePortError::AuthoritativeInput { .. }));
        assert!(
            error.to_string().contains("exceeds"),
            "an oversized rustdoc capture must fail at the bounded read: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_rejects_non_immediate_private_or_shared_target() {
        let workspace = tempfile::tempdir().unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let json = format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        );
        let outputs = [
            workspace.path().join(".sotp-rustdoc/selection/nested/doc/domain.json"),
            workspace.path().join("cargo-target/doc/domain.json"),
        ];

        for output in outputs {
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();
            std::fs::write(&output, &json).unwrap();
            let selected_output = output.clone();
            let result =
                capture_current_with_exporter(&crate_name, &[], move |_target, _features| {
                    Ok(selected_output)
                });
            assert!(result.is_err(), "non-exclusive rustdoc targets must fail closed");
            let error = result.unwrap_err();

            assert!(matches!(error, RustdocCratePortError::AuthoritativeInput { .. }));
            assert!(
                error.to_string().contains("not an exclusive .sotp-rustdoc area"),
                "the target ownership failure must be explicit: {error}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_rejects_symlinked_selection_directory() {
        let (workspace, adapter, crate_name) = lockfail_workspace();
        let cargo_target = workspace.path().join("cargo-target");
        let commands = workspace.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        write_metadata_test_toolchain(&commands);
        let metadata_path = workspace.path().join("cargo-metadata.json");
        write_metadata_fixture(&metadata_path, &cargo_target);

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let path = prepend_test_command_path(&commands);
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(cargo_target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let valid_identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let evaluation_start = adapter.capture_evaluation_start().unwrap();
                    let selection = valid_identity.target_directory().as_path().to_path_buf();
                    let exclusive_parent = selection.parent().unwrap();
                    assert_eq!(
                        exclusive_parent.file_name(),
                        Some(std::ffi::OsStr::new(".sotp-rustdoc")),
                        "the selection must be directly below the private rustdoc parent"
                    );

                    let redirected = workspace.path().join("redirected-selection");
                    std::fs::create_dir_all(&redirected).unwrap();
                    std::fs::create_dir_all(exclusive_parent).unwrap();
                    std::os::unix::fs::symlink(&redirected, &selection).unwrap();

                    let capture_error =
                        adapter.capture_current(&crate_name, &[], &evaluation_start).unwrap_err();
                    assert!(
                        matches!(capture_error, RustdocCratePortError::AuthoritativeInput { .. }),
                        "a symlinked selection directory must reject capture admission: {capture_error}"
                    );
                    assert!(
                        capture_error.to_string().contains("symlink"),
                        "the capture rejection must identify the symlink guard: {capture_error}"
                    );

                    let identity_error = adapter.execution_identity(&crate_name, &[]).unwrap_err();
                    assert!(
                        matches!(identity_error, RustdocCratePortError::AuthoritativeInput { .. }),
                        "a symlinked selection directory must reject identity admission: {identity_error}"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_keeps_locked_byte_snapshot_when_output_file_is_replaced() {
        let workspace = tempfile::tempdir().unwrap();
        let output = workspace.path().join(".sotp-rustdoc").join("fixture/doc/domain.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        let first = format!(
            r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        );
        std::fs::write(&output, first.as_bytes()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let snapshot =
            capture_current_with_exporter(
                &crate_name,
                &[],
                |_target, _features| Ok(output.clone()),
            )
            .unwrap();
        let original_hash = snapshot.json_hash().clone();

        std::fs::write(&output, b"generation-b").unwrap();
        assert_eq!(snapshot.json_hash(), &original_hash);
        assert_eq!(snapshot.crate_data().crate_version.as_deref(), Some("generation-a"));
        assert_ne!(std::fs::read(&output).unwrap(), first.as_bytes());
    }

    #[test]
    fn test_capture_current_discards_result_when_workspace_fingerprint_changes() {
        let crate_name = CrateName::new("domain").unwrap();
        let start = ImplementationFingerprint::new(Sha256Digest::try_new("a".repeat(64)).unwrap());
        let end = ImplementationFingerprint::new(Sha256Digest::try_new("b".repeat(64)).unwrap());
        let error = reject_unexpected_workspace_fingerprint(&crate_name, &start, &end, "after")
            .unwrap_err();
        assert!(
            matches!(error, RustdocCratePortError::AuthoritativeInput { .. }),
            "changed input fingerprint must discard the capture: {error}"
        );
        reject_unexpected_workspace_fingerprint(&crate_name, &start, &start, "after").unwrap();
    }

    #[test]
    fn test_rustdoc_crate_adapter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_capture_ports<T: RustdocCratePort + EvaluationStartCapturePort>() {}

        assert_send_sync::<RustdocCrateAdapter>();
        assert_capture_ports::<RustdocCrateAdapter>();
    }

    #[cfg(unix)]
    fn lockfail_workspace() -> (tempfile::TempDir, RustdocCrateAdapter, CrateName) {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"lockfail\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(workspace.path().join("src")).unwrap();
        std::fs::write(workspace.path().join("src/lib.rs"), "pub struct Fixture;\n").unwrap();
        let adapter = RustdocCrateAdapter::new(workspace.path().to_path_buf());
        let crate_name = CrateName::new("lockfail").unwrap();
        (workspace, adapter, crate_name)
    }

    #[cfg(unix)]
    #[test]
    fn test_execution_identity_owns_exclusive_sotp_rustdoc_target() {
        let (workspace, adapter, crate_name) = lockfail_workspace();
        let cargo_target = workspace.path().join("cargo-target");
        let commands = workspace.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        write_metadata_test_toolchain(&commands);
        let metadata_path = workspace.path().join("cargo-metadata.json");
        write_metadata_fixture(&metadata_path, &cargo_target);
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let path = prepend_test_command_path(&commands);
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(cargo_target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let exclusive = identity.target_directory().as_path();
                    assert!(
                        exclusive.starts_with(cargo_target.join(".sotp-rustdoc")),
                        "cooperative writers must own a private .sotp-rustdoc subtree: {}",
                        exclusive.display()
                    );
                    assert!(
                        identity.expected_json_path().as_path().starts_with(exclusive),
                        "expected rustdoc JSON must stay inside the exclusive target"
                    );
                    assert!(
                        !identity
                            .expected_json_path()
                            .as_path()
                            .starts_with(cargo_target.join("doc")),
                        "the shared Cargo rustdoc directory must not be authoritative"
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_non_cooperative_parent_cargo_target_json_is_not_authoritative() {
        let (workspace, adapter, crate_name) = lockfail_workspace();
        let cargo_target = workspace.path().join("cargo-target");
        let commands = workspace.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        write_metadata_test_toolchain(&commands);
        let metadata_path = workspace.path().join("cargo-metadata.json");
        write_metadata_fixture(&metadata_path, &cargo_target);
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let path = prepend_test_command_path(&commands);
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(cargo_target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let identity = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let shared_json = cargo_target.join("doc").join("lockfail.json");
                    std::fs::create_dir_all(shared_json.parent().unwrap()).unwrap();
                    std::fs::write(&shared_json, b"non-cooperative-writer").unwrap();
                    assert_ne!(
                        identity.expected_json_path().as_path(),
                        shared_json.as_path(),
                        "JSON written outside the exclusive lock boundary must not be the expected output"
                    );
                    crate::schema_export::require_exclusive_rustdoc_target(
                        identity.target_directory().as_path(),
                    )
                    .unwrap();
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_cooperative_writers_serialize_on_the_exclusive_target_lock() {
        use std::time::Duration;

        let (workspace, adapter, crate_name) = lockfail_workspace();
        let cargo_target = workspace.path().join("cargo-target");
        let commands = workspace.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        write_metadata_test_toolchain(&commands);
        let metadata_path = workspace.path().join("cargo-metadata.json");
        write_metadata_fixture(&metadata_path, &cargo_target);
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let path = prepend_test_command_path(&commands);
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(cargo_target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                    ("SOTOHE_TEST_CARGO_METADATA", Some(metadata_path.as_os_str())),
                    ("SOTOHE_TEST_NIGHTLY_TOOLCHAIN_DIR", Some(commands.as_os_str())),
                ],
                || {
                    let first = adapter.execution_identity(&crate_name, &[]).unwrap();
                    let second = adapter.execution_identity(&crate_name, &[]).unwrap();
                    assert_eq!(
                        first.target_directory(),
                        second.target_directory(),
                        "cooperative writers for one selection must share the exclusive target"
                    );
                    let exclusive = first.target_directory().as_path().to_path_buf();
                    let held = RustdocOutputLock::acquire(&exclusive).unwrap();
                    let contender = exclusive.clone();
                    let contention = std::thread::spawn(move || {
                        crate::tddd::rustdoc_output_lock::RustdocOutputLock::acquire_for_test(
                            &contender,
                            Duration::from_millis(25),
                        )
                    })
                    .join()
                    .unwrap()
                    .unwrap_err();
                    assert!(
                        contention.to_string().contains("timed out"),
                        "cooperative writers must serialize through the exclusive lock: {contention}"
                    );
                    drop(held);
                },
            );
        });
    }
}
