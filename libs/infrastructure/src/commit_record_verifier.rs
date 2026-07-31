//! Git secondary adapter verifying a commit hash before it is recorded against
//! a task (IN-22, AC-28, AC-29).
//!
//! [`GitCommitRecordVerifier`] answers one question about one hash: may this
//! repository record it? A hash qualifies when the repository holds it as a
//! commit object and that commit is reachable from `HEAD`; anything else is a
//! refusal, and a repository that cannot be read at all is reported as such
//! rather than as a refusal, so no caller can mistake an unperformed check for
//! an acceptance.
//!
//! Read-only: Git's lazy promisor fetching and replacement-object view are
//! disabled for every verification command, so checking an object can neither
//! fetch nor write it locally and always inspects the supplied object itself.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use domain::{CommitHash, FreeText};
use usecase::task_ops::{CommitRecordVerifierPort, CommitRecordVerifyError};

// The list of repository-selecting variables and the step that clears them live
// in `git_cli`: discovery and the verification commands that follow it must
// refuse the same environment, and one list is what keeps them from drifting
// apart.
use crate::git_cli::{
    SystemGitRepo, collect_bounded_git_output, spawn_bounded_git_child,
    terminate_bounded_git_child, without_history_rewrites, without_repository_selection,
};
use crate::sanitized_failure::{git_classification, io_classification};

/// Exit code `merge-base --is-ancestor` uses to say "not an ancestor". Every
/// other nonzero code is git failing rather than answering, and is reported as
/// an unreadable repository instead of as a refusal.
const NOT_AN_ANCESTOR: i32 = 1;
const MAX_VERIFICATION_GIT_OUTPUT_BYTES: usize = 1024;

fn unreadable(message: impl Into<String>) -> CommitRecordVerifyError {
    CommitRecordVerifyError::RepositoryUnreadable { message: FreeText::new(message.into()) }
}

/// Verifies a commit record through git.
///
/// Constructed with no arguments so composition roots stay zero-argument wiring
/// accessors; the items directory arrives with each call and names the
/// repository the track's artifacts live in.
#[derive(Debug, Default)]
pub struct GitCommitRecordVerifier;

impl GitCommitRecordVerifier {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> GitCommitRecordVerifier {
        GitCommitRecordVerifier
    }
}

impl CommitRecordVerifierPort for GitCommitRecordVerifier {
    fn verify_commit_record(
        &self,
        items_dir: &Path,
        commit_hash: &CommitHash,
    ) -> Result<(), CommitRecordVerifyError> {
        #[cfg(not(unix))]
        {
            let _ = (items_dir, commit_hash);
            return Err(unreadable("git commit verification requires Unix process-group support"));
        }

        // Anchored on the items directory, so the repository consulted is the one
        // the record would be written into — and isolated from the ambient Git
        // environment, which could otherwise name a different one.
        let (repo, anchor) =
            crate::discover_isolated_repo_for_items_dir(items_dir).map_err(|error| {
                unreadable(format!("git repository not discovered: {}", io_classification(&error)))
            })?;
        ensure_encloses(&repo, &anchor)?;

        if !holds_commit(&anchor, commit_hash)? {
            return Err(CommitRecordVerifyError::CommitNotFound {
                commit_hash: commit_hash.clone(),
            });
        }
        if !is_ancestor_of_head(&anchor, commit_hash)? {
            return Err(CommitRecordVerifyError::NotAncestorOfHead {
                commit_hash: commit_hash.clone(),
            });
        }
        Ok(())
    }
}

