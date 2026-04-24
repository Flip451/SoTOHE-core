//! Multilayer TDDD configuration parser for `architecture-rules.json`.
//!
//! Parses the optional `tddd` block on each `layers[]` entry and produces
//! a canonical list of enabled layers with their resolved catalogue file
//! names. Unknown layers and disabled layers are excluded.
//!
//! Each enabled layer produces one `TdddLayerBinding` describing which
//! catalogue file to read and which crates to export. `catalogue_file`
//! defaults to `<layers[].crate>-types.json` when omitted. Duplicate
//! `catalogue_file` values across enabled layers are rejected fail-closed so
//! that one layer cannot overwrite another's catalogue.
//!
//! Reference: ADR `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §D1.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
    catalogue_spec_signal_enabled: bool,
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

    /// Returns the per-layer evaluation-result file name for the TDDD signal split.
    ///
    /// The evaluation-result file (`<layer>-type-signals.json`) is introduced by
    /// ADR `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md`
    /// §D1 alongside the stripped-down declaration file `<layer>-types.json`.
    ///
    /// Naming transformation:
    ///
    /// - Strip the `.json` suffix to get the stem.
    /// - If the stem ends in `s` (the conventional plural in `<layer>-types`),
    ///   drop the trailing `s` and append `-signals`.
    ///   `"domain-types.json"` → `"domain-type-signals.json"`.
    /// - Otherwise, append `-signals` directly.
    ///   `"custom.json"` → `"custom-signals.json"`.
    ///
    /// No I/O — this is a pure string derivation from `catalogue_file`. Callers
    /// are responsible for applying the `reject_symlinks_below` guard before
    /// reading or writing the returned path.
    #[must_use]
    pub fn signal_file(&self) -> String {
        let stem = self.catalogue_file.strip_suffix(".json").unwrap_or(&self.catalogue_file);
        let signal_stem = if let Some(trimmed) = stem.strip_suffix('s') {
            format!("{trimmed}-signals")
        } else {
            format!("{stem}-signals")
        };
        format!("{signal_stem}.json")
    }

    /// Returns `true` when the layer has
    /// `tddd.catalogue_spec_signal.enabled = true` in `architecture-rules.json`.
    ///
    /// Defaults to `false` when the subblock is absent (per ADR §D5.4
    /// phased activation: catalogue-spec signal is opt-in per layer).
    #[must_use]
    pub fn catalogue_spec_signal_enabled(&self) -> bool {
        self.catalogue_spec_signal_enabled
    }

    /// Returns the per-layer catalogue-spec signals file name
    /// (`<layer_id>-catalogue-spec-signals.json`).
    ///
    /// This file stores the SoT Chain ② signals (catalogue-entry ↔ spec
    /// grounding) per ADR `2026-04-23-0344-catalogue-spec-signal-activation.md`
    /// §D2.2. The naming convention differs from [`Self::signal_file`]
    /// (which derives from `catalogue_file`): the catalogue-spec-signals
    /// path is always `<layer_id>-catalogue-spec-signals.json`, mirroring
    /// the `FsCatalogueSpecSignalsStore` write path (§D3.7).
    ///
    /// No I/O — pure string derivation from `layer_id`. Callers apply the
    /// `reject_symlinks_below` guard before reading.
    #[must_use]
    pub fn catalogue_spec_signal_file(&self) -> String {
        format!("{}-catalogue-spec-signals.json", self.layer_id)
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
        catalogue_spec_signal: Option<CatalogueSpecSignalBlock>,
        #[serde(default)]
        schema_export: Option<SchemaExportBlock>,
    }

    #[derive(Deserialize, Default)]
    struct CatalogueSpecSignalBlock {
        #[serde(default)]
        enabled: bool,
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
                // `[layer_id]` per the layer binding contract.
                vec![layer.crate_name.clone()]
            });
        let catalogue_spec_signal_enabled =
            tddd.catalogue_spec_signal.map(|b| b.enabled).unwrap_or(false);
        bindings.push(TdddLayerBinding {
            layer_id: layer.crate_name,
            catalogue_file,
            catalogue_spec_signal_enabled,
            targets,
        });
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

