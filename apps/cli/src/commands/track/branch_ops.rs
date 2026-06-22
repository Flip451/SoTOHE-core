//! Shared branch primitives for track branch operations.
//!
//! Contains the implementation of `track branch create` and `track branch switch`,
//! along with shared git helpers used by both operations.

use std::process::ExitCode;

use cli_composition::CliApp;

use crate::CliError;

use super::{
    BranchAction, BranchArgs, resolve_project_root, validate_track_branch_str,
    validate_track_id_str,
};

pub(super) fn execute_branch(action: BranchAction) -> Result<ExitCode, CliError> {
    match action {
        BranchAction::Create(args) => execute_branch_create(args),
        BranchAction::Switch(args) => execute_branch_switch(args),
    }
}

/// Creates a new `track/<track-id>` branch from `main` and switches to it.
///
/// # Errors
/// Returns `CliError::Message` when any of the following holds:
/// - `track_id` is malformed, or the derived branch name is invalid
/// - `items_dir` does not point at `<project-root>/track/items`
/// - the current branch is not `main`
/// - a branch named `track/<track-id>` already exists
/// - the underlying `git switch -c` invocation fails
fn execute_branch_create(args: BranchArgs) -> Result<ExitCode, CliError> {
    let BranchArgs { items_dir, track_id } = args;

    validate_track_id_str(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let branch_name = format!("track/{track_id}");

    validate_track_branch_str(&branch_name)
        .map_err(|err| CliError::Message(format!("invalid track branch: {err}")))?;
    resolve_project_root(&items_dir).map_err(CliError::Message)?;

    let app = CliApp::new();
    let outcome = app
        .track_branch_create(items_dir, track_id)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

/// Switches to an existing `track/<track-id>` branch.
///
/// # Errors
/// Returns `CliError::Message` when any of the following holds:
/// - `track_id` is malformed, or the derived branch name is invalid
/// - `items_dir` does not point at `<project-root>/track/items`
/// - a branch named `track/<track-id>` does not exist
/// - the underlying `git switch` invocation fails
fn execute_branch_switch(args: BranchArgs) -> Result<ExitCode, CliError> {
    let BranchArgs { items_dir, track_id } = args;

    validate_track_id_str(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let branch_name = format!("track/{track_id}");

    validate_track_branch_str(&branch_name)
        .map_err(|err| CliError::Message(format!("invalid track branch: {err}")))?;
    resolve_project_root(&items_dir).map_err(CliError::Message)?;

    let app = CliApp::new();
    let outcome = app
        .track_branch_switch(items_dir, track_id)
        .map_err(|e| CliError::Message(e.to_string()))?;
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Mutex;

    use infrastructure::git_cli::{GitError, GitRepository};

    use super::super::resolve_project_root;
    use std::path::Path;

    // ---------------------------------------------------------------------------
    // Git helper functions — test-only (only called from this test module)
    // ---------------------------------------------------------------------------

    /// Returns the git command list for a branch-create invocation.
    ///
    /// The create path intentionally emits only `git switch -c track/<id> main`; it must never
    /// stage or commit metadata so that `main` stays untouched while the new track branch is being
    /// bootstrapped.
    pub(super) fn branch_create_git_commands(branch_name: &str) -> Vec<Vec<String>> {
        vec![vec!["switch".to_owned(), "-c".to_owned(), branch_name.to_owned(), "main".to_owned()]]
    }

    pub(super) fn branch_exists(
        repo: &impl GitRepository,
        branch_name: &str,
    ) -> Result<bool, String> {
        let output = repo
            .output(&["rev-parse", "--verify", "--quiet", branch_name])
            .map_err(|e| e.to_string())?;
        Ok(output.status.success())
    }

    pub(super) fn rev_parse_oid(
        repo: &impl GitRepository,
        rev: &str,
    ) -> Result<Option<String>, String> {
        let spec = format!("{rev}^{{commit}}");
        let output = repo
            .output(&["rev-parse", "--verify", "--quiet", spec.as_str()])
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    }

    fn reject_stale_or_divergent_branch(
        repo: &impl GitRepository,
        branch_name: &str,
        exists: bool,
    ) -> Result<(), String> {
        if !exists {
            return Ok(());
        }

        if repo.current_branch().map_err(|e| e.to_string())?.as_deref() == Some(branch_name) {
            return Ok(());
        }

        let current_head = rev_parse_oid(repo, "HEAD")?
            .ok_or_else(|| "cannot resolve current HEAD for activation preflight".to_owned())?;
        let branch_head = rev_parse_oid(repo, branch_name)?
            .ok_or_else(|| format!("cannot resolve existing branch '{branch_name}'"))?;

        if current_head != branch_head {
            return Err(format!(
                "branch '{branch_name}' exists but does not point at the current HEAD; refuse to activate onto a stale/divergent branch"
            ));
        }

        Ok(())
    }

    pub(super) fn preflight_branch_operation(
        repo: &impl GitRepository,
        branch_name: &str,
        require_alignment: bool,
    ) -> Result<bool, String> {
        let exists = branch_exists(repo, branch_name)?;
        if require_alignment {
            reject_stale_or_divergent_branch(repo, branch_name, exists)?;
        }
        Ok(exists)
    }

    /// Executes the branch-create git commands against `repo` after validating preconditions.
    ///
    /// Preconditions:
    /// - current branch must be `main` (branch create must fork from main)
    /// - target branch `branch_name` must not yet exist
    ///
    /// The function guarantees it never runs `git add` / `git commit` — only the commands produced
    /// by [`branch_create_git_commands`] are issued.
    pub(super) fn branch_create_execute(
        repo: &impl GitRepository,
        branch_name: &str,
    ) -> Result<(), String> {
        let current = repo.current_branch().map_err(|err| err.to_string())?;
        if current.as_deref() != Some("main") {
            return Err(format!(
                "branch create must start from 'main'; current branch is {}",
                current.as_deref().unwrap_or("<detached>")
            ));
        }

        if branch_exists(repo, branch_name)? {
            return Err(format!("branch '{branch_name}' already exists"));
        }

        for command in branch_create_git_commands(branch_name) {
            let args: Vec<&str> = command.iter().map(String::as_str).collect();
            match repo.status(&args) {
                Ok(0) => {}
                Ok(_) => return Err(format!("git {} failed", args.join(" "))),
                Err(err) => return Err(format!("failed to run git {}: {err}", args.join(" "))),
            }
        }
        Ok(())
    }

    struct StubRepo {
        current_branch: Option<String>,
        outputs: HashMap<Vec<String>, Output>,
    }

    impl GitRepository for StubRepo {
        fn root(&self) -> &Path {
            Path::new(".")
        }

        fn status(&self, _args: &[&str]) -> Result<i32, GitError> {
            Ok(0)
        }

        fn output(&self, args: &[&str]) -> Result<Output, GitError> {
            self.outputs
                .get(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .cloned()
                .ok_or_else(|| GitError::CommandFailed {
                    command: args.join(" "),
                    code: -1,
                    stderr: format!("unexpected git args: {}", args.join(" ")),
                })
        }

        fn current_branch(&self) -> Result<Option<String>, GitError> {
            Ok(self.current_branch.clone())
        }
    }

    struct RecordingRepo {
        current_branch: Option<String>,
        outputs: HashMap<Vec<String>, Output>,
        status_calls: Mutex<Vec<Vec<String>>>,
    }

    impl GitRepository for RecordingRepo {
        fn root(&self) -> &Path {
            Path::new(".")
        }

        fn status(&self, args: &[&str]) -> Result<i32, GitError> {
            self.status_calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| (*arg).to_owned()).collect());
            Ok(0)
        }

        fn output(&self, args: &[&str]) -> Result<Output, GitError> {
            self.outputs
                .get(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .cloned()
                .ok_or_else(|| GitError::CommandFailed {
                    command: args.join(" "),
                    code: -1,
                    stderr: format!("unexpected git args: {}", args.join(" ")),
                })
        }

        fn current_branch(&self) -> Result<Option<String>, GitError> {
            Ok(self.current_branch.clone())
        }
    }

    fn success_output(stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn exit_output(code: i32, stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn resolve_project_root_accepts_standard_track_items_layout() {
        assert_eq!(
            resolve_project_root(Path::new("repo/track/items")),
            Ok(std::path::PathBuf::from("repo"))
        );
    }

    #[test]
    fn resolve_project_root_rejects_non_standard_layout() {
        assert!(matches!(
            resolve_project_root(Path::new("repo/custom-items")),
            Err(err) if err.contains("track/items")
        ));
    }

    #[test]
    fn resolve_project_root_returns_dot_for_relative_track_items_path() {
        // When items_dir is the bare relative path "track/items" (no leading ancestor
        // component), Path::parent() resolves the grandparent to an empty path "".
        // resolve_project_root must return "." instead of "" so that callers can pass
        // the result to Command::current_dir without triggering ENOENT (empty cwd).
        assert_eq!(resolve_project_root(Path::new("track/items")), Ok(PathBuf::from(".")));
    }

    #[test]
    fn branch_create_git_commands_returns_switch_c_main_only() {
        // Regression guard (ADR 2026-04-22-1432 §D3): branch create must only emit
        // `git switch -c track/<id> main`. No commit, no add, no branch -f — any
        // additional command would risk generating a commit on main.
        let commands = branch_create_git_commands("track/demo");

        assert_eq!(
            commands,
            vec![vec![
                "switch".to_owned(),
                "-c".to_owned(),
                "track/demo".to_owned(),
                "main".to_owned(),
            ]]
        );
    }

    #[test]
    fn branch_create_execute_runs_only_switch_c_main_and_no_commit() {
        // Regression guard (ADR 2026-04-22-1432 §D1): the execute_branch(Create)
        // path must never invoke `git add` or `git commit`. If any future refactor
        // reintroduces metadata persistence into this path, this test fails.
        let repo = RecordingRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "rev-parse".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    "track/demo".to_owned(),
                ],
                exit_output(1, ""),
            )]),
            status_calls: Mutex::new(Vec::new()),
        };

        branch_create_execute(&repo, "track/demo").unwrap();

        let calls = repo.status_calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec![vec![
                "switch".to_owned(),
                "-c".to_owned(),
                "track/demo".to_owned(),
                "main".to_owned(),
            ]],
            "branch create must only execute `git switch -c`; any commit/add call is a regression"
        );
        assert!(
            !calls.iter().any(|args| args.first().map(String::as_str) == Some("commit")),
            "branch create must not invoke `git commit`"
        );
        assert!(
            !calls.iter().any(|args| args.first().map(String::as_str) == Some("add")),
            "branch create must not invoke `git add`"
        );
    }

    #[test]
    fn branch_create_execute_rejects_non_main_source_branch() {
        let repo = RecordingRepo {
            current_branch: Some("feature".to_owned()),
            outputs: HashMap::new(),
            status_calls: Mutex::new(Vec::new()),
        };

        let err = branch_create_execute(&repo, "track/demo").unwrap_err();
        assert!(err.contains("must start from 'main'"));
        assert!(
            repo.status_calls.lock().unwrap().is_empty(),
            "no git side-effects must happen when preflight fails"
        );
    }

    #[test]
    fn branch_create_execute_rejects_existing_branch() {
        let repo = RecordingRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "rev-parse".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    "track/demo".to_owned(),
                ],
                success_output("track/demo\n"),
            )]),
            status_calls: Mutex::new(Vec::new()),
        };

        let err = branch_create_execute(&repo, "track/demo").unwrap_err();
        assert!(err.contains("already exists"));
        assert!(
            repo.status_calls.lock().unwrap().is_empty(),
            "no git side-effects must happen when preflight fails"
        );
    }

    #[test]
    fn preflight_branch_operation_rejects_existing_divergent_branch_in_auto_mode() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("bbb\n"),
                ),
            ]),
        };

        let err = preflight_branch_operation(&repo, "track/demo", true).unwrap_err();

        assert!(err.contains("stale/divergent"));
    }

    #[test]
    fn preflight_branch_operation_allows_existing_aligned_branch() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
            ]),
        };

        let result = preflight_branch_operation(&repo, "track/demo", true);

        assert!(result.is_ok());
    }

    #[test]
    fn preflight_branch_operation_allows_switch_to_existing_branch_with_different_head() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "rev-parse".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    "track/demo".to_owned(),
                ],
                success_output("track/demo\n"),
            )]),
        };

        let result = preflight_branch_operation(&repo, "track/demo", false);

        assert!(result.is_ok());
    }

    #[test]
    fn preflight_branch_operation_rejects_existing_divergent_branch_when_alignment_required() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("bbb\n"),
                ),
            ]),
        };

        let err = preflight_branch_operation(&repo, "track/demo", true).unwrap_err();

        assert!(err.contains("stale/divergent"));
    }
}
