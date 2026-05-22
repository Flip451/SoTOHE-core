//! Mermaid rendering internals for the baseline-graph renderer (T004–T010).
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `BaselineGraphRendererAdapter` and must not appear in the infrastructure
//! crate's public API (Decision P-3 / CN-11, symmetric to ContractMapRendererAdapter).
//!
//! **Scope (T004)**: private TOML schema DTOs + style config loading + skeleton
//! mermaid output (classDef block + layer subgraph frame). Full node/edge rendering
//! will be added in T005–T010.

use std::collections::BTreeMap;

use serde::Deserialize;

use domain::tddd::baseline_document::BaselineDocument;
use domain::tddd::baseline_graph_ports::BaselineGraphRendererError;
use domain::tddd::layer_id::LayerId;

// ---------------------------------------------------------------------------
// Private TOML schema DTOs (symmetric to ContractMapRendererAdapter render::StyleConfig)
//
// Section structure: [node.*] + [pattern.*] + [class.*] + [edge.*] + [filter].
// [role.*] is intentionally ABSENT — Reality View input is rustdoc_types::Crate
// which carries no catalogue role data (ADR 2026-05-22-1507 Decision C / IN-04).
// ---------------------------------------------------------------------------

/// Top-level structure for `.harness/config/baseline-graph-style.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StyleConfig {
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
fn class_def_line(name: &str, style: &ClassStyle) -> String {
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
fn layer_subgraph_id(layer: &str) -> String {
    sanitize(layer)
}

// ---------------------------------------------------------------------------
// T004: skeleton mermaid render (overview + clusters scaffold)
//
// render_overview and render_clusters return minimal valid mermaid output:
// - classDef definitions from the style config
// - layer subgraph frame (no node/edge content — T005-T010 will fill this in)
//
// todo!() / unimplemented!() are prohibited by coding rules; we return minimal
// valid mermaid strings here instead of panicking.
// ---------------------------------------------------------------------------

/// Render a depth-1 overview mermaid diagram (skeleton, T004).
///
/// T004 scope: emits classDef block + layer subgraph frame.
/// Full cluster/node/edge content will be added in T005-T009.
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` if style config is
/// structurally inconsistent (e.g. edge lookup fails in future T005-T009 code).
/// For T004, returns `Ok` as long as the style config loaded successfully.
pub(super) fn render_overview_mermaid(
    baselines: &[BaselineDocument],
    layer: &LayerId,
    style: &StyleConfig,
) -> Result<String, BaselineGraphRendererError> {
    let layer_str = layer.as_ref();
    let layer_sg_id = layer_subgraph_id(layer_str);

    // Section 1: classDef definitions (alphabetical, CN-08).
    let mut class_defs: Vec<String> = Vec::new();
    for (class_name, class_style) in &style.class {
        class_defs.push(class_def_line(class_name, class_style));
    }

    // Section 2: layer subgraph frame (cluster nodes will be added in T009).
    let mut subgraph_lines: Vec<String> = Vec::new();
    subgraph_lines.push(format!("subgraph {layer_sg_id}[\"{layer_str}\"]"));
    subgraph_lines.push("  direction TB".to_string());
    // Count baselines for this layer (crate names rendered as comments for future T009).
    for baseline in baselines {
        if baseline.layer == *layer {
            let crate_str = sanitize(baseline.crate_name.as_str());
            // Placeholder: cluster nodes will be emitted by T009.
            // Emit a comment-style no-op placeholder so the subgraph is non-empty.
            subgraph_lines.push(format!("  %% cluster placeholder: {crate_str}"));
        }
    }
    subgraph_lines.push("end".to_string());

    // Assemble output.
    let mut lines: Vec<String> = Vec::new();
    lines.push(
        "<!-- Generated baseline-graph-renderer (overview) — DO NOT EDIT DIRECTLY -->".to_string(),
    );
    lines.push("```mermaid".to_string());
    lines.push("flowchart LR".to_string());
    for cd in &class_defs {
        lines.push(cd.clone());
    }
    for sl in &subgraph_lines {
        lines.push(sl.clone());
    }
    lines.push("```".to_string());
    lines.push(String::new()); // trailing newline

    Ok(lines.join("\n"))
}

