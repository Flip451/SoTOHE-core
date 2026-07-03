//! DTOs, error types, secondary ports, the primary service trait, and the
//! [`DryWriteDriverInteractor`] orchestration for `sotp dry write`.
//!
//! Mirrors the multi-fine-grained-port-factory shape already established by
//! [`crate::fixpoint_resolve_driver`] (T017) and [`crate::dry_driver_shared`]
//! (T024) rather than a single coarse pass-through port.
//!
//! [`DryWriteDriverInteractor`] performs the entire dry-write orchestration
//! itself (ADR 2026-06-21-1328 D4) — no infrastructure adapter sits between
//! this interactor and the domain-level `run_dry_check` call.
//!
//! Design: ADR 2026-06-21-1328 D2 / D4 / D7, IN-14, AC-20.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::dry_check::{DryCheckConfigFingerprint, DryCheckCorpusFingerprint, DryCheckFinding};
use domain::semantic_dup::{CodeFragment, SimilarityThreshold};
use domain::{CommitHash, TrackId};

use crate::dry_check::{DryCheckConfig, DryCheckCoveragePort, DryCheckCycleError, DryCheckService};
use crate::dry_driver::{DryWriteDriverInput, DryWriteFindingSummary, DryWriteOutcome};
use crate::dry_driver_shared::{
    DryBaseBranchPort, DryCheckStorageFactoryPort, DryDiffBaseFactoryPort, DryRepoRootPort,
    IoFailureDetail,
};
use crate::fixpoint_resolve_driver::DryCheckConfigLoaderError;

mod failure_details;
pub use failure_details::{
    AgentConfigResolutionFailureDetail, CapabilityName, DiffHunkListingFailureDetail,
    EmbeddingModelLoadFailureDetail, FragmentPipelineFailureDetail, SemanticIndexOpenFailureDetail,
    SerializationFailureDetail,
};

// ── DryWriteConfigResolution ──────────────────────────────────────────────────

/// Resolved dry-check config, effective threshold, and config fingerprint for
/// a `dry write` run.
#[derive(Debug, Clone)]
pub struct DryWriteConfigResolution {
    /// Usecase-level dry-check configuration.
    pub dry_config: DryCheckConfig,
    /// The effective similarity threshold (override or file default).
    pub effective_threshold: SimilarityThreshold,
    /// SHA-256 fingerprint selected via the safe-approval rule.
    pub config_fingerprint: DryCheckConfigFingerprint,
}

// ── DryCorpusFragmentsOutput ──────────────────────────────────────────────────

/// Diff and corpus fragments plus the corpus fingerprint for a `dry write` run.
#[derive(Debug, Clone)]
pub struct DryCorpusFragmentsOutput {
    /// Hunk-scoped fragments overlapping the current diff.
    pub diff_fragments: Vec<CodeFragment>,
    /// Whole-workspace corpus fragments.
    pub corpus_fragments: Vec<CodeFragment>,
    /// SHA-256 fingerprint over the corpus fragments.
    pub corpus_fingerprint: DryCheckCorpusFingerprint,
}

// ── DryCorpusFragmentsError ───────────────────────────────────────────────────

/// Error returned by [`DryCorpusFragmentsPort::build`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryCorpusFragmentsError {
    /// The symlink guard rejected `workspace_root`.
    #[error("symlink guard workspace_root '{}': {detail}", workspace_root.display())]
    WorkspaceRootSymlinkRejected {
        /// The rejected `workspace_root` path.
        workspace_root: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// `workspace_root` does not exist, is not a directory, or resolves
    /// outside the repository root.
    #[error(
        "workspace_root '{}' must be an existing directory under the repository root",
        workspace_root.display()
    )]
    WorkspaceRootInvalid {
        /// The invalid `workspace_root` path.
        workspace_root: PathBuf,
    },
    /// `GitDryCheckDiffGetter::list_changed_hunks` failed.
    #[error("list_changed_hunks failed: {detail}")]
    DiffHunkListingFailed {
        /// Rendered diagnostic text.
        detail: DiffHunkListingFailureDetail,
    },
    /// `extract_code_fragments` failed over `workspace_root`.
    #[error("fragment extraction failed: {detail}")]
    FragmentExtractionFailed {
        /// Rendered diagnostic text.
        detail: FragmentPipelineFailureDetail,
    },
    /// The subsequent `CodeFragment` path-rebuild step
    /// (`normalize_fragment_paths`) failed.
    #[error("fragment path normalization failed: {detail}")]
    FragmentPathNormalizationFailed {
        /// Rendered diagnostic text.
        detail: FragmentPipelineFailureDetail,
    },
}

// ── DryCheckServiceFactoryCommand ─────────────────────────────────────────────

