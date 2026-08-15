//! Infrastructure adapters implementing the five `usecase::dry_write_driver`
//! secondary ports.
//!
//! Each adapter performs a single narrow I/O concern (ADR 2026-06-21-1328 D7 /
//! R1: `SecondaryAdapter` is infrastructure-only) — the orchestration itself
//! lives in [`usecase::dry_write_driver::DryWriteDriverInteractor`] (T027).
//! This module is purely additive: nothing outside it references these
//! adapters yet, so it compiles standalone without touching `cli_driver` or
//! `cli_composition`.
//!
//! Design: ADR 2026-06-21-1328 D7, IN-14, AC-20, CN-07.

mod checker_config;
mod fragments;
mod manifest;
mod persistent_index;
mod telemetry;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use domain::CommitHash;
use domain::dry_check::DryCheckFinding;
use usecase::dry_check::{DryCheckCycleError, DryCheckInteractor, DryCheckService};
use usecase::dry_driver_shared::IoFailureDetail;
use usecase::dry_write_driver::{
    AgentConfigResolutionFailureDetail, CapabilityName, DiffHunkListingFailureDetail,
    DryCheckServiceFactoryCommand, DryCheckServiceFactoryError, DryCheckServiceFactoryOutput,
    DryCheckServiceFactoryPort, DryCorpusFragmentsError, DryCorpusFragmentsOutput,
    DryCorpusFragmentsPort, DryCorpusRootManifestError, DryCorpusRootManifestWriterPort,
    DryTierTelemetryPort, DryWriteConfigLoaderPort, DryWriteConfigResolution,
    EmbeddingModelLoadFailureDetail, FragmentPipelineFailureDetail, SemanticIndexOpenFailureDetail,
    SerializationFailureDetail,
};
use usecase::fixpoint_resolve_driver::{
    DryCheckConfigLoadFailureDetail, DryCheckConfigLoaderError,
};

use crate::capability_exec::grok::resolve_grok_capability_definition;
use crate::dry_check::CodexDryChecker;
use crate::dry_check::DryCheckConfig as InfraDryCheckConfig;
use crate::dry_check::GrokDryChecker;
use crate::dry_check::recording_agent::{RecordingDryAgent, TieredDryAgentRecorder};
use crate::semantic_dup::embedding::FastEmbedAdapter;
use crate::telemetry::TelemetryWriter;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ── FsDryWriteConfigLoaderAdapter ─────────────────────────────────────────────

/// Filesystem adapter implementing [`DryWriteConfigLoaderPort`].
///
/// Loads `.harness/config/dry-check.json`, converts to the usecase
/// `DryCheckConfig` newtype, resolves the effective `SimilarityThreshold`
/// (override or file default), and selects `config_fingerprint` via the
/// safe-approval rule — reproducing `DryCompositionRoot`'s private
/// `resolve_write_config_fingerprint` helper exactly.
pub struct FsDryWriteConfigLoaderAdapter;

