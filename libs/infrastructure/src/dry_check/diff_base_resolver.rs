//! Filesystem-backed adapter for [`usecase::fixpoint_resolve::DiffBaseResolverPort`].
//!
//! Relocated from `cli_composition::track::fixpoint_resolve` per ADR
//! 2026-06-21-1328 D7 (secondary port implementations belong in `libs/infrastructure`).
//!
//! Three-branch fail-closed diff-base resolution:
//!
//! 1. `FsDryCheckCommitHashStore::read()` → `Ok(Some(hash))`: use it.
//! 2. `Ok(None)` (file absent or non-ancestor): fall back to
//!    `git rev-parse <base_branch>`.
//! 3. `Err(...)`: emit `[warn]` and fall back to `git rev-parse <base_branch>`
//!    (absorbed; must not abort the gate).
//!
//! `base_branch` is the configured base branch name (e.g. `"main"`, `"develop"`)
//! taken from `metadata.json#branch_strategy_snapshot.base_branch`. It is passed
//! as a separate process argument (argv-style) and never interpolated into a
//! shell command string (AC-04 command-boundary safety).

use std::path::{Path, PathBuf};

use domain::CommitHash;
use usecase::fixpoint_resolve::{DiffBaseResolverError, DiffBaseResolverPort};

use crate::dry_check::{DryCheckCommitHashError, FsDryCheckCommitHashStore};
use crate::git_cli::{GitError, SystemGitRepo, isolated_bounded_git_output};

/// A resolved base is one commit hash and a refusal is one short line, so the
/// same small retention limit the other verdict-bearing probes use applies here.
const MAX_BASE_RESOLUTION_OUTPUT_BYTES: usize = 1024;
/// `check-ref-format --branch` echoes the accepted branch name, unlike the
/// fixed-size object id returned by `rev-parse`.
const MAX_BRANCH_VALIDATION_OUTPUT_BYTES: usize = 8 * 1024;

/// Filesystem adapter implementing [`DiffBaseResolverPort`].
///
/// Owns the configured base branch (from the active track's
/// `branch_strategy_snapshot`) and reads `.commit_hash` from the track directory,
/// falling back to `git rev-parse <base_branch>` per the three-branch policy.
pub struct FsDiffBaseResolverAdapter {
    base_branch: String,
}

impl FsDiffBaseResolverAdapter {
    /// Construct with the configured base branch.
    #[must_use]
    pub fn new(base_branch: String) -> Self {
        Self { base_branch }
    }
}

impl DiffBaseResolverPort for FsDiffBaseResolverAdapter {
    fn resolve_diff_base(
        &self,
        track_dir: &Path,
        canonical_root: &Path,
        repo_root: &Path,
    ) -> Result<CommitHash, DiffBaseResolverError> {
        let commit_hash_path = trusted_commit_hash_path(track_dir, canonical_root)
            .map_err(DiffBaseResolverError::Unavailable)?;
        let store = FsDryCheckCommitHashStore::new(commit_hash_path, canonical_root.to_path_buf());
        match store.read() {
            Ok(Some(hash)) => return Ok(hash),
            Ok(None) => {}
            Err(cause) => eprintln!("{}", degradation_warning(&cause, &self.base_branch)),
        }

        git_rev_parse_base(repo_root, &self.base_branch).map_err(DiffBaseResolverError::Unavailable)
    }
}

/// The warning printed when an unusable `.commit_hash` degrades the base to the
/// configured branch.
///
/// Built here rather than inline so the text can be asserted without capturing
/// stderr. It names the record by its well-known per-track name and says what was
/// wrong with it; the error's own message renders the absolute path it was reading
/// and is deliberately not embedded. Degradation itself is unchanged — the caller
/// still falls back and the gate still runs.
fn degradation_warning(cause: &DryCheckCommitHashError, base_branch: &str) -> String {
    format!(
        "[warn] fixpoint-resolve: .commit_hash {}; falling back to {base_branch}",
        crate::sanitized_failure::commit_hash_classification(cause)
    )
}

/// Resolves the track's `.commit_hash` location inside the trusted root.
///
/// Its failures travel to an operator through the callers' own messages, so each
/// names the record by its well-known per-track name and says what was wrong —
/// never the absolute path it was resolving, and never the underlying error, whose
/// own text renders one.
fn trusted_commit_hash_path(track_dir: &Path, trusted_root: &Path) -> Result<PathBuf, String> {
    let canonical_root = trusted_root
        .canonicalize()
        .map_err(|e| commit_hash_path_failure("the trusted root could not be resolved", &e))?;
    let absolute_track_dir = if track_dir.is_absolute() {
        track_dir.to_path_buf()
    } else {
        canonical_root.join(track_dir)
    };
    let canonical_track_dir = absolute_track_dir
        .canonicalize()
        .map_err(|e| commit_hash_path_failure("the track directory could not be resolved", &e))?;

    if !canonical_track_dir.starts_with(&canonical_root) {
        return Err(
            "cannot locate .commit_hash: the track directory resolves outside the trusted root"
                .to_owned(),
        );
    }

    let commit_hash_path = absolute_track_dir.join(".commit_hash");
    crate::track::symlink_guard::reject_symlinks_below(&commit_hash_path, &canonical_root)
        .map_err(|e| commit_hash_path_failure("the record's path was refused", &e))?;

    Ok(canonical_track_dir.join(".commit_hash"))
}

