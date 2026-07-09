//! Filesystem adapter for loading parsed `spec.json` documents.

use std::path::{Path, PathBuf};

use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::{SpecDocument, SpecDocumentLoadError, SpecDocumentLoaderPort};

use crate::spec::codec::{self, SpecCodecError};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::path_safety::lexical_normalize;

/// Filesystem adapter implementing [`SpecDocumentLoaderPort`].
#[derive(Debug, Clone)]
pub struct FsSpecDocumentLoader {
    trusted_root: PathBuf,
}

impl FsSpecDocumentLoader {
    /// Builds a filesystem spec document loader rooted at `trusted_root`.
    #[must_use]
    pub fn new(trusted_root: PathBuf) -> Self {
        Self { trusted_root }
    }
}

impl SpecDocumentLoaderPort for FsSpecDocumentLoader {
    fn load(&self, spec_path: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        let guarded_path = self.guard_spec_path(spec_path)?;

        let text = match std::fs::read_to_string(&guarded_path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(SpecDocumentLoadError::NotFound { path: spec_path.to_path_buf() });
            }
            Err(source) => {
                return Err(SpecDocumentLoadError::Io {
                    path: spec_path.to_path_buf(),
                    reason: diagnostic(&source.to_string()),
                });
            }
        };

        codec::decode(&text).map_err(|source| map_decode_error(spec_path, source))
    }
}

impl FsSpecDocumentLoader {
    fn guard_spec_path(&self, spec_path: &Path) -> Result<PathBuf, SpecDocumentLoadError> {
        reject_parent_dir_components(&self.trusted_root, "trusted root", spec_path)?;
        reject_parent_dir_components(spec_path, "spec path", spec_path)?;
        let trusted_root = self.absolute_trusted_root(spec_path)?;
        reject_symlinked_trusted_root(&trusted_root, spec_path)?;

        let absolute_spec_path = if spec_path.is_absolute() {
            spec_path.to_path_buf()
        } else {
            current_dir(spec_path)?.join(spec_path)
        };
        let normalized_spec_path = lexical_normalize(&absolute_spec_path);
        if !normalized_spec_path.starts_with(&trusted_root) {
            return Err(SpecDocumentLoadError::Io {
                path: spec_path.to_path_buf(),
                reason: diagnostic(&format!(
                    "spec path '{}' resolves outside trusted root '{}'",
                    spec_path.display(),
                    trusted_root.display()
                )),
            });
        }

        match reject_symlinks_below(&normalized_spec_path, &trusted_root) {
            Ok(true) => Ok(normalized_spec_path),
            Ok(false) => Err(SpecDocumentLoadError::NotFound { path: spec_path.to_path_buf() }),
            Err(source) => Err(SpecDocumentLoadError::Io {
                path: spec_path.to_path_buf(),
                reason: diagnostic(&source.to_string()),
            }),
        }
    }

    fn absolute_trusted_root(&self, spec_path: &Path) -> Result<PathBuf, SpecDocumentLoadError> {
        let root = if self.trusted_root.is_absolute() {
            self.trusted_root.clone()
        } else {
            current_dir(spec_path)?.join(&self.trusted_root)
        };
        Ok(lexical_normalize(&root))
    }
}

fn reject_parent_dir_components(
    path: &Path,
    label: &str,
    spec_path: &Path,
) -> Result<(), SpecDocumentLoadError> {
    if path.components().any(|component| component == std::path::Component::ParentDir) {
        return Err(SpecDocumentLoadError::Io {
            path: spec_path.to_path_buf(),
            reason: diagnostic(&format!(
                "{label} '{}' contains parent directory traversal",
                path.display()
            )),
        });
    }
    Ok(())
}

fn current_dir(spec_path: &Path) -> Result<PathBuf, SpecDocumentLoadError> {
    std::env::current_dir().map_err(|source| SpecDocumentLoadError::Io {
        path: spec_path.to_path_buf(),
        reason: diagnostic(&format!("cannot determine current directory: {source}")),
    })
}

fn reject_symlinked_trusted_root(
    trusted_root: &Path,
    spec_path: &Path,
) -> Result<(), SpecDocumentLoadError> {
    for component in trusted_root.ancestors() {
        if component.as_os_str().is_empty() {
            continue;
        }
        match component.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(SpecDocumentLoadError::Io {
                    path: spec_path.to_path_buf(),
                    reason: diagnostic(&format!(
                        "refusing to use symlinked trusted root component '{}'",
                        component.display()
                    )),
                });
            }
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(SpecDocumentLoadError::Io {
                    path: spec_path.to_path_buf(),
                    reason: diagnostic(&format!(
                        "cannot inspect trusted root component '{}': {source}",
                        component.display()
                    )),
                });
            }
        }
    }
    Ok(())
}

fn map_decode_error(spec_path: &Path, source: SpecCodecError) -> SpecDocumentLoadError {
    let reason = diagnostic(&source.to_string());
    match source {
        SpecCodecError::Json(_) | SpecCodecError::UnsupportedSchemaVersion(_) => {
            SpecDocumentLoadError::JsonParse { path: spec_path.to_path_buf(), reason }
        }
        SpecCodecError::Validation(_)
        | SpecCodecError::DomainValidation(_)
        | SpecCodecError::InvalidField { .. } => {
            SpecDocumentLoadError::Invariant { path: spec_path.to_path_buf(), reason }
        }
    }
}

