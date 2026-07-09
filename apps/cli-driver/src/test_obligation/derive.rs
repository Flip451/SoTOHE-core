//! Primary adapter for `sotp test-obligation derive`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::test_obligation::derive::{
    DeriveTestObligationsApplicationService, DeriveTestObligationsCommand,
};
use usecase::{DiagnosticMessage, TrackId};

use crate::render::CommandOutcome;

use super::catalogue_command_input;

/// cli_driver-local DTO for `sotp test-obligation derive`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationDeriveInput {
    track_id: Option<TrackId>,
    current_branch: DiagnosticMessage,
}

impl TestObligationDeriveInput {
    /// Constructor for [`TestObligationDeriveInput`].
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

/// Primary adapter for `test-obligation derive`.
pub struct TestObligationDeriveHandler {
    pub service: Arc<dyn DeriveTestObligationsApplicationService>,
    /// Anchor used to make track-artifact paths (catalogue snapshots) absolute
    /// so the handler is not sensitive to the process cwd.
    workspace_root: PathBuf,
}

impl TestObligationDeriveHandler {
    /// Builds the handler over its application service and the discovered
    /// workspace root. `workspace_root` is the anchor used when constructing
    /// catalogue paths — pass the value the composition root obtained from
    /// git worktree discovery.
    #[must_use]
    pub fn new(
        service: Arc<dyn DeriveTestObligationsApplicationService>,
        workspace_root: PathBuf,
    ) -> Self {
        Self { service, workspace_root }
    }

    /// Handles one derive command.
    #[must_use]
    pub fn handle(&self, input: TestObligationDeriveInput) -> CommandOutcome {
        let command_input = match catalogue_command_input(
            &self.workspace_root,
            input.track_id(),
            input.current_branch(),
        ) {
            Ok(input) => input,
            Err(message) => return CommandOutcome::failure(Some(message)),
        };
        let command = DeriveTestObligationsCommand::new(command_input);
        match self.service.execute(&command) {
            Ok(()) => {
                CommandOutcome::success(Some("[OK] test-obligation derive completed".to_owned()))
            }
            Err(error) => {
                CommandOutcome::failure(Some(format!("test-obligation derive failed: {error:?}")))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct StubService {
        calls: Mutex<usize>,
    }

    impl DeriveTestObligationsApplicationService for StubService {
        fn execute(
            &self,
            _cmd: &DeriveTestObligationsCommand,
        ) -> Result<(), usecase::test_obligation::errors::ObligationDeriveError> {
            *self.calls.lock().unwrap() += 1;
            Ok(())
        }
    }

    fn branch() -> DiagnosticMessage {
        DiagnosticMessage::try_new("track/test-track".to_owned()).unwrap()
    }

    #[test]
    fn test_derive_handler_with_valid_input_invokes_service() {
        let service = Arc::new(StubService { calls: Mutex::new(0) });
        let handler = TestObligationDeriveHandler::new(service.clone(), PathBuf::from("/repo"));

        let outcome = handler.handle(TestObligationDeriveInput::new(None, branch()));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(*service.calls.lock().unwrap(), 1);
    }
}
