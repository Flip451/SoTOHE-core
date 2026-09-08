//! Primary adapter for compatibility track-id resolution.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use usecase::track_lifecycle::resolution_compat::{
    TrackResolutionCommand, TrackResolutionCompatError, TrackResolutionResult,
    TrackResolutionService,
};
use usecase::track_lifecycle::{
    TrackItemsDirectory, TrackLifecycleIdInput, TrackSelection, TrackWorkspaceRoot,
};

pub use crate::adr_baseline::TrackIdInput;

/// User-safe diagnostic returned by the resolution driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackResolutionDiagnostic {
    message: String,
}

impl TrackResolutionDiagnostic {
    /// Returns the diagnostic text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl std::fmt::Display for TrackResolutionDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// Validated workspace-root input for the resolution driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackWorkspaceRootInput {
    value: PathBuf,
}

impl TrackWorkspaceRootInput {
    /// Validates, normalizes, and wraps a workspace root path.
    pub fn try_new(value: PathBuf) -> Result<Self, TrackResolutionDiagnostic> {
        TrackWorkspaceRoot::try_new(value.clone())
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))?;

        let absolute = if value.is_absolute() {
            value
        } else {
            std::env::current_dir()
                .map_err(|error| {
                    TrackResolutionDiagnostic::new(format!(
                        "cannot resolve workspace root from the current directory: {error}"
                    ))
                })?
                .join(value)
        };
        let metadata = std::fs::symlink_metadata(&absolute).map_err(|error| {
            TrackResolutionDiagnostic::new(format!(
                "cannot access workspace root '{}': {error}",
                absolute.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(TrackResolutionDiagnostic::new(format!(
                "symlink guard rejected workspace root '{}'",
                absolute.display()
            )));
        }
        if !metadata.is_dir() {
            return Err(TrackResolutionDiagnostic::new(format!(
                "workspace root '{}' is not a directory",
                absolute.display()
            )));
        }
        let normalized = std::fs::canonicalize(&absolute).map_err(|error| {
            TrackResolutionDiagnostic::new(format!(
                "cannot canonicalize workspace root '{}': {error}",
                absolute.display()
            ))
        })?;

        Ok(Self { value: normalize_windows_verbatim_path(&normalized) })
    }

    /// Consumes the input and returns its path.
    #[must_use]
    pub fn into_path(self) -> PathBuf {
        self.value
    }
}

/// Validated `track/items` input shared by the resolution and TDDD drivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackItemsDirectoryInput {
    value: PathBuf,
}

impl TrackItemsDirectoryInput {
    /// Validates and wraps a `track/items` path.
    pub fn try_new(value: PathBuf) -> Result<Self, TrackResolutionDiagnostic> {
        TrackItemsDirectory::try_new(value.clone()).map_err(|error| {
            if error.to_string().contains("must end in 'track/items'") {
                TrackResolutionDiagnostic::new(format!(
                    "--items-dir must point to '<project-root>/track/items'; got {}",
                    value.display()
                ))
            } else {
                TrackResolutionDiagnostic::new(error.to_string())
            }
        })?;
        Ok(Self { value })
    }

    /// Derives and normalizes the workspace-root input from this items directory.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when the derived workspace root is unavailable,
    /// inaccessible, or cannot be normalized.
    pub fn workspace_root(&self) -> Result<TrackWorkspaceRootInput, TrackResolutionDiagnostic> {
        let root = self
            .value
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| PathBuf::from("."));
        TrackWorkspaceRootInput::try_new(root)
    }

    pub(crate) fn into_usecase(self) -> Result<TrackItemsDirectory, TrackResolutionDiagnostic> {
        TrackItemsDirectory::try_new(self.value)
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))
    }

    #[cfg(test)]
    pub(crate) fn from_test_value(value: PathBuf) -> Self {
        Self { value }
    }
}

