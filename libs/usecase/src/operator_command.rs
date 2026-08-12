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

impl AsRef<str> for CommandArgument {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandArgv(Vec<CommandArgument>);

impl CommandArgv {
    pub fn try_new(arguments: Vec<CommandArgument>) -> Result<Self, CommandArgvValidationError> {
        if arguments.is_empty() {
            return Err(CommandArgvValidationError::Empty);
        }
        let argv = Self(arguments);
        if let Some(prefix) = argv.recursive_prefix() {
            return Err(CommandArgvValidationError::RecursiveInvocation { prefix });
        }
        Ok(argv)
    }
    #[must_use]
    pub fn arguments(&self) -> &[CommandArgument] {
        &self.0
    }
    /// Returns this argv with trailing arguments after revalidating its invariants.
    pub fn with_appended_arguments(
        &self,
        additional: impl IntoIterator<Item = CommandArgument>,
    ) -> Result<Self, CommandArgvValidationError> {
        let mut arguments = self.0.clone();
        arguments.extend(additional);
        Self::try_new(arguments)
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
/// the canonical command. The basename is compared case-insensitively and
/// accepts an optional `.exe` suffix so configuration remains safe on
/// case-insensitive Windows filesystems. A bare `sotp` is also rejected
/// because it can resolve to this process through `PATH`.
fn is_sotp_executable(value: &CommandArgument) -> bool {
    let basename = value.as_str().rsplit(['/', '\\']).next().unwrap_or_default();
    let normalized_basename = basename.to_ascii_lowercase();
    let stem = normalized_basename.strip_suffix(".exe").unwrap_or(&normalized_basename);

    stem == "sotp"
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
    ) -> Result<Self, CommandTimeoutValidationError> {
        if seconds.0 == 0 || seconds.0 > MAX_TIMEOUT_SECONDS {
            return Err(CommandTimeoutValidationError::OutOfRange { seconds });
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
    ) -> Result<Self, ConfiguredCommandValidationError> {
        let argv = CommandArgv::try_new(arguments)?;
        if argv.arguments().iter().any(|argument| {
            let value = argument.as_str();
            value == "--host" || value.starts_with("--host=")
        }) {
            return Err(ConfiguredCommandValidationError::PersistedHostArgument);
        }

        Ok(Self {
            argv,
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
    pub fn try_new(value: String) -> Result<Self, CommandDeclarationIdValidationError> {
        if value.trim().is_empty() {
            return Err(CommandDeclarationIdValidationError::Empty { value: FreeText::new(value) });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CommandDeclarationId {
    fn as_ref(&self) -> &str {
        self.as_str()
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
pub enum CommandArgvValidationError {
    #[error("configured argv must not be empty")]
    Empty,
    #[error("recursive command invocation is forbidden")]
    RecursiveInvocation { prefix: Vec<CommandArgument> },
}

#[derive(Debug, Error)]
pub enum CommandTimeoutValidationError {
    #[error("command timeout is outside the supported range: {}", seconds.as_u64())]
    OutOfRange { seconds: UnvalidatedTimeoutSeconds },
}

#[derive(Debug, Error)]
pub enum ConfiguredCommandValidationError {
    #[error(transparent)]
    Argv(#[from] CommandArgvValidationError),
    #[error(transparent)]
    Timeout(#[from] CommandTimeoutValidationError),
    #[error("persisted --host argument is forbidden")]
    PersistedHostArgument,
}

#[derive(Debug, Error)]
pub enum CommandDeclarationIdValidationError {
    #[error("invalid command declaration id: {value}")]
    Empty { value: FreeText },
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
    #[error("persisted --host argument is forbidden")]
    PersistedHostArgument,
}

impl From<ConfiguredCommandValidationError> for CommandConfigValidationError {
    fn from(error: ConfiguredCommandValidationError) -> Self {
        match error {
            ConfiguredCommandValidationError::Argv(CommandArgvValidationError::Empty) => {
                Self::EmptyArgv
            }
            ConfiguredCommandValidationError::Argv(
                CommandArgvValidationError::RecursiveInvocation { prefix },
            ) => Self::RecursiveInvocation { prefix },
            ConfiguredCommandValidationError::Timeout(
                CommandTimeoutValidationError::OutOfRange { seconds },
            ) => Self::TimeoutOutOfRange { seconds },
            ConfiguredCommandValidationError::PersistedHostArgument => Self::PersistedHostArgument,
        }
    }
}

impl From<CommandDeclarationIdValidationError> for CommandConfigValidationError {
    fn from(error: CommandDeclarationIdValidationError) -> Self {
        match error {
            CommandDeclarationIdValidationError::Empty { value } => {
                Self::InvalidDeclarationId { value }
            }
        }
    }
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
        CommandArgument, CommandArgv, CommandArgvValidationError, CommandConfigSchemaVersion,
        CommandConfigValidationError, CommandDeclarationId, CommandDeclarationIdValidationError,
        CommandTimeoutSeconds, CommandTimeoutValidationError, ConfiguredCommand,
        ConfiguredCommandValidationError, UnvalidatedTimeoutSeconds,
    };

    fn argv(values: &[&str]) -> Vec<CommandArgument> {
        values.iter().map(|value| CommandArgument::try_new((*value).to_owned())).collect()
    }

    #[test]
    fn test_command_argv_empty_arguments_is_rejected() {
        assert!(matches!(CommandArgv::try_new(Vec::new()), Err(CommandArgvValidationError::Empty)));
    }

    #[test]
    fn test_command_argv_empty_array_is_rejected() {
        let empty_argv: Vec<CommandArgument> = Vec::new();
        assert!(matches!(CommandArgv::try_new(empty_argv), Err(CommandArgvValidationError::Empty)));
    }

    #[test]
    fn test_command_argv_rejects_empty_and_recursive_sequences() {
        assert!(matches!(CommandArgv::try_new(Vec::new()), Err(CommandArgvValidationError::Empty)));

        for subcommand in [["phase", "enter"], ["review", "local"], ["review", "fix-local"]] {
            assert!(matches!(
                CommandArgv::try_new(argv(&["bin/sotp", subcommand[0], subcommand[1]])),
                Err(CommandArgvValidationError::RecursiveInvocation { .. })
            ));
        }
    }

    #[test]
    fn test_command_argument_preserves_opaque_literal_value() {
        let argument = CommandArgument::try_new("--scope=implementation/日本語".to_owned());

        assert_eq!(argument.as_str(), "--scope=implementation/日本語");
        assert_eq!(argument.as_ref(), "--scope=implementation/日本語");
    }

    #[test]
    fn test_configured_command_recursive_prefix_is_rejected() {
        assert!(matches!(
            ConfiguredCommand::try_new(argv(&["bin/sotp", "phase", "enter"]), None),
            Err(ConfiguredCommandValidationError::Argv(
                CommandArgvValidationError::RecursiveInvocation { .. }
            ))
        ));
    }

    #[test]
    fn test_configured_command_persisted_host_argument_forms_are_rejected_and_convert() {
        for arguments in [argv(&["writer", "--host", "codex"]), argv(&["writer", "--host=codex"])] {
            let error = ConfiguredCommand::try_new(arguments, None).unwrap_err();

            assert!(matches!(error, ConfiguredCommandValidationError::PersistedHostArgument));
            assert!(matches!(
                CommandConfigValidationError::from(error),
                CommandConfigValidationError::PersistedHostArgument
            ));
        }
    }

    #[test]
    fn test_configured_command_rejects_recursive_sotp_commands_by_normalized_basename() {
        for executable in [
            "./bin/sotp",
            "bin/./sotp",
            "bin/../bin/sotp",
            "sotp",
            "target/debug/sotp",
            "target/release/sotp",
            "/opt/sotohe/target/debug/sotp",
            "target/debug/sotp.exe",
            "SOTP",
            "Sotp.exe",
            "target\\debug\\SOTP.EXE",
        ] {
            for subcommand in [["phase", "enter"], ["review", "local"], ["review", "fix-local"]] {
                let error = ConfiguredCommand::try_new(
                    argv(&[executable, subcommand[0], subcommand[1]]),
                    None,
                )
                .unwrap_err();

                assert!(matches!(
                    error,
                    ConfiguredCommandValidationError::Argv(
                        CommandArgvValidationError::RecursiveInvocation { .. }
                    )
                ));
                assert!(matches!(
                    CommandConfigValidationError::from(error),
                    CommandConfigValidationError::RecursiveInvocation { .. }
                ));
            }
        }
    }

    #[test]
    fn test_configured_command_allows_non_sotp_binaries_for_recursive_subcommands() {
        for executable in ["target/debug/not-sotp", "target/debug/sotp-helper", "sotp-backup"] {
            for subcommand in [["phase", "enter"], ["review", "local"], ["review", "fix-local"]] {
                assert!(
                    ConfiguredCommand::try_new(
                        argv(&[executable, subcommand[0], subcommand[1]]),
                        None
                    )
                    .is_ok(),
                    "non-sotp executable {executable} must not be rejected"
                );
            }
        }
    }

    #[test]
    fn test_configured_command_allows_unicode_non_sotp_basenames_with_exe_suffix() {
        for executable in ["日本", "target/debug/日本.exe"] {
            assert!(
                ConfiguredCommand::try_new(argv(&[executable, "review", "local"]), None).is_ok(),
                "non-sotp executable {executable} must not be rejected"
            );
        }
    }

    #[test]
    fn test_command_argv_appends_arguments_without_changing_prefix() {
        let command_argv = CommandArgv::try_new(argv(&["bin/sotp", "signal", "calc"])).unwrap();
        let augmented =
            command_argv.with_appended_arguments(argv(&["--items-dir", "custom/items"])).unwrap();
        let values: Vec<&str> = augmented.arguments().iter().map(CommandArgument::as_str).collect();
        assert_eq!(values, ["bin/sotp", "signal", "calc", "--items-dir", "custom/items"]);
    }

    #[test]
    fn test_command_argv_rejects_appended_recursive_prefix() {
        for executable in ["bin/sotp", "target/debug/sotp"] {
            let command_argv = CommandArgv::try_new(argv(&[executable])).unwrap();
            assert!(matches!(
                command_argv.with_appended_arguments(argv(&["review", "local"])),
                Err(CommandArgvValidationError::RecursiveInvocation { .. })
            ));
        }
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
        assert!(matches!(zero, Err(CommandTimeoutValidationError::OutOfRange { .. })));
        match CommandTimeoutSeconds::try_new(UnvalidatedTimeoutSeconds::new(3_601)) {
            Err(CommandTimeoutValidationError::OutOfRange { seconds }) => {
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
    fn test_configured_command_preserves_supplied_literal_argv() {
        let command = ConfiguredCommand::try_new(
            argv(&["bin/sotp", "capability", "exec", "implementer", "briefing.md"]),
            None,
        )
        .unwrap();

        let actual: Vec<&str> =
            command.argv().arguments().iter().map(CommandArgument::as_str).collect();
        assert_eq!(actual, ["bin/sotp", "capability", "exec", "implementer", "briefing.md"]);
    }

    #[test]
    fn test_configured_command_rejects_empty_argv_and_preserves_explicit_timeout() {
        assert!(matches!(
            ConfiguredCommand::try_new(Vec::new(), None),
            Err(ConfiguredCommandValidationError::Argv(CommandArgvValidationError::Empty))
        ));

        let command = ConfiguredCommand::try_new(
            argv(&["bin/sotp", "signal", "calc"]),
            Some(UnvalidatedTimeoutSeconds::new(1_200)),
        )
        .unwrap();
        assert_eq!(command.timeout().as_secs(), 1_200);

        assert!(matches!(
            ConfiguredCommand::try_new(
                argv(&["bin/sotp", "signal", "calc"]),
                Some(UnvalidatedTimeoutSeconds::new(0)),
            ),
            Err(ConfiguredCommandValidationError::Timeout(
                CommandTimeoutValidationError::OutOfRange { .. }
            ))
        ));

        assert!(matches!(
            ConfiguredCommand::try_new(
                argv(&["bin/sotp", "signal", "calc"]),
                Some(UnvalidatedTimeoutSeconds::new(3_601)),
            ),
            Err(ConfiguredCommandValidationError::Timeout(
                CommandTimeoutValidationError::OutOfRange { .. }
            ))
        ));
    }

    #[test]
    fn test_declaration_id_empty_value_and_schema_version_validation_are_explicit() {
        assert!(matches!(
            CommandDeclarationId::try_new(" ".to_owned()),
            Err(CommandDeclarationIdValidationError::Empty { .. })
        ));
        let declaration_id = CommandDeclarationId::try_new("phase-1".to_owned()).unwrap();
        assert_eq!(declaration_id.as_ref(), "phase-1");
        assert!(CommandConfigSchemaVersion::new(1).validate().is_ok());
        assert!(matches!(
            CommandConfigSchemaVersion::new(2).validate(),
            Err(CommandConfigValidationError::InvalidSchemaVersion { .. })
        ));
    }
}
