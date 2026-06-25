//! Rustdoc JSON export helpers for `[[bin]]`-only crates.
//!
//! `run_rustdoc` first tries `--lib`; for crates with no lib target it falls
//! back to `--bin <name>`, resolving the binary name via `cargo metadata`.
//! Multiple `[[bin]]` targets are selected through Cargo's package-level
//! `default_run`; without that canonical selector, export fails closed instead
//! of relying on metadata ordering.

use std::path::{Path, PathBuf};
use std::process::Command;

use domain::schema::SchemaExportError;

use super::path_resolution::resolve_target_dir;

pub(super) fn run_rustdoc(
    workspace_root: &Path,
    crate_name: &str,
) -> Result<PathBuf, SchemaExportError> {
    let args = |target: &[&str]| -> Vec<String> {
        let mut v = vec!["+nightly".into(), "rustdoc".into(), "-p".into(), crate_name.into()];
        v.extend(target.iter().map(|s| (*s).into()));
        v.extend(["--", "-Z", "unstable-options", "--output-format", "json"].map(Into::into));
        v
    };

    let lib_out = Command::new("cargo")
        .args(args(&["--lib"]))
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(e.to_string()))?;

    if lib_out.status.success() {
        let dir = resolve_target_dir(workspace_root)?;
        let name = rustdoc_artifact_name(crate_name);
        let path = dir.join("doc").join(format!("{name}.json"));
        super::ensure_rustdoc_json_path_safe(&dir, &path, "rustdoc --lib")?;
        return if path.is_file() {
            Ok(path)
        } else {
            Err(SchemaExportError::RustdocFailed(format!(
                "expected rustdoc JSON at {} but file not found",
                path.display()
            )))
        };
    }

    let lib_err = String::from_utf8_lossy(&lib_out.stderr);
    if lib_err.contains("did not match any packages")
        || (lib_err.contains("package(s) `") && lib_err.contains("not found in workspace"))
    {
        return Err(SchemaExportError::CrateNotFound(crate_name.to_owned()));
    }
    if !lib_err.contains("no library targets found") {
        return Err(SchemaExportError::RustdocFailed(lib_err.into_owned()));
    }

    // [[bin]]-only crate: resolve the binary target name and retry with --bin.
    let bin_name = resolve_bin_target_name(workspace_root, crate_name)?;
    let bin_out = Command::new("cargo")
        .args(args(&["--bin", &bin_name]))
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(e.to_string()))?;

    if !bin_out.status.success() {
        let stderr = String::from_utf8_lossy(&bin_out.stderr);
        return Err(SchemaExportError::RustdocFailed(format!(
            "rustdoc --bin '{bin_name}': {stderr}"
        )));
    }

    let dir = resolve_target_dir(workspace_root)?;
    let artifact_name = rustdoc_artifact_name(&bin_name);
    let path = dir.join("doc").join(format!("{artifact_name}.json"));
    super::ensure_rustdoc_json_path_safe(&dir, &path, "rustdoc --bin")?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(SchemaExportError::RustdocFailed(format!(
            "expected rustdoc JSON for bin '{bin_name}' at {} but file not found",
            path.display()
        )))
    }
}

fn rustdoc_artifact_name(target_name: &str) -> String {
    target_name.replace('-', "_")
}

