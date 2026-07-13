//! Cargo-metadata-backed rustdoc target resolution.
//!
//! The package name in a catalogue and the root emitted by rustdoc can differ:
//! Cargo selects a library target where one exists, otherwise a bin target. This
//! module owns that translation for schema export, catalogue import, and signal
//! evaluation so no caller guesses from package names or keeps a static map.

use std::path::{Path, PathBuf};
use std::process::Command;

use domain::schema::SchemaExportError;
use domain::tddd::catalogue_v2::CrateName;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use thiserror::Error;

use super::path_resolution::resolve_target_dir;

/// Validated name of a Cargo target selected for rustdoc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoTargetName(String);

impl CargoTargetName {
    /// Validates a Cargo target name and its corresponding rustdoc root.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocRootResolutionError::InvalidTargetName`] when the
    /// target is empty or its `-`-to-`_` normalized root is not a Rust
    /// identifier.
    pub fn try_new(value: String) -> Result<Self, RustdocRootResolutionError> {
        if value.trim().is_empty() {
            return invalid_target_name(value, "cargo metadata target name is empty".to_owned());
        }
        let normalized = value.replace('-', "_");
        if let Err(error) = CrateName::new(normalized) {
            return invalid_target_name(
                value.clone(),
                format!("cargo target `{value}` cannot form a rustdoc crate root: {error}"),
            );
        }
        Ok(Self(value))
    }

    /// Returns the Cargo target name as written in metadata.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kind of target Cargo selected for rustdoc.
#[derive(Debug, PartialEq, Eq)]
pub enum RustdocTargetKind {
    /// A library target, preferred whenever the package defines one.
    Library,
    /// A bin target selected from a bin-only package.
    Binary,
}

impl Copy for RustdocTargetKind {}

impl Clone for RustdocTargetKind {
    fn clone(&self) -> Self {
        *self
    }
}

/// Typed result of translating a package name to its rustdoc target and root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustdocTargetResolution {
    package_name: CrateName,
    target_name: CargoTargetName,
    rustdoc_root_name: CrateName,
    target_kind: RustdocTargetKind,
}

impl RustdocTargetResolution {
    /// Returns the catalogue package name supplied to the resolver.
    #[must_use]
    pub fn package_name(&self) -> &CrateName {
        &self.package_name
    }

    /// Returns the selected Cargo target name.
    #[must_use]
    pub fn target_name(&self) -> &CargoTargetName {
        &self.target_name
    }

    /// Returns the normalized root segment emitted by rustdoc paths.
    #[must_use]
    pub fn rustdoc_root_name(&self) -> &CrateName {
        &self.rustdoc_root_name
    }

    /// Returns whether Cargo selected a library or bin target.
    #[must_use]
    pub fn target_kind(&self) -> RustdocTargetKind {
        self.target_kind
    }
}

/// Failure while resolving a package's Cargo target and rustdoc root.
#[derive(Debug, Error)]
pub enum RustdocRootResolutionError {
    /// Cargo metadata could not be executed or returned a failure status.
    #[error("cargo metadata command failed: {}", .0.as_str())]
    MetadataCommand(DiagnosticMessage),
    /// Cargo metadata output could not be decoded as its JSON format.
    #[error("cargo metadata output could not be decoded: {}", .0.as_str())]
    MetadataDecode(DiagnosticMessage),
    /// The requested package is absent from cargo metadata.
    #[error("package `{0}` was not found in cargo metadata")]
    PackageNotFound(CrateName),
    /// Cargo metadata did not choose one deterministic rustdoc target.
    #[error("cargo target selection failed: {}", .0.as_str())]
    TargetSelection(DiagnosticMessage),
    /// A selected target name could not form a valid rustdoc root.
    #[error("invalid cargo target name: {}", .0.as_str())]
    InvalidTargetName(DiagnosticMessage),
}

/// Resolves the Cargo target and normalized rustdoc root for a package.
///
/// A library target wins over all bin targets. A bin-only package selects its
/// sole bin, or its `default_run` bin when several are present. Every other
/// ambiguous shape fails closed.
pub fn resolve_rustdoc_root_name(
    workspace_root: &Path,
    package_name: &CrateName,
) -> Result<RustdocTargetResolution, RustdocRootResolutionError> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| {
            resolution_error(
                format!("cannot run cargo metadata: {error}"),
                package_name,
                RustdocRootResolutionError::MetadataCommand,
            )
        })?;
    if !output.status.success() {
        return Err(resolution_error(
            format!(
                "cargo metadata exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            package_name,
            RustdocRootResolutionError::MetadataCommand,
        ));
    }

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        resolution_error(
            format!("cargo metadata JSON parse error: {error}"),
            package_name,
            RustdocRootResolutionError::MetadataDecode,
        )
    })?;
    let packages =
        metadata.get("packages").and_then(serde_json::Value::as_array).ok_or_else(|| {
            resolution_error(
                "cargo metadata does not contain a packages array",
                package_name,
                RustdocRootResolutionError::MetadataDecode,
            )
        })?;
    let package = packages
        .iter()
        .find(|package| {
            package.get("name").and_then(serde_json::Value::as_str) == Some(package_name.as_str())
        })
        .ok_or_else(|| RustdocRootResolutionError::PackageNotFound(package_name.clone()))?;
    target_resolution_from_package(package, package_name)
}

