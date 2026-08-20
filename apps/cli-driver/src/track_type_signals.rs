//! Primary-adapter conversion for TDDD type-signal evaluation.

use usecase::track_lifecycle::tddd::type_signals::{
    TrackTypeSignalsCommand, TrackTypeSignalsError, TrackTypeSignalsService,
};
use usecase::track_lifecycle::{TrackLayerSelection, TrackLifecycleIdInput, TrackSelection};

use crate::render::CommandOutcome;
use crate::track_tddd::TrackTdddTypeSignalsInput;

/// Converts typed CLI input, invokes the type-signals service, and renders its outcome.
pub(crate) fn render_track_type_signals_outcome(
    service: &dyn TrackTypeSignalsService,
    input: TrackTdddTypeSignalsInput,
) -> CommandOutcome {
    let command = match type_signals_input_to_command(input) {
        Ok(command) => command,
        Err(error) => return CommandOutcome::failure(Some(error)),
    };
    service
        .execute(command)
        .map(|_| CommandOutcome::success(None))
        .unwrap_or_else(type_signals_error_to_outcome)
}

fn type_signals_input_to_command(
    input: TrackTdddTypeSignalsInput,
) -> Result<TrackTypeSignalsCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase().map_err(|error| error.to_string())?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackTypeSignalsCommand { track, workspace_root, layer })
}

fn type_signals_error_to_outcome(error: TrackTypeSignalsError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}