fn diagnostic(message: &str) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "spec document load failure".to_owned()
    } else {
        message.to_owned()
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            Err(_) => text = "spec document load failure".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn loader_for(root: &Path) -> FsSpecDocumentLoader {
        FsSpecDocumentLoader::new(root.to_path_buf())
    }

    #[test]
    fn test_fs_spec_document_loader_missing_file_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("track/items");
        std::fs::create_dir_all(&root).unwrap();
        let loader = loader_for(&root);
        let path = root.join("missing-spec.json");

        let err = loader.load(&path).unwrap_err();

        assert!(matches!(err, SpecDocumentLoadError::NotFound { .. }));
    }

    #[test]
    fn test_fs_spec_document_loader_malformed_json_returns_json_parse() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("track/items");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("spec.json");
        std::fs::write(&path, "{").unwrap();
        let loader = loader_for(&root);

        let err = loader.load(&path).unwrap_err();

        assert!(matches!(err, SpecDocumentLoadError::JsonParse { .. }));
    }

    #[test]
    fn test_fs_spec_document_loader_invariant_failure_returns_invariant() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("track/items");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("spec.json");
        std::fs::write(
            &path,
            r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Spec",
  "goal": [{"id": "", "text": "invalid id"}],
  "scope": {"in_scope": [], "out_of_scope": []}
}"#,
        )
        .unwrap();
        let loader = loader_for(&root);

        let err = loader.load(&path).unwrap_err();

        assert!(matches!(err, SpecDocumentLoadError::Invariant { .. }));
    }

    #[test]
    fn test_fs_spec_document_loader_rejects_parent_dir_escape() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("repo");
        let root = workspace.join("track/items");
        let outside = workspace.join("secrets");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("spec.json"), "{").unwrap();
        let loader = loader_for(&root);

        let err = loader.load(&root.join("../../secrets/spec.json")).unwrap_err();

        match err {
            SpecDocumentLoadError::Io { reason, .. } => {
                assert!(reason.as_str().contains("parent directory traversal"));
            }
            other => panic!("expected traversal to map to Io error, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_spec_document_loader_symlink_leaf_returns_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("track/items");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("real-spec.json");
        let link = root.join("spec.json");
        std::fs::write(&target, "{}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let loader = loader_for(&root);

        let err = loader.load(&link).unwrap_err();

        match err {
            SpecDocumentLoadError::Io { reason, .. } => {
                assert!(reason.as_str().contains("refusing to follow symlink"));
            }
            other => panic!("expected symlink to map to Io error, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_spec_document_loader_symlink_parent_returns_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("track/items");
        std::fs::create_dir_all(&root).unwrap();
        let real_dir = root.join("real-track");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::fs::write(real_dir.join("spec.json"), "{}").unwrap();
        let link_dir = root.join("linked-track");
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();
        let loader = loader_for(&root);

        let err = loader.load(&link_dir.join("spec.json")).unwrap_err();

        match err {
            SpecDocumentLoadError::Io { reason, .. } => {
                assert!(reason.as_str().contains("refusing to follow symlink"));
            }
            other => panic!("expected symlink to map to Io error, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_spec_document_loader_symlink_trusted_root_returns_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let real_root = dir.path().join("real-items");
        let link_root = dir.path().join("linked-items");
        std::fs::create_dir_all(&real_root).unwrap();
        std::fs::write(real_root.join("spec.json"), "{}").unwrap();
        std::os::unix::fs::symlink(&real_root, &link_root).unwrap();
        let loader = loader_for(&link_root);

        let err = loader.load(&link_root.join("spec.json")).unwrap_err();

        match err {
            SpecDocumentLoadError::Io { reason, .. } => {
                assert!(reason.as_str().contains("symlinked trusted root"));
            }
            other => panic!("expected symlinked root to map to Io error, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_spec_document_loader_symlink_trusted_root_ancestor_returns_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let real_workspace = dir.path().join("real-repo");
        let real_root = real_workspace.join("track/items");
        std::fs::create_dir_all(&real_root).unwrap();
        std::fs::write(real_root.join("spec.json"), "{}").unwrap();
        let link_workspace = dir.path().join("linked-repo");
        std::os::unix::fs::symlink(&real_workspace, &link_workspace).unwrap();
        let link_root = link_workspace.join("track/items");
        let loader = loader_for(&link_root);

        let err = loader.load(&link_root.join("spec.json")).unwrap_err();

        match err {
            SpecDocumentLoadError::Io { reason, .. } => {
                assert!(reason.as_str().contains("symlinked trusted root component"));
            }
            other => panic!("expected symlinked root ancestor to map to Io error, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_spec_document_loader_rejects_parent_dir_through_symlink_before_normalizing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("track/items");
        let outside = dir.path().join("outside");
        let outside_nested = outside.join("nested");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside_nested).unwrap();
        std::fs::write(outside.join("spec.json"), "{}").unwrap();
        let link = root.join("linked-outside");
        std::os::unix::fs::symlink(&outside_nested, &link).unwrap();
        let loader = loader_for(&root);

        let err = loader.load(&link.join("../spec.json")).unwrap_err();

        match err {
            SpecDocumentLoadError::Io { reason, .. } => {
                assert!(reason.as_str().contains("parent directory traversal"));
            }
            other => panic!("expected parent traversal to map to Io error, got {other:?}"),
        }
    }
}
