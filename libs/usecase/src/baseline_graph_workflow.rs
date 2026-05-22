//! Reality View (baseline graph) render workflow.
//!
//! Hexagonal composition:
//!
//! * [`RenderBaselineGraph`] — **primary port** (application_service trait).
//!   CLI and future adapters depend on this trait, not on the concrete
//!   interactor below, so composition roots stay substitutable.
//! * [`RenderBaselineGraphInteractor`] — the interactor that orchestrates
//!   the secondary ports (`BaselineGraphLoader`, `BaselineGraphRenderer`,
//!   `BaselineGraphWriter`) (Decision E / IN-02 / IN-19). It implements
//!   [`RenderBaselineGraph`].
//!
//! The usecase layer stays pure-orchestrator per
//! `knowledge/conventions/hexagonal-architecture.md` §Usecase Purity:
//! no `std::fs`, no `println!`, no `chrono::Utc::now`, no env access.
//! All I/O flows through the domain ports.
//!
//! Symmetric to `contract_map_workflow.rs` (Decision E / IN-02 / IN-19 / AC-02).

use domain::TrackId;
use domain::tddd::{
    BaselineGraphLoader, BaselineGraphLoaderError, BaselineGraphRenderer,
    BaselineGraphRendererError, BaselineGraphWriter, BaselineGraphWriterError, ClusterRender,
    LayerId,
};

// ---------------------------------------------------------------------------
// RenderBaselineGraphCommand
// ---------------------------------------------------------------------------

/// Command input for [`RenderBaselineGraph::execute`].
///
/// `track_id` is a validated [`TrackId`] (CN-12: concept-bearing identity
/// field typed as domain value object). Callers (CLI) construct `TrackId`
/// at the boundary and pass a typed value — no raw string validation in
/// the interactor.
///
/// `layer_filter` restricts rendering to the listed [`LayerId`] values when
/// `Some`; the interactor fails with
/// [`RenderBaselineGraphError::LayerNotFound`] when any entry is absent from
/// the loader output set.
///
/// Symmetric to `RenderContractMapCommand` in the contract map workflow
/// (IN-02 / AC-01 / AC-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderBaselineGraphCommand {
    /// Validated track identifier (CN-12).
    pub track_id: TrackId,
    /// If `Some`, restricts rendering to the listed layer identifiers (CN-12).
    pub layer_filter: Option<Vec<LayerId>>,
}

// ---------------------------------------------------------------------------
// RenderBaselineGraphOutput
// ---------------------------------------------------------------------------

/// Output DTO returned by [`RenderBaselineGraph::execute`] on success.
///
/// Lightweight metrics for CLI callers to report a post-write summary.
///
/// - `rendered_layer_count`: number of layers processed.
/// - `written_file_count`: includes depth-1 `index.md` plus all depth-2
///   cluster files written.
///
/// (IN-02 / AC-02)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderBaselineGraphOutput {
    /// Number of layers that were rendered.
    pub rendered_layer_count: usize,
    /// Number of markdown files written (overview + cluster files per layer).
    pub written_file_count: usize,
}

// ---------------------------------------------------------------------------
// RenderBaselineGraphError
// ---------------------------------------------------------------------------

/// Error variants surfaced by [`RenderBaselineGraph::execute`].
///
/// `InvalidTrackId` variant is absent: the `Command` receives a validated
/// `TrackId` so the interactor never sees an invalid ID (CN-12 / AC-15).
///
/// (IN-02 / AC-02 / AC-15 / CN-12)
#[derive(Debug)]
pub enum RenderBaselineGraphError {
    /// Failure inside a [`BaselineGraphLoader`] implementation.
    LoaderFailed(BaselineGraphLoaderError),

    /// Failure inside a [`BaselineGraphWriter`] implementation.
    WriterFailed(BaselineGraphWriterError),

    /// The loader returned an empty layer set — no `tddd.enabled` layers
    /// exist for this track. Rendering an empty baseline graph is not a
    /// useful workflow, so we fail closed.
    ///
    /// `track_id: TrackId` per CN-12.
    EmptyBaseline { track_id: TrackId },

    /// The caller's `layer_filter` references a layer that the loader did
    /// not produce — typically a CLI typo or a disabled layer.
    ///
    /// `track_id: TrackId` and `layer_id: LayerId` per CN-12.
    LayerNotFound { track_id: TrackId, layer_id: LayerId },

