//! Filesystem-backed adapter for [`usecase::fixpoint_resolve::DryApprovalFactoryPort`].
//!
//! Relocated from `cli_composition::track::fixpoint_resolve` per ADR
//! 2026-06-21-1328 D7 (secondary port implementations belong in `libs/infrastructure`),
//! mirroring the sibling [`crate::dry_check::diff_base_resolver::FsDiffBaseResolverAdapter`]
//! relocation (IN-11).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::{DryCheckApprovalVerdict, FragmentRef};
use domain::dry_check::{DryCheckConfigFingerprint, DryCheckCorpusFingerprint};
use usecase::dry_check::{
    DryCheckApprovalInteractor, DryCheckApprovalService, DryCheckConfig, DryCheckCycleError,
};
use usecase::fixpoint_resolve::DryApprovalFactoryPort;

use crate::dry_check::{FsDryCheckCoverageAdapter, FsDryCheckStore};
use crate::track::symlink_guard::reject_symlinks_below;

fn trusted_track_artifact_path(
    track_dir: &Path,
    trusted_root: &Path,
    artifact_name: &str,
) -> Result<PathBuf, String> {
    let canonical_root = trusted_root.canonicalize().map_err(|e| {
        format!("cannot canonicalize trusted root '{}': {e}", trusted_root.display())
    })?;
    let absolute_track_dir = if track_dir.is_absolute() {
        track_dir.to_path_buf()
    } else {
        canonical_root.join(track_dir)
    };
    let canonical_track_dir = absolute_track_dir
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize track dir '{}': {e}", track_dir.display()))?;

    if !canonical_track_dir.starts_with(&canonical_root) {
        return Err(format!(
            "track dir '{}' resolves outside trusted root '{}'",
            track_dir.display(),
            canonical_root.display()
        ));
    }

    let raw_artifact_path = absolute_track_dir.join(artifact_name);
    reject_symlinks_below(&raw_artifact_path, &canonical_root).map_err(|e| {
        format!("symlink guard on '{artifact_name}' path '{}': {e}", raw_artifact_path.display())
    })?;

    Ok(canonical_track_dir.join(artifact_name))
}

struct InvalidDryApprovalService {
    detail: String,
}

impl DryCheckApprovalService for InvalidDryApprovalService {
    fn check_approved(
        &self,
        _track_id: &TrackId,
        _current_fragment_refs: &BTreeSet<FragmentRef>,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
        Err(DryCheckCycleError::CoveragePort(self.detail.clone()))
    }
}

// ── FsDryApprovalFactoryAdapter ───────────────────────────────────────────────

/// Infrastructure adapter implementing [`DryApprovalFactoryPort`].
///
/// Constructs a [`DryCheckApprovalInteractor`] from the injected
/// infrastructure-layer config (used for `current_config_fingerprint`) and the
/// resolved corpus metadata.
pub struct FsDryApprovalFactoryAdapter;

impl DryApprovalFactoryPort for FsDryApprovalFactoryAdapter {
    fn build_approval(
        &self,
        track_dir: &Path,
        canonical_root: &Path,
        dry_config: DryCheckConfig,
        config_fingerprint: DryCheckConfigFingerprint,
        corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> Arc<dyn DryCheckApprovalService + Send + Sync> {
        let dry_check_json_path =
            match trusted_track_artifact_path(track_dir, canonical_root, "dry-check.json") {
                Ok(path) => path,
                Err(detail) => {
                    return Arc::new(InvalidDryApprovalService { detail })
                        as Arc<dyn DryCheckApprovalService + Send + Sync>;
                }
            };
        let dry_check_coverage_path =
            match trusted_track_artifact_path(track_dir, canonical_root, "dry-check-coverage.json")
            {
                Ok(path) => path,
                Err(detail) => {
                    return Arc::new(InvalidDryApprovalService { detail })
                        as Arc<dyn DryCheckApprovalService + Send + Sync>;
                }
            };
        let store =
            Arc::new(FsDryCheckStore::new(dry_check_json_path, canonical_root.to_path_buf()));
        let coverage = Arc::new(FsDryCheckCoverageAdapter::new(
            dry_check_coverage_path,
            canonical_root.to_path_buf(),
        ));
        Arc::new(DryCheckApprovalInteractor::new(
            dry_config,
            store,
            coverage,
            config_fingerprint,
            corpus_fingerprint,
        )) as Arc<dyn DryCheckApprovalService + Send + Sync>
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use domain::dry_check::DryCheckApprovalVerdict;
    use usecase::dry_check::{DryCheckParallelism, DryCheckPercent};

    use super::*;

    fn test_dry_config() -> DryCheckConfig {
        DryCheckConfig::new(
            DryCheckPercent::try_new(10).unwrap(),
            DryCheckPercent::try_new(90).unwrap(),
            DryCheckParallelism::try_new(4).unwrap(),
            true,
        )
    }

    fn test_config_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("a".repeat(64)).unwrap()
    }

    fn test_corpus_fingerprint() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint::new("c".repeat(64)).unwrap()
    }

    fn test_track_id() -> TrackId {
        TrackId::try_new("my-track-2026").unwrap()
    }

    #[test]
    fn test_build_approval_valid_track_dir_with_missing_coverage_returns_blocked() {
        let root = tempfile::tempdir().unwrap();
        let track_dir = root.path().join("track").join("items").join("my-track-2026");
        std::fs::create_dir_all(&track_dir).unwrap();

        let service = FsDryApprovalFactoryAdapter.build_approval(
            &track_dir,
            root.path(),
            test_dry_config(),
            test_config_fingerprint(),
            test_corpus_fingerprint(),
        );

        let verdict = service.check_approved(&test_track_id(), &BTreeSet::new()).unwrap();

        assert_eq!(
            verdict,
            DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 },
            "missing coverage should keep the gate fail-closed"
        );
    }

    #[test]
    fn test_build_approval_outside_trusted_root_returns_erroring_service() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let outside_track_dir = dir.path().join("outside").join("my-track-2026");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside_track_dir).unwrap();

        let service = FsDryApprovalFactoryAdapter.build_approval(
            &outside_track_dir,
            &root,
            test_dry_config(),
            test_config_fingerprint(),
            test_corpus_fingerprint(),
        );

        let err = service.check_approved(&test_track_id(), &BTreeSet::new()).unwrap_err();

        assert!(
            err.to_string().contains("outside trusted root"),
            "outside-root track_dir should fail closed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_build_approval_symlinked_track_dir_returns_erroring_service() {
        let root = tempfile::tempdir().unwrap();
        let real_track_dir = root.path().join("real-track");
        let items_dir = root.path().join("track").join("items");
        let linked_track_dir = items_dir.join("my-track-2026");
        std::fs::create_dir_all(&real_track_dir).unwrap();
        std::fs::create_dir_all(&items_dir).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, &linked_track_dir).unwrap();

        let service = FsDryApprovalFactoryAdapter.build_approval(
            &linked_track_dir,
            root.path(),
            test_dry_config(),
            test_config_fingerprint(),
            test_corpus_fingerprint(),
        );

        let err = service.check_approved(&test_track_id(), &BTreeSet::new()).unwrap_err();

        assert!(
            err.to_string().contains("symlink guard"),
            "symlinked track_dir should fail closed, got: {err}"
        );
    }
}