impl DryWriteConfigLoaderPort for FsDryWriteConfigLoaderAdapter {
    fn load(
        &self,
        repo_root: &Path,
        threshold_override: Option<domain::semantic_dup::SimilarityThreshold>,
    ) -> Result<DryWriteConfigResolution, DryCheckConfigLoaderError> {
        let canonical_root = repo_root.canonicalize().map_err(|e| {
            DryCheckConfigLoaderError::RepoRootCanonicalizeFailed {
                repo_root: repo_root.to_path_buf(),
                detail: IoFailureDetail::new(e.to_string()),
            }
        })?;
        let config_path = canonical_root.join(".harness/config/dry-check.json");
        let canonical_config_path = config_path.canonicalize().map_err(|e| {
            DryCheckConfigLoaderError::ConfigPathCanonicalizeFailed {
                config_path: config_path.clone(),
                detail: IoFailureDetail::new(e.to_string()),
            }
        })?;
        if !canonical_config_path.starts_with(&canonical_root) {
            return Err(DryCheckConfigLoaderError::ConfigPathOutsideRepo {
                config_path,
                canonical_root,
            });
        }
        reject_symlinks_below(&config_path, &canonical_root).map_err(|e| {
            DryCheckConfigLoaderError::ConfigSymlinkRejected {
                config_path: config_path.clone(),
                detail: IoFailureDetail::new(e.to_string()),
            }
        })?;
        let infra_config = InfraDryCheckConfig::load(&canonical_config_path).map_err(|e| {
            DryCheckConfigLoaderError::ConfigLoadFailed {
                config_path: config_path.clone(),
                detail: DryCheckConfigLoadFailureDetail::new(e.to_string()),
            }
        })?;

        let effective_threshold = threshold_override.unwrap_or_else(|| infra_config.threshold());

        let dry_config =
            checker_config::build_usecase_dry_check_config(&infra_config).map_err(|e| match e {
                checker_config::BuildDryCheckConfigError::InvalidKnownBadPercent(value) => {
                    DryCheckConfigLoaderError::InvalidKnownBadPercent { value }
                }
                checker_config::BuildDryCheckConfigError::InvalidMaxParallelism(
                    configured_value,
                ) => DryCheckConfigLoaderError::InvalidMaxParallelism { configured_value },
            })?;

        // Safe-approval rule: a stricter-or-equal override uses the plain file
        // fingerprint (so `dry check-approved` can match); a looser override
        // uses the threshold-qualified fingerprint (so `dry check-approved`
        // sees a mismatch and correctly blocks approval).
        let config_fingerprint = if effective_threshold.value() > infra_config.threshold().value() {
            infra_config.fingerprint_with_threshold(effective_threshold)
        } else {
            infra_config.fingerprint()
        };

        Ok(DryWriteConfigResolution { dry_config, effective_threshold, config_fingerprint })
    }
}

// ── FsDryCorpusFragmentsAdapter ───────────────────────────────────────────────

/// Filesystem/git adapter implementing [`DryCorpusFragmentsPort`].
///
/// Validates `workspace_root` is an existing directory under `canonical_root`,
/// then reproduces `apps/cli-composition/src/dry/shared.rs::build_diff_and_corpus_fragments`'s
/// pipeline.
pub struct FsDryCorpusFragmentsAdapter;

impl DryCorpusFragmentsPort for FsDryCorpusFragmentsAdapter {
    fn build(
        &self,
        base: &CommitHash,
        workspace_root: &Path,
        repo_root: &Path,
        canonical_root: &Path,
    ) -> Result<DryCorpusFragmentsOutput, DryCorpusFragmentsError> {
        let absolute_workspace_root = if workspace_root.is_absolute() {
            workspace_root.to_path_buf()
        } else {
            repo_root.join(workspace_root)
        };
        reject_symlinks_below(&absolute_workspace_root, canonical_root).map_err(|e| {
            DryCorpusFragmentsError::WorkspaceRootSymlinkRejected {
                workspace_root: workspace_root.to_path_buf(),
                detail: IoFailureDetail::new(e.to_string()),
            }
        })?;
        let canonical_workspace_root = absolute_workspace_root.canonicalize().map_err(|_| {
            DryCorpusFragmentsError::WorkspaceRootInvalid {
                workspace_root: workspace_root.to_path_buf(),
            }
        })?;
        if !canonical_workspace_root.is_dir()
            || !canonical_workspace_root.starts_with(canonical_root)
        {
            return Err(DryCorpusFragmentsError::WorkspaceRootInvalid {
                workspace_root: workspace_root.to_path_buf(),
            });
        }

        let (diff_fragments, corpus_fragments) = fragments::build_diff_and_corpus_fragments(
            base,
            &canonical_workspace_root,
            canonical_root,
        )
        .map_err(|e| match e {
            fragments::FragmentsBuildError::DiffHunkListing(detail) => {
                DryCorpusFragmentsError::DiffHunkListingFailed {
                    detail: DiffHunkListingFailureDetail::new(detail),
                }
            }
            fragments::FragmentsBuildError::FragmentExtraction(detail) => {
                DryCorpusFragmentsError::FragmentExtractionFailed {
                    detail: FragmentPipelineFailureDetail::new(detail),
                }
            }
            fragments::FragmentsBuildError::FragmentPathNormalization(detail) => {
                DryCorpusFragmentsError::FragmentPathNormalizationFailed {
                    detail: FragmentPipelineFailureDetail::new(detail),
                }
            }
        })?;

        let corpus_fingerprint =
            fragments::compute_dry_corpus_fingerprint_from_fragments(&corpus_fragments);

        Ok(DryCorpusFragmentsOutput { diff_fragments, corpus_fragments, corpus_fingerprint })
    }
}

