//! Infrastructure adapter implementing the `BaselineGraphRenderer` domain port.
//!
//! * [`BaselineGraphRendererAdapter`] — public adapter struct.
//! * Private TOML schema DTO types for `.harness/config/baseline-graph-style.toml`
//!   live in the `render` submodule (Decision L / CN-11 / symmetric to ContractMapRendererAdapter).
//!   All style DTOs are private and never appear in the public API.
//!
//! **Scope (T004)**: style config reading (fail-closed: absent → `StyleConfigNotFound`,
//! invalid → `StyleConfigInvalid`) + skeleton mermaid output (classDef + layer subgraph frame).
//! Full rendering pipeline (node extraction, edge rendering, cluster enumeration) will be
//! implemented in T005–T010.
//!
//! Style config section structure:
//! - `[node.*]`    — shape + class for node categories (Method, Variant, Function).
//! - `[pattern.*]` — overlay class for structural patterns (future extension).
//! - `[class.*]`   — mermaid `classDef` parameters.
//! - `[edge.*]`    — arrow syntax + optional label.
//! - `[filter]`    — future rendering filters.
//!
//! `[role.*]` is intentionally absent: Reality View input is `rustdoc_types::Crate`,
//! which carries no catalogue role data (ADR 2026-05-22-1507 Decision C / IN-04).
//!
//! Symmetric to `ContractMapRendererAdapter`. (IN-02 / IN-04 / AC-02 / AC-15 / CN-02 / CN-06)

mod render;

use std::path::{Path, PathBuf};

use domain::tddd::baseline_document::BaselineDocument;
use domain::tddd::baseline_graph_ports::{
    BaselineGraphRenderer, BaselineGraphRendererError, ClusterRender,
};
use domain::tddd::layer_id::LayerId;

use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// Public adapter
// ---------------------------------------------------------------------------

/// Infrastructure adapter implementing [`BaselineGraphRenderer`].
///
/// Reads `.harness/config/baseline-graph-style.toml` on each `render_*` call
/// (fail-closed: file absent → `StyleConfigNotFound`, invalid TOML →
/// `StyleConfigInvalid`). Produces depth-1 overview and depth-2 cluster files.
///
/// Symmetric to `ContractMapRendererAdapter`. (IN-02 / IN-04 / AC-15 / CN-02 / CN-06)
pub struct BaselineGraphRendererAdapter {
    /// Path to `.harness/config/baseline-graph-style.toml`.
    pub style_config_path: PathBuf,
}

impl BaselineGraphRendererAdapter {
    /// Creates a new adapter (infallible — config loading is deferred to each render call).
    ///
    /// # Errors
    ///
    /// This constructor is infallible. Style config loading errors are reported
    /// lazily by `render_overview` and `render_clusters`.
    #[must_use]
    pub fn new(style_config_path: PathBuf) -> Self {
        Self { style_config_path }
    }

    /// Load and parse the style config TOML file (fail-closed, CN-02 / AC-15).
    ///
    /// # Errors
    ///
    /// - `StyleConfigNotFound { path }` — the file does not exist or is a symlink.
    /// - `StyleConfigInvalid { path, reason }` — I/O or TOML parse error.
    fn load_style_config(&self) -> Result<render::StyleConfig, BaselineGraphRendererError> {
        // Derive trusted_root as grandparent of the config file
        // (.harness/config/baseline-graph-style.toml → .harness/config → .harness → workspace).
        let trusted_root = self
            .style_config_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("/"));

        match reject_symlinks_below(&self.style_config_path, trusted_root) {
            Ok(true) => {}
            Ok(false) => {
                return Err(BaselineGraphRendererError::StyleConfigNotFound {
                    path: self.style_config_path.clone(),
                });
            }
            Err(e) => {
                return Err(BaselineGraphRendererError::StyleConfigInvalid {
                    path: self.style_config_path.clone(),
                    reason: e.to_string(),
                });
            }
        }

        let raw = std::fs::read_to_string(&self.style_config_path).map_err(|e| {
            BaselineGraphRendererError::StyleConfigInvalid {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })?;

        toml::from_str::<render::StyleConfig>(&raw).map_err(|e| {
            BaselineGraphRendererError::StyleConfigInvalid {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })
    }
}

impl BaselineGraphRenderer for BaselineGraphRendererAdapter {
    /// Render a depth-1 overview Mermaid diagram for all baselines in a layer.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphRendererError::StyleConfigNotFound`] when the style
    /// configuration file is absent (fail-closed, CN-02 / AC-15).
    /// Returns [`BaselineGraphRendererError::StyleConfigInvalid`] when the style
    /// configuration file exists but cannot be parsed.
    /// Returns [`BaselineGraphRendererError::RenderFailed`] if rendering fails.
    fn render_overview(
        &self,
        baselines: &[BaselineDocument],
        layer: &LayerId,
    ) -> Result<String, BaselineGraphRendererError> {
        let style = self.load_style_config()?;
        render::render_overview_mermaid(baselines, layer, &style)
    }

