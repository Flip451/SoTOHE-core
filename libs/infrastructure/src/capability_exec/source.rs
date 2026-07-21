//! Filesystem-backed common source adapter for generic capability dispatch.

use std::path::PathBuf;

use usecase::capability_exec::{
    BriefingText, CAPABILITY_EXEC_DISCIPLINE_PATH, CapabilityExecError, CapabilityFailureDetail,
    CapabilityFilePath, CapabilitySourcePort, DisciplineText,
};

use crate::capability_exec::{
    bounded_read_utf8_file,
    path_guard::{lexically_normalize, normalize_path_rejecting_symlinked_components},
};

/// Repository-relative canonical discipline template used for every dispatch.
pub const DISCIPLINE_TEMPLATE_PATH: &str = ".harness/prompts/capability-exec-discipline.md";

/// Loads briefing and discipline text from regular UTF-8 files under a repository root.
pub struct FsCapabilitySourceAdapter {
    repo_root: PathBuf,
}

impl FsCapabilitySourceAdapter {
    /// Creates an adapter rooted at `repo_root`.
    #[must_use]
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    fn resolve_path(
        &self,
        requested_path: &CapabilityFilePath,
    ) -> Result<PathBuf, CapabilityExecError> {
        let normalized_root = lexically_normalize(&self.repo_root);
        let path = self.repo_root.join(requested_path.as_path());
        let normalized_path = normalize_path_rejecting_symlinked_components(&path, &self.repo_root)
            .map_err(|error| {
                source_error(
                    requested_path.clone(),
                    format!("refusing to follow symlink at {}: {error}", path.display()),
                )
            })?;
        if !normalized_path.starts_with(&normalized_root) {
            return Err(source_error(
                requested_path.clone(),
                format!(
                    "path {} escapes repository root {}",
                    path.display(),
                    normalized_root.display()
                ),
            ));
        }
        let canonical_root = normalized_root.canonicalize().map_err(|error| {
            source_error(
                requested_path.clone(),
                format!(
                    "cannot canonicalize repository root {}: {error}",
                    self.repo_root.display()
                ),
            )
        })?;
        let canonical_path = normalized_path.canonicalize().map_err(|error| {
            source_error(
                requested_path.clone(),
                format!("cannot canonicalize {}: {error}", path.display()),
            )
        })?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(source_error(
                requested_path.clone(),
                format!(
                    "path {} escapes repository root {}",
                    path.display(),
                    canonical_root.display()
                ),
            ));
        }
        Ok(canonical_path)
    }

    fn load_text(&self, requested_path: CapabilityFilePath) -> Result<String, CapabilityExecError> {
        let absolute_path = self.resolve_path(&requested_path)?;
        let metadata = std::fs::metadata(&absolute_path).map_err(|error| {
            source_error(
                requested_path.clone(),
                format!("cannot inspect {}: {error}", absolute_path.display()),
            )
        })?;
        if !metadata.is_file() {
            return Err(source_error(
                requested_path,
                format!("{} is not a regular file", absolute_path.display()),
            ));
        }
        bounded_read_utf8_file(&absolute_path).map_err(|error| {
            source_error(
                requested_path,
                format!("cannot read UTF-8 text from {}: {error}", absolute_path.display()),
            )
        })
    }
}

impl CapabilitySourcePort for FsCapabilitySourceAdapter {
    fn load_briefing(
        &self,
        path: &CapabilityFilePath,
    ) -> Result<BriefingText, CapabilityExecError> {
        let content = self.load_text(path.clone())?;
        BriefingText::try_new(content).map_err(|error| {
            source_error(path.clone(), format!("invalid briefing content: {error}"))
        })
    }

    fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError> {
        let path = CAPABILITY_EXEC_DISCIPLINE_PATH.clone();
        let content = self.load_text(path.clone())?;
        DisciplineText::try_new(content)
            .map_err(|error| source_error(path, format!("invalid discipline content: {error}")))
    }
}