/// Command for [`DryCheckServiceFactoryPort::build`].
#[derive(Clone)]
pub struct DryCheckServiceFactoryCommand {
    /// Read port for `dry-check.json`.
    pub reader: Arc<dyn domain::dry_check::DryCheckReader>,
    /// Write port for `dry-check.json`.
    pub writer: Arc<dyn domain::dry_check::DryCheckWriter>,
    /// Coverage manifest port for `dry-check-coverage.json`.
    pub coverage: Arc<dyn DryCheckCoveragePort>,
    /// Active track.
    pub track_id: TrackId,
    /// Git repository root.
    pub repo_root: PathBuf,
    /// Canonicalized `track/items` directory.
    pub items_dir: PathBuf,
    /// Path to the LanceDB semantic index database.
    pub db_path: PathBuf,
    /// Whole-workspace corpus fragments used to build/update the semantic index.
    pub corpus_fragments: Vec<CodeFragment>,
    /// Optional explicit Codex model override.
    pub model: Option<String>,
    /// Capability name forwarded to `CodexDryChecker`.
    pub capability_name: String,
    /// Usecase-level dry-check configuration.
    pub dry_config: DryCheckConfig,
    /// SHA-256 fingerprint of the current `.harness/config/dry-check.json`.
    pub config_fingerprint: DryCheckConfigFingerprint,
    /// SHA-256 fingerprint of the current corpus.
    pub corpus_fingerprint: DryCheckCorpusFingerprint,
}

// ── DryCheckServiceFactoryError ───────────────────────────────────────────────

/// Error returned by [`DryCheckServiceFactoryPort::build`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryCheckServiceFactoryError {
    /// `FastEmbedAdapter::new()` failed (offline-cache preflight or model init).
    #[error("failed to load embedding model: {detail}")]
    EmbeddingModelLoadFailed {
        /// Rendered diagnostic text.
        detail: EmbeddingModelLoadFailureDetail,
    },
    /// `persistent_index::open_persistent_index_with_corpus` failed (LanceDB
    /// open/create, index cache marker I/O, or symlink guard).
    #[error("{detail}")]
    SemanticIndexOpenFailed {
        /// Rendered diagnostic text.
        detail: SemanticIndexOpenFailureDetail,
    },
    /// `checker_config::resolve_dry_checker_config` failed (agent-profiles.json
    /// symlink guard rejection, load/parse failure, no model configured for
    /// `capability_name`, or an invalid `reasoning_effort` value).
    #[error("{detail}")]
    AgentConfigResolutionFailed {
        /// The capability name that was being resolved.
        capability_name: CapabilityName,
        /// Rendered diagnostic text.
        detail: AgentConfigResolutionFailureDetail,
    },
}

// ── DryCheckServiceFactoryOutput ──────────────────────────────────────────────

/// Output of [`DryCheckServiceFactoryPort::build`].
#[derive(Clone)]
pub struct DryCheckServiceFactoryOutput {
    /// Fully-wired dry-check application service.
    pub service: Arc<dyn DryCheckService>,
    /// Per-tier telemetry recorder bound to the same service instance.
    pub tier_telemetry: Arc<dyn DryTierTelemetryPort>,
}

// ── DryCorpusRootManifestError ────────────────────────────────────────────────

