//! Typed, operator-owned command declarations.

use domain::FreeText;
use domain::review_v2::ScopeName;
use thiserror::Error;

const CONFIG_SCHEMA_VERSION: u32 = 1;
const MAX_TIMEOUT_SECONDS: u64 = 3_600;
const ONE_MEBIBYTE: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandArgument(String);

impl CommandArgument {
    #[must_use]
    pub fn try_new(value: String) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandArgv(Vec<CommandArgument>);

impl CommandArgv {
    pub fn try_new(arguments: Vec<CommandArgument>) -> Result<Self, CommandConfigValidationError> {
        if arguments.is_empty() {
            return Err(CommandConfigValidationError::EmptyArgv);
        }
        let argv = Self(arguments);
        if let Some(prefix) = argv.recursive_prefix() {
            return Err(CommandConfigValidationError::RecursiveInvocation { prefix });
        }
        Ok(argv)
    }
    #[must_use]
    pub fn arguments(&self) -> &[CommandArgument] {
        &self.0
    }
    /// Returns this already-validated argv with additional trailing arguments.
    ///
    /// Appending cannot change the executable or its first two subcommand
    /// tokens, so it preserves the non-empty and recursive-invocation
    /// invariants established by [`Self::try_new`].
    #[must_use]
    pub fn with_appended_arguments(
        &self,
        additional: impl IntoIterator<Item = CommandArgument>,
    ) -> Self {
        let mut arguments = self.0.clone();
        arguments.extend(additional);
        Self(arguments)
    }
    fn recursive_prefix(&self) -> Option<Vec<CommandArgument>> {
        const DENYLIST: [[&str; 3]; 3] = [
            ["bin/sotp", "phase", "enter"],
            ["bin/sotp", "review", "local"],
            ["bin/sotp", "review", "fix-local"],
        ];
        let prefix = self.0.get(..3)?;
        (is_sotp_executable(prefix.first()?)
            && DENYLIST.iter().any(|denied| {
                prefix.iter().skip(1).map(CommandArgument::as_str).eq(denied[1..].iter().copied())
            }))
        .then(|| prefix.to_vec())
    }
}

/// Detect a `sotp` executable path without resolving it against the filesystem.
///
/// Configuration is untrusted input, so lexical aliases such as `./bin/sotp`
/// and `bin/./sotp` must receive the same recursive-invocation protection as
/// the canonical command. A bare `sotp` is also rejected because it can
/// resolve to this process through `PATH`.
fn is_sotp_executable(value: &CommandArgument) -> bool {
    let mut components = Vec::new();
    for component in std::path::Path::new(value.as_str()).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if matches!(components.last(), Some(std::path::Component::Normal(_))) {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }
    let names: Vec<&str> = components
        .iter()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    names.as_slice() == ["sotp"] || names.ends_with(&["bin", "sotp"])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnvalidatedTimeoutSeconds(u64);
impl UnvalidatedTimeoutSeconds {
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTimeoutSeconds(u64);
impl CommandTimeoutSeconds {
    pub fn try_new(
        seconds: UnvalidatedTimeoutSeconds,
    ) -> Result<Self, CommandConfigValidationError> {
        if seconds.0 == 0 || seconds.0 > MAX_TIMEOUT_SECONDS {
            return Err(CommandConfigValidationError::TimeoutOutOfRange { seconds });
        }
        Ok(Self(seconds.0))
    }
    #[must_use]
    pub fn default_max() -> Self {
        Self(MAX_TIMEOUT_SECONDS)
    }
    #[must_use]
    pub fn as_secs(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputCaptureLimitBytes(usize);
impl OutputCaptureLimitBytes {
    #[must_use]
    pub fn one_mebibyte() -> Self {
        Self(ONE_MEBIBYTE)
    }
    #[must_use]
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredCommand {
    argv: CommandArgv,
    timeout: CommandTimeoutSeconds,
}
impl ConfiguredCommand {
    pub fn try_new(
        arguments: Vec<CommandArgument>,
        timeout_seconds: Option<UnvalidatedTimeoutSeconds>,
    ) -> Result<Self, CommandConfigValidationError> {
        Ok(Self {
            argv: CommandArgv::try_new(arguments)?,
            timeout: match timeout_seconds {
                Some(value) => CommandTimeoutSeconds::try_new(value)?,
                None => CommandTimeoutSeconds::default_max(),
            },
        })
    }
    #[must_use]
    pub fn argv(&self) -> &CommandArgv {
        &self.argv
    }
    #[must_use]
    pub fn timeout(&self) -> CommandTimeoutSeconds {
        self.timeout
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDeclarationId(String);
impl CommandDeclarationId {
    pub fn try_new(value: String) -> Result<Self, CommandConfigValidationError> {
        if value.trim().is_empty() {
            return Err(CommandConfigValidationError::InvalidDeclarationId {
                value: FreeText::new(value),
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandConfigSchemaVersion(u32);
impl CommandConfigSchemaVersion {
    #[must_use]
    pub fn new(value: u32) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_u32(&self) -> u32 {
        self.0
    }
    pub(crate) fn validate(&self) -> Result<(), CommandConfigValidationError> {
        if self.0 == CONFIG_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(CommandConfigValidationError::InvalidSchemaVersion { actual: self.clone() })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSequenceIndex(usize);
impl CommandSequenceIndex {
    #[must_use]
    pub fn new(value: usize) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Error)]
pub enum CommandConfigValidationError {
    #[error("unsupported command configuration schema version: {actual:?}")]
    InvalidSchemaVersion { actual: CommandConfigSchemaVersion },
    #[error("invalid command declaration id: {value}")]
    InvalidDeclarationId { value: FreeText },
    #[error("invalid review scope: {value}")]
    InvalidReviewScope { value: FreeText },
    #[error("duplicate command declaration: {}", .0.as_str())]
    DuplicateDeclaration(CommandDeclarationId),
    #[error("duplicate review scope: {0}")]
    DuplicateScope(ScopeName),
    #[error("configured argv must not be empty")]
    EmptyArgv,
    #[error("command timeout is outside the supported range: {}", seconds.as_u64())]
    TimeoutOutOfRange { seconds: UnvalidatedTimeoutSeconds },
    #[error("recursive command invocation is forbidden")]
    RecursiveInvocation { prefix: Vec<CommandArgument> },
}

#[derive(Debug, Error)]
pub enum CommandConfigLoadError {
    #[error("command configuration could not be read: {message}")]
    ReadFailed { message: FreeText },
    #[error("command configuration could not be decoded: {message}")]
    DecodeFailed { message: FreeText },
    #[error(transparent)]
    Invalid(#[from] CommandConfigValidationError),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        CommandArgument, CommandArgv, CommandConfigSchemaVersion, CommandConfigValidationError,
        CommandDeclarationId, CommandTimeoutSeconds, ConfiguredCommand, UnvalidatedTimeoutSeconds,
    };

    fn argv(values: &[&str]) -> Vec<CommandArgument> {
        values.iter().map(|value| CommandArgument::try_new((*value).to_owned())).collect()
    }

    #[test]
    fn test_command_argv_empty_arguments_is_rejected() {
        assert!(matches!(
            CommandArgv::try_new(Vec::new()),
            Err(CommandConfigValidationError::EmptyArgv)
        ));
    }

    #[test]
    fn test_command_argv_empty_array_is_rejected() {
        let empty_argv: Vec<CommandArgument> = Vec::new();
        assert!(matches!(
            CommandArgv::try_new(empty_argv),
            Err(CommandConfigValidationError::EmptyArgv)
        ));
    }

    #[test]
    fn test_command_argument_preserves_empty_literal_and_argv_order() {
        let empty = CommandArgument::try_new(String::new());
        assert_eq!(empty.as_str(), "");
        let command_argv = CommandArgv::try_new(argv(&["bin/sotp", "signal", "check"])).unwrap();
        let values: Vec<&str> =
            command_argv.arguments().iter().map(CommandArgument::as_str).collect();
        assert_eq!(values, ["bin/sotp", "signal", "check"]);
    }

    #[test]
    fn test_configured_command_recursive_prefix_is_rejected() {
        assert!(matches!(
            ConfiguredCommand::try_new(argv(&["bin/sotp", "phase", "enter"]), None),
            Err(CommandConfigValidationError::RecursiveInvocation { .. })
        ));
    }

    #[test]
    fn test_configured_command_rejects_lexical_sotp_aliases() {
        for executable in ["./bin/sotp", "bin/./sotp", "bin/../bin/sotp", "sotp"] {
            assert!(matches!(
                ConfiguredCommand::try_new(argv(&[executable, "review", "local"]), None),
                Err(CommandConfigValidationError::RecursiveInvocation { .. })
            ));
        }
    }

    #[test]
    fn test_command_argv_appends_arguments_without_changing_prefix() {
        let command_argv = CommandArgv::try_new(argv(&["bin/sotp", "signal", "calc"])).unwrap();
        let augmented =
            command_argv.with_appended_arguments(argv(&["--items-dir", "custom/items"]));
        let values: Vec<&str> = augmented.arguments().iter().map(CommandArgument::as_str).collect();
        assert_eq!(values, ["bin/sotp", "signal", "calc", "--items-dir", "custom/items"]);
    }

    #[test]
    fn test_command_timeout_bounds_and_error_value_are_preserved() {
        assert_eq!(
            CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(3_600))
                .unwrap()
                .as_secs(),
            3_600
        );
        let zero = CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(0));
        assert!(matches!(zero, Err(CommandConfigValidationError::TimeoutOutOfRange { .. })));
        match CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(3_601)) {
            Err(CommandConfigValidationError::TimeoutOutOfRange { seconds }) => {
                assert_eq!(seconds.as_u64(), 3_601);
            }
            other => panic!("expected an out-of-range timeout error, got {other:?}"),
        }
    }

    #[test]
    fn test_configured_command_omitted_timeout_uses_one_hour_default() {
        let command =
            ConfiguredCommand::try_new(argv(&["bin/sotp", "signal", "calc"]), None).unwrap();
        assert_eq!(command.timeout().as_secs(), 3_600);
    }

    #[test]
    fn test_declaration_id_empty_value_and_schema_version_validation_are_explicit() {
        assert!(matches!(
            CommandDeclarationId::try_new(" ".to_owned()),
            Err(CommandConfigValidationError::InvalidDeclarationId { .. })
        ));
        assert!(CommandConfigSchemaVersion::new(1).validate().is_ok());
        assert!(matches!(
            CommandConfigSchemaVersion::new(2).validate(),
            Err(CommandConfigValidationError::InvalidSchemaVersion { .. })
        ));
    }
}
