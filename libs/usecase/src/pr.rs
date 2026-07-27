//! PR command application service port (usecase layer).
//!
//! Provides a thin `PrCommandService` trait that the `cli_driver::PrDriver`
//! calls for all non-polling PR operations (push, ensure, status,
//! wait-and-merge, trigger-review, review-cycle). Polling is delegated to
//! the separate [`crate::pr_review_polling::PrReviewPollingService`].
//!
//! The function-pointer interactor pattern (mirroring `RunReviewInteractor`)
//! lets `cli_composition` inject the full infrastructure wiring without
//! violating the hexagonal boundary.

use std::sync::Arc;

// ── PrCommandOutput ───────────────────────────────────────────────────────────

/// Primitive output from a PR command operation.
///
/// Uses only stdlib types so the driver and usecase layers never import
/// infrastructure or domain types.
#[derive(Debug, Clone)]
pub struct PrCommandOutput {
    /// Optional stdout message.
    pub stdout: Option<String>,
    /// Optional stderr message.
    pub stderr: Option<String>,
    /// Exit code: 0 = success, non-zero = failure.
    pub exit_code: u8,
}

impl PrCommandOutput {
    /// Create a success output with optional message.
    #[must_use]
    pub fn success(msg: Option<String>) -> Self {
        Self { stdout: msg, stderr: None, exit_code: 0 }
    }

    /// Create a failure output with optional message.
    #[must_use]
    pub fn failure(msg: Option<String>) -> Self {
        Self { stdout: None, stderr: msg, exit_code: 1 }
    }

    /// Create an output with all fields specified.
    #[must_use]
    pub fn with_exit_code(stdout: Option<String>, stderr: Option<String>, exit_code: u8) -> Self {
        Self { stdout, stderr, exit_code }
    }
}

// ── PrCommandService ──────────────────────────────────────────────────────────

/// Application service (primary port) for the PR command family.
///
/// Covers all non-polling operations: push, ensure, status,
/// wait-and-merge, trigger-review, and review-cycle. The polling operation
/// (`pr_poll_review`) is handled by the separate
/// [`crate::pr_review_polling::PrReviewPollingService`].
///
/// All parameters use stdlib types only so that `cli_driver` never imports
/// `infrastructure` or `domain` types (CN-01 / architecture-rules.json).
pub trait PrCommandService: Send + Sync {
    /// Push the current track branch to origin.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    fn push(&self, track_id: Option<String>) -> PrCommandOutput;

    /// Create or reuse a PR for the current track branch.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    fn ensure(&self, track_id: Option<String>, base: String) -> PrCommandOutput;

    /// Show current PR check status.
    fn status(&self, pr: String) -> PrCommandOutput;

    /// Poll PR checks until they pass, then merge.
    fn wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> PrCommandOutput;

    /// Post `@codex review` comment on a PR.
    fn trigger_review(&self, pr: String) -> PrCommandOutput;

    /// Poll for a Codex review completion after triggering.
    fn poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> PrCommandOutput;

    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    fn review_cycle(&self, track_id: Option<String>, resume: bool) -> PrCommandOutput;
}

// ── PrCommandInteractor ───────────────────────────────────────────────────────

/// Validated identifier for a pull request selector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrIdentifier(String);

impl PrIdentifier {
    /// Returns `None` when `value` is empty.
    #[must_use]
    pub fn try_new(value: String) -> Option<Self> {
        if value.is_empty() { None } else { Some(Self(value)) }
    }

    /// Returns the selector as supplied by the driver.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the identifier satisfies its non-empty invariant.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
    }
}

/// Polling interval expressed in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrPollIntervalSeconds(u64);

impl PrPollIntervalSeconds {
    /// Creates an interval, including zero for the explicit no-delay case.
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the interval in seconds.
    #[must_use]
    pub fn as_secs(self) -> u64 {
        self.0
    }
}

/// Polling timeout expressed in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrPollTimeoutSeconds(u64);

