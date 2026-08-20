//! Primary-adapter rendering for the `track spec-element-hash` command.

use std::collections::BTreeMap;

use usecase::track_lifecycle::tddd::spec_element_hash::{
    TrackSpecElementHashCommand, TrackSpecElementHashError, TrackSpecElementHashResult,
};
use usecase::track_lifecycle::{TrackLifecycleIdInput, TrackSelection, TrackSpecAnchorSelection};

use crate::render::CommandOutcome;
use crate::track_tddd::{TrackSpecAnchorInput, TrackTdddSpecElementHashInput};

pub(crate) fn spec_element_hash_input_to_command(
    input: TrackTdddSpecElementHashInput,
) -> Result<TrackSpecElementHashCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let items_dir = input.items_dir.into_usecase().map_err(|error| error.to_string())?;
    let anchor = input
        .anchor
        .map(TrackSpecAnchorInput::into_usecase)
        .unwrap_or(TrackSpecAnchorSelection::All);
    Ok(TrackSpecElementHashCommand { track, items_dir, anchor })
}

/// Renders a typed spec-element-hash result using the legacy CLI format.
pub(crate) fn render_track_spec_element_hash_result(
    result: TrackSpecElementHashResult,
) -> CommandOutcome {
    match result {
        TrackSpecElementHashResult::Single(hash) => CommandOutcome::success(Some(hash.to_hex())),
        TrackSpecElementHashResult::All(hashes) => {
            let hashes: BTreeMap<String, String> = hashes
                .into_iter()
                .map(|(anchor, hash)| (anchor.to_string(), hash.to_hex()))
                .collect();
            match serde_json::to_string_pretty(&hashes) {
                Ok(output) => CommandOutcome::success(Some(output)),
                Err(error) => CommandOutcome::failure(Some(format!(
                    "spec-element-hash output encoding failed: {error}"
                ))),
            }
        }
    }
}

/// Converts a typed spec-element-hash failure into a CLI failure outcome.
pub(crate) fn track_spec_element_hash_error_to_outcome(
    error: TrackSpecElementHashError,
) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}