    /// Enumerate all depth-2 clusters for the given baselines and layer,
    /// render each cluster, and return them as `Vec<ClusterRender>`.
    ///
    /// The adapter is responsible for cluster enumeration (krate.paths scanning + cluster key
    /// generation per IN-14/IN-15). In T004, one root-cluster placeholder per crate is returned;
    /// full enumeration will be added in T010.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphRendererError::StyleConfigNotFound`] when the style
    /// configuration file is absent (fail-closed, CN-02 / AC-15).
    /// Returns [`BaselineGraphRendererError::StyleConfigInvalid`] when the style
    /// configuration file exists but cannot be parsed.
    /// Returns [`BaselineGraphRendererError::RenderFailed`] if rendering fails.
    fn render_clusters(
        &self,
        baselines: &[BaselineDocument],
        layer: &LayerId,
    ) -> Result<Vec<ClusterRender>, BaselineGraphRendererError> {
        let style = self.load_style_config()?;
        let raw = render::render_clusters_mermaid(baselines, layer, &style)?;
        Ok(raw
            .into_iter()
            .map(|(cluster_key, content)| ClusterRender { cluster_key, content })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::tddd::baseline_document::BaselineDocument;
    use domain::tddd::baseline_graph_ports::BaselineGraphRenderer;
    use domain::tddd::catalogue_v2::identifiers::CrateName;

    fn write_style_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("baseline-graph-style.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    const MINIMAL_VALID_CONFIG: &str = r#"
[filter]
include_functions = true
"#;

    const INVALID_TOML: &str = "node = [[[invalid toml";

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
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_str).unwrap(),
            minimal_crate(),
        )
    }

    // -----------------------------------------------------------------------
    // T004: fail-closed style config — render_overview
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_overview_absent_style_config_returns_style_config_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nonexistent-baseline-style.toml");
        let adapter = BaselineGraphRendererAdapter::new(missing.clone());
        let layer = LayerId::try_new("domain").unwrap();
        let err = adapter.render_overview(&[], &layer).unwrap_err();
        assert!(
            matches!(err, BaselineGraphRendererError::StyleConfigNotFound { ref path } if path == &missing),
            "expected StyleConfigNotFound with correct path, got {err:?}"
        );
    }

