//! Secondary ports for the Reality View (baseline graph) pipeline.
//!
//! ## Ports defined here
//!
//! - [`BaselineGraphLoader`] — loads per-layer rustdoc baselines for a track.
//! - [`BaselineGraphRenderer`] — renders Mermaid markdown from baseline data.
//! - [`BaselineGraphWriter`] — persists rendered Mermaid markdown to the track directory.
//!
//! ## Error types defined here
//!
//! - [`BaselineGraphLoaderError`] — errors returned by `BaselineGraphLoader`.
//! - [`BaselineGraphRendererError`] — errors returned by `BaselineGraphRenderer`.
//! - [`BaselineGraphWriterError`] — errors returned by `BaselineGraphWriter`.
//!
//! ## Symmetric design
//!
//! All three ports and their error types are symmetric to the Contract Map ports in
//! `catalogue_ports.rs` and `contract_map_renderer.rs`. The same hexagonal convention
//! applies: ports live in the domain layer, implementations in the infrastructure layer.
//!
//! Per ADR `knowledge/adr/2026-05-22-1507-baseline-graph-renderer-rustdoc-adaptation.md`
//! (Decision E, IN-02, IN-19, AC-02, AC-15, CN-03).
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

use std::path::PathBuf;

use crate::TrackId;
use crate::tddd::baseline_document::BaselineDocument;
use crate::tddd::layer_id::LayerId;

// ---------------------------------------------------------------------------
// BaselineGraphLoaderError
// ---------------------------------------------------------------------------

/// Error returned by [`BaselineGraphLoader::load_all`].
///
/// - `NotFound`: a layer's rustdoc baseline JSON is absent on disk (fail-closed — no skip).
/// - `ParseFailed`: the rustdoc JSON deserialization failed.
/// - `IoError`: non-symlink I/O failure during load.
/// - `SymlinkRejected`: path reaches through a symlink and must be refused.
/// - `LayerDiscoveryFailed`: `architecture-rules.json` could not be read or parsed.
///
/// Symmetric to `CatalogueLoaderError`. (IN-02 / IN-19 / AC-15)
#[derive(Debug)]
pub enum BaselineGraphLoaderError {
    /// A layer's rustdoc baseline JSON is absent on disk. Fail-closed — missing
    /// baseline files are always an error; they must not be silently skipped.
    NotFound {
        /// The layer whose baseline was not found.
        layer_id: LayerId,
        /// Absolute or workspace-relative path that was absent.
        path: PathBuf,
    },
    /// The rustdoc JSON deserialization failed for the given layer.
    ParseFailed {
        /// The layer whose baseline could not be parsed.
        layer_id: LayerId,
        /// Human-readable reason from the underlying parse error.
        reason: String,
    },
    /// Non-symlink I/O failure during load.
    IoError {
        /// The layer that was being loaded.
        layer_id: LayerId,
        /// Path that was being read.
        path: PathBuf,
        /// Human-readable reason from the underlying I/O error.
        reason: String,
    },
    /// The path reaches through a symlink; must be refused (fail-closed).
    SymlinkRejected {
        /// The symlink path that was rejected.
        path: PathBuf,
    },
    /// `architecture-rules.json` could not be read or parsed.
    LayerDiscoveryFailed {
        /// Human-readable reason from the underlying error.
        reason: String,
    },
}