/// Error returned by [`DryCorpusRootManifestWriterPort::write`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryCorpusRootManifestError {
    /// `repo_root` could not be canonicalized.
    #[error("failed to canonicalize repo root '{}': {detail}", repo_root.display())]
    RepoRootCanonicalizeFailed {
        /// The repo root that failed to canonicalize.
        repo_root: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// The resolved manifest path escapes `canonical_root`.
    #[error(
        "dry corpus root manifest path '{}' resolves outside repo root '{}'",
        manifest_path.display(),
        canonical_root.display()
    )]
    ManifestPathOutsideRepo {
        /// The out-of-bounds manifest path.
        manifest_path: PathBuf,
        /// The canonicalized repo root the path escaped.
        canonical_root: PathBuf,
    },
    /// `serde_json::to_vec_pretty` failed on the manifest DTO.
    #[error(
        "failed to serialize dry corpus root manifest '{}': {detail}",
        manifest_path.display()
    )]
    ManifestSerializeFailed {
        /// The manifest path being serialized for.
        manifest_path: PathBuf,
        /// Rendered `serde_json::Error` diagnostic text.
        detail: SerializationFailureDetail,
    },
    /// The symlink guard rejected either the manifest's parent directory or
    /// the manifest path itself.
    #[error("symlink guard dry corpus root manifest {}: {detail}", path.display())]
    ManifestSymlinkRejected {
        /// The rejected path (parent directory or manifest file).
        path: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// `std::fs::create_dir_all` failed for the manifest's parent directory.
    #[error(
        "failed to create dry corpus root manifest parent '{}': {detail}",
        parent.display()
    )]
    ManifestParentCreateFailed {
        /// The parent directory that failed to be created.
        parent: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// `atomic_write_file` failed for the manifest itself.
    #[error("failed to write dry corpus root manifest '{}': {detail}", manifest_path.display())]
    ManifestWriteFailed {
        /// The manifest path that failed to write.
        manifest_path: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
}

// ── DryWriteConfigLoaderPort ──────────────────────────────────────────────────

/// Secondary port resolving the effective dry-check config, threshold, and
/// config fingerprint for a `dry write` run.
///
/// Reuses the existing [`DryCheckConfigLoaderError`] from
/// [`crate::fixpoint_resolve_driver`] rather than declaring a near-duplicate.
pub trait DryWriteConfigLoaderPort: Send + Sync {
    /// Load `.harness/config/dry-check.json`, resolve the effective threshold
    /// (override or file default), and select `config_fingerprint` via the
    /// safe-approval rule: a stricter-or-equal override uses the plain file
    /// fingerprint, a looser override uses the threshold-qualified fingerprint.
    ///
    /// # Errors
    ///
    /// Returns a [`DryCheckConfigLoaderError`] variant on load or
    /// validation failure.
    fn load(
        &self,
        repo_root: &Path,
        threshold_override: Option<SimilarityThreshold>,
    ) -> Result<DryWriteConfigResolution, DryCheckConfigLoaderError>;
}

// ── DryCorpusFragmentsPort ────────────────────────────────────────────────────

/// Secondary port building the diff/corpus fragment sets for a `dry write` run.
pub trait DryCorpusFragmentsPort: Send + Sync {
    /// Validate `workspace_root` is an existing directory under
    /// `canonical_root`, list changed git hunks since `base`, extract the
    /// whole-workspace corpus, normalize fragment paths to repo-relative
    /// form, narrow `diff_fragments` to hunk-overlapping candidates, and
    /// compute the corpus fingerprint.
    ///
    /// # Errors
    ///
    /// Returns a [`DryCorpusFragmentsError`] variant on workspace-root
    /// validation, diff listing, or fragment extraction failure.
    fn build(
        &self,
        base: &domain::CommitHash,
        workspace_root: &Path,
        repo_root: &Path,
        canonical_root: &Path,
    ) -> Result<DryCorpusFragmentsOutput, DryCorpusFragmentsError>;
}

// ── DryCheckServiceFactoryPort ────────────────────────────────────────────────

/// Secondary port constructing a fully-wired [`DryCheckService`] plus a
/// [`DryTierTelemetryPort`] bound to the same tiered recorder embedded in the
/// constructed agent.
///
/// Does NOT invoke `run_dry_check` itself.
///
/// Implementations are infrastructure composition points: they may resolve
/// embedding/index/agent resources and may pre-resolve telemetry gating from
/// adapter-owned runtime state such as the current git branch, environment
/// configuration, and an adapter-managed monotonic timing anchor. Those runtime
/// reads are part of this secondary-port contract, not hidden usecase work.
pub trait DryCheckServiceFactoryPort: Send + Sync {
    /// Build the dry-check service and its tier-telemetry recorder.
    ///
    /// # Errors
    ///
    /// Returns a [`DryCheckServiceFactoryError`] variant on
    /// embedding-load, index-open, or agent-profile resolution failure.
    fn build(
        &self,
        cmd: DryCheckServiceFactoryCommand,
    ) -> Result<DryCheckServiceFactoryOutput, DryCheckServiceFactoryError>;
}

// ── DryCorpusRootManifestWriterPort ───────────────────────────────────────────

/// Secondary port persisting `<track_dir>/dry-check-corpus-root.json`.
pub trait DryCorpusRootManifestWriterPort: Send + Sync {
    /// Persist the corpus-root manifest recording the scanned `workspace_root`.
    ///
    /// # Errors
    ///
    /// Returns a [`DryCorpusRootManifestError`] variant on serialization
    /// or filesystem write failure.
    fn write(
        &self,
        track_dir: &Path,
        workspace_root: &Path,
        repo_root: &Path,
    ) -> Result<(), DryCorpusRootManifestError>;
}

// ── DryTierTelemetryPort ──────────────────────────────────────────────────────

/// Secondary port recording per-tier `ReviewRound`/`ExternalSubprocess`
/// telemetry for a `dry write` run.
///
/// Infallible, fire-and-forget.
///
/// Implementations may write telemetry and may use adapter-owned runtime
/// context captured by [`DryCheckServiceFactoryPort::build`] (for example
/// branch/env gating and monotonic or wall-clock timestamps). The interactor
/// only hands over the domain dry-check result; all external effects remain
/// behind this documented secondary port.
pub trait DryTierTelemetryPort: Send + Sync {
    /// Record telemetry for the given dry-check cycle result.
    fn record(&self, dry_result: &Result<Vec<DryCheckFinding>, DryCheckCycleError>);
}

// ── DryWriteDriverService ─────────────────────────────────────────────────────

/// Application service (primary port) for `sotp dry write`.
///
/// Reuses the pre-existing [`DryWriteDriverInput`]/[`DryWriteOutcome`] DTOs
/// from [`crate::dry_driver`], already the type consumed/produced at the
/// `cli_driver` boundary.
pub trait DryWriteDriverService: Send + Sync {
    /// Run `sotp dry write`.
    fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome;
}

// ── DryWriteDriverInteractor ──────────────────────────────────────────────────

/// Interactor implementing [`DryWriteDriverService`].
///
/// Holds the eight secondary ports below as private `Arc<dyn Port>` fields and
/// performs the entire dry-write orchestration itself (ADR 2026-06-21-1328
/// D4) — no infrastructure adapter sits between this interactor and the
/// domain-level `run_dry_check` call.
pub struct DryWriteDriverInteractor {
    repo_root: Arc<dyn DryRepoRootPort>,
    base_branch: Arc<dyn DryBaseBranchPort>,
    diff_base_resolver_factory: Arc<dyn DryDiffBaseFactoryPort>,
    config_loader: Arc<dyn DryWriteConfigLoaderPort>,
    corpus_fragments: Arc<dyn DryCorpusFragmentsPort>,
    storage_factory: Arc<dyn DryCheckStorageFactoryPort>,
    service_factory: Arc<dyn DryCheckServiceFactoryPort>,
    corpus_root_manifest: Arc<dyn DryCorpusRootManifestWriterPort>,
}

impl DryWriteDriverInteractor {
    /// Construct a new [`DryWriteDriverInteractor`] with all required ports.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo_root: Arc<dyn DryRepoRootPort>,
        base_branch: Arc<dyn DryBaseBranchPort>,
        diff_base_resolver_factory: Arc<dyn DryDiffBaseFactoryPort>,
        config_loader: Arc<dyn DryWriteConfigLoaderPort>,
        corpus_fragments: Arc<dyn DryCorpusFragmentsPort>,
        storage_factory: Arc<dyn DryCheckStorageFactoryPort>,
        service_factory: Arc<dyn DryCheckServiceFactoryPort>,
        corpus_root_manifest: Arc<dyn DryCorpusRootManifestWriterPort>,
    ) -> Self {
        Self {
            repo_root,
            base_branch,
            diff_base_resolver_factory,
            config_loader,
            corpus_fragments,
            storage_factory,
            service_factory,
            corpus_root_manifest,
        }
    }
}