fn resolve_bin_target_name(
    workspace_root: &Path,
    crate_name: &str,
) -> Result<String, SchemaExportError> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(format!("cargo metadata: {e}")))?;

    if !out.status.success() {
        return Err(SchemaExportError::RustdocFailed(format!(
            "cargo metadata non-zero for '{crate_name}': {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    let meta: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| SchemaExportError::RustdocFailed(format!("cargo metadata JSON parse: {e}")))?;

    let packages = meta.get("packages").and_then(|v| v.as_array()).ok_or_else(|| {
        SchemaExportError::RustdocFailed(format!("cargo metadata: no packages ('{crate_name}')"))
    })?;

    for pkg in packages {
        if pkg.get("name").and_then(|v| v.as_str()).unwrap_or("") != crate_name {
            continue;
        }
        return select_bin_target_name(pkg, crate_name);
    }

    Err(SchemaExportError::CrateNotFound(crate_name.to_owned()))
}

fn select_bin_target_name(
    package: &serde_json::Value,
    crate_name: &str,
) -> Result<String, SchemaExportError> {
    let bin_names = bin_target_names(package, crate_name)?;
    match bin_names.as_slice() {
        [] => Err(SchemaExportError::RustdocFailed(format!(
            "no bin targets for '{crate_name}' in cargo metadata"
        ))),
        [single] => Ok(single.clone()),
        _ => select_default_run_bin(package, crate_name, &bin_names),
    }
}

fn bin_target_names(
    package: &serde_json::Value,
    crate_name: &str,
) -> Result<Vec<String>, SchemaExportError> {
    let Some(targets) = package.get("targets").and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    for target in targets {
        if !is_bin_target(target) {
            continue;
        }
        let name = target.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            SchemaExportError::RustdocFailed(format!(
                "bin target for '{crate_name}': no name in metadata"
            ))
        })?;
        names.push(name.to_owned());
    }
    Ok(names)
}

fn is_bin_target(target: &serde_json::Value) -> bool {
    target
        .get("kind")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|k| k.as_str() == Some("bin")))
}

fn select_default_run_bin(
    package: &serde_json::Value,
    crate_name: &str,
    bin_names: &[String],
) -> Result<String, SchemaExportError> {
    let Some(default_run) = package.get("default_run").and_then(|v| v.as_str()) else {
        return Err(SchemaExportError::RustdocFailed(format!(
            "package '{crate_name}' has multiple bin targets ({}) and no default_run",
            bin_names.join(", ")
        )));
    };

    if bin_names.iter().any(|name| name == default_run) {
        Ok(default_run.to_owned())
    } else {
        Err(SchemaExportError::RustdocFailed(format!(
            "package '{crate_name}' default_run '{default_run}' does not match bin targets ({})",
            bin_names.join(", ")
        )))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_select_bin_target_name_single_bin_returns_bin_name() {
        let package = json!({
            "targets": [
                {"kind": ["lib"], "name": "cli"},
                {"kind": ["bin"], "name": "sotp"}
            ],
            "default_run": null
        });

        let selected = select_bin_target_name(&package, "cli").unwrap();

        assert_eq!(selected, "sotp");
    }

    #[test]
    fn test_rustdoc_artifact_name_hyphenated_target_uses_underscore() {
        assert_eq!(rustdoc_artifact_name("sotp-admin"), "sotp_admin");
    }

    #[test]
    fn test_select_bin_target_name_multiple_bins_with_default_run_returns_default_run() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "admin"},
                {"kind": ["bin"], "name": "sotp"}
            ],
            "default_run": "sotp"
        });

        let selected = select_bin_target_name(&package, "cli").unwrap();

        assert_eq!(selected, "sotp");
    }

    #[test]
    fn test_select_bin_target_name_multiple_bins_without_default_run_returns_error() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "admin"},
                {"kind": ["bin"], "name": "sotp"}
            ],
            "default_run": null
        });

        let err = select_bin_target_name(&package, "cli").unwrap_err();

        assert!(matches!(err, SchemaExportError::RustdocFailed(_)));
        assert!(err.to_string().contains("multiple bin targets"));
    }

    #[test]
    fn test_select_bin_target_name_multiple_bins_with_unknown_default_run_returns_error() {
        let package = json!({
            "targets": [
                {"kind": ["bin"], "name": "admin"},
                {"kind": ["bin"], "name": "sotp"}
            ],
            "default_run": "other"
        });

        let err = select_bin_target_name(&package, "cli").unwrap_err();

        assert!(matches!(err, SchemaExportError::RustdocFailed(_)));
        assert!(err.to_string().contains("does not match bin targets"));
    }
}