/// Refuses a repository that does not enclose the items directory the check was
/// asked about.
///
/// Discovery is supposed to walk up from the anchor, so its root is an ancestor
/// of it. This asserts that outcome rather than assuming it: a root elsewhere
/// means the answer would be about a different tree than the one the record is
/// written into, and there is no verdict to give about the tree that was asked
/// about. Both paths are canonical before they are compared, so a symlinked
/// checkout is not mistaken for a foreign one.
fn ensure_encloses(repo: &SystemGitRepo, anchor: &Path) -> Result<(), CommitRecordVerifyError> {
    let root = repo.root().canonicalize().map_err(|error| {
        unreadable(format!("repository root not resolved: {}", io_classification(&error)))
    })?;
    if anchor.starts_with(&root) {
        return Ok(());
    }
    // Neither path is named: both are absolute, and what an operator can act on
    // is that the two do not belong together.
    Err(unreadable("the discovered repository does not enclose the items directory"))
}

/// Builds a Git command used to verify a record.
///
/// Partial clones may lazily fetch a promised object for an ordinary read. A
/// verification must not make that network request or write fetched objects, so
/// its commands opt out explicitly.
fn verification_git_command(command_dir: &Path, args: &[&str]) -> Command {
    let mut command = crate::git_cli::guarded_git_command();
    command
        .env("GIT_NO_LAZY_FETCH", "1")
        .args(args)
        // Keep discovery anchored at the canonical items directory. In
        // particular, `core.worktree` can make `--show-toplevel` name an
        // enclosing path even though this directory's `.git` is the repository
        // that discovery selected.
        .current_dir(command_dir);
    without_repository_selection(&mut command);
    // The replacement-object and graft opt-outs are shared with base resolution
    // and measurement: one list, so a lane cannot be hardened while its
    // neighbours drift.
    without_history_rewrites(&mut command);
    command
}

/// Runs a one-line `cat-file --batch-check` query for the supplied object.
///
/// Batch mode gives a structured `missing` answer for an absent object. A
/// failing Git command is deliberately not treated as that answer: it means the
/// repository could not be read, rather than that it refused this hash.
fn object_type(
    command_dir: &Path,
    commit_hash: &CommitHash,
) -> Result<ObjectType, CommitRecordVerifyError> {
    const COMMAND: &str = "cat-file --batch-check=%(objectname) %(objecttype)";

    let mut command = verification_git_command(
        command_dir,
        &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
    );
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = spawn_bounded_git_child(&mut command)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    let mut stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            return Err(git_input_failure(
                COMMAND,
                std::io::Error::other("git stdin was not captured"),
                &mut child,
            ));
        }
    };
    if let Err(source) = writeln!(stdin, "{commit_hash}") {
        return Err(git_input_failure(COMMAND, source, &mut child));
    }
    drop(stdin);

    let output = collect_bounded_git_output(child, MAX_VERIFICATION_GIT_OUTPUT_BYTES)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    if !output.status.success() {
        return Err(unreadable("git cat-file: git command failed"));
    }

    parse_object_type_response(&output.stdout, commit_hash)
}

fn git_spawn_failure(command: &str, source: std::io::Error) -> CommitRecordVerifyError {
    unreadable(format!(
        "git {command}: {}",
        git_classification(&crate::git_cli::GitError::Spawn {
            command: command.to_owned(),
            source,
        })
    ))
}

fn git_input_failure(
    command: &str,
    source: std::io::Error,
    child: &mut std::process::Child,
) -> CommitRecordVerifyError {
    match terminate_bounded_git_child(child) {
        Ok(()) => git_spawn_failure(command, source),
        Err(cleanup) => unreadable(format!(
            "git {command}: input failed ({}); process cleanup failed ({})",
            io_classification(&source),
            io_classification(&cleanup),
        )),
    }
}

/// Whether the exact hash names a commit object.
///
/// The answer does not peel tags: an annotated tag that happens to point to an
/// ancestor is still a tag object, never a commit record.
fn holds_commit(
    command_dir: &Path,
    commit_hash: &CommitHash,
) -> Result<bool, CommitRecordVerifyError> {
    match object_type(command_dir, commit_hash)? {
        ObjectType::Commit => Ok(true),
        ObjectType::Missing => {
            classify_missing_object(command_dir, commit_hash)?;
            Ok(false)
        }
        ObjectType::NonCommit => Ok(false),
    }
}