/// Error returned by [`load_tddd_layers_from_path`].
#[derive(Debug, thiserror::Error)]
pub enum LoadTdddLayersError {
    #[error("I/O error for {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Parse(#[from] TdddLayerParseError),
}

/// Loads TDDD layer bindings from `architecture-rules.json` at `path`.
///
/// Delegates symlink rejection to the shared I/O guard
/// [`crate::track::symlink_guard::reject_symlinks_below`] so the symlink
/// handling policy lives in a single place in the infrastructure layer. When
/// the guard reports the path as genuinely absent, returns a synthetic
/// domain-only binding (legacy fallback for pre-multilayer tracks). Any
/// symlink at the leaf or an ancestor, or any other I/O error, is reported as
/// a hard failure so the misconfiguration surfaces instead of being masked by
/// the fallback.
///
/// Shared by `apps/cli::resolve_layers` and
/// `libs/infrastructure::track::render::sync_rendered_views` so callers do not
/// need to reimplement the symlink/legacy-fallback policy themselves.
///
/// `trusted_root` is passed through to `reject_symlinks_below` — only
/// components below it are inspected.
///
/// # Errors
///
/// Returns [`LoadTdddLayersError::Io`] when the symlink guard rejects the
/// path (symlink at leaf or ancestor, or stat/read failure), and
/// [`LoadTdddLayersError::Parse`] when the JSON is invalid or violates any
/// constraint enforced by [`parse_tddd_layers`].
pub fn load_tddd_layers_from_path(
    path: &Path,
    trusted_root: &Path,
) -> Result<Vec<TdddLayerBinding>, LoadTdddLayersError> {
    match crate::track::symlink_guard::reject_symlinks_below(path, trusted_root) {
        Ok(true) => {
            // Path exists as a regular file (not a symlink). Read it.
            let content = std::fs::read_to_string(path)
                .map_err(|e| LoadTdddLayersError::Io { path: path.to_path_buf(), source: e })?;
            parse_tddd_layers(&content).map_err(LoadTdddLayersError::Parse)
        }
        Ok(false) => {
            // Path is truly absent (neither a file nor a symlink at the leaf).
            // Legacy fallback: a single synthetic domain binding keeps
            // pre-multilayer tracks working.
            parse_tddd_layers(
                r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#,
            )
            .map_err(LoadTdddLayersError::Parse)
        }
        Err(e) => Err(LoadTdddLayersError::Io { path: path.to_path_buf(), source: e }),
    }
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

    // --- signal_file() accessor (ADR 2026-04-18-1400 §D1) ---

    fn binding_with_catalogue(catalogue_file: &str) -> TdddLayerBinding {
        TdddLayerBinding {
            layer_id: "test".to_owned(),
            catalogue_file: catalogue_file.to_owned(),
            catalogue_spec_signal_enabled: false,
            targets: vec!["test".to_owned()],
        }
    }

    #[test]
    fn test_signal_file_domain_types_drops_trailing_s() {
        let binding = binding_with_catalogue("domain-types.json");
        assert_eq!(binding.signal_file(), "domain-type-signals.json");
    }

    #[test]
    fn test_signal_file_usecase_types_drops_trailing_s() {
        let binding = binding_with_catalogue("usecase-types.json");
        assert_eq!(binding.signal_file(), "usecase-type-signals.json");
    }

    #[test]
    fn test_signal_file_infrastructure_types_drops_trailing_s() {
        let binding = binding_with_catalogue("infrastructure-types.json");
        assert_eq!(binding.signal_file(), "infrastructure-type-signals.json");
    }

    #[test]
    fn test_signal_file_non_standard_stem_appends_signals() {
        let binding = binding_with_catalogue("custom.json");
        assert_eq!(binding.signal_file(), "custom-signals.json");
    }

    #[test]
    fn test_signal_file_stem_without_json_extension_still_appends_signals() {
        // Defensive: catalogue_file without `.json` suffix — the strip
        // returns the whole string unchanged. `"no-extension"` ends in `n`,
        // so the trailing-`s` branch does not fire and `-signals.json` is
        // appended verbatim.
        let binding = binding_with_catalogue("no-extension");
        assert_eq!(binding.signal_file(), "no-extension-signals.json");
    }

    #[test]
    fn test_signal_file_stem_ending_in_s_without_json_drops_trailing_s() {
        // Without `.json`, a stem ending in `s` still has the trailing `s`
        // dropped before `-signals` is appended. This mirrors the happy
        // path for `<layer>-types.json` but exercises the code path where
        // the `.json` strip is a no-op.
        let binding = binding_with_catalogue("foos");
        assert_eq!(binding.signal_file(), "foo-signals.json");
    }

    #[test]
    fn test_signal_file_is_pure_string_derivation() {
        // Repeated calls on the same binding must produce identical output
        // (no I/O, no caching, no side effects).
        let binding = binding_with_catalogue("domain-types.json");
        let first = binding.signal_file();
        let second = binding.signal_file();
        assert_eq!(first, second);
    }

    #[test]
    fn test_signal_file_differs_from_catalogue_and_baseline() {
        let binding = binding_with_catalogue("domain-types.json");
        let catalogue = binding.catalogue_file().to_owned();
        let baseline = binding.baseline_file();
        let signal = binding.signal_file();
        assert_ne!(signal, catalogue);
        assert_ne!(signal, baseline);
    }

    // --- catalogue_spec_signal_file() accessor (ADR 2026-04-23-0344 §D2.2) ---

    fn binding_with_layer_id(layer_id: &str, catalogue_file: &str) -> TdddLayerBinding {
        TdddLayerBinding {
            layer_id: layer_id.to_owned(),
            catalogue_file: catalogue_file.to_owned(),
            catalogue_spec_signal_enabled: false,
            targets: vec![layer_id.to_owned()],
        }
    }

    #[test]
    fn test_catalogue_spec_signal_file_derives_from_layer_id_not_catalogue_file() {
        // The catalogue-spec-signals path is derived from `layer_id`, NOT from
        // `catalogue_file` — this differs from `signal_file()` which derives
        // from `catalogue_file`. The invariant mirrors `FsCatalogueSpecSignalsStore`
        // (ADR §D3.7).
        let binding = binding_with_layer_id("domain", "domain-types.json");
        assert_eq!(binding.catalogue_spec_signal_file(), "domain-catalogue-spec-signals.json");

        let binding = binding_with_layer_id("usecase", "usecase-types.json");
        assert_eq!(binding.catalogue_spec_signal_file(), "usecase-catalogue-spec-signals.json");

        let binding = binding_with_layer_id("infrastructure", "infrastructure-types.json");
        assert_eq!(
            binding.catalogue_spec_signal_file(),
            "infrastructure-catalogue-spec-signals.json"
        );
    }

    #[test]
    fn test_catalogue_spec_signal_file_is_pure_string_derivation() {
        let binding = binding_with_layer_id("domain", "domain-types.json");
        let first = binding.catalogue_spec_signal_file();
        let second = binding.catalogue_spec_signal_file();
        assert_eq!(first, second);
    }

    #[test]
    fn test_catalogue_spec_signal_file_differs_from_signal_file() {
        // Regression guard: the two signal files must not collide.
        let binding = binding_with_layer_id("domain", "domain-types.json");
        assert_ne!(binding.catalogue_spec_signal_file(), binding.signal_file());
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

    // --- catalogue_spec_signal_enabled() accessor (T018) ---

    #[test]
    fn test_parse_tddd_layers_absent_catalogue_spec_signal_defaults_to_false() {
        // When `tddd.catalogue_spec_signal` is omitted, the accessor must return
        // `false` (opt-in semantics per ADR §D5.4).
        let json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true } }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert!(!bindings[0].catalogue_spec_signal_enabled());
    }

    #[test]
    fn test_parse_tddd_layers_catalogue_spec_signal_enabled_true_is_surfaced() {
        // When `tddd.catalogue_spec_signal.enabled = true`, the accessor must
        // return `true`.
        let json = r#"{
          "layers": [
            {
              "crate": "domain",
              "tddd": {
                "enabled": true,
                "catalogue_spec_signal": { "enabled": true }
              }
            }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert!(bindings[0].catalogue_spec_signal_enabled());
    }

    #[test]
    fn test_parse_tddd_layers_catalogue_spec_signal_enabled_false_explicit() {
        // Explicit `enabled: false` is equivalent to the absent-subblock default.
        let json = r#"{
          "layers": [
            {
              "crate": "domain",
              "tddd": {
                "enabled": true,
                "catalogue_spec_signal": { "enabled": false }
              }
            }
          ]
        }"#;
        let bindings = parse_tddd_layers(json).unwrap();
        assert!(!bindings[0].catalogue_spec_signal_enabled());
    }

    #[test]
    fn test_find_binding_returns_matching() {
        let bindings = vec![
            TdddLayerBinding {
                layer_id: "domain".to_string(),
                catalogue_file: "domain-types.json".to_string(),
                catalogue_spec_signal_enabled: false,
                targets: vec!["domain".to_string()],
            },
            TdddLayerBinding {
                layer_id: "usecase".to_string(),
                catalogue_file: "usecase-types.json".to_string(),
                catalogue_spec_signal_enabled: false,
                targets: vec!["usecase".to_string()],
            },
        ];
        assert_eq!(find_binding(&bindings, "usecase").unwrap().layer_id(), "usecase");
        assert!(find_binding(&bindings, "infrastructure").is_none());
    }

    #[test]
    fn test_load_tddd_layers_from_path_regular_file_returns_parsed_bindings() {
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");
        let json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            { "crate": "usecase", "tddd": { "enabled": true, "catalogue_file": "usecase-types.json" } }
          ]
        }"#;
        std::fs::write(&rules_path, json).unwrap();

        let bindings = load_tddd_layers_from_path(&rules_path, dir.path()).unwrap();

        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].catalogue_file(), "domain-types.json");
        assert_eq!(bindings[1].catalogue_file(), "usecase-types.json");
    }

    #[test]
    fn test_load_tddd_layers_from_path_missing_file_returns_legacy_domain_fallback() {
        // When architecture-rules.json is genuinely absent (not a broken symlink),
        // callers must get the single synthetic domain binding so pre-multilayer
        // tracks continue to work.
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");

        let bindings = load_tddd_layers_from_path(&rules_path, dir.path()).unwrap();

        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].layer_id(), "domain");
        assert_eq!(bindings[0].catalogue_file(), "domain-types.json");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_tddd_layers_from_path_broken_symlink_fails_closed_not_legacy_fallback() {
        // Regression: a dangling `architecture-rules.json` symlink must NOT
        // silently degrade to the legacy synthetic domain-only binding. That
        // silent degradation would skip non-domain rendered view updates on
        // misconfigured workspaces. `reject_symlinks_below` guarantees the
        // symlink is rejected before any read is attempted.
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");
        let missing_target = dir.path().join("does-not-exist.json");
        std::os::unix::fs::symlink(&missing_target, &rules_path).unwrap();

        let err = load_tddd_layers_from_path(&rules_path, dir.path()).unwrap_err();

        match err {
            LoadTdddLayersError::Io { .. } => {} // expected
            other => panic!("expected Io error for broken symlink, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_load_tddd_layers_from_path_valid_symlink_fails_closed() {
        // Even a VALID symlink to a real architecture-rules.json must be
        // rejected by `reject_symlinks_below`. The symlink-rejection policy
        // is unconditional at the leaf path; consumers must resolve any
        // intended indirection in the caller (e.g., via the trusted composition
        // root) and pass a regular file to the helper.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-rules.json");
        let link = dir.path().join("architecture-rules.json");
        let json = r#"{
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } }
          ]
        }"#;
        std::fs::write(&real, json).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = load_tddd_layers_from_path(&link, dir.path()).unwrap_err();

        match err {
            LoadTdddLayersError::Io { .. } => {} // expected — symlink rejected
            other => panic!("expected Io error for symlink leaf, got {other:?}"),
        }
    }
}
