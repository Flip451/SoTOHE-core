//! Phase-command composition root.

use std::sync::Arc;

use cli_driver::phase_command::PhaseCommandDriver;
use infrastructure::operator_command_config::FsPhaseCommandConfigLoader;
use infrastructure::program_runner::ProcessProgramRunner;
use usecase::phase_command::{
    PhaseCommandConfigLoaderPort, PhaseCommandInteractor, PhaseCommandService,
};
use usecase::program_runner::ProgramRunnerPort;

/// Composition root for phase command adapters.
#[derive(Debug, Default)]
pub struct PhaseCompositionRoot;

impl PhaseCompositionRoot {
    /// Builds the phase-command driver with its filesystem and process adapters.
    #[must_use]
    pub fn build(&self) -> PhaseCommandDriver {
        let config_loader: Arc<dyn PhaseCommandConfigLoaderPort> =
            Arc::new(FsPhaseCommandConfigLoader::new());
        let runner: Arc<dyn ProgramRunnerPort> = Arc::new(ProcessProgramRunner::new());
        let service: Arc<dyn PhaseCommandService> =
            Arc::new(PhaseCommandInteractor::new(config_loader, runner));

        PhaseCommandDriver::new(service)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::str::FromStr;

    use cli_driver::phase_command::{PhaseCommandInput, PhaseIdArg};

    use super::PhaseCompositionRoot;

    fn write_phase_config(repository_root: &std::path::Path, source: &str) {
        let config_dir = repository_root.join(".harness/config");
        fs::create_dir_all(&config_dir).expect("test config directory is created");
        fs::write(config_dir.join("phase-commands.json"), source)
            .expect("test phase config is written");
    }

    fn test_repository() -> tempfile::TempDir {
        tempfile::tempdir_in(crate::test_support::repo_root_for_tests().join("tmp"))
            .expect("test repository is created")
    }

    #[test]
    fn test_phase_composition_root_valid_config_exposes_all_phase_operations() {
        let repository = test_repository();
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "example",
                    "writer": {"argv": ["printf", "writer\\n"], "timeout_seconds": 1},
                    "pre_entry_commands": [
                        {"argv": ["printf", "pre-entry\\n"], "timeout_seconds": 1}
                    ]
                }]
            }"#,
        );
        let driver = PhaseCompositionRoot.build();

        let validation = driver.handle(PhaseCommandInput::Validate {
            repository_root: repository.path().to_path_buf(),
        });
        assert_eq!(validation.exit_code, 0);

        let explanation = driver.handle(PhaseCommandInput::Explain {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("example").expect("valid phase id"),
        });
        assert_eq!(explanation.exit_code, 0);
        let explanation = explanation.stdout.expect("composed explanation is rendered");
        assert!(explanation.contains("phase example (output limit: 1048576 bytes)"));
        assert!(explanation.contains(r#"pre-entry 0: ["printf","pre-entry\\n"] (timeout: 1s)"#));
        assert!(explanation.contains(r#"writer: ["printf","writer\\n"] (timeout: 1s)"#));

        let outcome = driver.handle(PhaseCommandInput::Enter {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("example").expect("valid phase id"),
            host: None,
        });

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("composed commands emit stdout");
        let pre_entry_position = stdout.find("pre-entry").expect("pre-entry output is present");
        let writer_position = stdout.find("writer").expect("writer output is present");
        assert!(pre_entry_position < writer_position, "pre-entry runs before the writer");
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_phase_composition_root_second_pre_entry_failure_records_prior_success_and_stops_remaining_commands_and_writer()
     {
        let repository = test_repository();
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "blocked",
                    "writer": {"argv": ["printf", "writer-should-not-run\\n"], "timeout_seconds": 1},
                    "pre_entry_commands": [
                        {"argv": ["pwd"], "timeout_seconds": 1},
                        {"argv": ["false"], "timeout_seconds": 1},
                        {"argv": ["printf", "remaining-should-not-run\\n"], "timeout_seconds": 1}
                    ]
                }]
            }"#,
        );

        let outcome = PhaseCompositionRoot.build().handle(PhaseCommandInput::Enter {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("blocked").expect("valid phase id"),
            host: None,
        });

        assert_eq!(outcome.exit_code, 1);
        let stdout = outcome.stdout.expect("completed pre-entry output is rendered");
        assert!(stdout.contains(&repository.path().display().to_string()));
        assert!(stdout.contains("phase command sequence 0: [\"pwd\"]; outcome: exited with 0"));
        assert!(!stdout.contains("phase command sequence 2:"));
        assert!(!stdout.contains("remaining-should-not-run"));
        assert!(!stdout.contains("writer-should-not-run"));
        let stderr = outcome.stderr.expect("first failure is rendered");
        assert!(
            stderr.contains(
                "phase command blocked at sequence 1: [\"false\"]; outcome: exited with 1"
            )
        );
    }

    #[test]
    fn test_phase_composition_root_invalid_config_renders_validation_failure() {
        let repository = test_repository();
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "invalid",
                    "writer": {"argv": ["printf", "must-not-run\\n"], "timeout_seconds": 3601},
                    "pre_entry_commands": []
                }]
            }"#,
        );

        let outcome = PhaseCompositionRoot.build().handle(PhaseCommandInput::Validate {
            repository_root: repository.path().to_path_buf(),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stdout.is_none());
        assert!(
            outcome
                .stderr
                .expect("validation failure is rendered")
                .contains("command timeout is outside the supported range: 3601")
        );
    }

    #[test]
    fn test_phase_composition_root_omitted_timeout_renders_default_in_explain_and_enters() {
        let repository = test_repository();
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "default-timeout",
                    "writer": {"argv": ["printf", "default-timeout-writer\\n"]},
                    "pre_entry_commands": []
                }]
            }"#,
        );
        let driver = PhaseCompositionRoot.build();

        let explanation = driver.handle(PhaseCommandInput::Explain {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("default-timeout").expect("valid phase id"),
        });

        assert_eq!(explanation.exit_code, 0);
        assert!(
            explanation
                .stdout
                .expect("default timeout explanation is rendered")
                .contains(r#"writer: ["printf","default-timeout-writer\\n"] (timeout: 3600s)"#)
        );

        let outcome = driver.handle(PhaseCommandInput::Enter {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("default-timeout").expect("valid phase id"),
            host: None,
        });

        assert_eq!(outcome.exit_code, 0);
        assert!(
            outcome
                .stdout
                .expect("default-timeout enter audit is rendered")
                .contains("phase command sequence 0: [\"printf\",\"default-timeout-writer\\\\n\"]; outcome: exited with 0")
        );
    }

    #[test]
    fn test_phase_composition_root_zero_timeout_renders_validation_failure() {
        let repository = test_repository();
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "zero-timeout",
                    "writer": {"argv": ["printf", "must-not-run\\n"], "timeout_seconds": 0},
                    "pre_entry_commands": []
                }]
            }"#,
        );

        let outcome = PhaseCompositionRoot.build().handle(PhaseCommandInput::Validate {
            repository_root: repository.path().to_path_buf(),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome
                .stderr
                .expect("lower-bound validation failure is rendered")
                .contains("command timeout is outside the supported range: 0")
        );
    }

    #[test]
    fn test_phase_composition_root_timed_out_writer_renders_auditable_outcome() {
        let repository = test_repository();
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "timed-out",
                    "writer": {"argv": ["sleep", "2"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );

        let outcome = PhaseCompositionRoot.build().handle(PhaseCommandInput::Enter {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("timed-out").expect("valid phase id"),
            host: None,
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.expect("timeout outcome is rendered").contains(
            "phase command blocked at sequence 0: [\"sleep\",\"2\"]; outcome: timed out"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_phase_composition_root_truncated_stdout_writer_renders_tail_notice() {
        let repository = test_repository();
        let emitter = repository.path().join("emit-stdout");
        fs::write(&emitter, "#!/bin/sh\nhead -c 1048576 /dev/zero\nprintf 'stdout-tail-marker'\n")
            .expect("stdout emitter is written");
        crate::test_support::make_executable(&emitter);
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "truncated-output",
                    "writer": {"argv": ["./emit-stdout"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );

        let outcome = PhaseCompositionRoot.build().handle(PhaseCommandInput::Enter {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("truncated-output").expect("valid phase id"),
            host: None,
        });

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("truncated stdout outcome is rendered");
        assert!(stdout.contains("[output truncated; showing retained tail]"));
        assert!(stdout.contains("stdout-tail-marker"));
        assert!(
            stdout
                .contains("phase command sequence 0: [\"./emit-stdout\"]; outcome: exited with 0")
        );
        assert!(outcome.stderr.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_phase_composition_root_truncated_stderr_writer_renders_tail_notice() {
        let repository = test_repository();
        let emitter = repository.path().join("emit-stderr");
        fs::write(
            &emitter,
            "#!/bin/sh\nhead -c 1048576 /dev/zero >&2\nprintf 'stderr-tail-marker' >&2\n",
        )
        .expect("stderr emitter is written");
        crate::test_support::make_executable(&emitter);
        write_phase_config(
            repository.path(),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "truncated-stderr",
                    "writer": {"argv": ["./emit-stderr"], "timeout_seconds": 1},
                    "pre_entry_commands": []
                }]
            }"#,
        );

        let outcome = PhaseCompositionRoot.build().handle(PhaseCommandInput::Enter {
            repository_root: repository.path().to_path_buf(),
            phase_id: PhaseIdArg::from_str("truncated-stderr").expect("valid phase id"),
            host: None,
        });

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("completed command audit is rendered");
        assert!(
            stdout
                .contains("phase command sequence 0: [\"./emit-stderr\"]; outcome: exited with 0")
        );
        let stderr = outcome.stderr.expect("truncated stderr outcome is rendered");
        assert!(stderr.contains("[output truncated; showing retained tail]"));
        assert!(stderr.contains("stderr-tail-marker"));
    }
}
