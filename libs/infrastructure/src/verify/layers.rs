//! Verify workspace layer dependencies against architecture rules.
//!
//! Rust port of `scripts/check_layers.py`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

use domain::verify::{VerifyFinding, VerifyOutcome};

const ARCH_RULES_FILE: &str = "architecture-rules.json";

/// A parsed layer rule from `architecture-rules.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LayerRule {
    crate_name: String,
    path: String,
    may_depend_on: Vec<String>,
}

/// Check layer dependency constraints for all workspace crates.
///
/// Loads `architecture-rules.json`, runs `cargo metadata`, and verifies
/// that no workspace crate depends on another crate outside its allowed set.
///
/// # Errors
///
/// Returns findings when the rules file is missing, cargo metadata fails,
/// or any direct/transitive layer violation is detected.
pub fn verify(root: &Path) -> VerifyOutcome {
    let rules_json = match load_architecture_rules(root) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Failed to load {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let metadata = match load_cargo_metadata(root) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Failed to run cargo metadata: {e}"
            ))]);
        }
    };

    verify_with_metadata(root, &rules_json, &metadata)
}

/// Verify layer dependencies using pre-loaded cargo metadata JSON.
///
/// This variant accepts the metadata value directly so that tests can inject
/// mock metadata without spawning a real `cargo` subprocess.
///
/// # Errors
///
/// Returns findings when the rules cannot be parsed, the metadata structure is
/// invalid, or any direct/transitive layer violation is detected.
pub fn verify_with_metadata(
    root: &Path,
    rules_json: &serde_json::Value,
    metadata: &serde_json::Value,
) -> VerifyOutcome {
    let rules = match layer_rules(rules_json) {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Failed to parse layer rules: {e}"
            ))]);
        }
    };

    let actual_graph = match workspace_graph(metadata) {
        Ok(g) => g,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Failed to build workspace graph: {e}"
            ))]);
        }
    };

    let _ = root; // root is used only for loading files; not needed here
    let allowed_graph = allowed_dependency_graph(&rules);

    let mut findings = Vec::new();

    // Report crates required by rules but absent from workspace metadata.
    for rule in &rules {
        if !actual_graph.contains_key(rule.crate_name.as_str()) {
            findings.push(VerifyFinding::error(format!(
                "{}: required crate not found in workspace metadata",
                rule.crate_name
            )));
        }
    }

    // Check each crate that is present in both rules and actual graph.
    for rule in &rules {
        if !actual_graph.contains_key(rule.crate_name.as_str()) {
            continue;
        }
        for msg in direct_violations(&rule.crate_name, &actual_graph, &allowed_graph) {
            findings.push(VerifyFinding::error(msg));
        }
        for msg in transitive_violations(&rule.crate_name, &actual_graph, &allowed_graph) {
            findings.push(VerifyFinding::error(msg));
        }
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

/// Read and parse `architecture-rules.json` from the project root.
fn load_architecture_rules(root: &Path) -> Result<serde_json::Value, String> {
    let path = root.join(ARCH_RULES_FILE);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
}

/// Parse the `layers` array from the architecture rules JSON.
fn layer_rules(rules: &serde_json::Value) -> Result<Vec<LayerRule>, String> {
    let layers = rules
        .get("layers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing or invalid 'layers' array in architecture rules".to_owned())?;

    if layers.is_empty() {
        return Err("architecture rules 'layers' array is empty".to_owned());
    }

    let mut result = Vec::with_capacity(layers.len());
    let mut seen_crates = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();

    for (i, layer) in layers.iter().enumerate() {
        let obj = layer.as_object().ok_or_else(|| format!("layer[{i}] is not an object"))?;

        let crate_name = obj
            .get("crate")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("layer[{i}] 'crate' must be a non-empty string"))?
            .to_owned();

        let path = obj
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("layer '{crate_name}' must define a non-empty 'path'"))?
            .to_owned();

        let may_depend_on_arr = match obj.get("may_depend_on") {
            Some(v) => v
                .as_array()
                .ok_or_else(|| format!("layer '{crate_name}' has invalid 'may_depend_on'"))?,
            None => &Vec::new(), // Python defaults missing may_depend_on to []
        };
        let may_depend_on = may_depend_on_arr
            .iter()
            .enumerate()
            .map(|(j, v)| {
                v.as_str().filter(|s| !s.is_empty()).map(ToOwned::to_owned).ok_or_else(|| {
                    format!("layer '{crate_name}'.may_depend_on[{j}] must be a non-empty string")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        if !seen_crates.insert(crate_name.clone()) {
            return Err(format!("duplicate crate in architecture rules: {crate_name}"));
        }
        if !seen_paths.insert(path.clone()) {
            return Err(format!("duplicate path in architecture rules: {path}"));
        }

        result.push(LayerRule { crate_name, path, may_depend_on });
    }

    // Validate dependency references.
    let known: BTreeSet<&str> = result.iter().map(|l| l.crate_name.as_str()).collect();
    for layer in &result {
        for dep in &layer.may_depend_on {
            if !known.contains(dep.as_str()) {
                return Err(format!(
                    "layer '{}' references unknown dependency: {dep}",
                    layer.crate_name
                ));
            }
            if dep == &layer.crate_name {
                return Err(format!("layer '{}' cannot depend on itself", layer.crate_name));
            }
        }
    }

    Ok(result)
}

/// Run `cargo metadata --format-version 1 --locked` and return the parsed JSON.
fn load_cargo_metadata(root: &Path) -> Result<serde_json::Value, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to spawn cargo: {e}"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if detail.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        } else {
            detail
        };
        let detail =
            if detail.is_empty() { "unknown cargo metadata error".to_owned() } else { detail };
        return Err(format!("cargo metadata failed: {detail}"));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata returned invalid JSON: {e}"))
}

/// Build the actual dependency graph from cargo metadata.
///
/// Returns a map of `crate_name -> set of crate_names it depends on` for all
/// workspace crates, considering only normal (non-dev, non-build) dependencies.
fn workspace_graph(
    metadata: &serde_json::Value,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let packages = metadata
        .get("packages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "metadata missing 'packages' array".to_owned())?;

    let workspace_members = metadata
        .get("workspace_members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "metadata missing 'workspace_members' array".to_owned())?;

    let resolve = metadata
        .get("resolve")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "metadata missing 'resolve' object".to_owned())?;

    // Map package id -> package object.
    let package_by_id: BTreeMap<&str, &serde_json::Value> = packages
        .iter()
        .filter_map(|p| p.get("id").and_then(|id| id.as_str()).map(|id| (id, p)))
        .collect();

    // Set of workspace package ids.
    let workspace_ids: BTreeSet<&str> =
        workspace_members.iter().filter_map(|v| v.as_str()).collect();

    // Map workspace package id -> crate name.
    let name_by_id: BTreeMap<&str, &str> = workspace_ids
        .iter()
        .filter_map(|id| {
            package_by_id
                .get(id)
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(|name| (*id, name))
        })
        .collect();

    // Initialise graph with empty dep sets for every workspace crate.
    let mut graph: BTreeMap<String, BTreeSet<String>> =
        name_by_id.values().map(|&name| (name.to_owned(), BTreeSet::new())).collect();

    // Build resolve node lookup.
    let node_by_id: BTreeMap<&str, &serde_json::Value> = resolve
        .get("nodes")
        .and_then(|v| v.as_array())
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|n| n.get("id").and_then(|id| id.as_str()).map(|id| (id, n)))
                .collect()
        })
        .unwrap_or_default();

    for (package_id, package_name) in &name_by_id {
        let node = match node_by_id.get(package_id) {
            Some(n) => n,
            None => continue,
        };

        let entry = match graph.get_mut(*package_name) {
            Some(e) => e,
            None => continue,
        };

        if let Some(deps) = node.get("deps").and_then(|v| v.as_array()) {
            // Prefer structured `deps` array with dep_kinds.
            for dep_entry in deps {
                let dep_pkg = match dep_entry
                    .get("pkg")
                    .and_then(|v| v.as_str())
                    .filter(|id| name_by_id.contains_key(id))
                {
                    Some(p) => p,
                    None => continue,
                };

                let dep_name = match name_by_id.get(dep_pkg) {
                    Some(n) => *n,
                    None => continue,
                };

                match dep_entry.get("dep_kinds").and_then(|v| v.as_array()) {
                    None => {
                        // Missing dep_kinds: assume normal dep (safe default).
                        entry.insert(dep_name.to_owned());
                    }
                    Some(dep_kinds) if dep_kinds.is_empty() => {
                        // Empty dep_kinds: assume normal dep.
                        entry.insert(dep_name.to_owned());
                    }
                    Some(dep_kinds) => {
                        // Include only if at least one entry has kind == null (normal).
                        let has_normal = dep_kinds.iter().any(|dk| {
                            dk.as_object()
                                .is_some_and(|o| o.get("kind").is_none_or(|k| k.is_null()))
                        });
                        if has_normal {
                            entry.insert(dep_name.to_owned());
                        }
                    }
                }
            }
        } else {
            // Fallback: flat `dependencies` array (no kind info).
            if let Some(dependencies) = node.get("dependencies").and_then(|v| v.as_array()) {
                for dep_id in dependencies {
                    if let Some(dep_id_str) = dep_id.as_str() {
                        if let Some(&dep_name) = name_by_id.get(dep_id_str) {
                            entry.insert(dep_name.to_owned());
                        }
                    }
                }
            }
        }
    }

    Ok(graph)
}

