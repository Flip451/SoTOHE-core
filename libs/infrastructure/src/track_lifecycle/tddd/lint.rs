//! System adapter for the single-layer catalogue-lint port.

use std::path::Path;
use std::sync::Arc;

use domain::TrackId;
use domain::tddd::catalogue_linter::TypeRefPathExtractorPort;
use usecase::catalogue_lint_workflow::{
    RunCatalogueLint, RunCatalogueLintCommand, RunCatalogueLintError, RunCatalogueLintInteractor,
};
use usecase::track_lifecycle::tddd::lint::{
    TrackLintCommand, TrackLintError, TrackLintPort, TrackLintResult,
};

use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::fs_lint_config_loader::FsLintConfigLoader;
use crate::tddd::syn_primitive_occurrence_scanner::SynPrimitiveOccurrenceScanner;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::path_safety::lexical_normalize;

/// System-backed adapter for single-layer catalogue linting.
pub struct SystemTrackLintAdapter {
    extractor: Arc<dyn TypeRefPathExtractorPort>,
}

impl SystemTrackLintAdapter {
    /// Creates the system adapter with the parser-authoritative TypeRef extractor.
    #[must_use]
    pub fn new(extractor: Arc<dyn TypeRefPathExtractorPort>) -> Self {
        Self { extractor }
    }
}

impl TrackLintPort for SystemTrackLintAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackLintCommand,
    ) -> Result<TrackLintResult, TrackLintError> {
        let workspace_root = command.workspace_root.as_path();
        let config_path = resolve_config_path(workspace_root, command.rules_file.as_ref())
            .map_err(execution_failed)?;
        ensure_config_file(&config_path)?;

        let items_dir = workspace_root.join("track/items");
        let rules_path = workspace_root.join("architecture-rules.json");
        let loader = FsCatalogueLoader::new(items_dir, rules_path, workspace_root.to_path_buf());
        let config_loader = FsLintConfigLoader::new(config_path);
        let interactor = RunCatalogueLintInteractor::new(
            loader,
            config_loader,
            SynPrimitiveOccurrenceScanner,
            self.extractor.clone(),
        );
        let runner: &dyn RunCatalogueLint = &interactor;
        let violations = match runner.execute(RunCatalogueLintCommand {
            track_id: track_id.as_ref().to_owned(),
            layer_id: command.layer.as_ref().to_owned(),
            rules: Vec::new(),
        }) {
            Ok(violations) => violations,
            Err(RunCatalogueLintError::ConfigMissing { path }) => {
                return Err(execution_failed(lint_config_missing_message(&path)));
            }
            Err(error) => {
                return Err(execution_failed(format!("catalogue lint failed: {error}")));
            }
        };
        Ok(TrackLintResult { violations })
    }
}

pub(crate) fn resolve_config_path(
    workspace_root: &Path,
    rules_file: Option<&usecase::track_lifecycle::tddd::lint::TrackLintRulesFile>,
) -> Result<std::path::PathBuf, String> {
    let trusted_root = workspace_root.canonicalize().map_err(|error| {
        format!(
            "cannot canonicalize lint workspace trusted root '{}': {error}",
            workspace_root.display()
        )
    })?;
    let resolved = rules_file.map_or_else(
        || trusted_root.join(".harness/catalogue-lint/config.json"),
        |rules_file| {
            let path = rules_file.as_path();
            if path.is_absolute() { path.to_path_buf() } else { trusted_root.join(path) }
        },
    );
    let normalized = lexical_normalize(&resolved);
    if !normalized.starts_with(&trusted_root) {
        return Err(format!(
            "lint rules file is outside the workspace trusted root: {}",
            normalized.display()
        ));
    }
    reject_symlinks_below(&normalized, &trusted_root)
        .map_err(|error| format!("refusing to load a symlinked lint config: {error}"))?;
    match normalized.canonicalize() {
        Ok(contained) if !contained.starts_with(&trusted_root) => Err(format!(
            "lint rules file is outside the workspace trusted root: {}",
            normalized.display()
        )),
        Ok(_) => Ok(normalized),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(normalized),
        Err(error) => {
            Err(format!("cannot canonicalize lint rules file '{}': {error}", normalized.display()))
        }
    }
}

fn ensure_config_file(path: &Path) -> Result<(), TrackLintError> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(execution_failed(format!(
            "refusing to load a symlinked lint config: {}",
            path.display()
        ))),
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => {
            Err(execution_failed(format!("lint config is not a regular file: {}", path.display())))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(execution_failed(lint_config_missing_message(path)))
        }
        Err(error) => {
            Err(execution_failed(format!("cannot stat lint config '{}': {error}", path.display())))
        }
    }
}