impl std::fmt::Display for BaselineGraphLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { layer_id, path } => {
                write!(
                    f,
                    "rustdoc baseline not found for layer '{}' at {}",
                    layer_id,
                    path.display()
                )
            }
            Self::ParseFailed { layer_id, reason } => {
                write!(f, "failed to parse rustdoc baseline for layer '{}': {}", layer_id, reason)
            }
            Self::IoError { layer_id, path, reason } => {
                write!(
                    f,
                    "I/O error loading rustdoc baseline for layer '{}' at {}: {}",
                    layer_id,
                    path.display(),
                    reason
                )
            }
            Self::SymlinkRejected { path } => {
                write!(f, "symlink rejected at {}", path.display())
            }
            Self::LayerDiscoveryFailed { reason } => {
                write!(f, "layer discovery failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for BaselineGraphLoaderError {}

// ---------------------------------------------------------------------------
// BaselineGraphRendererError
// ---------------------------------------------------------------------------

/// Error returned by [`BaselineGraphRenderer`] methods.
///
/// - `StyleConfigNotFound` / `StyleConfigInvalid`: fail-closed style config loading
///   (CN-02 / AC-15).
/// - `RenderFailed`: open placeholder for future rendering errors.
///
/// Symmetric to `ContractMapRendererError`. (IN-02 / AC-15)
#[derive(Debug)]
pub enum BaselineGraphRendererError {
    /// The style configuration file was not found at the expected path.
    /// Fail-closed: absent config is always an error (CN-02 / AC-15).
    StyleConfigNotFound {
        /// Absolute or workspace-relative path that was absent.
        path: PathBuf,
    },
    /// The style configuration file exists but could not be parsed.
    /// Fail-closed: invalid config is always an error (CN-02 / AC-15).
    StyleConfigInvalid {
        /// Path of the invalid config file.
        path: PathBuf,
        /// Human-readable reason from the underlying parse error.
        reason: String,
    },
    /// Rendering failed for any other reason. Open placeholder.
    RenderFailed {
        /// Human-readable reason.
        reason: String,
    },
}

impl std::fmt::Display for BaselineGraphRendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StyleConfigNotFound { path } => {
                write!(f, "baseline-graph style configuration not found: {}", path.display())
            }
            Self::StyleConfigInvalid { path, reason } => {
                write!(
                    f,
                    "baseline-graph style configuration is invalid at {}: {}",
                    path.display(),
                    reason
                )
            }
            Self::RenderFailed { reason } => {
                write!(f, "baseline-graph rendering failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for BaselineGraphRendererError {}

// ---------------------------------------------------------------------------
// BaselineGraphWriterError
// ---------------------------------------------------------------------------

/// Error returned by [`BaselineGraphWriter`] implementations.
///
/// - `IoError`: non-symlink I/O failure during the atomic write.
/// - `SymlinkRejected`: write target or intermediate component is a symlink; write refused.
/// - `TrackNotFound`: the track directory does not exist.
///
/// Symmetric to `ContractMapWriterError`. (IN-02 / IN-19 / AC-15)
#[derive(Debug)]
pub enum BaselineGraphWriterError {
    /// Non-symlink I/O failure during the atomic write.
    IoError {
        /// Path that was being written.
        path: PathBuf,
        /// Human-readable reason from the underlying I/O error.
        reason: String,
    },
    /// Write target or an intermediate component is a symlink; write refused fail-closed.
    SymlinkRejected {
        /// The symlink path that was rejected.
        path: PathBuf,
    },
    /// The track directory does not exist.
    TrackNotFound {
        /// The track identifier that was not found.
        track_id: TrackId,
        /// Expected directory path that was absent.
        expected_dir: PathBuf,
    },
}

impl std::fmt::Display for BaselineGraphWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError { path, reason } => {
                write!(f, "I/O error at {}: {}", path.display(), reason)
            }
            Self::SymlinkRejected { path } => {
                write!(f, "symlink rejected at {}", path.display())
            }
            Self::TrackNotFound { track_id, expected_dir } => {
                write!(
                    f,
                    "track '{}' not found (expected directory: {})",
                    track_id,
                    expected_dir.display()
                )
            }
        }
    }
}

impl std::error::Error for BaselineGraphWriterError {}

// ---------------------------------------------------------------------------
// ClusterRender — DTO returned by BaselineGraphRenderer::render_clusters
// ---------------------------------------------------------------------------

/// Data transfer object returned by [`BaselineGraphRenderer::render_clusters`].
///
/// `cluster_key` follows the spec IN-14/IN-15 convention:
/// `'<crate_name>_root'` for crate-root clusters or
/// `'<crate_name>_<module_seg1>'` for top-level module clusters.
/// It is a renderer-generated composite synthetic key with no independent
/// domain-level constraint that warrants a newtype.
///
/// `content` is the rendered Mermaid markdown string — opaque Mermaid markup
/// text with no domain-level structure.
///
/// Placed in domain alongside the port it serves (R1 delta: port
/// return-value containers live in the same layer as their port).
///
/// (IN-14 / IN-15)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterRender {
    /// Renderer-generated composite synthetic cluster key
    /// (`<crate_name>_root` or `<crate_name>_<module_seg1>`).
    pub cluster_key: String,
    /// Rendered Mermaid markdown — opaque text produced by the adapter.
    pub content: String,
}