/// Build the allowed dependency graph from layer rules.
///
/// Returns a map of `crate_name -> allowed crate names`.
fn allowed_dependency_graph(rules: &[LayerRule]) -> BTreeMap<String, BTreeSet<String>> {
    rules
        .iter()
        .map(|r| (r.crate_name.clone(), r.may_depend_on.iter().cloned().collect::<BTreeSet<_>>()))
        .collect()
}

/// BFS from `start`, returning all reachable nodes and the path to each.
///
/// The returned map is `node -> path` where path is the list of nodes
/// traversed from `start` to reach `node` (inclusive of both endpoints).
fn reachable_paths(
    graph: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
) -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut queue: VecDeque<Vec<String>> = VecDeque::new();
    queue.push_back(vec![start.to_owned()]);

    while let Some(path) = queue.pop_front() {
        let current = match path.last() {
            Some(c) => c.clone(),
            None => continue,
        };
        let neighbors = match graph.get(&current) {
            Some(n) => n.clone(),
            None => BTreeSet::new(),
        };
        for dep in neighbors {
            if dep == start || found.contains_key(&dep) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(dep.clone());
            found.insert(dep, next_path.clone());
            queue.push_back(next_path);
        }
    }

    found
}

/// Return error messages for direct dependencies that violate layer rules.
fn direct_violations(
    crate_name: &str,
    actual: &BTreeMap<String, BTreeSet<String>>,
    allowed: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let actual_deps = match actual.get(crate_name) {
        Some(d) => d,
        None => return Vec::new(),
    };
    let allowed_deps = allowed.get(crate_name).cloned().unwrap_or_default();

    actual_deps
        .iter()
        .filter(|dep| !allowed_deps.contains(*dep))
        .map(|dep| format!("{crate_name}: prohibited direct dependency path {crate_name} -> {dep}"))
        .collect()
}

