//! `sotp phase` command transport.
//!
//! This module maps clap DTOs to the primary adapter input. Configuration
//! loading, validation, command execution, and outcome rendering remain in the
//! composition, usecase, and driver layers.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::PhaseCompositionRoot;
use cli_driver::capability::ProviderNameArg;
use cli_driver::phase_command::{PhaseCommandDriver, PhaseCommandInput, PhaseIdArg};

use crate::commands::driver_outcome_to_exit;

/// Public phase-command family.
#[derive(Debug, Subcommand)]
pub enum PhaseCommand {
    /// Validate the configured phase-command declarations.
    Validate(PhaseValidateArgs),
    /// Show the configured command sequence for a phase declaration.
    Explain(PhaseIdArgs),
    /// Run a phase declaration's pre-entry commands and canonical writer.
    Enter(PhaseEnterArgs),
}

/// Phase-validation CLI arguments.
#[derive(Debug, Args)]
pub struct PhaseValidateArgs;

/// Shared phase-id CLI arguments.
#[derive(Debug, Args)]
pub struct PhaseIdArgs {
    /// Validated phase declaration identifier.
    pub phase_id: PhaseIdArg,
}

/// Phase-enter CLI arguments with an optional caller-supplied host.
#[derive(Debug, Args)]
pub struct PhaseEnterArgs {
    /// Validated phase declaration identifier.
    pub phase_id: PhaseIdArg,
    /// Provider of the caller entering this phase, forwarded only when supplied.
    #[arg(long)]
    pub host: Option<ProviderNameArg>,
}

/// Dispatches `sotp phase <subcommand>` through the fully wired driver.
pub fn execute(command: PhaseCommand) -> ExitCode {
    let driver = PhaseCompositionRoot.build();
    execute_with_driver(command, &driver)
}

/// Dispatches a phase command through the supplied primary adapter.
pub fn execute_with_driver(command: PhaseCommand, driver: &PhaseCommandDriver) -> ExitCode {
    driver_outcome_to_exit(driver.handle(input_from_command(command)))
}

