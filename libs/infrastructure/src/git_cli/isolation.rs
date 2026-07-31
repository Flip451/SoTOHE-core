//! Repository isolation for the reads whose answers decide a gate.
//!
//! Part of the `git_cli` adapter, in its own file so that neither module
//! outgrows the workspace module-size limit. What lives here is one question:
//! given a directory, how is a git command made to answer about *that*
//! repository's recorded history and nothing else — not the repository the
//! environment names, and not the history the repository presents.

use std::path::Path;
use std::process::{Command, Output};

use super::bounded::{collect_bounded_git_output, spawn_bounded_git_child};
use super::guarded_git_command;

/// Git variables which may select a different repository, object database, or
/// ref namespace than the directory a command is aimed at.
///
/// `GIT_DIR` alone is enough to redirect a whole invocation: git documents it as
/// disabling ordinary discovery, so a command run inside one checkout can answer
/// about another.
pub(crate) const REPOSITORY_SELECTING_GIT_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_INDEX_FILE",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_SHALLOW_FILE",
];

/// Clears every variable by which the ambient environment could aim `command` at
/// a repository other than the one its working directory sits in.
///
/// This is deliberately a step a caller opts into rather than part of
/// [`guarded_git_command`]: the workflow commands are run by an operator who may
/// legitimately have pointed their environment at a repository, and taking that
/// away from them here would change what every existing call answers. The
/// callers that must not inherit it are the ones whose answer decides what gets
/// written into a specific tree.
pub(crate) fn without_repository_selection(command: &mut Command) {
    for variable in REPOSITORY_SELECTING_GIT_ENV {
        command.env_remove(variable);
    }
}

/// Makes `command` read the history as it is recorded, ignoring the two
/// mechanisms by which a repository can present a different one.
///
/// Replacement objects are honoured by ordinary reads — `diff`, `rev-parse` and
/// `merge-base` alike — so a `refs/replace` entry can make a base resolve to
/// HEAD or a diff come back empty. Grafts predate them and are a second input of
/// the same kind, from `info/grafts` or an inherited `GIT_GRAFT_FILE`; the empty
/// device disables both. Clearing the repository-selecting variables does not
/// cover either, because neither is a variable: they are repository state.
///
/// Like [`without_repository_selection`], this is opt-in. A command whose answer
/// decides a gate must see the real history; a command an operator runs for
/// themselves may legitimately see the presented one.
pub(crate) fn without_history_rewrites(command: &mut Command) {
    command.env("GIT_NO_REPLACE_OBJECTS", "1").env("GIT_GRAFT_FILE", "/dev/null");
}

/// Builds a git command that answers only about the repository at `dir`, with
/// the recorded history and nothing the environment has to say about either.
///
/// This is the shape every verdict-bearing read shares: discovery, base
/// resolution, ancestry and measurement all decide what a gate does, so all of
/// them refuse the same inputs.
pub(crate) fn isolated_git_command(dir: &Path, args: &[&str]) -> Command {
    let mut command = guarded_git_command();
    command
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    without_repository_selection(&mut command);
    without_history_rewrites(&mut command);
    command.env("GIT_NO_LAZY_FETCH", "1");
    command
}

/// Runs one [`isolated_git_command`] under the bounded runner.
///
/// # Errors
///
/// Returns the spawn failure, or the runner's deadline / retention refusal. A
/// nonzero exit is not an error here: the status travels back with the output so
/// each caller can classify its own command's refusal.
pub(crate) fn isolated_bounded_git_output(
    dir: &Path,
    args: &[&str],
    max_output_bytes: usize,
) -> std::io::Result<Output> {
    let mut command = isolated_git_command(dir, args);
    let child = spawn_bounded_git_child(&mut command)?;
    collect_bounded_git_output(child, max_output_bytes)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::ffi::OsStr;

    use super::{REPOSITORY_SELECTING_GIT_ENV, isolated_bounded_git_output, isolated_git_command};
    use crate::git_cli::{GUARDED_GIT_ENV, GUARDED_GIT_VALUE};
    use crate::verify::test_support::run_git;

    /// One repository with a commit of its own, so two fixtures have
    /// distinguishable histories.
    fn repo_with_a_commit(message: &str) -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        run_git(repo.path(), &["init"]);
        run_git(repo.path(), &["config", "user.email", "fixture@example.com"]);
        run_git(repo.path(), &["config", "user.name", "fixture"]);
        run_git(repo.path(), &["commit", "--allow-empty", "-m", message]);
        repo
    }

    fn head_of(dir: &std::path::Path) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "the fixture must have a resolvable HEAD");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn test_an_isolated_command_refuses_the_environment_and_the_presented_history() {
        // Every verdict-bearing read — discovery, base resolution, ancestry,
        // measurement — is built here, so this is the one place the whole shape
        // is asserted: nothing the environment says about which repository to
        // answer for, and nothing the repository says about what its history is.
        let command = isolated_git_command(std::path::Path::new("."), &["rev-parse", "HEAD"]);

        for variable in REPOSITORY_SELECTING_GIT_ENV {
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == OsStr::new(variable) && value.is_none()),
                "an isolated command must not inherit {variable}"
            );
        }
        assert!(
            command.get_envs().any(|(key, value)| {
                key == OsStr::new("GIT_NO_REPLACE_OBJECTS") && value == Some(OsStr::new("1"))
            }),
            "a replacement object must not be able to stand in for a commit"
        );
        assert!(
            command.get_envs().any(|(key, value)| {
                key == OsStr::new("GIT_GRAFT_FILE") && value == Some(OsStr::new("/dev/null"))
            }),
            "a graft must not be able to invent an ancestry"
        );
        assert!(
            command.get_envs().any(|(key, value)| {
                key == OsStr::new(GUARDED_GIT_ENV) && value == Some(OsStr::new(GUARDED_GIT_VALUE))
            }),
            "isolation must not drop the guard marker the hooks read"
        );
    }

    #[test]
    fn test_an_isolated_bounded_command_answers_about_the_directory_it_is_given() {
        let repo = repo_with_a_commit("the tree that is asked about");
        let elsewhere = repo_with_a_commit("a history of its own");

        let output =
            isolated_bounded_git_output(repo.path(), &["rev-parse", "HEAD"], 1024).unwrap();

        assert!(output.status.success(), "the bounded runner must carry the command through");
        let resolved = String::from_utf8(output.stdout).unwrap().trim().to_owned();
        assert_eq!(resolved, head_of(repo.path()));
        assert_ne!(resolved, head_of(elsewhere.path()));
    }
}
