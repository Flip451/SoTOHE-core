use std::path::{Path, PathBuf};

use domain::review_v2::types::FilePath;

use crate::track::symlink_guard::reject_symlinks_below;

pub(super) fn build_dry_fix_prompt(
    track_id: &str,
    briefing_file: &Path,
    trusted_root: &Path,
) -> Result<String, String> {
    let briefing_path = briefing_file.to_str().ok_or_else(|| {
        format!("briefing_file path is not valid UTF-8: {}", briefing_file.display())
    })?;
    validate_briefing_path_token(briefing_path)?;
    let briefing_file = resolve_safe_briefing_file(briefing_file, trusted_root)?;
    let briefing_content = std::fs::read_to_string(&briefing_file)
        .map_err(|e| format!("failed to read briefing file {briefing_path}: {e}"))?;
    let prompt = format!(
        "$dry-fix-lead\n\n\
         {briefing_content}\n\n\
         ---\n\n\
         ## Orchestrator Assignment\n\n\
         - Track ID: {track_id}\n\n\
         When you finish (DRY gate Approved, loop exhausted with violations remaining, \
         or tooling error), print EXACTLY one of these status lines as your final output \
         line, with no trailing text:\n\n\
         \x20\x20DRY_FIX_STATUS: completed\n\
         \x20\x20DRY_FIX_STATUS: blocked\n\
         \x20\x20DRY_FIX_STATUS: failed",
    );
    Ok(prompt)
}

fn validate_briefing_path_token(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(format!(
            "briefing_file path contains characters that are unsafe in the fixer prompt: \
             {path}",
        ));
    }
    FilePath::new(path.to_owned())
        .map_err(|e| format!("invalid briefing_file path '{path}': {e}"))?;
    if has_windows_drive_prefix(path) {
        return Err(format!(
            "invalid briefing_file path '{path}': Windows drive prefixes are not repo-relative"
        ));
    }
    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    matches!(
        (path.as_bytes().first(), path.as_bytes().get(1)),
        (Some(first), Some(second)) if *second == b':' && first.is_ascii_alphabetic()
    )
}

fn resolve_safe_briefing_file(
    briefing_file: &Path,
    trusted_root: &Path,
) -> Result<PathBuf, String> {
    let canonical_root = trusted_root.canonicalize().map_err(|e| {
        format!("failed to canonicalize trusted root '{}': {e}", trusted_root.display())
    })?;
    let candidate = canonical_root.join(briefing_file);
    if !candidate.starts_with(&canonical_root) {
        return Err(format!(
            "briefing_file path escapes trusted root: {}",
            briefing_file.display()
        ));
    }
    reject_symlinks_below(&candidate, &canonical_root).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidInput {
            format!(
                "symlink detected in briefing_file '{}' (rejected for security)",
                briefing_file.display()
            )
        } else {
            format!("failed to inspect briefing_file '{}': {e}", briefing_file.display())
        }
    })?;
    let canonical_path = candidate.canonicalize().map_err(|e| {
        format!("failed to canonicalize briefing_file '{}': {e}", briefing_file.display())
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "briefing_file path escapes trusted root: {}",
            briefing_file.display()
        ));
    }
    Ok(canonical_path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dry_fix_prompt_repo_relative_file_returns_prompt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("briefing.md"), "# dry briefing\n").unwrap();

        let prompt =
            build_dry_fix_prompt("dry-track", Path::new("briefing.md"), dir.path()).unwrap();

        assert!(prompt.contains("# dry briefing"));
        assert!(prompt.contains("- Track ID: dry-track"));
    }

    #[test]
    fn test_build_dry_fix_prompt_absolute_path_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let err =
            build_dry_fix_prompt("dry-track", Path::new("/etc/passwd"), dir.path()).unwrap_err();

        assert!(err.contains("invalid briefing_file path"), "got: {err}");
    }

    #[test]
    fn test_build_dry_fix_prompt_traversal_path_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let err =
            build_dry_fix_prompt("dry-track", Path::new("../secret.md"), dir.path()).unwrap_err();

        assert!(err.contains("invalid briefing_file path"), "got: {err}");
    }

    #[test]
    fn test_build_dry_fix_prompt_windows_drive_path_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let err =
            build_dry_fix_prompt("dry-track", Path::new("C:secret.md"), dir.path()).unwrap_err();

        assert!(err.contains("Windows drive"), "got: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_build_dry_fix_prompt_symlinked_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("secret.md");
        std::fs::write(&outside_file, "secret\n").unwrap();
        std::os::unix::fs::symlink(&outside_file, dir.path().join("briefing.md")).unwrap();

        let err =
            build_dry_fix_prompt("dry-track", Path::new("briefing.md"), dir.path()).unwrap_err();

        assert!(err.contains("symlink"), "got: {err}");
    }
}