impl DryWriteDriverService for DryWriteDriverInteractor {
    fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome {
        // ── (1) Validate track_id and the optional --threshold override. ───────
        let track_id = match TrackId::try_new(input.track_id) {
            Ok(id) => id,
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };
        let threshold_override = match input.threshold {
            Some(t) => match SimilarityThreshold::new(t) {
                Ok(v) => Some(v),
                Err(e) => {
                    return DryWriteOutcome::Failure {
                        message: format!("invalid --threshold: {e}"),
                    };
                }
            },
            None => None,
        };

        // ── (2) Resolve workspace paths. ────────────────────────────────────────
        let workspace = match self.repo_root.resolve(&input.items_dir) {
            Ok(w) => w,
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };

        // ── (3) Derive track_dir. ────────────────────────────────────────────────
        let track_dir = workspace.canonical_items_dir.join(track_id.as_ref());

        // ── (4) Resolve the diff base commit (override or store fallback). ─────
        let base = match input.base_commit {
            Some(ref s) => match CommitHash::try_new(s) {
                Ok(hash) => hash,
                Err(e) => {
                    return DryWriteOutcome::Failure {
                        message: format!("invalid --base-commit: {e}"),
                    };
                }
            },
            None => {
                let base_branch = match self.base_branch.resolve_base_branch(
                    &workspace.canonical_root,
                    &track_dir,
                    &track_id,
                ) {
                    Ok(b) => b,
                    Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
                };
                let resolver = self.diff_base_resolver_factory.build(&base_branch);
                match resolver.resolve_diff_base(
                    &track_dir,
                    &workspace.canonical_root,
                    &workspace.repo_root,
                ) {
                    Ok(hash) => hash,
                    Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
                }
            }
        };

        // ── (6) Resolve config + effective threshold + fingerprint. ────────────
        let config_resolution =
            match self.config_loader.load(&workspace.repo_root, threshold_override) {
                Ok(v) => v,
                Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
            };

        // ── (7) Build diff/corpus fragments. ─────────────────────────────────────
        let fragments_output = match self.corpus_fragments.build(
            &base,
            &input.workspace_root,
            &workspace.repo_root,
            &workspace.canonical_root,
        ) {
            Ok(v) => v,
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };
        let diff_fragments_processed = fragments_output.diff_fragments.len();

        // ── (8) Build the reader/writer/coverage trio; read records_before. ────
        let storage = self.storage_factory.build(&track_dir, &workspace.canonical_root);
        let records_before = match storage.reader.read_records() {
            Ok(records) => records.len(),
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };

        // ── (9) Write the corpus-root sidecar unconditionally. ──────────────────
        if let Err(e) =
            self.corpus_root_manifest.write(&track_dir, &input.workspace_root, &workspace.repo_root)
        {
            return DryWriteOutcome::Failure { message: e.to_string() };
        }

        // ── (10) Construct the fully-wired dry-check service. ───────────────────
        let cmd = DryCheckServiceFactoryCommand {
            reader: storage.reader.clone(),
            writer: storage.writer.clone(),
            coverage: storage.coverage.clone(),
            track_id: track_id.clone(),
            repo_root: workspace.repo_root.clone(),
            items_dir: workspace.canonical_items_dir.clone(),
            db_path: input.db_path,
            corpus_fragments: fragments_output.corpus_fragments,
            model: input.model,
            capability_name: input.capability_name,
            dry_config: config_resolution.dry_config,
            config_fingerprint: config_resolution.config_fingerprint,
            corpus_fingerprint: fragments_output.corpus_fingerprint,
        };
        let service_output = match self.service_factory.build(cmd) {
            Ok(v) => v,
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };

        // ── (11) Invoke the domain-level write cycle directly. ──────────────────
        // Empty `corpus_fragments` (`vec![]`): the semantic index was already
        // fully populated by `service_factory.build` (D7/IN-10 NullInsertIndexProxy
        // makes the interactor's own `build_corpus_index` call a no-op).
        let dry_result = service_output.service.run_dry_check(
            vec![],
            fragments_output.diff_fragments,
            config_resolution.effective_threshold,
            base,
        );

        // ── (12) Record tier telemetry unconditionally, before outcome mapping. ─
        service_output.tier_telemetry.record(&dry_result);

        // ── (13) Map the result into the outcome. ────────────────────────────────
        let findings = match dry_result {
            Ok(findings) => findings,
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };

        let records_after = match storage.reader.read_records() {
            Ok(records) => records.len(),
            Err(e) => return DryWriteOutcome::Failure { message: e.to_string() },
        };
        let records_appended = records_after.saturating_sub(records_before);
        let pairs_checked = records_appended;

        let findings: Vec<DryWriteFindingSummary> = findings
            .into_iter()
            .map(|finding| DryWriteFindingSummary {
                changed_path: finding.changed_fragment_ref().path().as_str().to_owned(),
                changed_content_hash: finding
                    .changed_fragment_ref()
                    .content_hash()
                    .as_str()
                    .to_owned(),
                candidate_path: finding.candidate_fragment_ref().path().as_str().to_owned(),
                candidate_content_hash: finding
                    .candidate_fragment_ref()
                    .content_hash()
                    .as_str()
                    .to_owned(),
                refactor_proposal: finding.refactor_proposal().as_str().to_owned(),
            })
            .collect();

        DryWriteOutcome::Success {
            pairs_checked,
            records_appended,
            diff_fragments_processed,
            findings,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::dry_check::{DryCheckCoverageRecord, DryCheckReaderError, DryCheckRecord};

    use super::*;
    use crate::dry_check::{DryCheckParallelism, DryCheckPercent};
    use crate::dry_driver_shared::{
        DryBaseBranchError, DryCheckStorageHandle, DryRepoWorkspace, DryRepoWorkspaceError,
        GitDiscoveryFailureDetail,
    };
    use crate::fixpoint_resolve::{DiffBaseResolverError, DiffBaseResolverPort};
    use crate::fixpoint_resolve_driver::DryCheckConfigLoadFailureDetail;

    // ── Test doubles ─────────────────────────────────────────────────────────

    struct StubRepoRoot {
        result: Result<DryRepoWorkspace, DryRepoWorkspaceError>,
    }

    impl DryRepoRootPort for StubRepoRoot {
        fn resolve(&self, _items_dir: &Path) -> Result<DryRepoWorkspace, DryRepoWorkspaceError> {
            self.result.clone()
        }
    }

    struct StubBaseBranch {
        result: Result<String, DryBaseBranchError>,
    }

    impl DryBaseBranchPort for StubBaseBranch {
        fn resolve_base_branch(
            &self,
            _canonical_root: &Path,
            _track_dir: &Path,
            _track_id: &TrackId,
        ) -> Result<String, DryBaseBranchError> {
            self.result.clone()
        }
    }

    struct StubDiffBaseResolver {
        result: Result<CommitHash, DiffBaseResolverError>,
    }

    impl DiffBaseResolverPort for StubDiffBaseResolver {
        fn resolve_diff_base(
            &self,
            _track_dir: &Path,
            _canonical_root: &Path,
            _repo_root: &Path,
        ) -> Result<CommitHash, DiffBaseResolverError> {
            self.result.clone()
        }
    }

    struct StubDiffBaseFactory {
        result: Result<CommitHash, DiffBaseResolverError>,
    }

    impl DryDiffBaseFactoryPort for StubDiffBaseFactory {
        fn build(&self, _base_branch: &str) -> Arc<dyn DiffBaseResolverPort> {
            Arc::new(StubDiffBaseResolver { result: self.result.clone() })
        }
    }

    struct PanicIfCalledDiffBaseFactory;

    impl DryDiffBaseFactoryPort for PanicIfCalledDiffBaseFactory {
        fn build(&self, _base_branch: &str) -> Arc<dyn DiffBaseResolverPort> {
            panic!("diff_base_resolver_factory must not be called when --base-commit is provided")
        }
    }

    struct StubConfigLoader {
        result: Result<DryWriteConfigResolution, DryCheckConfigLoaderError>,
    }

    impl DryWriteConfigLoaderPort for StubConfigLoader {
        fn load(
            &self,
            _repo_root: &Path,
            _threshold_override: Option<SimilarityThreshold>,
        ) -> Result<DryWriteConfigResolution, DryCheckConfigLoaderError> {
            self.result.clone()
        }
    }

    struct StubCorpusFragments {
        result: Result<DryCorpusFragmentsOutput, DryCorpusFragmentsError>,
    }

    impl DryCorpusFragmentsPort for StubCorpusFragments {
        fn build(
            &self,
            _base: &CommitHash,
            _workspace_root: &Path,
            _repo_root: &Path,
            _canonical_root: &Path,
        ) -> Result<DryCorpusFragmentsOutput, DryCorpusFragmentsError> {
            self.result.clone()
        }
    }

    struct StubReader {
        records: Vec<DryCheckRecord>,
    }

    impl domain::dry_check::DryCheckReader for StubReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Ok(self.records.clone())
        }
    }

