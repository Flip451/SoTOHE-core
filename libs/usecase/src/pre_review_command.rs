//! Scope-aware, ordered pre-review command dispatch.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::review_v2::{MainScopeName, ScopeName};
use domain::{FreeText, TrackId};
use thiserror::Error;

use crate::operator_command::{
    CommandConfigLoadError, CommandConfigSchemaVersion, CommandConfigValidationError,
    CommandSequenceIndex, ConfiguredCommand, OutputCaptureLimitBytes,
};
use crate::program_runner::{
    ClassifiedProgramExecutionRecord, FailedProgramExecutionRecord, ProgramExecutionRecord,
    ProgramInvocation, ProgramRunnerError, ProgramRunnerPort, SuccessfulProgramExecutionRecord,
};
use crate::review_v2::{
    ReviewApprovalOutput, ReviewAuxError, ReviewCheckApprovedError, ReviewRunInput,
    ReviewRunLocalOutput, ReviewService, RunReviewError, RunReviewOutput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreReviewScopeCommandDeclaration {
    scope: ScopeName,
    commands: Vec<ConfiguredCommand>,
}
impl PreReviewScopeCommandDeclaration {
    #[must_use]
    pub fn new(scope: ScopeName, commands: Vec<ConfiguredCommand>) -> Self {
        Self { scope, commands }
    }
    #[must_use]
    pub fn scope(&self) -> &ScopeName {
        &self.scope
    }
    #[must_use]
    pub fn commands(&self) -> &[ConfiguredCommand] {
        &self.commands
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreReviewCommandConfig {
    scopes: Vec<PreReviewScopeCommandDeclaration>,
}
impl PreReviewCommandConfig {
    pub fn try_new(
        schema_version: CommandConfigSchemaVersion,
        scopes: Vec<PreReviewScopeCommandDeclaration>,
    ) -> Result<Self, PreReviewCommandConfigValidationError> {
        if schema_version.validate().is_err() {
            return Err(PreReviewCommandConfigValidationError::InvalidSchemaVersion {
                actual: schema_version,
            });
        }
        for (index, declaration) in scopes.iter().enumerate() {
            if scopes.iter().take(index).any(|prior| prior.scope == declaration.scope) {
                return Err(PreReviewCommandConfigValidationError::DuplicateScope(
                    declaration.scope.clone(),
                ));
            }
        }
        Ok(Self { scopes })
    }
    #[must_use]
    pub fn commands_for(&self, scope: &ScopeName) -> Option<&[ConfiguredCommand]> {
        self.scopes
            .iter()
            .find(|declaration| declaration.scope == *scope)
            .map(PreReviewScopeCommandDeclaration::commands)
    }
}

/// Validation failures that can arise while assembling pre-review configuration.
#[derive(Debug, Error)]
pub enum PreReviewCommandConfigValidationError {
    #[error("unsupported command configuration schema version: {actual:?}")]
    InvalidSchemaVersion { actual: CommandConfigSchemaVersion },
    #[error("duplicate review scope: {0}")]
    DuplicateScope(ScopeName),
}

impl From<PreReviewCommandConfigValidationError> for CommandConfigValidationError {
    fn from(error: PreReviewCommandConfigValidationError) -> Self {
        match error {
            PreReviewCommandConfigValidationError::InvalidSchemaVersion { actual } => {
                Self::InvalidSchemaVersion { actual }
            }
            PreReviewCommandConfigValidationError::DuplicateScope(scope) => {
                Self::DuplicateScope(scope)
            }
        }
    }
}

pub trait PreReviewCommandConfigLoaderPort: Send + Sync {
    fn load(
        &self,
        repository_root: &Path,
        track_id: &TrackId,
    ) -> Result<PreReviewCommandConfig, CommandConfigLoadError>;
}
pub trait CurrentReviewTrackResolverPort: Send + Sync {
    fn resolve(&self, repository_root: &Path) -> Result<TrackId, CurrentReviewTrackResolveError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewTrackSelector {
    Explicit(TrackId),
    CurrentBranch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewScopeSelector {
    Named(MainScopeName),
    Other,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreReviewCommandDispatchCommand {
    pub repository_root: PathBuf,
    pub track: ReviewTrackSelector,
    pub scope: ReviewScopeSelector,
}

#[derive(Debug, Error)]
pub enum CurrentReviewTrackResolveError {
    #[error("could not resolve the current review track: {message}")]
    ResolveFailed { message: FreeText },
}
#[derive(Debug, Error)]
pub enum PreReviewCommandDispatchError {
    #[error(transparent)]
    Config(#[from] CommandConfigLoadError),
    #[error("unknown review scope: {0}")]
    UnknownScope(ScopeName),
    #[error(transparent)]
    TrackResolution(#[from] CurrentReviewTrackResolveError),
    #[error(
        "explicit track '{explicit}' does not match the current branch track '{resolved}'; \
         the configured pre-review commands resolve their track from the current branch"
    )]
    TrackMismatch { explicit: TrackId, resolved: TrackId },
    #[error(transparent)]
    Runner(#[from] ProgramRunnerError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewCommandDispatchOutcome {
    ReadyForReview {
        records: Vec<SuccessfulProgramExecutionRecord>,
    },
    Blocked {
        completed: Vec<SuccessfulProgramExecutionRecord>,
        failed: FailedProgramExecutionRecord,
    },
}
pub trait PreReviewCommandDispatchService: Send + Sync {
    fn dispatch(
        &self,
        command: PreReviewCommandDispatchCommand,
    ) -> Result<PreReviewCommandDispatchOutcome, PreReviewCommandDispatchError>;
}

pub struct PreReviewCommandDispatchInteractor {
    config_loader: Arc<dyn PreReviewCommandConfigLoaderPort>,
    track_resolver: Arc<dyn CurrentReviewTrackResolverPort>,
    runner: Arc<dyn ProgramRunnerPort>,
}
impl PreReviewCommandDispatchInteractor {
    #[must_use]
    pub fn new(
        config_loader: Arc<dyn PreReviewCommandConfigLoaderPort>,
        track_resolver: Arc<dyn CurrentReviewTrackResolverPort>,
        runner: Arc<dyn ProgramRunnerPort>,
    ) -> Self {
        Self { config_loader, track_resolver, runner }
    }
}
impl PreReviewCommandDispatchService for PreReviewCommandDispatchInteractor {
    fn dispatch(
        &self,
        command: PreReviewCommandDispatchCommand,
    ) -> Result<PreReviewCommandDispatchOutcome, PreReviewCommandDispatchError> {
        let track_id = match command.track {
            ReviewTrackSelector::Explicit(track_id) => {
                // Pre-review commands resolve their track from the current
                // branch. Reject a different explicit review selection so the
                // gate and review cannot observe different track artifacts.
                let resolved = self.track_resolver.resolve(&command.repository_root)?;
                if resolved != track_id {
                    return Err(PreReviewCommandDispatchError::TrackMismatch {
                        explicit: track_id,
                        resolved,
                    });
                }
                track_id
            }
            ReviewTrackSelector::CurrentBranch => {
                self.track_resolver.resolve(&command.repository_root)?
            }
        };
        let scope = match command.scope {
            ReviewScopeSelector::Named(name) => ScopeName::Main(name),
            ReviewScopeSelector::Other => ScopeName::Other,
        };
        let config = self.config_loader.load(&command.repository_root, &track_id)?;
        let commands = config
            .commands_for(&scope)
            .ok_or_else(|| PreReviewCommandDispatchError::UnknownScope(scope.clone()))?;
        let mut completed = Vec::new();
        for (position, configured) in commands.iter().cloned().enumerate() {
            let invoked_argv = configured.argv().clone();
            let outcome = self.runner.run(ProgramInvocation {
                // A configured command owns its complete argument contract. In
                // particular, the signal commands in the default matrix are
                // intentionally argless, so appending review-local arguments
                // would fail before the gate can execute.
                argv: invoked_argv.clone(),
                repository_root: command.repository_root.clone(),
                timeout: configured.timeout(),
                stdout_limit: OutputCaptureLimitBytes::one_mebibyte(),
                stderr_limit: OutputCaptureLimitBytes::one_mebibyte(),
            })?;
            let record = ProgramExecutionRecord {
                sequence_index: CommandSequenceIndex::new(position),
                invoked_argv,
                command: configured,
                outcome,
            };
            match record.classify() {
                ClassifiedProgramExecutionRecord::Succeeded(record) => completed.push(record),
                ClassifiedProgramExecutionRecord::Failed(failed) => {
                    return Ok(PreReviewCommandDispatchOutcome::Blocked { completed, failed });
                }
            }
        }
        Ok(PreReviewCommandDispatchOutcome::ReadyForReview { records: completed })
    }
}

pub struct PreReviewCommandGatedReviewInteractor {
    inner: Arc<dyn ReviewService>,
    dispatcher: Arc<dyn PreReviewCommandDispatchService>,
}
impl PreReviewCommandGatedReviewInteractor {
    #[must_use]
    pub fn new(
        inner: Arc<dyn ReviewService>,
        dispatcher: Arc<dyn PreReviewCommandDispatchService>,
    ) -> Self {
        Self { inner, dispatcher }
    }
}

impl ReviewService for PreReviewCommandGatedReviewInteractor {
    fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
        self.inner.run_codex(input)
    }
    fn run_claude(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
        self.inner.run_claude(input)
    }
    fn run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> ReviewRunLocalOutput {
        // The configured pre-review commands resolve their own artifact
        // directory as `track/items` under their working directory, so gating
        // is only sound for the canonical layout; a non-canonical items
        // directory would silently evaluate different artifacts than the
        // review, and must fail closed instead.
        if !items_dir.ends_with(Path::new("track").join("items")) {
            return blocked_output(
                "pre-review gates require the canonical `track/items` directory; \
                 refusing to gate a non-canonical review items directory",
            );
        }
        // The default items directory is the relative `track/items`, whose grandparent is
        // the empty path; the empty path denotes the process working directory here.
        let repository_root = match items_dir.parent().and_then(Path::parent) {
            Some(root) if root.as_os_str().is_empty() => PathBuf::from("."),
            Some(root) => root.to_path_buf(),
            None => {
                return blocked_output("cannot infer repository root from review items directory");
            }
        };
        let track = match track_id.as_ref() {
            Some(value) => match TrackId::try_new(value.clone()) {
                Ok(value) => ReviewTrackSelector::Explicit(value),
                Err(error) => return blocked_output(&error.to_string()),
            },
            None => ReviewTrackSelector::CurrentBranch,
        };
        let scope = if group == "other" {
            ReviewScopeSelector::Other
        } else {
            match MainScopeName::new(group.clone()) {
                Ok(name) => ReviewScopeSelector::Named(name),
                Err(error) => return blocked_output(&error.to_string()),
            }
        };
        match self.dispatcher.dispatch(PreReviewCommandDispatchCommand {
            repository_root,
            track,
            scope,
        }) {
            Ok(PreReviewCommandDispatchOutcome::ReadyForReview { .. }) => self.inner.run_local(
                model,
                timeout_seconds,
                briefing_file,
                prompt,
                track_id,
                round_type,
                group,
                items_dir,
            ),
            Ok(PreReviewCommandDispatchOutcome::Blocked { failed, .. }) => {
                blocked_output(&format!("pre-review command failed: {failed:?}"))
            }
            Err(error) => blocked_output(&format!("pre-review command dispatch failed: {error}")),
        }
    }
    fn check_approved(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
        self.inner.check_approved(track_id, items_dir)
    }
    fn results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        scope: Option<String>,
        all: bool,
        limit: u32,
        round_type: String,
        no_hint: bool,
    ) -> Result<String, ReviewAuxError> {
        self.inner.results(track_id, items_dir, scope, all, limit, round_type, no_hint)
    }
    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, ReviewAuxError> {
        self.inner.classify(paths, track_id, items_dir)
    }
    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, ReviewAuxError> {
        self.inner.files(scope, track_id, items_dir)
    }
    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), ReviewAuxError> {
        self.inner.validate_scope(scope, track_id, items_dir)
    }
    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, ReviewAuxError> {
        self.inner.get_briefing(scope, track_id, items_dir)
    }
    fn persist_commit_hash(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> Result<String, crate::commit_hash_persistence::CommitHashPersistenceError> {
        self.inner.persist_commit_hash(track_id, workspace_root)
    }
}

fn blocked_output(message: &str) -> ReviewRunLocalOutput {
    ReviewRunLocalOutput { stdout: None, stderr: Some(message.to_owned()), exit_code: 1 }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use domain::TrackId;
    use domain::review_v2::{MainScopeName, ScopeName};

    use crate::operator_command::{CommandArgument, CommandConfigSchemaVersion, ConfiguredCommand};
    use crate::program_runner::{
        CapturedProgramOutput, ClassifiedProgramExecutionRecord, FailedProgramExecutionRecord,
        ProgramExecutionRecord, ProgramExitCode, ProgramInvocation, ProgramOutputStream,
        ProgramRunOutcome, ProgramRunnerError, ProgramRunnerPort,
    };
    use crate::review_v2::{
        ReviewRunInput, ReviewRunLocalOutput, ReviewService, RunReviewError, RunReviewOutput,
    };

    use super::{
        CurrentReviewTrackResolveError, CurrentReviewTrackResolverPort, PreReviewCommandConfig,
        PreReviewCommandConfigLoaderPort, PreReviewCommandConfigValidationError,
        PreReviewCommandDispatchCommand, PreReviewCommandDispatchInteractor,
        PreReviewCommandDispatchOutcome, PreReviewCommandDispatchService,
        PreReviewCommandGatedReviewInteractor, PreReviewScopeCommandDeclaration,
        ReviewScopeSelector, ReviewTrackSelector,
    };

    fn scope(value: &str) -> ScopeName {
        ScopeName::Main(MainScopeName::new(value).unwrap())
    }

    fn command(value: &str) -> ConfiguredCommand {
        command_argv(&[value])
    }

    fn command_argv(values: &[&str]) -> ConfiguredCommand {
        ConfiguredCommand::try_new(
            values.iter().map(|value| CommandArgument::try_new((*value).to_owned())).collect(),
            None,
        )
        .unwrap()
    }

    fn failed_record(outcome: ProgramRunOutcome) -> FailedProgramExecutionRecord {
        let command = command("fail");
        let record = ProgramExecutionRecord {
            sequence_index: crate::operator_command::CommandSequenceIndex::new(0),
            invoked_argv: command.argv().clone(),
            command,
            outcome,
        };
        match record.classify() {
            ClassifiedProgramExecutionRecord::Failed(record) => record,
            ClassifiedProgramExecutionRecord::Succeeded(_) => {
                panic!("test fixture must create a failed program record")
            }
        }
    }

    #[test]
    fn test_pre_review_command_config_duplicate_scope_is_rejected() {
        let declarations = vec![
            PreReviewScopeCommandDeclaration::new(scope("implementation"), vec![command("first")]),
            PreReviewScopeCommandDeclaration::new(scope("implementation"), vec![command("second")]),
        ];
        assert!(matches!(
            PreReviewCommandConfig::try_new(CommandConfigSchemaVersion::new(1), declarations),
            Err(PreReviewCommandConfigValidationError::DuplicateScope(_))
        ));
    }

    #[test]
    fn test_pre_review_command_config_reports_pre_review_specific_validation_errors() {
        assert!(matches!(
            PreReviewCommandConfig::try_new(CommandConfigSchemaVersion::new(2), Vec::new()),
            Err(PreReviewCommandConfigValidationError::InvalidSchemaVersion { .. })
        ));
    }

    #[test]
    fn test_pre_review_command_config_resolves_declared_scope_in_order() {
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![command("first"), command("second")],
            )],
        )
        .unwrap();
        let commands = config.commands_for(&scope("implementation")).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(
            commands
                .first()
                .expect("first command exists")
                .argv()
                .arguments()
                .first()
                .expect("first argv argument exists")
                .as_str(),
            "first"
        );
        assert_eq!(
            commands
                .get(1)
                .expect("second command exists")
                .argv()
                .arguments()
                .first()
                .expect("second argv argument exists")
                .as_str(),
            "second"
        );
    }

    struct StaticLoader(PreReviewCommandConfig);
    impl PreReviewCommandConfigLoaderPort for StaticLoader {
        fn load(
            &self,
            _repository_root: &Path,
            _track_id: &TrackId,
        ) -> Result<PreReviewCommandConfig, crate::operator_command::CommandConfigLoadError>
        {
            Ok(self.0.clone())
        }
    }

    struct FixedTrackResolver;
    impl CurrentReviewTrackResolverPort for FixedTrackResolver {
        fn resolve(
            &self,
            _repository_root: &Path,
        ) -> Result<TrackId, CurrentReviewTrackResolveError> {
            TrackId::try_new("test-track".to_owned()).map_err(|error| {
                CurrentReviewTrackResolveError::ResolveFailed {
                    message: domain::FreeText::new(error.to_string()),
                }
            })
        }
    }

    struct RecordingRunner(Mutex<Vec<Vec<String>>>);
    impl ProgramRunnerPort for RecordingRunner {
        fn run(
            &self,
            invocation: ProgramInvocation,
        ) -> Result<ProgramRunOutcome, ProgramRunnerError> {
            let arguments: Vec<String> = invocation
                .argv
                .arguments()
                .iter()
                .map(|argument| argument.as_str().to_owned())
                .collect();
            let value = arguments.first().expect("configured command has a non-empty argv");
            let exit_code = if value == "fail" { 1 } else { 0 };
            self.0
                .lock()
                .map_err(|_| ProgramRunnerError::WaitFailed {
                    message: domain::FreeText::new("recording lock poisoned"),
                })?
                .push(arguments);
            Ok(ProgramRunOutcome::Exited {
                exit_code: ProgramExitCode::new(exit_code),
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            })
        }
    }

    #[test]
    fn test_pre_review_command_dispatch_returns_first_failure_with_completed_successes() {
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![command("first"), command("fail"), command("later")],
            )],
        )
        .unwrap();
        let runner = Arc::new(RecordingRunner(Mutex::new(Vec::new())));
        let dispatcher = PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        );
        let outcome = dispatcher
            .dispatch(PreReviewCommandDispatchCommand {
                repository_root: PathBuf::from("/repo"),
                track: ReviewTrackSelector::CurrentBranch,
                scope: ReviewScopeSelector::Named(MainScopeName::new("implementation").unwrap()),
            })
            .unwrap();
        match outcome {
            PreReviewCommandDispatchOutcome::Blocked { completed, failed } => {
                assert_eq!(completed.len(), 1);
                assert!(matches!(
                    failed.as_ref().outcome,
                    ProgramRunOutcome::Exited { ref exit_code, .. } if exit_code.as_i32() != 0
                ));
                let failed_argv: Vec<&str> = failed
                    .as_ref()
                    .command
                    .argv()
                    .arguments()
                    .iter()
                    .map(CommandArgument::as_str)
                    .collect();
                assert_eq!(failed_argv, ["fail"]);
            }
            other => panic!("expected a failed pre-review command, got {other:?}"),
        }
        assert_eq!(*runner.0.lock().unwrap(), vec![vec!["first"], vec!["fail"],]);
    }

    #[test]
    fn test_pre_review_command_dispatch_returns_successful_records_in_declaration_order() {
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![
                    command_argv(&["bin/sotp", "signal", "calc-impl-catalog"]),
                    command_argv(&["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"]),
                    command_argv(&["bin/sotp", "task-contract", "coverage"]),
                    command_argv(&["bin/sotp", "task-contract", "check"]),
                ],
            )],
        )
        .unwrap();
        let runner = Arc::new(RecordingRunner(Mutex::new(Vec::new())));
        let dispatcher = PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        );

        let outcome = dispatcher
            .dispatch(PreReviewCommandDispatchCommand {
                repository_root: PathBuf::from("/repo"),
                track: ReviewTrackSelector::CurrentBranch,
                scope: ReviewScopeSelector::Named(MainScopeName::new("implementation").unwrap()),
            })
            .unwrap();

        assert!(
            matches!(outcome, PreReviewCommandDispatchOutcome::ReadyForReview { records } if records.len() == 4)
        );
        assert_eq!(
            *runner.0.lock().unwrap(),
            vec![
                vec!["bin/sotp", "signal", "calc-impl-catalog"],
                vec!["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"],
                vec!["bin/sotp", "task-contract", "coverage"],
                vec!["bin/sotp", "task-contract", "check"],
            ]
        );
    }

    struct OutcomeRecordingRunner {
        outcomes: Mutex<std::collections::VecDeque<ProgramRunOutcome>>,
        invocations: Mutex<Vec<Vec<String>>>,
    }

    impl ProgramRunnerPort for OutcomeRecordingRunner {
        fn run(
            &self,
            invocation: ProgramInvocation,
        ) -> Result<ProgramRunOutcome, ProgramRunnerError> {
            let arguments = invocation
                .argv
                .arguments()
                .iter()
                .map(|argument| argument.as_str().to_owned())
                .collect();
            self.invocations.lock().expect("recording lock is healthy").push(arguments);
            self.outcomes.lock().expect("outcome lock is healthy").pop_front().ok_or_else(|| {
                ProgramRunnerError::WaitFailed {
                    message: domain::FreeText::new("missing program outcome fixture"),
                }
            })
        }
    }

    fn assert_dispatch_stops_after_bounded_failure(
        outcome: ProgramRunOutcome,
        expected_failure: fn(&FailedProgramExecutionRecord) -> bool,
    ) {
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![command("bounded"), command("later")],
            )],
        )
        .unwrap();
        let runner = Arc::new(OutcomeRecordingRunner {
            outcomes: Mutex::new(std::collections::VecDeque::from([outcome])),
            invocations: Mutex::new(Vec::new()),
        });
        let dispatcher = PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        );

        let outcome = dispatcher
            .dispatch(PreReviewCommandDispatchCommand {
                repository_root: PathBuf::from("/repo"),
                track: ReviewTrackSelector::CurrentBranch,
                scope: ReviewScopeSelector::Named(MainScopeName::new("implementation").unwrap()),
            })
            .unwrap();

        match outcome {
            PreReviewCommandDispatchOutcome::Blocked { completed, failed } => {
                assert!(completed.is_empty());
                assert!(expected_failure(&failed), "unexpected failure record: {failed:?}");
            }
            other => panic!("expected bounded command to block review, got {other:?}"),
        }
        assert_eq!(*runner.invocations.lock().unwrap(), vec![vec!["bounded"]]);
    }

    #[test]
    fn test_pre_review_command_dispatch_timeout_blocks_and_stops_before_later_command() {
        assert_dispatch_stops_after_bounded_failure(
            ProgramRunOutcome::TimedOut {
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            },
            |record| matches!(record.as_ref().outcome, ProgramRunOutcome::TimedOut { .. }),
        );
    }

    #[test]
    fn test_pre_review_command_dispatch_output_limit_blocks_and_stops_before_later_command() {
        assert_dispatch_stops_after_bounded_failure(
            ProgramRunOutcome::OutputLimitExceeded {
                stream: ProgramOutputStream::Stderr,
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            },
            |record| {
                matches!(record.as_ref().outcome, ProgramRunOutcome::OutputLimitExceeded { .. })
            },
        );
    }

    #[test]
    fn test_pre_review_command_dispatch_rejects_explicit_track_not_matching_branch() {
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![command("first")],
            )],
        )
        .unwrap();
        let runner = Arc::new(RecordingRunner(Mutex::new(Vec::new())));
        let dispatcher = PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        );

        let result = dispatcher.dispatch(PreReviewCommandDispatchCommand {
            repository_root: PathBuf::from("/repo"),
            track: ReviewTrackSelector::Explicit(
                TrackId::try_new("another-track".to_owned()).unwrap(),
            ),
            scope: ReviewScopeSelector::Named(MainScopeName::new("implementation").unwrap()),
        });

        assert!(matches!(result, Err(super::PreReviewCommandDispatchError::TrackMismatch { .. })));
        assert!(runner.0.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pre_review_command_dispatch_accepts_explicit_track_matching_branch() {
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![command("first")],
            )],
        )
        .unwrap();
        let runner = Arc::new(RecordingRunner(Mutex::new(Vec::new())));
        let dispatcher = PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        );
        let outcome = dispatcher
            .dispatch(PreReviewCommandDispatchCommand {
                repository_root: PathBuf::from("/repo"),
                track: ReviewTrackSelector::Explicit(
                    TrackId::try_new("test-track".to_owned()).unwrap(),
                ),
                scope: ReviewScopeSelector::Named(MainScopeName::new("implementation").unwrap()),
            })
            .unwrap();
        assert!(matches!(outcome, PreReviewCommandDispatchOutcome::ReadyForReview { .. }));
        assert_eq!(*runner.0.lock().unwrap(), vec![vec!["first"]]);
    }

    struct BlockingDispatcher;
    impl PreReviewCommandDispatchService for BlockingDispatcher {
        fn dispatch(
            &self,
            _command: PreReviewCommandDispatchCommand,
        ) -> Result<PreReviewCommandDispatchOutcome, super::PreReviewCommandDispatchError> {
            Ok(PreReviewCommandDispatchOutcome::Blocked {
                completed: Vec::new(),
                failed: failed_record(ProgramRunOutcome::Exited {
                    exit_code: ProgramExitCode::new(1),
                    output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
                }),
            })
        }
    }

    struct CountingReview(AtomicUsize);
    impl ReviewService for CountingReview {
        fn run_codex(&self, _input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
            panic!("not used")
        }
        fn run_claude(&self, _input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
            panic!("not used")
        }
        fn run_local(
            &self,
            _model: Option<String>,
            _timeout_seconds: u64,
            _briefing_file: Option<PathBuf>,
            _prompt: Option<String>,
            _track_id: Option<String>,
            _round_type: String,
            _group: String,
            _items_dir: PathBuf,
        ) -> ReviewRunLocalOutput {
            self.0.fetch_add(1, Ordering::SeqCst);
            ReviewRunLocalOutput { stdout: Some("reviewed".to_owned()), stderr: None, exit_code: 0 }
        }
        fn check_approved(
            &self,
            _track_id: String,
            _items_dir: PathBuf,
        ) -> Result<
            crate::review_v2::ReviewApprovalOutput,
            crate::review_v2::ReviewCheckApprovedError,
        > {
            panic!("not used")
        }
        fn results(
            &self,
            _track_id: Option<String>,
            _items_dir: PathBuf,
            _scope: Option<String>,
            _all: bool,
            _limit: u32,
            _round_type: String,
            _no_hint: bool,
        ) -> Result<String, crate::review_v2::ReviewAuxError> {
            panic!("not used")
        }
        fn classify(
            &self,
            _paths: Vec<String>,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Vec<(String, String)>, crate::review_v2::ReviewAuxError> {
            panic!("not used")
        }
        fn files(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Vec<String>, crate::review_v2::ReviewAuxError> {
            panic!("not used")
        }
        fn validate_scope(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<(), crate::review_v2::ReviewAuxError> {
            panic!("not used")
        }
        fn get_briefing(
            &self,
            _scope: String,
            _track_id: Option<String>,
            _items_dir: PathBuf,
        ) -> Result<Option<String>, crate::review_v2::ReviewAuxError> {
            panic!("not used")
        }
        fn persist_commit_hash(
            &self,
            _track_id: String,
            _workspace_root: PathBuf,
        ) -> Result<String, crate::commit_hash_persistence::CommitHashPersistenceError> {
            panic!("not used")
        }
    }

    struct CapturingDispatcher(Mutex<Vec<PreReviewCommandDispatchCommand>>);
    impl PreReviewCommandDispatchService for CapturingDispatcher {
        fn dispatch(
            &self,
            command: PreReviewCommandDispatchCommand,
        ) -> Result<PreReviewCommandDispatchOutcome, super::PreReviewCommandDispatchError> {
            self.0.lock().expect("capture lock is healthy").push(command);
            Ok(PreReviewCommandDispatchOutcome::ReadyForReview { records: Vec::new() })
        }
    }

    #[test]
    fn test_pre_review_gated_review_maps_relative_items_dir_to_working_directory_root() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let dispatcher = Arc::new(CapturingDispatcher(Mutex::new(Vec::new())));
        let gated = PreReviewCommandGatedReviewInteractor::new(review.clone(), dispatcher.clone());
        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "implementation".to_owned(),
            PathBuf::from("track/items"),
        );
        assert_eq!(output.exit_code, 0);
        assert_eq!(review.0.load(Ordering::SeqCst), 1);
        let captured = dispatcher.0.lock().unwrap();
        assert_eq!(
            captured.first().expect("dispatch command captured").repository_root,
            PathBuf::from(".")
        );
    }

    #[test]
    fn test_pre_review_gated_review_rejects_non_canonical_items_dir() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let dispatcher = Arc::new(CapturingDispatcher(Mutex::new(Vec::new())));
        let gated = PreReviewCommandGatedReviewInteractor::new(review.clone(), dispatcher.clone());
        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "implementation".to_owned(),
            PathBuf::from("custom/artifacts"),
        );
        assert_eq!(output.exit_code, 1);
        assert!(
            output.stderr.as_deref().unwrap_or("").contains("canonical `track/items` directory")
        );
        assert_eq!(review.0.load(Ordering::SeqCst), 0);
        assert!(dispatcher.0.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pre_review_gated_review_blocks_inner_review_when_pre_entry_fails() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let gated = PreReviewCommandGatedReviewInteractor::new(
            review.clone(),
            Arc::new(BlockingDispatcher),
        );
        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "implementation".to_owned(),
            PathBuf::from("track/items"),
        );
        assert_eq!(output.exit_code, 1);
        assert!(output.stderr.as_deref().unwrap_or("").contains("fail"));
        assert!(output.stderr.as_deref().unwrap_or("").contains("Exited"));
        assert_eq!(review.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_pre_review_gated_review_runs_implementation_gates_in_order_and_stops_on_failure() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let runner = Arc::new(OutcomeRecordingRunner {
            outcomes: Mutex::new(std::collections::VecDeque::from([
                ProgramRunOutcome::Exited {
                    exit_code: ProgramExitCode::new(0),
                    output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
                },
                ProgramRunOutcome::Exited {
                    exit_code: ProgramExitCode::new(1),
                    output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
                },
            ])),
            invocations: Mutex::new(Vec::new()),
        });
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![
                    command_argv(&["bin/sotp", "signal", "calc-impl-catalog"]),
                    command_argv(&["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"]),
                    command_argv(&["bin/sotp", "task-contract", "coverage"]),
                    command_argv(&["bin/sotp", "task-contract", "check"]),
                ],
            )],
        )
        .unwrap();
        let dispatcher = Arc::new(PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        ));
        let gated = PreReviewCommandGatedReviewInteractor::new(review.clone(), dispatcher);

        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "implementation".to_owned(),
            PathBuf::from("track/items"),
        );

        assert_eq!(output.exit_code, 1);
        assert!(output.stderr.as_deref().unwrap_or("").contains("check-impl-catalog"));
        assert_eq!(review.0.load(Ordering::SeqCst), 0);
        assert_eq!(
            *runner.invocations.lock().unwrap(),
            vec![
                vec!["bin/sotp", "signal", "calc-impl-catalog"],
                vec!["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"],
            ]
        );
    }

    /// When every implementation gate succeeds, the gated review runs the full
    /// four-command sequence to completion — `calc-impl-catalog`,
    /// `check-impl-catalog`, `task-contract coverage`, then
    /// `task-contract check`, in declaration order — and only then starts the
    /// reviewer.
    #[test]
    fn test_pre_review_gated_review_runs_all_implementation_gates_then_reviews_on_success() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let success = || ProgramRunOutcome::Exited {
            exit_code: ProgramExitCode::new(0),
            output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
        };
        let runner = Arc::new(OutcomeRecordingRunner {
            outcomes: Mutex::new(std::collections::VecDeque::from([
                success(),
                success(),
                success(),
                success(),
            ])),
            invocations: Mutex::new(Vec::new()),
        });
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(
                scope("implementation"),
                vec![
                    command_argv(&["bin/sotp", "signal", "calc-impl-catalog"]),
                    command_argv(&["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"]),
                    command_argv(&["bin/sotp", "task-contract", "coverage"]),
                    command_argv(&["bin/sotp", "task-contract", "check"]),
                ],
            )],
        )
        .unwrap();
        let dispatcher = Arc::new(PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        ));
        let gated = PreReviewCommandGatedReviewInteractor::new(review.clone(), dispatcher);

        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "implementation".to_owned(),
            PathBuf::from("track/items"),
        );

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout.as_deref(), Some("reviewed"));
        assert_eq!(review.0.load(Ordering::SeqCst), 1);
        assert_eq!(
            *runner.invocations.lock().unwrap(),
            vec![
                vec!["bin/sotp", "signal", "calc-impl-catalog"],
                vec!["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"],
                vec!["bin/sotp", "task-contract", "coverage"],
                vec!["bin/sotp", "task-contract", "check"],
            ]
        );
    }

    struct FailingLoader;
    impl PreReviewCommandConfigLoaderPort for FailingLoader {
        fn load(
            &self,
            _repository_root: &Path,
            _track_id: &TrackId,
        ) -> Result<PreReviewCommandConfig, crate::operator_command::CommandConfigLoadError>
        {
            Err(crate::operator_command::CommandConfigLoadError::DecodeFailed {
                message: domain::FreeText::new("configuration is not valid JSON"),
            })
        }
    }

    /// An invalid pre-review-gates configuration fails the gated review closed:
    /// the dispatch reports the configuration error, no configured command
    /// runs, and the reviewer is never started.
    #[test]
    fn test_pre_review_gated_review_rejects_invalid_configuration_without_reviewer_start() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let runner = Arc::new(RecordingRunner(Mutex::new(Vec::new())));
        let dispatcher = Arc::new(PreReviewCommandDispatchInteractor::new(
            Arc::new(FailingLoader),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        ));
        let gated = PreReviewCommandGatedReviewInteractor::new(review.clone(), dispatcher);

        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "implementation".to_owned(),
            PathBuf::from("track/items"),
        );

        assert_eq!(output.exit_code, 1);
        assert!(
            output.stderr.as_deref().unwrap_or("").contains("pre-review command dispatch failed")
        );
        assert_eq!(review.0.load(Ordering::SeqCst), 0);
        assert!(runner.0.lock().unwrap().is_empty());
    }

    /// A planning/SoT scope (`spec`) whose matrix row declares an empty command
    /// vector starts the reviewer even though NO downstream implementation
    /// artifact exists anywhere in this environment: the collaborators supply
    /// no task-contract document, no impl-catalog signal document, and no
    /// liveness evaluation — the recording runner proves that zero configured
    /// commands executed, so no downstream liveness or task-contract gate could
    /// have been consulted before the reviewer started.
    #[test]
    fn test_pre_review_gated_review_allows_planning_scope_without_downstream_liveness_artifacts() {
        let review = Arc::new(CountingReview(AtomicUsize::new(0)));
        let runner = Arc::new(RecordingRunner(Mutex::new(Vec::new())));
        let config = PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PreReviewScopeCommandDeclaration::new(scope("spec"), Vec::new())],
        )
        .unwrap();
        let dispatcher = Arc::new(PreReviewCommandDispatchInteractor::new(
            Arc::new(StaticLoader(config)),
            Arc::new(FixedTrackResolver),
            runner.clone(),
        ));
        let gated = PreReviewCommandGatedReviewInteractor::new(review.clone(), dispatcher);

        let output = gated.run_local(
            None,
            60,
            None,
            None,
            Some("test-track".to_owned()),
            "fast".to_owned(),
            "spec".to_owned(),
            PathBuf::from("track/items"),
        );

        // The inner reviewer ran exactly once and produced its output, with no
        // configured command executed first — the planning scope required no
        // downstream implementation liveness and no task-contract artifact.
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout.as_deref(), Some("reviewed"));
        assert_eq!(review.0.load(Ordering::SeqCst), 1);
        assert!(runner.0.lock().unwrap().is_empty());
    }
}
