//! Bulk loader for per-layer type catalogues across `tddd.enabled` layers.
//!
//! This module underpins the `CatalogueLoader` port implementation added in
//! track `tddd-contract-map-phase1-2026-04-17` (ADR 2026-04-17-1528). It
//! enumerates `tddd.enabled` layers from `architecture-rules.json`, decodes
//! each layer's catalogue file, and returns the decoded documents together
//! with a `may_depend_on`-based topological ordering (dependency-less layers
//! first). The ordering is required by downstream rendering so Contract Map
//! subgraphs appear left-to-right in dependency order without hard-coding
//! layer names (ADR §4.5 layer-agnostic invariant).
//!
//! All reads are guarded by `reject_symlinks_below` (see
//! `knowledge/conventions/security.md`): a missing catalogue is reported as
//! `CatalogueNotFound` rather than being silently skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use domain::ValidationError;
use domain::tddd::LayerId;
use domain::tddd::catalogue::TypeCatalogueDocument;

use crate::tddd::catalogue_codec::{self, TypeCatalogueCodecError};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::{self, LoadTdddLayersError, TdddLayerBinding};

/// Errors surfaced by [`load_all_catalogues`].
#[derive(Debug, thiserror::Error)]
pub enum LoadAllCataloguesError {
    /// `load_tddd_layers_from_path` failed (architecture-rules.json read /
    /// parse / symlink rejection for the rules file itself).
    #[error("failed to load tddd layer bindings: {0}")]
    LayerBindings(#[from] LoadTdddLayersError),

    /// Re-parse of `architecture-rules.json` for `may_depend_on` failed.
    #[error("failed to parse architecture-rules.json at {path}: {reason}")]
    ArchRulesParse { path: PathBuf, reason: String },

    /// I/O error while reading a catalogue file.
    #[error("I/O error for catalogue at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Catalogue file is missing on disk (fail-closed — we do not skip).
    #[error("catalogue file '{}' does not exist for layer '{layer_id}'", .path.display())]
    CatalogueNotFound { layer_id: String, path: PathBuf },

    /// Catalogue file failed to decode.
    #[error("failed to decode catalogue for layer '{layer_id}' at {}: {source}", .path.display())]
    Decode {
        layer_id: String,
        path: PathBuf,
        #[source]
        source: TypeCatalogueCodecError,
    },

    /// `may_depend_on` forms a cycle among `tddd.enabled` layers.
    #[error("topological sort failed: cycle detected among layers {cycle:?}")]
    TopologicalSortFailed { cycle: Vec<String> },

    /// A layer crate name violated [`LayerId`] validation rules.
    #[error("invalid layer id '{value}': {source}")]
    InvalidLayerId {
        value: String,
        #[source]
        source: ValidationError,
    },
}

/// Load every `tddd.enabled` layer's catalogue from `track_dir` and return
/// them in topological `may_depend_on` order.
///
/// The first element of the returned tuple is a `Vec<LayerId>` sorted so
/// that layers with zero dependencies (inside the enabled set) come first;
/// the second element maps each layer to its decoded
/// [`TypeCatalogueDocument`].
///
/// # Arguments
///
/// * `track_dir` — directory that holds each layer's catalogue file, e.g.
///   `track/items/<id>/`.
/// * `rules_path` — path to `architecture-rules.json` (typically at the
///   workspace root). Both `layers[].tddd.enabled` and `layers[].may_depend_on`
///   are read from this file.
/// * `trusted_root` — directory below which symlinks are rejected
///   (fail-closed). Should match the workspace root or the track root
///   depending on the caller's trust boundary.
///
/// # Errors
///
/// Returns [`LoadAllCataloguesError`] if:
/// * `architecture-rules.json` cannot be read, parsed, or is reached through
///   a symlink below `trusted_root`.
/// * Any enabled layer's catalogue file does not exist or is a symlink.
/// * Any catalogue file fails to decode via [`catalogue_codec::decode`].
/// * `may_depend_on` forms a cycle among the `tddd.enabled` layers (deps
///   pointing to disabled or absent layers are silently ignored so that
///   `tddd.enabled = false` layers do not force ordering decisions).
/// * A layer crate name fails [`LayerId`] validation.
pub fn load_all_catalogues(
    track_dir: &Path,
    rules_path: &Path,
    trusted_root: &Path,
) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), LoadAllCataloguesError> {
    let bindings = tddd_layers::load_tddd_layers_from_path(rules_path, trusted_root)?;
    let deps = parse_may_depend_on(rules_path, trusted_root)?;

    let enabled: Vec<&str> = bindings.iter().map(TdddLayerBinding::layer_id).collect();

    // Validate that every `may_depend_on` entry for an enabled layer names a
    // crate that appears in `architecture-rules.json`. A dep that is present
    // but `tddd.enabled = false` is acceptable (ordering will ignore it); a
    // dep that is entirely absent is a typo and should fail closed.
    let all_known: BTreeSet<&str> = deps.keys().map(String::as_str).collect();
    for &layer_id in &enabled {
        let layer_deps = deps.get(layer_id).map_or(&[] as &[String], Vec::as_slice);
        for dep in layer_deps {
            if !all_known.contains(dep.as_str()) {
                return Err(LoadAllCataloguesError::ArchRulesParse {
                    path: rules_path.to_path_buf(),
                    reason: format!(
                        "layer '{layer_id}' lists unknown dependency '{dep}' \
                         in may_depend_on (not found in architecture-rules.json)"
                    ),
                });
            }
        }
    }

    let ordered_names = topological_sort(&enabled, &deps)?;

    let binding_map: BTreeMap<&str, &TdddLayerBinding> =
        bindings.iter().map(|b| (b.layer_id(), b)).collect();

    let mut ordered_ids: Vec<LayerId> = Vec::with_capacity(ordered_names.len());
    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();

    for name in &ordered_names {
        let Some(binding) = binding_map.get(name.as_str()).copied() else {
            // Unreachable in normal execution: `ordered_names` is the
            // topological sort of `enabled`, which is the set of binding
            // layer ids. Guard with a Result rather than panic to comply
            // with the "no panic in library code" rule.
            return Err(LoadAllCataloguesError::ArchRulesParse {
                path: rules_path.to_path_buf(),
                reason: format!(
                    "internal invariant violation: ordered layer '{name}' not found in bindings"
                ),
            });
        };

        let layer_id = LayerId::try_new(name.clone()).map_err(|source| {
            LoadAllCataloguesError::InvalidLayerId { value: name.clone(), source }
        })?;
        let catalogue_path = track_dir.join(binding.catalogue_file());
        match reject_symlinks_below(&catalogue_path, trusted_root) {
            Ok(true) => {
                let json = std::fs::read_to_string(&catalogue_path).map_err(|source| {
                    LoadAllCataloguesError::Io { path: catalogue_path.clone(), source }
                })?;
                let doc = catalogue_codec::decode(&json).map_err(|source| {
                    LoadAllCataloguesError::Decode {
                        layer_id: layer_id.as_ref().to_owned(),
                        path: catalogue_path.clone(),
                        source,
                    }
                })?;
                catalogues.insert(layer_id.clone(), doc);
                ordered_ids.push(layer_id);
            }
            Ok(false) => {
                return Err(LoadAllCataloguesError::CatalogueNotFound {
                    layer_id: layer_id.as_ref().to_owned(),
                    path: catalogue_path,
                });
            }
            Err(source) => {
                return Err(LoadAllCataloguesError::Io { path: catalogue_path, source });
            }
        }
    }

    Ok((ordered_ids, catalogues))
}

