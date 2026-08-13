//! Primary adapter for phase-command transport input and outcome rendering.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use usecase::operator_command::CommandDeclarationId;
use usecase::phase_command::{
    PhaseCommandEnterOutcome, PhaseCommandExplanation, PhaseCommandService, PhaseEnterCommand,
    PhaseExplainQuery, PhaseValidateCommand,
};
use usecase::program_runner::{
    CapturedProgramOutput, ProgramExecutionRecord, ProgramOutputStream, ProgramRunOutcome,
};

use crate::capability::ProviderNameArg;
use crate::render::CommandOutcome;

/// CLI transport wrapper for a usecase-owned phase declaration identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseIdArg {
    value: CommandDeclarationId,
}

impl PhaseIdArg {
    /// Creates a transport wrapper from an already validated declaration id.
    #[must_use]
    pub fn new(value: CommandDeclarationId) -> Self {
        Self { value }
    }

    /// Returns the usecase-owned declaration id.
    #[must_use]
    pub fn as_declaration_id(&self) -> &CommandDeclarationId {
        &self.value
    }
}

impl FromStr for PhaseIdArg {
    type Err = usecase::operator_command::CommandConfigValidationError;

    /// Parses a phase id using the usecase-owned validation boundary.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(CommandDeclarationId::try_new(value.to_owned())?))
    }
}

/// Transport input for the phase command family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseCommandInput {
    /// Validate the phase-command configuration at the supplied repository root.
    Validate { repository_root: PathBuf },
    /// Explain the commands configured for one phase declaration.
    Explain { repository_root: PathBuf, phase_id: PhaseIdArg },
    /// Execute the configured pre-entry commands and canonical writer for one phase.
    Enter { repository_root: PathBuf, phase_id: PhaseIdArg, host: Option<ProviderNameArg> },
}

/// Primary adapter that invokes the injected phase-command service and renders its outcomes.
pub struct PhaseCommandDriver {
    service: Arc<dyn PhaseCommandService>,
}

impl std::fmt::Debug for PhaseCommandDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PhaseCommandDriver").finish_non_exhaustive()
    }
}

impl PhaseCommandDriver {
    /// Creates a phase-command driver over an injected usecase service.
    #[must_use]
    pub fn new(service: Arc<dyn PhaseCommandService>) -> Self {
        Self { service }
    }

    /// Invokes the selected usecase operation and renders it at the CLI boundary.
    #[must_use]
    pub fn handle(&self, input: PhaseCommandInput) -> CommandOutcome {
        match input {
            PhaseCommandInput::Validate { repository_root } => self
                .service
                .validate(PhaseValidateCommand { repository_root })
                .map(|()| {
                    CommandOutcome::success(Some("phase command configuration is valid".to_owned()))
                })
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
            PhaseCommandInput::Explain { repository_root, phase_id } => self
                .service
                .explain(PhaseExplainQuery { repository_root, phase_id: phase_id.value })
                .map(|explanation| CommandOutcome::success(Some(render_explanation(&explanation))))
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
            PhaseCommandInput::Enter { repository_root, phase_id, host } => self
                .service
                .enter(PhaseEnterCommand {
                    repository_root,
                    phase_id: phase_id.value,
                    host: host.map(|host| host.0),
                })
                .map(render_enter_outcome)
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
        }
    }
}

fn render_explanation(explanation: &PhaseCommandExplanation) -> String {
    let mut lines = vec![format!(
        "phase {} (output limit: {} bytes)",
        explanation.phase_id.as_str(),
        explanation.output_limit.as_usize()
    )];
    for (position, command) in explanation.pre_entry_commands.iter().enumerate() {
        lines.push(format!(
            "pre-entry {position}: {} (timeout: {}s)",
            render_argv(command.argv()),
            command.timeout().as_secs()
        ));
    }
    lines.push(format!(
        "writer: {} (timeout: {}s)",
        render_argv(explanation.writer.argv()),
        explanation.writer.timeout().as_secs()
    ));
    lines.join("\n")
}

