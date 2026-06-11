//! Verify that this checkout uses the repository-managed Git hooks directory.
//!
//! This verifier intentionally reads `core.hooksPath` through a direct
//! read-only `git config` invocation. The guarded-git token injection path is
//! only needed for write operations.

use std::path::Path;
use std::process::Command;

use domain::verify::{VerifyFinding, VerifyOutcome};

const EXPECTED_HOOKS_PATH: &str = ".githooks";
const REMEDIATION: &str = "Run `git config core.hooksPath .githooks` from the repository root.";

/// Verify that local Git config sets `core.hooksPath` to `.githooks`.
pub fn verify(root: &Path) -> VerifyOutcome {
    let output = match Command::new("git")
        .args(["config", "--local", "core.hooksPath"])
        .current_dir(root)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            return failing_outcome(format!(
                "Cannot read local Git config core.hooksPath: {e}. {REMEDIATION}"
            ));
        }
    };

    if !output.status.success() {
        return failing_outcome(format!(
            "Git config core.hooksPath is not set to {EXPECTED_HOOKS_PATH}. {REMEDIATION}"
        ));
    }

    let hooks_path = String::from_utf8_lossy(&output.stdout);
    let hooks_path = strip_git_config_line_ending(&hooks_path);

    if hooks_path == EXPECTED_HOOKS_PATH {
        return VerifyOutcome::pass();
    }

    failing_outcome(format!(
        "Git config core.hooksPath is `{hooks_path}`, expected `{EXPECTED_HOOKS_PATH}`. \
         {REMEDIATION}"
    ))
}

fn failing_outcome(message: String) -> VerifyOutcome {
    VerifyOutcome::from_findings(vec![VerifyFinding::error(message)])
}

fn strip_git_config_line_ending(value: &str) -> &str {
    if let Some(stripped) = value.strip_suffix("\r\n") {
        return stripped;
    }

    if let Some(stripped) = value.strip_suffix('\n') {
        return stripped;
    }

    value
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;

    fn init_repo(root: &Path) {
        run_git(root, &["init"]);
    }

    fn set_hooks_path(root: &Path, value: &str) {
        run_git(root, &["config", "--local", "core.hooksPath", value]);
    }

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git command must run in hooks-path verifier tests");
        assert!(
            output.status.success(),
            "git command failed: git {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn test_verify_with_hooks_path_githooks_passes() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        set_hooks_path(tmp.path(), ".githooks");

        let outcome = verify(tmp.path());

        assert!(outcome.is_ok());
    }

    #[test]
    fn test_verify_with_unset_hooks_path_returns_error() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors());
        assert!(
            outcome.findings().iter().any(|finding| finding
                .message()
                .contains("core.hooksPath is not set to .githooks"))
        );
    }

    #[test]
    fn test_verify_with_missing_root_returns_error() {
        let tmp = TempDir::new().unwrap();
        let missing_root = tmp.path().join("missing-repo");

        let outcome = verify(&missing_root);

        assert!(outcome.has_errors());
        assert!(
            outcome
                .findings()
                .iter()
                .any(|finding| finding.message().contains("Cannot read local Git config"))
        );
    }

    #[test]
    fn test_verify_with_other_hooks_path_returns_error() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        set_hooks_path(tmp.path(), ".git/hooks");

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors());
        assert!(
            outcome
                .findings()
                .iter()
                .any(|finding| finding.message().contains("expected `.githooks`"))
        );
    }

    #[test]
    fn test_verify_with_trailing_space_hooks_path_returns_error() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        set_hooks_path(tmp.path(), ".githooks ");

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors());
        assert!(
            outcome
                .findings()
                .iter()
                .any(|finding| finding.message().contains("expected `.githooks`"))
        );
    }

    #[test]
    fn test_verify_with_leading_space_hooks_path_returns_error() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        set_hooks_path(tmp.path(), " .githooks");

        let outcome = verify(tmp.path());

        assert!(outcome.has_errors());
        assert!(
            outcome
                .findings()
                .iter()
                .any(|finding| finding.message().contains("expected `.githooks`"))
        );
    }
}