/// Parse `layers[].may_depend_on` from `architecture-rules.json` for every
/// layer (including `tddd.enabled = false` ones — the caller filters
/// afterwards).
///
/// `trusted_root` is forwarded to [`reject_symlinks_below`] so every read of
/// `rules_path` is guarded, consistent with the fail-closed symlink policy.
fn parse_may_depend_on(
    rules_path: &Path,
    trusted_root: &Path,
) -> Result<BTreeMap<String, Vec<String>>, LoadAllCataloguesError> {
    match reject_symlinks_below(rules_path, trusted_root) {
        Ok(true) => {} // safe to read
        Ok(false) => {
            // File is absent — consistent with the legacy fallback in
            // `load_tddd_layers_from_path` (which synthesises a single
            // domain binding for pre-multilayer tracks). Return empty deps so
            // the topological sort treats the lone synthetic layer as having
            // no dependencies.
            return Ok(BTreeMap::new());
        }
        Err(source) => {
            return Err(LoadAllCataloguesError::Io { path: rules_path.to_path_buf(), source });
        }
    }
    let content = std::fs::read_to_string(rules_path)
        .map_err(|source| LoadAllCataloguesError::Io { path: rules_path.to_path_buf(), source })?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| LoadAllCataloguesError::ArchRulesParse {
            path: rules_path.to_path_buf(),
            reason: format!("JSON parse error: {e}"),
        })?;
    let layers = value.get("layers").and_then(|v| v.as_array()).ok_or_else(|| {
        LoadAllCataloguesError::ArchRulesParse {
            path: rules_path.to_path_buf(),
            reason: "missing 'layers' array".to_owned(),
        }
    })?;

    let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for layer in layers {
        let crate_name = layer.get("crate").and_then(|v| v.as_str()).ok_or_else(|| {
            LoadAllCataloguesError::ArchRulesParse {
                path: rules_path.to_path_buf(),
                reason: "layer entry missing 'crate' string".to_owned(),
            }
        })?;
        // Reject empty crate names — consistent with `is_safe_path_component`
        // enforced by `parse_tddd_layers`.
        if crate_name.is_empty() {
            return Err(LoadAllCataloguesError::ArchRulesParse {
                path: rules_path.to_path_buf(),
                reason: "layer entry has empty 'crate' string".to_owned(),
            });
        }
        // Reject duplicate crate names — consistent with the strict validation
        // enforced by `parse_tddd_layers` (which also rejects duplicate layer ids).
        if result.contains_key(crate_name) {
            return Err(LoadAllCataloguesError::ArchRulesParse {
                path: rules_path.to_path_buf(),
                reason: format!("duplicate 'crate' entry '{crate_name}' in layers array"),
            });
        }
        let deps = match layer.get("may_depend_on") {
            Some(arr) => {
                let arr = arr.as_array().ok_or_else(|| LoadAllCataloguesError::ArchRulesParse {
                    path: rules_path.to_path_buf(),
                    reason: format!("'may_depend_on' for '{crate_name}' is not an array"),
                })?;
                let mut parsed = Vec::with_capacity(arr.len());
                for entry in arr {
                    let dep =
                        entry.as_str().ok_or_else(|| LoadAllCataloguesError::ArchRulesParse {
                            path: rules_path.to_path_buf(),
                            reason: format!(
                                "'may_depend_on' entry for '{crate_name}' is not a string"
                            ),
                        })?;
                    if dep.is_empty() {
                        return Err(LoadAllCataloguesError::ArchRulesParse {
                            path: rules_path.to_path_buf(),
                            reason: format!(
                                "'may_depend_on' entry for '{crate_name}' is an empty string"
                            ),
                        });
                    }
                    parsed.push(dep.to_owned());
                }
                parsed
            }
            None => Vec::new(),
        };
        result.insert(crate_name.to_owned(), deps);
    }
    Ok(result)
}

