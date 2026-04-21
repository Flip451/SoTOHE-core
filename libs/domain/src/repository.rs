use crate::{
    DomainError, ImplPlanDocument, RepositoryError, TrackId, TrackMetadata, TrackReadError,
    TrackWriteError, WorktreeError,
};

/// Read-only port for track retrieval.
pub trait TrackReader: Send + Sync {
    /// Finds a track by its identifier.
    ///
    /// # Errors
    /// Returns `TrackReadError` on I/O or internal failure.
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError>;
}

/// Read port for `impl-plan.json` persistence.
pub trait ImplPlanReader: Send + Sync {
    /// Loads `impl-plan.json` for the given track, returning `None` when
    /// the file does not yet exist.
    ///
    /// # Errors
    /// Returns `RepositoryError` on I/O or decode failure.
    fn load_impl_plan(&self, id: &TrackId) -> Result<Option<ImplPlanDocument>, RepositoryError>;
}

/// Write port for `impl-plan.json` persistence.
pub trait ImplPlanWriter: Send + Sync {
    /// Atomically persists `impl-plan.json` for the given track.
    ///
    /// # Errors
    /// Returns `RepositoryError` on I/O or encode failure.
    fn save_impl_plan(&self, id: &TrackId, doc: &ImplPlanDocument) -> Result<(), RepositoryError>;
}

/// Port for inspecting worktree cleanliness.
///
/// Returns raw `git status --porcelain` output so that the use case layer
/// can parse and validate it without depending on infrastructure.
pub trait WorktreeReader: Send + Sync {
    /// Returns the raw porcelain output from `git status --porcelain`.
    ///
    /// # Errors
    /// Returns [`WorktreeError`] on I/O failure.
    fn porcelain_status(&self) -> Result<String, WorktreeError>;
}

/// Atomic mutation port for track persistence.
/// Implementations provide locking internally.
///
/// NOTE: `update<F>` makes this trait non-object-safe (generic method).
/// This is acceptable — use cases depend on concrete types or generics,
/// not `dyn TrackWriter`. If dyn dispatch is needed in the future,
/// extract a non-generic sub-trait.
pub trait TrackWriter: Send + Sync {
    /// Persists a track (insert or update — upsert semantics).
    ///
    /// # Errors
    /// Returns `TrackWriteError` on persistence failure.
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError>;

    /// Atomically loads, mutates, and persists a track under exclusive lock.
    ///
    /// # Errors
    /// - `TrackWriteError::Repository(TrackNotFound)` if the track does not exist.
    /// - `TrackWriteError::Repository(Message)` on I/O or lock failure.
    /// - `TrackWriteError::Domain` propagated from the mutation closure.
    fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>;
}
