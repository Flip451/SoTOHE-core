//! Primary adapter for `sotp test-obligation results`.

use std::sync::Arc;

use usecase::TrackId;
use usecase::test_obligation::results::{
    TestObligationResultsApplicationService, TestObligationResultsCommand,
};

use crate::render::CommandOutcome;

use super::resolve_track_id;

/// cli_driver-local DTO for `sotp test-obligation results`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationResultsInput {
    track_id: Option<TrackId>,
}

impl TestObligationResultsInput {
    /// Constructor for [`TestObligationResultsInput`].
    #[must_use]
    pub fn new(track_id: Option<TrackId>) -> Self {
        Self { track_id }
    }

    /// Builds the input from CLI string values.
    ///
    /// # Errors
    ///
    /// Returns an error when the optional track id or branch diagnostic is invalid.
    #[cfg(not(doc))]
    pub fn try_from_raw(track_id: Option<String>, current_branch: String) -> Result<Self, String> {
        let (track_id, current_branch) = super::parse_input_parts(track_id, current_branch)?;
        let track_id = resolve_track_id(track_id.as_ref(), &current_branch)?;
        Ok(Self::new(Some(track_id)))
    }

    /// Return the optional track id.
    #[must_use]
    pub fn track_id(&self) -> Option<&TrackId> {
        self.track_id.as_ref()
    }
}

/// Primary adapter for `test-obligation results`.
pub struct TestObligationResultsHandler {
    pub service: Arc<dyn TestObligationResultsApplicationService>,
}

impl TestObligationResultsHandler {
    /// Builds the handler over its application service.
    #[must_use]
    pub fn new(service: Arc<dyn TestObligationResultsApplicationService>) -> Self {
        Self { service }
    }

    /// Handles one results command.
    #[must_use]
    pub fn handle(&self, input: TestObligationResultsInput) -> CommandOutcome {
        let Some(track_id) = input.track_id().cloned() else {
            return CommandOutcome::failure(Some("--track-id is required".to_owned()));
        };
        let command = TestObligationResultsCommand::new(track_id);
        match self.service.execute(&command) {
            Ok(output) => {
                let mut text = String::new();
                text.push_str("test-obligation results\n");
                for lane in output.lane_summaries() {
                    text.push_str(&format!(
                        "{:?}:{} pass={} fail={} pending={}\n",
                        lane.chain_name(),
                        lane.layer().as_ref(),
                        lane.pass_count(),
                        lane.fail_count(),
                        lane.pending_count()
                    ));
                }
                text.push_str(&format!(
                    "records={} uncited_findings={}",
                    output.records().len(),
                    output.uncited_findings().len()
                ));
                CommandOutcome::success(Some(text))
            }
            Err(error) => {
                CommandOutcome::failure(Some(format!("test-obligation results failed: {error:?}")))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use usecase::test_obligation::results::TestObligationResultsOutput;

    use super::*;

    struct StubService;

    impl TestObligationResultsApplicationService for StubService {
        fn execute(
            &self,
            _cmd: &TestObligationResultsCommand,
        ) -> Result<
            TestObligationResultsOutput,
            usecase::test_obligation::errors::ObligationResultsError,
        > {
            Ok(TestObligationResultsOutput::new(Vec::new(), Vec::new(), Vec::new()))
        }
    }

    #[test]
    fn test_results_handler_with_valid_input_returns_success() {
        let handler = TestObligationResultsHandler::new(Arc::new(StubService));
        let input =
            TestObligationResultsInput::try_from_raw(None, "track/test-track".to_owned()).unwrap();

        let outcome = handler.handle(input);

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("records=0"));
    }
}
