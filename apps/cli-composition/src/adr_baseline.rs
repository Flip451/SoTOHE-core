//! Request-scoped composition root for the ADR baseline command family.

use std::sync::Arc;

use cli_driver::adr_baseline::{
    AdrBaselineDriver, AdrBaselineInput, AdrBaselineRequest, TrackIdInput,
};
use thiserror::Error;
use usecase::ValidationError;

use crate::CommandOutcome;
use crate::TrackCompositionRoot;

/// ADR baseline composition root; request items_dir selects request-scoped adapter wiring.
pub struct AdrBaselineCompositionRoot {}

/// Typed failures produced while resolving an ADR baseline request for execution.
#[derive(Debug, Error)]
pub enum AdrBaselineResolutionError {
    /// The request's items directory does not identify a project root.
    #[error("{0}")]
    ProjectRoot(crate::CompositionError),
    /// The request's track identity cannot be resolved from its branch context.
    #[error("{0}")]
    TrackResolution(crate::CompositionError),
    /// A resolved track identifier does not satisfy the driver input invariant.
    #[error("{0}")]
    ResolvedTrackInvalid(ValidationError),
    /// The composition-owned timestamp could not be constructed.
    #[error("{0}")]
    Timestamp(ValidationError),
}

impl Default for AdrBaselineCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl AdrBaselineCompositionRoot {
    /// Creates an ADR baseline composition root.
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    /// Resolves the request scope and executes it through filesystem and Git adapters.
    pub fn execute(
        &self,
        request: AdrBaselineRequest,
    ) -> Result<CommandOutcome, AdrBaselineResolutionError> {
        let track_root = TrackCompositionRoot::new();
        let project_root = track_root
            .track_resolve_project_root(request.items_dir().to_path_buf())
            .map_err(AdrBaselineResolutionError::ProjectRoot)?;
        let input = resolve_request(request, &track_root)?;
        Ok(self.adr_baseline_driver(project_root).handle(input))
    }

    fn adr_baseline_driver(&self, project_root: std::path::PathBuf) -> AdrBaselineDriver {
        use infrastructure::adr_baseline::{FsAdrBaselineStore, FsGitAdrBaselineSource};
        use usecase::adr_baseline::{
            AdrBaselineInteractor, AdrBaselineQueryInteractor, AdrBaselineSourcePort,
            AdrBaselineStorePort, AdrBaselineStoreReadPort,
        };

        let store = Arc::new(FsAdrBaselineStore::from(project_root.clone()));
        let source = Arc::new(FsGitAdrBaselineSource::from(project_root));
        let command_service = Arc::new(AdrBaselineInteractor::new(
            store.clone() as Arc<dyn AdrBaselineStorePort>,
            source.clone() as Arc<dyn AdrBaselineSourcePort>,
        ));
        let query_service = Arc::new(AdrBaselineQueryInteractor::new(
            store as Arc<dyn AdrBaselineStoreReadPort>,
            source as Arc<dyn AdrBaselineSourcePort>,
        ));
        AdrBaselineDriver::new(command_service, query_service)
    }
}

fn resolve_request(
    request: AdrBaselineRequest,
    track_root: &TrackCompositionRoot,
) -> Result<AdrBaselineInput, AdrBaselineResolutionError> {
    match request {
        AdrBaselineRequest::Snapshot { items_dir, track_id, source, kind, reason } => {
            let timestamp =
                infrastructure::timestamp_now().map_err(AdrBaselineResolutionError::Timestamp)?;
            Ok(AdrBaselineInput::Snapshot {
                track_id: resolve_for_write(track_root, track_id, &items_dir)?,
                source,
                kind,
                reason,
                timestamp,
            })
        }
        AdrBaselineRequest::Restore { items_dir, track_id, source } => {
            Ok(AdrBaselineInput::Restore {
                track_id: resolve_for_write(track_root, track_id, &items_dir)?,
                source,
            })
        }
        AdrBaselineRequest::CheckReview { items_dir, track_id, primary_source } => {
            Ok(AdrBaselineInput::CheckReview {
                track_id: resolve_for_read(track_root, track_id, &items_dir)?,
                primary_source,
            })
        }
        AdrBaselineRequest::CheckCommit { items_dir, track_id } => {
            Ok(AdrBaselineInput::CheckCommit {
                track_id: resolve_for_read(track_root, track_id, &items_dir)?,
            })
        }
    }
}

