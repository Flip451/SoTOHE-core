//! Infrastructure adapter implementing the `ContractMapRenderer` domain port.
//!
//! * [`ContractMapRendererAdapter`] — public adapter struct.
//! * Private TOML schema DTO types for `.harness/config/contract-map-style.toml`
//!   (Decision L-1 / CN-11 / Decision P-3). All style DTOs are private and
//!   never appear in the public API.
//!
//! **Scope (T003)**: this module implements the fail-closed style config loading
//! (absent → `StyleConfigNotFound`, invalid → `StyleConfigInvalid`, per CN-02 /
//! AC-11) and returns a minimal compiling placeholder for the render body.
//!
//! **TODO (T004–T009)**: the full mermaid rendering logic
//! (CatalogueNode / node_id / subgraphs / edges / classDef / class attach /
//! output assembly) is deferred to tasks T004–T009. Once implemented, replace
//! the `Ok(ContractMapContent::new("flowchart LR\n"))` placeholder body
//! inside `impl ContractMapRenderer for ContractMapRendererAdapter` with
//! the full pipeline.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::{
    ContractMapContent, ContractMapRenderOptions, ContractMapRenderer, ContractMapRendererError,
    LayerId,
};

use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// Private TOML schema DTOs (Decision P-3 / CN-11 / Decision L-1)
// These types are intentionally not `pub` — style schema changes must not
// leak into the public API.
//
// Fields are currently unused because the full rendering pipeline is
// deferred to T004–T009. The `#[allow(dead_code)]` attribute is intentional:
// the DTOs define the stable TOML schema contract and will be consumed
// incrementally as each rendering task is implemented.
// ---------------------------------------------------------------------------

/// Top-level structure for `.harness/config/contract-map-style.toml`.
///
/// `deny_unknown_fields` enforces fail-closed validation (CN-02): an
/// unrecognised top-level section (e.g. a typo like `[roles]` instead of
/// `[role]`) returns `StyleConfigInvalid` rather than being silently ignored.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleConfig {
    /// `[role.<RoleName>]` sections — DataRole / ContractRole / FunctionRole values.
    #[serde(default)]
    role: BTreeMap<String, RoleStyle>,

    /// `[node.<NodeCategory>]` sections — Method / Variant / Field / Function etc.
    #[serde(default)]
    node: BTreeMap<String, NodeStyle>,

    /// `[pattern.<PatternName>]` sections — currently only `Typestate`.
    #[serde(default)]
    pattern: BTreeMap<String, PatternStyle>,

    /// `[class.<ClassName>]` sections — mermaid classDef parameters.
    #[serde(default)]
    class: BTreeMap<String, ClassStyle>,

    /// `[edge.<EdgeKind>]` sections — arrow string + optional label.
    #[serde(default)]
    edge: BTreeMap<String, EdgeStyle>,

    /// `[filter]` section — future filter parameters.
    #[serde(default)]
    filter: FilterConfig,
}

/// `[role.<RoleName>]` — maps a role name to a mermaid class name.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleStyle {
    class: String,
}

/// `[node.<NodeCategory>]` — node shape and class for a node category.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeStyle {
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    class: Option<String>,
}

/// `[pattern.<PatternName>]` — overlay class applied to matching entries.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternStyle {
    overlay_class: String,
}

/// `[class.<ClassName>]` — mermaid classDef parameters.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassStyle {
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
/// EdgeKind values: `method_param` / `method_returns` / `transition` /
/// `trait_impl` / `variant_payload` / `field` / `alias`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeStyle {
    arrow: String,
    #[serde(default)]
    label: Option<String>,
}

