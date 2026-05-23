//! TOML schema DTOs and mermaid rendering helpers for the baseline-graph renderer.
//!
//! All items are `pub(super)` — implementation details of the render module.
//! This module is symmetric to `ContractMapRendererAdapter render::StyleConfig`.
//!
//! Section structure: `[node.*]` + `[pattern.*]` + `[class.*]` + `[edge.*]` + `[filter]`.
//! `[role.*]` is intentionally ABSENT — Reality View input is `rustdoc_types::Crate`
//! which carries no catalogue role data (ADR 2026-05-22-1507 Decision C / IN-04).

use std::collections::BTreeMap;

use serde::Deserialize;

use domain::tddd::baseline_graph_ports::BaselineGraphRendererError;

// ---------------------------------------------------------------------------
// Private TOML schema DTOs
// ---------------------------------------------------------------------------

/// Top-level structure for `.harness/config/baseline-graph-style.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::tddd::baseline_graph_renderer_adapter) struct StyleConfig {
    /// `[node.<NodeCategory>]` — shape + class for a node category (Method, Variant, Function).
    /// Used in T005-T010 for node shape/class rendering.
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) node: BTreeMap<String, NodeStyle>,
    /// `[pattern.<PatternName>]` — overlay_class for structural patterns (future extension).
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) pattern: BTreeMap<String, PatternStyle>,
    /// `[class.<ClassName>]` — mermaid classDef parameters.
    #[serde(default)]
    pub(super) class: BTreeMap<String, ClassStyle>,
    /// `[edge.<EdgeKind>]` — arrow syntax + optional label. Used in T005-T010 for edge rendering.
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) edge: BTreeMap<String, EdgeStyle>,
    /// `[filter]` — future extension point (I decision reserve).
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) filter: FilterConfig,
}

/// `[node.<NodeCategory>]` — shape template + class name for a node category.
///
/// Used in T005-T010 for applying node shapes and class names to rendered nodes.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeStyle {
    /// Optional mermaid shape template (e.g. `"([{label}])"` for stadium shape).
    /// When absent the default mermaid rectangular shape is used.
    #[serde(default)]
    pub(super) shape: Option<String>,
    /// Optional classDef name to apply to nodes of this category.
    #[serde(default)]
    pub(super) class: Option<String>,
}

/// `[pattern.<PatternName>]` — overlay class for a structural pattern (future).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatternStyle {
    /// classDef name used as an overlay for this pattern.
    pub(super) overlay_class: String,
}

/// `[class.<ClassName>]` — mermaid `classDef` CSS-like parameters.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClassStyle {
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    stroke: Option<String>,
    #[serde(default)]
    stroke_width: Option<String>,
    #[serde(default)]
    stroke_dasharray: Option<String>,
}

/// `[edge.<EdgeKind>]` — arrow syntax and optional label for an edge kind.
///
/// Used in T005-T010 for edge rendering (trait impl, variant payload, field, alias, etc.).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EdgeStyle {
    pub(super) arrow: String,
    #[serde(default)]
    pub(super) label: Option<String>,
}

/// `[filter]` — future rendering filter configuration (I decision reserve).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FilterConfig {
    /// Whether to include all public functions (default: true, all rendered).
    #[allow(dead_code)]
    #[serde(default = "default_include_functions")]
    include_functions: bool,
}

fn default_include_functions() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Rendering helpers (symmetric to ContractMapRendererAdapter render helpers)
// ---------------------------------------------------------------------------

/// Sanitize a string for use as a mermaid node_id segment.
///
/// Replaces every character that is not ASCII alphanumeric or underscore with `_`.
pub(super) fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

/// Format a mermaid `classDef` line from a `ClassStyle`.
pub(super) fn class_def_line(name: &str, style: &ClassStyle) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref fill) = style.fill {
        parts.push(format!("fill:{fill}"));
    }
    if let Some(ref stroke) = style.stroke {
        parts.push(format!("stroke:{stroke}"));
    }
    if let Some(ref sw) = style.stroke_width {
        parts.push(format!("stroke-width:{sw}"));
    }
    if let Some(ref sd) = style.stroke_dasharray {
        parts.push(format!("stroke-dasharray:{sd}"));
    }
    if parts.is_empty() {
        format!("classDef {name}")
    } else {
        format!("classDef {name} {}", parts.join(","))
    }
}

/// Apply a node shape template from a `NodeStyle` to a node label.
///
/// Used in T005-T010 for rendering nodes with configurable shapes.
#[allow(dead_code)]
pub(super) fn apply_shape(label: &str, shape: Option<&str>) -> String {
    match shape {
        Some(s) => s.replace("{label}", label),
        None => format!("[{label}]"),
    }
}

/// Resolve an `EdgeStyle` to `(arrow, label_option)`.
///
/// Returns `Ok((arrow, label))` when the edge key is present in the style map.
/// Returns `Err(BaselineGraphRendererError::RenderFailed)` when the key is absent —
/// fail-closed per CN-02 (no code-internal hard-coded fallback).
///
/// Used in T005-T010 for fail-closed edge style resolution.
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when `key` is not found in
/// `style_map`. The style config is required to define all edge kinds that the
/// renderer uses (CN-02 — no hard-coded styling in code).
#[allow(dead_code)]
pub(super) fn edge_arrow_label<'a>(
    style_map: &'a BTreeMap<String, EdgeStyle>,
    key: &str,
) -> Result<(&'a str, Option<&'a str>), BaselineGraphRendererError> {
    match style_map.get(key) {
        Some(es) => Ok((es.arrow.as_str(), es.label.as_deref())),
        None => Err(BaselineGraphRendererError::RenderFailed {
            reason: format!(
                "missing edge style configuration: [edge.{key}] not found in baseline-graph style config (CN-02)"
            ),
        }),
    }
}

/// Generate a subgraph id for a layer.
pub(super) fn layer_subgraph_id(layer: &str) -> String {
    sanitize(layer)
}
