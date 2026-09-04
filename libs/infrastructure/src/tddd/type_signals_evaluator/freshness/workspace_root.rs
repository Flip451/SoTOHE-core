use std::io::Error;
use std::path::Path;

use super::{FingerprintDeadline, RustdocInputFingerprintError, io_error};

pub(super) fn validate_workspace_root_for_fingerprint(
    workspace_root: &Path,
    deadline: &FingerprintDeadline,
) -> Result<(), RustdocInputFingerprintError> {
    #[cfg(not(unix))]
    {
        return Err(io_error(
            workspace_root,
            Error::new(
                std::io::ErrorKind::Unsupported,
                format!(
                    "descriptor-relative no-follow rustdoc locks are supported only on Unix (workspace root '{}')",
                    workspace_root.display()
                ),
            ),
        ));
    }

    #[cfg(unix)]
    {
        let guard_path = workspace_root.to_owned();
        deadline
            .run_io("workspace root validation", workspace_root.to_owned(), move || {
                crate::track::symlink_guard::reject_symlinks_up_to_root(&guard_path).map_err(
                    |error| {
                        Error::new(
                            error.kind(),
                            format!(
                                "symlink guard: refusing to use workspace root '{}': {error}",
                                guard_path.display()
                            ),
                        )
                    },
                )
            })
            .map_err(|error| map_workspace_root_validation_error(error, workspace_root))?;

        let metadata_path = workspace_root.to_owned();
        let metadata = deadline
            .run_io("workspace root validation", workspace_root.to_owned(), move || {
                metadata_path.symlink_metadata()
            })
            .map_err(|error| map_workspace_root_validation_error(error, workspace_root))?;
        if !metadata.is_dir() {
            return Err(io_error(
                workspace_root,
                Error::other(format!(
                    "trusted workspace root '{}' is not a directory",
                    workspace_root.display()
                )),
            ));
        }
        Ok(())
    }
}

fn map_workspace_root_validation_error(
    error: RustdocInputFingerprintError,
    workspace_root: &Path,
) -> RustdocInputFingerprintError {
    match error {
        RustdocInputFingerprintError::Io { source, .. } => io_error(
            workspace_root,
            Error::other(format!(
                "cannot inspect trusted workspace root '{}': {source}",
                workspace_root.display()
            )),
        ),
        other => other,
    }
}