    /// The [`BaselineGraphRenderer`] implementation returned an error.
    RendererFailed(BaselineGraphRendererError),
}

impl std::fmt::Display for RenderBaselineGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoaderFailed(e) => write!(f, "{e}"),
            Self::WriterFailed(e) => write!(f, "{e}"),
            Self::EmptyBaseline { track_id } => write!(
                f,
                "baseline loader returned no enabled layers for track '{track_id}'; \
                 check `architecture-rules.json` tddd blocks"
            ),
            Self::LayerNotFound { track_id, layer_id } => {
                write!(f, "layer '{layer_id}' is not a tddd.enabled layer for track '{track_id}'")
            }
            Self::RendererFailed(e) => write!(f, "renderer failed: {e}"),
        }
    }
}

impl std::error::Error for RenderBaselineGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LoaderFailed(e) => Some(e),
            Self::WriterFailed(e) => Some(e),
            Self::RendererFailed(e) => Some(e),
            _ => None,
        }
    }
}

impl From<BaselineGraphLoaderError> for RenderBaselineGraphError {
    fn from(e: BaselineGraphLoaderError) -> Self {
        Self::LoaderFailed(e)
    }
}

impl From<BaselineGraphWriterError> for RenderBaselineGraphError {
    fn from(e: BaselineGraphWriterError) -> Self {
        Self::WriterFailed(e)
    }
}

impl From<BaselineGraphRendererError> for RenderBaselineGraphError {
    fn from(e: BaselineGraphRendererError) -> Self {
        Self::RendererFailed(e)
    }
}

// ---------------------------------------------------------------------------
// RenderBaselineGraph — primary port
// ---------------------------------------------------------------------------

/// Primary port (ApplicationService) for the Reality View (baseline graph)
/// render workflow.
///
/// CLI commands invoke the workflow through this trait so composition roots
/// can swap implementations (e.g., a no-op shim in tests).
///
/// Symmetric to `RenderContractMap` in the contract map workflow
/// (Decision E / IN-02 / AC-02).
pub trait RenderBaselineGraph {
    /// Render the baseline graph for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`RenderBaselineGraphError`] if the loader fails, the renderer
    /// fails, the writer fails, the enabled-layer set is empty, or a
    /// `layer_filter` entry does not appear in the loader output.
    fn execute(
        &self,
        cmd: &RenderBaselineGraphCommand,
    ) -> Result<RenderBaselineGraphOutput, RenderBaselineGraphError>;
}

// ---------------------------------------------------------------------------
// RenderBaselineGraphInteractor
// ---------------------------------------------------------------------------

/// Default [`RenderBaselineGraph`] implementation.
///
/// Composes three secondary ports:
/// - `L` ([`BaselineGraphLoader`]) — loads rustdoc JSON per layer.
/// - `R` ([`BaselineGraphRenderer`]) — renders depth-1 overview via
///   `render_overview` and all depth-2 cluster files via `render_clusters`;
///   cluster enumeration is the renderer's responsibility.
/// - `W` ([`BaselineGraphWriter`]) — persists generated markdown.
///
/// The interactor calls `render_overview` then `render_clusters`, then passes
/// each [`ClusterRender`]`.cluster_key` + `.content` to `writer.write_cluster`
/// — no `krate.paths` parsing in the interactor.
///
/// Symmetric to `RenderContractMapInteractor` (Decision E / IN-02 / IN-19 /
/// AC-02). No direct infrastructure calls per hexagonal purity (CN-03).
pub struct RenderBaselineGraphInteractor<L, R, W>
where
    L: BaselineGraphLoader,
    R: BaselineGraphRenderer,
    W: BaselineGraphWriter,
{
    loader: L,
    renderer: R,
    writer: W,
}

impl<L, R, W> RenderBaselineGraphInteractor<L, R, W>
where
    L: BaselineGraphLoader,
    R: BaselineGraphRenderer,
    W: BaselineGraphWriter,
{
    /// Creates a new interactor wrapping the supplied secondary ports.
    #[must_use]
    pub fn new(loader: L, renderer: R, writer: W) -> Self {
        Self { loader, renderer, writer }
    }
}