/// Construct a [`CapabilityName`] from `raw`, falling back to the literal
/// `"dry-checker"` if `raw` is empty or whitespace-only.
///
/// `capability_name` reaches this construction site as an unvalidated
/// `String` (`DryCheckServiceFactoryCommand::capability_name`); every known
/// call site passes a non-empty CLI-resolved capability like `"dry-checker"`,
/// so the fallback branch is unreachable in practice. Written without
/// `unwrap`/`expect`/`panic` (denied workspace-wide) so it stays total even
/// if that assumption is ever violated.
fn parse_grok_effort(value: &str) -> Result<usecase::capability_exec::ReasoningEffort, String> {
    match value {
        "low" => Ok(usecase::capability_exec::ReasoningEffort::Low),
        "medium" => Ok(usecase::capability_exec::ReasoningEffort::Medium),
        "high" => Ok(usecase::capability_exec::ReasoningEffort::High),
        "xhigh" => Ok(usecase::capability_exec::ReasoningEffort::XHigh),
        "max" => Ok(usecase::capability_exec::ReasoningEffort::Max),
        other => Err(format!("invalid Grok dry-checker reasoning effort '{other}'")),
    }
}

fn capability_name_or_fallback(raw: String) -> CapabilityName {
    match CapabilityName::try_new(raw) {
        Ok(name) => name,
        Err(_) => match CapabilityName::try_new("dry-checker") {
            Ok(name) => name,
            Err(_) => capability_name_or_fallback("dry-checker".to_owned()),
        },
    }
}

// ── DryCheckServiceFactoryAdapter ─────────────────────────────────────────────

/// Factory adapter implementing [`DryCheckServiceFactoryPort`].
///
/// No `Fs`/`Git` prefix (per T018's `FsFixpointDryGateFactoryAdapter`-analogous
/// naming exception): it wires an embedding model, a vector index, and a
/// subprocess-backed agent — no single I/O technology dominates.
pub struct DryCheckServiceFactoryAdapter;

