//! Pure path-validation helpers for the `catalogue-impl-signals` interactor.
//!
//! This module contains only pure (no I/O) path-string validation. Symlink
//! rejection has been moved behind [`domain::SymlinkGuardPort`] and is
//! performed by the interactor via the injected port.

use super::service::CatalogueImplSignalsError;

// ---------------------------------------------------------------------------
// Path security helpers
// ---------------------------------------------------------------------------

/// Validates that a binding-supplied filename is a simple basename with no
/// directory separators or `..` path-traversal components, preventing path
/// traversal.
///
/// Expected format: `<layer>-types.json` or `<layer>-types-baseline.json` —
/// a single filename component with no `/` or `\` separators.
///
/// # Errors
///
/// Returns `CatalogueImplSignalsError::SymlinkRejected` (path containment
/// violation) if the filename is not a plain basename (contains `/`, `\`, or
/// the bare reserved components `.` / `..`).
pub(super) fn validate_binding_filename(
    filename: &str,
    context: &str,
) -> Result<(), CatalogueImplSignalsError> {
    // Fast-path: explicit rejection of bare reserved path components `..` and
    // `.`.  `Path::file_name()` returns `None` for these, but checking them
    // first provides a clear early error message.
    if filename == ".." || filename == "." {
        return Err(CatalogueImplSignalsError::SymlinkRejected {
            path: format!("{context}: '{filename}' is a reserved path component"),
        });
    }
    // A plain basename has the property that `Path::file_name()` equals the
    // entire `OsStr` of the path itself.  This is false for:
    // - paths containing `/` (Unix directory separator)
    // - paths ending in `..` or `.` (which return `None` from `file_name()`)
    // - absolute paths (same reason: they never match their full `as_os_str()`)
    //
    // We also explicitly reject `\` (Windows path separator) even on Linux,
    // where `\` is a valid filename character but would create an invalid binding
    // if the file were ever used on a Windows host.
    //
    // Note: do NOT test `filename.contains("..")` as a substring — that would
    // incorrectly reject valid filenames like `foo..json`.  The
    // `file_name() == as_os_str()` invariant already handles `..` appearing as a
    // path *component* (e.g. `"../passwd"` → `file_name()` returns
    // `Some("passwd")` which differs from `as_os_str()` `"../passwd"`).
    let path = std::path::Path::new(filename);
    let is_plain_filename = path.file_name() == Some(path.as_os_str())
        && !filename.contains('/')
        && !filename.contains('\\');
    if !is_plain_filename {
        return Err(CatalogueImplSignalsError::SymlinkRejected {
            path: format!(
                "{context}: '{filename}' is not a plain filename (path traversal rejected)"
            ),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // validate_binding_filename tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_binding_filename_plain_name_passes() {
        validate_binding_filename("domain-types.json", "catalogue_file").unwrap();
    }

    #[test]
    fn test_validate_binding_filename_baseline_plain_name_passes() {
        validate_binding_filename("domain-types-baseline.json", "baseline_file").unwrap();
    }

    #[test]
    fn test_validate_binding_filename_bare_dotdot_rejected() {
        // A bare `..` must be rejected: `track_dir.join("..")` escapes the track
        // directory one level up.
        let err = validate_binding_filename("..", "catalogue_file").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }));
    }

    #[test]
    fn test_validate_binding_filename_dotdot_in_path_rejected() {
        let err = validate_binding_filename("../etc/passwd", "catalogue_file").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }));
    }

    #[test]
    fn test_validate_binding_filename_absolute_path_rejected() {
        let err = validate_binding_filename("/etc/passwd", "catalogue_file").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }));
    }

    #[test]
    fn test_validate_binding_filename_subdirectory_rejected() {
        let err = validate_binding_filename("sub/domain-types.json", "catalogue_file").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }));
    }

    #[test]
    fn test_validate_binding_filename_windows_separator_rejected() {
        let err =
            validate_binding_filename("sub\\domain-types.json", "catalogue_file").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }));
    }

    #[test]
    fn test_validate_binding_filename_double_dot_in_middle_passes() {
        // A filename like `foo..json` contains ".." as a substring but is a
        // valid plain basename — it must NOT be rejected.
        validate_binding_filename("foo..json", "catalogue_file").unwrap();
    }
}