    struct ErrorReader;

    impl domain::dry_check::DryCheckReader for ErrorReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Err(DryCheckReaderError::Io {
                path: "dry-check.json".to_owned(),
                detail: "boom".to_owned(),
            })
        }
    }

    struct NoopWriter;

    impl domain::dry_check::DryCheckWriter for NoopWriter {
        fn append_record(
            &self,
            _entry: &domain::dry_check::DryCheckEntry,
        ) -> Result<(), domain::dry_check::DryCheckWriterError> {
            Ok(())
        }
    }

    struct NoopCoverage;

    impl DryCheckCoveragePort for NoopCoverage {
        fn read_coverage(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
            Ok(None)
        }

        fn write_coverage(
            &self,
            _track_id: &TrackId,
            _record: DryCheckCoverageRecord,
        ) -> Result<(), DryCheckCycleError> {
            Ok(())
        }
    }

    struct StubStorageFactory {
        record_count: usize,
        fail_reader: bool,
    }

    impl DryCheckStorageFactoryPort for StubStorageFactory {
        fn build(&self, _track_dir: &Path, _canonical_root: &Path) -> DryCheckStorageHandle {
            let reader: Arc<dyn domain::dry_check::DryCheckReader> = if self.fail_reader {
                Arc::new(ErrorReader)
            } else {
                Arc::new(StubReader { records: vec![make_dummy_record(); self.record_count] })
            };
            DryCheckStorageHandle {
                reader,
                writer: Arc::new(NoopWriter),
                coverage: Arc::new(NoopCoverage),
            }
        }
    }

    struct StubDryCheckService {
        result: Vec<DryCheckFinding>,
    }

    impl DryCheckService for StubDryCheckService {
        fn run_dry_check(
            &self,
            _corpus_fragments: Vec<CodeFragment>,
            _diff_fragments: Vec<CodeFragment>,
            _threshold: SimilarityThreshold,
            _base_commit: CommitHash,
        ) -> Result<Vec<DryCheckFinding>, DryCheckCycleError> {
            Ok(self.result.clone())
        }
    }

    struct NoopTierTelemetry;

    impl DryTierTelemetryPort for NoopTierTelemetry {
        fn record(&self, _dry_result: &Result<Vec<DryCheckFinding>, DryCheckCycleError>) {}
    }

    struct StubServiceFactory {
        findings: Vec<DryCheckFinding>,
    }

    impl DryCheckServiceFactoryPort for StubServiceFactory {
        fn build(
            &self,
            _cmd: DryCheckServiceFactoryCommand,
        ) -> Result<DryCheckServiceFactoryOutput, DryCheckServiceFactoryError> {
            Ok(DryCheckServiceFactoryOutput {
                service: Arc::new(StubDryCheckService { result: self.findings.clone() }),
                tier_telemetry: Arc::new(NoopTierTelemetry),
            })
        }
    }

    struct FailingServiceFactory;

    impl DryCheckServiceFactoryPort for FailingServiceFactory {
        fn build(
            &self,
            _cmd: DryCheckServiceFactoryCommand,
        ) -> Result<DryCheckServiceFactoryOutput, DryCheckServiceFactoryError> {
            Err(DryCheckServiceFactoryError::EmbeddingModelLoadFailed {
                detail: EmbeddingModelLoadFailureDetail::new("service factory boom"),
            })
        }
    }

    struct StubCorpusRootManifest {
        fail: bool,
    }

    impl DryCorpusRootManifestWriterPort for StubCorpusRootManifest {
        fn write(
            &self,
            _track_dir: &Path,
            _workspace_root: &Path,
            _repo_root: &Path,
        ) -> Result<(), DryCorpusRootManifestError> {
            if self.fail {
                Err(DryCorpusRootManifestError::ManifestWriteFailed {
                    manifest_path: PathBuf::from("dry-check-corpus-root.json"),
                    detail: IoFailureDetail::new("manifest boom"),
                })
            } else {
                Ok(())
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_dummy_record() -> DryCheckRecord {
        use domain::Timestamp;
        use domain::dry_check::{
            DryCheckEntry, DryCheckPairKey, DryCheckVerdict, FragmentContentHash, FragmentRef,
            Rationale,
        };
        use domain::review_v2::FilePath;
        use domain::semantic_dup::{SimilarityScore, SimilarityThreshold as DomainThreshold};

        let low = FragmentRef::new(
            FilePath::new("src/a.rs").unwrap(),
            FragmentContentHash::new("a".repeat(64)).unwrap(),
        );
        let high = FragmentRef::new(
            FilePath::new("src/b.rs").unwrap(),
            FragmentContentHash::new("b".repeat(64)).unwrap(),
        );
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let entry = DryCheckEntry::new(
            pair_key.clone(),
            pair_key.low().path().clone(),
            DryCheckVerdict::NotAViolation,
            SimilarityScore::new(0.5).unwrap(),
            DomainThreshold::new(0.5).unwrap(),
            make_commit_hash(),
            Rationale::new("test").unwrap(),
            make_config_fingerprint(),
        )
        .unwrap();
        DryCheckRecord::from_entry_and_timestamp(
            entry,
            Timestamp::new("2026-06-30T00:00:00Z").unwrap(),
        )
        .unwrap()
    }

    fn make_commit_hash() -> CommitHash {
        CommitHash::try_new("a".repeat(40)).unwrap()
    }

    fn make_test_track_id() -> TrackId {
        TrackId::try_new("test-track-2026").unwrap()
    }

    fn make_config_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("c".repeat(64)).unwrap()
    }

    fn make_corpus_fingerprint() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint::new("d".repeat(64)).unwrap()
    }

    fn test_dry_config() -> DryCheckConfig {
        DryCheckConfig::new(
            DryCheckPercent::try_new(10).unwrap(),
            DryCheckPercent::try_new(90).unwrap(),
            DryCheckParallelism::try_new(4).unwrap(),
            true,
        )
    }

    fn make_workspace() -> DryRepoWorkspace {
        DryRepoWorkspace {
            repo_root: PathBuf::from("/repo"),
            canonical_root: PathBuf::from("/repo"),
            canonical_items_dir: PathBuf::from("/repo/track/items"),
        }
    }

    fn make_input() -> DryWriteDriverInput {
        DryWriteDriverInput {
            track_id: "test-track-2026".to_owned(),
            base_commit: None,
            db_path: PathBuf::from("/tmp/db"),
            threshold: None,
            workspace_root: PathBuf::from("/repo"),
            items_dir: PathBuf::from("track/items"),
            model: None,
            capability_name: "dry-checker".to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_interactor(
        repo_root: Result<DryRepoWorkspace, DryRepoWorkspaceError>,
        base_branch: Result<String, DryBaseBranchError>,
        diff_base: Result<CommitHash, DiffBaseResolverError>,
        config: Result<DryWriteConfigResolution, DryCheckConfigLoaderError>,
        fragments: Result<DryCorpusFragmentsOutput, DryCorpusFragmentsError>,
        record_count: usize,
        fail_reader: bool,
        service_factory: Arc<dyn DryCheckServiceFactoryPort>,
        manifest_fail: bool,
    ) -> DryWriteDriverInteractor {
        DryWriteDriverInteractor::new(
            Arc::new(StubRepoRoot { result: repo_root }),
            Arc::new(StubBaseBranch { result: base_branch }),
            Arc::new(StubDiffBaseFactory { result: diff_base }),
            Arc::new(StubConfigLoader { result: config }),
            Arc::new(StubCorpusFragments { result: fragments }),
            Arc::new(StubStorageFactory { record_count, fail_reader }),
            service_factory,
            Arc::new(StubCorpusRootManifest { fail: manifest_fail }),
        )
    }

    fn happy_path_interactor(findings: Vec<DryCheckFinding>) -> DryWriteDriverInteractor {
        make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings }),
            false,
        )
    }

    // ── Success path ─────────────────────────────────────────────────────────

    #[test]
    fn dry_write_success_path_with_empty_findings_returns_success_outcome() {
        let interactor = happy_path_interactor(vec![]);
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Success {
                pairs_checked,
                records_appended,
                diff_fragments_processed,
                findings,
            } => {
                assert_eq!(pairs_checked, 0);
                assert_eq!(records_appended, 0);
                assert_eq!(diff_fragments_processed, 0);
                assert!(findings.is_empty());
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_records_appended_reflects_reader_delta() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            3,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Success { pairs_checked, records_appended, .. } => {
                // The stub reader returns the same fixed record_count on every
                // call, so records_before == records_after and the delta is 0
                // (verifies the saturating_sub composition, not the specific I/O).
                assert_eq!(records_appended, 0);
                assert_eq!(pairs_checked, records_appended);
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    // ── --base-commit override skips base-branch and diff-base resolution ──────

    #[test]
    fn dry_write_base_commit_override_skips_diff_base_resolver_call() {
        let interactor = DryWriteDriverInteractor::new(
            Arc::new(StubRepoRoot { result: Ok(make_workspace()) }),
            Arc::new(StubBaseBranch {
                result: Err(DryBaseBranchError::MetadataNotFound {
                    track_id: make_test_track_id(),
                }),
            }),
            Arc::new(PanicIfCalledDiffBaseFactory),
            Arc::new(StubConfigLoader {
                result: Ok(DryWriteConfigResolution {
                    dry_config: test_dry_config(),
                    effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                    config_fingerprint: make_config_fingerprint(),
                }),
            }),
            Arc::new(StubCorpusFragments {
                result: Ok(DryCorpusFragmentsOutput {
                    diff_fragments: vec![],
                    corpus_fragments: vec![],
                    corpus_fingerprint: make_corpus_fingerprint(),
                }),
            }),
            Arc::new(StubStorageFactory { record_count: 0, fail_reader: false }),
            Arc::new(StubServiceFactory { findings: vec![] }),
            Arc::new(StubCorpusRootManifest { fail: false }),
        );
        let mut input = make_input();
        input.base_commit = Some("b".repeat(40));

        let outcome = interactor.dry_write(input);

        assert!(
            matches!(outcome, DryWriteOutcome::Success { .. }),
            "expected Success (no panic from diff_base_resolver_factory), got {outcome:?}"
        );
    }

    #[test]
    fn dry_write_invalid_base_commit_override_returns_failure() {
        let interactor = happy_path_interactor(vec![]);
        let mut input = make_input();
        input.base_commit = Some("not-a-hash".to_owned());

        let outcome = interactor.dry_write(input);

        match outcome {
            DryWriteOutcome::Failure { message } => {
                assert!(message.contains("invalid --base-commit"), "got: {message}");
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    // ── --threshold validation ───────────────────────────────────────────────

    #[test]
    fn dry_write_invalid_threshold_returns_failure_before_any_port_call() {
        let interactor = make_interactor(
            Err(DryRepoWorkspaceError::ItemsDirInvalid {
                items_dir: PathBuf::from("must not be called"),
            }),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let mut input = make_input();
        input.threshold = Some(2.0);

        let outcome = interactor.dry_write(input);

        match outcome {
            DryWriteOutcome::Failure { message } => {
                assert!(message.contains("invalid --threshold"), "got: {message}");
                assert!(!message.contains("must not be called"), "got: {message}");
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    // ── Per-port failure propagation ─────────────────────────────────────────

    #[test]
    fn dry_write_invalid_track_id_returns_failure() {
        let interactor = happy_path_interactor(vec![]);
        let mut input = make_input();
        input.track_id = String::new();

        let outcome = interactor.dry_write(input);
        assert!(matches!(outcome, DryWriteOutcome::Failure { .. }));
    }

    #[test]
    fn dry_write_repo_root_failure_returns_failure() {
        let interactor = make_interactor(
            Err(DryRepoWorkspaceError::GitDiscoveryFailed {
                detail: GitDiscoveryFailureDetail::new("repo boom"),
            }),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => assert!(message.contains("repo boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_base_branch_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Err(DryBaseBranchError::MetadataReadFailed {
                track_id: make_test_track_id(),
                detail: IoFailureDetail::new("base branch boom"),
            }),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => assert!(message.contains("base branch boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_diff_base_resolver_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Err(DiffBaseResolverError::Unavailable("diff base boom".to_owned())),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => assert!(message.contains("diff base boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_config_loader_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Err(DryCheckConfigLoaderError::ConfigLoadFailed {
                config_path: PathBuf::from(".harness/config/dry-check.json"),
                detail: DryCheckConfigLoadFailureDetail::new("config boom"),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => assert!(message.contains("config boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_corpus_fragments_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Err(DryCorpusFragmentsError::DiffHunkListingFailed {
                detail: DiffHunkListingFailureDetail::new("fragments boom"),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => assert!(message.contains("fragments boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_storage_reader_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            true,
            Arc::new(StubServiceFactory { findings: vec![] }),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        assert!(matches!(outcome, DryWriteOutcome::Failure { .. }));
    }

    #[test]
    fn dry_write_corpus_root_manifest_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(StubServiceFactory { findings: vec![] }),
            true,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => assert!(message.contains("manifest boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_service_factory_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(FailingServiceFactory),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => {
                assert!(message.contains("service factory boom"))
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_write_run_dry_check_failure_returns_failure() {
        struct FailingServiceFactoryAtRunCycle;
        impl DryCheckServiceFactoryPort for FailingServiceFactoryAtRunCycle {
            fn build(
                &self,
                _cmd: DryCheckServiceFactoryCommand,
            ) -> Result<DryCheckServiceFactoryOutput, DryCheckServiceFactoryError> {
                struct FailingService;
                impl DryCheckService for FailingService {
                    fn run_dry_check(
                        &self,
                        _corpus_fragments: Vec<CodeFragment>,
                        _diff_fragments: Vec<CodeFragment>,
                        _threshold: SimilarityThreshold,
                        _base_commit: CommitHash,
                    ) -> Result<Vec<DryCheckFinding>, DryCheckCycleError> {
                        Err(DryCheckCycleError::InvalidParallelism)
                    }
                }
                Ok(DryCheckServiceFactoryOutput {
                    service: Arc::new(FailingService),
                    tier_telemetry: Arc::new(NoopTierTelemetry),
                })
            }
        }

        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok(DryWriteConfigResolution {
                dry_config: test_dry_config(),
                effective_threshold: SimilarityThreshold::new(0.85).unwrap(),
                config_fingerprint: make_config_fingerprint(),
            }),
            Ok(DryCorpusFragmentsOutput {
                diff_fragments: vec![],
                corpus_fragments: vec![],
                corpus_fingerprint: make_corpus_fingerprint(),
            }),
            0,
            false,
            Arc::new(FailingServiceFactoryAtRunCycle),
            false,
        );
        let outcome = interactor.dry_write(make_input());
        match outcome {
            DryWriteOutcome::Failure { message } => {
                assert!(message.contains("invalid dry-check parallelism"), "got: {message}");
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }
}