fn render_enter_outcome(outcome: PhaseCommandEnterOutcome) -> CommandOutcome {
    match outcome {
        PhaseCommandEnterOutcome::Completed { pre_entry_records, writer_record } => {
            let records = pre_entry_records
                .iter()
                .map(AsRef::as_ref)
                .chain(std::iter::once(writer_record.as_ref()));
            let (stdout, stderr) = render_records(records);
            CommandOutcome { stdout, stderr, exit_code: 0 }
        }
        PhaseCommandEnterOutcome::Blocked { completed, failed } => {
            let (stdout, stderr) = render_records(completed.iter().map(AsRef::as_ref));
            let (failed_stdout, failed_stderr) = render_record(failed.as_ref());
            let mut stdout_parts = stdout.into_iter().collect::<Vec<_>>();
            if let Some(value) = failed_stdout {
                stdout_parts.push(value);
            }
            let mut stderr_parts = stderr.into_iter().collect::<Vec<_>>();
            if let Some(value) = failed_stderr {
                stderr_parts.push(value);
            }
            stderr_parts.push(format!(
                "phase command blocked at sequence {}: {}; outcome: {}",
                failed.as_ref().sequence_index.as_usize(),
                render_argv(&failed.as_ref().invoked_argv),
                render_outcome_name(&failed.as_ref().outcome),
            ));
            CommandOutcome {
                stdout: join_nonempty(stdout_parts),
                stderr: join_nonempty(stderr_parts),
                exit_code: exit_code_for(&failed.as_ref().outcome),
            }
        }
    }
}

fn render_records<'a>(
    records: impl IntoIterator<Item = &'a ProgramExecutionRecord>,
) -> (Option<String>, Option<String>) {
    let mut stdout_parts = Vec::new();
    let mut stderr_parts = Vec::new();
    for record in records {
        let (stdout, stderr) = render_record(record);
        if let Some(value) = stdout {
            stdout_parts.push(value);
        }
        if let Some(value) = stderr {
            stderr_parts.push(value);
        }
        stdout_parts.push(render_completed_record_audit(record));
    }
    (join_nonempty(stdout_parts), join_nonempty(stderr_parts))
}

fn render_completed_record_audit(record: &ProgramExecutionRecord) -> String {
    format!(
        "phase command sequence {}: {}; outcome: {}",
        record.sequence_index.as_usize(),
        render_argv(&record.invoked_argv),
        render_outcome_name(&record.outcome),
    )
}

fn render_record(record: &ProgramExecutionRecord) -> (Option<String>, Option<String>) {
    match &record.outcome {
        ProgramRunOutcome::Exited { output, .. }
        | ProgramRunOutcome::TimedOut { output }
        | ProgramRunOutcome::OutputLimitExceeded { output, .. } => render_output(output),
    }
}

fn render_output(output: &CapturedProgramOutput) -> (Option<String>, Option<String>) {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    ((!stdout.is_empty()).then_some(stdout), (!stderr.is_empty()).then_some(stderr))
}

fn exit_code_for(outcome: &ProgramRunOutcome) -> u8 {
    match outcome {
        ProgramRunOutcome::Exited { exit_code, .. } => {
            u8::try_from(exit_code.as_i32()).ok().filter(|code| *code != 0).unwrap_or(1)
        }
        ProgramRunOutcome::TimedOut { .. } | ProgramRunOutcome::OutputLimitExceeded { .. } => 1,
    }
}

fn render_outcome_name(outcome: &ProgramRunOutcome) -> String {
    match outcome {
        ProgramRunOutcome::Exited { exit_code, .. } => {
            format!("exited with {}", exit_code.as_i32())
        }
        ProgramRunOutcome::TimedOut { .. } => "timed out".to_owned(),
        ProgramRunOutcome::OutputLimitExceeded { stream, .. } => match stream {
            ProgramOutputStream::Stdout => "output limit exceeded on stdout".to_owned(),
            ProgramOutputStream::Stderr => "output limit exceeded on stderr".to_owned(),
        },
    }
}

fn render_argv(argv: &usecase::operator_command::CommandArgv) -> String {
    let arguments = argv
        .arguments()
        .iter()
        .map(usecase::operator_command::CommandArgument::as_str)
        .collect::<Vec<_>>()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    match serde_json::to_string(&arguments) {
        Ok(rendered) => rendered,
        Err(error) => format!("[argv rendering failed: {error}]"),
    }
}