pub(super) fn run_rustdoc(
    workspace_root: &Path,
    crate_name: &str,
) -> Result<PathBuf, SchemaExportError> {
    let package_name = CrateName::new(crate_name.to_owned()).map_err(|error| {
        SchemaExportError::RustdocFailed(format!(
            "invalid catalogue crate name `{crate_name}`: {error}"
        ))
    })?;
    let resolution =
        resolve_rustdoc_root_name(workspace_root, &package_name).map_err(schema_export_error)?;
    let args = match resolution.target_kind() {
        RustdocTargetKind::Library => build_rustdoc_args(crate_name, &["--lib"]),
        RustdocTargetKind::Binary => {
            build_rustdoc_args(crate_name, &["--bin", resolution.target_name().as_str()])
        }
    };
    let output = Command::new("cargo")
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|error| SchemaExportError::RustdocFailed(error.to_string()))?;
    if !output.status.success() {
        return Err(SchemaExportError::RustdocFailed(format!(
            "cargo rustdoc for package `{crate_name}` target `{}` failed: {}",
            resolution.target_name().as_str(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let target_dir = resolve_target_dir(workspace_root)?;
    let path =
        target_dir.join("doc").join(format!("{}.json", resolution.rustdoc_root_name().as_str()));
    super::ensure_rustdoc_json_path_safe(&target_dir, &path, "cargo rustdoc")?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(SchemaExportError::RustdocFailed(format!(
            "expected rustdoc JSON for target `{}` at {} but file not found",
            resolution.target_name().as_str(),
            path.display()
        )))
    }
}

fn build_rustdoc_args(crate_name: &str, target: &[&str]) -> Vec<String> {
    let mut args = vec!["+nightly".into(), "rustdoc".into(), "-p".into(), crate_name.into()];
    args.extend(target.iter().map(|argument| (*argument).into()));
    args.extend(
        ["--", "-Z", "unstable-options", "--output-format", "json", "--document-hidden-items"]
            .map(Into::into),
    );
    args
}

fn target_resolution_from_package(
    package: &serde_json::Value,
    package_name: &CrateName,
) -> Result<RustdocTargetResolution, RustdocRootResolutionError> {
    let targets =
        package.get("targets").and_then(serde_json::Value::as_array).ok_or_else(|| {
            resolution_error(
                format!("package `{package_name}` has no targets array"),
                package_name,
                RustdocRootResolutionError::TargetSelection,
            )
        })?;
    let libraries = target_names_of_kind(targets, "lib")?;
    let (target_name, target_kind) = match libraries.as_slice() {
        [library] => (library.clone(), RustdocTargetKind::Library),
        [] => select_bin_target(package, targets, package_name)?,
        _ => {
            return Err(resolution_error(
                format!("package `{package_name}` has multiple library targets"),
                package_name,
                RustdocRootResolutionError::TargetSelection,
            ));
        }
    };
    let normalized_root = target_name.as_str().replace('-', "_");
    let rustdoc_root_name = CrateName::new(normalized_root).map_err(|error| {
        resolution_error(
            format!("target `{}` cannot form a rustdoc root: {error}", target_name.as_str()),
            package_name,
            RustdocRootResolutionError::InvalidTargetName,
        )
    })?;
    Ok(RustdocTargetResolution {
        package_name: package_name.clone(),
        target_name,
        rustdoc_root_name,
        target_kind,
    })
}

fn target_names_of_kind(
    targets: &[serde_json::Value],
    expected_kind: &str,
) -> Result<Vec<CargoTargetName>, RustdocRootResolutionError> {
    targets
        .iter()
        .filter(|target| {
            target
                .get("kind")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(expected_kind)))
        })
        .map(|target| {
            CargoTargetName::try_new(
                target
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect()
}

fn select_bin_target(
    package: &serde_json::Value,
    targets: &[serde_json::Value],
    package_name: &CrateName,
) -> Result<(CargoTargetName, RustdocTargetKind), RustdocRootResolutionError> {
    let bins = target_names_of_kind(targets, "bin")?;
    match bins.as_slice() {
        [] => Err(resolution_error(
            format!("package `{package_name}` has no library or bin target"),
            package_name,
            RustdocRootResolutionError::TargetSelection,
        )),
        [bin] => Ok((bin.clone(), RustdocTargetKind::Binary)),
        _ => {
            let default_run = package
                .get("default_run")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    resolution_error(
                        format!(
                            "package `{package_name}` has multiple bin targets and no default_run"
                        ),
                        package_name,
                        RustdocRootResolutionError::TargetSelection,
                    )
                })?;
            let bin = bins.iter().find(|bin| bin.as_str() == default_run).ok_or_else(|| {
                resolution_error(
                    format!(
                        "package `{package_name}` default_run `{default_run}` does not match a bin target"
                    ),
                    package_name,
                    RustdocRootResolutionError::TargetSelection,
                )
            })?;
            Ok((bin.clone(), RustdocTargetKind::Binary))
        }
    }
}

fn resolution_error(
    detail: impl Into<String>,
    package_name: &CrateName,
    constructor: impl FnOnce(DiagnosticMessage) -> RustdocRootResolutionError,
) -> RustdocRootResolutionError {
    match DiagnosticMessage::try_new(detail.into()) {
        Ok(message) => constructor(message),
        Err(_) => RustdocRootResolutionError::PackageNotFound(package_name.clone()),
    }
}

fn invalid_target_name(
    value: String,
    detail: String,
) -> Result<CargoTargetName, RustdocRootResolutionError> {
    match DiagnosticMessage::try_new(detail) {
        Ok(message) => Err(RustdocRootResolutionError::InvalidTargetName(message)),
        Err(_) => Ok(CargoTargetName(value)),
    }
}

fn schema_export_error(error: RustdocRootResolutionError) -> SchemaExportError {
    match error {
        RustdocRootResolutionError::PackageNotFound(package_name) => {
            SchemaExportError::CrateNotFound(package_name.as_str().to_owned())
        }
        other => SchemaExportError::RustdocFailed(other.to_string()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn package_name() -> CrateName {
        CrateName::new("catalogue_package").unwrap()
    }

    #[test]
    fn test_build_rustdoc_args_includes_document_hidden_items_flag() {
        let args = build_rustdoc_args("catalogue_package", &["--lib"]);

        assert!(args.iter().any(|argument| argument == "--document-hidden-items"));
    }

    #[test]
    fn test_target_resolution_library_target_preferred_and_normalized() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "utility-bin"},
                {"kind": ["lib"], "name": "library-target"}
            ]
        });

        let resolution = target_resolution_from_package(&package, &package_name()).unwrap();

        assert_eq!(resolution.target_kind(), RustdocTargetKind::Library);
        assert_eq!(resolution.target_name().as_str(), "library-target");
        assert_eq!(resolution.rustdoc_root_name().as_str(), "library_target");
        assert_eq!(resolution.package_name().as_str(), "catalogue_package");
    }

    #[test]
    fn test_target_resolution_single_bin_target_is_selected_and_normalized() {
        let package = json!({"targets": [{"kind": ["bin"], "name": "service-bin"}]});

        let resolution = target_resolution_from_package(&package, &package_name()).unwrap();

        assert_eq!(resolution.target_kind(), RustdocTargetKind::Binary);
        assert_eq!(resolution.target_name().as_str(), "service-bin");
        assert_eq!(resolution.rustdoc_root_name().as_str(), "service_bin");
    }

    #[test]
    fn test_target_resolution_multiple_bins_require_matching_default_run() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "admin"},
                {"kind": ["bin"], "name": "service-bin"}
            ],
            "default_run": "service-bin"
        });

        let resolution = target_resolution_from_package(&package, &package_name()).unwrap();

        assert_eq!(resolution.target_name().as_str(), "service-bin");
    }

    #[test]
    fn test_target_resolution_multiple_bins_without_default_run_is_rejected() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "admin"},
                {"kind": ["bin"], "name": "service"}
            ]
        });

        let error = target_resolution_from_package(&package, &package_name()).unwrap_err();

        assert!(matches!(error, RustdocRootResolutionError::TargetSelection(_)));
    }

    #[test]
    fn test_target_resolution_unknown_default_run_is_rejected() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "admin"},
                {"kind": ["bin"], "name": "service"}
            ],
            "default_run": "other"
        });

        let error = target_resolution_from_package(&package, &package_name()).unwrap_err();

        assert!(matches!(error, RustdocRootResolutionError::TargetSelection(_)));
    }

    #[test]
    fn test_target_resolution_missing_target_name_is_rejected() {
        let package = json!({"targets": [{"kind": ["bin"]}]});

        let error = target_resolution_from_package(&package, &package_name()).unwrap_err();

        assert!(matches!(error, RustdocRootResolutionError::InvalidTargetName(_)));
    }
}
