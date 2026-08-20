use std::collections::BTreeMap;
use std::sync::Arc;

use domain::{ContentHash, SpecElementId, TrackId};

use crate::git_workflow::DiagnosticText;

use super::super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort};
use super::TrackSpecAnchorSelection;

/// Typed command for reading canonical hashes for a track's spec elements.
pub struct TrackSpecElementHashCommand {
    /// Explicit or active track selection.
    pub track: TrackSelection,
    /// Directory containing the selected track.
    pub items_dir: TrackItemsDirectory,
    /// The requested anchor or all anchors.
    pub anchor: TrackSpecAnchorSelection,
}

/// Error returned by the spec-element-hash command boundary.
#[derive(Debug)]
pub enum TrackSpecElementHashError {
    /// The hash operation or selection resolution failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackSpecElementHashError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackSpecElementHashError {}

/// Presentation-free result of a spec-element-hash lookup.
pub enum TrackSpecElementHashResult {
    /// A single requested anchor hash.
    Single(ContentHash),
    /// All spec-element hashes in canonical identifier order.
    All(BTreeMap<SpecElementId, ContentHash>),
}

/// Secondary port for reading canonical spec-element hashes.
pub trait TrackSpecElementHashPort: Send + Sync {
    /// Executes the hash lookup for a resolved track.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackSpecElementHashCommand,
    ) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError>;
}

/// Application service for spec-element-hash lookups.
pub trait TrackSpecElementHashService: Send + Sync {
    /// Resolves the selection and executes the hash lookup.
    fn execute(
        &self,
        command: TrackSpecElementHashCommand,
    ) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError>;
}

/// Interactor for the spec-element-hash command context.
pub struct TrackSpecElementHashInteractor {
    operation: Arc<dyn TrackSpecElementHashPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackSpecElementHashInteractor {
    /// Creates an interactor from the hash operation and selection ports.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackSpecElementHashPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackSpecElementHashService for TrackSpecElementHashInteractor {
    fn execute(
        &self,
        command: TrackSpecElementHashCommand,
    ) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => self
                .resolver
                .resolve_required(&command.items_dir, &command.track)
                .map_err(execution_failed)?,
        };
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackSpecElementHashError {
    TrackSpecElementHashError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::{TrackViewsScope, TrackWorkspaceRoot};

    struct RecordingResolver {
        result: Result<TrackId, DiagnosticText>,
        calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            *self.calls.lock().expect("resolver lock is available") += 1;
            self.result.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            self.result.clone()
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            Ok(TrackViewsScope::RegistryOnly)
        }
    }

    struct RecordingOperation {
        calls: Mutex<Vec<TrackId>>,
        result: Result<TrackSpecElementHashResult, TrackSpecElementHashError>,
    }

    impl TrackSpecElementHashPort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            _command: TrackSpecElementHashCommand,
        ) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError> {
            self.calls.lock().expect("operation lock is available").push(track_id);
            match &self.result {
                Ok(TrackSpecElementHashResult::Single(hash)) => {
                    Ok(TrackSpecElementHashResult::Single(hash.clone()))
                }
                Ok(TrackSpecElementHashResult::All(hashes)) => {
                    Ok(TrackSpecElementHashResult::All(hashes.clone()))
                }
                Err(_) => Err(TrackSpecElementHashError::ExecutionFailed(DiagnosticText::new(
                    "hash operation failed",
                ))),
            }
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    fn command(track: TrackSelection) -> TrackSpecElementHashCommand {
        TrackSpecElementHashCommand {
            track,
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            anchor: TrackSpecAnchorSelection::All,
        }
    }

    #[test]
    fn test_track_spec_element_hash_interactor_resolves_selection_and_forwards_operation() {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            result: Ok(TrackSpecElementHashResult::Single(ContentHash::from_bytes([7; 32]))),
        });
        let resolver = Arc::new(RecordingResolver {
            result: Ok(track_id("hash-track")),
            calls: Mutex::new(0),
        });
        let interactor = TrackSpecElementHashInteractor::new(operation.clone(), resolver.clone());

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("hash-track"))))
            .expect("hash lookup succeeds");

        assert!(matches!(result, TrackSpecElementHashResult::Single(_)));
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 0);
        assert_eq!(operation.calls.lock().expect("operation lock is available").len(), 1);
    }

    #[test]
    fn test_track_spec_element_hash_interactor_resolver_failure_returns_error_without_operation() {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            result: Ok(TrackSpecElementHashResult::Single(ContentHash::from_bytes([7; 32]))),
        });
        let resolver = Arc::new(RecordingResolver {
            result: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        });
        let interactor = TrackSpecElementHashInteractor::new(operation.clone(), resolver);

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("resolver failure must stop the operation"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }
}