fn resolve_for_write(
    track_root: &TrackCompositionRoot,
    track_id: Option<TrackIdInput>,
    items_dir: &std::path::Path,
) -> Result<TrackIdInput, AdrBaselineResolutionError> {
    track_root
        .track_resolve_id_for_write(track_id.map(|id| id.to_string()), items_dir.to_path_buf())
        .map_err(AdrBaselineResolutionError::TrackResolution)?
        .parse()
        .map_err(AdrBaselineResolutionError::ResolvedTrackInvalid)
}

fn resolve_for_read(
    track_root: &TrackCompositionRoot,
    track_id: Option<TrackIdInput>,
    items_dir: &std::path::Path,
) -> Result<TrackIdInput, AdrBaselineResolutionError> {
    track_root
        .track_resolve_id(track_id.map(|id| id.to_string()), items_dir.to_path_buf())
        .map_err(AdrBaselineResolutionError::TrackResolution)?
        .parse()
        .map_err(AdrBaselineResolutionError::ResolvedTrackInvalid)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use cli_driver::adr_baseline::{
        AdrBaselineKindInput, AdrBaselineRequest, AdrSourceFileNameInput, TrackIdInput,
    };

    use super::{AdrBaselineCompositionRoot, AdrBaselineResolutionError};

    fn initialize_git_repository(project: &std::path::Path) {
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(project)
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new("git")
            .args(["checkout", "-q", "-b", "track/fixture-track"])
            .current_dir(project)
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "--allow-empty",
                "-q",
                "-m",
                "initial",
            ])
            .current_dir(project)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn test_adr_baseline_execute_snapshot_round_trip_writes_track_fixture() {
        let project = tempfile::tempdir().unwrap();
        let items_dir = project.path().join("track/items");
        let adr_dir = project.path().join("knowledge/adr");
        std::fs::create_dir_all(items_dir.join("fixture-track")).unwrap();
        std::fs::create_dir_all(&adr_dir).unwrap();
        std::fs::write(adr_dir.join("decision.md"), b"# Decision\n").unwrap();
        initialize_git_repository(project.path());

        let outcome = AdrBaselineCompositionRoot::new()
            .execute(AdrBaselineRequest::Snapshot {
                items_dir,
                track_id: Some("fixture-track".parse::<TrackIdInput>().unwrap()),
                source: "decision.md".parse::<AdrSourceFileNameInput>().unwrap(),
                kind: AdrBaselineKindInput::Init,
                reason: None,
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0, "unexpected outcome: {outcome:?}");
        let baseline_dir = project.path().join("track/items/fixture-track/adr-baseline");
        assert!(baseline_dir.join("ledger.jsonl").is_file());
        let copies = std::fs::read_dir(&baseline_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("decision."))
            .count();
        assert_eq!(copies, 1);
    }

    #[test]
    fn test_adr_baseline_execute_rejects_noncanonical_items_directory() {
        let error = AdrBaselineCompositionRoot::new().execute(AdrBaselineRequest::CheckCommit {
            items_dir: "fixture/items".into(),
            track_id: Some("fixture-track".parse::<TrackIdInput>().unwrap()),
        });

        let error = error.unwrap_err();
        assert_eq!(
            error.to_string(),
            "--items-dir must point to '<project-root>/track/items'; got fixture/items"
        );
    }

    #[test]
    fn test_adr_baseline_execute_returns_typed_track_resolution_error() {
        let project = tempfile::tempdir().unwrap();
        let items_dir = project.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let error = AdrBaselineCompositionRoot::new()
            .execute(AdrBaselineRequest::CheckCommit { items_dir, track_id: None })
            .unwrap_err();

        assert!(matches!(error, AdrBaselineResolutionError::TrackResolution(_)));
    }
}