fn source_error(path: CapabilityFilePath, detail: impl Into<String>) -> CapabilityExecError {
    CapabilityExecError::SourceValidation { path, detail: CapabilityFailureDetail::new(detail) }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs::{self, File};

    use super::{DISCIPLINE_TEMPLATE_PATH, FsCapabilitySourceAdapter};
    use crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES;
    use usecase::capability_exec::{CapabilityExecError, CapabilityFilePath, CapabilitySourcePort};

    #[test]
    fn test_fs_capability_source_adapter_valid_files_load_as_validated_text() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let discipline = directory.path().join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(discipline.parent().expect("discipline has parent"))
            .expect("discipline directory is created");
        fs::write(&discipline, "Do not make direct git writes.").expect("discipline is written");
        fs::write(directory.path().join("briefing.md"), "Perform the task.")
            .expect("briefing is written");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        let briefing =
            CapabilityFilePath::try_new("briefing.md".into()).expect("valid briefing path");

        assert_eq!(
            adapter.load_briefing(&briefing).expect("briefing loads").as_str(),
            "Perform the task."
        );
        assert_eq!(
            adapter.load_discipline().expect("discipline loads").as_str(),
            "Do not make direct git writes."
        );
    }

    #[test]
    fn test_fs_capability_source_adapter_directory_briefing_rejected() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        let briefing = CapabilityFilePath::try_new(".".into()).expect("valid non-empty path");

        assert!(matches!(
            adapter.load_briefing(&briefing),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[test]
    fn test_fs_capability_source_adapter_missing_briefing_is_rejected() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        let briefing = CapabilityFilePath::try_new("missing.md".into()).expect("valid test path");

        assert!(matches!(
            adapter.load_briefing(&briefing),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[test]
    fn test_fs_capability_source_adapter_rejects_empty_and_non_utf8_briefings() {
        let directory = tempfile::tempdir().expect("test directory is created");
        fs::write(directory.path().join("empty.md"), " \n\t")
            .expect("empty briefing fixture is written");
        fs::write(directory.path().join("invalid-utf8.md"), [0xff, 0xfe])
            .expect("invalid UTF-8 fixture is written");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        let empty = CapabilityFilePath::try_new("empty.md".into()).expect("valid test path");
        let invalid_utf8 =
            CapabilityFilePath::try_new("invalid-utf8.md".into()).expect("valid test path");

        assert!(matches!(
            adapter.load_briefing(&empty),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
        assert!(matches!(
            adapter.load_briefing(&invalid_utf8),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[test]
    fn test_fs_capability_source_adapter_oversize_briefing_and_discipline_are_rejected() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let briefing = directory.path().join("briefing.md");
        File::create(&briefing)
            .expect("briefing fixture is created")
            .set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)
            .expect("briefing fixture size is set");
        let discipline = directory.path().join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(discipline.parent().expect("discipline has parent"))
            .expect("discipline directory is created");
        File::create(&discipline)
            .expect("discipline fixture is created")
            .set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)
            .expect("discipline fixture size is set");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        let briefing_path =
            CapabilityFilePath::try_new("briefing.md".into()).expect("valid briefing path");

        assert!(matches!(
            adapter.load_briefing(&briefing_path),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
        assert!(matches!(
            adapter.load_discipline(),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[test]
    fn test_fs_capability_source_adapter_rejects_empty_fixed_discipline_template() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let discipline = directory.path().join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(discipline.parent().expect("discipline has parent"))
            .expect("discipline directory is created");
        fs::write(discipline, "\n  \t").expect("empty discipline is written");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());

        assert!(matches!(
            adapter.load_discipline(),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[test]
    fn test_fs_capability_source_adapter_invalid_fixed_discipline_sources_are_rejected() {
        let missing = tempfile::tempdir().expect("missing fixture directory is created");
        let missing_adapter = FsCapabilitySourceAdapter::new(missing.path().to_owned());
        assert!(matches!(
            missing_adapter.load_discipline(),
            Err(CapabilityExecError::SourceValidation { .. })
        ));

        let directory = tempfile::tempdir().expect("directory fixture is created");
        let discipline_directory = directory.path().join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(&discipline_directory).expect("discipline directory fixture is created");
        let directory_adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        assert!(matches!(
            directory_adapter.load_discipline(),
            Err(CapabilityExecError::SourceValidation { .. })
        ));

        let invalid_utf8 = tempfile::tempdir().expect("invalid UTF-8 fixture is created");
        let discipline_file = invalid_utf8.path().join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(discipline_file.parent().expect("discipline has parent"))
            .expect("discipline parent directory is created");
        fs::write(discipline_file, [0xff, 0xfe]).expect("invalid UTF-8 discipline is written");
        let invalid_utf8_adapter = FsCapabilitySourceAdapter::new(invalid_utf8.path().to_owned());
        assert!(matches!(
            invalid_utf8_adapter.load_discipline(),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_capability_source_adapter_rejects_symlinked_briefing_and_discipline() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let outside = tempfile::tempdir().expect("outside directory is created");
        let external_briefing = outside.path().join("briefing.md");
        let external_discipline = outside.path().join("discipline.md");
        fs::write(&external_briefing, "Perform the task.").expect("external briefing is written");
        fs::write(&external_discipline, "Do not make direct git writes.")
            .expect("external discipline is written");
        std::os::unix::fs::symlink(&external_briefing, directory.path().join("briefing.md"))
            .expect("briefing symlink is created");
        let discipline = directory.path().join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(discipline.parent().expect("discipline has parent"))
            .expect("discipline directory is created");
        std::os::unix::fs::symlink(&external_discipline, &discipline)
            .expect("discipline symlink is created");
        let adapter = FsCapabilitySourceAdapter::new(directory.path().to_owned());
        let briefing =
            CapabilityFilePath::try_new("briefing.md".into()).expect("valid briefing path");

        assert!(matches!(
            adapter.load_briefing(&briefing),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
        assert!(matches!(
            adapter.load_discipline(),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_capability_source_adapter_rejects_symlinked_root_component_before_parent() {
        let workspace = tempfile::tempdir().expect("workspace fixture is created");
        let repository = workspace.path().join("repository");
        fs::create_dir_all(&repository).expect("repository fixture is created");
        fs::write(workspace.path().join("briefing.md"), "Perform the task.")
            .expect("external briefing is written");
        std::os::unix::fs::symlink(workspace.path(), repository.join("internal-link"))
            .expect("internal symlink is created");
        let adapter = FsCapabilitySourceAdapter::new(repository.join("internal-link/.."));
        let briefing =
            CapabilityFilePath::try_new("briefing.md".into()).expect("valid briefing path");

        assert!(matches!(
            adapter.load_briefing(&briefing),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_capability_source_adapter_allows_workspace_root_reached_through_symlink() {
        let workspace = tempfile::tempdir().expect("workspace directory is created");
        let repository = workspace.path().join("repository");
        fs::create_dir_all(&repository).expect("repository directory is created");
        fs::write(repository.join("briefing.md"), "Perform the task.")
            .expect("briefing is written");
        let discipline = repository.join(DISCIPLINE_TEMPLATE_PATH);
        fs::create_dir_all(discipline.parent().expect("discipline has parent"))
            .expect("discipline directory is created");
        fs::write(&discipline, "Do not make direct git writes.").expect("discipline is written");
        let workspace_link = workspace.path().join("workspace-link");
        std::os::unix::fs::symlink(&repository, &workspace_link)
            .expect("workspace symlink is created");
        let adapter = FsCapabilitySourceAdapter::new(workspace_link);
        let briefing =
            CapabilityFilePath::try_new("briefing.md".into()).expect("valid briefing path");

        assert_eq!(
            adapter.load_briefing(&briefing).expect("briefing loads").as_str(),
            "Perform the task."
        );
        assert_eq!(
            adapter.load_discipline().expect("discipline loads").as_str(),
            "Do not make direct git writes."
        );
    }
}