impl DryCheckServiceFactoryPort for DryCheckServiceFactoryAdapter {
    fn build(
        &self,
        cmd: DryCheckServiceFactoryCommand,
    ) -> Result<DryCheckServiceFactoryOutput, DryCheckServiceFactoryError> {
        let capability_name = cmd.capability_name.clone();
        let (provider, fast_model, final_model, fast_reasoning_effort, final_reasoning_effort) =
            checker_config::resolve_dry_checker_config(
                &cmd.repo_root,
                &cmd.capability_name,
                cmd.model,
            )
            .map_err(|e| DryCheckServiceFactoryError::AgentConfigResolutionFailed {
                capability_name: capability_name_or_fallback(capability_name.clone()),
                detail: AgentConfigResolutionFailureDetail::new(e),
            })?;

        let embedding_port = Arc::new(FastEmbedAdapter::new().map_err(|e| {
            DryCheckServiceFactoryError::EmbeddingModelLoadFailed {
                detail: EmbeddingModelLoadFailureDetail::new(e.to_string()),
            }
        })?);

        // D7/IN-10: persistent index keyed by file-level content hash;
        // `NullInsertIndexProxy` makes the interactor's `build_corpus_index` a
        // no-op (corpus already correct).
        let index_port = persistent_index::open_persistent_index_with_corpus(
            &cmd.db_path,
            cmd.corpus_fragments,
            embedding_port.as_ref(),
        )
        .map_err(|e| DryCheckServiceFactoryError::SemanticIndexOpenFailed {
            detail: SemanticIndexOpenFailureDetail::new(e),
        })?;

        let config_error =
            |detail: String| DryCheckServiceFactoryError::AgentConfigResolutionFailed {
                capability_name: capability_name_or_fallback(capability_name.clone()),
                detail: AgentConfigResolutionFailureDetail::new(detail),
            };
        let (agent, tiered_recorder) = match provider.as_str() {
            "codex" => {
                let (agent, recorder) = RecordingDryAgent::new(CodexDryChecker::new(
                    fast_model.clone(),
                    fast_reasoning_effort,
                    final_model.clone(),
                    final_reasoning_effort,
                    cmd.capability_name,
                ));
                (Arc::new(agent) as Arc<dyn usecase::dry_check::DryCheckAgentPort>, recorder)
            }
            "grok" => {
                let final_model_name =
                    usecase::capability_exec::ModelName::try_new(final_model.clone())
                        .map_err(|error| config_error(error.to_string()))?;
                let definition = resolve_grok_capability_definition(
                    &cmd.repo_root,
                    &cmd.capability_name,
                    &final_model_name,
                )
                .map_err(config_error)?;
                let sandbox = definition.sandbox().cloned().ok_or_else(|| {
                    config_error(
                        "Grok dry-checker definition did not resolve a grok-sandbox permission"
                            .to_owned(),
                    )
                })?;
                let (agent, recorder) = RecordingDryAgent::new(GrokDryChecker::new(
                    usecase::capability_exec::ModelName::try_new(fast_model.clone())
                        .map_err(|error| config_error(error.to_string()))?,
                    parse_grok_effort(&fast_reasoning_effort).map_err(config_error)?,
                    final_model_name,
                    parse_grok_effort(&final_reasoning_effort).map_err(config_error)?,
                    usecase::dry_write_driver::CapabilityName::try_new(cmd.capability_name.clone())
                        .map_err(|error| config_error(error.to_string()))?,
                    sandbox,
                    cmd.repo_root.clone(),
                ));
                (Arc::new(agent) as Arc<dyn usecase::dry_check::DryCheckAgentPort>, recorder)
            }
            other => {
                return Err(config_error(format!(
                    "unsupported dry-checker provider '{other}' (supported: 'codex', 'grok')"
                )));
            }
        };

        let service = Arc::new(DryCheckInteractor::new(
            embedding_port,
            index_port,
            agent,
            cmd.writer,
            cmd.reader,
            cmd.coverage,
            cmd.track_id.clone(),
            cmd.dry_config,
            cmd.config_fingerprint,
            cmd.corpus_fingerprint,
        )) as Arc<dyn DryCheckService>;

        // T013 / IN-07 / AC-09 / CN-10: branch/kill-switch-gated telemetry writer.
        let writer = telemetry::resolve_dry_write_telemetry_writer(
            &cmd.repo_root,
            &cmd.items_dir,
            cmd.track_id.as_ref(),
        );
        let tier_telemetry: Arc<dyn DryTierTelemetryPort> =
            Arc::new(RecordingDryTierTelemetryAdapter::new(
                tiered_recorder,
                provider,
                fast_model,
                final_model,
                cmd.track_id.as_ref().to_owned(),
                writer,
            ));

        Ok(DryCheckServiceFactoryOutput { service, tier_telemetry })
    }
}

