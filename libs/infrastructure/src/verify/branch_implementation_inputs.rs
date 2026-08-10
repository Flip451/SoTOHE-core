//! Branch-blob implementation-input hashing for the strict merge gate.
//!
//! Branch and local hashing share the same closed architecture-layer graph and
//! framed digest contract. The branch side reads only committed blobs; it does
//! not interpret Cargo manifests or Rust source text.

use std::collections::BTreeMap;
use std::path::Path;

use domain::tddd::type_signals_doc::ImplementationInputHash;
use domain::tddd::{CargoFeatureName, LayerId, TdddFeatureDeclaration};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::tddd_layers::{TdddLayerBinding, parse_tddd_layers};
use crate::tddd::type_signals_evaluator::layer_graph::LayerGraph;
use crate::tddd::type_signals_evaluator::{build_inputs, inputs};
#[path = "branch_source_tree.rs"]
mod branch_source_tree;

pub(crate) const MAX_DECLARATION_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_SOURCE_FILES: usize = 10_000;
pub(crate) const MAX_SOURCE_ENTRIES: usize = 20_000;
pub(crate) const MAX_SOURCE_DEPTH: usize = 32;
pub(crate) const MAX_SOURCE_FILE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_TOTAL_SOURCE_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_TREE_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// Computes the authoritative branch implementation-input hash for one layer.
pub(crate) fn hash_branch_implementation_inputs(
    repo_root: &Path,
    branch: &str,
    track_id: &str,
    layer_id: &str,
) -> Result<Option<ImplementationInputHash>, String> {
    let architecture_rules = required_blob_limited_without_budget(
        repo_root,
        branch,
        "architecture-rules.json",
        build_inputs::MAX_ARCHITECTURE_RULES_BYTES,
    )?;
    let graph = LayerGraph::parse(&architecture_rules)?;
    let roots = graph.crate_roots_for(layer_id)?;
    let rules_text = String::from_utf8(architecture_rules)
        .map_err(|error| format!("architecture-rules.json is not valid UTF-8: {error}"))?;
    let bindings = parse_tddd_layers(&rules_text)
        .map_err(|error| format!("architecture-rules.json parse error: {error}"))?;
    let binding = bindings
        .iter()
        .find(|binding| binding.layer_id() == layer_id)
        .ok_or_else(|| format!("TDDD layer '{layer_id}' is not enabled on origin/{branch}"))?;
    if binding.targets().len() != 1 {
        return Err(format!("type-signal layer '{layer_id}' must have exactly one rustdoc target"));
    }
    let schema_export_target = binding
        .targets()
        .first()
        .map(String::as_str)
        .ok_or_else(|| format!("type-signal layer '{layer_id}' has no rustdoc target"))?;
    let toolchain_identifier = match build_inputs::probe_nightly_toolchain(repo_root)
        .map_err(|error| error.to_string())?
    {
        build_inputs::NightlyToolchainProbe::Installed(identity) => identity,
        build_inputs::NightlyToolchainProbe::Absent => return Ok(None),
    };
    let features = branch_features(repo_root, branch, track_id, &bindings, binding)?;

    let mut remaining_budget = MAX_TOTAL_SOURCE_BYTES;
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
    let tree_files = branch_source_tree::collect_branch_tree_file_digests(
        repo_root,
        branch,
        &roots,
        &mut remaining_budget,
    )?;
    let implementation_hash = build_inputs::hash_implementation_input_components(
        &tree_files,
        &workspace_manifest,
        &lockfile,
        schema_export_target,
        &toolchain_identifier,
    )
    .map_err(|error| error.to_string())?;
    inputs::implementation_hash_with_feature_selection(implementation_hash, &features)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn branch_features(
    repo_root: &Path,
    branch: &str,
    track_id: &str,
    bindings: &[TdddLayerBinding],
    binding: &TdddLayerBinding,
) -> Result<Vec<CargoFeatureName>, String> {
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

/// Decodes the committed feature declaration used by branch and local hashing.
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
    TdddFeatureDeclaration::try_new(layers, required_layers)
        .map_err(|error| format!("{declaration_path}: {error}"))
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

fn required_blob_limited(
    repo_root: &Path,
    branch: &str,
    path: &str,
    maximum_bytes: usize,
    remaining_budget: &mut usize,
) -> Result<Vec<u8>, String> {
    let bytes = super::merge_gate_adapter::fetch_branch_blob_limited(
        repo_root,
        branch,
        path,
        maximum_bytes,
    )?
    .ok_or_else(|| format!("implementation input '{path}' not found on origin/{branch}"))?;
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
    let bytes = super::merge_gate_adapter::fetch_branch_blob_limited(
        repo_root,
        branch,
        path,
        maximum_bytes,
    )?
    .ok_or_else(|| format!("branch artifact '{path}' not found on origin/{branch}"))?;
    Ok(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::hash_branch_implementation_inputs;
    use crate::tddd::type_signals_evaluator::inputs::hash_workspace_inputs;
    use std::path::Path;

    fn git(cwd: &Path, args: &[&str]) {
        crate::verify::test_support::git_with_identity(cwd, args);
    }

    fn setup_repo() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        std::fs::write(
            repo.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [
                {"crate":"domain","path":"libs/domain","may_depend_on":[]},
                {"crate":"usecase","path":"libs/usecase","may_depend_on":["domain"],
                 "tddd":{"enabled":true,"schema_export":{"targets":["usecase"]}}},
                {"crate":"outside","path":"libs/outside","may_depend_on":[]}
              ]
            }"#,
        )
        .unwrap();
        std::fs::write(repo.join("Cargo.toml"), "[workspace]\nresolver = \"2\"\n").unwrap();
        std::fs::write(repo.join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::write(repo.join(".test-nightly-toolchain-identity"), "nightly-fixture\n").unwrap();
        for crate_name in ["domain", "usecase", "outside"] {
            let root = repo.join("libs").join(crate_name);
            std::fs::create_dir_all(root.join("src")).unwrap();
            std::fs::write(
                root.join("Cargo.toml"),
                format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
            std::fs::write(root.join("src/lib.rs"), format!("pub struct {crate_name}Source;\n"))
                .unwrap();
        }
        std::fs::create_dir_all(repo.join("track/items/foo")).unwrap();
        std::fs::write(
            repo.join("track/items/foo/tddd-features.json"),
            r#"{"schema_version":1,"layers":{"usecase":[]}}"#,
        )
        .unwrap();
        git(repo, &["add", "."]);
        git(repo, &["commit", "--quiet", "-m", "initial"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);
        directory
    }

    #[test]
    fn test_branch_implementation_inputs_match_local_hash_for_identical_trees() {
        let directory = setup_repo();
        let local = hash_workspace_inputs(directory.path(), "usecase", &[]).unwrap();
        let branch = hash_branch_implementation_inputs(directory.path(), "main", "foo", "usecase")
            .unwrap()
            .unwrap();
        assert_eq!(branch, local);
    }

    #[test]
    fn test_branch_implementation_inputs_ignore_crate_outside_layer_closure() {
        let directory = setup_repo();
        let initial = hash_branch_implementation_inputs(directory.path(), "main", "foo", "usecase")
            .unwrap()
            .unwrap();
        std::fs::write(
            directory.path().join("libs/outside/src/lib.rs"),
            "pub struct OutsideChanged;\n",
        )
        .unwrap();
        let unchanged =
            hash_branch_implementation_inputs(directory.path(), "main", "foo", "usecase")
                .unwrap()
                .unwrap();
        assert_eq!(initial, unchanged);
    }

    #[test]
    fn test_branch_implementation_inputs_uses_committed_required_blob() {
        let directory = setup_repo();
        std::fs::remove_file(directory.path().join("Cargo.lock")).unwrap();
        assert!(
            hash_branch_implementation_inputs(directory.path(), "main", "foo", "usecase")
                .unwrap()
                .is_some(),
            "branch hashing must read the committed Cargo.lock blob"
        );
    }
}