impl<L, R, W> RenderBaselineGraph for RenderBaselineGraphInteractor<L, R, W>
where
    L: BaselineGraphLoader,
    R: BaselineGraphRenderer,
    W: BaselineGraphWriter,
{
    fn execute(
        &self,
        cmd: &RenderBaselineGraphCommand,
    ) -> Result<RenderBaselineGraphOutput, RenderBaselineGraphError> {
        // Load all per-layer baseline documents for the track.
        let baselines = self.loader.load_all(&cmd.track_id)?;

        // Fail closed when no enabled layers exist for the track.
        if baselines.is_empty() {
            return Err(RenderBaselineGraphError::EmptyBaseline { track_id: cmd.track_id.clone() });
        }

        // Collect distinct layer ids from the loader output, preserving insertion order.
        // Each BaselineDocument carries its own LayerId so we group by it here.
        let mut all_layers: Vec<LayerId> = Vec::new();
        for doc in &baselines {
            if !all_layers.contains(&doc.layer) {
                all_layers.push(doc.layer.clone());
            }
        }

        // Resolve layer_filter against the loaded layer set.
        // An absent layer name produces LayerNotFound (fail-closed).
        // Filtering is done by iterating `all_layers` and keeping only those
        // present in the filter set — this mirrors the contract-map workflow
        // and ensures that duplicate entries in `layer_filter` do not cause a
        // layer to be rendered or written more than once.
        let validated_filter: Option<Vec<LayerId>> = cmd
            .layer_filter
            .as_ref()
            .map(|filter| {
                filter
                    .iter()
                    .map(|requested| {
                        all_layers.iter().find(|l| *l == requested).cloned().ok_or_else(|| {
                            RenderBaselineGraphError::LayerNotFound {
                                track_id: cmd.track_id.clone(),
                                layer_id: requested.clone(),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        let filtered_layers: Vec<LayerId> = match validated_filter.as_deref() {
            Some(f) if !f.is_empty() => {
                all_layers.iter().filter(|l| f.contains(l)).cloned().collect()
            }
            _ => all_layers.clone(),
        };

        // Two-phase pipeline: render ALL layers first, then write ALL layers.
        //
        // Phase 1 renders every layer into an in-memory `LayerOutput` value.
        // Only after every layer has been rendered successfully does Phase 2
        // begin writing.  This guarantees that a `RendererFailed` error from
        // any layer — including a later layer — cannot leave files written for
        // earlier layers without a corresponding set of files for the failing
        // layer.  The load → render → write ordering specified in spec IN-19
        // is preserved end-to-end: the writer is never called until the
        // renderer has completed its work on every layer.
        //
        // Cluster enumeration (krate.paths parsing, external-item filtering,
        // cluster-key generation per IN-14/IN-15) is the renderer adapter's
        // responsibility.  The interactor calls `render_clusters` and receives
        // a `Vec<ClusterRender>` — no krate.paths access here (hexagonal purity).

        // `LayerOutput` collects one rendered layer: the overview content plus
        // zero or more `ClusterRender` values.
        struct LayerOutput {
            layer: LayerId,
            overview_content: String,
            cluster_renders: Vec<ClusterRender>,
        }

        // --- Phase 1: render every layer -------------------------------------
        let mut layer_outputs: Vec<LayerOutput> = Vec::with_capacity(filtered_layers.len());

        for layer in &filtered_layers {
            // Collect the baseline documents for this layer.
            let layer_docs_slice: Vec<domain::BaselineDocument> =
                baselines.iter().filter(|doc| &doc.layer == layer).cloned().collect();

            // Render depth-1 overview.
            let overview_content = self.renderer.render_overview(&layer_docs_slice, layer)?;

            // Delegate cluster enumeration and rendering to the adapter.
            // The adapter handles krate.paths parsing and cluster key generation
            // per IN-14/IN-15.
            let cluster_renders = self.renderer.render_clusters(&layer_docs_slice, layer)?;

            layer_outputs.push(LayerOutput {
                layer: layer.clone(),
                overview_content,
                cluster_renders,
            });
        }

        // --- Phase 2: write every layer --------------------------------------
        // All rendering has completed successfully.  Write only starts here so
        // a renderer failure in Phase 1 cannot leave a partial on-disk state.
        let mut written_file_count: usize = 0;

        for output in &layer_outputs {
            self.writer.write_overview(&cmd.track_id, &output.layer, &output.overview_content)?;
            written_file_count += 1;

            for cr in &output.cluster_renders {
                self.writer.write_cluster(
                    &cmd.track_id,
                    &output.layer,
                    &cr.cluster_key,
                    &cr.content,
                )?;
                written_file_count += 1;
            }
        }

        Ok(RenderBaselineGraphOutput {
            rendered_layer_count: filtered_layers.len(),
            written_file_count,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::clone_on_ref_ptr
)]
mod tests {
    use std::error::Error as StdError;
    use std::path::PathBuf;

    use domain::tddd::baseline_document::BaselineDocument;
    use mockall::{mock, predicate};

    use super::*;

    // -----------------------------------------------------------------------
    // Mock definitions
    // -----------------------------------------------------------------------

    mock! {
        pub Loader {}
        impl BaselineGraphLoader for Loader {
            fn load_all(
                &self,
                track_id: &TrackId,
            ) -> Result<Vec<BaselineDocument>, BaselineGraphLoaderError>;
        }
    }

    mock! {
        pub Renderer {}
        impl BaselineGraphRenderer for Renderer {
            fn render_overview(
                &self,
                baselines: &[BaselineDocument],
                layer: &LayerId,
            ) -> Result<String, BaselineGraphRendererError>;

            fn render_clusters(
                &self,
                baselines: &[BaselineDocument],
                layer: &LayerId,
            ) -> Result<Vec<ClusterRender>, BaselineGraphRendererError>;
        }
    }

    mock! {
        pub Writer {}
        impl BaselineGraphWriter for Writer {
            fn write_overview(
                &self,
                track_id: &TrackId,
                layer: &LayerId,
                content: &str,
            ) -> Result<(), BaselineGraphWriterError>;

            fn write_cluster(
                &self,
                track_id: &TrackId,
                layer: &LayerId,
                cluster_key: &str,
                content: &str,
            ) -> Result<(), BaselineGraphWriterError>;
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn track_id(s: &str) -> TrackId {
        TrackId::try_new(s).unwrap()
    }

    fn layer_id(s: &str) -> LayerId {
        LayerId::try_new(s).unwrap()
    }

    /// Build a minimal `rustdoc_types::Crate` for test purposes via serde_json
    /// to avoid depending on the exact field list (forward-compatible).
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

    fn baseline_doc(layer: &str, crate_n: &str) -> BaselineDocument {
        use domain::tddd::catalogue_v2::identifiers::CrateName;
        BaselineDocument::new(layer_id(layer), CrateName::new(crate_n).unwrap(), minimal_crate())
    }

    /// Build a `ClusterRender` for use in mock `render_clusters` returns.
    fn cluster_render(cluster_key: &str) -> ClusterRender {
        ClusterRender { cluster_key: cluster_key.to_owned(), content: "cluster-content".to_owned() }
    }

    // -----------------------------------------------------------------------
    // RenderBaselineGraphCommand derive tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_command_debug_does_not_panic() {
        let cmd = RenderBaselineGraphCommand { track_id: track_id("my-track"), layer_filter: None };
        let _ = format!("{cmd:?}");
    }

    #[test]
    fn test_command_clone_produces_equal_value() {
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("my-track"),
            layer_filter: Some(vec![layer_id("domain")]),
        };
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_command_partial_eq_distinguishes_different_track_ids() {
        let cmd1 = RenderBaselineGraphCommand { track_id: track_id("track-a"), layer_filter: None };
        let cmd2 = RenderBaselineGraphCommand { track_id: track_id("track-b"), layer_filter: None };
        assert_ne!(cmd1, cmd2);
    }

    // -----------------------------------------------------------------------
    // RenderBaselineGraphOutput derive tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_output_debug_does_not_panic() {
        let out = RenderBaselineGraphOutput { rendered_layer_count: 2, written_file_count: 4 };
        let _ = format!("{out:?}");
    }

    #[test]
    fn test_output_clone_produces_equal_value() {
        let out = RenderBaselineGraphOutput { rendered_layer_count: 1, written_file_count: 2 };
        assert_eq!(out.clone(), out);
    }

    #[test]
    fn test_output_partial_eq_distinguishes_differing_counts() {
        let a = RenderBaselineGraphOutput { rendered_layer_count: 1, written_file_count: 2 };
        let b = RenderBaselineGraphOutput { rendered_layer_count: 1, written_file_count: 3 };
        assert_ne!(a, b);
    }

    // -----------------------------------------------------------------------
    // RenderBaselineGraphError — Display tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_error_loader_failed_display_delegates_to_inner() {
        let inner = BaselineGraphLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() };
        let err = RenderBaselineGraphError::LoaderFailed(inner);
        let msg = err.to_string();
        assert!(msg.contains("boom"), "Display must delegate to inner: {msg}");
    }

    #[test]
    fn test_error_writer_failed_display_delegates_to_inner() {
        let inner = BaselineGraphWriterError::IoError {
            path: PathBuf::from("/tmp/out.md"),
            reason: "disk full".to_owned(),
        };
        let err = RenderBaselineGraphError::WriterFailed(inner);
        let msg = err.to_string();
        assert!(msg.contains("disk full"), "Display must delegate to inner: {msg}");
    }

    #[test]
    fn test_error_empty_baseline_display_mentions_track_id() {
        let err = RenderBaselineGraphError::EmptyBaseline { track_id: track_id("empty-track") };
        let msg = err.to_string();
        assert!(msg.contains("empty-track"), "Display must contain track_id: {msg}");
    }

    #[test]
    fn test_error_layer_not_found_display_mentions_layer_and_track() {
        let err = RenderBaselineGraphError::LayerNotFound {
            track_id: track_id("my-track"),
            layer_id: layer_id("ghost-layer"),
        };
        let msg = err.to_string();
        assert!(msg.contains("ghost-layer"), "Display must contain layer_id: {msg}");
        assert!(msg.contains("my-track"), "Display must contain track_id: {msg}");
    }

    #[test]
    fn test_error_renderer_failed_display_delegates_to_inner() {
        let inner = BaselineGraphRendererError::RenderFailed { reason: "timeout".to_owned() };
        let err = RenderBaselineGraphError::RendererFailed(inner);
        let msg = err.to_string();
        assert!(msg.contains("timeout"), "Display must delegate to inner: {msg}");
    }

    // -----------------------------------------------------------------------
    // RenderBaselineGraphError — From impls
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_loader_error_wraps_as_loader_failed() {
        let inner = BaselineGraphLoaderError::LayerDiscoveryFailed { reason: "x".to_owned() };
        let err: RenderBaselineGraphError = inner.into();
        assert!(matches!(err, RenderBaselineGraphError::LoaderFailed(_)));
    }

    #[test]
    fn test_from_writer_error_wraps_as_writer_failed() {
        let inner =
            BaselineGraphWriterError::IoError { path: PathBuf::from("/x"), reason: "y".to_owned() };
        let err: RenderBaselineGraphError = inner.into();
        assert!(matches!(err, RenderBaselineGraphError::WriterFailed(_)));
    }

    #[test]
    fn test_from_renderer_error_wraps_as_renderer_failed() {
        let inner = BaselineGraphRendererError::RenderFailed { reason: "z".to_owned() };
        let err: RenderBaselineGraphError = inner.into();
        assert!(matches!(err, RenderBaselineGraphError::RendererFailed(_)));
    }

    // -----------------------------------------------------------------------
    // RenderBaselineGraphError — std::error::Error impls
    // -----------------------------------------------------------------------

    #[test]
    fn test_error_implements_std_error_debug() {
        let err = RenderBaselineGraphError::EmptyBaseline { track_id: track_id("t") };
        let _ = format!("{err:?}");
    }

    #[test]
    fn test_error_source_is_some_for_loader_failed() {
        let inner = BaselineGraphLoaderError::LayerDiscoveryFailed { reason: "x".to_owned() };
        let err = RenderBaselineGraphError::LoaderFailed(inner);
        assert!(StdError::source(&err).is_some());
    }

    #[test]
    fn test_error_source_is_none_for_empty_baseline() {
        let err = RenderBaselineGraphError::EmptyBaseline { track_id: track_id("t") };
        assert!(StdError::source(&err).is_none());
    }

    // -----------------------------------------------------------------------
    // Interactor — happy path
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_happy_path_single_layer_returns_correct_counts() {
        // render_clusters returns 1 ClusterRender → 1 cluster + 1 overview = 2 files.
        let tid = track_id("happy-track");
        let docs = vec![baseline_doc("domain", "domain")];

        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader
                .expect_load_all()
                .with(predicate::function(|t: &TrackId| t.as_ref() == "happy-track"))
                .times(1)
                .returning(move |_| Ok(docs_clone.clone()));
        }

        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(1).returning(|_, _| Ok("overview".to_owned()));
        renderer
            .expect_render_clusters()
            .times(1)
            .returning(|_, _| Ok(vec![cluster_render("domain_root")]));

        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(1).returning(|_, _, _| Ok(()));
        // write_cluster receives the cluster_key from ClusterRender
        writer
            .expect_write_cluster()
            .with(
                predicate::always(),
                predicate::always(),
                predicate::eq("domain_root"),
                predicate::always(),
            )
            .times(1)
            .returning(|_, _, _, _| Ok(()));

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand { track_id: tid.clone(), layer_filter: None };
        let out = interactor.execute(&cmd).unwrap();

        // 1 layer, 1 overview + 1 cluster = 2 files
        assert_eq!(out.rendered_layer_count, 1);
        assert_eq!(out.written_file_count, 2);
    }

    #[test]
    fn test_execute_happy_path_single_layer_two_clusters_returns_correct_counts() {
        // render_clusters returns 2 ClusterRenders → 2 clusters + 1 overview = 3 files.
        let docs = vec![baseline_doc("domain", "domain")];

        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().times(1).returning(move |_| Ok(docs_clone.clone()));
        }

        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(1).returning(|_, _| Ok("overview".to_owned()));
        renderer.expect_render_clusters().times(1).returning(|_, _| {
            Ok(vec![cluster_render("domain_models"), cluster_render("domain_services")])
        });

        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(1).returning(|_, _, _| Ok(()));
        writer.expect_write_cluster().times(2).returning(|_, _, _, _| Ok(()));

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("two-cluster-track"),
            layer_filter: None,
        };
        let out = interactor.execute(&cmd).unwrap();

        // 1 layer, 1 overview + 2 cluster files = 3 files
        assert_eq!(out.rendered_layer_count, 1);
        assert_eq!(out.written_file_count, 3);
    }

    #[test]
    fn test_execute_multiple_layers_returns_summed_file_count() {
        // Each layer: render_clusters returns 1 ClusterRender → 1 cluster per layer.
        let docs = vec![baseline_doc("domain", "domain"), baseline_doc("usecase", "usecase")];

        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().times(1).returning(move |_| Ok(docs_clone.clone()));
        }

        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(2).returning(|_, _| Ok("overview".to_owned()));
        renderer
            .expect_render_clusters()
            .times(2)
            .returning(|_, _| Ok(vec![cluster_render("some_root")]));

        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(2).returning(|_, _, _| Ok(()));
        writer.expect_write_cluster().times(2).returning(|_, _, _, _| Ok(()));

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd =
            RenderBaselineGraphCommand { track_id: track_id("multi-track"), layer_filter: None };
        let out = interactor.execute(&cmd).unwrap();

        // 2 layers, each: 1 overview + 1 cluster = 4 total
        assert_eq!(out.rendered_layer_count, 2);
        assert_eq!(out.written_file_count, 4);
    }

    // -----------------------------------------------------------------------
    // Interactor — error propagation
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_propagates_loader_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_| {
            Err(BaselineGraphLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() })
        });
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(0);
        renderer.expect_render_clusters().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(0);
        writer.expect_write_cluster().times(0);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd =
            RenderBaselineGraphCommand { track_id: track_id("err-track"), layer_filter: None };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(matches!(err, RenderBaselineGraphError::LoaderFailed(_)));
    }

    #[test]
    fn test_execute_empty_baseline_returns_empty_baseline_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_| Ok(vec![]));
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(0);
        renderer.expect_render_clusters().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(0);
        writer.expect_write_cluster().times(0);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd =
            RenderBaselineGraphCommand { track_id: track_id("empty-track"), layer_filter: None };
        let err = interactor.execute(&cmd).unwrap_err();
        match err {
            RenderBaselineGraphError::EmptyBaseline { track_id } => {
                assert_eq!(track_id.as_ref(), "empty-track");
            }
            other => panic!("expected EmptyBaseline, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_layer_filter_unknown_layer_returns_layer_not_found() {
        let docs = vec![baseline_doc("domain", "domain"), baseline_doc("usecase", "usecase")];
        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().returning(move |_| Ok(docs_clone.clone()));
        }
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(0);
        renderer.expect_render_clusters().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(0);
        writer.expect_write_cluster().times(0);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("my-track"),
            layer_filter: Some(vec![layer_id("ghost-layer")]),
        };
        let err = interactor.execute(&cmd).unwrap_err();
        match err {
            RenderBaselineGraphError::LayerNotFound { track_id, layer_id } => {
                assert_eq!(track_id.as_ref(), "my-track");
                assert_eq!(layer_id.as_ref(), "ghost-layer");
            }
            other => panic!("expected LayerNotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_layer_filter_known_layer_renders_only_that_layer() {
        // Two layers available, filter to only "domain".
        // render_clusters returns 1 ClusterRender for the domain layer → 2 files.
        let docs = vec![baseline_doc("domain", "domain"), baseline_doc("usecase", "usecase")];
        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().returning(move |_| Ok(docs_clone.clone()));
        }
        // Only 1 overview + 1 cluster (for domain layer only)
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().times(1).returning(|_, _| Ok("d-overview".to_owned()));
        renderer
            .expect_render_clusters()
            .times(1)
            .returning(|_, _| Ok(vec![cluster_render("domain_root")]));

        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(1).returning(|_, _, _| Ok(()));
        writer.expect_write_cluster().times(1).returning(|_, _, _, _| Ok(()));

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("filter-track"),
            layer_filter: Some(vec![layer_id("domain")]),
        };
        let out = interactor.execute(&cmd).unwrap();
        assert_eq!(out.rendered_layer_count, 1);
        assert_eq!(out.written_file_count, 2);
    }

    #[test]
    fn test_execute_renderer_overview_error_propagates_as_renderer_failed() {
        let docs = vec![baseline_doc("domain", "domain")];
        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().returning(move |_| Ok(docs_clone.clone()));
        }
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().returning(|_, _| {
            Err(BaselineGraphRendererError::RenderFailed { reason: "crashed".to_owned() })
        });
        renderer.expect_render_clusters().times(0);

        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(0);
        writer.expect_write_cluster().times(0);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("renderer-err-track"),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(
            matches!(err, RenderBaselineGraphError::RendererFailed(_)),
            "expected RendererFailed, got {err:?}"
        );
    }

    #[test]
    fn test_execute_writer_overview_error_propagates_as_writer_failed() {
        let docs = vec![baseline_doc("domain", "domain")];
        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().returning(move |_| Ok(docs_clone.clone()));
        }
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().returning(|_, _| Ok("ok".to_owned()));
        renderer.expect_render_clusters().returning(|_, _| Ok(vec![]));

        let mut writer = MockWriter::new();
        writer.expect_write_overview().returning(|_, _, _| {
            Err(BaselineGraphWriterError::IoError {
                path: PathBuf::from("/out"),
                reason: "disk full".to_owned(),
            })
        });
        writer.expect_write_cluster().times(0);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("writer-err-track"),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(
            matches!(err, RenderBaselineGraphError::WriterFailed(_)),
            "expected WriterFailed, got {err:?}"
        );
    }

    #[test]
    fn test_execute_renderer_clusters_error_propagates_as_renderer_failed() {
        // render_clusters fails during the render phase, so write_overview must
        // NOT be called (render-before-write ordering guarantee).
        let docs = vec![baseline_doc("domain", "domain")];
        let mut loader = MockLoader::new();
        {
            let docs_clone = docs.clone();
            loader.expect_load_all().returning(move |_| Ok(docs_clone.clone()));
        }
        let mut renderer = MockRenderer::new();
        renderer.expect_render_overview().returning(|_, _| Ok("ok".to_owned()));
        renderer.expect_render_clusters().returning(|_, _| {
            Err(BaselineGraphRendererError::StyleConfigNotFound {
                path: PathBuf::from("/missing.toml"),
            })
        });

        // With render-before-write ordering: render_clusters fails during the
        // render phase, so write_overview and write_cluster are never called.
        let mut writer = MockWriter::new();
        writer.expect_write_overview().times(0);
        writer.expect_write_cluster().times(0);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let cmd = RenderBaselineGraphCommand {
            track_id: track_id("cluster-err-track"),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(
            matches!(err, RenderBaselineGraphError::RendererFailed(_)),
            "expected RendererFailed, got {err:?}"
        );
    }
}