/// One exact result from the `cat-file --batch-check` protocol.
#[derive(Debug, PartialEq, Eq)]
enum ObjectType {
    Commit,
    NonCommit,
    Missing,
}

/// Parses the complete, documented batch protocol response. Git ignores a
/// custom format for an absent object and emits `<object> missing`; accepting a
/// bare `missing` would classify malformed output as an ordinary absent hash.
fn parse_object_type_response(
    response: &[u8],
    commit_hash: &CommitHash,
) -> Result<ObjectType, CommitRecordVerifyError> {
    let response = std::str::from_utf8(response)
        .map_err(|_| unreadable("git cat-file: invalid protocol response"))?;
    let Some(line) = response.strip_suffix('\n') else {
        return Err(unreadable("git cat-file: invalid protocol response"));
    };
    let Some((object, kind)) = line.split_once(' ') else {
        return Err(unreadable("git cat-file: invalid protocol response"));
    };
    if line.contains('\n') || kind.is_empty() || kind.contains(char::is_whitespace) {
        return Err(unreadable("git cat-file: invalid protocol response"));
    }

    // Object names for ordinary type replies are expanded to a full id. Git's
    // documented special replies, however, echo an unresolved or ambiguous
    // abbreviation exactly as supplied, so validate those against the input.
    let is_special_reply = matches!(kind, "missing" | "ambiguous" | "excluded");
    if (is_special_reply && object != commit_hash.as_ref())
        || (!is_special_reply && !object_is_full_id_for(object, commit_hash))
    {
        return Err(unreadable("git cat-file: invalid protocol response"));
    }

    match kind {
        "commit" => Ok(ObjectType::Commit),
        "missing" => Ok(ObjectType::Missing),
        // These are the documented special replies. Neither identifies a
        // commit that may be recorded, but both are ordinary protocol answers.
        "ambiguous" | "excluded" | "tree" | "blob" | "tag" => Ok(ObjectType::NonCommit),
        _ => Err(unreadable("git cat-file: invalid protocol response")),
    }
}

fn object_is_full_id_for(object: &str, commit_hash: &CommitHash) -> bool {
    // CommitHash is a 7--40 character SHA-1 abbreviation. Batch output expands
    // it to the full id, so equality would reject every valid abbreviation.
    object.len() == 40
        && object.bytes().all(|byte| byte.is_ascii_hexdigit())
        && object.starts_with(commit_hash.as_ref())
}

/// Distinguishes an honestly absent object from storage that `cat-file` could
/// not inspect. Batch mode reports both as `<object> missing` and exits
/// successfully, so only this bounded one-object probe can keep repository
/// corruption from becoming a false ordinary absence.
fn classify_missing_object(
    command_dir: &Path,
    commit_hash: &CommitHash,
) -> Result<(), CommitRecordVerifyError> {
    const COMMAND: &str = "cat-file -e";
    let exact_hash = resolve_abbreviation(command_dir, commit_hash)?;
    let hash_to_check = exact_hash.as_deref().unwrap_or(commit_hash.as_ref());
    if commit_hash.as_ref().len() < 40 && exact_hash.is_none() {
        return Ok(());
    }
    let mut command = verification_git_command(command_dir, &["cat-file", "-e", hash_to_check]);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = spawn_bounded_git_child(&mut command)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    let output = collect_bounded_git_output(child, MAX_VERIFICATION_GIT_OUTPUT_BYTES)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    match output.status.code() {
        Some(1) if exact_hash.is_none() => Ok(()),
        // A successful exact lookup contradicts the batch response, while all
        // other exits mean object storage could not be checked.
        Some(0) => Err(unreadable("git cat-file: inconsistent object response")),
        _ => Err(unreadable("git cat-file: object lookup could not decide")),
    }
}

