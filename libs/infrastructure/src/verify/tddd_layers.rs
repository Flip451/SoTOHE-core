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

/// Returns the absolute path to this layer's catalogue-spec signals file
/// inside the given track:
/// `<workspace_root>/track/items/<track_id>/<layer_id>-catalogue-spec-signals.json`.
///
/// Takes `workspace_root` directly (not pre-computed `items_dir`) so the entire
/// `track/items/<track_id>/<file>` layout stays behind the infrastructure boundary —
/// raw track-id strings are validated via [`domain::TrackId`] before path construction.
///
/// Free function (not a method) so it does not extend the `TdddLayerBinding`
/// public API surface scanned by TDDD type-signals — the existing catalogue
/// entry stays untouched. Per ADR
/// `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` §D8-2.
#[must_use]
pub fn catalogue_spec_signals_path(
    binding: &TdddLayerBinding,
    workspace_root: &Path,
    track_id: &str,
) -> PathBuf {
    track_items_dir(workspace_root, track_id).join(binding.catalogue_spec_signal_file())
}

/// Returns the absolute path to this layer's impl-catalog (`signal_file`)
/// signals file inside the given track:
/// `<workspace_root>/track/items/<track_id>/<signal_file()>`.
///
/// Same purpose as [`catalogue_spec_signals_path`] but for chain ③
/// (impl ↔ catalog) signals.
#[must_use]
pub fn impl_catalog_signals_path(
    binding: &TdddLayerBinding,
    workspace_root: &Path,
    track_id: &str,
) -> PathBuf {
    track_items_dir(workspace_root, track_id).join(binding.signal_file())
}

fn track_items_dir(workspace_root: &Path, track_id: &str) -> PathBuf {
    match domain::TrackId::try_new(track_id.to_owned()) {
        Ok(valid_track_id) => {
            let track_id_component: &str = valid_track_id.as_ref();
            workspace_root.join("track").join("items").join(track_id_component)
        }
        Err(_) => invalid_track_items_dir(workspace_root),
    }
}

fn invalid_track_items_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("track").join("items").join(".invalid-track-id")
}