impl TrackWorkspaceRootInput {
    pub(crate) fn into_usecase(self) -> Result<TrackWorkspaceRoot, TrackResolutionDiagnostic> {
        TrackWorkspaceRoot::try_new(self.value)
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))
    }
}

/// Typed input to the compatibility resolution driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackResolutionInput {
    /// Read using a `track/items` anchor.
    ReadFromItems { track_id: Option<TrackIdInput>, items_dir: TrackItemsDirectoryInput },
    /// Read using a workspace-root anchor.
    ReadFromRoot { track_id: Option<TrackIdInput>, workspace_root: TrackWorkspaceRootInput },
    /// Write using a `track/items` anchor and branch guard.
    WriteFromItems { track_id: Option<TrackIdInput>, items_dir: TrackItemsDirectoryInput },
    /// Write using a workspace-root anchor and branch guard.
    WriteFromRoot { track_id: Option<TrackIdInput>, workspace_root: TrackWorkspaceRootInput },
    /// Detect the active track, if any.
    DetectActive { workspace_root: TrackWorkspaceRootInput },
}

/// Result of compatibility resolution at the primary-adapter boundary.
#[derive(Debug, PartialEq, Eq)]
pub enum TrackResolutionOutcome {
    /// A validated track id was resolved.
    Resolved(TrackIdInput),
    /// No active track exists for an active-detection request.
    Inactive,
    /// Resolution failed with a user-safe diagnostic.
    Failed(TrackResolutionDiagnostic),
}

/// Primary adapter for compatibility track-id resolution.
pub struct TrackResolutionDriver {
    service: Arc<dyn TrackResolutionService>,
}

impl std::fmt::Debug for TrackResolutionDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("TrackResolutionDriver").finish_non_exhaustive()
    }
}

impl TrackResolutionDriver {
    /// Creates a driver over an injected resolution service.
    #[must_use]
    pub fn new(service: Arc<dyn TrackResolutionService>) -> Self {
        Self { service }
    }

    /// Validates the delivery input, executes the usecase, and maps its result.
    pub fn resolve(&self, input: TrackResolutionInput) -> TrackResolutionOutcome {
        let command = match input_to_command(input) {
            Ok(command) => command,
            Err(error) => return TrackResolutionOutcome::Failed(error),
        };
        match self.service.execute(command) {
            Ok(TrackResolutionResult::Resolved(track_id)) => match track_id.to_string().parse() {
                Ok(track_id) => TrackResolutionOutcome::Resolved(track_id),
                Err(error) => TrackResolutionOutcome::Failed(TrackResolutionDiagnostic::new(
                    format!("invalid track id: {error}"),
                )),
            },
            Ok(TrackResolutionResult::Inactive) => TrackResolutionOutcome::Inactive,
            Err(error) => TrackResolutionOutcome::Failed(diagnostic_from_error(error)),
        }
    }
}

fn input_to_command(
    input: TrackResolutionInput,
) -> Result<TrackResolutionCommand, TrackResolutionDiagnostic> {
    match input {
        TrackResolutionInput::ReadFromItems { track_id, items_dir } => {
            Ok(TrackResolutionCommand::ReadFromItems {
                track: selection_from_input(track_id)?,
                items_dir: items_dir.into_usecase()?,
            })
        }
        TrackResolutionInput::ReadFromRoot { track_id, workspace_root } => {
            Ok(TrackResolutionCommand::ReadFromRoot {
                track: selection_from_input(track_id)?,
                workspace_root: workspace_root.into_usecase()?,
            })
        }
        TrackResolutionInput::WriteFromItems { track_id, items_dir } => {
            Ok(TrackResolutionCommand::WriteFromItems {
                track: selection_from_input(track_id)?,
                items_dir: items_dir.into_usecase()?,
            })
        }
        TrackResolutionInput::WriteFromRoot { track_id, workspace_root } => {
            Ok(TrackResolutionCommand::WriteFromRoot {
                track: selection_from_input(track_id)?,
                workspace_root: workspace_root.into_usecase()?,
            })
        }
        TrackResolutionInput::DetectActive { workspace_root } => {
            Ok(TrackResolutionCommand::DetectActive {
                workspace_root: workspace_root.into_usecase()?,
            })
        }
    }
}