/// Resolves a short prefix without reading the object it names. A matching
/// loose object's name remains discoverable even when its contents are corrupt,
/// so only an empty or ambiguous inventory may be treated as an ordinary
/// unresolved abbreviation; one candidate is checked by its exact full id.
fn resolve_abbreviation(
    command_dir: &Path,
    commit_hash: &CommitHash,
) -> Result<Option<String>, CommitRecordVerifyError> {
    if commit_hash.as_ref().len() == 40 {
        return Ok(None);
    }

    const COMMAND: &str = "rev-parse --disambiguate";
    let option = format!("--disambiguate={}", commit_hash.as_ref());
    let mut command = verification_git_command(command_dir, &["rev-parse", &option]);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = spawn_bounded_git_child(&mut command)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    let output = collect_bounded_git_output(child, MAX_VERIFICATION_GIT_OUTPUT_BYTES)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    if !output.status.success() {
        return Err(unreadable("git rev-parse: object lookup could not decide"));
    }
    let output = std::str::from_utf8(&output.stdout)
        .map_err(|_| unreadable("git rev-parse: invalid object response"))?;
    let candidates: Vec<_> = output.lines().collect();
    if candidates.iter().any(|candidate| !object_is_full_id_for(candidate, commit_hash)) {
        return Err(unreadable("git rev-parse: invalid object response"));
    }
    Ok(match candidates.as_slice() {
        [candidate] => Some((*candidate).to_owned()),
        _ => None,
    })
}

