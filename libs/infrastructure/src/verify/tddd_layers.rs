//! Multilayer TDDD configuration parser for `architecture-rules.json`.
//!
//! Parses the optional `tddd` block on each `layers[]` entry and produces
//! a canonical list of enabled layers with their resolved catalogue file
//! names. Unknown layers and disabled layers are excluded.
//!
//! T007 (TDDD-01 Phase 1 Task 7): each enabled layer produces one
//! `TdddLayerBinding` describing which catalogue file to read and which
//! crates to export. `catalogue_file` defaults to `<layers[].crate>-types.json`
//! when omitted. Duplicate `catalogue_file` values across enabled layers are
//! rejected fail-closed so that one layer cannot overwrite another's
//! catalogue.
//!
//! Reference: ADR `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §D1.

use std::collections::HashSet;

use serde::Deserialize;

/// A single enabled TDDD layer binding resolved from `architecture-rules.json`.
///
/// All fields are normalized:
/// - `layer_id` is the `layers[].crate` value.
/// - `catalogue_file` is the resolved file name (default
///   `<layer_id>-types.json` when `tddd.catalogue_file` is omitted).
/// - `targets` is the `tddd.schema_export.targets` crate list (defaults to
///   `[layer_id]` when omitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdddLayerBinding {
    layer_id: String,
    catalogue_file: String,
    targets: Vec<String>,
}

impl TdddLayerBinding {
    /// Returns the layer ID (matches `layers[].crate`).
    #[must_use]
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }

    /// Returns the catalogue file name relative to the track directory.
    #[must_use]
    pub fn catalogue_file(&self) -> &str {
        &self.catalogue_file
    }

    /// Returns the baseline file name (`<stem>-baseline.json`).
    #[must_use]
    pub fn baseline_file(&self) -> String {
        let stem = self.catalogue_file.strip_suffix(".json").unwrap_or(&self.catalogue_file);
        format!("{stem}-baseline.json")
    }

    /// Returns the rendered markdown file name (`<stem>.md`).
    #[must_use]
    pub fn rendered_file(&self) -> String {
        let stem = self.catalogue_file.strip_suffix(".json").unwrap_or(&self.catalogue_file);
        format!("{stem}.md")
    }

    /// Returns the crate targets used by `schema_export`.
    #[must_use]
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

/// Error returned by [`parse_tddd_layers`].
#[derive(Debug, thiserror::Error)]
pub enum TdddLayerParseError {
    #[error("architecture-rules.json is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("architecture-rules.json must define a non-empty 'layers' array")]
    MissingLayers,

    #[error(
        "duplicate catalogue_file '{path}' across TDDD-enabled layers: '{first}' and '{second}'"
    )]
    DuplicateCatalogueFile { path: String, first: String, second: String },

    #[error("duplicate layer id '{id}' in TDDD-enabled bindings")]
    DuplicateLayerId { id: String },

    #[error(
        "layer id '{id}' contains unsafe path characters (must be a simple name without '/', '\\\\', or '..')"
    )]
    InvalidLayerId { id: String },

    #[error(
        "catalogue_file '{file}' contains unsafe path characters (must be a simple filename without '/', '\\\\', or '..')"
    )]
    InvalidCatalogueFile { file: String },
}

/// Returns `true` when `name` is a safe, simple filename or identifier with no
/// path traversal or invalid characters.
///
/// Rejects strings that:
/// - are empty,
/// - contain `/` or `\` (path separators),
/// - equal `..` (path traversal),
/// - contain `::` (Rust path separator — would produce invalid catalogue file names).
fn is_safe_path_component(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Reject absolute paths and path separators.
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    // Reject `..` as the entire string or as a component (e.g. `foo/../bar`
    // is already caught by the `/` check above, so we only need to handle
    // the bare `..` case).
    if name == ".." {
        return false;
    }
    // Reject `::` which is the Rust path separator. A layer id such as
    // `core::domain` would produce `core::domain-types.json` which is an
    // invalid catalogue filename (violates the L1 codec constraint and the
    // briefing's explicit D10 rule).
    if name.contains("::") {
        return false;
    }
    true
}

