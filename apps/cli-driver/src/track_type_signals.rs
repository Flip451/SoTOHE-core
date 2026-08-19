//! Primary-adapter conversion for TDDD type-signal evaluation.

use usecase::track_lifecycle::tddd::type_signals::TrackTypeSignalsError;

use crate::render::CommandOutcome;
pub(crate) fn type_signals_error_to_outcome(error: TrackTypeSignalsError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}
