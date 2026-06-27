//! `task-contract` command family — primary adapter driver.
//!
//! [`TaskContractDriver`] holds a single injected
//! [`usecase::pre_review_gate::PreReviewGateService`] and exposes
//! `handle(input) -> CommandOutcome`.
//!
//! Input strings (`layer`, `track_id`) are validated into domain value objects
//! (`Option<LayerId>`, `TrackId`) inside `handle`, so that parsing errors are
//! surfaced as `CommandOutcome::failure` rather than panics.

use std::sync::Arc;

use usecase::LayerId;
use usecase::pre_review_gate::{
    PreReviewGateCommand, PreReviewGateError, PreReviewGateOutcome, PreReviewGateService,
    PreReviewGateViolation,
};
use usecase::{TrackId, ValidationError};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// TaskContractInput
// ---------------------------------------------------------------------------

/// Typed input for the task-contract command family.
///
/// `Check`: run the pre-review conformance gate for the given optional TDDD
/// layer and track. `layer` is an optional opaque CLI string validated into
/// `Option<LayerId>` by this primary adapter; when `None`, the gate iterates
/// all 6 canonical TDDD layers internally. `track_id` is validated into
/// `TrackId`.
///
/// The filesystem root is not part of this primary-adapter input;
/// `cli_composition` builds the injected service with the requested `items_dir`
/// before constructing the driver.
#[derive(Debug, Clone)]
pub enum TaskContractInput {
    /// Run the pre-review conformance gate check.
    Check {
        /// Optional TDDD layer (e.g. `Some("domain")`, `Some("usecase")`).
        /// `None` iterates all 6 canonical TDDD layers.
        layer: Option<String>,
        /// Active track identifier.
        track_id: String,
    },
}

// ---------------------------------------------------------------------------
// TaskContractDriver
// ---------------------------------------------------------------------------

/// Primary adapter for the task-contract command family.
///
/// Holds a private `Arc<dyn PreReviewGateService>` and dispatches
/// [`TaskContractInput`] variants to the appropriate use-case operation.
/// Converts CLI strings to domain value objects (`TrackId`, `Option<LayerId>`)
/// and renders the [`PreReviewGateOutcome`] as a [`CommandOutcome`] (exit 0 on
/// `Passed`, exit 1 with violation list on `Blocked`).
pub struct TaskContractDriver {
    service: Arc<dyn PreReviewGateService>,
}

impl std::fmt::Debug for TaskContractDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskContractDriver").finish_non_exhaustive()
    }
}

impl TaskContractDriver {
    /// Construct a `TaskContractDriver` by injecting the primary application
    /// service port.
    #[must_use]
    pub fn new(service: Arc<dyn PreReviewGateService>) -> Self {
        Self { service }
    }

    /// Dispatch a [`TaskContractInput`] variant to the appropriate use-case
    /// operation and render the result as a [`CommandOutcome`].
    pub fn handle(&self, input: TaskContractInput) -> CommandOutcome {
        match input {
            TaskContractInput::Check { layer, track_id } => self.handle_check(layer, track_id),
        }
    }

    fn handle_check(&self, layer: Option<String>, track_id: String) -> CommandOutcome {
        // Parse optional layer CLI string into Option<LayerId>.
        let layer_opt = match layer {
            Some(layer_str) => match LayerId::try_new(layer_str.clone()) {
                Ok(l) => Some(l),
                Err(ValidationError::InvalidLayerId(v)) => {
                    return CommandOutcome::failure(Some(format!(
                        "invalid layer '{v}': must be a non-empty ASCII identifier"
                    )));
                }
                Err(e) => {
                    return CommandOutcome::failure(Some(format!(
                        "invalid layer '{layer_str}': {e}"
                    )));
                }
            },
            None => None,
        };

        let tid = match TrackId::try_new(track_id.clone()) {
            Ok(t) => t,
            Err(e) => {
                return CommandOutcome::failure(Some(format!(
                    "invalid track_id '{track_id}': {e}"
                )));
            }
        };

        let cmd = PreReviewGateCommand { track_id: tid, layer: layer_opt };

        match self.service.check(cmd) {
            Ok(PreReviewGateOutcome::Passed { conformance_summary }) => {
                CommandOutcome::success(Some(conformance_summary))
            }
            Ok(PreReviewGateOutcome::Blocked { violations, .. }) => {
                let message = render_violations(&violations);
                CommandOutcome::failure(Some(message))
            }
            Err(PreReviewGateError::TaskContractNotFound) => CommandOutcome::failure(Some(
                "task-contract.json not found for track — run the impl-planner to generate it"
                    .to_owned(),
            )),
            Err(PreReviewGateError::TaskContractReadFailed { message }) => CommandOutcome::failure(
                Some(format!("failed to read task-contract.json: {message}")),
            ),
            Err(PreReviewGateError::SignalReadFailed { layer, message }) => {
                CommandOutcome::failure(Some(format!(
                    "failed to read type-signals for layer '{layer}': {message}"
                )))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn render_violations(violations: &[PreReviewGateViolation]) -> String {
    let mut lines: Vec<String> =
        vec!["pre-review gate BLOCKED — the following violations must be resolved:".to_owned()];

    for v in violations {
        let line = match v {
            PreReviewGateViolation::MissingTaskContract => {
                "  - MissingTaskContract: task-contract.json is absent for this track".to_owned()
            }
            PreReviewGateViolation::OrphanEntry { entry } => {
                format!(
                    "  - OrphanEntry: {} / {} has no task attribution in task-contract.json",
                    entry.layer().as_ref(),
                    entry.entry_key().as_str()
                )
            }
            PreReviewGateViolation::InvalidEntryRef { entry, reason } => {
                format!(
                    "  - InvalidEntryRef: {} / {} — {reason}",
                    entry.layer().as_ref(),
                    entry.entry_key().as_str()
                )
            }
            PreReviewGateViolation::NonBlueSignal { entry, signal } => {
                format!(
                    "  - NonBlueSignal: {} / {} has signal {:?} (expected Blue)",
                    entry.layer().as_ref(),
                    entry.entry_key().as_str(),
                    signal
                )
            }
        };
        lines.push(line);
    }

    lines.join("\n")
}