fn join_nonempty(parts: Vec<String>) -> Option<String> {
    let joined = parts.join("\n");
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    use domain::FreeText;
    use usecase::operator_command::{
        CommandArgument, CommandConfigLoadError, CommandConfigValidationError,
        CommandDeclarationId, CommandSequenceIndex, ConfiguredCommand, OutputCaptureLimitBytes,
        UnvalidatedTimeoutSeconds,
    };
    use usecase::phase_command::{
        PhaseCommandEnterError, PhaseCommandEnterOutcome, PhaseCommandExplanation,
        PhaseCommandService, PhaseEnterCommand, PhaseExplainQuery, PhaseValidateCommand,
    };
    use usecase::program_runner::{
        CapturedProgramOutput, ClassifiedProgramExecutionRecord, FailedProgramExecutionRecord,
        ProgramExecutionRecord, ProgramExitCode, ProgramOutputStream, ProgramRunOutcome,
        SuccessfulProgramExecutionRecord,
    };

    use crate::capability::ProviderNameArg;

    use super::{
        PhaseCommandDriver, PhaseCommandInput, PhaseIdArg, render_argv, render_enter_outcome,
    };

    #[derive(Default)]
    struct StubPhaseService {
        calls: Mutex<Vec<&'static str>>,
        hosts: Mutex<Vec<Option<String>>>,
    }

    impl PhaseCommandService for StubPhaseService {
        fn validate(&self, _command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError> {
            self.calls.lock().expect("test mutex").push("validate");
            Ok(())
        }

        fn explain(
            &self,
            query: PhaseExplainQuery,
        ) -> Result<PhaseCommandExplanation, usecase::phase_command::PhaseCommandExplainError>
        {
            self.calls.lock().expect("test mutex").push("explain");
            Ok(PhaseCommandExplanation {
                phase_id: query.phase_id,
                pre_entry_commands: vec![command_with_timeout(&["bin/sotp", "check"], 12)],
                writer: command_with_timeout(&["bin/sotp", "capability", "exec"], 34),
                output_limit: OutputCaptureLimitBytes::one_mebibyte(),
            })
        }

        fn enter(
            &self,
            command: PhaseEnterCommand,
        ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
            self.calls.lock().expect("test mutex").push("enter");
            self.hosts
                .lock()
                .expect("test mutex")
                .push(command.host.as_ref().map(|host| host.as_str().to_owned()));
            Ok(PhaseCommandEnterOutcome::Completed {
                pre_entry_records: vec![successful_record(0, &["pre"], 0, b"before", b"")],
                writer_record: successful_record(1, &["writer"], 0, b"writer output", b""),
            })
        }
    }

    struct FailingPhaseService;

    impl PhaseCommandService for FailingPhaseService {
        fn validate(&self, _command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError> {
            Err(config_error())
        }

        fn explain(
            &self,
            _query: PhaseExplainQuery,
        ) -> Result<PhaseCommandExplanation, usecase::phase_command::PhaseCommandExplainError>
        {
            Err(usecase::phase_command::PhaseCommandExplainError::Config(config_error()))
        }

        fn enter(
            &self,
            _command: PhaseEnterCommand,
        ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
            Err(PhaseCommandEnterError::Config(config_error()))
        }
    }

    struct TimeoutValidationPhaseService;

    impl PhaseCommandService for TimeoutValidationPhaseService {
        fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError> {
            let seconds = if command.repository_root.ends_with("zero-timeout") { 0 } else { 3_601 };
            Err(CommandConfigValidationError::TimeoutOutOfRange {
                seconds: UnvalidatedTimeoutSeconds::new(seconds),
            }
            .into())
        }

        fn explain(
            &self,
            _query: PhaseExplainQuery,
        ) -> Result<PhaseCommandExplanation, usecase::phase_command::PhaseCommandExplainError>
        {
            Err(usecase::phase_command::PhaseCommandExplainError::Config(config_error()))
        }

        fn enter(
            &self,
            _command: PhaseEnterCommand,
        ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
            Err(PhaseCommandEnterError::Config(config_error()))
        }
    }

    fn config_error() -> CommandConfigLoadError {
        CommandConfigLoadError::ReadFailed {
            message: FreeText::new("fixture configuration failure"),
        }
    }

    fn command_with_timeout(arguments: &[&str], seconds: u64) -> ConfiguredCommand {
        ConfiguredCommand::try_new(
            arguments
                .iter()
                .map(|argument| CommandArgument::try_new((*argument).to_owned()))
                .collect(),
            Some(UnvalidatedTimeoutSeconds::new(seconds)),
        )
        .expect("test command")
    }

    fn command_with_default_timeout(arguments: &[&str]) -> ConfiguredCommand {
        ConfiguredCommand::try_new(
            arguments
                .iter()
                .map(|argument| CommandArgument::try_new((*argument).to_owned()))
                .collect(),
            None,
        )
        .expect("test command")
    }

    fn phase_id() -> PhaseIdArg {
        PhaseIdArg::new(CommandDeclarationId::try_new("phase-1".to_owned()).expect("test phase id"))
    }

    fn raw_record(
        sequence: usize,
        arguments: &[&str],
        exit_code: i32,
        stdout: &[u8],
        stderr: &[u8],
    ) -> ProgramExecutionRecord {
        let command = command_with_timeout(arguments, 1);
        ProgramExecutionRecord {
            sequence_index: CommandSequenceIndex::new(sequence),
            invoked_argv: command.argv().clone(),
            command,
            outcome: ProgramRunOutcome::Exited {
                exit_code: ProgramExitCode::new(exit_code),
                output: CapturedProgramOutput { stdout: stdout.to_vec(), stderr: stderr.to_vec() },
            },
        }
    }

    fn successful_record(
        sequence: usize,
        arguments: &[&str],
        exit_code: i32,
        stdout: &[u8],
        stderr: &[u8],
    ) -> SuccessfulProgramExecutionRecord {
        match raw_record(sequence, arguments, exit_code, stdout, stderr).classify() {
            ClassifiedProgramExecutionRecord::Succeeded(record) => record,
            ClassifiedProgramExecutionRecord::Failed(record) => {
                panic!("expected success, got {record:?}")
            }
        }
    }

    fn failed_record(outcome: ProgramRunOutcome) -> FailedProgramExecutionRecord {
        let command = command_with_timeout(&["bin/sotp", "capability", "exec"], 1);
        let invoked_argv = match command.argv().with_appended_arguments([
            CommandArgument::try_new("--host".to_owned()),
            CommandArgument::try_new("codex".to_owned()),
        ]) {
            Ok(argv) => argv,
            Err(error) => panic!("capability exec host arguments must remain valid: {error:?}"),
        };
        let record = ProgramExecutionRecord {
            sequence_index: CommandSequenceIndex::new(1),
            invoked_argv,
            command,
            outcome,
        };
        match record.classify() {
            ClassifiedProgramExecutionRecord::Succeeded(record) => {
                panic!("expected failure, got {record:?}")
            }
            ClassifiedProgramExecutionRecord::Failed(record) => record,
        }
    }

    #[test]
    fn test_phase_driver_validate_success_renders_confirmation() {
        let service = Arc::new(StubPhaseService::default());
        let driver = PhaseCommandDriver::new(Arc::clone(&service) as Arc<dyn PhaseCommandService>);

        let outcome =
            driver.handle(PhaseCommandInput::Validate { repository_root: "/repo".into() });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("phase command configuration is valid"));
        assert_eq!(service.calls.lock().expect("test mutex").as_slice(), ["validate"]);
    }

    #[test]
    fn test_phase_driver_validate_failure_renders_service_diagnostic() {
        let outcome = PhaseCommandDriver::new(Arc::new(FailingPhaseService))
            .handle(PhaseCommandInput::Validate { repository_root: "/repo".into() });

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stderr.expect("failure diagnostic").contains("fixture configuration failure")
        );
    }

    #[test]
    fn test_phase_driver_explain_expands_full_argv_and_timeouts() {
        let outcome = PhaseCommandDriver::new(Arc::new(StubPhaseService::default())).handle(
            PhaseCommandInput::Explain { repository_root: "/repo".into(), phase_id: phase_id() },
        );

        assert_eq!(outcome.exit_code, 0);
        let rendered = outcome.stdout.expect("explanation");
        assert!(rendered.contains("pre-entry 0: [\"bin/sotp\",\"check\"] (timeout: 12s)"));
        assert!(rendered.contains("writer: [\"bin/sotp\",\"capability\",\"exec\"] (timeout: 34s)"));
        assert!(rendered.contains("output limit: 1048576 bytes"));
    }

    #[test]
    fn test_phase_driver_explain_renders_default_timeout_when_unspecified() {
        struct DefaultTimeoutPhaseService;

        impl PhaseCommandService for DefaultTimeoutPhaseService {
            fn validate(
                &self,
                _command: PhaseValidateCommand,
            ) -> Result<(), CommandConfigLoadError> {
                Ok(())
            }

            fn explain(
                &self,
                query: PhaseExplainQuery,
            ) -> Result<PhaseCommandExplanation, usecase::phase_command::PhaseCommandExplainError>
            {
                Ok(PhaseCommandExplanation {
                    phase_id: query.phase_id,
                    pre_entry_commands: vec![command_with_default_timeout(&["pre"])],
                    writer: command_with_default_timeout(&["writer"]),
                    output_limit: OutputCaptureLimitBytes::one_mebibyte(),
                })
            }

            fn enter(
                &self,
                _command: PhaseEnterCommand,
            ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
                Err(PhaseCommandEnterError::Config(config_error()))
            }
        }

        let outcome = PhaseCommandDriver::new(Arc::new(DefaultTimeoutPhaseService)).handle(
            PhaseCommandInput::Explain { repository_root: "/repo".into(), phase_id: phase_id() },
        );

        assert_eq!(outcome.exit_code, 0);
        let rendered = outcome.stdout.expect("explanation");
        assert!(rendered.contains("pre-entry 0: [\"pre\"] (timeout: 3600s)"));
        assert!(rendered.contains("writer: [\"writer\"] (timeout: 3600s)"));
    }

    #[test]
    fn test_phase_driver_validate_renders_timeout_range_failures() {
        let driver = PhaseCommandDriver::new(Arc::new(TimeoutValidationPhaseService));

        for (root, invalid_seconds) in [("/zero-timeout", 0), ("/excessive-timeout", 3_601)] {
            let outcome =
                driver.handle(PhaseCommandInput::Validate { repository_root: root.into() });

            assert_eq!(outcome.exit_code, 1);
            assert!(
                outcome
                    .stderr
                    .expect("timeout validation diagnostic")
                    .contains(&format!("outside the supported range: {invalid_seconds}"))
            );
        }
    }

    #[test]
    fn test_phase_driver_enter_forwards_present_and_absent_host_verbatim() {
        let service = Arc::new(StubPhaseService::default());
        let driver = PhaseCommandDriver::new(Arc::clone(&service) as Arc<dyn PhaseCommandService>);

        let present = driver.handle(PhaseCommandInput::Enter {
            repository_root: "/repo".into(),
            phase_id: phase_id(),
            host: Some(ProviderNameArg::from_str("codex").expect("valid test host")),
        });
        let absent = driver.handle(PhaseCommandInput::Enter {
            repository_root: "/repo".into(),
            phase_id: phase_id(),
            host: None,
        });

        assert_eq!(present.exit_code, 0);
        assert_eq!(absent.exit_code, 0);
        assert_eq!(
            service.hosts.lock().expect("test mutex").as_slice(),
            [Some("codex".to_owned()), None],
        );
    }

    #[test]
    fn test_phase_driver_enter_completed_renders_output_and_execution_audit() {
        let outcome = PhaseCommandDriver::new(Arc::new(StubPhaseService::default())).handle(
            PhaseCommandInput::Enter {
                repository_root: "/repo".into(),
                phase_id: phase_id(),
                host: None,
            },
        );

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "before\nphase command sequence 0: [\"pre\"]; outcome: exited with 0\nwriter output\nphase command sequence 1: [\"writer\"]; outcome: exited with 0"
            )
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_phase_enter_completed_host_bearing_writer_renders_actual_argv() {
        let command = command_with_timeout(&["bin/sotp", "capability", "exec"], 1);
        let invoked_argv = match command.argv().with_appended_arguments([
            CommandArgument::try_new("--host".to_owned()),
            CommandArgument::try_new("codex".to_owned()),
        ]) {
            Ok(argv) => argv,
            Err(error) => panic!("capability exec host arguments must remain valid: {error:?}"),
        };
        let writer_record = match (ProgramExecutionRecord {
            sequence_index: CommandSequenceIndex::new(3),
            invoked_argv,
            command,
            outcome: ProgramRunOutcome::Exited {
                exit_code: ProgramExitCode::new(0),
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            },
        })
        .classify()
        {
            ClassifiedProgramExecutionRecord::Succeeded(record) => record,
            ClassifiedProgramExecutionRecord::Failed(record) => {
                panic!("expected success, got {record:?}")
            }
        };
        let outcome = render_enter_outcome(PhaseCommandEnterOutcome::Completed {
            pre_entry_records: Vec::new(),
            writer_record,
        });

        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "phase command sequence 3: [\"bin/sotp\",\"capability\",\"exec\",\"--host\",\"codex\"]; outcome: exited with 0"
            )
        );
    }

    #[test]
    fn test_render_argv_with_whitespace_or_newline_preserves_argument_boundaries() {
        let whitespace = command_with_timeout(&["cmd", "a b"], 1);
        let newline = command_with_timeout(&["cmd", "a\nb"], 1);

        assert_eq!(render_argv(whitespace.argv()), "[\"cmd\",\"a b\"]");
        assert_eq!(render_argv(newline.argv()), "[\"cmd\",\"a\\nb\"]");
        assert_ne!(
            render_argv(whitespace.argv()),
            render_argv(command_with_timeout(&["cmd", "a", "b"], 1).argv())
        );
    }

    #[test]
    fn test_phase_id_arg_invalid_transport_value_is_rejected() {
        assert!(" ".parse::<PhaseIdArg>().is_err());
    }

    #[test]
    fn test_phase_id_arg_valid_transport_value_preserves_declaration_id() {
        let parsed = "phase-1".parse::<PhaseIdArg>().expect("valid phase id");

        assert_eq!(parsed.as_declaration_id().as_str(), "phase-1");
    }

    #[test]
    fn test_phase_enter_blocked_renders_invoked_argv_and_nonzero_outcome() {
        let outcome = render_enter_outcome(PhaseCommandEnterOutcome::Blocked {
            completed: vec![successful_record(0, &["pre"], 0, b"before", b"")],
            failed: failed_record(ProgramRunOutcome::Exited {
                exit_code: ProgramExitCode::new(7),
                output: CapturedProgramOutput {
                    stdout: Vec::new(),
                    stderr: b"failed output".to_vec(),
                },
            }),
        });

        assert_eq!(outcome.exit_code, 7);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("before\nphase command sequence 0: [\"pre\"]; outcome: exited with 0")
        );
        let diagnostic = outcome.stderr.expect("blocked diagnostic");
        assert!(diagnostic.contains("failed output"));
        assert!(diagnostic.contains("[\"bin/sotp\",\"capability\",\"exec\",\"--host\",\"codex\"]"));
        assert!(diagnostic.contains("outcome: exited with 7"));
    }

    #[test]
    fn test_phase_enter_blocked_renders_timeout_and_output_limit_outcomes() {
        let timeout = render_enter_outcome(PhaseCommandEnterOutcome::Blocked {
            completed: Vec::new(),
            failed: failed_record(ProgramRunOutcome::TimedOut {
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            }),
        });
        let output_limit = render_enter_outcome(PhaseCommandEnterOutcome::Blocked {
            completed: Vec::new(),
            failed: failed_record(ProgramRunOutcome::OutputLimitExceeded {
                stream: ProgramOutputStream::Stdout,
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            }),
        });

        assert_eq!(timeout.exit_code, 1);
        assert!(timeout.stderr.expect("timeout diagnostic").contains("outcome: timed out"));
        assert_eq!(output_limit.exit_code, 1);
        assert!(
            output_limit
                .stderr
                .expect("output-limit diagnostic")
                .contains("outcome: output limit exceeded on stdout")
        );
    }
}