/// Parses the `tddd` blocks from `architecture-rules.json` content and
/// returns the list of enabled layers in `layers[]` order.
///
/// # Errors
///
/// Returns [`TdddLayerParseError`] when:
/// - the JSON is invalid,
/// - the top-level `layers` key is missing or not an array,
/// - a layer id or catalogue_file contains unsafe path characters, or
/// - two enabled layers resolve to the same `catalogue_file` value.
pub fn parse_tddd_layers(json: &str) -> Result<Vec<TdddLayerBinding>, TdddLayerParseError> {
    #[derive(Deserialize)]
    struct Root {
        layers: Vec<Layer>,
    }

    #[derive(Deserialize)]
    struct Layer {
        #[serde(rename = "crate")]
        crate_name: String,
        #[serde(default)]
        tddd: Option<TdddBlock>,
    }

    #[derive(Deserialize, Default)]
    struct TdddBlock {
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        catalogue_file: Option<String>,
        #[serde(default)]
        schema_export: Option<SchemaExportBlock>,
    }

    #[derive(Deserialize)]
    struct SchemaExportBlock {
        #[serde(default)]
        targets: Vec<String>,
    }

    let root: Root = serde_json::from_str(json)?;
    if root.layers.is_empty() {
        return Err(TdddLayerParseError::MissingLayers);
    }

    let mut bindings = Vec::new();
    let mut seen_catalogues: HashSet<String> = HashSet::new();
    let mut seen_layer_ids: HashSet<String> = HashSet::new();
    for layer in root.layers {
        let Some(tddd) = layer.tddd else {
            continue;
        };
        if !tddd.enabled {
            continue;
        }
        // Validate the layer id before using it to derive a filename.
        if !is_safe_path_component(&layer.crate_name) {
            return Err(TdddLayerParseError::InvalidLayerId { id: layer.crate_name });
        }
        // Reject duplicate layer ids. `find_binding` is a first-match lookup,
        // so a later duplicate would be silently shadowed and its catalogue
        // never verified — fail-closed during parsing instead.
        if !seen_layer_ids.insert(layer.crate_name.clone()) {
            return Err(TdddLayerParseError::DuplicateLayerId { id: layer.crate_name });
        }
        let catalogue_file =
            tddd.catalogue_file.unwrap_or_else(|| format!("{}-types.json", layer.crate_name));
        // Validate the resolved catalogue_file — covers both explicit values
        // and the default derived from the layer id (already validated above,
        // but an explicit override could still contain path traversal chars).
        if !is_safe_path_component(&catalogue_file) {
            return Err(TdddLayerParseError::InvalidCatalogueFile { file: catalogue_file });
        }
        if !seen_catalogues.insert(catalogue_file.clone()) {
            // Find the first binding that claims this catalogue file to
            // produce a descriptive error message.
            let first = bindings
                .iter()
                .find(|b: &&TdddLayerBinding| b.catalogue_file == catalogue_file)
                .map_or_else(|| "<unknown>".to_owned(), |b| b.layer_id.clone());
            return Err(TdddLayerParseError::DuplicateCatalogueFile {
                path: catalogue_file,
                first,
                second: layer.crate_name,
            });
        }
        let targets =
            tddd.schema_export.map(|s| s.targets).filter(|t| !t.is_empty()).unwrap_or_else(|| {
                // Default: `targets = [layer_id]`.
                // An absent or empty `schema_export.targets` both default to
                // `[layer_id]` per the T007 contract.
                vec![layer.crate_name.clone()]
            });
        bindings.push(TdddLayerBinding { layer_id: layer.crate_name, catalogue_file, targets });
    }

    Ok(bindings)
}

/// Returns the binding whose `layer_id` matches `layer_id`, or `None`.
#[must_use]
pub fn find_binding<'a>(
    bindings: &'a [TdddLayerBinding],
    layer_id: &str,
) -> Option<&'a TdddLayerBinding> {
    bindings.iter().find(|b| b.layer_id == layer_id)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tddd_layers_enabled_only_returned() {
        let json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            { "crate": "usecase", "tddd": { "enabled": false } },
            { "crate": "infrastructure" }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].layer_id(), "domain");
        assert_eq!(bindings[0].catalogue_file(), "domain-types.json");
    }

    #[test]
    fn test_parse_tddd_layers_default_catalogue_file_uses_crate_name() {
        let json = r#"{
          "layers": [
            { "crate": "my-layer", "tddd": { "enabled": true } }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].catalogue_file(), "my-layer-types.json");
    }

    #[test]
    fn test_parse_tddd_layers_baseline_and_rendered_derived_from_stem() {
        let json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert_eq!(bindings[0].baseline_file(), "domain-types-baseline.json");
        assert_eq!(bindings[0].rendered_file(), "domain-types.md");
    }

    #[test]
    fn test_parse_tddd_layers_default_targets_is_crate_name() {
        let json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true } }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert_eq!(bindings[0].targets(), &["domain".to_string()]);
    }

    #[test]
    fn test_parse_tddd_layers_explicit_targets_preserved() {
        let json = r#"{
          "layers": [
            {
              "crate": "domain",
              "tddd": {
                "enabled": true,
                "schema_export": { "method": "rustdoc", "targets": ["domain", "shared"] }
              }
            }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert_eq!(bindings[0].targets(), &["domain".to_string(), "shared".to_string()]);
    }

    #[test]
    fn test_parse_tddd_layers_duplicate_catalogue_rejected() {
        let json = r#"{
          "layers": [
            { "crate": "a", "tddd": { "enabled": true, "catalogue_file": "shared.json" } },
            { "crate": "b", "tddd": { "enabled": true, "catalogue_file": "shared.json" } }
          ]
        }"#;
        let err = parse_tddd_layers(json).unwrap_err();
        match err {
            TdddLayerParseError::DuplicateCatalogueFile { path, first, second } => {
                assert_eq!(path, "shared.json");
                assert_eq!(first, "a");
                assert_eq!(second, "b");
            }
            other => panic!("expected DuplicateCatalogueFile, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_tddd_layers_empty_layers_rejected() {
        let json = r#"{ "layers": [] }"#;
        let err = parse_tddd_layers(json).unwrap_err();
        assert!(matches!(err, TdddLayerParseError::MissingLayers));
    }

    #[test]
    fn test_parse_tddd_layers_missing_layers_rejected() {
        let json = r#"{}"#;
        assert!(parse_tddd_layers(json).is_err());
    }

    #[test]
    fn test_parse_tddd_layers_invalid_json_rejected() {
        let json = r#"not json"#;
        assert!(matches!(parse_tddd_layers(json).unwrap_err(), TdddLayerParseError::Json(_)));
    }

    #[test]
    fn test_find_binding_returns_matching() {
        let bindings = vec![
            TdddLayerBinding {
                layer_id: "domain".to_string(),
                catalogue_file: "domain-types.json".to_string(),
                targets: vec!["domain".to_string()],
            },
            TdddLayerBinding {
                layer_id: "usecase".to_string(),
                catalogue_file: "usecase-types.json".to_string(),
                targets: vec!["usecase".to_string()],
            },
        ];
        assert_eq!(find_binding(&bindings, "usecase").unwrap().layer_id(), "usecase");
        assert!(find_binding(&bindings, "infrastructure").is_none());
    }
}
