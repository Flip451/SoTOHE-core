use crate::{
    DomainError, ReviewJson, TrackId, TrackMetadata, TrackReadError, TrackWriteError, WorktreeError,
};

/// Read-only port for track retrieval.
pub trait TrackReader: Send + Sync {
    /// Finds a track by its identifier.
    ///
    /// # Errors
    /// Returns `TrackReadError` on I/O or internal failure.
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError>;
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

/// Read-only port for review.json retrieval.
///
/// Separate from `TrackReader` because review.json has a different lifecycle
/// (may not exist for older tracks) and different callers.
pub trait ReviewJsonReader: Send + Sync {
    /// Reads the review.json for the given track.
    ///
    /// Returns `Ok(None)` if review.json does not exist (NoCycle state).
    ///
    /// # Errors
    /// Returns `TrackReadError` on I/O or codec failure.
    fn find_review(&self, id: &TrackId) -> Result<Option<ReviewJson>, TrackReadError>;
}

/// Write port for review.json persistence.
///
/// Implementations should use atomic writes for crash safety.
pub trait ReviewJsonWriter: Send + Sync {
    /// Persists review.json for the given track (upsert semantics).
    ///
    /// # Errors
    /// Returns `TrackWriteError` on persistence or codec failure.
    fn save_review(&self, id: &TrackId, review: &ReviewJson) -> Result<(), TrackWriteError>;
}