/// `[filter]` — future filter configuration.
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilterConfig {
    /// Future: restrict rendered FunctionEntry roles (I-1 reserve).
    #[serde(default)]
    include_function_roles: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public adapter
// ---------------------------------------------------------------------------

/// Infrastructure adapter implementing [`ContractMapRenderer`].
///
/// Performs fail-closed style config loading from `style_config_path` on every
/// `render` call: absent file → `StyleConfigNotFound`, parse error →
/// `StyleConfigInvalid` (CN-02 / AC-11 / Decision P-1 / P-3 / IN-21).
///
/// The `new` constructor is infallible — config loading is deferred to the
/// first `render` call so the composition root stays cheap and error-free.
///
/// **Full mermaid rendering** (T004–T009) is not yet implemented.
/// The current `render` body loads and validates the style config, then
/// returns a minimal valid `ContractMapContent` placeholder.
pub struct ContractMapRendererAdapter {
    /// Path to `.harness/config/contract-map-style.toml`.
    /// Loaded on each `render` call (fail-closed).
    pub style_config_path: PathBuf,
}

impl ContractMapRendererAdapter {
    /// Creates a new adapter with the given style config path.
    ///
    /// The constructor is infallible. Style config loading (and its
    /// fail-closed error) happens inside [`ContractMapRenderer::render`].
    #[must_use]
    pub fn new(style_config_path: PathBuf) -> Self {
        Self { style_config_path }
    }

    /// Load and parse the style configuration file.
    ///
    /// Fail-closed (CN-02 / AC-11):
    /// - absent file (`NotFound`) → `StyleConfigNotFound { path }`
    /// - symlink at the path or any ancestor → `StyleConfigInvalid { path, reason }`
    /// - other I/O error (permission denied, etc.) → `StyleConfigInvalid { path, reason }`
    /// - TOML parse error → `StyleConfigInvalid { path, reason }`
    fn load_style_config(&self) -> Result<StyleConfig, ContractMapRendererError> {
        // Full-path symlink guard (CN-02 fail-closed): reject symlinks at the config
        // path and at every ancestor below the workspace root. This prevents both leaf
        // symlink swaps and parent-directory substitution attacks (including TOCTOU).
        //
        // The style config is always at `.harness/config/contract-map-style.toml` —
        // three levels below the workspace root — so we derive `trusted_root` by
        // walking up three levels. If the path is shallower than expected we fall back
        // to "/" so the guard still runs (it simply checks fewer ancestors).
        let trusted_root = self
            .style_config_path
            .parent() // .harness/config
            .and_then(Path::parent) // .harness
            .and_then(Path::parent) // workspace root
            .unwrap_or_else(|| Path::new("/"));

        match reject_symlinks_below(&self.style_config_path, trusted_root) {
            Ok(true) => {} // file exists, no symlinks — proceed to read
            Ok(false) => {
                // File does not exist — report as StyleConfigNotFound (CN-02 / AC-11)
                return Err(ContractMapRendererError::StyleConfigNotFound {
                    path: self.style_config_path.clone(),
                });
            }
            Err(e) => {
                // Symlink detected or stat failure — reject as StyleConfigInvalid
                return Err(ContractMapRendererError::StyleConfigInvalid {
                    path: self.style_config_path.clone(),
                    reason: e.to_string(),
                });
            }
        }

        let raw = std::fs::read_to_string(&self.style_config_path).map_err(|e| {
            ContractMapRendererError::StyleConfigInvalid {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })?;

        toml::from_str::<StyleConfig>(&raw).map_err(|e| {
            ContractMapRendererError::StyleConfigInvalid {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })
    }
}

impl ContractMapRenderer for ContractMapRendererAdapter {
    /// Render the contract map.
    ///
    /// **T003 scope**: loads and validates the style config (fail-closed),
    /// then returns a minimal valid `ContractMapContent` placeholder.
    ///
    /// **TODO (T004–T009)**: replace the placeholder body with the full
    /// mermaid rendering pipeline:
    /// - T004: CatalogueNode enum + node_id + global trait index
    /// - T005: subgraph / node placement
    /// - T006: method nodes + inherent_impls aggregation + typestate edges
    /// - T007: enum variant / TypeAlias / struct field edges
    /// - T008: trait impl edges + TraitEntry method nodes
    /// - T009: output assembly + style application
    fn render(
        &self,
        _catalogues: &[CatalogueDocument],
        _layer_order: &[LayerId],
        _opts: &ContractMapRenderOptions,
    ) -> Result<ContractMapContent, ContractMapRendererError> {
        // Fail-closed style config loading (CN-02 / AC-11).
        // If the config is absent or invalid, propagate the error immediately.
        let _style = self.load_style_config()?;

        // TODO(T004–T009): implement full mermaid rendering pipeline here.
        // For now, return a minimal valid ContractMapContent so the wiring
        // chain compiles and CI passes.
        Ok(ContractMapContent::new("flowchart LR\n"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;

    fn write_style_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("contract-map-style.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    const MINIMAL_VALID_CONFIG: &str = r#"
[filter]
include_function_roles = []
"#;

    const INVALID_TOML: &str = "role = [[[invalid toml";

    /// T003 / AC-11: absent style config file → StyleConfigNotFound.
    #[test]
    fn test_render_absent_style_config_returns_style_config_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nonexistent-style.toml");
        let adapter = ContractMapRendererAdapter::new(missing.clone());
        let opts = ContractMapRenderOptions::default();
        let err = adapter.render(&[], &[], &opts).unwrap_err();
        assert!(
            matches!(err, ContractMapRendererError::StyleConfigNotFound { ref path } if path == &missing),
            "expected StyleConfigNotFound with correct path, got {err:?}"
        );
    }

    /// T003 / AC-11: invalid TOML in style config → StyleConfigInvalid.
    #[test]
    fn test_render_invalid_toml_returns_style_config_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), INVALID_TOML);
        let adapter = ContractMapRendererAdapter::new(path.clone());
        let opts = ContractMapRenderOptions::default();
        let err = adapter.render(&[], &[], &opts).unwrap_err();
        match err {
            ContractMapRendererError::StyleConfigInvalid { path: ref err_path, .. } => {
                assert_eq!(err_path, &path, "StyleConfigInvalid must report the config path");
            }
            other => panic!("expected StyleConfigInvalid, got {other:?}"),
        }
    }

    /// T003: valid style config → render returns Ok with minimal placeholder.
    #[test]
    fn test_render_valid_style_config_returns_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();
        let result = adapter.render(&[], &[], &opts);
        assert!(result.is_ok(), "expected Ok with valid config, got {result:?}");
        let content = result.unwrap();
        assert!(
            content.as_ref().contains("flowchart LR"),
            "placeholder must contain 'flowchart LR'"
        );
    }

    /// T003: adapter new() is infallible — missing path does not panic at construction.
    #[test]
    fn test_adapter_new_is_infallible() {
        let missing = PathBuf::from("/this/does/not/exist.toml");
        // Should not panic.
        let _adapter = ContractMapRendererAdapter::new(missing);
    }
}