/// Converts parsed phase arguments to a driver input without interpreting commands.
pub fn input_from_command(command: PhaseCommand) -> PhaseCommandInput {
    let repository_root = PathBuf::from(".");
    match command {
        PhaseCommand::Validate(PhaseValidateArgs) => {
            PhaseCommandInput::Validate { repository_root }
        }
        PhaseCommand::Explain(PhaseIdArgs { phase_id }) => {
            PhaseCommandInput::Explain { repository_root, phase_id }
        }
        PhaseCommand::Enter(PhaseEnterArgs { phase_id, host }) => {
            PhaseCommandInput::Enter { repository_root, phase_id, host }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::too_many_lines, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    use clap::Parser;
    use cli_composition::PhaseCompositionRoot;
    use cli_driver::phase_command::{PhaseCommandDriver, PhaseCommandInput, PhaseIdArg};
    use domain::FreeText;
    use usecase::operator_command::{
        CommandArgument, CommandConfigLoadError, CommandSequenceIndex, ConfiguredCommand,
        OutputCaptureLimitBytes,
    };
    use usecase::phase_command::{
        PhaseCommandEnterError, PhaseCommandEnterOutcome, PhaseCommandExplainError,
        PhaseCommandExplanation, PhaseCommandService, PhaseEnterCommand, PhaseExplainQuery,
        PhaseValidateCommand,
    };
    use usecase::program_runner::{
        CapturedProgramOutput, ClassifiedProgramExecutionRecord, ProgramExecutionRecord,
        ProgramExitCode, ProgramRunOutcome, SuccessfulProgramExecutionRecord,
    };

    use super::{
        PhaseCommand, PhaseEnterArgs, PhaseIdArgs, PhaseValidateArgs, execute, execute_with_driver,
        input_from_command,
    };
    use crate::commands::track::test_support::{process_env_lock, run_in_dir};

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: PhaseCommand,
    }

    #[derive(Default)]
    struct RecordingService {
        calls: Mutex<Vec<String>>,
    }

    impl PhaseCommandService for RecordingService {
        fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError> {
            self.calls
                .lock()
                .expect("test mutex")
                .push(format!("validate:{}", command.repository_root.display()));
            Ok(())
        }

        fn explain(
            &self,
            query: PhaseExplainQuery,
        ) -> Result<PhaseCommandExplanation, PhaseCommandExplainError> {
            self.calls
                .lock()
                .expect("test mutex")
                .push(format!("explain:{}", query.phase_id.as_str()));
            Ok(PhaseCommandExplanation {
                phase_id: query.phase_id,
                pre_entry_commands: vec![command("pre-entry")],
                writer: command("writer"),
                output_limit: OutputCaptureLimitBytes::one_mebibyte(),
            })
        }

        fn enter(
            &self,
            command: PhaseEnterCommand,
        ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
            self.calls.lock().expect("test mutex").push(format!(
                "enter:{}:{:?}",
                command.phase_id.as_str(),
                command.host
            ));
            Ok(PhaseCommandEnterOutcome::Completed {
                pre_entry_records: vec![successful_record(0, "pre-entry")],
                writer_record: successful_record(1, "writer"),
            })
        }
    }

    struct FailingValidationService;

    impl PhaseCommandService for FailingValidationService {
        fn validate(&self, _command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError> {
            Err(CommandConfigLoadError::ReadFailed {
                message: FreeText::new("fixture validation failure"),
            })
        }

        fn explain(
            &self,
            _query: PhaseExplainQuery,
        ) -> Result<PhaseCommandExplanation, PhaseCommandExplainError> {
            Err(PhaseCommandExplainError::Config(fixture_config_error()))
        }

        fn enter(
            &self,
            _command: PhaseEnterCommand,
        ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
            Err(PhaseCommandEnterError::Config(fixture_config_error()))
        }
    }

    fn fixture_config_error() -> CommandConfigLoadError {
        CommandConfigLoadError::ReadFailed { message: FreeText::new("fixture validation failure") }
    }

    fn command(value: &str) -> ConfiguredCommand {
        ConfiguredCommand::try_new(vec![CommandArgument::try_new(value.to_owned())], None)
            .expect("test command")
    }

    fn successful_record(sequence: usize, value: &str) -> SuccessfulProgramExecutionRecord {
        let command = command(value);
        let record = ProgramExecutionRecord {
            sequence_index: CommandSequenceIndex::new(sequence),
            invoked_argv: command.argv().clone(),
            command,
            outcome: ProgramRunOutcome::Exited {
                exit_code: ProgramExitCode::new(0),
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            },
        };
        match record.classify() {
            ClassifiedProgramExecutionRecord::Succeeded(record) => record,
            ClassifiedProgramExecutionRecord::Failed(record) => {
                panic!("expected successful record, got {record:?}")
            }
        }
    }

    fn driver(service: Arc<RecordingService>) -> PhaseCommandDriver {
        PhaseCommandDriver::new(service as Arc<dyn PhaseCommandService>)
    }

    fn write_phase_config(config_path: &std::path::Path, source: &str) {
        fs::write(config_path, source).expect("phase config is written");
    }

    fn write_phase_entry_fixture(repository_root: &Path) {
        let bin_dir = repository_root.join("bin");
        fs::create_dir_all(&bin_dir).expect("fixture bin directory is created");
        let fixture = bin_dir.join("sotp");
        fs::write(
            &fixture,
            r#"#!/bin/sh
printf '%s\n' "$*" >> phase-audit.txt
if [ "$*" = "ref-verify check-approved --chain 1" ] && [ -f fail-chain-1 ]; then
    exit 1
fi
"#,
        )
        .expect("phase entry fixture is written");
        let mut permissions =
            fs::metadata(&fixture).expect("fixture metadata is read").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fixture, permissions).expect("phase entry fixture is executable");
    }

    fn capture_cli_output<T>(run: impl FnOnce() -> T) -> (T, String, String) {
        static CLI_OUTPUT_REDIRECT: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _serialized =
            CLI_OUTPUT_REDIRECT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stdout_capture = tempfile::tempfile().expect("stdout capture is created");
        let mut stderr_capture = tempfile::tempfile().expect("stderr capture is created");
        let stdout_fd = std::io::stdout().as_raw_fd();
        let stderr_fd = std::io::stderr().as_raw_fd();
        std::io::stdout().flush().expect("stdout is flushed");
        std::io::stderr().flush().expect("stderr is flushed");

        // Safety: stdout and stderr are valid process file descriptors.
        let saved_stdout = unsafe { libc::dup(stdout_fd) };
        // Safety: stdout and stderr are valid process file descriptors.
        let saved_stderr = unsafe { libc::dup(stderr_fd) };
        assert!(saved_stdout >= 0, "dup(stdout) failed");
        assert!(saved_stderr >= 0, "dup(stderr) failed");
        // Safety: the capture descriptors and process stdout descriptor are valid.
        let stdout_redirect = unsafe { libc::dup2(stdout_capture.as_raw_fd(), stdout_fd) };
        // Safety: the capture descriptors and process stderr descriptor are valid.
        let stderr_redirect = unsafe { libc::dup2(stderr_capture.as_raw_fd(), stderr_fd) };
        assert_eq!(stdout_redirect, stdout_fd, "dup2(stdout capture) failed");
        assert_eq!(stderr_redirect, stderr_fd, "dup2(stderr capture) failed");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));

        std::io::stdout().flush().expect("captured stdout is flushed");
        std::io::stderr().flush().expect("captured stderr is flushed");
        // Safety: saved descriptors came from dup and restore the original streams.
        let stdout_restore = unsafe { libc::dup2(saved_stdout, stdout_fd) };
        // Safety: saved descriptors came from dup and restore the original streams.
        let stderr_restore = unsafe { libc::dup2(saved_stderr, stderr_fd) };
        assert_eq!(stdout_restore, stdout_fd, "dup2(saved stdout) failed");
        assert_eq!(stderr_restore, stderr_fd, "dup2(saved stderr) failed");
        // Safety: the saved descriptors are no longer needed after restoration.
        assert_eq!(unsafe { libc::close(saved_stdout) }, 0, "close(saved stdout) failed");
        // Safety: the saved descriptors are no longer needed after restoration.
        assert_eq!(unsafe { libc::close(saved_stderr) }, 0, "close(saved stderr) failed");

        stdout_capture.seek(SeekFrom::Start(0)).expect("stdout capture is rewound");
        stderr_capture.seek(SeekFrom::Start(0)).expect("stderr capture is rewound");
        let mut stdout = String::new();
        let mut stderr = String::new();
        stdout_capture.read_to_string(&mut stdout).expect("stdout capture is read");
        stderr_capture.read_to_string(&mut stderr).expect("stderr capture is read");

        match result {
            Ok(value) => (value, stdout, stderr),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[test]
    fn test_phase_validate_parses_without_a_phase_id() {
        let cli = TestCli::try_parse_from(["sotp", "validate"]).expect("validate must parse");

        assert!(matches!(cli.command, PhaseCommand::Validate(PhaseValidateArgs)));
    }

    #[test]
    fn test_phase_id_commands_parse_validated_phase_ids() {
        for verb in ["explain", "enter"] {
            let cli = TestCli::try_parse_from(["sotp", verb, "phase-1"])
                .expect("a valid phase id must parse");
            let phase_id = match cli.command {
                PhaseCommand::Explain(PhaseIdArgs { phase_id })
                | PhaseCommand::Enter(PhaseEnterArgs { phase_id, .. }) => phase_id,
                PhaseCommand::Validate(_) => panic!("expected a phase-id command"),
            };
            assert_eq!(phase_id.as_declaration_id().as_str(), "phase-1");
        }
    }

    #[test]
    fn test_phase_enter_accepts_optional_host_only_at_the_cli_boundary() {
        let cli = TestCli::try_parse_from(["sotp", "enter", "phase-1", "--host", "codex"])
            .expect("host-bearing enter command must parse");

        let PhaseCommand::Enter(args) = cli.command else {
            panic!("expected phase enter command");
        };
        assert_eq!(args.phase_id.as_declaration_id().as_str(), "phase-1");
        assert_eq!(args.host, Some("codex".parse().expect("valid test host")));
    }

    #[test]
    fn test_phase_id_commands_reject_missing_or_invalid_phase_ids() {
        for argv in [
            ["sotp", "explain", ""].as_slice(),
            ["sotp", "enter", ""].as_slice(),
            ["sotp", "explain"].as_slice(),
            ["sotp", "enter"].as_slice(),
        ] {
            assert!(TestCli::try_parse_from(argv).is_err(), "{argv:?} must fail closed");
        }
    }

    #[test]
    fn test_input_from_command_preserves_supplied_and_omitted_host_verbatim() {
        let phase_id = || "phase-1".parse().expect("test phase id");
        let supplied = input_from_command(PhaseCommand::Enter(PhaseEnterArgs {
            phase_id: phase_id(),
            host: Some("codex".parse().expect("test host")),
        }));
        let omitted = input_from_command(PhaseCommand::Enter(PhaseEnterArgs {
            phase_id: phase_id(),
            host: None,
        }));

        assert!(matches!(
            supplied,
            PhaseCommandInput::Enter { host: Some(host), .. }
                if host == "codex".parse().expect("test host")
        ));
        assert!(matches!(omitted, PhaseCommandInput::Enter { host: None, .. }));
    }

    #[test]
    fn test_input_from_command_maps_all_phase_variants_with_repository_root_and_phase_id() {
        let validate = input_from_command(PhaseCommand::Validate(PhaseValidateArgs));
        let PhaseCommandInput::Validate { repository_root } = validate else {
            panic!("validate command must map to validate input");
        };
        assert_eq!(repository_root, Path::new("."));

        let explain = input_from_command(PhaseCommand::Explain(PhaseIdArgs {
            phase_id: "explain-phase".parse().expect("valid explain phase id"),
        }));
        let PhaseCommandInput::Explain { repository_root, phase_id } = explain else {
            panic!("explain command must map to explain input");
        };
        assert_eq!(repository_root, Path::new("."));
        assert_eq!(phase_id.as_declaration_id().as_str(), "explain-phase");

        let enter = input_from_command(PhaseCommand::Enter(PhaseEnterArgs {
            phase_id: "enter-phase".parse().expect("valid enter phase id"),
            host: Some("codex".parse().expect("valid host")),
        }));
        let PhaseCommandInput::Enter { repository_root, phase_id, host } = enter else {
            panic!("enter command must map to enter input");
        };
        assert_eq!(repository_root, Path::new("."));
        assert_eq!(phase_id.as_declaration_id().as_str(), "enter-phase");
        assert_eq!(host, Some("codex".parse().expect("valid host")));
    }

    #[test]
    fn test_execute_with_driver_dispatches_validate_explain_and_enter_commands() {
        let service = Arc::new(RecordingService::default());
        let driver = driver(Arc::clone(&service));

        assert_eq!(
            execute_with_driver(PhaseCommand::Validate(PhaseValidateArgs), &driver),
            std::process::ExitCode::SUCCESS
        );
        assert_eq!(
            execute_with_driver(
                PhaseCommand::Explain(PhaseIdArgs {
                    phase_id: "phase-1".parse().expect("test phase id"),
                }),
                &driver,
            ),
            std::process::ExitCode::SUCCESS
        );
        assert_eq!(
            execute_with_driver(
                PhaseCommand::Enter(PhaseEnterArgs {
                    phase_id: "phase-1".parse().expect("test phase id"),
                    host: None,
                }),
                &driver,
            ),
            std::process::ExitCode::SUCCESS
        );
        assert_eq!(
            service.calls.lock().expect("test mutex").as_slice(),
            ["validate:.", "explain:phase-1", "enter:phase-1:None"]
        );
    }

    #[test]
    fn test_execute_with_driver_runs_composed_phase_commands_in_order_and_fails_closed() {
        let _process_guard = process_env_lock().lock().expect("test process lock");
        let repository = tempfile::tempdir().expect("temporary repository is created");
        let config_dir = repository.path().join(".harness/config");
        fs::create_dir_all(&config_dir).expect("phase config directory is created");
        let config_path = config_dir.join("phase-commands.json");
        let audit_path = repository.path().join("phase-audit.txt");

        fs::write(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "review",
                    "writer": {"argv": ["sh", "-c", "printf writer >> phase-audit.txt"]},
                    "pre_entry_commands": [
                        {"argv": ["sh", "-c", "printf pre-entry >> phase-audit.txt"]}
                    ]
                }]
            }"#,
        )
        .expect("phase config is written");

        run_in_dir(repository.path(), || {
            let driver = PhaseCompositionRoot.build();
            assert_eq!(
                execute_with_driver(PhaseCommand::Validate(PhaseValidateArgs), &driver),
                std::process::ExitCode::SUCCESS
            );
            assert_eq!(
                execute_with_driver(
                    PhaseCommand::Explain(PhaseIdArgs {
                        phase_id: PhaseIdArg::from_str("review").expect("valid phase id"),
                    }),
                    &driver,
                ),
                std::process::ExitCode::SUCCESS
            );
            assert_eq!(
                execute_with_driver(
                    PhaseCommand::Enter(PhaseEnterArgs {
                        phase_id: PhaseIdArg::from_str("review").expect("valid phase id"),
                        host: None,
                    }),
                    &driver,
                ),
                std::process::ExitCode::SUCCESS
            );
        });
        assert_eq!(
            fs::read_to_string(&audit_path).expect("successful phase execution is recorded"),
            "pre-entrywriter"
        );

        fs::write(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "blocked",
                    "writer": {"argv": ["sh", "-c", "printf writer >> phase-audit.txt"]},
                    "pre_entry_commands": [
                        {"argv": ["sh", "-c", "printf first >> phase-audit.txt"]},
                        {"argv": ["false"]},
                        {"argv": ["sh", "-c", "printf later >> phase-audit.txt"]}
                    ]
                }]
            }"#,
        )
        .expect("blocked phase config is written");
        fs::write(&audit_path, "").expect("audit is reset");

        let exit = run_in_dir(repository.path(), || {
            execute_with_driver(
                PhaseCommand::Enter(PhaseEnterArgs {
                    phase_id: PhaseIdArg::from_str("blocked").expect("valid phase id"),
                    host: None,
                }),
                &PhaseCompositionRoot.build(),
            )
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert_eq!(
            fs::read_to_string(&audit_path).expect("blocked execution is recorded"),
            "first"
        );

        fs::write(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "invalid",
                    "writer": {"argv": ["true"], "timeout_seconds": 3601},
                    "pre_entry_commands": []
                }]
            }"#,
        )
        .expect("invalid phase config is written");
        let exit = run_in_dir(repository.path(), || {
            execute_with_driver(
                PhaseCommand::Validate(PhaseValidateArgs),
                &PhaseCompositionRoot.build(),
            )
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }

    #[test]
    fn test_phase_enter_type_design_matrix_failure_blocks_remaining_checks_and_writer() {
        let _process_guard = process_env_lock().lock().expect("test process lock");
        let repository = tempfile::tempdir().expect("temporary repository is created");
        let config_dir = repository.path().join(".harness/config");
        fs::create_dir_all(&config_dir).expect("phase config directory is created");
        write_phase_entry_fixture(repository.path());
        write_phase_config(
            &config_dir.join("phase-commands.json"),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "type-design",
                    "writer": {
                        "argv": ["bin/sotp", "capability", "exec", "type-designer"]
                    },
                    "pre_entry_commands": [
                        {
                            "argv": ["bin/sotp", "signal", "check-spec-adr", "--gate", "commit"]
                        },
                        {
                            "argv": ["bin/sotp", "ref-verify", "check-approved", "--chain", "1"]
                        },
                        {
                            "argv": [
                                "bin/sotp", "review", "check-zero-findings", "--scope", "spec",
                                "--round", "final"
                            ]
                        }
                    ]
                }]
            }"#,
        );
        fs::write(repository.path().join("fail-chain-1"), "blocked")
            .expect("failing convergence check is configured");

        let exit = run_in_dir(repository.path(), || {
            execute_with_driver(
                PhaseCommand::Enter(PhaseEnterArgs {
                    phase_id: PhaseIdArg::from_str("type-design").expect("valid phase id"),
                    host: None,
                }),
                &PhaseCompositionRoot.build(),
            )
        });

        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert_eq!(
            fs::read_to_string(repository.path().join("phase-audit.txt"))
                .expect("blocked phase entry is recorded"),
            "signal check-spec-adr --gate commit\nref-verify check-approved --chain 1\n"
        );
    }

    #[test]
    fn test_phase_enter_type_design_matrix_success_runs_every_check_then_writer() {
        let _process_guard = process_env_lock().lock().expect("test process lock");
        let repository = tempfile::tempdir().expect("temporary repository is created");
        let config_dir = repository.path().join(".harness/config");
        fs::create_dir_all(&config_dir).expect("phase config directory is created");
        write_phase_entry_fixture(repository.path());
        write_phase_config(
            &config_dir.join("phase-commands.json"),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "type-design",
                    "writer": {
                        "argv": ["bin/sotp", "capability", "exec", "type-designer"]
                    },
                    "pre_entry_commands": [
                        {
                            "argv": ["bin/sotp", "signal", "check-spec-adr", "--gate", "commit"]
                        },
                        {
                            "argv": ["bin/sotp", "ref-verify", "check-approved", "--chain", "1"]
                        },
                        {
                            "argv": [
                                "bin/sotp", "review", "check-zero-findings", "--scope", "spec",
                                "--round", "final"
                            ]
                        }
                    ]
                }]
            }"#,
        );

        let exit = run_in_dir(repository.path(), || {
            execute_with_driver(
                PhaseCommand::Enter(PhaseEnterArgs {
                    phase_id: PhaseIdArg::from_str("type-design").expect("valid phase id"),
                    host: None,
                }),
                &PhaseCompositionRoot.build(),
            )
        });

        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        assert_eq!(
            fs::read_to_string(repository.path().join("phase-audit.txt"))
                .expect("completed phase entry is recorded"),
            concat!(
                "signal check-spec-adr --gate commit\n",
                "ref-verify check-approved --chain 1\n",
                "review check-zero-findings --scope spec --round final\n",
                "capability exec type-designer\n"
            )
        );
    }

    #[test]
    fn test_shipped_phase_commands_declare_direct_upstream_convergence_matrix() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../../../.harness/config/phase-commands.json"))
                .expect("shipped phase config is valid JSON");
        let phases = config
            .get("phases")
            .and_then(serde_json::Value::as_array)
            .expect("phase declarations are an array");

        let commands_for = |phase_id: &str| {
            phases
                .iter()
                .find(|phase| phase.get("id").and_then(serde_json::Value::as_str) == Some(phase_id))
                .unwrap_or_else(|| panic!("{phase_id} declaration is present"))
                .get("pre_entry_commands")
                .and_then(serde_json::Value::as_array)
                .expect("pre-entry commands are an array")
                .iter()
                .map(|command| command.get("argv").expect("command argv is present").clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            commands_for("spec-design"),
            vec![
                serde_json::json!(["bin/sotp", "signal", "check-adr-user", "--gate", "commit"]),
                serde_json::json!([
                    "bin/sotp",
                    "review",
                    "check-zero-findings",
                    "--scope",
                    "adr",
                    "--round",
                    "final"
                ]),
            ]
        );
        assert_eq!(
            commands_for("type-design"),
            vec![
                serde_json::json!(["bin/sotp", "signal", "check-spec-adr", "--gate", "commit"]),
                serde_json::json!(["bin/sotp", "ref-verify", "check-approved", "--chain", "1"]),
                serde_json::json!([
                    "bin/sotp",
                    "review",
                    "check-zero-findings",
                    "--scope",
                    "spec",
                    "--round",
                    "final"
                ]),
            ]
        );
        assert_eq!(
            commands_for("impl-plan"),
            vec![
                serde_json::json!(["bin/sotp", "signal", "check-catalog-spec", "--gate", "commit"]),
                serde_json::json!(["bin/sotp", "ref-verify", "check-approved", "--chain", "2"]),
                serde_json::json!([
                    "bin/sotp",
                    "review",
                    "check-zero-findings",
                    "--scope",
                    "types",
                    "--round",
                    "final"
                ]),
            ]
        );
    }

    #[test]
    fn test_execute_with_driver_renders_default_timeout_and_failure_outcomes() {
        let _process_guard = process_env_lock().lock().expect("test process lock");
        let repository = tempfile::tempdir().expect("temporary repository is created");
        let config_dir = repository.path().join(".harness/config");
        fs::create_dir_all(&config_dir).expect("phase config directory is created");
        let config_path = config_dir.join("phase-commands.json");

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "default-timeout",
                    "writer": {"argv": ["printf", "default-timeout-writer\\n"]},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute_with_driver(
                    PhaseCommand::Explain(PhaseIdArgs {
                        phase_id: PhaseIdArg::from_str("default-timeout").expect("valid phase id"),
                    }),
                    &PhaseCompositionRoot.build(),
                )
            })
        });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        assert!(
            stdout.contains(r#"writer: ["printf","default-timeout-writer\\n"] (timeout: 3600s)"#)
        );

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "zero-timeout",
                    "writer": {"argv": ["true"], "timeout_seconds": 0},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute_with_driver(
                    PhaseCommand::Validate(PhaseValidateArgs),
                    &PhaseCompositionRoot.build(),
                )
            })
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert!(stdout.is_empty());
        assert!(stderr.contains("command timeout is outside the supported range: 0"));

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "output-limited",
                    "writer": {"argv": ["yes"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute_with_driver(
                    PhaseCommand::Enter(PhaseEnterArgs {
                        phase_id: PhaseIdArg::from_str("output-limited").expect("valid phase id"),
                        host: None,
                    }),
                    &PhaseCompositionRoot.build(),
                )
            })
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert_eq!(stdout.len(), 1_048_577, "1 MiB capture plus CLI newline");
        assert!(stderr.contains(
            "phase command blocked at sequence 0: [\"yes\"]; outcome: output limit exceeded on stdout"
        ));

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "timed-out",
                    "writer": {"argv": ["sleep", "2"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute_with_driver(
                    PhaseCommand::Enter(PhaseEnterArgs {
                        phase_id: PhaseIdArg::from_str("timed-out").expect("valid phase id"),
                        host: None,
                    }),
                    &PhaseCompositionRoot.build(),
                )
            })
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert!(stdout.is_empty());
        assert!(stderr.contains(
            "phase command blocked at sequence 0: [\"sleep\",\"2\"]; outcome: timed out"
        ));
    }

    #[test]
    fn test_execute_composes_default_timeout_and_failure_phase_outcomes() {
        let _process_guard = process_env_lock().lock().expect("test process lock");
        let repository = tempfile::tempdir().expect("temporary repository is created");
        let config_dir = repository.path().join(".harness/config");
        fs::create_dir_all(&config_dir).expect("phase config directory is created");
        let config_path = config_dir.join("phase-commands.json");

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "default-timeout",
                    "writer": {"argv": ["true"]},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute(PhaseCommand::Explain(PhaseIdArgs {
                    phase_id: PhaseIdArg::from_str("default-timeout").expect("valid phase id"),
                }))
            })
        });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        assert!(
            stdout.contains(r#"writer: ["true"] (timeout: 3600s)"#),
            "default timeout must reach the CLI output"
        );

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "zero-timeout",
                    "writer": {"argv": ["true"], "timeout_seconds": 0},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || execute(PhaseCommand::Validate(PhaseValidateArgs)))
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert!(stdout.is_empty());
        assert!(stderr.contains("command timeout is outside the supported range: 0"));

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "output-limited",
                    "writer": {"argv": ["yes"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute(PhaseCommand::Enter(PhaseEnterArgs {
                    phase_id: PhaseIdArg::from_str("output-limited").expect("valid phase id"),
                    host: None,
                }))
            })
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert_eq!(stdout.len(), 1_048_577, "1 MiB capture plus CLI newline");
        assert!(stderr.contains(
            "phase command blocked at sequence 0: [\"yes\"]; outcome: output limit exceeded on stdout"
        ));

        write_phase_config(
            &config_path,
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "timed-out",
                    "writer": {"argv": ["sleep", "2"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let (exit, stdout, stderr) = capture_cli_output(|| {
            run_in_dir(repository.path(), || {
                execute(PhaseCommand::Enter(PhaseEnterArgs {
                    phase_id: PhaseIdArg::from_str("timed-out").expect("valid phase id"),
                    host: None,
                }))
            })
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert!(stdout.is_empty());
        assert!(stderr.contains(
            "phase command blocked at sequence 0: [\"sleep\",\"2\"]; outcome: timed out"
        ));
    }

    #[test]
    fn test_phase_validate_failure_is_rendered_as_a_nonzero_cli_outcome() {
        let driver = PhaseCommandDriver::new(Arc::new(FailingValidationService));
        let outcome = driver.handle(input_from_command(PhaseCommand::Validate(PhaseValidateArgs)));

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stderr.expect("validation diagnostic").contains("fixture validation failure")
        );
    }

    #[test]
    fn test_phase_transport_never_interprets_the_command_or_filesystem() {
        let source = include_str!("phase.rs");
        let production_source = source.split("#[cfg(test)]").next().expect("test delimiter");

        for forbidden in ["std::process::Command", "std::fs::", "read_to_string", "sh -c"] {
            assert!(
                !production_source.contains(forbidden),
                "phase transport must not contain {forbidden}"
            );
        }

        let input = input_from_command(PhaseCommand::Validate(PhaseValidateArgs));
        let PhaseCommandInput::Validate { repository_root } = input else {
            panic!("validate must map to driver input");
        };
        assert_eq!(repository_root, Path::new("."));
    }
}
