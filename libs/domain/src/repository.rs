use crate::{RepositoryError, TrackId, TrackMetadata};

/// Port for persisting and retrieving track metadata.
pub trait TrackRepository: Send + Sync {
    /// Finds a track by its identifier.
    ///
    /// # Errors
    /// Returns `RepositoryError` on I/O or internal failure.
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, RepositoryError>;

    /// Saves (inserts or updates) a track.
    ///
    /// # Errors
    /// Returns `RepositoryError` on I/O or internal failure.
    fn save(&self, track: &TrackMetadata) -> Result<(), RepositoryError>;
}