    #[test]
    fn test_render_overview_invalid_toml_returns_style_config_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), INVALID_TOML);
        let adapter = BaselineGraphRendererAdapter::new(path.clone());
        let layer = LayerId::try_new("domain").unwrap();
        let err = adapter.render_overview(&[], &layer).unwrap_err();
        match err {
            BaselineGraphRendererError::StyleConfigInvalid { path: ref err_path, .. } => {
                assert_eq!(err_path, &path, "StyleConfigInvalid must report the config path");
            }
            other => panic!("expected StyleConfigInvalid, got {other:?}"),
        }
    }

    #[test]
    fn test_render_overview_valid_style_config_returns_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let result = adapter.render_overview(&[], &layer);
        assert!(result.is_ok(), "expected Ok with valid config, got {result:?}");
        let content = result.unwrap();
        assert!(content.contains("flowchart LR"), "must contain 'flowchart LR'");
    }

    // -----------------------------------------------------------------------
    // T004: fail-closed style config — render_clusters
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_clusters_absent_style_config_returns_style_config_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nonexistent-baseline-style.toml");
        let adapter = BaselineGraphRendererAdapter::new(missing.clone());
        let layer = LayerId::try_new("domain").unwrap();
        let err = adapter.render_clusters(&[], &layer).unwrap_err();
        assert!(
            matches!(err, BaselineGraphRendererError::StyleConfigNotFound { ref path } if path == &missing),
            "expected StyleConfigNotFound with correct path, got {err:?}"
        );
    }

    #[test]
    fn test_render_clusters_invalid_toml_returns_style_config_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), INVALID_TOML);
        let adapter = BaselineGraphRendererAdapter::new(path.clone());
        let layer = LayerId::try_new("domain").unwrap();
        let err = adapter.render_clusters(&[], &layer).unwrap_err();
        match err {
            BaselineGraphRendererError::StyleConfigInvalid { path: ref err_path, .. } => {
                assert_eq!(err_path, &path, "StyleConfigInvalid must report the config path");
            }
            other => panic!("expected StyleConfigInvalid, got {other:?}"),
        }
    }

    #[test]
    fn test_render_clusters_valid_config_empty_baselines_returns_empty_vec() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let result = adapter.render_clusters(&[], &layer);
        assert!(result.is_ok(), "expected Ok with valid config, got {result:?}");
        let clusters = result.unwrap();
        assert!(clusters.is_empty(), "no baselines → no clusters");
    }

    // -----------------------------------------------------------------------
    // T004: adapter constructor is infallible
    // -----------------------------------------------------------------------

    #[test]
    fn test_adapter_new_is_infallible() {
        let missing = PathBuf::from("/this/does/not/exist.toml");
        let _adapter = BaselineGraphRendererAdapter::new(missing);
    }

    // -----------------------------------------------------------------------
    // T004: render_overview output structure
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_overview_output_is_mermaid_fenced_markdown_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let content = adapter.render_overview(&[], &layer).unwrap();

        // Header comment must be the very first line.
        assert!(
            content.starts_with("<!-- Generated baseline-graph-renderer"),
            "output must start with generated-file header comment, got: {:?}",
            &content[..content.len().min(80)]
        );

        // Opening fence must immediately follow the header line.
        let after_header = content.find('\n').map(|i| &content[i + 1..]).unwrap_or("");
        assert!(
            after_header.starts_with("```mermaid\n"),
            "opening ```mermaid fence must follow the header comment, got: {:?}",
            &after_header[..after_header.len().min(40)]
        );

        // Closing fence must be present.
        assert!(content.contains("\n```\n"), "closing ``` fence must be present");

        // The mermaid body inside the fence must begin with 'flowchart LR'.
        let fence_open = "```mermaid\n";
        let fence_start = content.find(fence_open).expect("opening fence") + fence_open.len();
        let fence_end = content[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1)
            .unwrap_or(content.len());
        let mermaid_body = &content[fence_start..fence_end];
        assert!(
            mermaid_body.starts_with("flowchart LR\n"),
            "mermaid body inside the fence must start with 'flowchart LR\\n', got: {:?}",
            &mermaid_body[..mermaid_body.len().min(40)]
        );
    }

    #[test]
    fn test_render_overview_output_contains_layer_subgraph() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let content = adapter.render_overview(&[baseline], &layer).unwrap();
        assert!(content.contains("subgraph domain"), "must contain layer subgraph label");
    }

    // -----------------------------------------------------------------------
    // T004: render_clusters output structure
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_clusters_returns_cluster_render_with_correct_key() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let clusters = adapter.render_clusters(&[baseline], &layer).unwrap();
        assert_eq!(clusters.len(), 1, "one cluster per crate in T004");
        assert_eq!(clusters[0].cluster_key, "domain_root");
    }

    #[test]
    fn test_render_clusters_content_is_mermaid_fenced_markdown() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let clusters = adapter.render_clusters(&[baseline], &layer).unwrap();
        let content = &clusters[0].content;
        assert!(content.contains("```mermaid\n"), "cluster content must have mermaid fence");
        assert!(content.contains("\n```\n"), "cluster content must have closing fence");
        assert!(content.contains("flowchart LR"), "cluster content must contain flowchart LR");
    }

    // -----------------------------------------------------------------------
    // T004: layer-agnostic (not hardcoded to specific layer names)
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_overview_layer_agnostic_custom_layer_name() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);

        let layer = LayerId::try_new("my_custom_layer").unwrap();
        let baseline = make_baseline("my_custom_layer", "my_crate");
        let content = adapter.render_overview(&[baseline], &layer).unwrap();
        assert!(
            content.contains("my_custom_layer"),
            "layer name must appear in output; got: {content}"
        );
    }

    #[test]
    fn test_render_overview_two_layer_names_not_hardcoded() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = BaselineGraphRendererAdapter::new(path);

        for layer_str in ["alpha", "beta"] {
            let layer = LayerId::try_new(layer_str).unwrap();
            let baseline = make_baseline(layer_str, layer_str);
            let content = adapter.render_overview(&[baseline], &layer).unwrap();
            assert!(content.contains(layer_str), "layer name '{layer_str}' must appear in output");
        }
    }
}