// ── RecordingDryTierTelemetryAdapter ──────────────────────────────────────────

/// Adapter implementing [`DryTierTelemetryPort`].
///
/// Holds the recorder/models/track_id/writer as private fields. Constructed
/// only by [`DryCheckServiceFactoryAdapter`]'s `build` method via its own
/// `new` constructor; never constructed standalone. A `None` `writer` field
/// (construction-time gating excluded this invocation) makes `record` a
/// silent no-op.
pub struct RecordingDryTierTelemetryAdapter {
    recorder: TieredDryAgentRecorder,
    provider: String,
    fast_model: String,
    final_model: String,
    track_id: String,
    writer: Option<TelemetryWriter>,
    /// Captured at construction time (immediately before `run_dry_check` is
    /// invoked by the T027 interactor) — used as the fallback elapsed-time
    /// anchor for tiers whose precise start/duration cannot be determined.
    fallback_start: Instant,
}

impl RecordingDryTierTelemetryAdapter {
    /// Construct a new [`RecordingDryTierTelemetryAdapter`].
    pub fn new(
        recorder: TieredDryAgentRecorder,
        provider: String,
        fast_model: String,
        final_model: String,
        track_id: String,
        writer: Option<TelemetryWriter>,
    ) -> Self {
        Self {
            recorder,
            provider,
            fast_model,
            final_model,
            track_id,
            writer,
            fallback_start: Instant::now(),
        }
    }
}

impl DryTierTelemetryPort for RecordingDryTierTelemetryAdapter {
    fn record(&self, dry_result: &Result<Vec<DryCheckFinding>, DryCheckCycleError>) {
        let Some(writer) = self.writer.as_ref() else {
            return;
        };

        // T013 / IN-07 / AC-09 / CN-10: emit per-tier ReviewRound telemetry.
        let (fast_telemetry, final_telemetry) =
            telemetry::dry_tiered_telemetry_for_result(dry_result, &self.recorder);
        if let Some(ref round) = fast_telemetry {
            telemetry::emit_dry_tier_review_round(
                writer,
                &self.track_id,
                &self.provider,
                &self.fast_model,
                "fast",
                round,
                self.fallback_start,
            );
            if round.subprocess_started_at.is_some() {
                telemetry::emit_dry_tier_external_subprocess(
                    writer,
                    &self.track_id,
                    &self.provider,
                    round,
                    self.fallback_start,
                );
            }
        }
        if let Some(ref round) = final_telemetry {
            telemetry::emit_dry_tier_review_round(
                writer,
                &self.track_id,
                &self.provider,
                &self.final_model,
                "final",
                round,
                self.fallback_start,
            );
            if round.subprocess_started_at.is_some() {
                telemetry::emit_dry_tier_external_subprocess(
                    writer,
                    &self.track_id,
                    &self.provider,
                    round,
                    self.fallback_start,
                );
            }
        }
    }
}

// ── FsDryCorpusRootManifestAdapter ────────────────────────────────────────────

const DRY_CORPUS_ROOT_MANIFEST_FILE: &str = "dry-check-corpus-root.json";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct DryCorpusRootManifestDto {
    schema_version: u32,
    workspace_root: PathBuf,
}

fn workspace_root_for_manifest(workspace_root: &Path, repo_root: &Path) -> PathBuf {
    match workspace_root.strip_prefix(repo_root) {
        Ok(rel) if rel.as_os_str().is_empty() => PathBuf::from("."),
        Ok(rel) => rel.to_path_buf(),
        Err(_) => workspace_root.to_path_buf(),
    }
}

