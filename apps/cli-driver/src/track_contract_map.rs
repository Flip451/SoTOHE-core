//! Primary-adapter conversion and rendering for the TDDD contract map.

use usecase::track_lifecycle::tddd::contract_map::{
    TrackContractMapCommand, TrackContractMapError, TrackContractMapResult, TrackContractMapService,
};
use usecase::track_lifecycle::{TrackLayerFilter, TrackLifecycleIdInput, TrackSelection};

use crate::render::CommandOutcome;
use crate::track_tddd::TrackTdddContractMapInput;

/// Converts typed CLI input, invokes the contract-map service, and renders its result.
pub(crate) fn render_track_contract_map_outcome(
    service: &dyn TrackContractMapService,
    input: TrackTdddContractMapInput,
) -> CommandOutcome {
    let command = match input_to_command(input) {
        Ok(command) => command,
        Err(error) => return CommandOutcome::failure(Some(error)),
    };
    service
        .execute(command)
        .map(render_contract_map_result)
        .unwrap_or_else(contract_map_error_to_outcome)
}

fn input_to_command(input: TrackTdddContractMapInput) -> Result<TrackContractMapCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let items_dir = input.items_dir.into_usecase().map_err(|error| error.to_string())?;
    let workspace_root = input.workspace_root.into_usecase().map_err(|error| error.to_string())?;
    let layers = input.layers.map(|layers| layers.into_usecase()).unwrap_or(TrackLayerFilter::All);
    Ok(TrackContractMapCommand { track, items_dir, workspace_root, layers })
}

fn render_contract_map_result(result: TrackContractMapResult) -> CommandOutcome {
    let warnings = if result.warnings.is_empty() {
        String::new()
    } else {
        format!(", warnings={:?}", result.warnings)
    };
    CommandOutcome::success(Some(format!(
        "[OK] contract-map: wrote track/items/{}/contract-map.md (layers={}, entries={}{warnings})",
        result.track_id,
        result.rendered_layers.value(),
        result.catalogue_entries.value(),
    )))
}

fn contract_map_error_to_outcome(error: TrackContractMapError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::adr_baseline::TrackIdInput;
    use crate::track_tddd::{TrackItemsDirectoryInput, TrackLayersInput, TrackWorkspaceRootInput};

    struct RecordingService {
        result: Mutex<Option<Result<TrackContractMapResult, TrackContractMapError>>>,
    }

    impl TrackContractMapService for RecordingService {
        fn execute(
            &self,
            _command: TrackContractMapCommand,
        ) -> Result<TrackContractMapResult, TrackContractMapError> {
            self.result.lock().expect("service lock is available").take().expect("one result")
        }
    }

    fn input() -> TrackTdddContractMapInput {
        TrackTdddContractMapInput {
            track_id: Some(
                "contract-map-track".parse::<TrackIdInput>().expect("track id is valid"),
            ),
            items_dir: TrackItemsDirectoryInput::try_new("workspace/track/items".into())
                .expect("items directory is valid"),
            workspace_root: TrackWorkspaceRootInput::try_from(std::path::PathBuf::from(
                "workspace",
            ))
            .expect("workspace root is valid"),
            layers: Some(TrackLayersInput::try_new("domain".to_owned()).expect("layer is valid")),
        }
    }

    fn result() -> TrackContractMapResult {
        TrackContractMapResult {
            track_id: usecase::TrackId::try_new("contract-map-track").expect("track id is valid"),
            rendered_layers: usecase::track_lifecycle::TrackRenderedLayerCount::new(1),
            catalogue_entries: usecase::track_lifecycle::TrackCatalogueEntryCount::new(2),
            warnings: Vec::new(),
        }
    }

    fn service(result: Result<TrackContractMapResult, TrackContractMapError>) -> RecordingService {
        RecordingService { result: Mutex::new(Some(result)) }
    }

    #[test]
    fn test_track_tddd_driver_contract_map_success_preserves_cli_format() {
        let outcome = render_track_contract_map_outcome(&service(Ok(result())), input());

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] contract-map: wrote track/items/contract-map-track/contract-map.md (layers=1, entries=2)"
            )
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_track_tddd_driver_contract_map_service_error_returns_failure() {
        let error = TrackContractMapError::ExecutionFailed(
            usecase::git_workflow::DiagnosticText::new("contract-map failed"),
        );
        let outcome = render_track_contract_map_outcome(&service(Err(error)), input());

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("contract-map failed"));
    }
}
