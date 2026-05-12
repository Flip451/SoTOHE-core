//! Private security helpers for path validation and symlink rejection.
//!
//! These helpers enforce the path-safety invariants required before any
//! filesystem I/O in the `catalogue-impl-signals` interactor.

use super::service::CatalogueImplSignalsError;

// ---------------------------------------------------------------------------
// Path security helpers
// ---------------------------------------------------------------------------

/// Rejects any symlink component in the full path from the filesystem root
/// down to (and including) `path`.
///
/// This is stricter than checking only the leaf component: a symlink in any
/// ancestor directory would redirect all I/O beneath it.  Used to validate
/// `workspace_root` before it becomes the trusted anchor for all subsequent
/// path derivations.
///
/// # Errors
///
/// Returns `CatalogueImplSignalsError::SymlinkRejected` if any component of
/// `path` is a symlink.  Components that cannot be stat'd (e.g. not yet
/// created) are skipped — only confirmed symlinks are rejected.
pub(super) fn reject_path_symlinks_from_root(
    path: &std::path::Path,
) -> Result<(), CatalogueImplSignalsError> {
    // Collect all ancestors from the root down to `path` (inclusive).
    let mut components: Vec<&std::path::Path> = path.ancestors().collect();
    // `ancestors()` yields leaf → root; reverse to root → leaf.
    components.reverse();

    for component in &components {
        if component.as_os_str().is_empty() {
            continue;
        }
        // Only act on successful metadata reads: if the path does not exist
        // yet or cannot be stat'd for any reason, skip the component (it
        // cannot be a symlink if metadata is unavailable).
        if let Ok(meta) = component.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Err(CatalogueImplSignalsError::SymlinkRejected {
                    path: component.display().to_string(),
                });
            }
        }
    }
    Ok(())
}

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

/// Rejects symlinks at the leaf path and every ancestor between it and
/// `trusted_root` (exclusive).
///
/// Returns `Ok(())` if no symlinks are detected.
///
/// # Errors
///
/// Returns `CatalogueImplSignalsError::SymlinkRejected` if any component of
/// `path` below `trusted_root` is a symlink.  Components that cannot be
/// stat'd (e.g. not yet created) are skipped — only confirmed symlinks are
/// rejected.
pub(super) fn reject_symlinks_below(
    path: &std::path::Path,
    trusted_root: &std::path::Path,
) -> Result<(), CatalogueImplSignalsError> {
    let mut components: Vec<&std::path::Path> = Vec::new();
    for ancestor in path.ancestors() {
        if ancestor == trusted_root || ancestor.as_os_str().is_empty() {
            break;
        }
        components.push(ancestor);
    }
    // Walk root → leaf (parents first)
    components.reverse();

    for component in &components {
        // Only act on successful metadata reads: if the path does not exist
        // yet or cannot be stat'd for any reason, skip the component (it
        // cannot be a symlink if metadata is unavailable).
        if let Ok(meta) = component.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Err(CatalogueImplSignalsError::SymlinkRejected {
                    path: component.display().to_string(),
                });
            }
        }
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
