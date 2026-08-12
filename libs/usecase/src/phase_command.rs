//! Typed phase-command configuration and validation boundary.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::capability_exec::ProviderName;
use crate::operator_command::{
    CommandArgument, CommandArgv, CommandConfigLoadError, CommandConfigSchemaVersion,
    CommandConfigValidationError, CommandDeclarationId, CommandSequenceIndex, ConfiguredCommand,
    ConfiguredCommandValidationError, OutputCaptureLimitBytes, is_sotp_executable,
};
use crate::program_runner::{
    ClassifiedProgramExecutionRecord, FailedProgramExecutionRecord, ProgramExecutionRecord,
    ProgramInvocation, ProgramRunnerError, ProgramRunnerPort, SuccessfulProgramExecutionRecord,
};

/// A configured writer and its ordered pre-entry commands for one phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseCommandDeclaration {
    id: CommandDeclarationId,
    writer: ConfiguredCommand,
    pre_entry_commands: Vec<ConfiguredCommand>,
}

impl PhaseCommandDeclaration {
    #[must_use]
    pub fn new(
        id: CommandDeclarationId,
        writer: ConfiguredCommand,
        pre_entry_commands: Vec<ConfiguredCommand>,
    ) -> Self {
        Self { id, writer, pre_entry_commands }
    }

    #[must_use]
    pub fn id(&self) -> &CommandDeclarationId {
        &self.id
    }

    #[must_use]
    pub fn writer(&self) -> &ConfiguredCommand {
        &self.writer
    }

    #[must_use]
    pub fn pre_entry_commands(&self) -> &[ConfiguredCommand] {
        &self.pre_entry_commands
    }
}

/// Validated machine-readable phase-command configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseCommandConfig {
    declarations: Vec<PhaseCommandDeclaration>,
}

impl PhaseCommandConfig {
    /// Validates the schema version and unique declaration identifiers.
    ///
    /// # Errors
    /// Returns an error when the schema version is unsupported or a declaration id repeats.
    pub fn try_new(
        schema_version: CommandConfigSchemaVersion,
        declarations: Vec<PhaseCommandDeclaration>,
    ) -> Result<Self, PhaseCommandConfigValidationError> {
        if schema_version.validate().is_err() {
            return Err(PhaseCommandConfigValidationError::InvalidSchemaVersion {
                actual: schema_version,
            });
        }

        for (index, declaration) in declarations.iter().enumerate() {
            if declarations.iter().take(index).any(|prior| prior.id == declaration.id) {
                return Err(PhaseCommandConfigValidationError::DuplicateDeclaration(
                    declaration.id.clone(),
                ));
            }
        }

        Ok(Self { declarations })
    }

    #[must_use]
    pub fn declaration(&self, id: &CommandDeclarationId) -> Option<&PhaseCommandDeclaration> {
        self.declarations.iter().find(|declaration| declaration.id == *id)
    }
}

/// Aggregate-level phase-command configuration validation errors.
#[derive(Debug, Error)]
pub enum PhaseCommandConfigValidationError {
    #[error("unsupported command configuration schema version: {actual:?}")]
    InvalidSchemaVersion { actual: CommandConfigSchemaVersion },
    #[error("duplicate command declaration: {}", .0.as_str())]
    DuplicateDeclaration(CommandDeclarationId),
}

impl PhaseCommandConfigValidationError {
    #[must_use]
    pub fn into_command_config_validation_error(self) -> CommandConfigValidationError {
        match self {
            Self::InvalidSchemaVersion { actual } => {
                CommandConfigValidationError::InvalidSchemaVersion { actual }
            }
            Self::DuplicateDeclaration(id) => {
                CommandConfigValidationError::DuplicateDeclaration(id)
            }
        }
    }
}

/// Loads a validated phase-command configuration for a repository.
pub trait PhaseCommandConfigLoaderPort: Send + Sync {
    /// # Errors
    /// Returns an error when the configuration cannot be read, decoded, or validated.
    fn load(&self, repository_root: &Path) -> Result<PhaseCommandConfig, CommandConfigLoadError>;
}

/// Request to validate the phase-command configuration rooted at a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseValidateCommand {
    pub repository_root: PathBuf,
}

/// Request to explain one configured phase without executing its commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseExplainQuery {
    pub repository_root: PathBuf,
    pub phase_id: CommandDeclarationId,
}

/// Observable command declaration details for one configured phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseCommandExplanation {
    pub phase_id: CommandDeclarationId,
    pub pre_entry_commands: Vec<ConfiguredCommand>,
    pub writer: ConfiguredCommand,
    pub output_limit: OutputCaptureLimitBytes,
}

/// Failures while loading or resolving a phase explanation.
#[derive(Debug, Error)]
pub enum PhaseCommandExplainError {
    #[error(transparent)]
    Config(#[from] CommandConfigLoadError),
    #[error("unknown phase: {}", .0.as_str())]
    UnknownPhase(CommandDeclarationId),
}

/// Request to enter one configured phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseEnterCommand {
    pub repository_root: PathBuf,
    pub phase_id: CommandDeclarationId,
    pub host: Option<ProviderName>,
}

/// Observable result of entering a phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseCommandEnterOutcome {
    Completed {
        pre_entry_records: Vec<SuccessfulProgramExecutionRecord>,
        writer_record: SuccessfulProgramExecutionRecord,
    },
    Blocked {
        completed: Vec<SuccessfulProgramExecutionRecord>,
        failed: FailedProgramExecutionRecord,
    },
}