impl PrPollTimeoutSeconds {
    /// Creates a timeout, including zero for the explicit no-poll case.
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the timeout in seconds.
    #[must_use]
    pub fn as_secs(self) -> u64 {
        self.0
    }
}

/// Review-cycle start mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrReviewCycleMode {
    /// Start a new cycle.
    Start,
    /// Resume saved trigger state.
    Resume,
}

/// Compatibility override supplied through the legacy `--track-id` argument.
///
/// This retains the exact transport text because an active `track/<id>` branch
/// remains authoritative when the PR adapter resolves its branch context.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrTrackIdOverride(String);

impl PrTrackIdOverride {
    /// Creates an override without validation or normalization.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the exact override text supplied by the driver.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Explicit non-empty PR base override.
///
/// Whitespace is meaningful to preserve the legacy command boundary exactly;
/// only the exact empty string denotes an omitted base.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrBaseOverride(String);

impl PrBaseOverride {
    /// Returns `None` only when `value` is exactly empty.
    #[must_use]
    pub fn try_new(value: String) -> Option<Self> {
        if value.is_empty() { None } else { Some(Self(value)) }
    }

    /// Returns the exact base text supplied by the driver.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether this override satisfies the exact-nonempty invariant.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
    }
}

/// Typed application command for the PR command family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrCommand {
    /// Push a track branch.
    Push { track_id: Option<PrTrackIdOverride> },
    /// Ensure a PR exists.
    Ensure { track_id: Option<PrTrackIdOverride>, base: Option<PrBaseOverride> },
    /// Read check status.
    Status(PrIdentifier),
    /// Wait for checks and merge the PR.
    WaitAndMerge {
        pr: PrIdentifier,
        interval: PrPollIntervalSeconds,
        timeout: PrPollTimeoutSeconds,
        method: Option<domain::branch_strategy::MergeMethod>,
    },
    /// Trigger a review.
    TriggerReview(PrIdentifier),
    /// Poll for a review result.
    PollReview {
        pr: PrIdentifier,
        trigger_timestamp: domain::Timestamp,
        interval: PrPollIntervalSeconds,
        timeout: PrPollTimeoutSeconds,
    },
    /// Run the complete review cycle.
    ReviewCycle { track_id: Option<PrTrackIdOverride>, mode: PrReviewCycleMode },
}

/// Typed driven port for PR command execution.
pub trait PrCommandPort: Send + Sync {
    /// Executes the command synchronously at the blocking CLI boundary.
    fn execute(&self, command: PrCommand) -> PrCommandOutput;
}

/// Dependency-bearing implementation of [`PrCommandService`].
pub struct PrCommandInteractor {
    port: Arc<dyn PrCommandPort>,
}

impl PrCommandInteractor {
    /// Creates an interactor with a typed PR command port.
    #[must_use]
    pub fn new(port: Arc<dyn PrCommandPort>) -> Self {
        Self { port }
    }
}

impl PrCommandService for PrCommandInteractor {
    fn push(&self, track_id: Option<String>) -> PrCommandOutput {
        self.port.execute(PrCommand::Push { track_id: track_id.map(PrTrackIdOverride::new) })
    }

    fn ensure(&self, track_id: Option<String>, base: String) -> PrCommandOutput {
        let track_id = track_id.map(PrTrackIdOverride::new);
        let base = PrBaseOverride::try_new(base);
        self.port.execute(PrCommand::Ensure { track_id, base })
    }

    fn status(&self, pr: String) -> PrCommandOutput {
        match PrIdentifier::try_new(pr) {
            Some(pr) => self.port.execute(PrCommand::Status(pr)),
            None => validation_failure("PR identifier must not be empty".to_owned()),
        }
    }

    fn wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> PrCommandOutput {
        let Some(pr) = PrIdentifier::try_new(pr) else {
            return validation_failure("PR identifier must not be empty".to_owned());
        };
        let method = match parse_merge_method(method) {
            Ok(method) => method,
            Err(error) => return validation_failure(error),
        };
        self.port.execute(PrCommand::WaitAndMerge {
            pr,
            interval: PrPollIntervalSeconds::new(interval),
            timeout: PrPollTimeoutSeconds::new(timeout),
            method,
        })
    }

