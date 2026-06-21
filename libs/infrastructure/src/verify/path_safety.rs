//! Shared path-normalization and workspace-guard helpers used by multiple
//! verify modules.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::git_cli::GitRepository as _;
use crate::track::symlink_guard::reject_symlinks_below;

/// Lexically normalize a path by collapsing `..` components without I/O (without following
/// symlinks).
///
/// Each component is pushed onto a stack; `..` pops the last pushed component. This prevents
/// `..` traversal bypasses in containment checks (`starts_with`) while avoiding the symlink
/// follow-through that `canonicalize()` would introduce.
pub(crate) fn lexical_normalize(path: &Path) -> PathBuf {
    let mut components: Vec<std::path::Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => match components.last() {
                Some(std::path::Component::Normal(_)) => {
                    components.pop();
                }
                _ => {
                    components.push(component);
                }
            },
            std::path::Component::CurDir => {}
            _ => {
                components.push(component);
            }
        }
    }
    components.iter().collect()
}

/// Normalize `path` to an absolute, `..`-collapsed form relative to `workspace_root`
/// (relative paths are joined with `workspace_root`, absolute paths are used as-is),
/// verify the result is contained within `workspace_root`, then run `reject_symlinks_below`.
///
/// Returns:
/// - `Ok(normalized_path)` — path is safe to read.
/// - `Err(finding)` — containment violation, symlink, or path not found (fail-closed).
pub(crate) fn normalize_and_guard_path(
    path: &Path,
    workspace_root: &Path,
    display_path: &Path,
    not_found_msg: &str,
) -> Result<PathBuf, VerifyFinding> {
    let abs_path = if path.is_absolute() { path.to_path_buf() } else { workspace_root.join(path) };
    let normalized = lexical_normalize(&abs_path);
    if !normalized.starts_with(workspace_root) {
        return Err(VerifyFinding::error(format!(
            "'{}' resolves outside workspace root '{}'. \
             Only paths under the workspace are allowed.",
            display_path.display(),
            workspace_root.display()
        )));
    }
    match reject_symlinks_below(&normalized, workspace_root) {
        Ok(true) => Ok(normalized),
        Ok(false) => Err(VerifyFinding::error(not_found_msg.to_owned())),
        Err(e) => {
            Err(VerifyFinding::error(format!("symlink guard: {}: {e}", display_path.display())))
        }
    }
}

/// Shared pipeline for signals-file freshness checks.
///
/// Handles workspace discovery, path guarding, file read, decode, and hash comparison,
/// parameterized so both `check_impl_catalog_from_signals_file` and
/// `check_catalog_spec_from_signals_file` can share the common front half while keeping
/// their domain-specific validation steps separate.
///
/// # Parameters
///
/// - `signals_path`: path to the signals file (absolute or workspace-relative).
/// - `catalog_hash_hex`: current hash to compare against the recorded hash.
/// - `not_found_msg`: message emitted when the signals file does not exist.
/// - `decode`: parse the file text into a decoded document or a decode-error string.
/// - `get_recorded_hash`: extract the recorded hash string from the decoded document.
/// - `stale_msg`: build the staleness error message from `(recorded, current, display_path)`.
/// - `run_check`: downstream validation; receives `(doc, normalized_signals, workspace_root)` so
///   callers that need to resolve related paths (e.g. catalogue path lookup) can do so.
///
/// # Returns
///
/// `VerifyOutcome` from whichever early-exit or downstream check fires first.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_signals_file<D>(
    signals_path: &Path,
    catalog_hash_hex: &str,
    not_found_msg: &str,
    decode: impl FnOnce(&str) -> Result<D, String>,
    get_recorded_hash: impl FnOnce(&D) -> String,
    stale_msg: impl FnOnce(&str, &str, &Path) -> String,
    run_check: impl FnOnce(D, PathBuf, PathBuf) -> VerifyOutcome,
) -> VerifyOutcome {
    // Derive the git discovery starting point from the supplied path rather than
    // the process CWD. This keeps the helper correct when the binary is invoked
    // from outside the target repository while an absolute `--signals-path` is
    // supplied (e.g. a wrapper script that passes an absolute path from a different
    // working directory). For absolute paths, use the path's parent; for relative
    // paths, fall back to the process CWD so behavior is unchanged for the common
    // invocation pattern.
    let discover_start: std::borrow::Cow<'_, Path> = if signals_path.is_absolute() {
        signals_path
            .parent()
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| std::borrow::Cow::Owned(signals_path.to_path_buf()))
    } else {
        match std::env::current_dir() {
            Ok(cwd) => std::borrow::Cow::Owned(cwd),
            Err(e) => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "cannot determine current directory for workspace discovery: {e}"
                ))]);
            }
        }
    };

    // Discover the workspace root starting from the signals-path directory so the
    // result is independent of the process working directory when an absolute path
    // is provided.
    let workspace_root = match crate::git_cli::SystemGitRepo::discover_from(&discover_start) {
        Ok(repo) => repo.root().to_path_buf(),
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot discover git repository root for workspace containment check: {e}"
            ))]);
        }
    };

    // Normalize, contain, and symlink-guard the signals path.
    let normalized_signals = match normalize_and_guard_path(
        signals_path,
        &workspace_root,
        signals_path,
        not_found_msg,
    ) {
        Ok(p) => p,
        Err(finding) => return VerifyOutcome::from_findings(vec![finding]),
    };

    let text = match std::fs::read_to_string(&normalized_signals) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                signals_path.display()
            ))]);
        }
    };
    let doc = match decode(&text) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: decode error: {e}",
                signals_path.display()
            ))]);
        }
    };

    let recorded = get_recorded_hash(&doc);
    if recorded != catalog_hash_hex {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(stale_msg(
            &recorded,
            catalog_hash_hex,
            signals_path,
        ))]);
    }

    run_check(doc, normalized_signals, workspace_root)
}