/// Loads TDDD layer bindings from `<workspace_root>/architecture-rules.json`.
///
/// Wraps [`load_tddd_layers`] so callers only hold `workspace_root` and never
/// need to know the `architecture-rules.json` filename — the convention stays
/// behind the infrastructure boundary. Per ADR §D8-2.
pub fn load_tddd_layers_from_workspace(
    workspace_root: &Path,
) -> Result<Vec<TdddLayerBinding>, LoadTdddLayersError> {
    let rules_path = workspace_root.join("architecture-rules.json");
    load_tddd_layers(&rules_path, workspace_root)
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
    validate_architecture_rules_version(json).map_err(TdddLayerParseError::Json)?;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Root {
        #[allow(dead_code)]
        version: u32,
        // These top-level blocks are owned by other architecture checks. Keep
        // them explicit so this decoder rejects misspelled top-level keys
        // without duplicating parsers for configuration it does not consume.
        #[serde(default)]
        #[allow(dead_code)]
        module_limits: Option<serde_json::Value>,
        #[serde(default)]
        #[allow(dead_code)]
        canonical_modules: Option<serde_json::Value>,
        #[serde(default)]
        #[allow(dead_code)]
        extra_dirs: Option<serde_json::Value>,
        layers: Vec<Layer>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Layer {
        #[serde(rename = "crate")]
        crate_name: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        may_depend_on: Vec<String>,
        #[serde(default)]
        deny_reason: Option<String>,
        #[serde(default)]
        verify: Option<serde_json::Value>,
        #[serde(default)]
        tddd: Option<TdddBlock>,
    }

    #[derive(Deserialize, Default)]
    #[serde(deny_unknown_fields)]
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
    #[serde(deny_unknown_fields)]
    struct CatalogueSpecSignalBlock {
        #[serde(default)]
        enabled: bool,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct SchemaExportBlock {
        #[serde(default)]
        targets: Vec<String>,
        #[serde(default)]
        method: Option<String>,
        #[serde(default)]
        notes: Option<String>,
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

/// Parses all crate names listed in the `layers[]` array of an
/// `architecture-rules.json` content string, regardless of whether each layer
/// has TDDD enabled.
///
/// The returned `HashSet` contains every `layers[].crate` value and is
/// suitable for use as the `workspace_crates` argument passed to
/// `domain::check_consistency` so that the IN-10 reverse checks (workspace-
/// origin trait filter for Interactor / SecondaryAdapter) are active in the
/// verify and signal-evaluation paths.
///
/// # Errors
///
/// Returns `serde_json::Error` when `json` is not valid JSON or does not
/// contain the mandatory `layers` key.
pub fn parse_workspace_crate_names(
    json: &str,
) -> Result<std::collections::HashSet<String>, serde_json::Error> {
    validate_architecture_rules_version(json)?;

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Root {
        #[allow(dead_code)]
        version: u32,
        // These blocks are intentionally not interpreted by the workspace
        // crate-name reader, but must still be named to fail closed on a
        // misspelled top-level architecture-rules key.
        #[serde(default)]
        #[allow(dead_code)]
        module_limits: Option<serde_json::Value>,
        #[serde(default)]
        #[allow(dead_code)]
        canonical_modules: Option<serde_json::Value>,
        #[serde(default)]
        #[allow(dead_code)]
        extra_dirs: Option<serde_json::Value>,
        layers: Vec<Layer>,
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Layer {
        #[serde(rename = "crate")]
        crate_name: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        may_depend_on: Vec<String>,
        #[serde(default)]
        deny_reason: Option<String>,
        #[serde(default)]
        verify: Option<serde_json::Value>,
        #[serde(default)]
        tddd: Option<serde_json::Value>,
    }

    let root: Root = serde_json::from_str(json)?;
    if root.layers.is_empty() {
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "architecture-rules.json must define a non-empty 'layers' array",
        ));
    }
    Ok(root.layers.into_iter().map(|l| l.crate_name).collect())
}

fn validate_architecture_rules_version(json: &str) -> Result<(), serde_json::Error> {
    #[derive(Deserialize)]
    struct VersionEnvelope {
        version: u32,
    }

    let envelope: VersionEnvelope = serde_json::from_str(json)?;
    if envelope.version != 2 {
        return Err(<serde_json::Error as serde::de::Error>::custom(format!(
            "architecture-rules.json version {} is not supported (expected 2)",
            envelope.version
        )));
    }
    Ok(())
}

/// Loads workspace crate names from `architecture-rules.json` at `path`.
///
/// Fails closed when the file does not exist: without the configuration the
/// IN-10 reverse checks cannot be trusted. Any I/O error or JSON parse failure
/// is returned as `Err`.
///
/// # Errors
///
/// Returns [`LoadTdddLayersError::Io`] for I/O failures and
/// [`LoadTdddLayersError::Parse`] for JSON errors.
pub fn load_workspace_crate_names(
    path: &Path,
    trusted_root: &Path,
) -> Result<std::collections::HashSet<String>, LoadTdddLayersError> {
    let (path, trusted_root) = confined_rules_path(path, trusted_root)
        .map_err(|source| LoadTdddLayersError::Io { path: path.to_path_buf(), source })?;
    match crate::track::symlink_guard::reject_symlinks_below(&path, &trusted_root) {
        Ok(true) => {
            let content = crate::capability_exec::bounded_read_utf8_file(&path)
                .map_err(|e| LoadTdddLayersError::Io { path: path.to_path_buf(), source: e })?;
            parse_workspace_crate_names(&content)
                .map_err(|error| LoadTdddLayersError::Parse(TdddLayerParseError::Json(error)))
        }
        Ok(false) => Err(LoadTdddLayersError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "architecture-rules.json not found; workspace crates cannot be determined",
            ),
        }),
        Err(e) => Err(LoadTdddLayersError::Io { path: path.to_path_buf(), source: e }),
    }
}

/// Error returned by [`load_tddd_layers`].
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

/// Loads TDDD layer bindings from `architecture-rules.json` at `path`,
/// failing closed when the file is absent (no synthetic fallback).
///
/// An absent `architecture-rules.json` is treated as a hard configuration
/// error: the set of catalogues to check is unknowable without an explicit
/// rules file, so silently passing would be a fail-open regression.
///
/// `trusted_root` is passed through to `reject_symlinks_below` — only
/// components below it are inspected.
///
/// # Errors
///
/// Returns [`LoadTdddLayersError::Io`] when the file is absent, when the
/// symlink guard rejects the path, or on any other I/O failure. Returns
/// [`LoadTdddLayersError::Parse`] when the JSON is invalid or violates any
/// constraint enforced by [`parse_tddd_layers`].
pub fn load_tddd_layers(
    path: &Path,
    trusted_root: &Path,
) -> Result<Vec<TdddLayerBinding>, LoadTdddLayersError> {
    let (path, trusted_root) = confined_rules_path(path, trusted_root)
        .map_err(|source| LoadTdddLayersError::Io { path: path.to_path_buf(), source })?;
    match crate::track::symlink_guard::reject_symlinks_below(&path, &trusted_root) {
        Ok(true) => {
            let content = crate::capability_exec::bounded_read_utf8_file(&path)
                .map_err(|e| LoadTdddLayersError::Io { path: path.to_path_buf(), source: e })?;
            parse_tddd_layers(&content).map_err(LoadTdddLayersError::Parse)
        }
        Ok(false) => {
            // File genuinely absent — fail closed.
            Err(LoadTdddLayersError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "architecture-rules.json not found; TDDD layer bindings cannot be \
                     determined without an explicit rules file",
                ),
            })
        }
        Err(e) => Err(LoadTdddLayersError::Io { path: path.to_path_buf(), source: e }),
    }
}

