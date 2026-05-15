//! Infrastructure adapter for the `ContractMapRenderer` domain port.
//!
//! [`ContractMapRendererAdapter`] implements the [`ContractMapRenderer`] trait
//! defined in `domain::tddd::contract_map_renderer`. It loads the style
//! configuration from `.harness/config/contract-map-style.toml` on each
//! `render()` call (fail-closed per CN-03) and delegates actual rendering to
//! the `render` sub-module added in T006.
//!
//! ## Key decisions implemented here
//!
//! - **Decision A-3'**: accepts `&[CatalogueDocument]` (self-descriptive per crate).
//! - **Decision B-1**: `CatalogueNode<'a>` enum (Type / Trait / Function) for
//!   entry-kind dispatch.
//! - **Decision C + CN-03**: style config loaded from
//!   `.harness/config/contract-map-style.toml`; absent file is a fail-closed error.
//! - **Decision D-2**: `node_id` generation with prefix + length-prefix + sanitized parts.
//! - **Decision E-3c**: this adapter is in infrastructure; the port is in domain.
//! - **Decision F-2+b2-ii + F-2+d1**: TypeEntry/TraitEntry → subgraphs; FunctionEntry →
//!   standalone callable node. Implemented in T006 via `render` sub-module.
//! - **Decision K-2+(d) + K-2**: PlainStruct/TupleStruct field edges. T006.
//! - **Decision U-6d-iii**: 4-level nesting: layer → top-module → entry → method. T006.
//! - **Decision L-1 + L-8 + L-10**: TOML schema with `[role.*]`, `[node.*]`,
//!   `[pattern.*]`, `[class.*]`, `[edge.*]`, `[filter]` sections.

use std::collections::HashMap;
use std::path::PathBuf;

use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FunctionEntry, FunctionPath, TraitEntry, TraitName, TypeEntry,
    TypeName,
};
use domain::tddd::{
    ContractMapContent, ContractMapRenderOptions, ContractMapRenderer, ContractMapRendererError,
    LayerId,
};
use serde::Deserialize;

// Sub-module: mermaid rendering logic (T006+).
mod render;

// ---------------------------------------------------------------------------
// StyleConfig — TOML schema structs (Decision L-1 + L-8 + L-10)
// ---------------------------------------------------------------------------

/// Top-level deserialization target for `.harness/config/contract-map-style.toml`.
///
/// All fields are permissive `HashMap`s — semantic validation is deferred to
/// T008 per the task scope. A parse failure (missing required field, wrong type)
/// results in `ContractMapRendererError::StyleConfigParse`.
///
/// Exposed as `pub(super)` so that the `render` sub-module can access style
/// fields for edge arrows, role class names, and classDef generation (T006).
#[derive(Debug, Deserialize)]
pub(super) struct StyleConfig {
    /// `[role.<RoleName>]` sections — maps role name to `RoleStyle`.
    #[serde(default)]
    pub(super) role: HashMap<String, RoleStyle>,

    /// `[node.<NodeCategory>]` sections — maps node category to `NodeStyle`.
    #[serde(default)]
    pub(super) node: HashMap<String, NodeStyle>,

    /// `[pattern.<PatternName>]` sections — maps pattern name to `PatternStyle`.
    /// Used by `typestate::maybe_emit_typestate_overlay` (T007).
    #[serde(default)]
    pub(super) pattern: HashMap<String, PatternStyle>,

    /// `[class.<ClassName>]` sections — maps class name to mermaid classDef fields.
    #[serde(default)]
    pub(super) class: HashMap<String, ClassStyle>,

    /// `[edge.<EdgeKind>]` sections — maps edge kind to arrow and optional label.
    #[serde(default)]
    pub(super) edge: HashMap<String, EdgeStyle>,

    /// `[filter]` section — render filter configuration.
    /// Used by T008 filter enforcement. `dead_code` allow covers T006.
    #[serde(default)]
    #[allow(dead_code)]
    pub(super) filter: FilterConfig,
}