/// Topologically sort `enabled_ids` by `deps` (Kahn's algorithm).
///
/// Only dependencies *within* `enabled_ids` are considered — a dependency
/// that points outside the enabled set is silently ignored so that
/// `tddd.enabled = false` layers do not force cross-set ordering decisions.
/// Unknown dependencies (not in `all_known_crates`) must be rejected by the
/// caller before invoking this function.
fn topological_sort(
    enabled_ids: &[&str],
    deps: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, LoadAllCataloguesError> {
    let enabled_set: BTreeSet<&str> = enabled_ids.iter().copied().collect();

    let mut in_degree: BTreeMap<&str, usize> = enabled_ids.iter().map(|&id| (id, 0)).collect();
    let mut adj: BTreeMap<&str, Vec<&str>> =
        enabled_ids.iter().map(|&id| (id, Vec::new())).collect();

    for &id in enabled_ids {
        let id_deps = deps.get(id).map_or(&[] as &[String], Vec::as_slice);
        // Deduplicate deps for this layer so that a repeated entry in
        // `may_depend_on` does not inflate `in_degree` and cause a false
        // cycle detection.
        let mut seen_deps: BTreeSet<&str> = BTreeSet::new();
        for dep in id_deps {
            let dep_str = dep.as_str();
            if !enabled_set.contains(dep_str) {
                // Known crate but not tddd.enabled — silently ignore for ordering.
                continue;
            }
            if !seen_deps.insert(dep_str) {
                // Already counted this dep for `id`; skip to avoid double-counting.
                continue;
            }
            if let Some(list) = adj.get_mut(dep_str) {
                list.push(id);
            }
            if let Some(count) = in_degree.get_mut(id) {
                *count += 1;
            }
        }
    }

    // Preserve the input ordering for all ties by scanning `enabled_ids` at
    // each step and picking the first unprocessed node with in_degree == 0.
    // A simple FIFO queue is insufficient here: when two nodes become ready
    // at different processing steps, FIFO emits them in processing order
    // rather than in their original input order. Scanning from the front of
    // `enabled_ids` each time guarantees stable input-order tie-breaking
    // across all scheduling points.
    let mut processed: BTreeSet<&str> = BTreeSet::new();
    let mut result: Vec<String> = Vec::with_capacity(enabled_ids.len());

    loop {
        // Find the first unprocessed node in input order that is ready.
        let next = enabled_ids
            .iter()
            .copied()
            .find(|id| !processed.contains(id) && in_degree.get(id).copied() == Some(0));
        let Some(id) = next else {
            break;
        };
        processed.insert(id);
        result.push(id.to_owned());
        if let Some(neighbors) = adj.get(id) {
            for &n in neighbors {
                if let Some(count) = in_degree.get_mut(n) {
                    *count -= 1;
                }
            }
        }
    }

    if result.len() != enabled_ids.len() {
        let cycle: Vec<String> = in_degree
            .iter()
            .filter_map(|(id, &d)| if d > 0 { Some((*id).to_owned()) } else { None })
            .collect();
        return Err(LoadAllCataloguesError::TopologicalSortFailed { cycle });
    }
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Minimal valid `architecture-rules.json` with 3 tddd.enabled layers
    /// declared in reverse topological order (infrastructure first, domain
    /// last) to prove that the loader re-sorts by `may_depend_on`.
    const RULES_REVERSED_ORDER: &str = r#"{
      "version": 2,
      "layers": [
        {
          "crate": "infrastructure",
          "path": "libs/infrastructure",
          "may_depend_on": ["domain", "usecase"],
          "deny_reason": "no reverse dep",
          "tddd": {
            "enabled": true,
            "catalogue_file": "infrastructure-types.json",
            "schema_export": {"method": "rustdoc", "targets": ["infrastructure"]}
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
        },
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
        }
      ]
    }"#;

    const EMPTY_CATALOGUE_JSON: &str = r#"{
      "schema_version": 2,
      "type_definitions": []
    }"#;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_load_all_catalogues_happy_path_sorts_topologically() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rules = root.join("architecture-rules.json");
        write(&rules, RULES_REVERSED_ORDER);

        let track_dir = root.join("track-item");
        write(&track_dir.join("domain-types.json"), EMPTY_CATALOGUE_JSON);
        write(&track_dir.join("usecase-types.json"), EMPTY_CATALOGUE_JSON);
        write(&track_dir.join("infrastructure-types.json"), EMPTY_CATALOGUE_JSON);

        let (order, catalogues) = load_all_catalogues(&track_dir, &rules, root).unwrap();

        let order_names: Vec<&str> = order.iter().map(LayerId::as_ref).collect();
        assert_eq!(
            order_names,
            ["domain", "usecase", "infrastructure"],
            "topological order must place dependency-less layers first"
        );
        assert_eq!(catalogues.len(), 3);
        for layer in &order {
            let doc = catalogues.get(layer).unwrap();
            assert_eq!(doc.entries().len(), 0);
        }
    }

    #[test]
    fn test_load_all_catalogues_missing_catalogue_returns_error_not_skip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rules = root.join("architecture-rules.json");
        write(&rules, RULES_REVERSED_ORDER);

        let track_dir = root.join("track-item");
        // Write only 2 of 3 catalogues — `infrastructure-types.json` missing.
        write(&track_dir.join("domain-types.json"), EMPTY_CATALOGUE_JSON);
        write(&track_dir.join("usecase-types.json"), EMPTY_CATALOGUE_JSON);

        let err = load_all_catalogues(&track_dir, &rules, root).unwrap_err();
        match err {
            LoadAllCataloguesError::CatalogueNotFound { layer_id, path } => {
                assert_eq!(layer_id, "infrastructure");
                assert!(path.ends_with("infrastructure-types.json"));
            }
            other => panic!("expected CatalogueNotFound, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_load_all_catalogues_rejects_symlink_catalogue() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rules = root.join("architecture-rules.json");
        write(&rules, RULES_REVERSED_ORDER);

        let track_dir = root.join("track-item");
        std::fs::create_dir_all(&track_dir).unwrap();
        write(&track_dir.join("domain-types.json"), EMPTY_CATALOGUE_JSON);
        write(&track_dir.join("usecase-types.json"), EMPTY_CATALOGUE_JSON);

        // infrastructure-types.json is a symlink pointing to a real file elsewhere.
        let real_target = root.join("elsewhere.json");
        write(&real_target, EMPTY_CATALOGUE_JSON);
        let symlink = track_dir.join("infrastructure-types.json");
        std::os::unix::fs::symlink(&real_target, &symlink).unwrap();

        let err = load_all_catalogues(&track_dir, &rules, root).unwrap_err();
        assert!(
            matches!(err, LoadAllCataloguesError::Io { .. }),
            "symlink catalogue must be rejected via Io error, got {err:?}"
        );
    }

    #[test]
    fn test_topological_sort_detects_cycle() {
        let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
        deps.insert("a".to_owned(), vec!["b".to_owned()]);
        deps.insert("b".to_owned(), vec!["a".to_owned()]);

        let err = topological_sort(&["a", "b"], &deps).unwrap_err();
        match err {
            LoadAllCataloguesError::TopologicalSortFailed { cycle } => {
                let mut sorted = cycle.clone();
                sorted.sort();
                assert_eq!(sorted, vec!["a".to_owned(), "b".to_owned()]);
            }
            other => panic!("expected TopologicalSortFailed, got {other:?}"),
        }
    }

    #[test]
    fn test_topological_sort_ignores_deps_outside_enabled_set() {
        // `usecase` depends on `domain`, but only `usecase` is enabled.
        let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
        deps.insert("usecase".to_owned(), vec!["domain".to_owned()]);
        deps.insert("domain".to_owned(), vec![]);

        let ordered = topological_sort(&["usecase"], &deps).unwrap();
        assert_eq!(ordered, vec!["usecase".to_owned()]);
    }

    #[test]
    fn test_topological_sort_preserves_tie_order_from_input() {
        // Two independent roots; input order is [b, a]; output must match.
        let deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let ordered = topological_sort(&["b", "a"], &deps).unwrap();
        assert_eq!(ordered, vec!["b".to_owned(), "a".to_owned()]);
    }

    #[test]
    fn test_extract_type_names_is_reusable_from_this_module() {
        // Visibility test: the pub(crate) promotion of extract_type_names
        // allows `catalogue_bulk_loader` tests to import and call it.
        use crate::tddd::type_graph_render::extract_type_names;
        let names = extract_type_names("Result<Option<User>, DomainError>");
        assert_eq!(names, vec!["Result", "Option", "User", "DomainError"]);
    }
}
