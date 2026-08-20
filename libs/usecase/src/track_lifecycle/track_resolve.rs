use std::sync::Arc;

use domain::track_phase::TrackPhase;

use crate::git_workflow::DiagnosticText;
use crate::operator_command::{CommandArgument, CommandArgv};
use crate::track_phase::{TrackPhaseError, TrackPhaseOutput, TrackPhaseService};

use super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort};

/// Validated command for resolving the current track phase.
pub struct TrackResolveCommand {
    /// The track items directory used by the query.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Presentation-free result of a track-phase resolution.
pub enum TrackResolveResult {
    /// The track is not blocked and can proceed.
    Ready {
        /// The derived user-facing phase.
        phase: TrackPhase,
        /// Why this phase was selected.
        reason: DiagnosticText,
        /// Recommended next command tokens.
        next_command: CommandArgv,
    },
    /// The track is blocked and the blocker must be shown.
    Blocked {
        /// The derived user-facing phase.
        phase: TrackPhase,
        /// Why this phase was selected.
        reason: DiagnosticText,
        /// Recommended next command tokens.
        next_command: CommandArgv,
        /// The blocker text that must be shown.
        blocker: DiagnosticText,
    },
}

/// Error returned by the track-resolve command boundary.
#[derive(Debug)]
pub enum TrackResolveError {
    /// Selection or phase resolution failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackResolveError {}

/// Application service for resolving the current track phase.
pub trait TrackResolveService: Send + Sync {
    /// Resolves the selection and returns the current phase.
    fn execute(
        &self,
        command: TrackResolveCommand,
    ) -> Result<TrackResolveResult, TrackResolveError>;
}

/// Interactor for the track-resolve command context.
pub struct TrackResolveInteractor {
    phase: Arc<dyn TrackPhaseService>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackResolveInteractor {
    /// Creates an interactor from the phase and selection ports.
    #[must_use]
    pub fn new(phase: Arc<dyn TrackPhaseService>, resolver: Arc<dyn TrackSelectionPort>) -> Self {
        Self { phase, resolver }
    }
}

impl TrackResolveService for TrackResolveInteractor {
    fn execute(
        &self,
        command: TrackResolveCommand,
    ) -> Result<TrackResolveResult, TrackResolveError> {
        let TrackResolveCommand { items_dir, track } = command;
        let track_id = match &track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => self
                .resolver
                .resolve_required(&items_dir, &track)
                .map_err(|error| execution_failed(error.to_string()))?,
        };
        let output = self
            .phase
            .resolve(track_id.as_ref().to_owned(), items_dir.as_path().to_path_buf())
            .map_err(map_phase_error)?;
        map_output(output)
    }
}

fn map_output(output: TrackPhaseOutput) -> Result<TrackResolveResult, TrackResolveError> {
    let phase = parse_phase(&output.phase)?;
    let reason = DiagnosticText::new(output.reason);
    let next_command = parse_next_command(&output.next_command)?;
    match output.blocker {
        Some(blocker) => Ok(TrackResolveResult::Blocked {
            phase,
            reason,
            next_command,
            blocker: DiagnosticText::new(blocker),
        }),
        None => Ok(TrackResolveResult::Ready { phase, reason, next_command }),
    }
}

fn parse_phase(value: &str) -> Result<TrackPhase, TrackResolveError> {
    match value {
        "Planning" => Ok(TrackPhase::Planning),
        "In Progress" => Ok(TrackPhase::InProgress),
        "Ready to Ship" => Ok(TrackPhase::ReadyToShip),
        "Blocked" => Ok(TrackPhase::Blocked),
        "Cancelled" => Ok(TrackPhase::Cancelled),
        "Archived" => Ok(TrackPhase::Archived),
        other => Err(execution_failed(format!("unrecognized track phase: {other}"))),
    }
}

fn parse_next_command(value: &str) -> Result<CommandArgv, TrackResolveError> {
    CommandArgv::try_new(vec![CommandArgument::try_new(value.to_owned())])
        .map_err(|error| execution_failed(error.to_string()))
}

fn map_phase_error(error: TrackPhaseError) -> TrackResolveError {
    execution_failed(error.to_string())
}

fn execution_failed(error: impl Into<String>) -> TrackResolveError {
    TrackResolveError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use domain::TrackId;

    use super::*;
    use crate::track_lifecycle::{TrackViewsScope, TrackWorkspaceRoot};

    struct RecordingResolver {
        active: Result<TrackId, DiagnosticText>,
        calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            *self.calls.lock().expect("resolver lock is available") += 1;
            self.active.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            self.active.clone()
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            Ok(TrackViewsScope::RegistryOnly)
        }
    }

    struct RecordingPhase {
        result: Result<TrackPhaseOutput, TrackPhaseError>,
        calls: Mutex<Vec<(String, PathBuf)>>,
    }

    impl TrackPhaseService for RecordingPhase {
        fn resolve(
            &self,
            track_id: String,
            items_dir: PathBuf,
        ) -> Result<TrackPhaseOutput, TrackPhaseError> {
            self.calls.lock().expect("phase lock is available").push((track_id, items_dir));
            match &self.result {
                Ok(output) => Ok(TrackPhaseOutput {
                    phase: output.phase.clone(),
                    reason: output.reason.clone(),
                    next_command: output.next_command.clone(),
                    blocker: output.blocker.clone(),
                }),
                Err(TrackPhaseError::InvalidTrackId(value)) => {
                    Err(TrackPhaseError::InvalidTrackId(value.clone()))
                }
                Err(TrackPhaseError::TrackNotFound(value)) => {
                    Err(TrackPhaseError::TrackNotFound(value.clone()))
                }
                Err(TrackPhaseError::ImplPlanLoadFailed(value)) => {
                    Err(TrackPhaseError::ImplPlanLoadFailed(value.clone()))
                }
            }
        }
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("track/items"))
            .expect("items directory is valid")
    }

