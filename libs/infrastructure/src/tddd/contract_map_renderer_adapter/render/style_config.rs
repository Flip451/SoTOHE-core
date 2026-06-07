//! TOML style-config DTOs and edge/node rendering helpers.
//!
//! All items are `pub(super)` — implementation details of the render module.

use std::collections::BTreeMap;

use serde::Deserialize;

use domain::tddd::ContractMapRendererError;

// Re-export shared mermaid style DTOs and rendering helpers from the common module so
// that callers (`render/mod.rs`, `emit.rs`) can continue to import them via
// `style_config::{apply_shape, class_def_line, ...}` without change.
pub(crate) use crate::tddd::mermaid_style::{
    ClassStyle, EdgeStyle, NodeStyle, PatternStyle, apply_shape, class_def_line,
};

// ---------------------------------------------------------------------------
// Private TOML schema DTOs (Decision P-3 / CN-11 / Decision L-1)
// ---------------------------------------------------------------------------

/// Top-level structure for `.harness/config/contract-map-style.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StyleConfig {
    #[serde(default)]
    pub(crate) role: BTreeMap<String, RoleStyle>,
    #[serde(default)]
    pub(crate) node: BTreeMap<String, NodeStyle>,
    #[serde(default)]
    pub(crate) pattern: BTreeMap<String, PatternStyle>,
    #[serde(default)]
    pub(crate) class: BTreeMap<String, ClassStyle>,
    #[serde(default)]
    pub(crate) edge: BTreeMap<String, EdgeStyle>,
    // [filter] is structurally read on deserialization but its fields are not yet
    // used for filtering logic (I-1 reserve: all FunctionEntries are rendered).
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) filter: FilterConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoleStyle {
    pub(crate) class: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FilterConfig {
    // Future extension point for role-based function filtering (I-1 reserve).
    // Not used in current implementation — all FunctionEntries are rendered.
    #[allow(dead_code)]
    #[serde(default)]
    include_function_roles: Vec<String>,
}

// ---------------------------------------------------------------------------
// Style-related rendering helpers
// ---------------------------------------------------------------------------

/// Resolve an `EdgeStyle` to `(arrow, label_option)`.
///
/// Returns `Ok((arrow, label))` when the edge key is present in the style map.
/// Returns `Err(ContractMapRendererError::RenderFailed)` when the key is absent —
/// fail-closed per CN-02 (no code-internal hard-coded fallback or code default).
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when `key` is not found in
/// `style_map`. The style config is required to define all edge kinds that the
/// renderer uses (CN-02 / AC-11 — no hard-coded styling in code).
pub(crate) fn edge_arrow_label<'a>(
    style_map: &'a BTreeMap<String, EdgeStyle>,
    key: &str,
) -> Result<(&'a str, Option<&'a str>), ContractMapRendererError> {
    match style_map.get(key) {
        Some(es) => Ok((es.arrow.as_str(), es.label.as_deref())),
        None => Err(ContractMapRendererError::RenderFailed {
            reason: format!(
                "missing edge style configuration: [edge.{key}] not found in style config (CN-02)"
            ),
        }),
    }
}

/// Format an edge line: `source arrow[|label|] target`.
pub(crate) fn edge_line(source: &str, arrow: &str, label: Option<&str>, target: &str) -> String {
    match label {
        Some(l) => format!("{source} {arrow}|{l}| {target}"),
        None => format!("{source} {arrow} {target}"),
    }
}
