#![forbid(unsafe_code)]
//! Infrastructure layer for the SoTOHE-core track state machine.

pub mod adr_decision;
pub mod agent_profiles;
pub mod arch;
pub mod code_profile_builder;
pub mod codex_common;
pub mod codex_planner;
pub mod conventions;
pub mod demo;
pub mod dry_check;
pub mod file_port;
pub mod gh_cli;
pub mod git_cli;
pub mod impl_catalog_signal_reader;
pub mod impl_plan_codec;
pub mod impl_plan_reader;
pub mod pr_review;
pub mod ref_verify;
pub mod review_v2;
pub mod schema_export;
pub mod schema_export_codec;
#[cfg(test)]
mod schema_export_tests;
pub mod semantic_dup;
pub mod shell;
pub mod signal_layer_reader;
pub mod spec;
pub mod task_contract_codec;
pub mod task_contract_reader;
pub mod task_coverage_codec;
pub mod tddd;
pub mod telemetry;
pub mod track;
pub use dry_check::noop_approval::NoOpDryApprovalService;
pub use dry_check::recording_agent::RecordingDryAgent;
pub use git_cli::workflow_adapter::FsGitWorkflowAdapter;
pub use pr_review::SystemSleepAdapter;
pub use semantic_dup::fragment_extractor_adapter::CodeFragmentExtractorAdapter;
pub use semantic_dup::noop_adapter::NoopSemanticIndexPort;
pub use semantic_dup::null_insert_proxy::NullInsertIndexProxy;
pub use telemetry::archived_track::FsArchivedTrackTelemetryAdapter;
pub use telemetry::report_adapter::{FsTelemetryEmitDynamicAdapter, FsTelemetryReportAdapter};
pub use track::fs_symlink_guard::FsSymlinkGuard;
pub use track::gate_state::{FsRefVerifyGateStateAdapter, FsReviewGateStateAdapter};
pub mod type_catalogue_render;
pub mod verify;
pub mod verify_adapter;
pub use ref_verify::{
    FsRefVerifyAggregateAdapter, FsRefVerifyCheckApprovedAdapter, FsRefVerifyRunAdapter,
};
pub use verify_adapter::FsVerifyAdapter;

pub(crate) fn resolve_items_dir_under_current_repo(
    items_dir: &std::path::Path,
) -> Result<std::path::PathBuf, std::io::Error> {
    use std::path::Component;

    use crate::git_cli::{GitRepository as _, SystemGitRepo};

    if items_dir.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "items_dir must not be empty",
        ));
    }
    if items_dir
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("items_dir cannot escape the current repository root: {}", items_dir.display()),
        ));
    }

    let repo = SystemGitRepo::discover().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("cannot discover current git repository: {e}"),
        )
    })?;
    let repo_root = repo.root().canonicalize().map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!(
                "failed to canonicalize current repository root {}: {e}",
                repo.root().display()
            ),
        )
    })?;
    match repo_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to use symlinked repository root: {}", repo_root.display()),
            ));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(std::io::Error::new(
                e.kind(),
                format!("failed to stat repository root {}: {e}", repo_root.display()),
            ));
        }
    }

    let absolute_items_dir =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { repo_root.join(items_dir) };
    if !absolute_items_dir.starts_with(&repo_root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "items_dir must resolve inside the current repository root {}; got {}",
                repo_root.display(),
                items_dir.display()
            ),
        ));
    }

    crate::track::symlink_guard::reject_symlinks_below(&absolute_items_dir, &repo_root)
        .map(|_| ())?;

    let canonical_items_dir = absolute_items_dir.canonicalize().map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("failed to canonicalize items_dir {}: {e}", items_dir.display()),
        )
    })?;
    if !canonical_items_dir.starts_with(&repo_root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "items_dir resolves outside the current repository root {}; got {}",
                repo_root.display(),
                canonical_items_dir.display()
            ),
        ));
    }
    if !canonical_items_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("items_dir is not a directory: {}", items_dir.display()),
        ));
    }

    Ok(canonical_items_dir)
}

/// Returns a `Timestamp` for the current UTC instant, truncated to whole seconds.
///
/// Consolidates `chrono::Utc::now()` into a single infrastructure function so that
/// domain/usecase layers receive timestamps as arguments (hexagonal purity).
///
/// # Errors
///
/// Returns `domain::ValidationError` if chrono produces an unparsable string (should never happen).
pub fn timestamp_now() -> Result<domain::Timestamp, domain::ValidationError> {
    use chrono::Timelike as _;
    let now = chrono::Utc::now();
    let dt = now.with_nanosecond(0).unwrap_or(now);
    let raw = dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    domain::Timestamp::new(raw)
}

use std::collections::HashMap;
use std::sync::Mutex;

use domain::{
    DomainError, RepositoryError, TrackId, TrackMetadata, TrackReadError, TrackReader,
    TrackWriteError, TrackWriter,
};

/// In-memory implementation of `TrackReader` + `TrackWriter` for testing.
#[derive(Default)]
pub struct InMemoryTrackStore {
    tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
}

impl InMemoryTrackStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TrackReader for InMemoryTrackStore {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
        let tracks = self
            .tracks
            .lock()
            .map_err(|_| RepositoryError::Message("internal repository error".to_owned()))?;
        Ok(tracks.get(id).cloned())
    }
}

impl TrackWriter for InMemoryTrackStore {
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
        let mut tracks = self
            .tracks
            .lock()
            .map_err(|_| RepositoryError::Message("internal repository error".to_owned()))?;
        tracks.insert(track.id().clone(), track.clone());
        Ok(())
    }

    fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>,
    {
        let mut tracks = self.tracks.lock().map_err(|_| {
            TrackWriteError::Repository(RepositoryError::Message(
                "internal repository error".to_owned(),
            ))
        })?;
        let track = tracks.get_mut(id).ok_or_else(|| {
            TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
        })?;
        mutate(track).map_err(TrackWriteError::from)?;
        Ok(track.clone())
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::{
        StatusOverride, TrackId, TrackMetadata, TrackReader, TrackWriter, derive_track_status,
    };

    use super::InMemoryTrackStore;

    fn sample_track() -> TrackMetadata {
        // TrackMetadata is identity-only; status derived from impl-plan + override.
        TrackMetadata::new(
            TrackId::try_new("track-state-machine").unwrap(),
            "Track state machine",
            None,
        )
        .unwrap()
    }

    #[test]
    fn store_returns_saved_track() {
        let store = InMemoryTrackStore::new();
        let track = sample_track();

        store.save(&track).unwrap();

        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded, track);
    }

    #[test]
    fn update_atomically_mutates_and_persists() {
        let store = InMemoryTrackStore::new();
        let track = sample_track();

        store.save(&track).unwrap();

        let updated = store
            .update(track.id(), |t| {
                t.set_status_override(Some(StatusOverride::blocked("testing").unwrap()));
                Ok(())
            })
            .unwrap();

        assert!(updated.status_override().is_some());
        assert_eq!(derive_track_status(None, updated.status_override()).to_string(), "blocked");

        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert!(reloaded.status_override().is_some());
        assert_eq!(derive_track_status(None, reloaded.status_override()).to_string(), "blocked");
    }
}