/// `[role.<RoleName>]` section: maps a role to a mermaid class name.
#[derive(Debug, Deserialize)]
pub(super) struct RoleStyle {
    pub(super) class: String,
}

/// `[node.<NodeCategory>]` section: shape identifier and mermaid class name.
///
/// `shape` drives the mermaid node syntax for Method and Function nodes in T006.
/// `class` is used for class-attach lines on method and function nodes.
#[derive(Debug, Deserialize)]
pub(super) struct NodeStyle {
    /// Shape identifier (e.g. `"round"`, `"subroutine"`). Consumed by T006
    /// `node_shape()` helper to format method and function nodes with the correct
    /// mermaid syntax.
    pub(super) shape: String,
    pub(super) class: String,
}

/// `[pattern.<PatternName>]` section: overlay class appended additively.
///
/// Used by `typestate::maybe_emit_typestate_overlay` (T007) to attach the
/// typestate overlay class to PlainStruct entries that carry a typestate marker.
#[derive(Debug, Deserialize)]
pub(super) struct PatternStyle {
    pub(super) overlay_class: String,
}

/// `[class.<ClassName>]` section: mermaid classDef fill / stroke / width / dasharray.
///
/// All fields consumed by T006 `collect_classdefs`. No dead_code allow needed.
#[derive(Debug, Deserialize)]
pub(super) struct ClassStyle {
    pub(super) fill: String,
    pub(super) stroke: String,
    pub(super) stroke_width: String,
    pub(super) stroke_dasharray: String,
}

/// `[edge.<EdgeKind>]` section: mermaid arrow syntax and optional label.
///
/// `arrow` consumed by field/method/param edge emission (T006+).
/// `label` consumed by T007 typestate transition edges and alias edges.
#[derive(Debug, Deserialize)]
pub(super) struct EdgeStyle {
    pub(super) arrow: String,
    #[serde(default)]
    pub(super) label: Option<String>,
}

/// `[filter]` section: render filter configuration.
///
/// Default produces "render everything" behaviour (Decision I-1).
/// `include_function_roles` used by T008 filter enforcement.
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub(super) struct FilterConfig {
    #[serde(default)]
    pub(super) include_function_roles: Vec<String>,
}

// ---------------------------------------------------------------------------
// CatalogueNode — entry-kind enum (Decision B-1)
// ---------------------------------------------------------------------------

/// Internal enum that classifies a catalogue entry by kind (Type / Trait / Function).
///
/// Used inside `render()` to dispatch entry-kind–specific shape/edge logic.
/// The lifetime `'a` borrows from the input `&[CatalogueDocument]` slice —
/// no data is cloned during flattening.
///
/// T006–T008 will consume the fields in each variant. The `dead_code` allow is
/// intentional for the T005 scaffold; it will be removed when rendering logic lands.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum CatalogueNode<'a> {
    /// A type entry (struct / enum / type alias). Role: `DataRole`.
    Type {
        layer: &'a LayerId,
        doc: &'a CatalogueDocument,
        name: &'a TypeName,
        entry: &'a TypeEntry,
    },
    /// A trait entry. Role: `ContractRole`.
    Trait {
        layer: &'a LayerId,
        doc: &'a CatalogueDocument,
        name: &'a TraitName,
        entry: &'a TraitEntry,
    },
    /// A free function entry. Role: `FunctionRole`.
    Function {
        layer: &'a LayerId,
        doc: &'a CatalogueDocument,
        path: &'a FunctionPath,
        entry: &'a FunctionEntry,
    },
}

