//! Branch-blob implementation-input hashing for the strict merge gate.
//!
//! The evaluator hashes the checked-out workspace. The merge gate reads an
//! `origin/<branch>` tree instead, so this module acquires the same components
//! from git blobs and delegates the final digest construction to the evaluator
//! hashing authority.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use domain::tddd::type_signals_doc::ImplementationInputHash;
use domain::tddd::{CargoFeatureName, LayerId, TdddFeatureDeclaration};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::tddd_layers::{TdddLayerBinding, parse_tddd_layers};
use crate::git_cli::show::{BlobResult, fetch_blob_safe, git_ls_tree_recursive_regular_files};
use crate::tddd::type_signals_evaluator::{build_inputs, inputs};

pub(crate) const MAX_DECLARATION_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_FILES: usize = 10_000;
const MAX_SOURCE_DEPTH: usize = 32;
const MAX_SOURCE_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// Computes the authoritative implementation-input hash for one layer on a
/// branch ref.
///
/// The source paths, raw bytes, manifests, optional build script, lockfile,
/// toolchain identity, and feature-selection post-hash are the same inputs
/// used by hash_workspace_inputs. `Ok(None)` means the local toolchain
/// enumeration succeeded and found no installed nightly, so the caller may
/// skip only the implementation-input comparison. Any unavailable or
/// ambiguous input, including a failed probe or unreadable installed-nightly
/// identity, returns an error so the merge gate can block.
pub(crate) fn hash_branch_implementation_inputs(
    repo_root: &Path,
    branch: &str,
    track_id: &str,
    layer_id: &str,
) -> Result<Option<ImplementationInputHash>, String> {
    let architecture_rules = required_blob(repo_root, branch, "architecture-rules.json")?;
    let architecture_rules = String::from_utf8(architecture_rules)
        .map_err(|error| format!("architecture-rules.json is not valid UTF-8: {error}"))?;
    let bindings = parse_tddd_layers(&architecture_rules)
        .map_err(|error| format!("architecture-rules.json parse error: {error}"))?;
    let binding = bindings
        .iter()
        .find(|binding| binding.layer_id() == layer_id)
        .cloned()
        .ok_or_else(|| format!("TDDD layer '{layer_id}' is not enabled on origin/{branch}"))?;
    let target_crate = match binding.targets() {
        [target] => target.as_str(),
        [] => return Err(format!("TDDD layer '{layer_id}' has no rustdoc target")),
        _ => return Err(format!("TDDD layer '{layer_id}' has multiple rustdoc targets")),
    };
    let target_root = target_root(target_crate)?;
    let toolchain_identifier = match build_inputs::probe_nightly_toolchain(repo_root)
        .map_err(|error| error.to_string())?
    {
        build_inputs::NightlyToolchainProbe::Installed(identity) => identity,
        build_inputs::NightlyToolchainProbe::Absent => return Ok(None),
    };
    let features = branch_features(repo_root, branch, track_id, &bindings, &binding)?;

    let source_dir = format!("{target_root}/src");
    let source_paths = git_ls_tree_recursive_regular_files(repo_root, branch, &source_dir)?;
    if source_paths.is_empty() {
        return Err(format!("implementation source directory '{source_dir}' is unavailable"));
    }
    if source_paths.len() > MAX_SOURCE_FILES {
        return Err(format!(
            "implementation source traversal exceeds maximum of {MAX_SOURCE_FILES} files"
        ));
    }

    let mut remaining_budget = MAX_TOTAL_SOURCE_BYTES;
    let mut source_files = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let relative = path
            .strip_prefix(&format!("{source_dir}/"))
            .ok_or_else(|| format!("git returned a source path outside '{source_dir}': {path}"))?;
        if relative.split('/').count() > MAX_SOURCE_DEPTH {
            return Err(format!(
                "implementation source traversal exceeds maximum depth of {MAX_SOURCE_DEPTH} at '{path}'"
            ));
        }
        let bytes = required_blob_limited(
            repo_root,
            branch,
            &path,
            MAX_SOURCE_FILE_BYTES,
            &mut remaining_budget,
        )?;
        source_files.push((path.into_bytes(), bytes));
    }

    let crate_manifest_path = format!("{target_root}/Cargo.toml");
    let crate_manifest = required_blob_limited(
        repo_root,
        branch,
        &crate_manifest_path,
        MAX_SOURCE_FILE_BYTES,
        &mut remaining_budget,
    )?;
    validate_features_against_manifest(&crate_manifest, &features)?;

    let build_script_path = format!("{target_root}/build.rs");
    let build_script = optional_blob_limited(
        repo_root,
        branch,
        &build_script_path,
        MAX_SOURCE_FILE_BYTES,
        &mut remaining_budget,
    )?;
    let workspace_manifest = required_blob_limited(
        repo_root,
        branch,
        "Cargo.toml",
        MAX_SOURCE_FILE_BYTES,
        &mut remaining_budget,
    )?;
    let lockfile = required_blob_limited(
        repo_root,
        branch,
        "Cargo.lock",
        MAX_SOURCE_FILE_BYTES,
        &mut remaining_budget,
    )?;
    let implementation_hash = build_inputs::hash_implementation_input_components(
        &source_files,
        &crate_manifest,
        build_script.as_deref(),
        &workspace_manifest,
        &lockfile,
        &toolchain_identifier,
    )
    .map_err(|error| error.to_string())?;
    inputs::implementation_hash_with_feature_selection(implementation_hash, &features)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn target_root(target_crate: &str) -> Result<&'static str, String> {
    match target_crate {
        "domain" => Ok("libs/domain"),
        "usecase" => Ok("libs/usecase"),
        "infrastructure" => Ok("libs/infrastructure"),
        "cli" => Ok("apps/cli"),
        "cli_driver" => Ok("apps/cli-driver"),
        "cli_composition" => Ok("apps/cli-composition"),
        other => Err(format!("unsupported TDDD target crate '{other}'")),
    }
}