// ---------------------------------------------------------------------------
// BaselineGraphLoader — secondary port
// ---------------------------------------------------------------------------

/// Secondary port that loads all per-layer rustdoc baselines for a given track
/// as a `Vec<BaselineDocument>`.
///
/// Each `BaselineDocument` carries its own `LayerId` so callers can group by layer
/// without additional context. Symmetric to `CatalogueLoader` (which loads
/// `CatalogueDocument` per layer).
///
/// Implementations discover `tddd.enabled` layers via `architecture-rules.json` and
/// load the associated rustdoc JSON for each layer. (IN-02 / IN-19 / AC-02 / CN-03)
pub trait BaselineGraphLoader: Send + Sync {
    /// Load all enabled-layer rustdoc baselines for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphLoaderError`] if the architecture rules cannot be
    /// discovered, a baseline file is missing or reached through a rejected symlink,
    /// a baseline fails to parse, or any non-symlink I/O error occurs.
    fn load_all(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<BaselineDocument>, BaselineGraphLoaderError>;
}

// ---------------------------------------------------------------------------
// BaselineGraphRenderer — secondary port
// ---------------------------------------------------------------------------

/// Secondary port for Reality View (baseline graph) rendering.
///
/// Infrastructure adapter implements this port. `RenderBaselineGraphInteractor`
/// injects it via generic `R`.
///
/// - `render_overview` produces depth-1 overview content for a layer.
/// - `render_clusters` enumerates all clusters internally (krate.paths parsing +
///   external item filter + cluster key generation per IN-14/IN-15) and returns
///   `Vec<`[`ClusterRender`]`>`. This keeps cluster-enumeration logic inside the
///   adapter, making the design symmetric to `ContractMapRenderer::render`
///   (single method, no cluster enumeration in the interactor).
///
/// Replaces the prior `render_cluster` (single-cluster) that required the
/// interactor to enumerate clusters from `krate.paths`.
///
/// Symmetric to `ContractMapRenderer` (Decision E / IN-02 / AC-02). No syn
/// dependency (rustdoc already parses types — Decision E note).
pub trait BaselineGraphRenderer: Send + Sync {
    /// Render a depth-1 overview Mermaid diagram for all baselines in a layer.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphRendererError`] when style configuration is absent
    /// or invalid (fail-closed, CN-02 / AC-15), or when rendering fails.
    fn render_overview(
        &self,
        baselines: &[BaselineDocument],
        layer: &LayerId,
    ) -> Result<String, BaselineGraphRendererError>;

    /// Enumerate all depth-2 clusters for the given baselines and layer,
    /// render each cluster, and return them as a `Vec<`[`ClusterRender`]`>`.
    ///
    /// The adapter is responsible for `krate.paths` parsing, external-item
    /// filtering (`crate_id != 0`), and cluster key generation per the
    /// IN-14/IN-15 convention (`<crate_name>_root` / `<crate_name>_<module_seg1>`).
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphRendererError`] when style configuration is absent
    /// or invalid (fail-closed, CN-02 / AC-15), or when rendering fails.
    fn render_clusters(
        &self,
        baselines: &[BaselineDocument],
        layer: &LayerId,
    ) -> Result<Vec<ClusterRender>, BaselineGraphRendererError>;
}

// ---------------------------------------------------------------------------
// BaselineGraphWriter — secondary port
// ---------------------------------------------------------------------------

/// Secondary port that persists Reality View (baseline graph) Mermaid markdown files.
///
/// - `write_overview` writes `<layer>-graph-d1/index.md`.
/// - `write_cluster` writes `<layer>-graph-d2/<cluster_key>.md`.
///
/// Symmetric to `ContractMapWriter`. Implementations must write atomically and
/// refuse to follow symlinks below the caller-supplied trust root. (IN-02 / IN-19 / AC-02 / CN-03)
pub trait BaselineGraphWriter: Send + Sync {
    /// Persist overview Mermaid content for a given track and layer.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphWriterError`] if the track directory is missing,
    /// the target path is reached through a symlink, or the underlying I/O fails.
    fn write_overview(
        &self,
        track_id: &TrackId,
        layer: &LayerId,
        content: &str,
    ) -> Result<(), BaselineGraphWriterError>;