/// Flattens `catalogues` into a list of `CatalogueNode`s, preserving layer /
/// crate / module context.
///
/// Iteration order: for each document, types (BTreeMap alphabetical), then
/// traits (BTreeMap alphabetical), then functions (BTreeMap alphabetical).
/// Document order preserves the slice order of the input.
///
/// The T006 render sub-module uses direct BTreeMap iteration grouped by layer
/// rather than this flat list. This helper is retained for potential use by
/// T008+ analysis passes. `dead_code` allow covers the gap.
#[allow(dead_code)]
pub(crate) fn catalogues_to_nodes<'a>(
    catalogues: &'a [CatalogueDocument],
) -> Vec<CatalogueNode<'a>> {
    let mut nodes = Vec::new();
    for doc in catalogues {
        let layer = &doc.layer;
        for (name, entry) in &doc.types {
            nodes.push(CatalogueNode::Type { layer, doc, name, entry });
        }
        for (name, entry) in &doc.traits {
            nodes.push(CatalogueNode::Trait { layer, doc, name, entry });
        }
        for (path, entry) in &doc.functions {
            nodes.push(CatalogueNode::Function { layer, doc, path, entry });
        }
    }
    nodes
}

// ---------------------------------------------------------------------------
// node_id generation (Decision D-2)
// ---------------------------------------------------------------------------

/// Replaces every character in `s` that is not ASCII alphanumeric with `_`.
///
/// Used by the `*_node_id` functions and the `render` sub-module. The resulting
/// string contains only `[a-zA-Z0-9_]` characters and is safe for use as a
/// Mermaid node-id component.
pub(super) fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

/// Generates a mermaid node id for a `TypeEntry`.
///
/// Format: `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
/// where `<len>` = char count of the **unsanitized** `name` (Decision D-2).
/// Including `crate_name` prevents id collision when two crates in the same
/// layer declare a type with the same name (Decision D-2).
///
/// # Examples
///
/// `layer = "domain"`, `crate_name = "mylib"`, `name = "UserEmail"` →
/// `T9_domain_mylib_UserEmail` (len("UserEmail") == 9)
///
/// The layer component uses the same injective encoding as the layer subgraph id
/// (`render::escape_id_component`), so `"my-layer"` and `"my_layer"` produce
/// distinct node ids even though both collapse to `"my_layer"` under `sanitize`.
pub(crate) fn type_node_id(layer: &LayerId, crate_name: &CrateName, name: &TypeName) -> String {
    let sl = render::escape_id_component(layer.as_ref());
    let sc = sanitize(crate_name.as_str());
    let sn = sanitize(name.as_str());
    let len = name.as_str().chars().count();
    let body = format!("{sl}_{sc}_{sn}");
    format!("T{len}_{body}")
}

/// Generates a mermaid node id for a `TraitEntry`.
///
/// Format: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
/// where `<len>` = char count of the **unsanitized** `name` (Decision D-2).
///
/// Prefix `R` (tRait) ensures no collision with `TypeEntry` ids when the type
/// and trait share the same name in the same layer. Including `crate_name`
/// prevents collision when two crates in the same layer declare a trait with the
/// same name (Decision D-2).
pub(crate) fn trait_node_id(layer: &LayerId, crate_name: &CrateName, name: &TraitName) -> String {
    let sl = render::escape_id_component(layer.as_ref());
    let sc = sanitize(crate_name.as_str());
    let sn = sanitize(name.as_str());
    let len = name.as_str().chars().count();
    let body = format!("{sl}_{sc}_{sn}");
    format!("R{len}_{body}")
}

/// Generates a mermaid node id for a `FunctionEntry`.
///
/// Format: `F<len>_<sanitized_layer>_<sanitized_full_path>`
/// where `<sanitized_full_path>` is derived from `FunctionPath`'s `Display` form
/// (`crate_name[::module_path]::name`) passed through `sanitize` (non-alphanumeric
/// → `_`). Using the `Display` form ensures `crate_name` is included, preventing
/// collisions between functions with the same module path and name in different
/// crates. `<len>` = char count of the **unsanitized** `Display` string
/// (Decision D-2).
///
/// # Example
///
/// `crate_name="domain"`, `module_path=["tddd"]`, `name="register"`:
///   - `full_path_raw = "domain::tddd::register"` (22 chars)
///   - `sfp = sanitize("domain::tddd::register") = "domain__tddd__register"`
///   - `sl = "domain"`
///   - Result: `F22_domain_domain__tddd__register`
pub(crate) fn function_node_id(layer: &LayerId, path: &FunctionPath) -> String {
    let sl = render::escape_id_component(layer.as_ref());
    // Use the Display form of FunctionPath (`crate::module::name`) as the full_path_raw.
    // Including crate_name prevents collisions when two catalogues in the same layer
    // expose the same module path and function name in different crates.
    let full_path_raw = path.to_string();
    let len = full_path_raw.chars().count();
    let sfp = sanitize(&full_path_raw);
    let body = format!("{sl}_{sfp}");
    format!("F{len}_{body}")
}