fn resolve_manifest_path_under_repo(
    track_dir: &Path,
    repo_root: &Path,
) -> Result<PathBuf, DryCorpusRootManifestError> {
    let canonical_root = repo_root.canonicalize().map_err(|e| {
        DryCorpusRootManifestError::RepoRootCanonicalizeFailed {
            repo_root: repo_root.to_path_buf(),
            detail: IoFailureDetail::new(e.to_string()),
        }
    })?;
    let manifest_path = track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE);
    let absolute_path = if manifest_path.is_absolute() {
        manifest_path
    } else {
        canonical_root.join(manifest_path)
    };

    if absolute_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
        || !absolute_path.starts_with(&canonical_root)
    {
        return Err(DryCorpusRootManifestError::ManifestPathOutsideRepo {
            manifest_path: absolute_path,
            canonical_root,
        });
    }

    Ok(absolute_path)
}

/// Filesystem adapter implementing [`DryCorpusRootManifestWriterPort`].
///
/// Reproduces `apps/cli-composition/src/dry/corpus_root.rs::write_dry_corpus_root_manifest`'s
/// serialize + atomic write + symlink-guard logic for
/// `<track_dir>/dry-check-corpus-root.json` (schema_version 1, repo-relative
/// `workspace_root`).
pub struct FsDryCorpusRootManifestAdapter;