fn branch_features(
    repo_root: &Path,
    branch: &str,
    track_id: &str,
    bindings: &[TdddLayerBinding],
    binding: &TdddLayerBinding,
) -> Result<Vec<CargoFeatureName>, String> {
    // Authority-availability boundary: the feature-selection baseline snapshot
    // (`tddd-features-baseline.json`) is gitignored local operational state and
    // does not exist on a branch. The committed feature DECLARATION is the
    // authority a merge gate can read, so the branch-side hash derives its
    // feature selection from it; the declaration↔baseline equality is enforced
    // by the local pre-commit gates where the snapshot exists.
    let declaration_path = format!("track/items/{track_id}/tddd-features.json");
    let declaration_bytes = required_blob_limited_without_budget(
        repo_root,
        branch,
        &declaration_path,
        MAX_DECLARATION_BYTES,
    )?;

    let required_layers = bindings
        .iter()
        .map(|item| {
            LayerId::try_new(item.layer_id().to_owned())
                .map_err(|error| format!("invalid TDDD layer '{}': {error}", item.layer_id()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let declaration =
        parse_feature_declaration(&declaration_bytes, &declaration_path, &required_layers)?;
    let layer = LayerId::try_new(binding.layer_id().to_owned())
        .map_err(|error| format!("invalid TDDD layer '{}': {error}", binding.layer_id()))?;
    declaration
        .features_for(&layer)
        .map(|features| features.to_vec())
        .map_err(|error| format!("{declaration_path}: {error}"))
}

/// Decodes the committed feature declaration used by both branch and local
/// implementation-input hashing.
pub(crate) fn parse_feature_declaration(
    declaration_bytes: &[u8],
    declaration_path: &str,
    required_layers: &[LayerId],
) -> Result<TdddFeatureDeclaration, String> {
    let dto: FeatureDeclarationDto = serde_json::from_slice(declaration_bytes)
        .map_err(|error| format!("{declaration_path} decode error: {error}"))?;
    if dto.schema_version != 1 {
        return Err(format!(
            "{declaration_path} has unsupported schema version {}",
            dto.schema_version
        ));
    }
    let mut layers = BTreeMap::new();
    for (layer, features) in dto.layers {
        let layer_id = LayerId::try_new(layer.clone())
            .map_err(|error| format!("{declaration_path}: invalid layer '{layer}': {error}"))?;
        let features = features
            .into_iter()
            .map(|feature| {
                CargoFeatureName::try_new(feature.clone()).map_err(|error| {
                    format!("{declaration_path}: invalid feature '{feature}': {error}")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if layers.insert(layer_id, features).is_some() {
            return Err(format!("{declaration_path}: duplicate layer declaration"));
        }
    }
    let declaration = TdddFeatureDeclaration::try_new(layers, required_layers)
        .map_err(|error| format!("{declaration_path}: {error}"))?;
    Ok(declaration)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureDeclarationDto {
    schema_version: u32,
    #[serde(deserialize_with = "deserialize_layer_map")]
    layers: BTreeMap<String, Vec<String>>,
}

fn deserialize_layer_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrictLayerMapVisitor;

    impl<'de> Visitor<'de> for StrictLayerMapVisitor {
        type Value = BTreeMap<String, Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map from layer name to Cargo feature list")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut layers = BTreeMap::new();
            while let Some(layer) = map.next_key::<String>()? {
                let features = map.next_value()?;
                if layers.insert(layer.clone(), features).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate key '{layer}' in feature declaration layers"
                    )));
                }
            }
            Ok(layers)
        }
    }

    deserializer.deserialize_map(StrictLayerMapVisitor)
}

fn validate_features_against_manifest(
    manifest_bytes: &[u8],
    features: &[CargoFeatureName],
) -> Result<(), String> {
    let text = std::str::from_utf8(manifest_bytes)
        .map_err(|error| format!("target Cargo.toml is not valid UTF-8: {error}"))?;
    let manifest: toml::Value =
        toml::from_str(text).map_err(|error| format!("target Cargo.toml decode error: {error}"))?;
    let mut available = BTreeSet::new();
    let mut suppressed_implicit_features = BTreeSet::new();
    if let Some(feature_table) = manifest.get("features") {
        let feature_table = feature_table
            .as_table()
            .ok_or_else(|| "target Cargo.toml features value must be a table".to_owned())?;
        for (name, definition) in feature_table {
            let entries = definition.as_array().ok_or_else(|| {
                format!("target Cargo.toml feature '{name}' must be an array of strings")
            })?;
            for entry in entries {
                let entry = entry.as_str().ok_or_else(|| {
                    format!("target Cargo.toml feature '{name}' must be an array of strings")
                })?;
                if let Some(dependency) = entry.strip_prefix("dep:") {
                    suppressed_implicit_features.insert(dependency.to_owned());
                }
            }
            available.insert(name.clone());
        }
    }
    let optional_dependencies = collect_optional_dependencies(&manifest);
    available.extend(optional_dependencies.difference(&suppressed_implicit_features).cloned());
    for feature in features {
        if !available.contains(feature.as_str()) {
            return Err(format!(
                "target Cargo.toml does not define declared feature '{}'",
                feature.as_str()
            ));
        }
    }
    Ok(())
}

fn collect_optional_dependencies(manifest: &toml::Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        collect_optional_dependency_names(manifest.get(table_name), &mut names);
    }
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                collect_optional_dependency_names(target.get(table_name), &mut names);
            }
        }
    }
    names
}