// ---------------------------------------------------------------------------
// ContractMapRendererAdapter — struct + constructor
// ---------------------------------------------------------------------------

/// Infrastructure adapter that implements the [`ContractMapRenderer`] port.
///
/// Holds the path to the style configuration TOML
/// (`.harness/config/contract-map-style.toml`). The config is loaded on each
/// `render()` call — no long-lived index is kept (Decision O-a: avoid stale
/// issues). A missing config is a fail-closed error (CN-03).
///
/// Rendering logic will be added incrementally in T006–T008. This T005 scaffold
/// returns a placeholder `ContractMapContent` after successful config load.
pub struct ContractMapRendererAdapter {
    /// Path to `.harness/config/contract-map-style.toml`. Declared `pub` to match
    /// the catalogue's `has_stripped_fields: false` field list for this struct.
    pub style_config_path: PathBuf,
}

impl ContractMapRendererAdapter {
    /// Creates a new `ContractMapRendererAdapter`.
    ///
    /// * `style_config_path` — path to `.harness/config/contract-map-style.toml`.
    #[must_use]
    pub fn new(style_config_path: PathBuf) -> Self {
        Self { style_config_path }
    }

    /// Loads and parses the style configuration TOML.
    ///
    /// # Errors
    ///
    /// Returns `StyleConfigNotFound` when the file is absent.
    /// Returns `StyleConfigParse` when the TOML is malformed.
    fn load_style_config(&self) -> Result<StyleConfig, ContractMapRendererError> {
        let content = std::fs::read_to_string(&self.style_config_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ContractMapRendererError::StyleConfigNotFound {
                    path: self.style_config_path.clone(),
                }
            } else {
                ContractMapRendererError::StyleConfigParse {
                    path: self.style_config_path.clone(),
                    reason: e.to_string(),
                }
            }
        })?;
        toml::from_str::<StyleConfig>(&content).map_err(|e| {
            ContractMapRendererError::StyleConfigParse {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })
    }
}

impl ContractMapRenderer for ContractMapRendererAdapter {
    /// Render the contract map from the given catalogues and layer order.
    ///
    /// Loads the style config (fail-closed per CN-03), then delegates to
    /// `render::render_mermaid` which generates the mermaid flowchart string
    /// implementing Decisions U-6d-iii, F-2+b2-ii, F-2+d1, K-2+(d), and K-2.
    ///
    /// # Errors
    ///
    /// Returns [`ContractMapRendererError::StyleConfigNotFound`] if the style
    /// configuration file is absent (fail-closed, CN-03).
    ///
    /// Returns [`ContractMapRendererError::StyleConfigParse`] if the style
    /// configuration file cannot be parsed.
    fn render(
        &self,
        catalogues: &[CatalogueDocument],
        layer_order: &[LayerId],
        opts: &ContractMapRenderOptions,
    ) -> Result<ContractMapContent, ContractMapRendererError> {
        // Fail-closed: load style config first (CN-03).
        let style = self.load_style_config()?;

        // Delegate actual mermaid generation to the render sub-module (T006).
        let mermaid = render::render_mermaid(catalogues, layer_order, opts, &style);
        Ok(ContractMapContent::new(mermaid))
    }
}

// ---------------------------------------------------------------------------
// Tests (split into external file to keep production code within 700 lines)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "contract_map_renderer_adapter_tests.rs"]
mod tests;