fn selection_from_input(
    track_id: Option<TrackIdInput>,
) -> Result<TrackSelection, TrackResolutionDiagnostic> {
    let validated = track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))?;
    Ok(TrackSelection::from_input(validated))
}

fn diagnostic_from_error(error: TrackResolutionCompatError) -> TrackResolutionDiagnostic {
    TrackResolutionDiagnostic::new(error.to_string())
}

fn normalize_windows_verbatim_path(path: &Path) -> PathBuf {
    let encoded_path = path.as_os_str().as_encoded_bytes();
    let Some(non_verbatim_path) = encoded_path.strip_prefix(br"\\?\") else {
        return path.to_path_buf();
    };
    if let Some(unc_path) = non_verbatim_path.strip_prefix(br"UNC\") {
        let mut normalized = Vec::with_capacity(2 + unc_path.len());
        normalized.extend_from_slice(br"\\");
        normalized.extend_from_slice(unc_path);
        return PathBuf::from(os_string_from_encoded_bytes(normalized));
    }
    let is_drive_path = match non_verbatim_path {
        [drive, b':', b'\\', ..] => drive.is_ascii_alphabetic(),
        _ => false,
    };
    if is_drive_path {
        PathBuf::from(os_string_from_encoded_bytes(non_verbatim_path.to_vec()))
    } else {
        path.to_path_buf()
    }
}