/// Failures while entering a configured phase.
#[derive(Debug, Error)]
pub enum PhaseCommandEnterError {
    #[error(transparent)]
    Config(#[from] CommandConfigLoadError),
    #[error("unknown phase: {}", .0.as_str())]
    UnknownPhase(CommandDeclarationId),
    #[error(transparent)]
    Runner(#[from] ProgramRunnerError),
}

/// Application service for phase-command operations.
pub trait PhaseCommandService: Send + Sync {
    /// # Errors
    /// Returns an error when the phase configuration cannot be loaded or validated.
    fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError>;

    /// # Errors
    /// Returns an error when the configuration cannot be loaded or the requested phase is absent.
    fn explain(
        &self,
        query: PhaseExplainQuery,
    ) -> Result<PhaseCommandExplanation, PhaseCommandExplainError>;

    /// # Errors
    /// Returns an error when the configuration cannot be loaded, the phase is absent, or a
    /// command cannot be run.
    fn enter(
        &self,
        command: PhaseEnterCommand,
    ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError>;
}

/// Usecase interactor that runs pre-entry commands before a phase writer.
pub struct PhaseCommandInteractor {
    config_loader: Arc<dyn PhaseCommandConfigLoaderPort>,
    runner: Arc<dyn ProgramRunnerPort>,
}

impl PhaseCommandInteractor {
    #[must_use]
    pub fn new(
        config_loader: Arc<dyn PhaseCommandConfigLoaderPort>,
        runner: Arc<dyn ProgramRunnerPort>,
    ) -> Self {
        Self { config_loader, runner }
    }

    fn run(
        &self,
        sequence_index: CommandSequenceIndex,
        configured: ConfiguredCommand,
        argv: crate::operator_command::CommandArgv,
        repository_root: &Path,
    ) -> Result<ProgramExecutionRecord, ProgramRunnerError> {
        let outcome = self.runner.run(ProgramInvocation {
            argv: argv.clone(),
            repository_root: repository_root.to_path_buf(),
            timeout: configured.timeout(),
            stdout_limit: OutputCaptureLimitBytes::one_mebibyte(),
            stderr_limit: OutputCaptureLimitBytes::one_mebibyte(),
        })?;
        Ok(ProgramExecutionRecord {
            sequence_index,
            command: configured,
            invoked_argv: argv,
            outcome,
        })
    }
}

impl PhaseCommandService for PhaseCommandInteractor {
    fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError> {
        self.config_loader.load(&command.repository_root).map(|_| ())
    }

    fn explain(
        &self,
        query: PhaseExplainQuery,
    ) -> Result<PhaseCommandExplanation, PhaseCommandExplainError> {
        let config = self.config_loader.load(&query.repository_root)?;
        let declaration = config
            .declaration(&query.phase_id)
            .ok_or_else(|| PhaseCommandExplainError::UnknownPhase(query.phase_id.clone()))?;
        Ok(PhaseCommandExplanation {
            phase_id: query.phase_id,
            pre_entry_commands: declaration.pre_entry_commands().to_vec(),
            writer: declaration.writer().clone(),
            output_limit: OutputCaptureLimitBytes::one_mebibyte(),
        })
    }

    fn enter(
        &self,
        command: PhaseEnterCommand,
    ) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> {
        let config = self.config_loader.load(&command.repository_root)?;
        let declaration = config
            .declaration(&command.phase_id)
            .ok_or_else(|| PhaseCommandEnterError::UnknownPhase(command.phase_id.clone()))?;
        let mut completed = Vec::new();

        for (position, configured) in declaration.pre_entry_commands().iter().cloned().enumerate() {
            let record = self.run(
                CommandSequenceIndex::new(position),
                configured.clone(),
                configured.argv().clone(),
                &command.repository_root,
            )?;
            match record.classify() {
                ClassifiedProgramExecutionRecord::Succeeded(record) => completed.push(record),
                ClassifiedProgramExecutionRecord::Failed(failed) => {
                    return Ok(PhaseCommandEnterOutcome::Blocked { completed, failed });
                }
            }
        }

        let writer = declaration.writer().clone();
        let record = self.run(
            CommandSequenceIndex::new(completed.len()),
            writer.clone(),
            writer_argv(&writer, command.host.as_ref())?,
            &command.repository_root,
        )?;
        match record.classify() {
            ClassifiedProgramExecutionRecord::Succeeded(writer_record) => {
                Ok(PhaseCommandEnterOutcome::Completed {
                    pre_entry_records: completed,
                    writer_record,
                })
            }
            ClassifiedProgramExecutionRecord::Failed(failed) => {
                Ok(PhaseCommandEnterOutcome::Blocked { completed, failed })
            }
        }
    }
}

fn writer_argv(
    command: &ConfiguredCommand,
    host: Option<&ProviderName>,
) -> Result<CommandArgv, CommandConfigLoadError> {
    match host.filter(|_| is_capability_exec(command)) {
        Some(host) => command
            .argv()
            .with_appended_arguments([
                CommandArgument::try_new("--host".to_owned()),
                CommandArgument::try_new(host.as_str().to_owned()),
            ])
            .map_err(|error| {
                CommandConfigLoadError::Invalid(
                    ConfiguredCommandValidationError::Argv(error).into(),
                )
            }),
        None => Ok(command.argv().clone()),
    }
}

fn is_capability_exec(command: &ConfiguredCommand) -> bool {
    let Some((executable, arguments)) = command.argv().arguments().split_first() else {
        return false;
    };
    is_sotp_executable(executable)
        && arguments.iter().take(2).map(CommandArgument::as_str).eq(["capability", "exec"])
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::{
        PhaseCommandConfig, PhaseCommandConfigLoaderPort, PhaseCommandConfigValidationError,
        PhaseCommandDeclaration, PhaseCommandEnterOutcome, PhaseCommandExplainError,
        PhaseCommandExplanation, PhaseCommandInteractor, PhaseCommandService, PhaseEnterCommand,
        PhaseExplainQuery, PhaseValidateCommand,
    };
    use crate::capability_exec::ProviderName;
    use crate::operator_command::{
        CommandArgument, CommandConfigLoadError, CommandConfigSchemaVersion,
        CommandConfigValidationError, CommandDeclarationId, ConfiguredCommand,
        OutputCaptureLimitBytes, UnvalidatedTimeoutSeconds,
    };
    use crate::program_runner::{
        CapturedProgramOutput, ProgramExitCode, ProgramInvocation, ProgramOutputStream,
        ProgramRunOutcome, ProgramRunnerError, ProgramRunnerPort,
    };

    fn command(value: &str) -> ConfiguredCommand {
        ConfiguredCommand::try_new(vec![CommandArgument::try_new(value.to_owned())], None).unwrap()
    }

    fn command_with_timeout(value: &str, seconds: u64) -> ConfiguredCommand {
        ConfiguredCommand::try_new(
            vec![CommandArgument::try_new(value.to_owned())],
            Some(UnvalidatedTimeoutSeconds::new(seconds)),
        )
        .unwrap()
    }

    fn declaration(id: &str) -> PhaseCommandDeclaration {
        PhaseCommandDeclaration::new(
            CommandDeclarationId::try_new(id.to_owned()).unwrap(),
            command("writer"),
            Vec::new(),
        )
    }

    struct FailingLoader;

    impl PhaseCommandConfigLoaderPort for FailingLoader {
        fn load(
            &self,
            _repository_root: &Path,
        ) -> Result<PhaseCommandConfig, CommandConfigLoadError> {
            Err(CommandConfigLoadError::ReadFailed {
                message: domain::FreeText::new("configuration unavailable".to_owned()),
            })
        }
    }

    #[derive(Clone)]
    struct RawCommand {
        arguments: Vec<String>,
        timeout_seconds: Option<u64>,
    }

    #[derive(Clone)]
    struct RawDeclaration {
        id: String,
        writer: RawCommand,
        pre_entry_commands: Vec<RawCommand>,
    }

    #[derive(Clone)]
    struct RawConfig {
        schema_version: u32,
        declarations: Vec<RawDeclaration>,
    }

    struct RootedValidatingLoader {
        configurations: BTreeMap<PathBuf, RawConfig>,
    }

    impl RootedValidatingLoader {
        fn new(configurations: impl IntoIterator<Item = (PathBuf, RawConfig)>) -> Self {
            Self { configurations: configurations.into_iter().collect() }
        }
    }

    impl PhaseCommandConfigLoaderPort for RootedValidatingLoader {
        fn load(
            &self,
            repository_root: &Path,
        ) -> Result<PhaseCommandConfig, crate::operator_command::CommandConfigLoadError> {
            let raw = self.configurations.get(repository_root).ok_or_else(|| {
                crate::operator_command::CommandConfigLoadError::ReadFailed {
                    message: domain::FreeText::new(format!(
                        "no phase command configuration for {}",
                        repository_root.display()
                    )),
                }
            })?;
            decode_config(raw)
        }
    }

    fn decode_config(
        raw: &RawConfig,
    ) -> Result<PhaseCommandConfig, crate::operator_command::CommandConfigLoadError> {
        let declarations = raw
            .declarations
            .iter()
            .map(|declaration| {
                let id =
                    CommandDeclarationId::try_new(declaration.id.clone()).map_err(|error| {
                        crate::operator_command::CommandConfigLoadError::Invalid(error.into())
                    })?;
                let writer = decode_command(&declaration.writer)?;
                let pre_entry_commands = declaration
                    .pre_entry_commands
                    .iter()
                    .map(decode_command)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(PhaseCommandDeclaration::new(id, writer, pre_entry_commands))
            })
            .collect::<Result<Vec<_>, crate::operator_command::CommandConfigLoadError>>()?;

        PhaseCommandConfig::try_new(
            CommandConfigSchemaVersion::new(raw.schema_version),
            declarations,
        )
        .map_err(|error| {
            crate::operator_command::CommandConfigLoadError::Invalid(
                error.into_command_config_validation_error(),
            )
        })
    }

    fn decode_command(
        command: &RawCommand,
    ) -> Result<ConfiguredCommand, crate::operator_command::CommandConfigLoadError> {
        let arguments = command.arguments.iter().cloned().map(CommandArgument::try_new).collect();
        ConfiguredCommand::try_new(
            arguments,
            command.timeout_seconds.map(UnvalidatedTimeoutSeconds::new),
        )
        .map_err(|error| crate::operator_command::CommandConfigLoadError::Invalid(error.into()))
    }

    fn raw_command(arguments: &[&str], timeout_seconds: Option<u64>) -> RawCommand {
        RawCommand {
            arguments: arguments.iter().map(ToString::to_string).collect(),
            timeout_seconds,
        }
    }

    fn raw_declaration(
        id: &str,
        writer: RawCommand,
        pre_entry_commands: Vec<RawCommand>,
    ) -> RawDeclaration {
        RawDeclaration { id: id.to_owned(), writer, pre_entry_commands }
    }

    fn raw_config(schema_version: u32, declarations: Vec<RawDeclaration>) -> RawConfig {
        RawConfig { schema_version, declarations }
    }

    #[test]
    fn test_phase_command_declaration_preserves_writer_and_pre_entry_order() {
        let writer = command("writer");
        let first = command("first");
        let second = command_with_timeout("second", 45);
        let declaration = PhaseCommandDeclaration::new(
            CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            writer.clone(),
            vec![first.clone(), second.clone()],
        );

        assert_eq!(declaration.id().as_str(), "phase-one");
        assert_eq!(declaration.writer(), &writer);
        assert_eq!(declaration.pre_entry_commands(), &[first, second]);
        assert_eq!(declaration.writer().timeout().as_secs(), 3_600);
        assert_eq!(
            declaration.pre_entry_commands().get(1).map(|command| command.timeout().as_secs()),
            Some(45)
        );
    }

    #[test]
    fn test_phase_command_explanation_preserves_observable_expanded_commands_and_limits() {
        let phase_id = CommandDeclarationId::try_new("phase-one".to_owned()).unwrap();
        let writer = command("writer");
        let pre_entry =
            vec![command("first-pre-entry"), command_with_timeout("second-pre-entry", 45)];
        let explanation = PhaseCommandExplanation {
            phase_id: phase_id.clone(),
            pre_entry_commands: pre_entry.clone(),
            writer: writer.clone(),
            output_limit: OutputCaptureLimitBytes::one_mebibyte(),
        };
        let query = PhaseExplainQuery { repository_root: "/repo".into(), phase_id };

        assert_eq!(query.repository_root, PathBuf::from("/repo"));
        assert_eq!(query.phase_id, explanation.phase_id);
        assert_eq!(explanation.pre_entry_commands, pre_entry);
        assert_eq!(explanation.writer, writer);
        assert_eq!(explanation.output_limit.as_usize(), 1_048_576);
    }

    #[test]
    fn test_phase_command_explain_error_preserves_unknown_phase_identifier() {
        let phase_id = CommandDeclarationId::try_new("unknown-phase".to_owned()).unwrap();
        let error = PhaseCommandExplainError::UnknownPhase(phase_id);

        assert_eq!(error.to_string(), "unknown phase: unknown-phase");
    }

    #[test]
    fn test_phase_command_explain_error_converts_configuration_failure() {
        let error = PhaseCommandExplainError::from(CommandConfigLoadError::ReadFailed {
            message: domain::FreeText::new("configuration is unavailable".to_owned()),
        });

        assert!(matches!(
            error,
            PhaseCommandExplainError::Config(CommandConfigLoadError::ReadFailed { message })
                if message.as_str() == "configuration is unavailable"
        ));
    }

    #[test]
    fn test_phase_command_config_duplicate_declaration_id_is_rejected() {
        let error = PhaseCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![declaration("phase-one"), declaration("phase-one")],
        )
        .unwrap_err();

        assert!(matches!(
            error,
            PhaseCommandConfigValidationError::DuplicateDeclaration(id) if id.as_str() == "phase-one"
        ));
    }

    #[test]
    fn test_phase_command_config_invalid_schema_is_rejected_and_converts() {
        let error = PhaseCommandConfig::try_new(CommandConfigSchemaVersion::new(2), Vec::new())
            .unwrap_err();

        assert!(matches!(
            error.into_command_config_validation_error(),
            CommandConfigValidationError::InvalidSchemaVersion { actual } if actual.as_u32() == 2
        ));
    }

    #[test]
    fn test_phase_command_config_rejects_recursive_review_fix_local_prefix() {
        for executable in ["./bin/sotp", "target/debug/sotp", "/opt/sotohe/target/debug/sotp"] {
            for subcommand in [["phase", "enter"], ["review", "local"], ["review", "fix-local"]] {
                let recursive_writer = raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&[executable, subcommand[0], subcommand[1]], None),
                        Vec::new(),
                    )],
                );
                let recursive_pre_entry = raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], None),
                        vec![raw_command(&[executable, subcommand[0], subcommand[1]], None)],
                    )],
                );

                for configuration in [&recursive_writer, &recursive_pre_entry] {
                    assert!(matches!(
                        decode_config(configuration),
                        Err(crate::operator_command::CommandConfigLoadError::Invalid(
                            CommandConfigValidationError::RecursiveInvocation { .. }
                        ))
                    ));
                }
            }
        }
    }

    #[test]
    fn test_phase_command_config_rejects_persisted_host_in_writer_and_pre_entry() {
        let persisted_host_writer = raw_config(
            1,
            vec![raw_declaration(
                "phase-one",
                raw_command(&["writer", "--host=claude"], None),
                Vec::new(),
            )],
        );
        let persisted_host_pre_entry = raw_config(
            1,
            vec![raw_declaration(
                "phase-one",
                raw_command(&["writer"], None),
                vec![raw_command(&["pre-entry", "--host=claude"], None)],
            )],
        );

        for configuration in [&persisted_host_writer, &persisted_host_pre_entry] {
            assert!(matches!(
                decode_config(configuration),
                Err(crate::operator_command::CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::PersistedHostArgument
                ))
            ));
        }
    }

    #[test]
    fn test_phase_command_config_preserves_canonical_writer_order_and_default_timeout() {
        let phase_id = CommandDeclarationId::try_new("phase-one".to_owned()).unwrap();
        let config = PhaseCommandConfig::try_new(
            CommandConfigSchemaVersion::new(1),
            vec![PhaseCommandDeclaration::new(
                phase_id.clone(),
                command("writer"),
                vec![command("first-pre-entry"), command_with_timeout("second-pre-entry", 45)],
            )],
        )
        .unwrap();
        let declaration = config.declaration(&phase_id).unwrap();

        assert_eq!(
            declaration
                .writer()
                .argv()
                .arguments()
                .iter()
                .map(CommandArgument::as_str)
                .collect::<Vec<_>>(),
            vec!["writer"]
        );
        assert_eq!(
            declaration
                .pre_entry_commands()
                .iter()
                .filter_map(|command| command.argv().arguments().first())
                .map(CommandArgument::as_str)
                .collect::<Vec<_>>(),
            vec!["first-pre-entry", "second-pre-entry"]
        );
        assert_eq!(declaration.writer().timeout().as_secs(), 3_600);
        assert_eq!(
            declaration.pre_entry_commands().first().map(|command| command.timeout().as_secs()),
            Some(3_600)
        );
    }

    #[test]
    fn test_phase_command_config_loader_rejects_invalid_config_before_execution() {
        let loader = RootedValidatingLoader::new([
            (
                PathBuf::from("/invalid-schema"),
                raw_config(
                    2,
                    vec![raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new())],
                ),
            ),
            (
                PathBuf::from("/duplicate"),
                raw_config(
                    1,
                    vec![
                        raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new()),
                        raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new()),
                    ],
                ),
            ),
            (
                PathBuf::from("/empty-argv"),
                raw_config(
                    1,
                    vec![raw_declaration("phase-one", raw_command(&[], None), Vec::new())],
                ),
            ),
            (
                PathBuf::from("/zero-timeout"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], Some(0)),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/excessive-timeout"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], Some(3_601)),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/recursive-writer"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["bin/sotp", "phase", "enter"], None),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/recursive-pre-entry"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], None),
                        vec![raw_command(&["bin/sotp", "review", "local"], None)],
                    )],
                ),
            ),
            (
                PathBuf::from("/recursive-fix-local"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], None),
                        vec![raw_command(&["bin/sotp", "review", "fix-local"], None)],
                    )],
                ),
            ),
            (
                PathBuf::from("/persisted-host-writer"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer", "--host", "codex"], None),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/persisted-host-pre-entry"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], None),
                        vec![raw_command(&["pre-entry", "--host", "codex"], None)],
                    )],
                ),
            ),
        ]);

        assert!(matches!(
            loader.load(Path::new("/invalid-schema")),
            Err(crate::operator_command::CommandConfigLoadError::Invalid(
                CommandConfigValidationError::InvalidSchemaVersion { .. }
            ))
        ));
        assert!(matches!(
            loader.load(Path::new("/duplicate")),
            Err(crate::operator_command::CommandConfigLoadError::Invalid(
                CommandConfigValidationError::DuplicateDeclaration(_)
            ))
        ));
        assert!(matches!(
            loader.load(Path::new("/empty-argv")),
            Err(crate::operator_command::CommandConfigLoadError::Invalid(
                CommandConfigValidationError::EmptyArgv
            ))
        ));
        for root in ["/zero-timeout", "/excessive-timeout"] {
            assert!(matches!(
                loader.load(Path::new(root)),
                Err(crate::operator_command::CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::TimeoutOutOfRange { .. }
                ))
            ));
        }
        for root in ["/recursive-writer", "/recursive-pre-entry", "/recursive-fix-local"] {
            assert!(matches!(
                loader.load(Path::new(root)),
                Err(crate::operator_command::CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::RecursiveInvocation { .. }
                ))
            ));
        }
        for root in ["/persisted-host-writer", "/persisted-host-pre-entry"] {
            assert!(matches!(
                loader.load(Path::new(root)),
                Err(crate::operator_command::CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::PersistedHostArgument
                ))
            ));
        }
    }

    #[test]
    fn test_phase_command_config_loader_rejects_recursive_sotp_basename_variants() {
        let mut configurations = Vec::new();
        let mut roots = Vec::new();

        for (executable_index, executable) in
            ["./bin/sotp", "target/debug/sotp", "/opt/sotohe/target/debug/sotp", "SOTP.EXE"]
                .iter()
                .enumerate()
        {
            for (subcommand_index, subcommand) in
                [["phase", "enter"], ["review", "local"], ["review", "fix-local"]]
                    .iter()
                    .enumerate()
            {
                let root =
                    PathBuf::from(format!("/recursive-{executable_index}-{subcommand_index}"));
                configurations.push((
                    root.clone(),
                    raw_config(
                        1,
                        vec![raw_declaration(
                            "phase-one",
                            raw_command(&[executable, subcommand[0], subcommand[1]], None),
                            Vec::new(),
                        )],
                    ),
                ));
                roots.push(root);
            }
        }

        let loader = RootedValidatingLoader::new(configurations);
        for root in roots {
            assert!(matches!(
                loader.load(&root),
                Err(crate::operator_command::CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::RecursiveInvocation { .. }
                ))
            ));
        }
    }

    #[test]
    fn test_phase_command_config_loader_preserves_order_and_default_timeout_for_selected_root() {
        let loader = RootedValidatingLoader::new([
            (
                PathBuf::from("/first-repository"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["first-writer"], None),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/second-repository"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["second-writer"], None),
                        vec![
                            raw_command(&["first-pre-entry"], None),
                            raw_command(&["second-pre-entry"], Some(45)),
                        ],
                    )],
                ),
            ),
        ]);
        let phase_id = CommandDeclarationId::try_new("phase-one".to_owned()).unwrap();

        let loaded = loader.load(Path::new("/second-repository")).unwrap();

        let declaration = loaded.declaration(&phase_id).unwrap();

        assert_eq!(
            declaration.writer().argv().arguments().first().map(CommandArgument::as_str),
            Some("second-writer")
        );
        assert_eq!(declaration.writer().timeout().as_secs(), 3_600);
        assert_eq!(
            declaration
                .pre_entry_commands()
                .iter()
                .filter_map(|command| command.argv().arguments().first())
                .map(CommandArgument::as_str)
                .collect::<Vec<_>>(),
            vec!["first-pre-entry", "second-pre-entry"]
        );
        assert_eq!(
            declaration.pre_entry_commands().first().map(|command| command.timeout().as_secs()),
            Some(3_600)
        );
    }

    #[test]
    fn test_phase_validate_command_surfaces_loader_success_and_validation_failure() {
        let loader = RootedValidatingLoader::new([
            (
                PathBuf::from("/valid-repository"),
                raw_config(
                    1,
                    vec![raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new())],
                ),
            ),
            (
                PathBuf::from("/invalid-repository"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], Some(0)),
                        Vec::new(),
                    )],
                ),
            ),
        ]);
        let success = PhaseValidateCommand { repository_root: PathBuf::from("/valid-repository") };
        let failure =
            PhaseValidateCommand { repository_root: PathBuf::from("/invalid-repository") };

        assert!(loader.load(&success.repository_root).is_ok());
        let error = loader.load(&failure.repository_root).unwrap_err();
        assert!(matches!(
            error,
            crate::operator_command::CommandConfigLoadError::Invalid(
                CommandConfigValidationError::TimeoutOutOfRange { .. }
            )
        ));
        assert!(error.to_string().contains("outside the supported range"));
    }

    #[derive(Clone)]
    struct StaticLoader(PhaseCommandConfig);

    impl PhaseCommandConfigLoaderPort for StaticLoader {
        fn load(
            &self,
            _repository_root: &Path,
        ) -> Result<PhaseCommandConfig, CommandConfigLoadError> {
            Ok(self.0.clone())
        }
    }

    struct RecordingRunner {
        invocations: Mutex<Vec<ProgramInvocation>>,
    }

    impl RecordingRunner {
        fn invocations(&self) -> Vec<ProgramInvocation> {
            self.invocations.lock().unwrap().clone()
        }
    }

    impl ProgramRunnerPort for RecordingRunner {
        fn run(
            &self,
            invocation: ProgramInvocation,
        ) -> Result<ProgramRunOutcome, ProgramRunnerError> {
            let exit_code = invocation
                .argv
                .arguments()
                .first()
                .map(CommandArgument::as_str)
                .map_or(0, |name| if name == "fail" { 1 } else { 0 });
            self.invocations.lock().unwrap().push(invocation);
            Ok(ProgramRunOutcome::Exited {
                exit_code: ProgramExitCode::new(exit_code),
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            })
        }
    }

    struct FailingRunner;

    impl ProgramRunnerPort for FailingRunner {
        fn run(
            &self,
            _invocation: ProgramInvocation,
        ) -> Result<ProgramRunOutcome, ProgramRunnerError> {
            Err(ProgramRunnerError::SpawnFailed {
                message: domain::FreeText::new("runner unavailable".to_owned()),
            })
        }
    }

    struct TimedOutRunner {
        invocations: Mutex<Vec<ProgramInvocation>>,
    }

    impl TimedOutRunner {
        fn invocations(&self) -> Vec<ProgramInvocation> {
            self.invocations.lock().unwrap().clone()
        }
    }

    impl ProgramRunnerPort for TimedOutRunner {
        fn run(
            &self,
            invocation: ProgramInvocation,
        ) -> Result<ProgramRunOutcome, ProgramRunnerError> {
            self.invocations.lock().unwrap().push(invocation);
            Ok(ProgramRunOutcome::TimedOut {
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            })
        }
    }

    struct OutputLimitedRunner {
        invocations: Mutex<Vec<ProgramInvocation>>,
    }

    impl OutputLimitedRunner {
        fn invocations(&self) -> Vec<ProgramInvocation> {
            self.invocations.lock().unwrap().clone()
        }
    }

    impl ProgramRunnerPort for OutputLimitedRunner {
        fn run(
            &self,
            invocation: ProgramInvocation,
        ) -> Result<ProgramRunOutcome, ProgramRunnerError> {
            self.invocations.lock().unwrap().push(invocation);
            Ok(ProgramRunOutcome::OutputLimitExceeded {
                stream: ProgramOutputStream::Stderr,
                output: CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() },
            })
        }
    }

    fn command_argv(arguments: &[&str]) -> ConfiguredCommand {
        ConfiguredCommand::try_new(
            arguments
                .iter()
                .map(|argument| CommandArgument::try_new((*argument).to_owned()))
                .collect(),
            None,
        )
        .unwrap()
    }

    fn enter_service(
        writer: ConfiguredCommand,
        pre_entry_commands: Vec<ConfiguredCommand>,
    ) -> (PhaseCommandInteractor, Arc<RecordingRunner>) {
        let declaration = PhaseCommandDeclaration::new(
            CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            writer,
            pre_entry_commands,
        );
        let config =
            PhaseCommandConfig::try_new(CommandConfigSchemaVersion::new(1), vec![declaration])
                .unwrap();
        let runner = Arc::new(RecordingRunner { invocations: Mutex::new(Vec::new()) });
        (PhaseCommandInteractor::new(Arc::new(StaticLoader(config)), runner.clone()), runner)
    }

    fn enter_command(host: Option<ProviderName>) -> PhaseEnterCommand {
        PhaseEnterCommand {
            repository_root: PathBuf::from("/repository"),
            phase_id: CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            host,
        }
    }

    fn argv_values(invocation: &ProgramInvocation) -> Vec<&str> {
        invocation.argv.arguments().iter().map(CommandArgument::as_str).collect()
    }

    #[test]
    fn test_phase_command_enter_runs_pre_entry_commands_sequentially_at_repository_root() {
        let (service, runner) = enter_service(
            command_argv(&["writer"]),
            vec![command_argv(&["first"]), command_argv(&["second"])],
        );

        let outcome = service.enter(enter_command(None)).unwrap();

        match outcome {
            PhaseCommandEnterOutcome::Completed { pre_entry_records, writer_record } => {
                assert_eq!(pre_entry_records.len(), 2);
                assert_eq!(writer_record.as_ref().sequence_index.as_usize(), 2);
            }
            PhaseCommandEnterOutcome::Blocked { .. } => panic!("expected completed enter outcome"),
        }
        let invocations = runner.invocations();
        assert_eq!(
            invocations.iter().map(argv_values).collect::<Vec<_>>(),
            vec![vec!["first"], vec!["second"], vec!["writer"]]
        );
        assert!(
            invocations
                .iter()
                .all(|invocation| invocation.repository_root == Path::new("/repository"))
        );
    }

    #[test]
    fn test_phase_command_service_validate_returns_success_and_loader_error() {
        let (service, runner) = enter_service(command_argv(&["writer"]), Vec::new());

        assert!(
            service
                .validate(PhaseValidateCommand { repository_root: PathBuf::from("/repository") })
                .is_ok()
        );

        let failing_service = PhaseCommandInteractor::new(Arc::new(FailingLoader), runner);
        assert!(matches!(
            failing_service
                .validate(PhaseValidateCommand { repository_root: PathBuf::from("/repository") }),
            Err(CommandConfigLoadError::ReadFailed { .. })
        ));
    }

    #[test]
    fn test_phase_command_service_validate_rejects_invalid_configuration() {
        let loader = RootedValidatingLoader::new([(
            PathBuf::from("/invalid-repository"),
            raw_config(
                1,
                vec![raw_declaration("phase-one", raw_command(&["writer"], Some(0)), Vec::new())],
            ),
        )]);
        let runner = Arc::new(RecordingRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(Arc::new(loader), runner);

        assert!(matches!(
            service.validate(PhaseValidateCommand {
                repository_root: PathBuf::from("/invalid-repository"),
            }),
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::TimeoutOutOfRange { .. }
            ))
        ));
    }

    #[test]
    fn test_phase_command_service_validate_rejects_all_declared_invalid_configurations() {
        let loader = RootedValidatingLoader::new([
            (
                PathBuf::from("/invalid-schema"),
                raw_config(
                    2,
                    vec![raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new())],
                ),
            ),
            (
                PathBuf::from("/duplicate"),
                raw_config(
                    1,
                    vec![
                        raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new()),
                        raw_declaration("phase-one", raw_command(&["writer"], None), Vec::new()),
                    ],
                ),
            ),
            (
                PathBuf::from("/empty-argv"),
                raw_config(
                    1,
                    vec![raw_declaration("phase-one", raw_command(&[], None), Vec::new())],
                ),
            ),
            (
                PathBuf::from("/excessive-timeout"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], Some(3_601)),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/recursive-writer"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["bin/sotp", "phase", "enter"], None),
                        Vec::new(),
                    )],
                ),
            ),
            (
                PathBuf::from("/recursive-pre-entry"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], None),
                        vec![raw_command(&["bin/sotp", "review", "local"], None)],
                    )],
                ),
            ),
            (
                PathBuf::from("/recursive-fix-local"),
                raw_config(
                    1,
                    vec![raw_declaration(
                        "phase-one",
                        raw_command(&["writer"], None),
                        vec![raw_command(&["bin/sotp", "review", "fix-local"], None)],
                    )],
                ),
            ),
        ]);
        let runner = Arc::new(RecordingRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(Arc::new(loader), runner.clone());

        let validate = |root: &str| {
            service.validate(PhaseValidateCommand { repository_root: PathBuf::from(root) })
        };

        assert!(matches!(
            validate("/invalid-schema"),
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::InvalidSchemaVersion { .. }
            ))
        ));
        assert!(matches!(
            validate("/duplicate"),
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::DuplicateDeclaration(_)
            ))
        ));
        assert!(matches!(
            validate("/empty-argv"),
            Err(CommandConfigLoadError::Invalid(CommandConfigValidationError::EmptyArgv))
        ));
        assert!(matches!(
            validate("/excessive-timeout"),
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::TimeoutOutOfRange { .. }
            ))
        ));
        for root in ["/recursive-writer", "/recursive-pre-entry", "/recursive-fix-local"] {
            let result = validate(root);
            assert!(
                matches!(
                    result,
                    Err(CommandConfigLoadError::Invalid(
                        CommandConfigValidationError::RecursiveInvocation { .. }
                    ))
                ),
                "expected recursive invocation rejection for {root}, got {result:?}"
            );
        }
        assert!(runner.invocations().is_empty());
    }

    #[test]
    fn test_phase_command_service_validate_rejects_recursive_sotp_basename_variants_before_launch()
    {
        let mut configurations = Vec::new();
        let mut roots = Vec::new();

        for (executable_index, executable) in
            ["./bin/sotp", "target/debug/sotp", "/opt/sotohe/target/debug/sotp", "SOTP.EXE"]
                .iter()
                .enumerate()
        {
            for (subcommand_index, subcommand) in
                [["phase", "enter"], ["review", "local"], ["review", "fix-local"]]
                    .iter()
                    .enumerate()
            {
                let root = PathBuf::from(format!(
                    "/service-recursive-{executable_index}-{subcommand_index}"
                ));
                configurations.push((
                    root.clone(),
                    raw_config(
                        1,
                        vec![raw_declaration(
                            "phase-one",
                            raw_command(&[executable, subcommand[0], subcommand[1]], None),
                            Vec::new(),
                        )],
                    ),
                ));
                roots.push(root);
            }
        }

        let runner = Arc::new(RecordingRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(
            Arc::new(RootedValidatingLoader::new(configurations)),
            runner.clone(),
        );

        for root in roots {
            assert!(matches!(
                service.validate(PhaseValidateCommand { repository_root: root }),
                Err(CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::RecursiveInvocation { .. }
                ))
            ));
        }
        assert!(runner.invocations().is_empty());
    }

    #[test]
    fn test_phase_command_service_explain_returns_selected_declaration() {
        let (service, _) = enter_service(
            command_argv(&["bin/sotp", "capability", "exec", "implementer", "briefing.md"]),
            vec![
                command_argv(&["bin/sotp", "signal", "calc-impl-catalog"]),
                command_argv(&["bin/sotp", "task-contract", "check"]),
            ],
        );

        let explanation = service
            .explain(PhaseExplainQuery {
                repository_root: PathBuf::from("/repository"),
                phase_id: CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            })
            .unwrap();

        assert_eq!(explanation.phase_id.as_str(), "phase-one");
        assert_eq!(
            explanation
                .writer
                .argv()
                .arguments()
                .iter()
                .map(CommandArgument::as_str)
                .collect::<Vec<_>>(),
            ["bin/sotp", "capability", "exec", "implementer", "briefing.md"]
        );
        assert_eq!(
            explanation
                .pre_entry_commands
                .iter()
                .map(|command| {
                    command
                        .argv()
                        .arguments()
                        .iter()
                        .map(CommandArgument::as_str)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            [["bin/sotp", "signal", "calc-impl-catalog"], ["bin/sotp", "task-contract", "check"],]
        );
    }

    #[test]
    fn test_phase_command_enter_returns_config_error_without_starting_commands() {
        let runner = Arc::new(RecordingRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(Arc::new(FailingLoader), runner.clone());

        assert!(matches!(
            service.enter(enter_command(None)),
            Err(super::PhaseCommandEnterError::Config(CommandConfigLoadError::ReadFailed { .. }))
        ));
        assert!(runner.invocations().is_empty());
    }

    #[test]
    fn test_phase_command_enter_invalid_configuration_returns_auditable_error() {
        let loader = RootedValidatingLoader::new([(
            PathBuf::from("/invalid-repository"),
            raw_config(
                1,
                vec![raw_declaration("phase-one", raw_command(&["writer"], Some(0)), Vec::new())],
            ),
        )]);
        let runner = Arc::new(RecordingRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(Arc::new(loader), runner.clone());

        assert!(matches!(
            service.enter(PhaseEnterCommand {
                repository_root: PathBuf::from("/invalid-repository"),
                phase_id: CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
                host: None,
            }),
            Err(super::PhaseCommandEnterError::Config(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::TimeoutOutOfRange { .. }
            )))
        ));
        assert!(runner.invocations().is_empty());
    }

    #[test]
    fn test_phase_command_enter_returns_unknown_phase_error_without_running_commands() {
        let (service, runner) = enter_service(command("writer"), Vec::new());
        let unknown_phase = CommandDeclarationId::try_new("unknown-phase".to_owned()).unwrap();

        assert!(matches!(
            service.enter(PhaseEnterCommand {
                repository_root: PathBuf::from("/repository"),
                phase_id: unknown_phase.clone(),
                host: None,
            }),
            Err(super::PhaseCommandEnterError::UnknownPhase(phase_id)) if phase_id == unknown_phase
        ));
        assert!(runner.invocations().is_empty());
    }

    #[test]
    fn test_phase_command_enter_propagates_runner_failure() {
        let declaration = PhaseCommandDeclaration::new(
            CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            command("writer"),
            Vec::new(),
        );
        let config =
            PhaseCommandConfig::try_new(CommandConfigSchemaVersion::new(1), vec![declaration])
                .unwrap();
        let service =
            PhaseCommandInteractor::new(Arc::new(StaticLoader(config)), Arc::new(FailingRunner));

        assert!(matches!(
            service.enter(enter_command(None)),
            Err(super::PhaseCommandEnterError::Runner(ProgramRunnerError::SpawnFailed { .. }))
        ));
    }

    #[test]
    fn test_phase_command_enter_propagates_configured_timeouts_and_capture_limits() {
        let (service, runner) =
            enter_service(command("writer"), vec![command_with_timeout("pre-entry", 42)]);

        assert!(matches!(
            service.enter(enter_command(None)),
            Ok(PhaseCommandEnterOutcome::Completed { .. })
        ));

        let invocations = runner.invocations();
        let [pre_entry, writer] = invocations.as_slice() else {
            panic!("expected pre-entry and writer invocations");
        };
        assert_eq!(pre_entry.timeout.as_secs(), 42);
        assert_eq!(writer.timeout.as_secs(), 3_600);
        assert!(invocations.iter().all(|invocation| {
            invocation.stdout_limit.as_usize() == 1_048_576
                && invocation.stderr_limit.as_usize() == 1_048_576
        }));
    }

    #[test]
    fn test_phase_command_enter_returns_timed_out_command_as_blocked_outcome() {
        let declaration = PhaseCommandDeclaration::new(
            CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            command("writer"),
            Vec::new(),
        );
        let config =
            PhaseCommandConfig::try_new(CommandConfigSchemaVersion::new(1), vec![declaration])
                .unwrap();
        let runner = Arc::new(TimedOutRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(Arc::new(StaticLoader(config)), runner.clone());

        match service.enter(enter_command(None)).unwrap() {
            PhaseCommandEnterOutcome::Blocked { completed, failed } => {
                assert!(completed.is_empty());
                assert!(matches!(failed.as_ref().outcome, ProgramRunOutcome::TimedOut { .. }));
            }
            PhaseCommandEnterOutcome::Completed { .. } => panic!("expected blocked enter outcome"),
        }
        let invocations = runner.invocations();
        let [writer] = invocations.as_slice() else {
            panic!("expected one writer invocation");
        };
        assert_eq!(writer.timeout.as_secs(), 3_600);
    }

    #[test]
    fn test_phase_command_enter_returns_output_limited_pre_entry_as_blocked_outcome() {
        let declaration = PhaseCommandDeclaration::new(
            CommandDeclarationId::try_new("phase-one".to_owned()).unwrap(),
            command_argv(&["writer"]),
            vec![command_argv(&["limited-pre-entry"]), command_argv(&["later-pre-entry"])],
        );
        let config =
            PhaseCommandConfig::try_new(CommandConfigSchemaVersion::new(1), vec![declaration])
                .unwrap();
        let runner = Arc::new(OutputLimitedRunner { invocations: Mutex::new(Vec::new()) });
        let service = PhaseCommandInteractor::new(Arc::new(StaticLoader(config)), runner.clone());

        match service.enter(enter_command(None)).unwrap() {
            PhaseCommandEnterOutcome::Blocked { completed, failed } => {
                assert!(completed.is_empty());
                assert_eq!(failed.as_ref().sequence_index.as_usize(), 0);
                assert_eq!(
                    failed.as_ref().command.argv().arguments().first().map(CommandArgument::as_str),
                    Some("limited-pre-entry")
                );
                assert!(matches!(
                    failed.as_ref().outcome,
                    ProgramRunOutcome::OutputLimitExceeded {
                        stream: ProgramOutputStream::Stderr,
                        ..
                    }
                ));
            }
            PhaseCommandEnterOutcome::Completed { .. } => panic!("expected blocked enter outcome"),
        }
        assert_eq!(
            runner.invocations().iter().map(argv_values).collect::<Vec<_>>(),
            vec![vec!["limited-pre-entry"]]
        );
    }

    #[test]
    fn test_phase_command_enter_pre_entry_failure_stops_writer_and_remaining_commands() {
        let (service, runner) = enter_service(
            command_argv(&["writer"]),
            vec![command_argv(&["first"]), command_argv(&["fail"]), command_argv(&["later"])],
        );

        let outcome = service.enter(enter_command(None)).unwrap();

        match outcome {
            PhaseCommandEnterOutcome::Blocked { completed, failed } => {
                assert_eq!(completed.len(), 1);
                assert_eq!(failed.as_ref().sequence_index.as_usize(), 1);
                assert_eq!(
                    failed.as_ref().command.argv().arguments().first().map(CommandArgument::as_str),
                    Some("fail")
                );
                assert_eq!(
                    failed
                        .as_ref()
                        .invoked_argv
                        .arguments()
                        .iter()
                        .map(CommandArgument::as_str)
                        .collect::<Vec<_>>(),
                    vec!["fail"]
                );
                assert!(matches!(
                    failed.as_ref().outcome,
                    ProgramRunOutcome::Exited { ref exit_code, .. } if exit_code.as_i32() == 1
                ));
            }
            PhaseCommandEnterOutcome::Completed { .. } => panic!("expected blocked enter outcome"),
        }
        assert_eq!(
            runner.invocations().iter().map(argv_values).collect::<Vec<_>>(),
            vec![vec!["first"], vec!["fail"]]
        );
    }

    #[test]
    fn test_phase_command_enter_appends_supplied_host_only_to_capability_exec_writer() {
        for executable in ["sotp", "./bin/sotp", "target/debug/sotp"] {
            let writer = command_argv(&[executable, "capability", "exec", "implementer"]);
            let (service, runner) = enter_service(writer.clone(), Vec::new());

            let outcome = service
                .enter(enter_command(Some(ProviderName::try_new("codex").unwrap())))
                .unwrap();

            let invocations = runner.invocations();
            let [writer_invocation] = invocations.as_slice() else {
                panic!("expected one writer invocation");
            };
            assert_eq!(
                argv_values(writer_invocation),
                vec![executable, "capability", "exec", "implementer", "--host", "codex"]
            );
            match outcome {
                PhaseCommandEnterOutcome::Completed { writer_record, .. } => {
                    assert_eq!(writer_record.as_ref().command, writer);
                }
                PhaseCommandEnterOutcome::Blocked { .. } => {
                    panic!("expected completed enter outcome")
                }
            }
        }

        let writer = command_argv(&["writer", "capability", "exec", "implementer"]);
        let (service, runner) = enter_service(writer.clone(), Vec::new());
        let outcome =
            service.enter(enter_command(Some(ProviderName::try_new("codex").unwrap()))).unwrap();

        let invocations = runner.invocations();
        let [writer_invocation] = invocations.as_slice() else {
            panic!("expected one writer invocation");
        };
        assert_eq!(
            argv_values(writer_invocation),
            vec!["writer", "capability", "exec", "implementer"]
        );
        match outcome {
            PhaseCommandEnterOutcome::Completed { writer_record, .. } => {
                assert_eq!(writer_record.as_ref().command, writer);
                assert_eq!(writer_record.as_ref().invoked_argv, *writer.argv());
            }
            PhaseCommandEnterOutcome::Blocked { .. } => panic!("expected completed enter outcome"),
        }
    }

    #[test]
    fn test_phase_command_enter_omitted_host_preserves_writer_argv() {
        let writer = command_argv(&["bin/sotp", "capability", "exec", "implementer"]);
        let (service, runner) = enter_service(writer.clone(), Vec::new());

        let outcome = service.enter(enter_command(None)).unwrap();

        let invocations = runner.invocations();
        let [writer_invocation] = invocations.as_slice() else {
            panic!("expected one writer invocation");
        };
        assert_eq!(
            argv_values(writer_invocation),
            vec!["bin/sotp", "capability", "exec", "implementer"]
        );
        assert_eq!(
            writer.argv().arguments().iter().map(CommandArgument::as_str).collect::<Vec<_>>(),
            vec!["bin/sotp", "capability", "exec", "implementer"]
        );
        match outcome {
            PhaseCommandEnterOutcome::Completed { writer_record, .. } => {
                assert_eq!(writer_record.as_ref().invoked_argv, *writer.argv());
                assert_eq!(writer_record.as_ref().command, writer);
            }
            PhaseCommandEnterOutcome::Blocked { .. } => panic!("expected completed enter outcome"),
        }
    }
}
