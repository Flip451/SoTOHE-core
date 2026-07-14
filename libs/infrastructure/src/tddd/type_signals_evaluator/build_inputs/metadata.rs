//! Cargo metadata, toolchain, and configuration discovery for build-input hashing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{
    EvaluateSignalsError, MAX_BUILD_INPUT_COMMAND_DURATION, MAX_BUILD_INPUT_COMMAND_OUTPUT_BYTES,
    read_optional_regular_file, required_str,
};

pub(super) fn cargo_metadata(
    workspace_root: &Path,
) -> Result<serde_json::Value, EvaluateSignalsError> {
    let mut command = Command::new("cargo");
    command
        .args(["+nightly", "metadata", "--format-version", "1", "--locked"])
        .current_dir(workspace_root);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_BUILD_INPUT_COMMAND_OUTPUT_BYTES,
        MAX_BUILD_INPUT_COMMAND_DURATION,
        "cargo metadata for build inputs",
    )
    .map_err(|error| EvaluateSignalsError(format!("cannot resolve Cargo build inputs: {error}")))?;
    if !output.status.success() {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve Cargo build inputs: cargo metadata exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| EvaluateSignalsError(format!("cannot decode Cargo metadata: {error}")))
}

pub(super) fn nightly_toolchain_identity() -> Result<Vec<u8>, EvaluateSignalsError> {
    let mut command = Command::new("rustup");
    command.args(["run", "nightly", "rustc", "-Vv"]);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_BUILD_INPUT_COMMAND_OUTPUT_BYTES,
        MAX_BUILD_INPUT_COMMAND_DURATION,
        "nightly rustc identity",
    )
    .map_err(|error| {
        EvaluateSignalsError(format!("cannot resolve nightly rustc identity: {error}"))
    })?;
    if !output.status.success() {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve nightly rustc identity: rustup run nightly rustc -Vv exited with {}",
            output.status
        )));
    }
    Ok(output.stdout)
}

pub(super) fn resolved_package_closure(
    metadata: &serde_json::Value,
    target_crate: &str,
) -> Result<Vec<serde_json::Value>, EvaluateSignalsError> {
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| EvaluateSignalsError("Cargo metadata has no packages array".to_owned()))?;
    let package_by_id: BTreeMap<&str, &serde_json::Value> = packages
        .iter()
        .filter_map(|package| {
            package.get("id").and_then(serde_json::Value::as_str).map(|id| (id, package))
        })
        .collect();
    let targets: Vec<&str> = package_by_id
        .iter()
        .filter_map(|(id, package)| {
            (package.get("name").and_then(serde_json::Value::as_str) == Some(target_crate))
                .then_some(*id)
        })
        .collect();
    let [target_id] = targets.as_slice() else {
        return Err(EvaluateSignalsError(format!(
            "Cargo metadata must resolve exactly one package named '{target_crate}', found {}",
            targets.len()
        )));
    };
    let nodes = metadata
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            EvaluateSignalsError("Cargo metadata has no resolved dependency graph".to_owned())
        })?;
    let node_by_id: BTreeMap<&str, &serde_json::Value> = nodes
        .iter()
        .filter_map(|node| node.get("id").and_then(serde_json::Value::as_str).map(|id| (id, node)))
        .collect();
    let mut pending = vec![(*target_id).to_owned()];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let node = node_by_id.get(id.as_str()).ok_or_else(|| {
            EvaluateSignalsError(format!("Cargo metadata has no resolve node for package '{id}'"))
        })?;
        let dependencies =
            node.get("deps").and_then(serde_json::Value::as_array).ok_or_else(|| {
                EvaluateSignalsError(format!("Cargo metadata has no dependencies for '{id}'"))
            })?;
        for dependency in dependencies {
            if dependency_is_rustdoc_input(dependency)? {
                let package = required_str(dependency, "pkg")?;
                pending.push(package.to_owned());
            }
        }
    }
    visited
        .into_iter()
        .map(|id| {
            let mut package =
                package_by_id.get(id.as_str()).cloned().cloned().ok_or_else(|| {
                    EvaluateSignalsError(format!("Cargo metadata has no package '{id}'"))
                })?;
            let features = node_by_id
                .get(id.as_str())
                .and_then(|node| node.get("features"))
                .cloned()
                .ok_or_else(|| {
                    EvaluateSignalsError(format!(
                        "Cargo metadata has no active features for '{id}'"
                    ))
                })?;
            package
                .as_object_mut()
                .ok_or_else(|| {
                    EvaluateSignalsError(format!("Cargo metadata package '{id}' is not an object"))
                })?
                .insert("resolved_features".to_owned(), features);
            Ok(package)
        })
        .collect()
}

fn dependency_is_rustdoc_input(
    dependency: &serde_json::Value,
) -> Result<bool, EvaluateSignalsError> {
    let kinds =
        dependency.get("dep_kinds").and_then(serde_json::Value::as_array).ok_or_else(|| {
            EvaluateSignalsError("Cargo metadata dependency has no dep_kinds".to_owned())
        })?;
    Ok(kinds.iter().any(|kind| {
        kind.get("kind").and_then(serde_json::Value::as_str).is_none_or(|kind| kind != "dev")
    }))
}

pub(super) fn cargo_configs(
    workspace_root: &Path,
) -> Result<Vec<(PathBuf, Vec<u8>)>, EvaluateSignalsError> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        roots.push(PathBuf::from(home));
    } else if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".cargo"));
    }
    let mut ancestors: Vec<PathBuf> = workspace_root.ancestors().map(Path::to_path_buf).collect();
    ancestors.reverse();
    roots.extend(ancestors.into_iter().map(|path| path.join(".cargo")));
    let mut configs = Vec::new();
    for root in roots {
        let legacy = root.join("config");
        let toml = root.join("config.toml");
        let legacy_contents = read_optional_regular_file(&legacy)?;
        let toml_contents = read_optional_regular_file(&toml)?;
        if legacy_contents.is_some() && toml_contents.is_some() {
            return Err(EvaluateSignalsError(format!(
                "cannot normalize Cargo config: both '{}' and '{}' exist",
                legacy.display(),
                toml.display()
            )));
        }
        for (path, contents) in [(legacy, legacy_contents), (toml, toml_contents)] {
            if let Some(contents) = contents {
                configs.push((path, contents));
            }
        }
    }
    Ok(configs)
}