fn os_string_from_encoded_bytes(bytes: Vec<u8>) -> OsString {
    // SAFETY:
    // - bytes is composed of bytes copied from OsStr::as_encoded_bytes.
    // - Any newly inserted bytes are ASCII, which is valid in the OS encoding.
    // - Prefix removal and insertion happen only at ASCII boundaries.
    unsafe { OsString::from_encoded_bytes_unchecked(bytes) }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
pub(crate) fn canonicalized_path(path: impl AsRef<Path>) -> PathBuf {
    let canonicalized = std::fs::canonicalize(path).unwrap();
    normalize_windows_verbatim_path(&canonicalized)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct StubResolutionService {
        result: Mutex<Option<Result<TrackResolutionResult, TrackResolutionCompatError>>>,
        commands: Mutex<Vec<TrackResolutionCommand>>,
    }

    impl StubResolutionService {
        fn new(result: Result<TrackResolutionResult, TrackResolutionCompatError>) -> Self {
            Self { result: Mutex::new(Some(result)), commands: Mutex::new(Vec::new()) }
        }
    }

    impl TrackResolutionService for StubResolutionService {
        fn execute(
            &self,
            command: TrackResolutionCommand,
        ) -> Result<TrackResolutionResult, TrackResolutionCompatError> {
            self.commands.lock().unwrap().push(command);
            match self.result.lock().unwrap().as_ref() {
                Some(Ok(TrackResolutionResult::Resolved(track_id))) => {
                    Ok(TrackResolutionResult::Resolved(track_id.clone()))
                }
                Some(Ok(TrackResolutionResult::Inactive)) => Ok(TrackResolutionResult::Inactive),
                Some(Err(error)) => Err(clone_compat_error(error)),
                None => panic!("stub resolution service has no configured result"),
            }
        }
    }

    fn clone_compat_error(error: &TrackResolutionCompatError) -> TrackResolutionCompatError {
        match error {
            TrackResolutionCompatError::Unavailable(diagnostic) => {
                TrackResolutionCompatError::Unavailable(usecase::git_workflow::DiagnosticText::new(
                    diagnostic.to_string(),
                ))
            }
        }
    }

    fn items_dir() -> TrackItemsDirectoryInput {
        TrackItemsDirectoryInput::try_new("track/items".into()).unwrap()
    }

    fn explicit_id() -> TrackIdInput {
        "fixture-track".parse().unwrap()
    }

    #[test]
    fn test_normalize_windows_verbatim_path_strips_drive_prefix() {
        assert_eq!(
            normalize_windows_verbatim_path(Path::new(r"\\?\C:\repo")),
            PathBuf::from(r"C:\repo")
        );
    }

    #[test]
    fn test_normalize_windows_verbatim_path_converts_unc_prefix() {
        let path = PathBuf::from(r"\\?\UNC\server\share");

        assert_eq!(normalize_windows_verbatim_path(&path), PathBuf::from(r"\\server\share"));
    }

    #[cfg(unix)]
    #[test]
    fn test_normalize_windows_verbatim_path_converts_non_unicode_unc_prefix() {
        use std::os::unix::ffi::OsStringExt as _;

        let path = PathBuf::from(OsString::from_vec(b"\\\\?\\UNC\\server\\share\\\xff".to_vec()));
        let expected = PathBuf::from(OsString::from_vec(b"\\\\server\\share\\\xff".to_vec()));

        assert_eq!(normalize_windows_verbatim_path(&path), expected);
    }

    #[test]
    fn test_normalize_windows_verbatim_path_preserves_non_drive_namespace() {
        let path = PathBuf::from(r"\\?\Volume{fixture-guid}\repo");

        assert_eq!(normalize_windows_verbatim_path(&path), path);
    }

    #[test]
    fn test_track_workspace_root_input_normalizes_default_relative_path() {
        let current_dir = std::env::current_dir().unwrap();
        let input = TrackWorkspaceRootInput::try_new(PathBuf::from(".")).unwrap();

        assert!(input.value.is_absolute());
        assert_eq!(input.into_path(), canonicalized_path(current_dir));
    }

    #[test]
    fn test_track_workspace_root_input_normalizes_explicit_absolute_path() {
        let current_dir = std::env::current_dir().unwrap();
        let input = TrackWorkspaceRootInput::try_new(current_dir.clone()).unwrap();

        assert_eq!(input.into_path(), current_dir);
    }

    #[test]
    fn test_track_workspace_root_input_rejects_missing_root_before_processing() {
        let missing = std::env::current_dir()
            .unwrap()
            .join("this-workspace-root-does-not-exist-for-track-resolution-tests");

        let result = TrackWorkspaceRootInput::try_new(missing);

        assert!(result.is_err());
    }

    #[test]
    fn test_track_workspace_root_input_rejects_non_directory_root() {
        let result = TrackWorkspaceRootInput::try_new(PathBuf::from("Cargo.toml"));

        assert!(result.is_err());
    }

    #[test]
    fn test_track_items_directory_input_rejects_noncanonical_path() {
        let result = TrackItemsDirectoryInput::try_new("fixture/items".into());

        assert_eq!(
            result.unwrap_err().message(),
            "--items-dir must point to '<project-root>/track/items'; got fixture/items"
        );
    }

    #[test]
    fn test_track_items_directory_input_normalizes_relative_workspace_root() {
        let expected = canonicalized_path(std::env::current_dir().unwrap());
        let items_dir = TrackItemsDirectoryInput::try_new(PathBuf::from("track/items")).unwrap();

        let workspace_root = items_dir.workspace_root().unwrap();

        assert_eq!(workspace_root.into_path(), expected);
    }

    #[test]
    fn test_track_items_directory_input_derives_non_default_workspace_root() {
        let items_dir =
            TrackItemsDirectoryInput::try_new(PathBuf::from("src/track/items")).unwrap();
        let expected = canonicalized_path(PathBuf::from("src"));

        let workspace_root = items_dir.workspace_root().unwrap();

        assert_eq!(workspace_root.into_path(), expected);
    }

    #[test]
    fn test_track_items_directory_input_rejects_missing_derived_workspace_root() {
        let items_dir = TrackItemsDirectoryInput::try_new(PathBuf::from(
            "missing-workspace-root-for-derived-items-test/track/items",
        ))
        .unwrap();

        let result = items_dir.workspace_root();

        assert!(result.is_err());
    }

    #[test]
    fn test_track_resolution_driver_maps_resolved_result() {
        let service = Arc::new(StubResolutionService::new(Ok(TrackResolutionResult::Resolved(
            usecase::TrackId::try_new("fixture-track").unwrap(),
        ))));
        let driver = TrackResolutionDriver::new(service.clone());

        let result = driver.resolve(TrackResolutionInput::ReadFromItems {
            track_id: Some(explicit_id()),
            items_dir: items_dir(),
        });

        assert!(
            matches!(result, TrackResolutionOutcome::Resolved(id) if id.to_string() == "fixture-track")
        );
        assert_eq!(service.commands.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_track_resolution_driver_maps_inactive_result() {
        let driver = TrackResolutionDriver::new(Arc::new(StubResolutionService::new(Ok(
            TrackResolutionResult::Inactive,
        ))));
        let workspace_root = TrackWorkspaceRootInput::try_new(".".into()).unwrap();

        let result = driver.resolve(TrackResolutionInput::DetectActive { workspace_root });

        assert_eq!(result, TrackResolutionOutcome::Inactive);
    }

    #[test]
    fn test_track_resolution_driver_maps_service_error() {
        let driver = TrackResolutionDriver::new(Arc::new(StubResolutionService::new(Err(
            TrackResolutionCompatError::Unavailable(usecase::git_workflow::DiagnosticText::new(
                "git unavailable",
            )),
        ))));

        let result = driver.resolve(TrackResolutionInput::ReadFromItems {
            track_id: None,
            items_dir: items_dir(),
        });

        assert!(matches!(
            result,
            TrackResolutionOutcome::Failed(diagnostic) if diagnostic.message() == "git unavailable"
        ));
    }

    #[test]
    fn test_track_resolution_input_variants_route_matching_commands() {
        let service = Arc::new(StubResolutionService::new(Ok(TrackResolutionResult::Inactive)));
        let driver = TrackResolutionDriver::new(service.clone());
        let workspace_root = TrackWorkspaceRootInput::try_new(".".into()).unwrap();

        for input in [
            TrackResolutionInput::ReadFromItems {
                track_id: Some(explicit_id()),
                items_dir: items_dir(),
            },
            TrackResolutionInput::ReadFromRoot {
                track_id: Some(explicit_id()),
                workspace_root: workspace_root.clone(),
            },
            TrackResolutionInput::WriteFromItems {
                track_id: Some(explicit_id()),
                items_dir: items_dir(),
            },
            TrackResolutionInput::WriteFromRoot {
                track_id: Some(explicit_id()),
                workspace_root: workspace_root.clone(),
            },
            TrackResolutionInput::DetectActive { workspace_root },
        ] {
            assert_eq!(driver.resolve(input), TrackResolutionOutcome::Inactive);
        }

        let commands = service.commands.lock().unwrap();
        assert!(matches!(commands.first(), Some(TrackResolutionCommand::ReadFromItems { .. })));
        assert!(matches!(commands.get(1), Some(TrackResolutionCommand::ReadFromRoot { .. })));
        assert!(matches!(commands.get(2), Some(TrackResolutionCommand::WriteFromItems { .. })));
        assert!(matches!(commands.get(3), Some(TrackResolutionCommand::WriteFromRoot { .. })));
        assert!(matches!(commands.get(4), Some(TrackResolutionCommand::DetectActive { .. })));
    }

    #[test]
    fn test_track_resolution_driver_preserves_invalid_items_dir_cli_diagnostic() {
        let expected = "--items-dir must point to '<project-root>/track/items'; got fixture/items";
        let items_dir = TrackItemsDirectoryInput::try_new("fixture/items".into()).unwrap_err();
        assert_eq!(items_dir.message(), expected);
    }
}
