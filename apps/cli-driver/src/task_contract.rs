//! `task-contract` command family — primary adapter driver.
//!
//! [`TaskContractDriver`] holds two injected services:
//! - `Arc<dyn PreReviewGateService>` for `check` (liveness gate), and
//! - `Arc<dyn CoverageVerifyService>` for `coverage` (attribution completeness).
//!
//! It exposes `handle(input) -> CommandOutcome`.
//!
//! Input strings (`layer`, `track_id`) are validated into domain value objects
//! (`Option<LayerId>`, `TrackId`) inside `handle`, so that parsing errors are
//! surfaced as `CommandOutcome::failure` rather than panics.

use std::sync::Arc;

use usecase::LayerId;
use usecase::pre_review_gate::{
    CoverageVerifyCommand, CoverageVerifyService, PreReviewGateCommand, PreReviewGateError,
    PreReviewGateOutcome, PreReviewGateService, PreReviewGateViolation,
};
pub use usecase::pre_review_gate::{CoverageVerifyOutcome, CoverageViolation};
use usecase::{TrackId, ValidationError};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// TaskContractInput
// ---------------------------------------------------------------------------

/// Typed input for the task-contract command family.
///
/// `Check`: run the pre-review liveness gate for the given optional TDDD
/// layer and track. `layer` is an optional opaque CLI string validated into
/// `Option<LayerId>` by this primary adapter; when `None`, the gate iterates
/// all 6 canonical TDDD layers internally. `track_id` is validated into
/// `TrackId`.
///
/// `Coverage`: run the attribution-completeness check for the given track.
/// Always iterates all 6 canonical TDDD layers (no per-layer flag).
///
/// The filesystem root is not part of this primary-adapter input;
/// `cli_composition` builds the injected services with the requested `items_dir`
/// before constructing the driver.
#[derive(Debug, Clone)]
pub enum TaskContractInput {
    /// Run the pre-review liveness gate check.
    Check {
        /// Optional TDDD layer (e.g. `Some("domain")`, `Some("usecase")`).
        /// `None` iterates all 6 canonical TDDD layers.
        layer: Option<String>,
        /// Active track identifier.
        track_id: String,
    },
    /// Run the attribution-completeness coverage check.
    Coverage {
        /// Active track identifier.
        track_id: String,
    },
}

// ---------------------------------------------------------------------------
// TaskContractDriver
// ---------------------------------------------------------------------------

/// Primary adapter for the task-contract command family.
///
/// Holds two private service ports:
/// - `Arc<dyn PreReviewGateService>` for `check` (liveness gate).
/// - `Arc<dyn CoverageVerifyService>` for `coverage` (attribution completeness).
///
/// Dispatches [`TaskContractInput`] variants to the appropriate use-case
/// operation. Converts CLI strings to domain value objects (`TrackId`,
/// `Option<LayerId>`) and renders the outcome as a [`CommandOutcome`] (exit 0
/// on `Passed`, exit 1 with violation list on `Blocked`).
pub struct TaskContractDriver {
    check_service: Arc<dyn PreReviewGateService>,
    coverage_service: Arc<dyn CoverageVerifyService>,
}

impl std::fmt::Debug for TaskContractDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskContractDriver").finish_non_exhaustive()
    }
}

impl TaskContractDriver {
    /// Construct a `TaskContractDriver` by injecting the check service
    /// ([`PreReviewGateService`] for liveness) and coverage service
    /// ([`CoverageVerifyService`] for attribution completeness).
    #[must_use]
    pub fn new(
        check_service: Arc<dyn PreReviewGateService>,
        coverage_service: Arc<dyn CoverageVerifyService>,
    ) -> Self {
        Self { check_service, coverage_service }
    }