/// Whether the commit is reachable from `HEAD`.
///
/// Only exit codes 0 and 1 are answers. `merge-base` exiting otherwise — an
/// unborn `HEAD`, a repository it cannot read — is a check that did not happen,
/// and is reported as such.
fn is_ancestor_of_head(
    command_dir: &Path,
    commit_hash: &CommitHash,
) -> Result<bool, CommitRecordVerifyError> {
    const COMMAND: &str = "merge-base --is-ancestor";

    let mut command = verification_git_command(
        command_dir,
        &["merge-base", "--is-ancestor", commit_hash.as_ref(), "HEAD"],
    );
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = spawn_bounded_git_child(&mut command)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    let output = collect_bounded_git_output(child, MAX_VERIFICATION_GIT_OUTPUT_BYTES)
        .map_err(|source| git_spawn_failure(COMMAND, source))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(NOT_AN_ANCESTOR) => Ok(false),
        // The code is the whole of what is reported: git writes paths into its
        // stderr, and the operator's actionable fact is that ancestry could not
        // be decided at all.
        other => Err(unreadable(format!(
            "git merge-base --is-ancestor could not decide (exit {})",
            other.unwrap_or(-1)
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::git_cli::isolation::REPOSITORY_SELECTING_GIT_ENV;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    /// A repository whose single commit carries `contents`, with the track's
    /// items directory materialised inside it.
    ///
    /// Each repository is seeded with content of its own: two fixtures assembled
    /// from identical trees in the same second produce identical commit ids,
    /// which would make one repository's hash resolvable in the other and leave
    /// the non-ancestor lane untested.
    fn fixture_repo(contents: &str) -> tempfile::TempDir {
        let repo = tempfile::Builder::new()
            .prefix("commit-record-verifier-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let root = repo.path();

        std::fs::create_dir_all(root.join("track/items/some-track")).unwrap();
        std::fs::write(root.join("history.txt"), contents).unwrap();

        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "fixture@example.com"]);
        git(root, &["config", "user.name", "fixture"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-m", "base"]);

        repo
    }

    fn items_dir(repo: &tempfile::TempDir) -> std::path::PathBuf {
        repo.path().join("track/items")
    }

    fn head_of(repo: &tempfile::TempDir) -> CommitHash {
        head_at(repo.path())
    }

    fn head_at(path: &Path) -> CommitHash {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "the fixture must have a resolvable HEAD");
        CommitHash::try_new(String::from_utf8(output.stdout).unwrap().trim()).unwrap()
    }

    fn write_loose_blob(repo: &tempfile::TempDir) -> CommitHash {
        let mut child = std::process::Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(repo.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.as_mut().unwrap().write_all(b"object to corrupt\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "the fixture must write a loose object");
        CommitHash::try_new(String::from_utf8(output.stdout).unwrap().trim()).unwrap()
    }

    fn annotated_tag_of(repo: &tempfile::TempDir) -> CommitHash {
        git(repo.path(), &["tag", "-a", "record-check-tag", "-m", "a tag is not a commit"]);
        let output = std::process::Command::new("git")
            .args(["rev-parse", "record-check-tag"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "the fixture must have an annotated tag");
        CommitHash::try_new(String::from_utf8(output.stdout).unwrap().trim()).unwrap()
    }

    fn absent_locally(repo: &tempfile::TempDir, commit_hash: &CommitHash) -> bool {
        !std::process::Command::new("git")
            .args(["cat-file", "-e", &format!("{}^{{commit}}", commit_hash.as_ref())])
            .current_dir(repo.path())
            .output()
            .unwrap()
            .status
            .success()
    }

    #[test]
    fn test_a_commit_reachable_from_head_may_be_recorded() {
        let repo = fixture_repo("the recording repository\n");
        let head = head_of(&repo);

        GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &head)
            .expect("a commit that is HEAD itself is reachable from HEAD");
    }

    #[test]
    fn test_a_well_formed_hash_the_repository_does_not_hold_is_not_found() {
        let repo = fixture_repo("the recording repository\n");
        // Well-formed and syntactically indistinguishable from a real hash: what
        // separates it is that this repository holds no such object.
        let absent = CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap();
        assert!(absent_locally(&repo, &absent), "the fixture must not hold the hash");

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &absent)
            .expect_err("a hash naming no commit object cannot be recorded");

        match error {
            CommitRecordVerifyError::CommitNotFound { commit_hash } => {
                assert_eq!(commit_hash, absent, "the refusal names the hash it refused");
            }
            other => panic!("an absent commit must be reported as not found, got {other:?}"),
        }
    }

    #[test]
    fn test_an_annotated_tag_to_head_is_not_accepted_as_a_commit() {
        let repo = fixture_repo("the recording repository\n");
        let tag = annotated_tag_of(&repo);

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &tag)
            .expect_err("a tag must not be accepted merely because it peels to HEAD");

        match error {
            CommitRecordVerifyError::CommitNotFound { commit_hash } => {
                assert_eq!(commit_hash, tag, "the refusal names the tag hash it refused");
            }
            other => panic!("an annotated tag must be refused before ancestry, got {other:?}"),
        }
    }

    #[test]
    fn test_verification_commands_disable_lazy_fetching() {
        let repo = fixture_repo("the recording repository\n");
        let git_repo = SystemGitRepo::discover_from(repo.path()).unwrap();
        let command = verification_git_command(
            git_repo.root(),
            &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
        );

        assert!(
            command.get_envs().any(|(key, value)| {
                key == std::ffi::OsStr::new("GIT_NO_LAZY_FETCH")
                    && value == Some(std::ffi::OsStr::new("1"))
            }),
            "every verification command must prohibit lazy promisor fetching"
        );
        assert!(
            command.get_envs().any(|(key, value)| {
                key == std::ffi::OsStr::new("GIT_NO_REPLACE_OBJECTS")
                    && value == Some(std::ffi::OsStr::new("1"))
            }),
            "every verification command must inspect un-replaced objects"
        );
        assert!(
            command.get_envs().any(|(key, value)| {
                key == std::ffi::OsStr::new("GIT_GRAFT_FILE")
                    && value == Some(std::ffi::OsStr::new("/dev/null"))
            }),
            "every verification command must disable graft-based history rewriting"
        );
        for variable in REPOSITORY_SELECTING_GIT_ENV {
            assert!(
                command.get_envs().any(|(key, value)| {
                    key == std::ffi::OsStr::new(variable) && value.is_none()
                }),
                "verification must not inherit {variable}, which can select another repository"
            );
        }
    }

    #[test]
    fn test_batch_protocol_requires_full_framing_and_accepts_a_full_id_for_an_abbreviation() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        let hash = CommitHash::try_new(full).unwrap();
        let abbreviated = CommitHash::try_new(&full[..12]).unwrap();

        assert_eq!(
            parse_object_type_response(
                b"0123456789abcdef0123456789abcdef01234567 missing\n",
                &hash
            )
            .unwrap(),
            ObjectType::Missing
        );
        assert_eq!(
            parse_object_type_response(b"0123456789abcdef0123456789abcdef01234567 commit\n", &hash)
                .unwrap(),
            ObjectType::Commit
        );
        assert_eq!(
            parse_object_type_response(
                b"0123456789abcdef0123456789abcdef01234567 commit\n",
                &abbreviated
            )
            .unwrap(),
            ObjectType::Commit,
            "Git expands a valid abbreviation to its full object ID"
        );
        assert_eq!(
            parse_object_type_response(b"0123456789ab missing\n", &abbreviated).unwrap(),
            ObjectType::Missing,
            "Git echoes an unresolved abbreviation in a documented special reply"
        );
        assert_eq!(
            parse_object_type_response(b"0123456789ab ambiguous\n", &abbreviated).unwrap(),
            ObjectType::NonCommit,
            "an ambiguous abbreviation is an ordinary refusal rather than malformed output"
        );
        assert!(
            parse_object_type_response(
                b"0123456789abcdef0123456789abcdef01234567 missing\n",
                &abbreviated
            )
            .is_err(),
            "special replies must echo an abbreviation exactly rather than expand it"
        );
        assert!(
            parse_object_type_response(b"missing\n", &hash).is_err(),
            "Git's batch protocol never emits a bare missing token"
        );
        assert!(
            parse_object_type_response(
                b"ffffffffffffffffffffffffffffffffffffffff missing\n",
                &hash
            )
            .is_err(),
            "the response must correspond to the requested object"
        );
        for malformed in [
            b"0123456789abcdef0123456789abcdef01234567 banana\n".as_slice(),
            b"0123456789abcdef0123456789abcdef01234567 commit".as_slice(),
            b"0123456789abcdef0123456789abcdef01234567 commit\nextra\n".as_slice(),
        ] {
            assert!(
                parse_object_type_response(malformed, &hash).is_err(),
                "only documented, newline-terminated protocol replies are accepted: {malformed:?}"
            );
        }
    }

    #[test]
    fn test_an_unresolved_abbreviation_is_reported_as_a_missing_commit() {
        let repo = fixture_repo("the recording repository\n");
        let unresolved = CommitHash::try_new("0123456").unwrap();

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &unresolved)
            .expect_err("an unresolved abbreviation must not be recordable");

        assert!(
            matches!(error, CommitRecordVerifyError::CommitNotFound { .. }),
            "an expected unresolved abbreviation is not repository damage: {error:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_a_corrupt_object_is_unreadable_not_an_absent_commit() {
        use std::os::unix::fs::PermissionsExt as _;

        let repo = fixture_repo("the recording repository\n");
        let object = write_loose_blob(&repo);
        let object_path = repo
            .path()
            .join(".git/objects")
            .join(&object.as_ref()[..2])
            .join(&object.as_ref()[2..]);
        let mut permissions = std::fs::metadata(&object_path).unwrap().permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(&object_path, permissions).unwrap();
        std::fs::write(&object_path, b"corrupt loose object").unwrap();

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &object)
            .expect_err("corrupt object storage cannot be reported as an ordinary absence");
        assert!(
            matches!(error, CommitRecordVerifyError::RepositoryUnreadable { .. }),
            "a failed secondary object lookup must make the repository unreadable: {error:?}"
        );

        let abbreviated = CommitHash::try_new(&object.as_ref()[..7]).unwrap();
        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &abbreviated)
            .expect_err("a corrupt object must stay unreadable through its short prefix");
        assert!(
            matches!(error, CommitRecordVerifyError::RepositoryUnreadable { .. }),
            "the abbreviated corruption lane must not degrade to an absent commit: {error:?}"
        );
    }

    #[test]
    fn test_a_commit_from_another_repository_is_refused_as_a_non_ancestor() {
        // A commit that exists as an object here but is not reachable from HEAD.
        // It is fetched from a second repository, so the object is genuinely
        // present rather than merely well-formed, which is what separates this
        // lane from the not-found one.
        let repo = fixture_repo("the recording repository\n");
        let foreign = fixture_repo("a history of its own\n");
        let foreign_head = head_of(&foreign);
        assert!(
            absent_locally(&repo, &foreign_head),
            "the two fixtures must not share a commit id, or the lane is untested"
        );

        git(repo.path(), &["fetch", "--no-tags", foreign.path().to_str().unwrap(), "main"]);
        assert!(
            !absent_locally(&repo, &foreign_head),
            "the fetched commit must be an object of the recording repository"
        );
        // Without the verification environment guard, both `cat-file` and
        // `merge-base` apply this replacement and wrongly accept foreign_head.
        let head = head_of(&repo);
        git(repo.path(), &["replace", foreign_head.as_ref(), head.as_ref()]);

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &foreign_head)
            .expect_err("a commit unreachable from HEAD cannot be recorded");

        match error {
            CommitRecordVerifyError::NotAncestorOfHead { commit_hash } => {
                assert_eq!(commit_hash, foreign_head, "the refusal names the hash it refused");
            }
            other => panic!("an unreachable commit must be refused for ancestry, got {other:?}"),
        }
    }

    #[test]
    fn test_verification_ignores_grafts_that_would_make_a_foreign_commit_an_ancestor() {
        let repo = fixture_repo("the recording repository\n");
        let foreign = fixture_repo("a history of its own\n");
        let foreign_head = head_of(&foreign);
        git(repo.path(), &["fetch", "--no-tags", foreign.path().to_str().unwrap(), "main"]);
        let head = head_of(&repo);
        std::fs::write(repo.path().join(".git/info/grafts"), format!("{head} {foreign_head}\n"))
            .unwrap();

        let inheriting_status = std::process::Command::new("git")
            .args(["merge-base", "--is-ancestor", foreign_head.as_ref(), "HEAD"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(
            inheriting_status.success(),
            "the fixture must show that a repository graft can forge ancestry"
        );

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &foreign_head)
            .expect_err("grafts must not influence admission verification");
        assert!(
            matches!(error, CommitRecordVerifyError::NotAncestorOfHead { .. }),
            "the isolated verifier must reject the truly unreachable commit: {error:?}"
        );
    }

    #[test]
    fn test_verification_stays_with_the_git_directory_discovered_from_the_anchor() {
        let outer = fixture_repo("the enclosing repository\n");
        let inner = outer.path().join("nested-repository");
        std::fs::create_dir_all(inner.join("track/items")).unwrap();
        git(&inner, &["init", "-b", "main"]);
        git(&inner, &["config", "user.email", "fixture@example.com"]);
        git(&inner, &["config", "user.name", "fixture"]);
        std::fs::write(inner.join("inner-history.txt"), "inner history\n").unwrap();
        git(&inner, &["add", "-A"]);
        git(&inner, &["commit", "-m", "inner base"]);
        let inner_head = head_at(&inner);
        git(&inner, &["config", "core.worktree", outer.path().to_str().unwrap()]);

        let anchor = inner.join("track/items");
        let reported_root = SystemGitRepo::discover_from(&anchor).unwrap();
        assert_eq!(
            reported_root.root().canonicalize().unwrap(),
            outer.path().canonicalize().unwrap(),
            "core.worktree deliberately makes --show-toplevel name the enclosing directory"
        );

        GitCommitRecordVerifier::new()
            .verify_commit_record(&anchor, &inner_head)
            .expect("verification must retain the inner repository selected at the anchor");
    }

    #[test]
    fn test_a_repository_that_does_not_enclose_the_items_directory_is_unreadable() {
        // Discovery walking up from the anchor cannot produce this, which is why
        // it is asserted rather than assumed: were a redirected discovery ever to
        // return a root elsewhere, the check would be about one tree while its
        // verdict decides what is written into another. The refusal is an
        // unreadable repository — nothing was judged — and names no path.
        let repo = fixture_repo("the recording repository\n");
        let elsewhere = fixture_repo("a history of its own\n");
        let discovered = SystemGitRepo::discover_from(elsewhere.path()).unwrap();
        let anchor = items_dir(&repo).canonicalize().unwrap();

        let error = ensure_encloses(&discovered, &anchor)
            .expect_err("a repository that does not enclose the anchor cannot answer for it");

        match error {
            CommitRecordVerifyError::RepositoryUnreadable { message } => {
                assert!(
                    message.as_str().contains("does not enclose"),
                    "the diagnostic names the mismatch: {message}"
                );
                assert!(
                    !message.as_str().contains(&elsewhere.path().display().to_string())
                        && !message.as_str().contains(&anchor.display().to_string()),
                    "no absolute path may reach the operator: {message}"
                );
            }
            other => panic!("a foreign repository must be unreadable, got {other:?}"),
        }

        // The ordinary case still passes: the repository the items directory
        // sits in encloses it.
        let own = SystemGitRepo::discover_from(repo.path()).unwrap();
        ensure_encloses(&own, &anchor).expect("the enclosing repository must be accepted");
    }

    #[test]
    fn test_verification_discovers_the_repository_the_items_directory_sits_in() {
        // The end of the same hazard the isolated discovery lane closes: with
        // `GIT_DIR` naming another repository, an inheriting discovery would
        // verify that repository's HEAD and record it here. What must never
        // happen is this hash being accepted for this items directory.
        let repo = fixture_repo("the recording repository\n");
        let elsewhere = fixture_repo("a history of its own\n");
        let foreign_head = head_of(&elsewhere);
        assert!(
            absent_locally(&repo, &foreign_head),
            "the two fixtures must not share a commit id, or the lane is untested"
        );

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(&items_dir(&repo), &foreign_head)
            .expect_err("another repository's HEAD is not recordable here");

        assert!(
            matches!(error, CommitRecordVerifyError::CommitNotFound { .. }),
            "the answer must be about this repository, got {error:?}"
        );
    }

    #[test]
    fn test_a_directory_no_repository_encloses_is_reported_as_unreadable() {
        // The check could not be performed at all, which is a different answer
        // from a refusal: the caller must not record the hash on the strength of
        // having received no refusal.
        let outside =
            tempfile::Builder::new().prefix("commit-record-verifier-outside-").tempdir().unwrap();
        let hash = CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap();

        let error = GitCommitRecordVerifier::new()
            .verify_commit_record(outside.path(), &hash)
            .expect_err("a directory outside any repository cannot yield a verdict");

        match error {
            CommitRecordVerifyError::RepositoryUnreadable { message } => {
                assert!(
                    message.as_str().contains("no enclosing git repository"),
                    "the diagnostic names why no verdict could be produced: {message}"
                );
                assert!(
                    !message.as_str().contains(&outside.path().display().to_string()),
                    "no absolute path may reach the operator: {message}"
                );
            }
            other => panic!("an undiscoverable repository must be unreadable, got {other:?}"),
        }
    }
}