impl DryCorpusRootManifestWriterPort for FsDryCorpusRootManifestAdapter {
    fn write(
        &self,
        track_dir: &Path,
        workspace_root: &Path,
        repo_root: &Path,
    ) -> Result<(), DryCorpusRootManifestError> {
        let manifest = DryCorpusRootManifestDto {
            schema_version: 1,
            workspace_root: workspace_root_for_manifest(workspace_root, repo_root),
        };
        let manifest_path = resolve_manifest_path_under_repo(track_dir, repo_root)?;
        // Materialize the manifest as a Value so the persisted artifact uses
        // canonical JSON key order before the atomic write.
        let value = serde_json::to_value(&manifest).map_err(|e| {
            DryCorpusRootManifestError::ManifestSerializeFailed {
                manifest_path: manifest_path.clone(),
                detail: SerializationFailureDetail::new(e.to_string()),
            }
        })?;
        let content = serde_json::to_vec_pretty(&value).map_err(|e| {
            DryCorpusRootManifestError::ManifestSerializeFailed {
                manifest_path: manifest_path.clone(),
                detail: SerializationFailureDetail::new(e.to_string()),
            }
        })?;
        if let Some(parent) = manifest_path.parent() {
            if !parent.as_os_str().is_empty() {
                reject_symlinks_below(parent, repo_root).map_err(|e| {
                    DryCorpusRootManifestError::ManifestSymlinkRejected {
                        path: parent.to_path_buf(),
                        detail: IoFailureDetail::new(e.to_string()),
                    }
                })?;
                std::fs::create_dir_all(parent).map_err(|e| {
                    DryCorpusRootManifestError::ManifestParentCreateFailed {
                        parent: parent.to_path_buf(),
                        detail: IoFailureDetail::new(e.to_string()),
                    }
                })?;
            }
        }
        reject_symlinks_below(&manifest_path, repo_root).map_err(|e| {
            DryCorpusRootManifestError::ManifestSymlinkRejected {
                path: manifest_path.clone(),
                detail: IoFailureDetail::new(e.to_string()),
            }
        })?;
        atomic_write_file(&manifest_path, &content).map_err(|e| {
            DryCorpusRootManifestError::ManifestWriteFailed {
                manifest_path: manifest_path.clone(),
                detail: IoFailureDetail::new(e.to_string()),
            }
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── FsDryWriteConfigLoaderAdapter ────────────────────────────────────────

    fn write_dry_check_config(dir: &Path) {
        let harness_config_dir = dir.join(".harness/config");
        std::fs::create_dir_all(&harness_config_dir).unwrap();
        std::fs::write(
            harness_config_dir.join("dry-check.json"),
            r#"{
  "schema_version": 4,
  "enabled": true,
  "threshold": 0.85,
  "max_parallelism": 4,
  "known_bad_injection_rate_percent": 10,
  "known_bad_detection_threshold_percent": 90
}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_parse_grok_effort_accepts_profile_values() {
        assert!(parse_grok_effort("low").is_ok());
        assert!(parse_grok_effort("high").is_ok());
        assert!(parse_grok_effort("turbo").is_err());
    }

    #[test]
    fn test_resolve_grok_capability_definition_admits_shared_sandbox_and_rejects_missing_definition()
     {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join(".agents/skills/dry-checker");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: dry-checker\ndescription: Test dry-checker.\ngrok-sandbox: workspace\n---\n",
        )
        .unwrap();
        let model = usecase::capability_exec::ModelName::try_new("grok-final".to_owned()).unwrap();
        let definition = resolve_grok_capability_definition(dir.path(), "dry-checker", &model)
            .expect("admitted definition");
        assert_eq!(definition.sandbox(), Some(&crate::grok_common::GrokSandbox::Workspace));

        std::fs::remove_file(skill_dir.join("SKILL.md")).unwrap();
        let missing = resolve_grok_capability_definition(dir.path(), "dry-checker", &model);
        assert!(missing.is_err(), "missing shared definition must fail closed");
    }

    #[test]
    fn test_fs_dry_write_config_loader_no_override_uses_file_threshold_and_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        write_dry_check_config(dir.path());

        let resolution = FsDryWriteConfigLoaderAdapter.load(dir.path(), None).unwrap();

        assert!((resolution.effective_threshold.value() - 0.85_f32).abs() < f32::EPSILON);
        let config_path = dir.path().join(".harness/config/dry-check.json");
        let infra_config = InfraDryCheckConfig::load(&config_path).unwrap();
        assert_eq!(resolution.config_fingerprint, infra_config.fingerprint());
    }

    #[test]
    fn test_fs_dry_write_config_loader_looser_override_uses_threshold_qualified_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        write_dry_check_config(dir.path());
        let looser = domain::semantic_dup::SimilarityThreshold::new(0.95).unwrap();

        let resolution = FsDryWriteConfigLoaderAdapter.load(dir.path(), Some(looser)).unwrap();

        let config_path = dir.path().join(".harness/config/dry-check.json");
        let infra_config = InfraDryCheckConfig::load(&config_path).unwrap();
        assert_eq!(resolution.config_fingerprint, infra_config.fingerprint_with_threshold(looser));
        assert_ne!(resolution.config_fingerprint, infra_config.fingerprint());
    }

    #[test]
    fn test_fs_dry_write_config_loader_stricter_override_uses_file_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        write_dry_check_config(dir.path());
        let stricter = domain::semantic_dup::SimilarityThreshold::new(0.5).unwrap();

        let resolution = FsDryWriteConfigLoaderAdapter.load(dir.path(), Some(stricter)).unwrap();

        let config_path = dir.path().join(".harness/config/dry-check.json");
        let infra_config = InfraDryCheckConfig::load(&config_path).unwrap();
        assert_eq!(resolution.config_fingerprint, infra_config.fingerprint());
    }

    #[test]
    fn test_fs_dry_write_config_loader_missing_config_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = FsDryWriteConfigLoaderAdapter.load(dir.path(), None).unwrap_err();
        assert!(err.to_string().contains("dry-check config"));
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_dry_write_config_loader_symlinked_config_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let harness_config_dir = dir.path().join(".harness/config");
        std::fs::create_dir_all(&harness_config_dir).unwrap();
        std::fs::write(
            harness_config_dir.join("real-dry-check.json"),
            r#"{
  "schema_version": 4,
  "enabled": true,
  "threshold": 0.85,
  "max_parallelism": 4,
  "known_bad_injection_rate_percent": 10,
  "known_bad_detection_threshold_percent": 90
}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(
            harness_config_dir.join("real-dry-check.json"),
            harness_config_dir.join("dry-check.json"),
        )
        .unwrap();

        let err = FsDryWriteConfigLoaderAdapter.load(dir.path(), None).unwrap_err();

