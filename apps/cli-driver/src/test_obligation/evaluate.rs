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
    /// Anchor used to make track-artifact paths (catalogue snapshots and the
    /// evaluate command's `spec_path`) absolute so the handler is not
    /// sensitive to the process cwd.
    workspace_root: PathBuf,
}

impl TestObligationEvaluateHandler {
    /// Builds the handler over its application service and the discovered
    /// workspace root. `workspace_root` is the anchor used when constructing
    /// catalogue paths and the evaluate spec path — pass the value the
    /// composition root obtained from git worktree discovery.
    #[must_use]
    pub fn new(
        service: Arc<dyn EvaluateTestObligationsApplicationService>,
        workspace_root: PathBuf,
    ) -> Self {
        Self { service, workspace_root }
    }

    /// Handles one evaluate command.
    #[must_use]
    pub fn handle(&self, input: TestObligationEvaluateInput) -> CommandOutcome {
        let track_id = match resolve_track_id(input.track_id(), input.current_branch()) {
            Ok(track_id) => track_id,
            Err(message) => return CommandOutcome::failure(Some(message)),
        };
        let spec_path = self
            .workspace_root
            .join("track")
            .join("items")
            .join(track_id.as_ref())
            .join("spec.json");
        let command = EvaluateTestObligationsCommand::new(
            track_id.clone(),
            input.current_branch().as_str().to_owned(),
            default_catalogue_paths(&self.workspace_root, &track_id),
            spec_path,
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

/// Drives `future` to completion on the current thread, parking between polls so pending futures do not busy-spin.
fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);

    impl std::task::Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
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
        let handler =
            TestObligationEvaluateHandler::new(Arc::new(StubService), PathBuf::from("/repo"));
        let branch = DiagnosticMessage::try_new("track/test-track".to_owned()).unwrap();

        let outcome = handler.handle(TestObligationEvaluateInput::new(None, branch));

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("pass=1"));
    }

    #[test]
    fn test_evaluate_handler_anchors_command_paths_at_workspace_root() {
        use std::sync::Mutex;
        // Regression: the handler used to build `catalogue_paths` and
        // `spec_path` from `PathBuf::from("track")`, which is cwd-relative
        // and blew up when `bin/sotp test-obligation ...` was invoked from a
        // repo subdirectory. Capture the command the service received and
        // compare it whole-for-whole with the command built from the same
        // absolute workspace anchor — command equality is the tightest
        // available assertion (the usecase command's fields are private).
        struct CapturingService {
            captured: Mutex<Option<EvaluateTestObligationsCommand>>,
        }

        impl EvaluateTestObligationsApplicationService for CapturingService {
            fn execute<'a>(
                &'a self,
                cmd: &'a EvaluateTestObligationsCommand,
            ) -> EvaluateTestObligationsFuture<'a> {
                *self.captured.lock().unwrap() = Some(cmd.clone());
                Box::pin(async {
                    Ok(EvaluateTestObligationsOutcome::new(
                        0,
                        0,
                        0,
                        usecase::DetectionRatePercent::try_new(100).unwrap(),
                    ))
                })
            }
        }

        let service = Arc::new(CapturingService { captured: Mutex::new(None) });
        let workspace_root = PathBuf::from("/discovered/workspace");
        let handler = TestObligationEvaluateHandler::new(service.clone(), workspace_root.clone());
        let branch = DiagnosticMessage::try_new("track/example".to_owned()).unwrap();

        let outcome = handler.handle(TestObligationEvaluateInput::new(None, branch));

        assert_eq!(outcome.exit_code, 0);
        let captured = service.captured.lock().unwrap().clone().unwrap();
        let track_id = usecase::TrackId::try_new("example").unwrap();
        let expected = EvaluateTestObligationsCommand::new(
            track_id.clone(),
            "track/example".to_owned(),
            default_catalogue_paths(&workspace_root, &track_id),
            workspace_root.join("track").join("items").join(track_id.as_ref()).join("spec.json"),
        );
        assert_eq!(captured, expected);
    }

    #[test]
    fn test_block_on_completes_self_waking_pending_future() {
        use std::pin::Pin;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct SelfWakingOnce {
            polled: AtomicBool,
        }

        impl Future for SelfWakingOnce {
            type Output = u32;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.polled.swap(true, Ordering::SeqCst) {
                    Poll::Ready(42)
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        }

        let result = block_on(SelfWakingOnce { polled: AtomicBool::new(false) });
        assert_eq!(result, 42);
    }
}