/// One shape for every `.commit_hash` location failure: what was being resolved,
/// and the sanitized classification of why it could not be.
fn commit_hash_path_failure(what: &str, cause: &std::io::Error) -> String {
    format!(
        "cannot locate .commit_hash: {what} ({})",
        crate::sanitized_failure::io_classification(cause)
    )
}

/// Runs one base-resolution git command, reporting a failure the way this
/// module's messages already do: the sanitized classification only, never git's
/// own text, which renders the repository path and its stderr.
///
/// The runner's own refusals — its deadline and its retention limit — arrive as
/// the same classification a failure to start does. Both mean the command
/// produced no answer, which is the only distinction the caller makes.
fn base_resolution_output(
    repo_root: &Path,
    args: &[&str],
) -> Result<std::process::Output, &'static str> {
    base_resolution_output_with_limit(repo_root, args, MAX_BASE_RESOLUTION_OUTPUT_BYTES)
}

fn base_resolution_output_with_limit(
    repo_root: &Path,
    args: &[&str],
    max_output_bytes: usize,
) -> Result<std::process::Output, &'static str> {
    isolated_bounded_git_output(repo_root, args, max_output_bytes).map_err(|source| {
        crate::sanitized_failure::git_classification(&GitError::Spawn {
            command: args.join(" "),
            source,
        })
    })
}

