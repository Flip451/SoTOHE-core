//! Shared test helpers for the `verify` module suite.
//!
//! In infrastructure unit tests (`#[cfg(test)]`), this module exposes small
//! panicking conveniences for test setup.  When the `test-helpers` feature is
//! enabled by dependent crates, only non-panicking public helpers are compiled.

#![allow(dead_code)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used, clippy::panic))]

use std::path::Path;

/// Write a minimal ADR YAML file with a single decision whose decision reference
/// is set to `ref_key: ref_value`.
///
/// Use this to create Yellow-signal ADRs (`review_finding_ref`) or Blue-signal
/// ADRs (`user_decision_ref`) without duplicating the fixture schema in every
/// test module.
#[cfg(test)]
pub(crate) fn write_minimal_adr(adr_dir: &Path, filename: &str, ref_key: &str, ref_value: &str) {
    let content = format!(
        "---\nadr_id: test-adr\ndecisions:\n  - id: D1\n    status: accepted\n    {ref_key}: {ref_value}\n---\n# Test ADR\n"
    );
    std::fs::write(adr_dir.join(filename), content).unwrap();
}

#[cfg(all(feature = "test-helpers", not(test)))]
#[derive(Debug)]
pub enum RunGitError {
    Spawn { command: String, source: std::io::Error },
    Failed { command: String, code: Option<i32>, stdout: String, stderr: String },
}

#[cfg(all(feature = "test-helpers", not(test)))]
impl std::fmt::Display for RunGitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn { command, source } => {
                write!(f, "git command failed to start: git {command}: {source}")
            }
            Self::Failed { command, code, stdout, stderr } => write!(
                f,
                "git command failed: git {command} (exit {:?})\nstdout:\n{stdout}\nstderr:\n{stderr}",
                code
            ),
        }
    }
}

#[cfg(all(feature = "test-helpers", not(test)))]
impl std::error::Error for RunGitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } => Some(source),
            Self::Failed { .. } => None,
        }
    }
}

#[cfg(all(feature = "test-helpers", not(test)))]
#[derive(Debug)]
pub struct RunGitResult {
    inner: Result<(), RunGitError>,
}

#[cfg(all(feature = "test-helpers", not(test)))]
impl RunGitResult {
    pub fn into_result(self) -> Result<(), RunGitError> {
        self.inner
    }

    pub fn is_ok(&self) -> bool {
        self.inner.is_ok()
    }

    pub fn is_err(&self) -> bool {
        self.inner.is_err()
    }
}

#[cfg(all(feature = "test-helpers", not(test)))]
impl From<RunGitResult> for Result<(), RunGitError> {
    fn from(result: RunGitResult) -> Self {
        result.into_result()
    }
}

/// Run a git command in `root` and assert it succeeds.
///
/// Captures stdout and stderr so that test failures include full git output.
/// Panics when `git` is not installed or when the command exits with a
/// non-zero status.  Use this instead of duplicating the `Command::new("git")`
/// invocation pattern in multiple test modules.
#[cfg(test)]
pub fn run_git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git command must run in tests — is git installed?");
    assert!(
        output.status.success(),
        "git command failed: git {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Run a git command in `cwd` with a deterministic locale and author identity.
///
/// Sets `LANG`/`LC_ALL`/`LANGUAGE` to `"C"` and provides stub
/// `GIT_AUTHOR_*` / `GIT_COMMITTER_*` identity so that `git commit` succeeds
/// in environments without a global git config.  Panics on spawn failure or
/// non-zero exit, printing full stdout and stderr for diagnosis.
///
/// Use this helper (instead of [`run_git`]) when the git repo fixture needs
/// commits, because `git commit` requires a configured author identity.
#[cfg(test)]
pub(crate) fn git_with_identity(cwd: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git command failed to spawn");
    if !output.status.success() {
        panic!(
            "git {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Run a git command in `root`.
///
/// This feature-gated variant returns a value carrying the result instead of
/// panicking, so enabling `test-helpers` does not add panic paths to the library.
#[cfg(all(feature = "test-helpers", not(test)))]
pub fn run_git(root: &Path, args: &[&str]) -> RunGitResult {
    RunGitResult { inner: try_run_git(root, args) }
}

/// Run a git command in `root`, returning detailed failure output.
///
/// # Errors
///
/// Returns [`RunGitError`] when `git` cannot be started or exits unsuccessfully.
#[cfg(all(feature = "test-helpers", not(test)))]
pub fn try_run_git(root: &Path, args: &[&str]) -> Result<(), RunGitError> {
    let command = args.join(" ");
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|source| RunGitError::Spawn { command: command.clone(), source })?;

    if output.status.success() {
        return Ok(());
    }

    Err(RunGitError::Failed {
        command,
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Initialise a minimal git repository in `dir`.
///
/// Configures a stub identity (`test@test.com` / `Test`) so git does not error
/// on missing global config.  Panics when `git` is not installed — these tests
/// require a working `git` binary on `PATH`.
#[cfg(test)]
pub(crate) fn git_init(dir: &Path) {
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir)
        .status()
        .expect("git must be installed for these tests");
    assert!(status.success(), "git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .status()
        .ok();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .status()
        .ok();
}
