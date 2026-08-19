//! Resolve-command rendering for [`crate::track::TrackDriver`].

use std::path::PathBuf;

use usecase::operator_command::CommandArgv;
use usecase::track_lifecycle::track_resolve::{
    TrackResolveCommand, TrackResolveError, TrackResolveResult, TrackResolveService,
};
use usecase::track_lifecycle::{TrackItemsDirectory, TrackLifecycleIdInput, TrackSelection};

use crate::render::CommandOutcome;

pub(crate) fn render_track_resolve_outcome(
    service: &dyn TrackResolveService,
    items_dir: PathBuf,
    track_id: Option<String>,
) -> CommandOutcome {
    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return invalid_items_dir(&items_dir_for_error),
    };
    let track = match track_id
        .map(TrackLifecycleIdInput::try_new)
        .transpose()
        .map_err(|error| error.to_string())
    {
        Ok(track_id) => TrackSelection::from_input(track_id),
        Err(error) => return invalid_track_id(error),
    };
    service
        .execute(TrackResolveCommand { items_dir, track })
        .map(render_track_resolve_result)
        .unwrap_or_else(track_resolve_error_to_outcome)
}

fn render_track_resolve_result(result: TrackResolveResult) -> CommandOutcome {
    let (phase, reason, next_command, blocker) = match result {
        TrackResolveResult::Ready { phase, reason, next_command } => {
            (phase, reason, next_command, None)
        }
        TrackResolveResult::Blocked { phase, reason, next_command, blocker } => {
            (phase, reason, next_command, Some(blocker))
        }
    };
    let mut lines = vec![
        format!("Current phase: {phase}"),
        format!("Reason: {reason}"),
        format!("Recommended next command: {}", display_argv(&next_command)),
    ];
    if let Some(blocker) = blocker {
        lines.push(format!("Blocker: {blocker}"));
    }
    CommandOutcome::success(Some(lines.join("\n")))
}

fn display_argv(argv: &CommandArgv) -> String {
    argv.arguments().iter().map(|argument| argument.as_str()).collect::<Vec<_>>().join(" ")
}

fn track_resolve_error_to_outcome(error: TrackResolveError) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] resolve failed: {error}")))
}

fn invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    CommandOutcome::failure(Some(format!("resolve failed: invalid track id: {legacy_error}")))
}

fn invalid_items_dir(items_dir: &std::path::Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "--items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use domain::track_phase::TrackPhase;
    use usecase::git_workflow::DiagnosticText;
    use usecase::operator_command::CommandArgument;

    use super::*;

    struct RecordingService {
        result: Result<TrackResolveResult, String>,
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
    }

    impl TrackResolveService for RecordingService {
        fn execute(
            &self,
            command: TrackResolveCommand,
        ) -> Result<TrackResolveResult, TrackResolveError> {
            self.calls
                .lock()
                .expect("service lock is available")
                .push((command.items_dir.as_path().to_path_buf(), command.track.clone()));
            match &self.result {
                Ok(TrackResolveResult::Ready { phase, reason, next_command }) => {
                    Ok(TrackResolveResult::Ready {
                        phase: phase.clone(),
                        reason: DiagnosticText::new(reason.as_str()),
                        next_command: clone_argv(next_command),
                    })
                }
                Ok(TrackResolveResult::Blocked { phase, reason, next_command, blocker }) => {
                    Ok(TrackResolveResult::Blocked {
                        phase: phase.clone(),
                        reason: DiagnosticText::new(reason.as_str()),
                        next_command: clone_argv(next_command),
                        blocker: DiagnosticText::new(blocker.as_str()),
                    })
                }
                Err(error) => Err(TrackResolveError::ExecutionFailed(DiagnosticText::new(error))),
            }
        }
    }

    fn clone_argv(argv: &CommandArgv) -> CommandArgv {
        CommandArgv::try_new(
            argv.arguments()
                .iter()
                .map(|argument| CommandArgument::try_new(argument.as_str().to_owned()))
                .collect(),
        )
        .expect("cloned argv is valid")
    }

    fn argv(value: &str) -> CommandArgv {
        CommandArgv::try_new(vec![CommandArgument::try_new(value.to_owned())])
            .expect("command argv is valid")
    }

    #[test]
    fn test_render_track_resolve_outcome_ready_preserves_success_contract() {
        let service = RecordingService {
            result: Ok(TrackResolveResult::Ready {
                phase: TrackPhase::InProgress,
                reason: DiagnosticText::new("track has unresolved tasks"),
                next_command: argv("/track:implement"),
            }),
            calls: Mutex::new(Vec::new()),
        };
        let items_dir = PathBuf::from("workspace/track/items");
        let outcome = render_track_resolve_outcome(
            &service,
            items_dir.clone(),
            Some("resolve-track".to_owned()),
        );
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Current phase: In Progress\nReason: track has unresolved tasks\nRecommended next command: /track:implement"
            )
        );
        assert_eq!(outcome.stderr, None);
        let calls = service.calls.lock().expect("service lock is available");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls.first().map(|(path, _)| path), Some(&items_dir));
    }

    #[test]
    fn test_render_track_resolve_outcome_blocked_includes_blocker_line() {
        let service = RecordingService {
            result: Ok(TrackResolveResult::Blocked {
                phase: TrackPhase::Blocked,
                reason: DiagnosticText::new("waiting on review"),
                next_command: argv("/track:status"),
                blocker: DiagnosticText::new("waiting on review"),
            }),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_resolve_outcome(
            &service,
            PathBuf::from("workspace/track/items"),
            Some("blocked-track".to_owned()),
        );
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Current phase: Blocked\nReason: waiting on review\nRecommended next command: /track:status\nBlocker: waiting on review"
            )
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_render_track_resolve_outcome_service_error_maps_to_prefixed_failure() {
        let service = RecordingService {
            result: Err("track not found: missing".to_owned()),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_resolve_outcome(
            &service,
            PathBuf::from("workspace/track/items"),
            Some("missing".to_owned()),
        );
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] resolve failed: track not found: missing")
        );
        assert_eq!(outcome.stdout, None);
    }

    #[test]
    fn test_render_track_resolve_outcome_invalid_items_dir_preserves_legacy_message() {
        let service = RecordingService {
            result: Ok(TrackResolveResult::Ready {
                phase: TrackPhase::Planning,
                reason: DiagnosticText::new("track is planned"),
                next_command: argv("/track:implement"),
            }),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_resolve_outcome(
            &service,
            PathBuf::from("not-items"),
            Some("id".to_owned()),
        );
        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.as_deref().is_some_and(|stderr| {
            stderr.contains("--items-dir must point to '<project-root>/track/items'")
        }));
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }
}
