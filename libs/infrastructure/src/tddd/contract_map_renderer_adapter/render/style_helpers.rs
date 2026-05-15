//! Style config helper functions for the T006/T007 mermaid renderer.
//!
//! Provides accessors for role class names, node class names, node shapes,
//! and classDef line collection from the style config TOML.

use super::super::{ClassStyle, NodeStyle, StyleConfig};

// ---------------------------------------------------------------------------
// Role and node class name helpers
// ---------------------------------------------------------------------------

/// Returns the classDef class name for a role string (e.g. `"ValueObject"`).
///
/// Looks up `style.role[role_str].class`. Falls back to the lowercase role
/// string if the role is not configured (should not happen with a well-formed
/// config file).
pub(super) fn role_class_name(role_str: &str, style: &StyleConfig) -> String {
    style.role.get(role_str).map(|r| r.class.clone()).unwrap_or_else(|| role_str.to_lowercase())
}

/// Returns the classDef class name for a node category (e.g. `"Method"`, `"Function"`).
///
/// Looks up `style.node[category].class`. Falls back to the lowercase category
/// if not configured.
pub(super) fn node_class_name(category: &str, style: &StyleConfig) -> String {
    style
        .node
        .get(category)
        .map(|n: &NodeStyle| n.class.clone())
        .unwrap_or_else(|| category.to_lowercase())
}

/// Returns the mermaid shape string for a node category (e.g. `"Method"`, `"Function"`).
///
/// Looks up `style.node[category].shape`. Falls back to `"round"` if the category
/// is not configured, so that unconfigured nodes still produce valid Mermaid output.
pub(super) fn node_shape(category: &str, style: &StyleConfig) -> String {
    style
        .node
        .get(category)
        .map(|n: &NodeStyle| n.shape.clone())
        .unwrap_or_else(|| "round".to_owned())
}

// ---------------------------------------------------------------------------
// classDef line collection
// ---------------------------------------------------------------------------

/// Collects `classDef` lines from the style config in alphabetical order by class name.
///
/// Alphabetical ordering ensures deterministic renderer output across runs
/// regardless of `HashMap` iteration order. T008 may enforce a different canonical
/// ordering; for T006/T007 alphabetical is a stable baseline.
pub(super) fn collect_classdefs(style: &StyleConfig) -> Vec<String> {
    let mut entries: Vec<(&String, &ClassStyle)> = style.class.iter().collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    entries
        .into_iter()
        .map(|(name, cs)| {
            format!(
                "classDef {name} fill:{},stroke:{},stroke-width:{},stroke-dasharray:{}",
                cs.fill, cs.stroke, cs.stroke_width, cs.stroke_dasharray
            )
        })
        .collect()
}
