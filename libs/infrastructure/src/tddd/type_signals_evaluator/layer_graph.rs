//! Closed implementation-input closure derived from architecture-rules.json.
//!
//! The graph is deliberately the only dependency authority used by the
//! implementation-input hash. Cargo manifests and Rust source text are not
//! interpreted here: a layer includes its own crate directory and every crate
//! reachable through may_depend_on.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LayerCrateRoot {
    pub(crate) crate_name: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LayerGraph {
    layers: BTreeMap<String, LayerNode>,
    schema_export_targets: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
struct LayerNode {
    path: String,
    may_depend_on: Vec<String>,
}

impl LayerGraph {
    /// Parses and validates the layer graph from the committed rules bytes.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| format!("architecture-rules.json is not valid UTF-8: {error}"))?;
        let value: Value = serde_json::from_str(text)
            .map_err(|error| format!("architecture-rules.json parse error: {error}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "architecture-rules.json must be a JSON object".to_owned())?;
        let version = object
            .get("version")
            .and_then(Value::as_u64)
            .ok_or_else(|| "architecture-rules.json version is missing or invalid".to_owned())?;
        if version != 2 {
            return Err(format!(
                "architecture-rules.json version {version} is not supported (expected 2)"
            ));
        }

        let layer_values = object
            .get("layers")
            .and_then(Value::as_array)
            .filter(|layers| !layers.is_empty())
            .ok_or_else(|| {
                "architecture-rules.json must define a non-empty 'layers' array".to_owned()
            })?;
        let mut layers = BTreeMap::new();
        let mut schema_export_targets = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for layer_value in layer_values {
            let layer = layer_value
                .as_object()
                .ok_or_else(|| "architecture-rules layer entries must be objects".to_owned())?;
            let crate_name = required_string(layer, "crate", "layer crate")?;
            validate_crate_name(&crate_name)?;
            let path = required_string(layer, "path", "layer path")?;
            validate_layer_path(&crate_name, &path)?;
            if layers.contains_key(&crate_name) {
                return Err(format!("duplicate crate in architecture rules: {crate_name}"));
            }
            if !paths.insert(path.clone()) {
                return Err(format!("duplicate path in architecture rules: {path}"));
            }
            let may_depend_on = match layer.get("may_depend_on") {
                None => Vec::new(),
                Some(value) => value
                    .as_array()
                    .ok_or_else(|| {
                        format!("layer '{crate_name}' has invalid 'may_depend_on' entries")
                    })?
                    .iter()
                    .map(|dependency| {
                        let dependency = dependency
                            .as_str()
                            .filter(|name| !name.is_empty())
                            .ok_or_else(|| {
                                format!("layer '{crate_name}' has invalid 'may_depend_on' entries")
                            })?;
                        if dependency == crate_name {
                            return Err(format!("layer '{crate_name}' cannot depend on itself"));
                        }
                        Ok(dependency.to_owned())
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            };
            for target in enabled_schema_export_targets(layer, &crate_name)? {
                if let Some(previous_layer) =
                    schema_export_targets.insert(target.clone(), crate_name.clone())
                {
                    return Err(format!(
                        "schema-export target '{target}' is declared by both '{previous_layer}' and '{crate_name}'"
                    ));
                }
            }
            layers.insert(crate_name, LayerNode { path, may_depend_on });
        }

        for (crate_name, layer) in &layers {
            for dependency in &layer.may_depend_on {
                if !layers.contains_key(dependency) {
                    return Err(format!(
                        "layer '{crate_name}' references unknown dependency '{dependency}'"
                    ));
                }
            }
        }

        for (target, layer) in &schema_export_targets {
            if layers.contains_key(target) && target != layer {
                return Err(format!(
                    "schema-export target '{target}' conflicts with architecture layer '{target}' (owned by '{layer}')"
                ));
            }
        }

        Ok(Self { layers, schema_export_targets })
    }

    /// Returns the target layer plus its transitive allowed dependency layers,
    /// in stable crate-name order. The input may be either a layer id or an
    /// explicit schema-export target; the latter is resolved through the
    /// committed architecture rules before traversing the layer graph.
    pub(crate) fn crate_roots_for(&self, target: &str) -> Result<Vec<LayerCrateRoot>, String> {
        let target_layer = if self.layers.contains_key(target) {
            target.to_owned()
        } else {
            self.schema_export_targets.get(target).cloned().ok_or_else(|| {
                format!(
                    "target layer or schema-export target '{target}' is not present in architecture-rules.json"
                )
            })?
        };
        let mut pending = vec![target_layer];
        let mut selected = BTreeSet::new();
        while let Some(crate_name) = pending.pop() {
            if !selected.insert(crate_name.clone()) {
                continue;
            }
            let layer = self.layers.get(&crate_name).ok_or_else(|| {
                format!("target layer '{target}' is not present in architecture-rules.json")
            })?;
            pending.extend(layer.may_depend_on.iter().cloned());
        }

        selected
            .into_iter()
            .map(|crate_name| {
                let layer = self
                    .layers
                    .get(&crate_name)
                    .ok_or_else(|| format!("layer '{crate_name}' disappeared from graph"))?;
                Ok(LayerCrateRoot { crate_name, path: layer.path.clone() })
            })
            .collect()
    }
}

fn enabled_schema_export_targets(
    layer: &serde_json::Map<String, Value>,
    crate_name: &str,
) -> Result<Vec<String>, String> {
    let Some(tddd) = layer.get("tddd") else {
        return Ok(Vec::new());
    };
    let tddd = tddd
        .as_object()
        .ok_or_else(|| format!("layer '{crate_name}' has an invalid 'tddd' object"))?;
    if tddd.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(Vec::new());
    }
    let Some(schema_export) = tddd.get("schema_export") else {
        return Ok(Vec::new());
    };
    let schema_export = schema_export
        .as_object()
        .ok_or_else(|| format!("layer '{crate_name}' has an invalid 'schema_export' object"))?;
    let Some(targets) = schema_export.get("targets") else {
        return Ok(Vec::new());
    };
    let targets = targets
        .as_array()
        .ok_or_else(|| format!("layer '{crate_name}' has invalid schema-export targets"))?;
    targets
        .iter()
        .map(|target| {
            target
                .as_str()
                .filter(|target| !target.is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("layer '{crate_name}' has an invalid schema-export target"))
        })
        .collect()
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{label} must be a non-empty string"))
}

fn validate_crate_name(crate_name: &str) -> Result<(), String> {
    if crate_name.contains('/') || crate_name.contains('\\') || crate_name.contains("::") {
        return Err(format!("layer crate '{crate_name}' is not a safe name"));
    }
    if crate_name.chars().any(char::is_control) {
        return Err(format!("layer crate '{crate_name}' contains a control character"));
    }
    Ok(())
}

fn validate_layer_path(crate_name: &str, path: &str) -> Result<(), String> {
    if path.contains('\\')
        || path.ends_with('/')
        || path.split('/').any(str::is_empty)
        || path.chars().any(char::is_control)
    {
        return Err(format!("layer '{crate_name}' has an unsafe repo-relative path '{path}'"));
    }
    for component in Path::new(path).components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!("layer '{crate_name}' has an unsafe repo-relative path '{path}'"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::LayerGraph;

    #[test]
    fn test_layer_graph_includes_transitive_may_depend_on_closure() {
        let graph = LayerGraph::parse(
            br#"{
              "version": 2,
              "layers": [
                {"crate":"domain","path":"libs/domain","may_depend_on":[]},
                {"crate":"usecase","path":"libs/usecase","may_depend_on":["domain"]},
                {"crate":"cli","path":"apps/cli","may_depend_on":["usecase"]},
                {"crate":"outside","path":"libs/outside","may_depend_on":[]}
              ]
            }"#,
        )
        .unwrap();
        let roots = graph.crate_roots_for("cli").unwrap();
        assert_eq!(
            roots.iter().map(|root| root.path.as_str()).collect::<Vec<_>>(),
            vec!["apps/cli", "libs/domain", "libs/usecase"]
        );
        assert!(!roots.iter().any(|root| root.crate_name == "outside"));
    }

    #[test]
    fn test_layer_graph_rejects_unknown_dependency() {
        let error = LayerGraph::parse(
            br#"{
              "version": 2,
              "layers": [{"crate":"usecase","path":"libs/usecase","may_depend_on":["missing"]}]
            }"#,
        )
        .unwrap_err();
        assert!(error.contains("unknown dependency"), "got: {error}");
    }

    #[test]
    fn test_layer_graph_ignores_module_limit_exclusions() {
        let graph = LayerGraph::parse(
            br#"{
              "version": 2,
              "module_limits": {"exclude": ["vendor/", "tmp/**"]},
              "layers": [{"crate":"domain","path":"libs/domain","may_depend_on":[]}]
            }"#,
        )
        .unwrap();
        let roots = graph.crate_roots_for("domain").unwrap();
        assert_eq!(
            roots.iter().map(|root| root.path.as_str()).collect::<Vec<_>>(),
            ["libs/domain"]
        );
    }

    #[test]
    fn test_layer_graph_resolves_explicit_schema_export_target_to_owning_layer() {
        let graph = LayerGraph::parse(
            br#"{
              "version": 2,
              "layers": [
                {"crate":"domain","path":"libs/domain","may_depend_on":[]},
                {"crate":"usecase","path":"libs/usecase","may_depend_on":["domain"],
                 "tddd":{"enabled":true,"schema_export":{"targets":["application"]}}}
              ]
            }"#,
        )
        .unwrap();
        let roots = graph.crate_roots_for("application").unwrap();
        assert_eq!(
            roots.iter().map(|root| root.crate_name.as_str()).collect::<Vec<_>>(),
            vec!["domain", "usecase"]
        );
    }

    #[test]
    fn test_layer_graph_rejects_trailing_separator_in_layer_path() {
        let error = LayerGraph::parse(
            br#"{
              "version": 2,
              "layers": [{"crate":"domain","path":"libs/domain/","may_depend_on":[]}]
            }"#,
        )
        .unwrap_err();
        assert!(error.contains("unsafe repo-relative path"), "got: {error}");
    }

    #[test]
    fn test_layer_graph_rejects_repeated_separator_in_layer_path() {
        let error = LayerGraph::parse(
            br#"{
              "version": 2,
              "layers": [{"crate":"domain","path":"libs//domain","may_depend_on":[]}]
            }"#,
        )
        .unwrap_err();
        assert!(error.contains("unsafe repo-relative path"), "got: {error}");
    }
}