    /// Dispatch a [`TaskContractInput`] variant to the appropriate use-case
    /// operation and render the result as a [`CommandOutcome`].
    pub fn handle(&self, input: TaskContractInput) -> CommandOutcome {
        match input {
            TaskContractInput::Check { layer, track_id } => self.handle_check(layer, track_id),
            TaskContractInput::Coverage { track_id } => self.handle_coverage(track_id),
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

        match self.check_service.check(cmd) {
            Ok(PreReviewGateOutcome::Passed) => CommandOutcome::success(None),
            Ok(PreReviewGateOutcome::Blocked { violations, .. }) => {
                let message = render_check_violations(&violations);
                CommandOutcome::failure(Some(message))
            }
            Err(e) => CommandOutcome::failure(Some(render_gate_error(e))),
        }
    }

    fn handle_coverage(&self, track_id: String) -> CommandOutcome {
        let tid = match TrackId::try_new(track_id.clone()) {
            Ok(t) => t,
            Err(e) => {
                return CommandOutcome::failure(Some(format!(
                    "invalid track_id '{track_id}': {e}"
                )));
            }
        };

        let cmd = CoverageVerifyCommand { track_id: tid };

        match self.coverage_service.verify_coverage(cmd) {
            Ok(CoverageVerifyOutcome::Passed) => CommandOutcome::success(None),
            Ok(CoverageVerifyOutcome::Blocked { violations, .. }) => {
                let message = render_coverage_violations(&violations);
                CommandOutcome::failure(Some(message))
            }
            Err(e) => CommandOutcome::failure(Some(render_gate_error(e))),
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn render_gate_error(e: PreReviewGateError) -> String {
    match e {
        PreReviewGateError::TaskContractNotFound => {
            "task-contract.json not found for track — run the impl-planner to generate it"
                .to_owned()
        }
        PreReviewGateError::TaskContractReadFailed { message } => {
            format!("failed to read task-contract.json: {message}")
        }
        PreReviewGateError::SignalReadFailed { layer, message } => {
            format!("failed to read type-signals for layer '{layer}': {message}")
        }
        PreReviewGateError::ImplPlanReadFailed { message } => {
            format!("failed to read impl-plan.json: {message}")
        }
    }
}

fn render_check_violations(violations: &[PreReviewGateViolation]) -> String {
    let mut lines: Vec<String> = vec![
        "pre-review liveness gate BLOCKED — the following violations must be resolved:".to_owned(),
    ];

    for v in violations {
        let line = match v {
            PreReviewGateViolation::MissingTaskContract => {
                "  - MissingTaskContract: task-contract.json is absent for this track".to_owned()
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

fn render_coverage_violations(violations: &[CoverageViolation]) -> String {
    let mut lines: Vec<String> = vec![
        "task-contract coverage BLOCKED — attribution violations must be resolved:".to_owned(),
    ];

    for v in violations {
        let line = match v {
            CoverageViolation::MissingTaskContract => {
                "  - MissingTaskContract: task-contract.json is absent for this track".to_owned()
            }
            CoverageViolation::OrphanEntry { entry } => {
                format!(
                    "  - OrphanEntry: {} / {} has no task attribution in task-contract.json",
                    entry.layer().as_ref(),
                    entry.entry_key().as_str()
                )
            }
            CoverageViolation::InvalidEntryRef { entry, reason } => {
                format!(
                    "  - InvalidEntryRef: {} / {} — {reason}",
                    entry.layer().as_ref(),
                    entry.entry_key().as_str()
                )
            }
            CoverageViolation::MissingSignalDocument { layer } => {
                format!(
                    "  - MissingSignalDocument: {}-type-signals.json is absent; \
                     run `bin/sotp signal calc-impl-catalog` to generate it",
                    layer.as_ref()
                )
            }
            CoverageViolation::InvalidTaskRef { task_id, entry_keys } => {
                let keys = entry_keys
                    .iter()
                    .map(|e| format!("{}/{}", e.layer().as_ref(), e.entry_key().as_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "  - InvalidTaskRef: task '{}' is not in impl-plan.json but task-contract.json \
                     attributes entries [{keys}]; either update impl-plan.json or remove the stale \
                     attributions",
                    task_id.as_ref()
                )
            }
        };
        lines.push(line);
    }

    lines.join("\n")
}