/// Run `git rev-parse <base_branch>` from `repo_root` and return the resulting
/// `CommitHash`. `base_branch` is passed argv-style (AC-04).
///
/// The branch name is the identifier an operator acts on and is named; git's own
/// error text is not, because it renders the repository path and its stderr.
///
/// Every step runs in the isolated, bounded lane the rest of the gate's git
/// reads use. The base decides what the measured diff is measured against, so an
/// ambient `GIT_DIR` or `GIT_NAMESPACE` selecting a crafted `main` — or a
/// replacement object standing in for its tip — would silently move the figure
/// the ceiling is compared with.
fn git_rev_parse_base(repo_root: &Path, base_branch: &str) -> Result<CommitHash, String> {
    // Discovery selects the repository; every command then runs from the same
    // directory discovery started at, never from the root it reported. A nested
    // repository configured with `core.worktree` makes `--show-toplevel` name an
    // enclosing checkout, and resolving the base from there would answer with
    // that checkout's `main`.
    SystemGitRepo::discover_from_isolated(repo_root).map_err(|e| {
        format!(
            "cannot resolve {base_branch}: {}",
            crate::sanitized_failure::git_classification(&e)
        )
    })?;
    // The configured value must name a local branch, not merely something git can
    // resolve. Two layers, because neither suffices alone:
    //
    // 1. `check-ref-format --branch` is git's own authority on what is a legal
    //    branch name. It refuses `HEAD`, `main~1`, `-x` and the rest of the
    //    revision grammar. It does expand `@{-1}`-style shorthands rather than
    //    rejecting them, which is why it is not the only layer.
    // 2. Resolving through the `refs/heads/` namespace requires a real local
    //    branch to exist under that exact name, which is what catches the
    //    shorthands layer 1 lets through. `--end-of-options` keeps a
    //    `-`-prefixed value out of option parsing, and the `^{commit}` peel with
    //    `--verify` requires exactly one commit.
    //
    // Prefixing alone would not do: `refs/heads/main~1^{commit}` still resolves,
    // so the name has to be validated as a name first.
    // `check-ref-format` accepts no option terminator (neither `--end-of-options`
    // nor `--`), but it needs none here: an option-shaped value makes it exit
    // nonzero, which is the refusal this wants.
    let checked = base_resolution_output_with_limit(
        repo_root,
        &["check-ref-format", "--branch", base_branch],
        MAX_BRANCH_VALIDATION_OUTPUT_BYTES,
    )
    .map_err(|e| format!("cannot resolve {base_branch}: {e}"))?;
    if !checked.status.success() {
        return Err(format!("cannot resolve {base_branch}: not a valid branch name"));
    }

    let revision = format!("refs/heads/{base_branch}^{{commit}}");
    let output = base_resolution_output(
        repo_root,
        &["rev-parse", "--verify", "--end-of-options", &revision],
    )
    .map_err(|e| format!("cannot resolve {base_branch}: {e}"))?;
    if !output.status.success() {
        return Err(format!("cannot resolve {base_branch}: git rev-parse refused it"));
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    // The validation failure describes the hash git printed, not any path.
    CommitHash::try_new(&sha)
        .map_err(|e| format!("cannot resolve {base_branch}: git printed an invalid hash ({e})"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_trusted_commit_hash_path_when_track_dir_is_under_root_returns_commit_hash_path() {
        let root = tempfile::tempdir().unwrap();
        let track_dir = root.path().join("track").join("items").join("track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        let path = trusted_commit_hash_path(&track_dir, root.path()).unwrap();

        assert_eq!(path, track_dir.canonicalize().unwrap().join(".commit_hash"));
    }

    #[test]
    fn test_trusted_commit_hash_path_when_track_dir_escapes_root_returns_error() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_dir = outside.path().join("track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        let err = trusted_commit_hash_path(&track_dir, root.path()).unwrap_err();

        assert!(err.contains("resolves outside the trusted root"), "got: {err}");
        assert!(err.contains(".commit_hash"), "names the record: {err}");
        assert!(
            !err.contains(&outside.path().display().to_string())
                && !err.contains(&root.path().display().to_string()),
            "neither path may reach the operator: {err}"
        );
    }

    #[test]
    fn test_the_degradation_warning_names_the_record_and_the_branch_but_no_path() {
        // Every variant renders the absolute path it was reading, so the warning
        // must not embed the error — only the record's well-known name, what was
        // wrong with it, and the branch the base degrades to.
        let causes = [
            DryCheckCommitHashError::Io {
                path: "/absolute/secret/track/items/some-track/.commit_hash".to_owned(),
                detail: "read: permission denied".to_owned(),
            },
            DryCheckCommitHashError::SymlinkDetected {
                path: "/absolute/secret/track/items/some-track/.commit_hash".to_owned(),
            },
            DryCheckCommitHashError::Format(
                "invalid commit hash in .commit_hash: not hexadecimal".to_owned(),
            ),
        ];

        for cause in &causes {
            // The two variants carrying a path render it; `Format` no longer does,
            // because its payload is built with the record's relative name.
            if matches!(
                cause,
                DryCheckCommitHashError::Io { .. }
                    | DryCheckCommitHashError::SymlinkDetected { .. }
            ) {
                assert!(
                    cause.to_string().contains("/absolute/secret"),
                    "the fixture must be an error that would leak: {cause}"
                );
            }
            let warning = degradation_warning(cause, "main");

            assert!(warning.contains(".commit_hash"), "names the record: {warning}");
            assert!(warning.contains("falling back to main"), "names the branch: {warning}");
            assert!(
                !warning.contains("/absolute/secret"),
                "no absolute path may reach stderr: {warning}"
            );
        }

        // The three causes stay distinguishable to whoever reads the warning.
        let words: Vec<String> =
            causes.iter().map(|cause| degradation_warning(cause, "main")).collect();
        let mut unique = words.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), words.len(), "each cause reads differently: {words:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_trusted_commit_hash_path_when_track_dir_is_symlink_returns_error() {
        let root = tempfile::tempdir().unwrap();
        let real_track_dir = root.path().join("real-track");
        let symlink_track_dir = root.path().join("track-link");
        std::fs::create_dir_all(&real_track_dir).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, &symlink_track_dir).unwrap();

        let err = trusted_commit_hash_path(&symlink_track_dir, root.path()).unwrap_err();

        assert!(err.contains("rejected as a symlink"), "got: {err}");
        assert!(err.contains(".commit_hash"), "names the record: {err}");
        assert!(
            !err.contains(&root.path().display().to_string()),
            "no absolute path may reach the operator: {err}"
        );
    }

    /// A repository with one commit on `main`.
    fn repo_with_a_commit() -> tempfile::TempDir {
        let repo = tempfile::Builder::new()
            .prefix("diff-base-resolver-repo-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        for args in [
            ["init", "-b", "main"].as_slice(),
            ["config", "user.email", "fixture@example.com"].as_slice(),
            ["config", "user.name", "fixture"].as_slice(),
            ["commit", "--allow-empty", "-m", "base"].as_slice(),
        ] {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} must succeed in the fixture");
        }
        repo
    }

    #[test]
    fn test_a_base_branch_that_looks_like_an_option_is_not_parsed_as_one() {
        let repo = repo_with_a_commit();

        // The configured branch names a real commit: resolution succeeds.
        let resolved = git_rev_parse_base(repo.path(), "main").unwrap();
        assert_eq!(resolved.as_ref().len(), 40, "a full commit hash is returned");

        // A value that git would otherwise read as an option. `--disambiguate=`
        // makes bare `rev-parse` exit zero having printed nothing useful, so
        // without the option terminator this would not even reach the hash check
        // as a failure.
        let err = git_rev_parse_base(repo.path(), "--disambiguate=0000")
            .expect_err("an option-shaped branch name must not be accepted as an option");

        assert!(err.contains("cannot resolve --disambiguate=0000"), "got: {err}");
        assert!(err.contains("not a valid branch name"), "refused as a name: {err}");
    }

    #[test]
    fn test_only_a_local_branch_name_resolves_as_the_base() {
        let repo = repo_with_a_commit();

        // Every one of these is something git can resolve to a commit, and none of
        // them is the configured base branch. `HEAD` is the dangerous one: on a
        // track branch it is the branch's own tip, which would zero the committed
        // half of the measured diff.
        for not_a_branch in ["HEAD", "main~1", "main^{tree}", "@{-1}", "refs/heads/main"] {
            let err = git_rev_parse_base(repo.path(), not_a_branch)
                .expect_err("only a local branch name may be accepted as the base: {not_a_branch}");
            assert!(err.contains(&format!("cannot resolve {not_a_branch}")), "got: {err}");
        }

        // The configured base itself still resolves.
        assert_eq!(git_rev_parse_base(repo.path(), "main").unwrap().as_ref().len(), 40);
    }

    #[test]
    fn test_a_long_valid_base_branch_is_not_limited_by_the_commit_reply_cap() {
        let repo = repo_with_a_commit();
        // Keep every path component below the filesystem's name limit while the
        // complete ref name exceeds the 1 KiB object-id reply cap.
        let base_branch = (0..8)
            .map(|segment| format!("segment-{segment}-{}", "x".repeat(180)))
            .collect::<Vec<_>>()
            .join("/");
        let status = std::process::Command::new("git")
            .args(["branch", &base_branch])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success(), "the fixture must create the long branch");
        assert!(base_branch.len() > MAX_BASE_RESOLUTION_OUTPUT_BYTES);

        assert_eq!(
            git_rev_parse_base(repo.path(), &base_branch).unwrap().as_ref().len(),
            40,
            "validation may echo a long name, but resolution still returns one object id"
        );
    }

    #[test]
    fn test_the_base_is_resolved_against_the_repository_selected_at_the_anchor() {
        // A repository nested inside another, configured so that its own
        // `--show-toplevel` names the enclosing checkout. Resolving from that
        // reported root would answer with the outer repository's `main` — a base
        // outside the measured branch's history, and a diff measured against the
        // wrong tree.
        let outer = repo_with_a_commit();
        let inner = outer.path().join("nested-repository");
        std::fs::create_dir_all(&inner).unwrap();
        for args in [
            ["init", "-b", "main"].as_slice(),
            ["config", "user.email", "fixture@example.com"].as_slice(),
            ["config", "user.name", "fixture"].as_slice(),
            ["commit", "--allow-empty", "-m", "inner base"].as_slice(),
        ] {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(&inner)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} must succeed in the inner fixture");
        }
        let inner_main = rev_parse(&inner, "refs/heads/main");
        let outer_main = rev_parse(outer.path(), "refs/heads/main");
        assert_ne!(inner_main, outer_main, "the two fixtures must have distinct tips");

        let status = std::process::Command::new("git")
            .args(["config", "core.worktree", outer.path().to_str().unwrap()])
            .current_dir(&inner)
            .status()
            .unwrap();
        assert!(status.success(), "the fixture must be able to redirect the reported worktree");
        assert_eq!(
            SystemGitRepo::discover_from(&inner).unwrap().root().canonicalize().unwrap(),
            outer.path().canonicalize().unwrap(),
            "core.worktree deliberately makes --show-toplevel name the enclosing directory"
        );

        let resolved = git_rev_parse_base(&inner, "main").unwrap();

        assert_eq!(
            resolved.as_ref(),
            inner_main,
            "the base must come from the repository the anchor selected"
        );
    }

    fn rev_parse(dir: &Path, revision: &str) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", revision])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "the fixture must resolve {revision}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn test_a_base_branch_that_cannot_be_resolved_is_reported_without_a_path() {
        // No repository encloses the system temp directory, so discovery fails —
        // the lane whose git error renders both the path it searched and git's
        // stderr.
        let elsewhere = std::env::temp_dir();

        let err = git_rev_parse_base(&elsewhere, "main").unwrap_err();

        assert!(err.contains("cannot resolve main"), "names the branch: {err}");
        assert!(
            !err.contains(&elsewhere.display().to_string()),
            "no absolute path may reach the operator: {err}"
        );
    }
}