fn collect_optional_dependency_names(value: Option<&toml::Value>, names: &mut BTreeSet<String>) {
    let Some(table) = value.and_then(toml::Value::as_table) else {
        return;
    };
    for (name, dependency) in table {
        if dependency
            .as_table()
            .and_then(|table| table.get("optional"))
            .and_then(toml::Value::as_bool)
            == Some(true)
        {
            names.insert(name.clone());
        }
    }
}

fn required_blob(repo_root: &Path, branch: &str, path: &str) -> Result<Vec<u8>, String> {
    match fetch_blob_safe(repo_root, branch, path) {
        BlobResult::Found(bytes) => Ok(bytes),
        BlobResult::NotFound => {
            Err(format!("implementation input '{path}' not found on origin/{branch}"))
        }
        BlobResult::CommandFailed(message) => Err(message),
    }
}

fn required_blob_limited(
    repo_root: &Path,
    branch: &str,
    path: &str,
    maximum_bytes: usize,
    remaining_budget: &mut usize,
) -> Result<Vec<u8>, String> {
    let bytes = required_blob(repo_root, branch, path)?;
    if bytes.len() > maximum_bytes {
        return Err(format!(
            "implementation input '{path}' exceeds the {maximum_bytes}-byte per-file limit"
        ));
    }
    if bytes.len() > *remaining_budget {
        return Err(format!(
            "implementation inputs exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit"
        ));
    }
    *remaining_budget -= bytes.len();
    Ok(bytes)
}

fn required_blob_limited_without_budget(
    repo_root: &Path,
    branch: &str,
    path: &str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, String> {
    let bytes = required_blob(repo_root, branch, path)?;
    if bytes.len() > maximum_bytes {
        return Err(format!("branch artifact '{path}' exceeds the {maximum_bytes}-byte limit"));
    }
    Ok(bytes)
}

fn optional_blob_limited(
    repo_root: &Path,
    branch: &str,
    path: &str,
    maximum_bytes: usize,
    remaining_budget: &mut usize,
) -> Result<Option<Vec<u8>>, String> {
    let bytes = match fetch_blob_safe(repo_root, branch, path) {
        BlobResult::Found(bytes) => bytes,
        BlobResult::NotFound => return Ok(None),
        BlobResult::CommandFailed(message) => return Err(message),
    };
    if bytes.len() > maximum_bytes {
        return Err(format!(
            "implementation input '{path}' exceeds the {maximum_bytes}-byte per-file limit"
        ));
    }
    if bytes.len() > *remaining_budget {
        return Err(format!(
            "implementation inputs exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit"
        ));
    }
    *remaining_budget -= bytes.len();
    Ok(Some(bytes))
}
