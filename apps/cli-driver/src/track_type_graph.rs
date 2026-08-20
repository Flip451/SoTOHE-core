//! Primary-adapter conversion for the removed TDDD type-graph command.

use usecase::track_lifecycle::tddd::type_graph::{
    TrackTypeGraphCommand, TrackTypeGraphError, TrackTypeGraphService,
};
use usecase::track_lifecycle::{TrackLayerSelection, TrackLifecycleIdInput, TrackSelection};

use crate::render::CommandOutcome;
use crate::track_tddd::TrackTdddTypeGraphInput;

/// Converts typed CLI input, invokes the type-graph service, and renders its failure.
pub(crate) fn render_track_type_graph_outcome(
    service: &dyn TrackTypeGraphService,
    input: TrackTdddTypeGraphInput,
) -> CommandOutcome {
    let command = match type_graph_input_to_command(input) {
        Ok(command) => command,
        Err(error) => return CommandOutcome::failure(Some(error)),
    };
    match service.execute(command) {
        Ok(result) => match result {},
        Err(error) => type_graph_error_to_outcome(error),
    }
}

fn type_graph_input_to_command(
    input: TrackTdddTypeGraphInput,
) -> Result<TrackTypeGraphCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let items_dir = input.items_dir.into_usecase().map_err(|error| error.to_string())?;
    let workspace_root = input.workspace_root.into_usecase().map_err(|error| error.to_string())?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackTypeGraphCommand {
        track,
        items_dir,
        workspace_root,
        layer,
        cluster_depth: input.cluster_depth.into_usecase(),
        edges: input.edges.into_usecase(),
    })
}

fn type_graph_error_to_outcome(error: TrackTypeGraphError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}