/// Return error messages for transitive dependencies that violate layer rules.
///
/// Only violations that are NOT already reported as direct violations are included.
fn transitive_violations(
    crate_name: &str,
    actual: &BTreeMap<String, BTreeSet<String>>,
    allowed: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let actual_paths = reachable_paths(actual, crate_name);
    let allowed_reachable: BTreeSet<String> =
        reachable_paths(allowed, crate_name).into_keys().collect();
    let direct_deps = actual.get(crate_name).cloned().unwrap_or_default();

    actual_paths
        .iter()
        .filter(|(dep, _)| !allowed_reachable.contains(*dep) && !direct_deps.contains(*dep))
        .map(|(_, path)| {
            format!("{crate_name}: prohibited transitive dependency path {}", path.join(" -> "))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    /// Build minimal cargo metadata JSON that mimics the structure cargo produces.
    ///
    /// `packages`: list of `(id, name)` pairs — all are workspace members.
    /// `deps`: list of `(from_id, to_id, is_dev)` tuples.
    fn build_metadata(packages: &[(&str, &str)], deps: &[(&str, &str, bool)]) -> serde_json::Value {
        // Construct packages array.
        let packages_json: Vec<serde_json::Value> = packages
            .iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "version": "0.1.0",
                    "dependencies": []
                })
            })
            .collect();

        // workspace_members: all package ids.
        let workspace_members: Vec<serde_json::Value> =
            packages.iter().map(|(id, _)| serde_json::json!(id)).collect();

        // Build node -> deps map.
        let mut node_deps: BTreeMap<&str, Vec<serde_json::Value>> = BTreeMap::new();
        for (from_id, _, _) in deps {
            node_deps.entry(from_id).or_default();
        }
        for (from_id, to_id, is_dev) in deps {
            let kind = if *is_dev { serde_json::json!("dev") } else { serde_json::Value::Null };
            node_deps.entry(from_id).or_default().push(serde_json::json!({
                "pkg": to_id,
                "name": to_id, // simplified
                "dep_kinds": [{"kind": kind, "target": null}]
            }));
        }

        // Build nodes array — one node per package.
        let nodes: Vec<serde_json::Value> = packages
            .iter()
            .map(|(id, _)| {
                let node_dep_list = node_deps.get(id).cloned().unwrap_or_default();
                serde_json::json!({
                    "id": id,
                    "deps": node_dep_list,
                    "features": [],
                    "dependencies": []
                })
            })
            .collect();

        serde_json::json!({
            "packages": packages_json,
            "workspace_members": workspace_members,
            "resolve": {
                "nodes": nodes,
                "root": null
            }
        })
    }

    /// Build a minimal architecture rules JSON for the test workspace.
    fn build_rules(layers: &[(&str, &str, &[&str])]) -> serde_json::Value {
        let layers_json: Vec<serde_json::Value> = layers
            .iter()
            .map(|(crate_name, path, may_depend_on)| {
                serde_json::json!({
                    "crate": crate_name,
                    "path": path,
                    "may_depend_on": may_depend_on,
                    "deny_reason": ""
                })
            })
            .collect();
        serde_json::json!({ "layers": layers_json })
    }

    // -----------------------------------------------------------------------
    // Test: clean workspace — no violations
    // -----------------------------------------------------------------------
    #[test]
    fn test_clean_workspace_passes() {
        // domain <- usecase, domain <- infrastructure, cli <- all three
        let packages = &[
            ("domain 0.1.0", "domain"),
            ("usecase 0.1.0", "usecase"),
            ("infrastructure 0.1.0", "infrastructure"),
            ("cli 0.1.0", "cli"),
        ];
        let deps = &[
            ("usecase 0.1.0", "domain 0.1.0", false),
            ("infrastructure 0.1.0", "domain 0.1.0", false),
            ("cli 0.1.0", "domain 0.1.0", false),
            ("cli 0.1.0", "usecase 0.1.0", false),
            ("cli 0.1.0", "infrastructure 0.1.0", false),
        ];
        let metadata = build_metadata(packages, deps);
        let rules = build_rules(&[
            ("domain", "libs/domain", &[]),
            ("usecase", "libs/usecase", &["domain"]),
            ("infrastructure", "libs/infrastructure", &["domain"]),
            ("cli", "apps/cli", &["domain", "usecase", "infrastructure"]),
        ]);

        let outcome = verify_with_metadata(Path::new("/fake"), &rules, &metadata);
        assert!(outcome.is_ok(), "expected pass but got: {outcome}");
    }

    // -----------------------------------------------------------------------
    // Test: direct violation detected
    // -----------------------------------------------------------------------
    #[test]
    fn test_direct_violation_detected() {
        // infrastructure depends on usecase — that is NOT allowed.
        let packages = &[
            ("domain 0.1.0", "domain"),
            ("usecase 0.1.0", "usecase"),
            ("infrastructure 0.1.0", "infrastructure"),
            ("cli 0.1.0", "cli"),
        ];
        let deps = &[
            ("usecase 0.1.0", "domain 0.1.0", false),
            // violation: infrastructure -> usecase
            ("infrastructure 0.1.0", "domain 0.1.0", false),
            ("infrastructure 0.1.0", "usecase 0.1.0", false),
            ("cli 0.1.0", "domain 0.1.0", false),
            ("cli 0.1.0", "usecase 0.1.0", false),
            ("cli 0.1.0", "infrastructure 0.1.0", false),
        ];
        let metadata = build_metadata(packages, deps);
        let rules = build_rules(&[
            ("domain", "libs/domain", &[]),
            ("usecase", "libs/usecase", &["domain"]),
            ("infrastructure", "libs/infrastructure", &["domain"]),
            ("cli", "apps/cli", &["domain", "usecase", "infrastructure"]),
        ]);

        let outcome = verify_with_metadata(Path::new("/fake"), &rules, &metadata);
        assert!(outcome.has_errors(), "expected violation but outcome passed");

        let messages: Vec<&str> = outcome.findings().iter().map(|f| f.message()).collect();
        let has_direct_violation = messages
            .iter()
            .any(|m| m.contains("infrastructure") && m.contains("usecase") && m.contains("direct"));
        assert!(has_direct_violation, "expected direct violation message; got: {messages:?}");
    }

    // -----------------------------------------------------------------------
    // Test: transitive violation detected
    // -----------------------------------------------------------------------
    #[test]
    fn test_transitive_violation_detected() {
        // domain must not depend on anything.
        // Introduce a crate "external" that is in the workspace and domain
        // depends on it transitively via a helper crate that is allowed
        // directly but reaches "external".
        //
        // Simpler scenario: usecase should only depend on domain.
        // If usecase -> infrastructure (direct) -> domain (ok),
        // the transitive chain usecase -> infrastructure is already a direct
        // violation; to isolate a _purely_ transitive violation we need a
        // three-hop chain: usecase -> domain -> bad_lib where domain is not
        // allowed to depend on bad_lib.
        //
        // Setup: layers are domain (may: []) and usecase (may: [domain]).
        // actual: domain -> bad_lib (direct, violates domain rule).
        // usecase -> domain (allowed direct).
        // usecase -> bad_lib is then a transitive violation from usecase's PoV
        // because bad_lib is not in usecase's allowed set and it is reached only
        // via domain.
        let packages = &[
            ("domain 0.1.0", "domain"),
            ("usecase 0.1.0", "usecase"),
            ("bad_lib 0.1.0", "bad_lib"),
        ];
        let deps = &[
            // domain directly depends on bad_lib — direct violation for domain
            ("domain 0.1.0", "bad_lib 0.1.0", false),
            // usecase depends on domain (allowed)
            ("usecase 0.1.0", "domain 0.1.0", false),
            // usecase -> bad_lib is transitive (not direct)
        ];
        let metadata = build_metadata(packages, deps);
        let rules = build_rules(&[
            ("domain", "libs/domain", &[]),
            ("usecase", "libs/usecase", &["domain"]),
            ("bad_lib", "libs/bad_lib", &[]),
        ]);

        let outcome = verify_with_metadata(Path::new("/fake"), &rules, &metadata);
        assert!(outcome.has_errors(), "expected violations but outcome passed");

        let messages: Vec<&str> = outcome.findings().iter().map(|f| f.message()).collect();

        // domain: direct violation domain -> bad_lib
        let has_domain_direct = messages
            .iter()
            .any(|m| m.contains("domain") && m.contains("bad_lib") && m.contains("direct"));
        assert!(has_domain_direct, "expected domain direct violation; got: {messages:?}");

        // usecase: transitive violation usecase -> domain -> bad_lib
        let has_usecase_transitive = messages.iter().any(|m| {
            m.contains("usecase")
                && m.contains("domain")
                && m.contains("bad_lib")
                && m.contains("transitive")
        });
        assert!(has_usecase_transitive, "expected usecase transitive violation; got: {messages:?}");
    }

    // -----------------------------------------------------------------------
    // Test: crate in rules but absent from workspace metadata is reported
    // -----------------------------------------------------------------------
    #[test]
    fn test_missing_crate_reported() {
        // Only "domain" is in the workspace, but rules also require "usecase".
        let packages = &[("domain 0.1.0", "domain")];
        let deps = &[];
        let metadata = build_metadata(packages, deps);
        let rules = build_rules(&[
            ("domain", "libs/domain", &[]),
            ("usecase", "libs/usecase", &["domain"]),
        ]);

        let outcome = verify_with_metadata(Path::new("/fake"), &rules, &metadata);
        assert!(outcome.has_errors(), "expected missing crate error but outcome passed");

        let messages: Vec<&str> = outcome.findings().iter().map(|f| f.message()).collect();
        let has_missing = messages.iter().any(|m| m.contains("usecase") && m.contains("not found"));
        assert!(has_missing, "expected missing-crate message; got: {messages:?}");
    }

    // -----------------------------------------------------------------------
    // Test: dev-dependencies are excluded from violation checks
    // -----------------------------------------------------------------------
    #[test]
    fn test_dev_dependencies_ignored() {
        // domain has a dev-dependency on "test_helpers" — this must NOT be
        // flagged as a violation because dev-deps are excluded.
        let packages = &[("domain 0.1.0", "domain"), ("test_helpers 0.1.0", "test_helpers")];
        let deps = &[
            // dev-only dep: should be ignored
            ("domain 0.1.0", "test_helpers 0.1.0", true),
        ];
        let metadata = build_metadata(packages, deps);
        let rules = build_rules(&[
            ("domain", "libs/domain", &[]),
            ("test_helpers", "libs/test_helpers", &[]),
        ]);

        let outcome = verify_with_metadata(Path::new("/fake"), &rules, &metadata);
        assert!(outcome.is_ok(), "dev-dep should not trigger violation; got: {outcome}");
    }

    // -----------------------------------------------------------------------
    // Helper unit tests for internal functions
    // -----------------------------------------------------------------------

    #[test]
    fn test_reachable_paths_bfs_order() {
        // Graph: a -> b, b -> c, a -> c (shortcut)
        // BFS from a: b is found via [a,b], c via [a,c] (shorter path wins).
        let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        graph.insert("a".to_owned(), ["b".to_owned(), "c".to_owned()].into());
        graph.insert("b".to_owned(), ["c".to_owned()].into());
        graph.insert("c".to_owned(), BTreeSet::new());

        let paths = reachable_paths(&graph, "a");
        assert_eq!(paths.get("b").unwrap(), &vec!["a".to_owned(), "b".to_owned()]);
        // BFS finds c via a -> c before a -> b -> c
        assert_eq!(paths.get("c").unwrap(), &vec!["a".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn test_layer_rules_parsing_succeeds() {
        let json = serde_json::json!({
            "layers": [
                {"crate": "domain", "path": "libs/domain", "may_depend_on": [], "deny_reason": ""},
                {"crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"], "deny_reason": ""}
            ]
        });
        let rules = layer_rules(&json).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].crate_name, "domain");
        assert!(rules[0].may_depend_on.is_empty());
        assert_eq!(rules[1].crate_name, "usecase");
        assert_eq!(rules[1].may_depend_on, vec!["domain".to_owned()]);
    }

    #[test]
    fn test_layer_rules_missing_field_returns_error() {
        let json = serde_json::json!({
            "layers": [
                {"path": "libs/domain", "may_depend_on": []}
                // missing "crate" field
            ]
        });
        let result = layer_rules(&json);
        assert!(result.is_err(), "expected error for missing 'crate' field");
    }
}
