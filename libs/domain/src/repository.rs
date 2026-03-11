use crate::{DomainError, TrackId, TrackMetadata, TrackReadError, TrackWriteError};

/// Read-only port for track retrieval.
pub trait TrackReader: Send + Sync {
    /// Finds a track by its identifier.
    ///
    /// # Errors
    /// Returns `TrackReadError` on I/O or internal failure.
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError>;
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