    fn ready_output() -> TrackPhaseOutput {
        TrackPhaseOutput {
            phase: "In Progress".to_owned(),
            reason: "track has unresolved tasks".to_owned(),
            next_command: "/track:implement".to_owned(),
            blocker: None,
        }
    }

    fn blocked_output() -> TrackPhaseOutput {
        TrackPhaseOutput {
            phase: "Blocked".to_owned(),
            reason: "waiting on review".to_owned(),
            next_command: "/track:status".to_owned(),
            blocker: Some("waiting on review".to_owned()),
        }
    }

    fn next_command_text(argv: &CommandArgv) -> String {
        argv.arguments().iter().map(CommandArgument::as_str).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn test_track_resolve_interactor_explicit_selection_returns_ready() {
        let phase = RecordingPhase { result: Ok(ready_output()), calls: Mutex::new(Vec::new()) };
        let resolver = RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        };
        let interactor = TrackResolveInteractor::new(Arc::new(phase), Arc::new(resolver));
        let result = interactor
            .execute(TrackResolveCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("resolve-track").expect("track id is valid"),
                ),
            })
            .expect("phase resolution succeeds");
        match result {
            TrackResolveResult::Ready { phase, reason, next_command } => {
                assert_eq!(phase, TrackPhase::InProgress);
                assert_eq!(reason.as_str(), "track has unresolved tasks");
                assert_eq!(next_command_text(&next_command), "/track:implement");
            }
            TrackResolveResult::Blocked { .. } => panic!("ready output must not become blocked"),
        }
    }

    #[test]
    fn test_track_resolve_interactor_blocked_output_returns_blocked() {
        let phase = RecordingPhase { result: Ok(blocked_output()), calls: Mutex::new(Vec::new()) };
        let resolver = RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        };
        let interactor = TrackResolveInteractor::new(Arc::new(phase), Arc::new(resolver));
        let result = interactor
            .execute(TrackResolveCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("blocked-track").expect("track id is valid"),
                ),
            })
            .expect("blocked resolution succeeds");
        match result {
            TrackResolveResult::Blocked { phase, reason, next_command, blocker } => {
                assert_eq!(phase, TrackPhase::Blocked);
                assert_eq!(reason.as_str(), "waiting on review");
                assert_eq!(next_command_text(&next_command), "/track:status");
                assert_eq!(blocker.as_str(), "waiting on review");
            }
            TrackResolveResult::Ready { .. } => panic!("blocked output must keep the blocker"),
        }
    }

    #[test]
    fn test_track_resolve_interactor_active_selection_uses_resolver() {
        let phase = RecordingPhase { result: Ok(ready_output()), calls: Mutex::new(Vec::new()) };
        let resolver = Arc::new(RecordingResolver {
            active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
            calls: Mutex::new(0),
        });
        let interactor = TrackResolveInteractor::new(Arc::new(phase), resolver.clone());
        interactor
            .execute(TrackResolveCommand { items_dir: items_dir(), track: TrackSelection::Active })
            .expect("active resolution succeeds");
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 1);
    }

    #[test]
    fn test_track_resolve_interactor_explicit_selection_skips_resolver() {
        let phase = RecordingPhase { result: Ok(ready_output()), calls: Mutex::new(Vec::new()) };
        let resolver = Arc::new(RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        });
        let interactor = TrackResolveInteractor::new(Arc::new(phase), resolver.clone());
        interactor
            .execute(TrackResolveCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("resolve-track").expect("track id is valid"),
                ),
            })
            .expect("explicit resolution succeeds");
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 0);
    }

    #[test]
    fn test_track_resolve_interactor_resolution_failure_returns_execution_error() {
        let phase =
            Arc::new(RecordingPhase { result: Ok(ready_output()), calls: Mutex::new(Vec::new()) });
        let resolver = RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        };
        let interactor = TrackResolveInteractor::new(phase.clone(), Arc::new(resolver));
        let error = match interactor
            .execute(TrackResolveCommand { items_dir: items_dir(), track: TrackSelection::Active })
        {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "active track unavailable");
        assert!(phase.calls.lock().expect("phase lock is available").is_empty());
    }

    #[test]
    fn test_track_resolve_interactor_phase_failure_returns_execution_error() {
        let phase = RecordingPhase {
            result: Err(TrackPhaseError::TrackNotFound("missing".to_owned())),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        };
        let interactor = TrackResolveInteractor::new(Arc::new(phase), Arc::new(resolver));
        let error = match interactor.execute(TrackResolveCommand {
            items_dir: items_dir(),
            track: TrackSelection::Explicit(
                TrackId::try_new("missing").expect("track id is valid"),
            ),
        }) {
            Ok(_) => panic!("phase failure must propagate"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "track not found: missing");
    }

    #[test]
    fn test_track_resolve_command_context_colocates_boundary_types() {
        let source = include_str!("track_resolve.rs");
        assert!(source.contains("pub struct TrackResolveCommand"));
        assert!(source.contains("pub enum TrackResolveError"));
        assert!(source.contains("pub enum TrackResolveResult"));
        assert!(source.contains("pub trait TrackResolveService"));
        assert!(source.contains("pub struct TrackResolveInteractor"));
        assert!(source.contains("impl TrackResolveService for TrackResolveInteractor"));
    }
}
