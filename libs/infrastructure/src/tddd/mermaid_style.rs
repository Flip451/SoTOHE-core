//! Shared mermaid classDef, node-shape, and style-DTO helpers.
//!
//! Both `baseline_graph_renderer_adapter` and `contract_map_renderer_adapter`
//! require identical `ClassStyle`, `NodeStyle`, `PatternStyle`, `EdgeStyle`
//! deserialization and `classDef` / `apply_shape` rendering logic.  This module
//! is the single source of truth for those shared behaviors so that future
//! changes (e.g., adding a new CSS field or edge label format) only require one
//! edit.

use serde::Deserialize;

/// `[class.<ClassName>]` — mermaid `classDef` CSS-like parameters.
///
/// Fields are intentionally private — callers use [`class_def_line`] to render
/// the classDef string rather than accessing individual style fields directly.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClassStyle {
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    stroke: Option<String>,
    #[serde(default)]
    stroke_width: Option<String>,
    #[serde(default)]
    stroke_dasharray: Option<String>,
}

/// `[node.<NodeCategory>]` — shape template and optional class name for a node category.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodeStyle {
    /// Optional mermaid shape template (e.g. `"([{label}])"` for stadium shape).
    /// When absent the default mermaid rectangular shape is used.
    #[serde(default)]
    pub(crate) shape: Option<String>,
    /// Optional classDef name to apply to nodes of this category.
    #[serde(default)]
    pub(crate) class: Option<String>,
}

/// `[pattern.<PatternName>]` — overlay class for a structural pattern.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternStyle {
    /// classDef name used as an overlay for this pattern.
    pub(crate) overlay_class: String,
}

/// `[edge.<EdgeKind>]` — arrow syntax and optional label for an edge kind.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EdgeStyle {
    pub(crate) arrow: String,
    #[serde(default)]
    pub(crate) label: Option<String>,
}

/// Format a mermaid `classDef` line from a [`ClassStyle`].
///
/// Produces `classDef <name> fill:…,stroke:…,…` when at least one field is
/// present, or `classDef <name>` when all optional fields are absent.
pub(crate) fn class_def_line(name: &str, style: &ClassStyle) -> String {
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

/// Apply a node shape template to a node label.
///
/// When `shape` is `Some(template)`, replaces the `{label}` placeholder in
/// `template` with `label`.  When `shape` is `None`, wraps `label` in the
/// default mermaid rectangular brackets `[label]`.
pub(crate) fn apply_shape(label: &str, shape: Option<&str>) -> String {
    match shape {
        Some(s) => s.replace("{label}", label),
        None => format!("[{label}]"),
    }
}