/// Resolves an architecture-rules path under the canonical workspace root
/// without following a candidate symlink for the actual read. This prevents a
/// caller from selecting an unrelated rules file before the shared component
/// symlink guard runs.
fn confined_rules_path(
    path: &Path,
    trusted_root: &Path,
) -> Result<(PathBuf, PathBuf), std::io::Error> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(trusted_root)?;
    let canonical_root = trusted_root.canonicalize().map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("failed to canonicalize trusted root {}: {error}", trusted_root.display()),
        )
    })?;
    let absolute_path =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let lexical_path = crate::verify::path_safety::lexical_normalize(&absolute_path);
    if !lexical_path.starts_with(&canonical_root) {
        return Err(path_outside_trusted_root(&lexical_path, &canonical_root));
    }
    match lexical_path.canonicalize() {
        Ok(canonical_path) if canonical_path.starts_with(&canonical_root) => {}
        Ok(canonical_path) => {
            return Err(path_outside_trusted_root(&canonical_path, &canonical_root));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok((lexical_path, canonical_root))
}

fn path_outside_trusted_root(path: &Path, trusted_root: &Path) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("path {} is outside trusted root {}", path.display(), trusted_root.display()),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tddd_layers_enabled_only_returned() {
        let json = r#"{
          "version": 2,
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
          "version": 2,
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
          "version": 2,
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
    fn test_catalogue_spec_signals_path_uses_validated_track_id_under_workspace() {
        let binding = binding_with_layer_id("domain", "domain-types.json");
        let workspace_root = PathBuf::from("/workspace");

        let path = catalogue_spec_signals_path(&binding, &workspace_root, "safe-track-2026");

        assert_eq!(
            path,
            PathBuf::from(
                "/workspace/track/items/safe-track-2026/domain-catalogue-spec-signals.json"
            )
        );
    }

    #[test]
    fn test_impl_catalog_signals_path_uses_validated_track_id_under_workspace() {
        let binding = binding_with_layer_id("domain", "domain-types.json");
        let workspace_root = PathBuf::from("/workspace");

        let path = impl_catalog_signals_path(&binding, &workspace_root, "safe-track-2026");

        assert_eq!(
            path,
            PathBuf::from("/workspace/track/items/safe-track-2026/domain-type-signals.json")
        );
    }

    #[test]
    fn test_signal_path_helpers_reject_raw_track_id_escape_before_construction() {
        assert!(domain::TrackId::try_new("../escape").is_err());
        assert!(domain::TrackId::try_new("/tmp/escape").is_err());

        let binding = binding_with_layer_id("domain", "domain-types.json");
        let workspace_root = PathBuf::from("/workspace");
        let items_dir = workspace_root.join("track").join("items");

        let parent_escape = catalogue_spec_signals_path(&binding, &workspace_root, "../escape");
        assert_eq!(
            parent_escape,
            items_dir.join(".invalid-track-id").join("domain-catalogue-spec-signals.json")
        );

        let absolute_escape = impl_catalog_signals_path(&binding, &workspace_root, "/tmp/escape");
        assert_eq!(
            absolute_escape,
            items_dir.join(".invalid-track-id").join("domain-type-signals.json")
        );
    }

    #[test]
    fn test_parse_tddd_layers_default_targets_is_crate_name() {
        let json = r#"{
          "version": 2,
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
          "version": 2,
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
          "version": 2,
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
        let json = r#"{ "version": 2, "layers": [] }"#;
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
    fn test_parse_tddd_layers_rejects_unknown_top_level_configuration() {
        let json = r#"{
          "version": 2,
          "layers": [{ "crate": "domain", "tddd": { "enabled": true } }],
          "modul_limits": { "max_lines": 700 }
        }"#;

        assert!(matches!(parse_tddd_layers(json), Err(TdddLayerParseError::Json(_))));
    }

    #[test]
    fn test_parse_tddd_layers_rejects_misspelled_catalogue_spec_signal_field() {
        let json = r#"{
          "version": 2,
          "layers": [{
            "crate": "domain",
            "tddd": {
              "enabled": true,
              "catalogue_spec_signal": { "enabld": true }
            }
          }]
        }"#;

        assert!(matches!(parse_tddd_layers(json), Err(TdddLayerParseError::Json(_))));
    }

    // --- catalogue_spec_signal_enabled() accessor (T018) ---

    #[test]
    fn test_parse_tddd_layers_absent_catalogue_spec_signal_defaults_to_false() {
        // When `tddd.catalogue_spec_signal` is omitted, the accessor must return
        // `false` (opt-in semantics per ADR §D5.4).
        let json = r#"{
          "version": 2,
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
          "version": 2,
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
          "version": 2,
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
    fn test_load_tddd_layers_regular_file_returns_parsed_bindings() {
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");
        let json = r#"{
          "version": 2,
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            { "crate": "usecase", "tddd": { "enabled": true, "catalogue_file": "usecase-types.json" } }
          ]
        }"#;
        std::fs::write(&rules_path, json).unwrap();

        let bindings = load_tddd_layers(&rules_path, dir.path()).unwrap();

        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].catalogue_file(), "domain-types.json");
        assert_eq!(bindings[1].catalogue_file(), "usecase-types.json");
    }

    #[test]
    fn test_load_tddd_layers_oversized_rules_returns_typed_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");
        std::fs::File::create(&rules_path)
            .unwrap()
            .set_len(crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)
            .unwrap();

        let err = load_tddd_layers(&rules_path, dir.path()).unwrap_err();

        match err {
            LoadTdddLayersError::Io { source, .. } => {
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("expected typed Io error for oversized rules, got {other:?}"),
        }
    }

    #[test]
    fn test_load_tddd_layers_missing_file_fails_closed() {
        // An absent architecture-rules.json is a hard error — no synthetic fallback.
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");

        let err = load_tddd_layers(&rules_path, dir.path()).unwrap_err();
        assert!(
            matches!(err, LoadTdddLayersError::Io { .. }),
            "expected Io error for absent architecture-rules.json, got: {err:?}"
        );
    }

    #[test]
    fn test_load_tddd_layers_outside_trusted_root_fails_closed() {
        let trusted_root = tempfile::tempdir().unwrap();
        let outside_root = tempfile::tempdir().unwrap();
        let rules_path = outside_root.path().join("architecture-rules.json");
        std::fs::write(&rules_path, r#"{"layers":[{"crate":"domain"}]}"#).unwrap();

        assert!(matches!(
            load_tddd_layers(&rules_path, trusted_root.path()),
            Err(LoadTdddLayersError::Io { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_tddd_layers_broken_symlink_fails_closed() {
        // A dangling symlink must fail closed. `reject_symlinks_below` guarantees
        // the symlink is rejected before any read is attempted.
        let dir = tempfile::tempdir().unwrap();
        let rules_path = dir.path().join("architecture-rules.json");
        let missing_target = dir.path().join("does-not-exist.json");
        std::os::unix::fs::symlink(&missing_target, &rules_path).unwrap();

        let err = load_tddd_layers(&rules_path, dir.path()).unwrap_err();

        match err {
            LoadTdddLayersError::Io { .. } => {} // expected
            other => panic!("expected Io error for broken symlink, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_load_tddd_layers_valid_symlink_fails_closed() {
        // Even a VALID symlink to a real architecture-rules.json must be
        // rejected by `reject_symlinks_below`. The symlink-rejection policy
        // is unconditional at the leaf path.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-rules.json");
        let link = dir.path().join("architecture-rules.json");
        let json = r#"{
          "version": 2,
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } }
          ]
        }"#;
        std::fs::write(&real, json).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = load_tddd_layers(&link, dir.path()).unwrap_err();

        match err {
            LoadTdddLayersError::Io { .. } => {} // expected — symlink rejected
            other => panic!("expected Io error for symlink leaf, got {other:?}"),
        }
    }

    // --- parse_workspace_crate_names() (Finding 2 / IN-10 fix) ---

    #[test]
    fn test_parse_workspace_crate_names_returns_all_layer_crates_regardless_of_tddd() {
        // All layers must be returned — both tddd-enabled and disabled — because
        // the set is used for IN-10 workspace-origin filtering, not TDDD routing.
        let json = r#"{
          "version": 2,
          "layers": [
            { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
            { "crate": "usecase", "tddd": { "enabled": false } },
            { "crate": "infrastructure" }
          ]
        }"#;
        let names = parse_workspace_crate_names(json).unwrap();
        assert!(names.contains("domain"), "domain should be present");
        assert!(names.contains("usecase"), "usecase should be present");
        assert!(names.contains("infrastructure"), "infrastructure should be present");
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_parse_workspace_crate_names_empty_layers_fails_closed() {
        let json = r#"{ "version": 2, "layers": [] }"#;
        assert!(parse_workspace_crate_names(json).is_err());
    }

    #[test]
    fn test_parse_workspace_crate_names_absent_layers_key_fails_closed() {
        let json = r#"{}"#;
        assert!(parse_workspace_crate_names(json).is_err());
    }

    #[test]
    fn test_parse_workspace_crate_names_invalid_json_returns_error() {
        let result = parse_workspace_crate_names("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_workspace_crate_names_rejects_unknown_top_level_configuration() {
        let json = r#"{
          "version": 2,
          "layers": [{ "crate": "domain" }],
          "layerz": []
        }"#;

        assert!(parse_workspace_crate_names(json).is_err());
    }

    // --- load_workspace_crate_names() (Finding 2 / IN-10 fix) ---

    #[test]
    fn test_load_workspace_crate_names_missing_file_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("architecture-rules.json");

        assert!(matches!(
            load_workspace_crate_names(&path, dir.path()),
            Err(LoadTdddLayersError::Io { .. })
        ));
    }

    #[test]
    fn test_load_workspace_crate_names_outside_trusted_root_fails_closed() {
        let trusted_root = tempfile::tempdir().unwrap();
        let outside_root = tempfile::tempdir().unwrap();
        let rules_path = outside_root.path().join("architecture-rules.json");
        std::fs::write(&rules_path, r#"{"layers":[{"crate":"domain"}]}"#).unwrap();

        assert!(matches!(
            load_workspace_crate_names(&rules_path, trusted_root.path()),
            Err(LoadTdddLayersError::Io { .. })
        ));
    }

    #[test]
    fn test_load_workspace_crate_names_returns_names_from_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("architecture-rules.json");
        let json = r#"{
          "version": 2,
          "layers": [
            { "crate": "domain" },
            { "crate": "usecase", "tddd": { "enabled": false } },
            { "crate": "infrastructure", "tddd": { "enabled": true } }
          ]
        }"#;
        std::fs::write(&path, json).unwrap();

        let names = load_workspace_crate_names(&path, dir.path()).unwrap();
        assert!(names.contains("domain"));
        assert!(names.contains("usecase"));
        assert!(names.contains("infrastructure"));
        assert_eq!(names.len(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn test_load_workspace_crate_names_broken_symlink_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("architecture-rules.json");
        let missing_target = dir.path().join("does-not-exist.json");
        std::os::unix::fs::symlink(&missing_target, &path).unwrap();

        let err = load_workspace_crate_names(&path, dir.path()).unwrap_err();
        match err {
            LoadTdddLayersError::Io { .. } => {} // expected — symlink rejected
            other => panic!("expected Io error for broken symlink, got {other:?}"),
        }
    }
}
