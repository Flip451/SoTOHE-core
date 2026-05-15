//! Infrastructure adapter for the `ContractMapRenderer` domain port.
//!
//! [`ContractMapRendererAdapter`] implements the [`ContractMapRenderer`] trait
//! defined in `domain::tddd::contract_map_renderer`. It loads the style
//! configuration from `.harness/config/contract-map-style.toml` on each
//! `render()` call (fail-closed per CN-03) and returns a scaffold placeholder
//! `ContractMapContent` for T005. Actual rendering logic will be added in
//! T006–T008.
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
//! - **Decision L-1 + L-8 + L-10**: TOML schema with `[role.*]`, `[node.*]`,
//!   `[pattern.*]`, `[class.*]`, `[edge.*]`, `[filter]` sections.

use std::collections::HashMap;
use std::path::PathBuf;

use domain::tddd::catalogue_v2::{
    CatalogueDocument, FunctionEntry, FunctionPath, TraitEntry, TraitName, TypeEntry, TypeName,
};
use domain::tddd::{
    ContractMapContent, ContractMapRenderOptions, ContractMapRenderer, ContractMapRendererError,
    LayerId,
};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// StyleConfig — TOML schema structs (Decision L-1 + L-8 + L-10)
// ---------------------------------------------------------------------------

/// Top-level deserialization target for `.harness/config/contract-map-style.toml`.
///
/// All fields are permissive `HashMap`s — semantic validation is deferred to
/// T008 per the task scope. A parse failure (missing required field, wrong type)
/// results in `ContractMapRendererError::StyleConfigParse`.
///
/// The `dead_code` allow covers the T005 scaffold phase; individual fields will be
/// consumed by T006–T008 rendering logic. Fields are accessed via `load_style_config`
/// whose return value is bound to `_style` in the scaffold `render` method; actual
/// field reads happen in T006–T008.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct StyleConfig {
    /// `[role.<RoleName>]` sections — maps role name to `RoleStyle`.
    #[serde(default)]
    role: HashMap<String, RoleStyle>,

    /// `[node.<NodeCategory>]` sections — maps node category to `NodeStyle`.
    #[serde(default)]
    node: HashMap<String, NodeStyle>,

    /// `[pattern.<PatternName>]` sections — maps pattern name to `PatternStyle`.
    #[serde(default)]
    pattern: HashMap<String, PatternStyle>,

    /// `[class.<ClassName>]` sections — maps class name to mermaid classDef fields.
    #[serde(default)]
    class: HashMap<String, ClassStyle>,

    /// `[edge.<EdgeKind>]` sections — maps edge kind to arrow and optional label.
    #[serde(default)]
    edge: HashMap<String, EdgeStyle>,

    /// `[filter]` section — render filter configuration.
    #[serde(default)]
    filter: FilterConfig,
}

/// `[role.<RoleName>]` section: maps a role to a mermaid class name.
///
/// Fields are used by T006–T008. `dead_code` allow covers the T005 scaffold.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RoleStyle {
    class: String,
}

/// `[node.<NodeCategory>]` section: shape identifier and mermaid class name.
///
/// Fields are used by T006–T008. `dead_code` allow covers the T005 scaffold.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct NodeStyle {
    shape: String,
    class: String,
}

/// `[pattern.<PatternName>]` section: overlay class appended additively.
///
/// Fields are used by T007. `dead_code` allow covers the T005 scaffold.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PatternStyle {
    overlay_class: String,
}

/// `[class.<ClassName>]` section: mermaid classDef fill / stroke / width / dasharray.
///
/// Fields are used by T008. `dead_code` allow covers the T005 scaffold.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ClassStyle {
    fill: String,
    stroke: String,
    stroke_width: String,
    stroke_dasharray: String,
}

/// `[edge.<EdgeKind>]` section: mermaid arrow syntax and optional label.
///
/// Fields are used by T006–T008. `dead_code` allow covers the T005 scaffold.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EdgeStyle {
    arrow: String,
    #[serde(default)]
    label: Option<String>,
}

/// `[filter]` section: render filter configuration.
///
/// Default produces "render everything" behaviour (Decision I-1).
/// Fields are used by T008. `dead_code` allow covers the T005 scaffold.
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
struct FilterConfig {
    #[serde(default)]
    include_function_roles: Vec<String>,
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
/// T006–T008 will consume this list for subgraph / edge generation.
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
/// Used by the `*_node_id` functions. The resulting string contains only
/// `[a-zA-Z0-9_]` characters and is safe for use as a Mermaid node-id component.
///
/// The `dead_code` allow covers the T005 scaffold phase; it will be removed when
/// T006 consumes these helpers.
#[allow(dead_code)]
fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

/// Generates a mermaid node id for a `TypeEntry`.
///
/// Format: `T<len>_<sanitized_layer>_<sanitized_name>`
/// where `<len>` = char count of the **unsanitized** `name` (Decision D-2).
///
/// # Examples
///
/// `layer = "domain"`, `name = "UserEmail"` →
/// `T9_domain_UserEmail` (len("UserEmail") == 9)
///
/// The `dead_code` allow covers the T005 scaffold phase; removed when T006 lands.
#[allow(dead_code)]
pub(crate) fn type_node_id(layer: &LayerId, name: &TypeName) -> String {
    let sl = sanitize(layer.as_ref());
    let sn = sanitize(name.as_str());
    let len = name.as_str().chars().count();
    let body = format!("{sl}_{sn}");
    format!("T{len}_{body}")
}

/// Generates a mermaid node id for a `TraitEntry`.
///
/// Format: `R<len>_<sanitized_layer>_<sanitized_name>`
/// where `<len>` = char count of the **unsanitized** `name` (Decision D-2).
///
/// Using prefix `R` (tRait) ensures no collision with `TypeEntry` ids even when
/// the type and trait share the same name in the same layer (Decision D-2).
///
/// The `dead_code` allow covers the T005 scaffold phase; removed when T006 lands.
#[allow(dead_code)]
pub(crate) fn trait_node_id(layer: &LayerId, name: &TraitName) -> String {
    let sl = sanitize(layer.as_ref());
    let sn = sanitize(name.as_str());
    let len = name.as_str().chars().count();
    let body = format!("{sl}_{sn}");
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
///
/// The `dead_code` allow covers the T005 scaffold phase; removed when T006 lands.
#[allow(dead_code)]
pub(crate) fn function_node_id(layer: &LayerId, path: &FunctionPath) -> String {
    let sl = sanitize(layer.as_ref());
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
    /// # Current state (T005 scaffold)
    ///
    /// Loads and parses the style config (fail-closed). If loading succeeds,
    /// flattens the catalogues to a `Vec<CatalogueNode>` (unused until T006)
    /// and returns a placeholder `ContractMapContent`. The actual mermaid
    /// rendering logic will be implemented in T006–T008.
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
        _layer_order: &[LayerId],
        _opts: &ContractMapRenderOptions,
    ) -> Result<ContractMapContent, ContractMapRendererError> {
        // Fail-closed: load style config first (CN-03).
        let _style = self.load_style_config()?;

        // Flatten catalogue entries into CatalogueNode list.
        // T006–T008 will consume this to emit subgraph / edge / class lines.
        let _nodes = catalogues_to_nodes(catalogues);

        // T005 scaffold: return empty placeholder until rendering logic lands.
        Ok(ContractMapContent::new(String::new()))
    }
}

// ---------------------------------------------------------------------------
// Tests (split into external file to keep production code within 700 lines)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "contract_map_renderer_adapter_tests.rs"]
mod tests;