/// Enumerate clusters in `baselines` for `layer` and render each as a skeleton
/// depth-2 cluster mermaid diagram (T004).
///
/// T004 scope: per-cluster file contains classDef block + layer subgraph frame.
/// Full entry subgraph / edge content will be added in T010.
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` if rendering fails.
/// For T004 this always succeeds as long as the style config was loaded.
pub(super) fn render_clusters_mermaid(
    baselines: &[BaselineDocument],
    layer: &LayerId,
    style: &StyleConfig,
) -> Result<Vec<(String, String)>, BaselineGraphRendererError> {
    let layer_str = layer.as_ref();

    // Enumerate clusters: (cluster_key, crate_name_str) pairs.
    // cluster_key per IN-14/IN-15: "<crate_name>_root" or "<crate_name>_<module_seg1>".
    // T004 emits one "root" cluster per crate (full cluster enumeration in T010).
    let mut cluster_keys: Vec<(String, String)> = Vec::new();
    for baseline in baselines {
        if baseline.layer != *layer {
            continue;
        }
        let crate_str = baseline.crate_name.as_str();
        // T004: emit one placeholder root cluster per crate.
        // T010 will replace this with the full krate.paths enumeration.
        let cluster_key = format!("{crate_str}_root");
        cluster_keys.push((cluster_key, crate_str.to_string()));
    }

    let mut results: Vec<(String, String)> = Vec::new();

    for (cluster_key, crate_str) in &cluster_keys {
        let layer_sg_id = layer_subgraph_id(layer_str);
        let crate_sg = sanitize(crate_str);

        // Section 1: classDef definitions (alphabetical, CN-08).
        let mut class_defs: Vec<String> = Vec::new();
        for (class_name, class_style) in &style.class {
            class_defs.push(class_def_line(class_name, class_style));
        }

        // Section 2: layer subgraph > cluster subgraph frame.
        let mut subgraph_lines: Vec<String> = Vec::new();
        subgraph_lines.push(format!("subgraph {layer_sg_id}[\"{layer_str}\"]"));
        subgraph_lines.push("  direction TB".to_string());
        subgraph_lines.push(format!("  subgraph {crate_sg}_root_sg[\"{crate_str} root\"]"));
        subgraph_lines.push("    direction TB".to_string());
        // Placeholder — entry subgraphs will be filled in by T010.
        subgraph_lines.push(format!("    %% entry placeholder: {crate_sg}"));
        subgraph_lines.push("  end".to_string());
        subgraph_lines.push("end".to_string());

        // Assemble output.
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "<!-- Generated baseline-graph-renderer (cluster: {cluster_key}) — DO NOT EDIT DIRECTLY -->"
        ));
        lines.push("```mermaid".to_string());
        lines.push("flowchart LR".to_string());
        for cd in &class_defs {
            lines.push(cd.clone());
        }
        for sl in &subgraph_lines {
            lines.push(sl.clone());
        }
        lines.push("```".to_string());
        lines.push(String::new()); // trailing newline

        results.push((cluster_key.clone(), lines.join("\n")));
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests for render internals
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // sanitize
    // -----------------------------------------------------------------------

    #[test]
    fn test_sanitize_replaces_hyphens_and_colons() {
        let result = sanitize("my-crate::module");
        assert_eq!(result, "my_crate__module");
    }

    #[test]
    fn test_sanitize_preserves_alphanumeric_and_underscore() {
        let result = sanitize("domain_crate_123");
        assert_eq!(result, "domain_crate_123");
    }

    // -----------------------------------------------------------------------
    // apply_shape
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_shape_with_template() {
        let result = apply_shape("MyNode", Some("([{label}])"));
        assert_eq!(result, "([MyNode])");
    }

    #[test]
    fn test_apply_shape_without_template_uses_default_brackets() {
        let result = apply_shape("MyNode", None);
        assert_eq!(result, "[MyNode]");
    }

    // -----------------------------------------------------------------------
    // edge_arrow_label
    // -----------------------------------------------------------------------

    #[test]
    fn test_edge_arrow_label_returns_ok_for_present_key() {
        let mut map: BTreeMap<String, EdgeStyle> = BTreeMap::new();
        map.insert(
            "trait_impl".to_string(),
            EdgeStyle { arrow: "-.impl.->".to_string(), label: None },
        );
        let (arrow, label) = edge_arrow_label(&map, "trait_impl").unwrap();
        assert_eq!(arrow, "-.impl.->");
        assert!(label.is_none());
    }

    #[test]
    fn test_edge_arrow_label_returns_err_for_absent_key() {
        let map: BTreeMap<String, EdgeStyle> = BTreeMap::new();
        let err = edge_arrow_label(&map, "alias").unwrap_err();
        assert!(
            matches!(err, BaselineGraphRendererError::RenderFailed { .. }),
            "absent edge key must return RenderFailed"
        );
    }

    #[test]
    fn test_edge_arrow_label_returns_label_when_present() {
        let mut map: BTreeMap<String, EdgeStyle> = BTreeMap::new();
        map.insert(
            "alias".to_string(),
            EdgeStyle { arrow: "---".to_string(), label: Some("alias_of".to_string()) },
        );
        let (arrow, label) = edge_arrow_label(&map, "alias").unwrap();
        assert_eq!(arrow, "---");
        assert_eq!(label, Some("alias_of"));
    }

    // -----------------------------------------------------------------------
    // render_overview_mermaid: minimal valid output checks
    // -----------------------------------------------------------------------

    fn minimal_style() -> StyleConfig {
        toml::from_str::<StyleConfig>("[filter]\ninclude_functions = true\n").unwrap()
    }

    fn minimal_crate() -> rustdoc_types::Crate {
        let json = format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {},
                "target": {{"triple": "", "target_features": []}}
            }}"#,
            rustdoc_types::FORMAT_VERSION
        );
        serde_json::from_str(&json).expect("minimal_crate JSON must be valid")
    }

    fn make_baseline(layer_str: &str, crate_str: &str) -> BaselineDocument {
        use domain::tddd::baseline_document::BaselineDocument;
        use domain::tddd::catalogue_v2::identifiers::CrateName;
        use domain::tddd::layer_id::LayerId;
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_str).unwrap(),
            minimal_crate(),
        )
    }

    #[test]
    fn test_render_overview_mermaid_starts_with_header_comment() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(
            output.starts_with("<!-- Generated baseline-graph-renderer"),
            "must start with header comment; got: {:?}",
            &output[..output.len().min(80)]
        );
    }

    #[test]
    fn test_render_overview_mermaid_contains_mermaid_fence() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[], &layer, &style).unwrap();
        assert!(output.contains("```mermaid\n"), "must contain opening mermaid fence");
        assert!(output.contains("\n```\n"), "must contain closing mermaid fence");
    }

    #[test]
    fn test_render_overview_mermaid_body_starts_with_flowchart_lr() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[], &layer, &style).unwrap();
        let fence_open = "```mermaid\n";
        let fence_start = output.find(fence_open).unwrap() + fence_open.len();
        let fence_end = output[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1)
            .unwrap_or(output.len());
        let mermaid_body = &output[fence_start..fence_end];
        assert!(
            mermaid_body.starts_with("flowchart LR\n"),
            "mermaid body must start with 'flowchart LR\\n'; got: {:?}",
            &mermaid_body[..mermaid_body.len().min(40)]
        );
    }

    #[test]
    fn test_render_overview_mermaid_contains_layer_subgraph() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(output.contains("subgraph domain"), "must contain layer subgraph");
    }

    #[test]
    fn test_render_overview_mermaid_filters_to_given_layer() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline_domain = make_baseline("domain", "domain");
        let baseline_usecase = make_baseline("usecase", "usecase");
        let style = minimal_style();
        let output =
            render_overview_mermaid(&[baseline_domain, baseline_usecase], &layer, &style).unwrap();
        // Must contain a placeholder for domain crate.
        assert!(output.contains("domain"), "must mention domain");
        // Must NOT contain a placeholder for usecase (different layer).
        assert!(
            !output.contains("%% cluster placeholder: usecase"),
            "must not include usecase cluster in domain layer render"
        );
    }

    // -----------------------------------------------------------------------
    // render_clusters_mermaid: minimal valid output checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_clusters_mermaid_returns_one_root_per_crate() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 1, "T004 emits one root cluster per crate");
        assert_eq!(clusters[0].0, "domain_root");
    }

    #[test]
    fn test_render_clusters_mermaid_cluster_content_contains_mermaid_fence() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        let content = &clusters[0].1;
        assert!(content.contains("```mermaid\n"), "cluster content must have mermaid fence");
        assert!(content.contains("\n```\n"), "cluster content must have closing fence");
    }

    #[test]
    fn test_render_clusters_mermaid_empty_baselines_returns_empty() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[], &layer, &style).unwrap();
        assert!(clusters.is_empty(), "no baselines → no clusters");
    }

    #[test]
    fn test_render_clusters_mermaid_filters_to_given_layer() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline_domain = make_baseline("domain", "domain");
        let baseline_usecase = make_baseline("usecase", "usecase");
        let style = minimal_style();
        let clusters =
            render_clusters_mermaid(&[baseline_domain, baseline_usecase], &layer, &style).unwrap();
        // Only domain crate's cluster is returned.
        assert_eq!(clusters.len(), 1, "only domain layer baseline should produce clusters");
        assert!(
            clusters[0].0.starts_with("domain"),
            "cluster key must be prefixed with crate name"
        );
    }

    // -----------------------------------------------------------------------
    // StyleConfig TOML deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_style_config_minimal_deserializes_ok() {
        let toml_str = "[filter]\ninclude_functions = true\n";
        let config = toml::from_str::<StyleConfig>(toml_str);
        assert!(config.is_ok(), "minimal valid TOML must deserialize; err={config:?}");
    }

    #[test]
    fn test_style_config_with_class_section_deserializes_ok() {
        // Use single-quote TOML literals to avoid Rust raw-string termination issues.
        let toml_str = r#"
[class.struct_entry]
fill = '#dbeafe'
stroke = '#1e40af'
stroke_width = '2px'

[filter]
include_functions = true
"#;
        let config = toml::from_str::<StyleConfig>(toml_str);
        assert!(config.is_ok(), "TOML with [class.*] must deserialize; err={config:?}");
        let config = config.unwrap();
        assert!(config.class.contains_key("struct_entry"));
    }

    #[test]
    fn test_style_config_with_edge_section_deserializes_ok() {
        let toml_str = r#"
[edge.trait_impl]
arrow = "-.impl.->"

[filter]
include_functions = true
"#;
        let config = toml::from_str::<StyleConfig>(toml_str);
        assert!(config.is_ok(), "TOML with [edge.*] must deserialize; err={config:?}");
        let config = config.unwrap();
        let edge = &config.edge["trait_impl"];
        assert_eq!(edge.arrow, "-.impl.->");
        assert!(edge.label.is_none());
    }

    #[test]
    fn test_style_config_with_role_section_fails_deny_unknown_fields() {
        // [role.*] is NOT part of baseline-graph-style.toml schema (IN-04).
        // deny_unknown_fields must reject it.
        let toml_str = r#"
[role.Entity]
class = "entity"

[filter]
include_functions = true
"#;
        let result = toml::from_str::<StyleConfig>(toml_str);
        assert!(
            result.is_err(),
            "[role.*] section must be rejected by deny_unknown_fields in baseline StyleConfig"
        );
    }
}
