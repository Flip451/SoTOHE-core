//! Primary adapter for `sotp test-obligation check`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::test_obligation::check::{
    CheckTestObligationsApplicationService, CheckTestObligationsCommand,
};
use usecase::{DiagnosticMessage, TrackId};

use crate::render::CommandOutcome;

use super::catalogue_command_input;

/// cli_driver-local DTO for `sotp test-obligation check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationCheckInput {
    track_id: Option<TrackId>,
    current_branch: DiagnosticMessage,
}

impl TestObligationCheckInput {
    /// Constructor for [`TestObligationCheckInput`].
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

/// Primary adapter for `test-obligation check`.
pub struct TestObligationCheckHandler {
    pub service: Arc<dyn CheckTestObligationsApplicationService>,
    /// Anchor used to make track-artifact paths (catalogue snapshots) absolute
    /// so the handler is not sensitive to the process cwd.
    workspace_root: PathBuf,
}

impl TestObligationCheckHandler {
    /// Builds the handler over its application service and the discovered
    /// workspace root. `workspace_root` is the anchor used when constructing
    /// catalogue paths — pass the value the composition root obtained from
    /// git worktree discovery.
    #[must_use]
    pub fn new(
        service: Arc<dyn CheckTestObligationsApplicationService>,
        workspace_root: PathBuf,
    ) -> Self {
        Self { service, workspace_root }
    }

    /// Handles one check command.
    #[must_use]
    pub fn handle(&self, input: TestObligationCheckInput) -> CommandOutcome {
        let command_input = match catalogue_command_input(
            &self.workspace_root,
            input.track_id(),
            input.current_branch(),
        ) {
            Ok(input) => input,
            Err(message) => return CommandOutcome::failure(Some(message)),
        };
        let command = CheckTestObligationsCommand::new(command_input);
        match self.service.execute(&command) {
            Ok(output) => CommandOutcome::success(Some(format!(
                "[OK] test-obligation check passed: resolved_edges={} uncited_findings={}",
                output.resolved_edges().len(),
                output.uncited_findings().len()
            ))),
            Err(error) => {
                CommandOutcome::failure(Some(format!("test-obligation check failed: {error:?}")))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct StubService;

    impl CheckTestObligationsApplicationService for StubService {
        fn execute(
            &self,
            _cmd: &CheckTestObligationsCommand,
        ) -> Result<
            usecase::test_obligation::check::CheckTestObligationsOutcome,
            usecase::test_obligation::errors::ObligationCheckError,
        > {
            Ok(usecase::test_obligation::check::CheckTestObligationsOutcome::new_empty_scope(
                Vec::new(),
            ))
        }
    }

    #[test]
    fn test_check_handler_with_valid_input_returns_success() {
        let handler =
            TestObligationCheckHandler::new(Arc::new(StubService), PathBuf::from("/repo"));
        let branch = DiagnosticMessage::try_new("track/test-track".to_owned()).unwrap();

        let outcome = handler.handle(TestObligationCheckInput::new(None, branch));

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("resolved_edges=0"));
    }
}