fn lint_config_missing_message(path: &Path) -> String {
    format!(
        "lint config not found at {}. Copy `.harness/catalogue-lint/presets/ddd-strict.json` to that location to enable linting.",
        path.display()
    )
}

fn execution_failed(message: impl Into<String>) -> TrackLintError {
    TrackLintError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(message))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::tddd::type_ref_parser::SynTypeRefPathExtractorAdapter;
    use usecase::track_lifecycle::{TrackSelection, TrackWorkspaceRoot};

    fn command(root: &std::path::Path) -> TrackLintCommand {
        TrackLintCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("lint-track").expect("track id is valid"),
            ),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            layer: domain::tddd::LayerId::try_new("domain".to_owned()).expect("layer is valid"),
            rules_file: None,
        }
    }

    #[test]
    fn test_system_track_lint_adapter_missing_config_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let error = match SystemTrackLintAdapter::new(Arc::new(SynTypeRefPathExtractorAdapter))
            .execute(
                TrackId::try_new("lint-track").expect("track id is valid"),
                command(workspace.path()),
            ) {
            Ok(_) => panic!("missing config must fail closed"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("lint config not found"));
    }

    #[test]
    fn test_system_track_lint_adapter_directory_config_fails_closed() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        std::fs::create_dir_all(workspace.path().join(".harness/catalogue-lint/config.json"))
            .expect("directory occupies the lint config path");
        let error = match SystemTrackLintAdapter::new(Arc::new(SynTypeRefPathExtractorAdapter))
            .execute(
                TrackId::try_new("lint-track").expect("track id is valid"),
                command(workspace.path()),
            ) {
            Ok(_) => panic!("directory config must fail closed"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("lint config is not a regular file"),
            "directory config must fail closed: {error}"
        );
    }

    #[test]
    fn test_system_track_lint_adapter_symlink_config_fails_closed() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let config_dir = workspace.path().join(".harness/catalogue-lint");
        std::fs::create_dir_all(&config_dir).expect("lint config directory exists");
        let target = config_dir.join("target.json");
        std::fs::write(&target, r#"{"schema_version":1,"rules":[]}"#)
            .expect("symlink target is written");
        std::os::unix::fs::symlink(&target, config_dir.join("config.json"))
            .expect("lint config symlink is created");
        let error = match SystemTrackLintAdapter::new(Arc::new(SynTypeRefPathExtractorAdapter))
            .execute(
                TrackId::try_new("lint-track").expect("track id is valid"),
                command(workspace.path()),
            ) {
            Ok(_) => panic!("symlinked config must fail closed"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("refusing to load a symlinked lint config"),
            "symlinked config must fail closed: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_lint_adapter_symlinked_default_parent_fails_closed() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let outside = tempfile::tempdir().expect("outside config exists");
        let outside_harness = outside.path().join(".harness/catalogue-lint");
        std::fs::create_dir_all(&outside_harness).expect("outside lint directory exists");
        std::fs::write(outside_harness.join("config.json"), r#"{"schema_version":1,"rules":[]}"#)
            .expect("outside config is written");
        std::os::unix::fs::symlink(
            outside.path().join(".harness"),
            workspace.path().join(".harness"),
        )
        .expect("default config parent symlink is created");

        let error = resolve_config_path(workspace.path(), None).expect_err("symlink must fail");

        assert!(error.contains("symlink"), "symlinked parent must fail closed: {error}");
    }

    #[test]
    fn test_system_track_lint_adapter_rules_file_outside_workspace_fails_closed() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let outside = tempfile::tempdir().expect("outside config exists");
        let rules = outside.path().join("rules.json");
        std::fs::write(&rules, r#"{"schema_version":1,"rules":[]}"#)
            .expect("outside rules file is written");
        let mut command = command(workspace.path());
        command.rules_file = Some(
            usecase::track_lifecycle::tddd::lint::TrackLintRulesFile::try_new(rules)
                .expect("rules file is valid"),
        );
        let error = match SystemTrackLintAdapter::new(Arc::new(SynTypeRefPathExtractorAdapter))
            .execute(TrackId::try_new("lint-track").expect("track id is valid"), command)
        {
            Ok(_) => panic!("outside rules file must fail closed"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("outside the workspace trusted root"),
            "outside rules file must fail closed: {error}"
        );
    }
}
