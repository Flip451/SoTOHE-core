//! Primary adapter for `sotp test-obligation evaluate`.

use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use usecase::test_obligation::evaluate::{
    EvaluateTestObligationsApplicationService, EvaluateTestObligationsCommand,
};
use usecase::{DiagnosticMessage, TrackId};

use crate::render::CommandOutcome;

use super::{default_catalogue_paths, resolve_track_id};

/// cli_driver-local DTO for `sotp test-obligation evaluate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationEvaluateInput {
    track_id: Option<TrackId>,
    current_branch: DiagnosticMessage,
}

impl TestObligationEvaluateInput {
    /// Constructor for [`TestObligationEvaluateInput`].
    #[must_use]
    pub fn new(track_id: Option<TrackId>, current_branch: DiagnosticMessage) -> Self {
        Self { track_id, current_branch }
    }

    /// Builds the input from CLI string values.
    ///
    /// # Errors
    ///
    /// Returns an error when the optional track id or branch diagnostic is invalid.
    #[cfg(not(doc))]
    pub fn try_from_raw(track_id: Option<String>, current_branch: String) -> Result<Self, String> {
        let (track_id, current_branch) = super::parse_input_parts(track_id, current_branch)?;
        Ok(Self::new(track_id, current_branch))
    }

    /// Return the optional track id.
    #[must_use]
    pub fn track_id(&self) -> Option<&TrackId> {
        self.track_id.as_ref()
    }

    /// Return the caller's current git branch.
    #[must_use]
    pub fn current_branch(&self) -> &DiagnosticMessage {
        &self.current_branch
    }
}

/// Primary adapter for `test-obligation evaluate`.
pub struct TestObligationEvaluateHandler {
    pub service: Arc<dyn EvaluateTestObligationsApplicationService>,
}

impl TestObligationEvaluateHandler {
    /// Builds the handler over its application service.
    #[must_use]
    pub fn new(service: Arc<dyn EvaluateTestObligationsApplicationService>) -> Self {
        Self { service }
    }

    /// Handles one evaluate command.
    #[must_use]
    pub fn handle(&self, input: TestObligationEvaluateInput) -> CommandOutcome {
        let track_id = match resolve_track_id(input.track_id(), input.current_branch()) {
            Ok(track_id) => track_id,
            Err(message) => return CommandOutcome::failure(Some(message)),
        };
        let command = EvaluateTestObligationsCommand::new(
            track_id.clone(),
            input.current_branch().as_str().to_owned(),
            default_catalogue_paths(&track_id),
            PathBuf::from("track").join("items").join(track_id.as_ref()).join("spec.json"),
        );
        match block_on(self.service.execute(&command)) {
            Ok(output) => CommandOutcome::success(Some(format!(
                "[OK] test-obligation evaluate completed: pass={} fail={} pending={} known_bad_detection_rate={}",
                output.pass_count(),
                output.fail_count(),
                output.pending_count(),
                output.known_bad_detection_rate().value()
            ))),
            Err(error) => {
                CommandOutcome::failure(Some(format!("test-obligation evaluate failed: {error:?}")))
            }
        }
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => continue,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use usecase::test_obligation::evaluate::{
        EvaluateTestObligationsFuture, EvaluateTestObligationsOutcome,
    };

    use super::*;

    struct StubService;

    impl EvaluateTestObligationsApplicationService for StubService {
        fn execute<'a>(
            &'a self,
            _cmd: &'a EvaluateTestObligationsCommand,
        ) -> EvaluateTestObligationsFuture<'a> {
            Box::pin(async {
                Ok(EvaluateTestObligationsOutcome::new(
                    1,
                    0,
                    0,
                    usecase::DetectionRatePercent::try_new(100).unwrap(),
                ))
            })
        }
    }

    #[test]
    fn test_evaluate_handler_with_valid_input_returns_success() {
        let handler = TestObligationEvaluateHandler::new(Arc::new(StubService));
        let branch = DiagnosticMessage::try_new("track/test-track".to_owned()).unwrap();

        let outcome = handler.handle(TestObligationEvaluateInput::new(None, branch));

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("pass=1"));
    }
}
