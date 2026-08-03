//! Typed phase-command configuration and validation boundary.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::operator_command::{
    CommandConfigLoadError, CommandConfigSchemaVersion, CommandConfigValidationError,
    CommandDeclarationId, ConfiguredCommand,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::{
        PhaseCommandConfig, PhaseCommandConfigLoaderPort, PhaseCommandConfigValidationError,
        PhaseCommandDeclaration, PhaseValidateCommand,
    };
    use crate::operator_command::{
        CommandArgument, CommandConfigSchemaVersion, CommandConfigValidationError,
        CommandDeclarationId, ConfiguredCommand, UnvalidatedTimeoutSeconds,
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
        let recursive_writer = raw_config(
            1,
            vec![raw_declaration(
                "phase-one",
                raw_command(&["bin/sotp", "review", "fix-local"], None),
                Vec::new(),
            )],
        );
        let recursive_pre_entry = raw_config(
            1,
            vec![raw_declaration(
                "phase-one",
                raw_command(&["writer"], None),
                vec![raw_command(&["bin/sotp", "review", "fix-local"], None)],
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
}