    fn trigger_review(&self, pr: String) -> PrCommandOutput {
        match PrIdentifier::try_new(pr) {
            Some(pr) => self.port.execute(PrCommand::TriggerReview(pr)),
            None => validation_failure("PR identifier must not be empty".to_owned()),
        }
    }

    fn poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> PrCommandOutput {
        let Some(pr) = PrIdentifier::try_new(pr) else {
            return validation_failure("PR identifier must not be empty".to_owned());
        };
        let trigger_timestamp = match domain::Timestamp::new(trigger_timestamp) {
            Ok(timestamp) => timestamp,
            Err(error) => return validation_failure(error.to_string()),
        };
        self.port.execute(PrCommand::PollReview {
            pr,
            trigger_timestamp,
            interval: PrPollIntervalSeconds::new(interval),
            timeout: PrPollTimeoutSeconds::new(timeout),
        })
    }

    fn review_cycle(&self, track_id: Option<String>, resume: bool) -> PrCommandOutput {
        self.port.execute(PrCommand::ReviewCycle {
            track_id: track_id.map(PrTrackIdOverride::new),
            mode: if resume { PrReviewCycleMode::Resume } else { PrReviewCycleMode::Start },
        })
    }
}

fn parse_merge_method(value: String) -> Result<Option<domain::MergeMethod>, String> {
    match value.as_str() {
        "" => Ok(None),
        "merge" => Ok(Some(domain::MergeMethod::Merge)),
        "squash" => Ok(Some(domain::MergeMethod::Squash)),
        "rebase" => Ok(Some(domain::MergeMethod::Rebase)),
        _ => Err(format!("invalid merge method: {value}")),
    }
}

