//! Repository-aware context resolution for command-completion telemetry.
//!
//! This adapter performs the branch lookup used by the CLI driver's injected
//! completion resolver. Keeping it in infrastructure prevents the binary and
//! composition root from performing repository/configuration work directly.

use std::path::{Path, PathBuf};

use crate::git_cli::SystemGitRepo;
use crate::telemetry::TelemetryConfig;

/// Resolves the active `track/<id>` branch for `items_dir`.
///
/// Returns `None` when the items path is malformed, the repository cannot be
/// discovered, the branch is detached/non-track, or telemetry is disabled.
/// The function performs no telemetry file I/O; the existing writer remains
/// the sole persistence sink.
#[must_use]
pub fn resolve_telemetry_track_id(items_dir: &Path) -> Option<String> {
    let project_root = resolve_project_root(items_dir)?;
    if items_dir.components().any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }
    let default_items_dir = !items_dir.is_absolute() && items_dir == Path::new("track/items");
    let cwd = std::env::current_dir().ok()?;
    let absolute_root =
        if project_root.is_absolute() { project_root } else { cwd.join(project_root) };
    crate::track::symlink_guard::reject_symlinks_up_to_root(&absolute_root).ok()?;
    let canonical_project_root = absolute_root.canonicalize().ok()?;
    if !canonical_project_root.is_dir() {
        return None;
    }
    let repo = SystemGitRepo::discover_from_isolated(&canonical_project_root).ok()?;
    let discovered_root = repo.root().canonicalize().ok()?;
    let canonical_root = if default_items_dir {
        // `track/items` is the default configured path. When invoked from a
        // repository subdirectory, anchor it to the enclosing repository
        // rather than treating that subdirectory as a project root.
        discovered_root
    } else {
        // Explicit non-default items roots pass through unchanged, including
        // nested in-repository layouts such as `<repo>/custom/track/items`.
        // Only roots outside the enclosing repository stay fail-closed.
        if !canonical_project_root.starts_with(&discovered_root) {
            return None;
        }
        canonical_project_root
    };
    let absolute_items_dir = if items_dir.is_absolute() {
        items_dir.to_path_buf()
    } else if default_items_dir {
        canonical_root.join("track").join("items")
    } else {
        cwd.join(items_dir)
    };
    crate::track::symlink_guard::reject_symlinks_below(&absolute_items_dir, &canonical_root)
        .ok()?;
    // The completion context is captured before dispatch. Some commands (for
    // example `dry write`) materialize `track/items` as part of their own
    // operation, so the directory may not exist yet. Canonicalize existing
    // paths for containment/symlink validation, while retaining the validated
    // absolute path for a not-yet-created directory.
    let canonical_items_dir = match absolute_items_dir.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Rebase the not-yet-created suffix onto the canonical root. This
            // avoids comparing a Windows drive-letter/UNC spelling with a
            // canonical extended-length path, while the schema-derived root
            // still guarantees the destination is exactly `track/items`.
            canonical_root.join("track").join("items")
        }
        Err(_) => return None,
    };
    if !canonical_items_dir.starts_with(&canonical_root)
        || (canonical_items_dir.exists() && !canonical_items_dir.is_dir())
    {
        return None;
    }
    let branch_output = crate::git_cli::isolated_bounded_git_output(
        &canonical_root,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        16 * 1024,
    )
    .ok()?;
    if !branch_output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_owned();
    if branch.is_empty() {
        return None;
    }
    let track_id = usecase::track_resolution::resolve_track_id_from_branch(Some(&branch)).ok()?;
    TelemetryConfig::from_env().is_enabled().then_some(track_id)
}