    /// Persist cluster-detail Mermaid content for a given track, layer, and cluster key.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphWriterError`] if the track directory is missing,
    /// the target path is reached through a symlink, or the underlying I/O fails.
    fn write_cluster(
        &self,
        track_id: &TrackId,
        layer: &LayerId,
        cluster_key: &str,
        content: &str,
    ) -> Result<(), BaselineGraphWriterError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // BaselineGraphLoaderError — Display tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_loader_error_not_found_display_contains_layer_and_path() {
        let err = BaselineGraphLoaderError::NotFound {
            layer_id: LayerId::try_new("domain").unwrap(),
            path: PathBuf::from("/track/items/my-track/baseline/domain.json"),
        };
        let msg = err.to_string();
        assert!(msg.contains("domain"), "Display must include layer id; got: {msg}");
        assert!(
            msg.contains("/track/items/my-track/baseline/domain.json"),
            "Display must include path; got: {msg}"
        );
        assert!(msg.contains("not found"), "Display must mention 'not found'; got: {msg}");
    }

    #[test]
    fn test_loader_error_parse_failed_display_contains_layer_and_reason() {
        let err = BaselineGraphLoaderError::ParseFailed {
            layer_id: LayerId::try_new("usecase").unwrap(),
            reason: "unexpected field `foo`".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("usecase"), "Display must include layer id; got: {msg}");
        assert!(msg.contains("unexpected field `foo`"), "Display must include reason; got: {msg}");
    }

    #[test]
    fn test_loader_error_io_error_display_contains_layer_path_and_reason() {
        let err = BaselineGraphLoaderError::IoError {
            layer_id: LayerId::try_new("infrastructure").unwrap(),
            path: PathBuf::from("/tmp/baseline.json"),
            reason: "permission denied".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("infrastructure"), "Display must include layer id; got: {msg}");
        assert!(msg.contains("/tmp/baseline.json"), "Display must include path; got: {msg}");
        assert!(msg.contains("permission denied"), "Display must include reason; got: {msg}");
    }

    #[test]
    fn test_loader_error_symlink_rejected_display_contains_path() {
        let err =
            BaselineGraphLoaderError::SymlinkRejected { path: PathBuf::from("/some/symlink") };
        let msg = err.to_string();
        assert!(msg.contains("/some/symlink"), "Display must include path; got: {msg}");
        assert!(msg.contains("symlink"), "Display must mention 'symlink'; got: {msg}");
    }

    #[test]
    fn test_loader_error_layer_discovery_failed_display_contains_reason() {
        let err = BaselineGraphLoaderError::LayerDiscoveryFailed {
            reason: "architecture-rules.json missing".to_owned(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("architecture-rules.json missing"),
            "Display must include reason; got: {msg}"
        );
    }

    #[test]
    fn test_loader_error_implements_std_error() {
        let err: &dyn std::error::Error =
            &BaselineGraphLoaderError::LayerDiscoveryFailed { reason: "test".to_owned() };
        assert!(err.source().is_none());
    }

    // -----------------------------------------------------------------------
    // BaselineGraphRendererError — Display tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_renderer_error_style_config_not_found_display_contains_path() {
        let path = PathBuf::from("/conf/baseline-graph-style.toml");
        let err = BaselineGraphRendererError::StyleConfigNotFound { path: path.clone() };
        let msg = err.to_string();
        assert!(
            msg.contains("/conf/baseline-graph-style.toml"),
            "Display must include path; got: {msg}"
        );
        assert!(msg.contains("not found"), "Display must mention 'not found'; got: {msg}");
    }

    #[test]
    fn test_renderer_error_style_config_invalid_display_contains_path_and_reason() {
        let path = PathBuf::from("/conf/style.toml");
        let err = BaselineGraphRendererError::StyleConfigInvalid {
            path: path.clone(),
            reason: "unexpected key `alpha`".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("/conf/style.toml"), "Display must include path; got: {msg}");
        assert!(msg.contains("unexpected key `alpha`"), "Display must include reason; got: {msg}");
    }

    #[test]
    fn test_renderer_error_render_failed_display_contains_reason() {
        let err = BaselineGraphRendererError::RenderFailed { reason: "graph overflow".to_owned() };
        let msg = err.to_string();
        assert!(msg.contains("graph overflow"), "Display must include reason; got: {msg}");
        assert!(
            msg.contains("rendering failed"),
            "Display must mention 'rendering failed'; got: {msg}"
        );
    }

    #[test]
    fn test_renderer_error_implements_std_error() {
        let err: &dyn std::error::Error =
            &BaselineGraphRendererError::RenderFailed { reason: "test".to_owned() };
        assert!(err.source().is_none());
    }

    // -----------------------------------------------------------------------
    // BaselineGraphWriterError — Display tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_writer_error_io_error_display_contains_path_and_reason() {
        let err = BaselineGraphWriterError::IoError {
            path: PathBuf::from("/out/layer-graph-d1/index.md"),
            reason: "disk full".to_owned(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("/out/layer-graph-d1/index.md"),
            "Display must include path; got: {msg}"
        );
        assert!(msg.contains("disk full"), "Display must include reason; got: {msg}");
    }

    #[test]
    fn test_writer_error_symlink_rejected_display_contains_path() {
        let err = BaselineGraphWriterError::SymlinkRejected { path: PathBuf::from("/link/target") };
        let msg = err.to_string();
        assert!(msg.contains("/link/target"), "Display must include path; got: {msg}");
        assert!(msg.contains("symlink"), "Display must mention 'symlink'; got: {msg}");
    }

    #[test]
    fn test_writer_error_track_not_found_display_contains_track_id_and_dir() {
        let track_id = TrackId::try_new("reality-view-renderer-2026-05-22").unwrap();
        let err = BaselineGraphWriterError::TrackNotFound {
            track_id: track_id.clone(),
            expected_dir: PathBuf::from("/track/items/reality-view-renderer-2026-05-22"),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("reality-view-renderer-2026-05-22"),
            "Display must include track id; got: {msg}"
        );
        assert!(
            msg.contains("/track/items/reality-view-renderer-2026-05-22"),
            "Display must include expected dir; got: {msg}"
        );
    }

    #[test]
    fn test_writer_error_implements_std_error() {
        let err: &dyn std::error::Error = &BaselineGraphWriterError::IoError {
            path: PathBuf::from("/tmp/test.md"),
            reason: "test".to_owned(),
        };
        assert!(err.source().is_none());
    }

    // -----------------------------------------------------------------------
    // Debug impls — sanity
    // -----------------------------------------------------------------------

    #[test]
    fn test_loader_error_debug_does_not_panic() {
        let err = BaselineGraphLoaderError::LayerDiscoveryFailed { reason: "oops".to_owned() };
        let _ = format!("{err:?}");
    }

    #[test]
    fn test_renderer_error_debug_does_not_panic() {
        let err = BaselineGraphRendererError::RenderFailed { reason: "oops".to_owned() };
        let _ = format!("{err:?}");
    }

    #[test]
    fn test_writer_error_debug_does_not_panic() {
        let err = BaselineGraphWriterError::IoError {
            path: PathBuf::from("/tmp/x"),
            reason: "oops".to_owned(),
        };
        let _ = format!("{err:?}");
    }
}