fn validation_failure(message: String) -> PrCommandOutput {
    PrCommandOutput::failure(Some(message))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct RecordingPort {
        commands: Mutex<Vec<PrCommand>>,
        output: PrCommandOutput,
    }

    impl RecordingPort {
        fn new() -> Self {
            Self::with_output(PrCommandOutput::success(Some("delegated".to_owned())))
        }

        fn with_output(output: PrCommandOutput) -> Self {
            Self { commands: Mutex::new(Vec::new()), output }
        }
    }

    impl PrCommandPort for RecordingPort {
        fn execute(&self, command: PrCommand) -> PrCommandOutput {
            if let Ok(mut commands) = self.commands.lock() {
                commands.push(command);
            }
            self.output.clone()
        }
    }

    #[test]
    fn test_pr_command_interactor_push_with_valid_track_delegates_command() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.push(Some("valid-track".to_owned()));

        assert_eq!(output.stdout.as_deref(), Some("delegated"));
        let commands = port.commands.lock().unwrap();
        assert!(
            matches!(commands.as_slice(), [PrCommand::Push { track_id: Some(id) }] if id.as_str() == "valid-track")
        );
    }

    #[test]
    fn test_pr_command_interactor_push_preserves_port_output_and_track_payload() {
        let port = Arc::new(RecordingPort::with_output(PrCommandOutput::with_exit_code(
            Some("persisted stdout".to_owned()),
            Some("persisted stderr".to_owned()),
            17,
        )));
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.push(Some("preserved-track".to_owned()));

        assert_eq!(output.stdout.as_deref(), Some("persisted stdout"));
        assert_eq!(output.stderr.as_deref(), Some("persisted stderr"));
        assert_eq!(output.exit_code, 17);
        let commands = port.commands.lock().unwrap();
        assert!(
            matches!(commands.as_slice(), [PrCommand::Push { track_id: Some(id) }] if id.as_str() == "preserved-track")
        );
    }

    #[test]
    fn test_pr_command_interactor_invalid_track_ids_preserve_compatibility_overrides() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let outputs = [
            interactor.push(Some("INVALID".to_owned())),
            interactor.ensure(Some("INVALID".to_owned()), String::new()),
            interactor.review_cycle(Some("INVALID".to_owned()), false),
        ];

        assert!(outputs.iter().all(|output| output.exit_code == 0));
        let commands = port.commands.lock().unwrap();
        assert!(matches!(
            commands.as_slice(),
            [
                PrCommand::Push { track_id: Some(push_track_id) },
                PrCommand::Ensure { track_id: Some(ensure_track_id), base: None },
                PrCommand::ReviewCycle { track_id: Some(review_track_id), mode: PrReviewCycleMode::Start },
            ] if push_track_id.as_str() == "INVALID"
                && ensure_track_id.as_str() == "INVALID"
                && review_track_id.as_str() == "INVALID"
        ));
    }

    #[test]
    fn test_pr_command_interactor_ensure_with_spaced_base_preserves_exact_value() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.ensure(None, " main ".to_owned());

        assert_eq!(output.exit_code, 0);
        let commands = port.commands.lock().unwrap();
        assert!(matches!(
            commands.as_slice(),
            [PrCommand::Ensure { base: Some(base), .. }] if base.as_str() == " main " && base.is_valid()
        ));
    }

    #[test]
    fn test_pr_base_override_with_empty_value_returns_none() {
        assert_eq!(PrBaseOverride::try_new(String::new()), None);
    }

    #[test]
    fn test_pr_command_interactor_ensure_with_omitted_base_delegates_none() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.ensure(None, String::new());

        assert_eq!(output.exit_code, 0);
        let commands = port.commands.lock().unwrap();
        assert!(matches!(commands.as_slice(), [PrCommand::Ensure { track_id: None, base: None }]));
    }

    #[test]
    fn test_pr_command_interactor_status_with_empty_identifier_returns_failure() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.status(String::new());

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stderr.as_deref(), Some("PR identifier must not be empty"));
        assert!(port.commands.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pr_command_interactor_wait_and_merge_with_empty_identifier_returns_failure() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.wait_and_merge(String::new(), 1, 1, "merge".to_owned());

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stderr.as_deref(), Some("PR identifier must not be empty"));
        assert!(port.commands.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pr_command_interactor_wait_and_merge_with_invalid_method_returns_failure() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.wait_and_merge("123".to_owned(), 1, 1, "invalid".to_owned());

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stderr.as_deref(), Some("invalid merge method: invalid"));
        assert!(port.commands.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pr_command_interactor_trigger_review_with_empty_identifier_returns_failure() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.trigger_review(String::new());

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stderr.as_deref(), Some("PR identifier must not be empty"));
        assert!(port.commands.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pr_command_interactor_poll_review_with_empty_identifier_returns_failure() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.poll_review(String::new(), "2026-07-26T00:00:00Z".to_owned(), 1, 1);

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stderr.as_deref(), Some("PR identifier must not be empty"));
        assert!(port.commands.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pr_command_interactor_poll_review_with_invalid_timestamp_returns_failure() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output = interactor.poll_review("123".to_owned(), "invalid".to_owned(), 1, 1);

        assert_eq!(output.exit_code, 1);
        assert!(output.stderr.as_deref().is_some_and(|message| message.contains("timestamp")));
        assert!(port.commands.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pr_command_interactor_poll_review_with_zero_timeout_delegates_zero() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let output =
            interactor.poll_review("123".to_owned(), "2026-07-26T00:00:00Z".to_owned(), 0, 0);

        assert_eq!(output.exit_code, 0);
        let commands = port.commands.lock().unwrap();
        assert!(
            matches!(commands.as_slice(), [PrCommand::PollReview { interval, timeout, .. }] if interval.as_secs() == 0 && timeout.as_secs() == 0)
        );
    }

    #[test]
    fn test_pr_command_interactor_each_command_delegates_typed_variant() {
        let port = Arc::new(RecordingPort::new());
        let interactor = PrCommandInteractor::new(port.clone());

        let outputs = [
            interactor.push(Some("track-id".to_owned())),
            interactor.ensure(Some("track-id".to_owned()), "develop".to_owned()),
            interactor.status("123".to_owned()),
            interactor.wait_and_merge("123".to_owned(), 2, 3, "squash".to_owned()),
            interactor.trigger_review("123".to_owned()),
            interactor.poll_review("123".to_owned(), "2026-07-26T00:00:00Z".to_owned(), 0, 1),
            interactor.review_cycle(Some("track-id".to_owned()), false),
            interactor.review_cycle(Some("track-id".to_owned()), true),
        ];

        assert!(outputs.iter().all(|output| output.exit_code == 0));
        let commands = port.commands.lock().unwrap();
        assert!(matches!(
            commands.as_slice(),
            [
                PrCommand::Push { track_id: Some(push_track_id) },
                PrCommand::Ensure { track_id: Some(ensure_track_id), base: Some(base) },
                PrCommand::Status(status_pr),
                PrCommand::WaitAndMerge { pr: merge_pr, interval, timeout, method },
                PrCommand::TriggerReview(trigger_pr),
                PrCommand::PollReview { pr: poll_pr, trigger_timestamp, interval: poll_interval, timeout: poll_timeout },
                PrCommand::ReviewCycle { track_id: Some(start_track_id), mode: PrReviewCycleMode::Start },
                PrCommand::ReviewCycle { track_id: Some(resume_track_id), mode: PrReviewCycleMode::Resume },
            ] if push_track_id.as_str() == "track-id"
                && ensure_track_id.as_str() == "track-id"
                && base.as_str() == "develop"
                && status_pr.as_str() == "123"
                && merge_pr.as_str() == "123"
                && timeout.as_secs() == 3
                && matches!(method, Some(domain::MergeMethod::Squash))
                && trigger_pr.as_str() == "123"
                && poll_pr.as_str() == "123"
                && trigger_timestamp.as_str() == "2026-07-26T00:00:00Z"
                && poll_interval.as_secs() == 0
                && poll_timeout.as_secs() == 1
                && start_track_id.as_str() == "track-id"
                && resume_track_id.as_str() == "track-id"
        ));
    }

    #[test]
    fn test_pr_command_interactor_all_variants_preserve_port_output_through_typed_port() {
        let port = Arc::new(RecordingPort::with_output(PrCommandOutput::with_exit_code(
            Some("port stdout".to_owned()),
            Some("port stderr".to_owned()),
            29,
        )));
        let interactor = PrCommandInteractor::new(port.clone());

        let outputs = [
            interactor.push(Some("track-id".to_owned())),
            interactor.ensure(Some("track-id".to_owned()), "develop".to_owned()),
            interactor.status("123".to_owned()),
            interactor.wait_and_merge("123".to_owned(), 2, 3, "squash".to_owned()),
            interactor.trigger_review("123".to_owned()),
            interactor.poll_review("123".to_owned(), "2026-07-26T00:00:00Z".to_owned(), 0, 1),
            interactor.review_cycle(Some("track-id".to_owned()), false),
            interactor.review_cycle(Some("track-id".to_owned()), true),
        ];

        assert!(outputs.iter().all(|output| {
            output.stdout.as_deref() == Some("port stdout")
                && output.stderr.as_deref() == Some("port stderr")
                && output.exit_code == 29
        }));
        let commands = port.commands.lock().unwrap();
        assert!(matches!(
            commands.as_slice(),
            [
                PrCommand::Push { .. },
                PrCommand::Ensure { .. },
                PrCommand::Status(_),
                PrCommand::WaitAndMerge { .. },
                PrCommand::TriggerReview(_),
                PrCommand::PollReview { .. },
                PrCommand::ReviewCycle { mode: PrReviewCycleMode::Start, .. },
                PrCommand::ReviewCycle { mode: PrReviewCycleMode::Resume, .. },
            ]
        ));
    }
}