fn resolve_project_root(items_dir: &Path) -> Option<PathBuf> {
    let items_name = items_dir.file_name().and_then(|name| name.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(Path::file_name).and_then(|name| name.to_str());
    let project_root = track_dir.and_then(Path::parent);
    if items_name == Some("items") && track_name == Some("track") {
        Some(if project_root.is_some_and(|root| root.as_os_str().is_empty()) {
            PathBuf::from(".")
        } else {
            project_root?.to_path_buf()
        })
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::resolve_telemetry_track_id;

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repo(path: &Path, branch: &str) {
        run_git(path, &["init", "--quiet", "--initial-branch=main"]);
        run_git(path, &["config", "user.email", "test@example.invalid"]);
        run_git(path, &["config", "user.name", "Telemetry Context Test"]);
        std::fs::create_dir_all(path.join("track/items")).unwrap();
        std::fs::write(path.join("README.md"), "fixture\n").unwrap();
        run_git(path, &["add", "."]);
        run_git(path, &["commit", "--quiet", "-m", "fixture"]);
        run_git(path, &["checkout", "--quiet", "-b", branch]);
    }

    fn seed_repo(branch: &str) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        init_repo(tmp.path(), branch);
        tmp
    }

    fn test_lock() -> MutexGuard<'static, ()> {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn test_resolve_telemetry_track_id_returns_current_track_branch() {
        let _lock = test_lock();
        let repo = seed_repo("track/context-test");
        let items_dir = repo.path().join("track/items");

        let result = temp_env::with_var("SOTP_TELEMETRY", Some("1"), || {
            resolve_telemetry_track_id(&items_dir)
        });

        assert_eq!(result.as_deref(), Some("context-test"));
    }

    #[test]
    fn test_resolve_telemetry_track_id_rejects_malformed_and_non_track_contexts() {
        let _lock = test_lock();
        assert_eq!(resolve_telemetry_track_id(Path::new("wrong/path")), None);
        let repo = seed_repo("track/context-nontrack");
        run_git(repo.path(), &["checkout", "--quiet", "main"]);
        let items_dir = repo.path().join("track/items");
        assert_eq!(resolve_telemetry_track_id(&items_dir), None);
    }

    #[test]
    fn test_resolve_telemetry_track_id_honors_kill_switch() {
        let _lock = test_lock();
        let repo = seed_repo("track/context-disabled");
        let items_dir = repo.path().join("track/items");
        let result = temp_env::with_var("SOTP_TELEMETRY", Some("0"), || {
            resolve_telemetry_track_id(&items_dir)
        });
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_telemetry_track_id_allows_missing_items_dir_before_dispatch() {
        let _lock = test_lock();
        let repo = seed_repo("track/context-missing-items");
        std::fs::remove_dir_all(repo.path().join("track/items")).unwrap();
        let result = temp_env::with_var("SOTP_TELEMETRY", Some("1"), || {
            resolve_telemetry_track_id(&repo.path().join("track/items"))
        });
        assert_eq!(result.as_deref(), Some("context-missing-items"));
    }

    #[test]
    fn test_resolve_telemetry_track_id_preserves_nested_in_repository_items_dir() {
        let _lock = test_lock();
        let repo = seed_repo("track/context-nested");
        let nested_items = repo.path().join("custom").join("track").join("items");
        std::fs::create_dir_all(&nested_items).unwrap();

        let result = temp_env::with_var("SOTP_TELEMETRY", Some("1"), || {
            resolve_telemetry_track_id(&nested_items)
        });

        assert_eq!(result.as_deref(), Some("context-nested"));
    }

    #[test]
    fn test_resolve_telemetry_track_id_anchors_nonempty_relative_repository_path() {
        let _lock = test_lock();
        if std::env::var_os("SOTP_CONTEXT_RELATIVE_CHILD").is_some() {
            let repo_root =
                std::path::PathBuf::from(std::env::var_os("SOTP_CONTEXT_RELATIVE_ROOT").unwrap());
            let repo_name = repo_root.file_name().unwrap().to_owned();
            let relative_items = std::path::PathBuf::from(repo_name).join("track/items");
            let result = temp_env::with_var("SOTP_TELEMETRY", Some("1"), || {
                resolve_telemetry_track_id(&relative_items)
            });
            assert_eq!(result.as_deref(), Some("context-relative"));
            return;
        }

        // Relative-path resolution is exercised in a subprocess so tests that
        // temporarily change the parent process CWD cannot race this check.
        let stable_parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = tempfile::tempdir_in(stable_parent).unwrap();
        init_repo(repo.path(), "track/context-relative");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "telemetry::context::tests::test_resolve_telemetry_track_id_anchors_nonempty_relative_repository_path",
                "--nocapture",
            ])
            .current_dir(stable_parent)
            .env("SOTP_CONTEXT_RELATIVE_CHILD", "1")
            .env("SOTP_CONTEXT_RELATIVE_ROOT", repo.path())
            .status()
            .unwrap();
        assert!(status.success(), "relative context subprocess failed: {status}");
    }

    #[test]
    fn test_resolve_telemetry_track_id_anchors_default_items_dir_to_enclosing_repository() {
        let _lock = test_lock();
        if std::env::var_os("SOTP_CONTEXT_DEFAULT_CHILD").is_some() {
            let result = temp_env::with_var("SOTP_TELEMETRY", Some("1"), || {
                resolve_telemetry_track_id(Path::new("track/items"))
            });
            assert_eq!(result.as_deref(), Some("context-default"));
            return;
        }

        // Execute from a child directory to cover the default relative path
        // used by the CLI when it is launched below the repository root.
        let stable_parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = tempfile::tempdir_in(stable_parent).unwrap();
        init_repo(repo.path(), "track/context-default");
        let nested_dir = repo.path().join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "telemetry::context::tests::test_resolve_telemetry_track_id_anchors_default_items_dir_to_enclosing_repository",
                "--nocapture",
            ])
            .current_dir(nested_dir)
            .env("SOTP_CONTEXT_DEFAULT_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success(), "default context subprocess failed: {status}");
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_telemetry_track_id_rejects_symlinked_items_path() {
        let _lock = test_lock();
        let repo = seed_repo("track/context-symlink");
        let outside = tempfile::TempDir::new().unwrap();
        let track_path = repo.path().join("track");
        std::fs::remove_dir_all(&track_path).unwrap();
        std::os::unix::fs::symlink(outside.path(), &track_path).unwrap();

        assert_eq!(resolve_telemetry_track_id(&track_path.join("items")), None);
    }
}
