//! Filesystem adapters and serde DTOs for operator-owned command configuration.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use domain::review_v2::{MainScopeName, ScopeName};
use domain::{FreeText, TrackId};
use serde::Deserialize;
use usecase::operator_command::{
    CommandArgument, CommandConfigLoadError, CommandConfigSchemaVersion,
    CommandConfigValidationError, CommandDeclarationId, ConfiguredCommand,
    UnvalidatedTimeoutSeconds,
};
use usecase::phase_command::{
    PhaseCommandConfig, PhaseCommandConfigLoaderPort, PhaseCommandDeclaration,
};
use usecase::pre_review_command::{
    CurrentReviewTrackResolveError, CurrentReviewTrackResolverPort, PreReviewCommandConfig,
    PreReviewCommandConfigLoaderPort, PreReviewScopeCommandDeclaration,
};

use crate::sanitized_failure::io_classification;
use crate::track::symlink_guard::reject_symlinks_below;

const PHASE_COMMANDS_CONFIG: &str = ".harness/config/phase-commands.json";
const PRE_REVIEW_GATES_CONFIG: &str = ".harness/config/pre-review-gates.json";
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_CURRENT_BRANCH_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CommandArgumentDto {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CommandArgvDto {
    pub arguments: Vec<CommandArgumentDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CommandTimeoutSecondsDto {
    pub seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CommandConfigSchemaVersionDto {
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct ReviewScopeNameDto {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredCommandDto {
    pub argv: CommandArgvDto,
    pub timeout_seconds: Option<CommandTimeoutSecondsDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CommandDeclarationIdDto {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseCommandDeclarationDto {
    pub id: CommandDeclarationIdDto,
    pub writer: ConfiguredCommandDto,
    pub pre_entry_commands: Vec<ConfiguredCommandDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseCommandConfigDto {
    pub schema_version: CommandConfigSchemaVersionDto,
    pub phases: Vec<PhaseCommandDeclarationDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreReviewScopeCommandDeclarationDto {
    pub scope: ReviewScopeNameDto,
    pub commands: Vec<ConfiguredCommandDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreReviewCommandConfigDto {
    pub schema_version: CommandConfigSchemaVersionDto,
    pub scopes: Vec<PreReviewScopeCommandDeclarationDto>,
}

#[derive(Debug, Deserialize)]
struct CommandConfigVersionEnvelope {
    schema_version: CommandConfigSchemaVersionDto,
}

/// Decodes a phase-command document into its usecase contract.
pub fn decode_phase_command_config(
    dto: PhaseCommandConfigDto,
) -> Result<PhaseCommandConfig, CommandConfigValidationError> {
    let declarations = dto
        .phases
        .into_iter()
        .map(|phase| {
            Ok(PhaseCommandDeclaration::new(
                CommandDeclarationId::try_new(phase.id.value)?,
                decode_command(phase.writer)?,
                phase
                    .pre_entry_commands
                    .into_iter()
                    .map(decode_command)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        })
        .collect::<Result<Vec<_>, CommandConfigValidationError>>()?;

    PhaseCommandConfig::try_new(
        CommandConfigSchemaVersion::new(dto.schema_version.version),
        declarations,
    )
    .map_err(|error| error.into_command_config_validation_error())
}

/// Decodes a pre-review-command document into its usecase contract.
pub fn decode_pre_review_command_config(
    dto: PreReviewCommandConfigDto,
) -> Result<PreReviewCommandConfig, CommandConfigValidationError> {
    let scopes = dto
        .scopes
        .into_iter()
        .map(|declaration| {
            Ok(PreReviewScopeCommandDeclaration::new(
                decode_scope(declaration.scope)?,
                declaration
                    .commands
                    .into_iter()
                    .map(decode_command)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        })
        .collect::<Result<Vec<_>, CommandConfigValidationError>>()?;
    PreReviewCommandConfig::try_new(
        CommandConfigSchemaVersion::new(dto.schema_version.version),
        scopes,
    )
    .map_err(CommandConfigValidationError::from)
}

fn decode_command(
    dto: ConfiguredCommandDto,
) -> Result<ConfiguredCommand, CommandConfigValidationError> {
    Ok(ConfiguredCommand::try_new(
        dto.argv
            .arguments
            .into_iter()
            .map(|argument| CommandArgument::try_new(argument.value))
            .collect(),
        dto.timeout_seconds.map(|timeout| UnvalidatedTimeoutSeconds::new(timeout.seconds)),
    )?)
}

fn decode_scope(dto: ReviewScopeNameDto) -> Result<ScopeName, CommandConfigValidationError> {
    if dto.value == "other" {
        return Ok(ScopeName::Other);
    }
    MainScopeName::new(dto.value.clone()).map(ScopeName::Main).map_err(|_| {
        CommandConfigValidationError::InvalidReviewScope { value: FreeText::new(dto.value) }
    })
}

/// Filesystem adapter for `phase-commands.json`.
#[derive(Debug, Default)]
pub struct FsPhaseCommandConfigLoader;

impl FsPhaseCommandConfigLoader {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PhaseCommandConfigLoaderPort for FsPhaseCommandConfigLoader {
    fn load(&self, repository_root: &Path) -> Result<PhaseCommandConfig, CommandConfigLoadError> {
        let source = read_config(repository_root, PHASE_COMMANDS_CONFIG)?;
        let version: CommandConfigVersionEnvelope =
            serde_json::from_str(&source).map_err(decode_failed)?;
        PhaseCommandConfig::try_new(
            CommandConfigSchemaVersion::new(version.schema_version.version),
            Vec::new(),
        )
        .map_err(|error| error.into_command_config_validation_error())
        .map_err(CommandConfigLoadError::from)?;
        let dto = serde_json::from_str(&source).map_err(decode_failed)?;
        decode_phase_command_config(dto).map_err(CommandConfigLoadError::from)
    }
}

/// Filesystem adapter for `pre-review-gates.json`.
#[derive(Debug, Default)]
pub struct FsPreReviewCommandConfigLoader;

impl FsPreReviewCommandConfigLoader {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PreReviewCommandConfigLoaderPort for FsPreReviewCommandConfigLoader {
    fn load(
        &self,
        repository_root: &Path,
        _track_id: &TrackId,
    ) -> Result<PreReviewCommandConfig, CommandConfigLoadError> {
        let source = read_config(repository_root, PRE_REVIEW_GATES_CONFIG)?;
        let version: CommandConfigVersionEnvelope =
            serde_json::from_str(&source).map_err(decode_failed)?;
        // Validate the version before decoding the version-specific strict DTO:
        // a future document can legitimately change its payload shape.
        PreReviewCommandConfig::try_new(
            CommandConfigSchemaVersion::new(version.schema_version.version),
            Vec::new(),
        )
        .map_err(CommandConfigValidationError::from)
        .map_err(CommandConfigLoadError::from)?;
        let dto = serde_json::from_str(&source).map_err(decode_failed)?;
        decode_pre_review_command_config(dto).map_err(CommandConfigLoadError::from)
    }
}

fn read_config(
    repository_root: &Path,
    relative_path: &str,
) -> Result<String, CommandConfigLoadError> {
    let root = repository_root.canonicalize().map_err(read_failed)?;
    if !root.is_dir() {
        return Err(read_failed(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "repository root is not a directory",
        )));
    }
    let path = root.join(relative_path);
    match reject_symlinks_below(&path, &root).map_err(read_failed)? {
        true => {}
        false => {
            return Err(read_failed(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "configuration file is not present",
            )));
        }
    }
    let metadata = fs::symlink_metadata(&path).map_err(read_failed)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CONFIG_BYTES {
        return Err(read_failed(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "configuration file is not a bounded regular file",
        )));
    }
    let mut source = String::new();
    fs::File::open(path)
        .map_err(read_failed)?
        .take(MAX_CONFIG_BYTES.saturating_add(1))
        .read_to_string(&mut source)
        .map_err(read_failed)?;
    if source.len() as u64 > MAX_CONFIG_BYTES {
        return Err(read_failed(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "configuration file exceeds its size limit",
        )));
    }
    Ok(source)
}

fn read_failed(error: std::io::Error) -> CommandConfigLoadError {
    CommandConfigLoadError::ReadFailed { message: FreeText::new(io_classification(&error)) }
}

fn decode_failed(_: serde_json::Error) -> CommandConfigLoadError {
    CommandConfigLoadError::DecodeFailed {
        message: FreeText::new("configuration is not valid JSON"),
    }
}

/// Repository-state adapter for resolving the current `track/<id>` branch.
#[derive(Debug, Default)]
pub struct GitCurrentReviewTrackResolver;

impl GitCurrentReviewTrackResolver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CurrentReviewTrackResolverPort for GitCurrentReviewTrackResolver {
    fn resolve(&self, repository_root: &Path) -> Result<TrackId, CurrentReviewTrackResolveError> {
        let root = repository_root.canonicalize().map_err(resolve_failed)?;
        let repo = crate::git_cli::SystemGitRepo::discover_from_isolated(&root)
            .map_err(|_| resolve_message("repository could not be discovered"))?;
        let discovered_root = repo.root().canonicalize().map_err(resolve_failed)?;
        if discovered_root != root {
            return Err(resolve_message("repository root does not match the requested root"));
        }
        // Discovery was isolated, so the branch read must use the same lane.
        // Calling `repo.current_branch()` here would re-inherit `GIT_DIR` and
        // related ambient selectors after we have proved containment.
        let output = crate::git_cli::isolated_bounded_git_output(
            &root,
            &["rev-parse", "--abbrev-ref", "HEAD"],
            MAX_CURRENT_BRANCH_BYTES,
        )
        .map_err(|_| resolve_message("current branch could not be read"))?;
        if !output.status.success() {
            return Err(resolve_message("current branch is unavailable"));
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if branch.is_empty() {
            return Err(resolve_message("current branch is unavailable"));
        }
        let track_id = branch
            .strip_prefix("track/")
            .ok_or_else(|| resolve_message("current branch is not a track branch"))?;
        TrackId::try_new(track_id).map_err(|_| resolve_message("track branch has an invalid id"))
    }
}

fn resolve_failed(error: std::io::Error) -> CurrentReviewTrackResolveError {
    resolve_message(io_classification(&error))
}

fn resolve_message(message: &str) -> CurrentReviewTrackResolveError {
    CurrentReviewTrackResolveError::ResolveFailed { message: FreeText::new(message) }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::process::Command;

    use super::*;

    fn command(values: &[&str], timeout: Option<u64>) -> ConfiguredCommandDto {
        ConfiguredCommandDto {
            argv: CommandArgvDto {
                arguments: values
                    .iter()
                    .map(|value| CommandArgumentDto { value: (*value).to_owned() })
                    .collect(),
            },
            timeout_seconds: timeout.map(|seconds| CommandTimeoutSecondsDto { seconds }),
        }
    }

    fn phase_declaration(
        id: &str,
        writer: ConfiguredCommandDto,
        pre_entry_commands: Vec<ConfiguredCommandDto>,
    ) -> PhaseCommandDeclarationDto {
        PhaseCommandDeclarationDto {
            id: CommandDeclarationIdDto { value: id.to_owned() },
            writer,
            pre_entry_commands,
        }
    }

    fn phase_config(phases: Vec<PhaseCommandDeclarationDto>) -> PhaseCommandConfigDto {
        PhaseCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 1 },
            phases,
        }
    }

    fn load_phase_config(source: &str) -> Result<PhaseCommandConfig, CommandConfigLoadError> {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("phase-commands.json"), source).unwrap();
        FsPhaseCommandConfigLoader::new().load(repo.path())
    }

    fn load_pre_review_config(
        source: &str,
    ) -> Result<PreReviewCommandConfig, CommandConfigLoadError> {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("pre-review-gates.json"), source).unwrap();
        FsPreReviewCommandConfigLoader::new()
            .load(repo.path(), &TrackId::try_new("example-track").unwrap())
    }

    fn phase_dto_with_pre_entry_command(command: &str) -> PhaseCommandConfigDto {
        serde_json::from_str(&format!(
            r#"{{
                "schema_version": 1,
                "phases": [{{
                    "id": "implementation",
                    "writer": {{"argv": ["bin/sotp"], "timeout_seconds": null}},
                    "pre_entry_commands": [{command}]
                }}]
            }}"#,
        ))
        .unwrap()
    }

    fn phase_dto_with_writer_command(command: &str) -> PhaseCommandConfigDto {
        serde_json::from_str(&format!(
            r#"{{
                "schema_version": 1,
                "phases": [{{
                    "id": "implementation",
                    "writer": {command},
                    "pre_entry_commands": []
                }}]
            }}"#,
        ))
        .unwrap()
    }

    #[test]
    fn test_decode_pre_review_config_uses_literal_argv_and_default_timeout() {
        let config = decode_pre_review_command_config(PreReviewCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 1 },
            scopes: vec![PreReviewScopeCommandDeclarationDto {
                scope: ReviewScopeNameDto { value: "implementation".to_owned() },
                commands: vec![command(&["bin/sotp", "signal", "check"], Some(12))],
            }],
        })
        .unwrap();
        let scope =
            decode_scope(ReviewScopeNameDto { value: "implementation".to_owned() }).unwrap();
        let commands = config.commands_for(&scope).unwrap();
        let argv: Vec<&str> =
            commands[0].argv().arguments().iter().map(|argument| argument.as_str()).collect();
        assert_eq!(argv, ["bin/sotp", "signal", "check"]);
        assert_eq!(commands[0].timeout().as_secs(), 12);
    }

    #[test]
    fn test_decode_pre_review_config_preserves_ordered_argv_and_timeout() {
        let config = decode_pre_review_command_config(PreReviewCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 1 },
            scopes: vec![PreReviewScopeCommandDeclarationDto {
                scope: ReviewScopeNameDto { value: "implementation".to_owned() },
                commands: vec![
                    command(&["bin/sotp", "signal", "calc-impl-catalog"], Some(45)),
                    command(
                        &["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"],
                        Some(3_600),
                    ),
                ],
            }],
        })
        .unwrap();
        let scope =
            decode_scope(ReviewScopeNameDto { value: "implementation".to_owned() }).unwrap();
        let commands = config.commands_for(&scope).unwrap();
        let argv: Vec<Vec<&str>> = commands
            .iter()
            .map(|configured| {
                configured.argv().arguments().iter().map(|argument| argument.as_str()).collect()
            })
            .collect();

        assert_eq!(
            argv,
            [
                vec!["bin/sotp", "signal", "calc-impl-catalog"],
                vec!["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit",],
            ]
        );
        assert_eq!(
            commands.iter().map(|configured| configured.timeout().as_secs()).collect::<Vec<_>>(),
            [45, 3_600]
        );
    }

    #[test]
    fn test_decode_phase_config_preserves_literal_argv_order_and_timeouts() {
        let config = decode_phase_command_config(PhaseCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 1 },
            phases: vec![PhaseCommandDeclarationDto {
                id: CommandDeclarationIdDto { value: "implementation".to_owned() },
                writer: command(&["bin/sotp", "capability", "exec", "implementer"], None),
                pre_entry_commands: vec![
                    command(&["bin/sotp", "signal", "calc-impl-catalog"], Some(12)),
                    command(
                        &["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"],
                        Some(3_600),
                    ),
                ],
            }],
        })
        .unwrap();

        let id = CommandDeclarationId::try_new("implementation".to_owned()).unwrap();
        let declaration = config.declaration(&id).unwrap();
        let writer: Vec<&str> = declaration
            .writer()
            .argv()
            .arguments()
            .iter()
            .map(|argument| argument.as_str())
            .collect();
        let pre_entry: Vec<Vec<&str>> = declaration
            .pre_entry_commands()
            .iter()
            .map(|configured| {
                configured.argv().arguments().iter().map(|argument| argument.as_str()).collect()
            })
            .collect();

        assert_eq!(writer, ["bin/sotp", "capability", "exec", "implementer"]);
        assert_eq!(declaration.writer().timeout().as_secs(), 3_600);
        assert_eq!(
            pre_entry,
            [
                vec!["bin/sotp", "signal", "calc-impl-catalog"],
                vec!["bin/sotp", "signal", "check-impl-catalog", "--gate", "commit"],
            ]
        );
        assert_eq!(
            declaration
                .pre_entry_commands()
                .iter()
                .map(|configured| configured.timeout().as_secs())
                .collect::<Vec<_>>(),
            [12, 3_600]
        );
    }

    #[test]
    fn test_decode_phase_config_rejects_invalid_schema_and_commands() {
        let unsupported_schema = PhaseCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 2 },
            phases: Vec::new(),
        };
        assert!(matches!(
            decode_phase_command_config(unsupported_schema),
            Err(CommandConfigValidationError::InvalidSchemaVersion { .. })
        ));

        for (writer, pre_entry_commands) in [
            (command(&[], None), Vec::new()),
            (command(&["bin/sotp", "phase", "enter"], None), Vec::new()),
            (command(&["bin/sotp", "review", "local"], None), Vec::new()),
            (command(&["bin/sotp", "review", "fix-local"], None), Vec::new()),
            (command(&["bin/sotp", "signal"], Some(0)), Vec::new()),
            (command(&["bin/sotp", "signal"], Some(3_601)), Vec::new()),
            (
                command(&["bin/sotp", "signal"], None),
                vec![command(&["bin/sotp", "phase", "enter"], None)],
            ),
        ] {
            let config = PhaseCommandConfigDto {
                schema_version: CommandConfigSchemaVersionDto { version: 1 },
                phases: vec![PhaseCommandDeclarationDto {
                    id: CommandDeclarationIdDto { value: "implementation".to_owned() },
                    writer,
                    pre_entry_commands,
                }],
            };
            assert!(decode_phase_command_config(config).is_err());
        }
    }

    #[test]
    fn test_decode_phase_config_rejects_duplicate_declarations_and_all_recursion_forms() {
        let duplicate = phase_config(vec![
            phase_declaration("implementation", command(&["bin/sotp", "signal"], None), Vec::new()),
            phase_declaration("implementation", command(&["bin/sotp", "signal"], None), Vec::new()),
        ]);
        assert!(matches!(
            decode_phase_command_config(duplicate),
            Err(CommandConfigValidationError::DuplicateDeclaration(_))
        ));

        for declaration in [
            phase_declaration(
                "implementation",
                command(&["bin/sotp", "phase", "enter"], None),
                Vec::new(),
            ),
            phase_declaration(
                "implementation",
                command(&["bin/sotp", "signal"], None),
                vec![command(&["bin/sotp", "review", "local"], None)],
            ),
            phase_declaration(
                "implementation",
                command(&["bin/sotp", "signal"], None),
                vec![command(&["bin/sotp", "review", "fix-local"], None)],
            ),
        ] {
            assert!(matches!(
                decode_phase_command_config(phase_config(vec![declaration])),
                Err(CommandConfigValidationError::RecursiveInvocation { .. })
            ));
        }
    }

    #[test]
    fn test_phase_config_dto_decode_rejects_unsupported_schema_document() {
        let dto: PhaseCommandConfigDto =
            serde_json::from_str(r#"{"schema_version":2,"phases":[]}"#).unwrap();

        assert!(matches!(
            decode_phase_command_config(dto),
            Err(CommandConfigValidationError::InvalidSchemaVersion { .. })
        ));
    }

    #[test]
    fn test_phase_declaration_dto_decode_rejects_invalid_declaration_id() {
        let dto: PhaseCommandConfigDto = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "",
                    "writer": {"argv": ["bin/sotp", "signal"], "timeout_seconds": null},
                    "pre_entry_commands": []
                }]
            }"#,
        )
        .unwrap();

        assert!(matches!(
            decode_phase_command_config(dto),
            Err(CommandConfigValidationError::InvalidDeclarationId { .. })
        ));
    }

    #[test]
    fn test_phase_declaration_dto_decode_rejects_duplicate_and_recursive_content() {
        let duplicate: PhaseCommandConfigDto = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "phases": [
                    {"id":"implementation","writer":{"argv":["bin/sotp"],"timeout_seconds":null},"pre_entry_commands":[]},
                    {"id":"implementation","writer":{"argv":["bin/sotp"],"timeout_seconds":null},"pre_entry_commands":[]}
                ]
            }"#,
        )
        .unwrap();
        assert!(matches!(
            decode_phase_command_config(duplicate),
            Err(CommandConfigValidationError::DuplicateDeclaration(_))
        ));

        let recursive_pre_entry: PhaseCommandConfigDto = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id":"implementation",
                    "writer":{"argv":["bin/sotp"],"timeout_seconds":null},
                    "pre_entry_commands":[{"argv":["bin/sotp","review","local"],"timeout_seconds":null}]
                }]
            }"#,
        )
        .unwrap();
        assert!(matches!(
            decode_phase_command_config(recursive_pre_entry),
            Err(CommandConfigValidationError::RecursiveInvocation { .. })
        ));
    }

    #[test]
    fn test_phase_declaration_dto_decode_rejects_invalid_pre_entry_commands() {
        let empty_argv =
            phase_dto_with_pre_entry_command(r#"{"argv": [], "timeout_seconds": null}"#);
        assert!(matches!(
            decode_phase_command_config(empty_argv),
            Err(CommandConfigValidationError::EmptyArgv)
        ));

        for timeout_seconds in [0, 3_601] {
            let timeout = phase_dto_with_pre_entry_command(&format!(
                r#"{{"argv": ["bin/sotp"], "timeout_seconds": {timeout_seconds}}}"#,
            ));
            assert!(matches!(
                decode_phase_command_config(timeout),
                Err(CommandConfigValidationError::TimeoutOutOfRange { .. })
            ));
        }

        for argv in [r#"["bin/sotp", "phase", "enter"]"#, r#"["bin/sotp", "review", "fix-local"]"#]
        {
            let recursive = phase_dto_with_pre_entry_command(&format!(
                r#"{{"argv": {argv}, "timeout_seconds": null}}"#,
            ));
            assert!(matches!(
                decode_phase_command_config(recursive),
                Err(CommandConfigValidationError::RecursiveInvocation { .. })
            ));
        }
    }

    #[test]
    fn test_phase_declaration_dto_decode_rejects_invalid_writer_argv_and_timeout() {
        let empty_writer_argv: PhaseCommandConfigDto = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id":"implementation",
                    "writer":{"argv":[],"timeout_seconds":null},
                    "pre_entry_commands":[]
                }]
            }"#,
        )
        .unwrap();
        assert!(matches!(
            decode_phase_command_config(empty_writer_argv),
            Err(CommandConfigValidationError::EmptyArgv)
        ));

        let out_of_range_writer_timeout: PhaseCommandConfigDto = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id":"implementation",
                    "writer":{"argv":["bin/sotp"],"timeout_seconds":3601},
                    "pre_entry_commands":[]
                }]
            }"#,
        )
        .unwrap();
        assert!(matches!(
            decode_phase_command_config(out_of_range_writer_timeout),
            Err(CommandConfigValidationError::TimeoutOutOfRange { .. })
        ));
    }

    #[test]
    fn test_phase_declaration_dto_decode_rejects_recursive_writer_prefixes() {
        for argv in [
            r#"["bin/sotp", "phase", "enter"]"#,
            r#"["bin/sotp", "review", "local"]"#,
            r#"["bin/sotp", "review", "fix-local"]"#,
        ] {
            let dto = phase_dto_with_writer_command(&format!(
                r#"{{"argv": {argv}, "timeout_seconds": null}}"#,
            ));
            assert!(matches!(
                decode_phase_command_config(dto),
                Err(CommandConfigValidationError::RecursiveInvocation { .. })
            ));
        }
    }

    #[test]
    fn test_decode_pre_review_config_rejects_unsupported_schema_version() {
        let config = PreReviewCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 2 },
            scopes: Vec::new(),
        };

        assert!(matches!(
            decode_pre_review_command_config(config),
            Err(CommandConfigValidationError::InvalidSchemaVersion { .. })
        ));
    }

    #[test]
    fn test_decode_rejects_duplicate_scope_empty_argv_timeout_and_recursion() {
        let duplicate = PreReviewCommandConfigDto {
            schema_version: CommandConfigSchemaVersionDto { version: 1 },
            scopes: vec![
                PreReviewScopeCommandDeclarationDto {
                    scope: ReviewScopeNameDto { value: "implementation".to_owned() },
                    commands: vec![],
                },
                PreReviewScopeCommandDeclarationDto {
                    scope: ReviewScopeNameDto { value: "implementation".to_owned() },
                    commands: vec![],
                },
            ],
        };
        assert!(matches!(
            decode_pre_review_command_config(duplicate),
            Err(CommandConfigValidationError::DuplicateScope(_))
        ));
        for invalid in [
            command(&[], None),
            command(&["bin/sotp", "phase", "enter"], None),
            command(&["bin/sotp", "review", "local"], None),
            command(&["bin/sotp", "review", "fix-local"], None),
            command(&["bin/sotp", "signal"], Some(0)),
            command(&["bin/sotp", "signal"], Some(3_601)),
        ] {
            let config = PreReviewCommandConfigDto {
                schema_version: CommandConfigSchemaVersionDto { version: 1 },
                scopes: vec![PreReviewScopeCommandDeclarationDto {
                    scope: ReviewScopeNameDto { value: "implementation".to_owned() },
                    commands: vec![invalid],
                }],
            };
            assert!(decode_pre_review_command_config(config).is_err());
        }
    }

    #[test]
    fn test_decode_pre_review_config_rejects_invalid_review_scope() {
        for scope in ["", "Other", "範囲"] {
            let dto = PreReviewCommandConfigDto {
                schema_version: CommandConfigSchemaVersionDto { version: 1 },
                scopes: vec![PreReviewScopeCommandDeclarationDto {
                    scope: ReviewScopeNameDto { value: scope.to_owned() },
                    commands: Vec::new(),
                }],
            };

            assert!(matches!(
                decode_pre_review_command_config(dto),
                Err(CommandConfigValidationError::InvalidReviewScope { .. })
            ));
        }
    }

    #[test]
    fn test_pre_review_loader_rejects_all_denylisted_prefixes_with_trailing_arguments() {
        for argv in [
            r#"["bin/sotp", "phase", "enter", "implementation"]"#,
            r#"["bin/sotp", "review", "local", "--scope", "infrastructure"]"#,
            r#"["bin/sotp", "review", "fix-local", "--scope", "infrastructure"]"#,
        ] {
            let result = load_pre_review_config(&format!(
                r#"{{
                    "schema_version": 1,
                    "scopes": [{{
                        "scope": "infrastructure",
                        "commands": [{{"argv": {argv}, "timeout_seconds": null}}]
                    }}]
                }}"#,
            ));

            assert!(matches!(
                result,
                Err(CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::RecursiveInvocation { .. }
                ))
            ));
        }
    }

    #[test]
    fn test_loaders_reject_malformed_and_symlinked_files() {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        let track_id = TrackId::try_new("example-track").unwrap();
        fs::write(config.join("pre-review-gates.json"), "{").unwrap();
        assert!(matches!(
            FsPreReviewCommandConfigLoader::new().load(repo.path(), &track_id),
            Err(CommandConfigLoadError::DecodeFailed { .. })
        ));
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::write(repo.path().join("outside.json"), "{}").unwrap();
            fs::remove_file(config.join("pre-review-gates.json")).unwrap();
            symlink(repo.path().join("outside.json"), config.join("pre-review-gates.json"))
                .unwrap();
            assert!(matches!(
                FsPreReviewCommandConfigLoader::new().load(repo.path(), &track_id),
                Err(CommandConfigLoadError::ReadFailed { .. })
            ));
        }
    }

    #[test]
    fn test_loader_rejects_unknown_schema_before_its_payload_shape() {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("pre-review-gates.json"),
            r#"{"schema_version":2,"scopes":"a future payload"}"#,
        )
        .unwrap();

        let result = FsPreReviewCommandConfigLoader::new()
            .load(repo.path(), &TrackId::try_new("example-track").unwrap());
        assert!(matches!(
            result,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::InvalidSchemaVersion { .. }
            ))
        ));
    }

    #[test]
    fn test_phase_loader_rejects_malformed_and_symlinked_files() {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("phase-commands.json"), "{").unwrap();
        assert!(matches!(
            FsPhaseCommandConfigLoader::new().load(repo.path()),
            Err(CommandConfigLoadError::DecodeFailed { .. })
        ));
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            fs::write(repo.path().join("outside.json"), "{}").unwrap();
            fs::remove_file(config.join("phase-commands.json")).unwrap();
            symlink(repo.path().join("outside.json"), config.join("phase-commands.json")).unwrap();
            assert!(matches!(
                FsPhaseCommandConfigLoader::new().load(repo.path()),
                Err(CommandConfigLoadError::ReadFailed { .. })
            ));
        }
    }

    #[test]
    fn test_phase_loader_rejects_unknown_schema_before_its_payload_shape() {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("phase-commands.json"),
            r#"{"schema_version":2,"phases":"a future payload"}"#,
        )
        .unwrap();

        let result = FsPhaseCommandConfigLoader::new().load(repo.path());
        assert!(matches!(
            result,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::InvalidSchemaVersion { .. }
            ))
        ));
    }

    #[test]
    fn test_phase_loader_rejects_invalid_declarations_before_returning_config() {
        let duplicate = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [
                    {"id":"implementation","writer":{"argv":["bin/sotp"],"timeout_seconds":null},"pre_entry_commands":[]},
                    {"id":"implementation","writer":{"argv":["bin/sotp"],"timeout_seconds":null},"pre_entry_commands":[]}
                ]
            }"#,
        );
        assert!(matches!(
            duplicate,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::DuplicateDeclaration(_)
            ))
        ));

        let empty_argv = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{"id":"implementation","writer":{"argv":[],"timeout_seconds":null},"pre_entry_commands":[]}]
            }"#,
        );
        assert!(matches!(
            empty_argv,
            Err(CommandConfigLoadError::Invalid(CommandConfigValidationError::EmptyArgv))
        ));

        let timeout = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{"id":"implementation","writer":{"argv":["bin/sotp"],"timeout_seconds":3601},"pre_entry_commands":[]}]
            }"#,
        );
        assert!(matches!(
            timeout,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::TimeoutOutOfRange { .. }
            ))
        ));

        let zero_timeout = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{"id":"implementation","writer":{"argv":["bin/sotp"],"timeout_seconds":0},"pre_entry_commands":[]}]
            }"#,
        );
        assert!(matches!(
            zero_timeout,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::TimeoutOutOfRange { .. }
            ))
        ));

        let recursive_writer = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{"id":"implementation","writer":{"argv":["bin/sotp","phase","enter"],"timeout_seconds":null},"pre_entry_commands":[]}]
            }"#,
        );
        assert!(matches!(
            recursive_writer,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::RecursiveInvocation { .. }
            ))
        ));

        let recursive_pre_entry = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id":"implementation",
                    "writer":{"argv":["bin/sotp"],"timeout_seconds":null},
                    "pre_entry_commands":[{"argv":["bin/sotp","review","local"],"timeout_seconds":null}]
                }]
            }"#,
        );
        assert!(matches!(
            recursive_pre_entry,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::RecursiveInvocation { .. }
            ))
        ));

        let recursive_review_fix_local = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id":"implementation",
                    "writer":{"argv":["bin/sotp","review","fix-local"],"timeout_seconds":null},
                    "pre_entry_commands":[]
                }]
            }"#,
        );
        assert!(matches!(
            recursive_review_fix_local,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::RecursiveInvocation { .. }
            ))
        ));
    }

    #[test]
    fn test_phase_loader_rejects_denylisted_prefix_with_trailing_arguments() {
        let result = load_phase_config(
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id":"implementation",
                    "writer":{
                        "argv":["bin/sotp","review","local","--scope","cli"],
                        "timeout_seconds":null
                    },
                    "pre_entry_commands":[]
                }]
            }"#,
        );

        assert!(matches!(
            result,
            Err(CommandConfigLoadError::Invalid(
                CommandConfigValidationError::RecursiveInvocation { .. }
            ))
        ));
    }

    #[test]
    fn test_phase_loader_decodes_valid_configuration_into_usecase_port() {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("phase-commands.json"),
            r#"{
                "schema_version": 1,
                "phases": [{
                    "id": "implementation",
                    "writer": {
                        "argv": ["bin/sotp", "capability", "exec", "implementer"],
                        "timeout_seconds": null
                    },
                    "pre_entry_commands": [
                        {"argv": ["bin/sotp", "signal", "calc-impl-catalog"], "timeout_seconds": 4},
                        {"argv": ["bin/sotp", "signal", "check-impl-catalog"], "timeout_seconds": 12}
                    ]
                }]
            }"#,
        )
        .unwrap();

        let config = FsPhaseCommandConfigLoader::new().load(repo.path()).unwrap();
        let id = CommandDeclarationId::try_new("implementation".to_owned()).unwrap();
        let declaration = config.declaration(&id).unwrap();
        let writer: Vec<&str> = declaration
            .writer()
            .argv()
            .arguments()
            .iter()
            .map(|argument| argument.as_str())
            .collect();
        let pre_entry: Vec<Vec<&str>> = declaration
            .pre_entry_commands()
            .iter()
            .map(|command| {
                command.argv().arguments().iter().map(|argument| argument.as_str()).collect()
            })
            .collect();

        assert_eq!(writer, ["bin/sotp", "capability", "exec", "implementer"]);
        assert_eq!(declaration.writer().timeout().as_secs(), 3_600);
        assert_eq!(
            pre_entry,
            [
                vec!["bin/sotp", "signal", "calc-impl-catalog"],
                vec!["bin/sotp", "signal", "check-impl-catalog"],
            ]
        );
        assert_eq!(
            declaration
                .pre_entry_commands()
                .iter()
                .map(|command| command.timeout().as_secs())
                .collect::<Vec<_>>(),
            [4, 12]
        );
    }

    #[test]
    fn test_loader_rejects_persisted_host_argument_as_invalid_config() {
        for argv in [r#"["bin/sotp", "--host", "codex"]"#, r#"["bin/sotp", "--host=codex"]"#] {
            let repo = tempfile::tempdir().unwrap();
            let config = repo.path().join(".harness/config");
            fs::create_dir_all(&config).unwrap();
            fs::write(
                config.join("pre-review-gates.json"),
                format!(
                    r#"{{
                        "schema_version": 1,
                        "scopes": [{{
                            "scope": "implementation",
                            "commands": [{{"argv": {argv}, "timeout_seconds": null}}]
                        }}]
                    }}"#
                ),
            )
            .unwrap();

            let result = FsPreReviewCommandConfigLoader::new()
                .load(repo.path(), &TrackId::try_new("example-track").unwrap());

            assert!(matches!(
                result,
                Err(CommandConfigLoadError::Invalid(
                    CommandConfigValidationError::PersistedHostArgument
                ))
            ));
        }
    }

    #[test]
    fn test_loaders_decode_valid_configuration_into_usecase_ports() {
        let repo = tempfile::tempdir().unwrap();
        let config = repo.path().join(".harness/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("pre-review-gates.json"),
            r#"{
                "schema_version": 1,
                "scopes": [{
                    "scope": "implementation",
                    "commands": [{"argv": ["bin/sotp"], "timeout_seconds": null}]
                }]
            }"#,
        )
        .unwrap();

        let track_id = TrackId::try_new("example-track").unwrap();
        let pre_review =
            FsPreReviewCommandConfigLoader::new().load(repo.path(), &track_id).unwrap();
        let commands = pre_review
            .commands_for(
                &decode_scope(ReviewScopeNameDto { value: "implementation".to_owned() }).unwrap(),
            )
            .unwrap();
        let argv: Vec<&str> =
            commands[0].argv().arguments().iter().map(|argument| argument.as_str()).collect();
        assert_eq!(argv, ["bin/sotp"]);
        assert_eq!(commands[0].timeout().as_secs(), 3_600);
    }

    #[test]
    fn test_current_track_resolver_accepts_track_branch_and_rejects_other_branch() {
        let repo = tempfile::tempdir().unwrap();
        for args in [
            ["init", "-q"].as_slice(),
            ["config", "user.email", "test@example.invalid"].as_slice(),
            ["config", "user.name", "test"].as_slice(),
            ["commit", "--allow-empty", "-qm", "initial"].as_slice(),
            ["checkout", "-qb", "track/example-track"].as_slice(),
        ] {
            assert!(
                Command::new("git").args(args).current_dir(repo.path()).status().unwrap().success()
            );
        }
        let resolver = GitCurrentReviewTrackResolver::new();
        assert_eq!(resolver.resolve(repo.path()).unwrap().as_ref(), "example-track");
        assert!(
            Command::new("git")
                .args(["checkout", "-qb", "ordinary"])
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(matches!(
            resolver.resolve(repo.path()),
            Err(CurrentReviewTrackResolveError::ResolveFailed { .. })
        ));
    }

    #[test]
    fn test_command_argv_dto_deserialization_preserves_argument_order() {
        let dto: CommandArgvDto =
            serde_json::from_str(r#"["bin/sotp","task-contract","check"]"#).unwrap();
        let values: Vec<&str> =
            dto.arguments.iter().map(|argument| argument.value.as_str()).collect();
        assert_eq!(values, ["bin/sotp", "task-contract", "check"]);
    }

    #[test]
    fn test_current_track_resolver_ignores_ambient_repository_selection() {
        let requested = track_repository("requested-track");
        let elsewhere = track_repository("elsewhere-track");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "operator_command_config::tests::test_current_track_resolver_ignores_ambient_repository_selection_subprocess",
            ])
            .env("GIT_DIR", elsewhere.path().join(".git"))
            .env("SOTP_TEST_REQUESTED_REPOSITORY", requested.path())
            .status()
            .unwrap();
        assert!(status.success(), "the isolated child test must pass");
    }

    #[test]
    fn test_current_track_resolver_ignores_ambient_repository_selection_subprocess() {
        let Some(requested) = std::env::var_os("SOTP_TEST_REQUESTED_REPOSITORY") else {
            return;
        };
        let resolved = GitCurrentReviewTrackResolver::new().resolve(Path::new(&requested));
        assert_eq!(resolved.unwrap().as_ref(), "requested-track");
    }

    fn track_repository(branch: &str) -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        for args in [
            ["init", "-q"].as_slice(),
            ["config", "user.email", "test@example.invalid"].as_slice(),
            ["config", "user.name", "test"].as_slice(),
            ["commit", "--allow-empty", "-qm", "initial"].as_slice(),
        ] {
            assert!(
                Command::new("git").args(args).current_dir(repo.path()).status().unwrap().success()
            );
        }
        assert!(
            Command::new("git")
                .args(["checkout", "-qb", &format!("track/{branch}")])
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success()
        );
        repo
    }
}