        assert!(
            err.to_string().contains("symlink guard"),
            "symlinked dry-check config must fail closed, got: {err}"
        );
    }

    // ── FsDryCorpusFragmentsAdapter ──────────────────────────────────────────

    #[test]
    fn test_fs_dry_corpus_fragments_adapter_workspace_root_outside_repo_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();

        let commit = CommitHash::try_new("a".repeat(40)).unwrap();
        let err = FsDryCorpusFragmentsAdapter
            .build(&commit, outside.path(), &repo_root, &repo_root)
            .unwrap_err();

        assert!(err.to_string().contains("workspace_root"), "got: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_dry_corpus_fragments_adapter_symlinked_workspace_root_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace = repo_root.join("workspace");
        let link = repo_root.join("workspace-link");
        std::fs::create_dir_all(&workspace).unwrap();
        std::os::unix::fs::symlink(&workspace, &link).unwrap();

        let commit = CommitHash::try_new("a".repeat(40)).unwrap();
        let err =
            FsDryCorpusFragmentsAdapter.build(&commit, &link, &repo_root, &repo_root).unwrap_err();

        assert!(
            err.to_string().contains("symlink guard"),
            "symlinked workspace_root must fail closed, got: {err}"
        );
    }

    // ── FsDryCorpusRootManifestAdapter ───────────────────────────────────────

    #[test]
    fn test_fs_dry_corpus_root_manifest_adapter_writes_repo_relative_workspace_root() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();

        FsDryCorpusRootManifestAdapter.write(&track_dir, &workspace_root, &repo_root).unwrap();

        let content =
            std::fs::read_to_string(track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE)).unwrap();
        let manifest: DryCorpusRootManifestDto = serde_json::from_str(&content).unwrap();
        assert_eq!(manifest.workspace_root, PathBuf::from("apps/cli-composition"));
        assert_eq!(manifest.schema_version, 1);
    }

    #[test]
    fn test_fs_dry_corpus_root_manifest_writer_repeated_write_returns_canonical_bytes() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        let manifest_path = track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE);

        FsDryCorpusRootManifestAdapter.write(&track_dir, &workspace_root, &repo_root).unwrap();
        let first = std::fs::read(&manifest_path).unwrap();
        FsDryCorpusRootManifestAdapter.write(&track_dir, &workspace_root, &repo_root).unwrap();
        let second = std::fs::read(manifest_path).unwrap();

        assert_eq!(first, second);
        assert!(
            std::str::from_utf8(&first).unwrap().starts_with("{\n  \"schema_version\":"),
            "manifest keys must be canonical"
        );
    }

    #[test]
    fn test_fs_dry_corpus_root_manifest_adapter_creates_missing_track_dir() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let items_dir = repo_root.join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&items_dir).unwrap();

        FsDryCorpusRootManifestAdapter.write(&track_dir, &workspace_root, &repo_root).unwrap();

        assert!(track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE).is_file());
    }

    #[test]
    fn test_fs_dry_corpus_root_manifest_adapter_out_of_repo_track_dir_returns_error() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let outside_track_dir = outside.path().join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();

        let err = FsDryCorpusRootManifestAdapter
            .write(&outside_track_dir, &workspace_root, &repo_root)
            .unwrap_err();

        assert!(
            err.to_string().contains("outside repo root"),
            "out-of-repo manifest path must fail closed, got: {err}"
        );
        assert!(!outside_track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_dry_corpus_root_manifest_adapter_rejects_symlinked_sidecar() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();

        let outside_target = outside.path().join("target.json");
        std::fs::write(&outside_target, "do not overwrite").unwrap();
        std::os::unix::fs::symlink(&outside_target, track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE))
            .unwrap();

        let result = FsDryCorpusRootManifestAdapter.write(&track_dir, &workspace_root, &repo_root);

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&outside_target).unwrap(), "do not overwrite");
    }
}
