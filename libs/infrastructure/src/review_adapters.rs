//! Infrastructure adapters for review workflow port traits.
//!
//! - `RecordRoundProtocolImpl`: atomic record-round protocol that writes to
//!   review.json with real frozen scope from DiffScopeProvider.
//! - `SystemGitHasher`: worktree content-based per-group scope hash.

use domain::{ReviewConcern, ReviewGroupName, RoundType, Timestamp, TrackId, Verdict};
use usecase::review_workflow::usecases::{
    GitHasher, RecordRoundProtocol, RecordRoundProtocolError,
};

/// Loads the `groups` field from `review-scope.json`.
///
/// Returns an empty map if the file lacks a `groups` field.
///
/// # Errors
/// Returns a string error on I/O or JSON parse failure.
pub fn load_base_review_groups(
    path: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, crate::review_group_policy::ReviewGroupConfig>, String>
{
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let doc: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let Some(groups_val) = doc.get("groups") else {
        return Ok(std::collections::BTreeMap::new());
    };
    serde_json::from_value(groups_val.clone())
        .map_err(|e| format!("parse groups in {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// GitHasher — thin delegation
// ---------------------------------------------------------------------------

/// Computes review-scope content hashes from worktree files.
pub struct SystemGitHasher;

impl GitHasher for SystemGitHasher {
    fn group_scope_hash(&self, scope: &[String]) -> Result<String, String> {
        use std::io::Read as _;

        use sha2::Digest;

        use crate::git_cli::{GitRepository, SystemGitRepo};

        if scope.is_empty() {
            let digest = sha2::Sha256::digest(b"");
            return Ok(format!("rvw1:sha256:{digest:x}"));
        }

        let git = SystemGitRepo::discover().map_err(|e| format!("git error: {e}"))?;
        let root = git.root().to_path_buf();

        // Sort scope paths for deterministic manifest.
        let mut sorted_scope: Vec<&str> = scope.iter().map(String::as_str).collect();
        sorted_scope.sort();

        // Build manifest from worktree file contents (git-independent).
        // Each entry: "<path>\t<sha256_of_content>\n" for existing files,
        //             "<path>\tDELETED\n" for missing files (tombstone).
        let mut manifest = String::new();
        for path in &sorted_scope {
            // Reject absolute paths (Unix and Windows) and parent traversal.
            // Check path segments (not substring) to allow valid names like "v1..v2.md".
            {
                let p = std::path::Path::new(path);
                let has_traversal_or_absolute = p.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                });
                if has_traversal_or_absolute {
                    return Err(format!("invalid scope path (traversal or absolute): {path}"));
                }
            }
            // Design: hash reads from worktree (not git index) per ADR §5.
            // Git is only used for diff detection; hash must be staging-independent.
            // Pre-commit workflow ensures add-all aligns index with worktree.
            let abs_path = root.join(path);
            match open_nofollow_read(&abs_path) {
                Ok(mut file) => {
                    // Post-open verification: check the opened fd's real path
                    // stays within repo root. This closes the TOCTOU gap between
                    // path resolution and file open — we verify the OPENED file,
                    // not the path we intended to open.
                    verify_fd_within_root(&file, &root, path)?;
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)
                        .map_err(|e| format!("failed to read {path}: {e}"))?;
                    let file_hash = sha2::Sha256::digest(&bytes);
                    manifest.push_str(&format!("{path}\t{file_hash:x}\n"));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    manifest.push_str(&format!("{path}\tDELETED\n"));
                }
                Err(e) => {
                    return Err(format!("failed to open {path}: {e}"));
                }
            }
        }

        let digest = sha2::Sha256::digest(manifest.as_bytes());
        Ok(format!("rvw1:sha256:{digest:x}"))
    }
}

// ---------------------------------------------------------------------------
// Post-open fd verification
// ---------------------------------------------------------------------------

/// Verifies that an opened file descriptor refers to a file inside `root`.
///
/// On Unix, uses `/proc/self/fd/<fd>` readlink to get the real path of the
/// opened file and checks it starts with `root`. This closes the TOCTOU gap
/// between path resolution and open.
///
/// On non-Unix, falls back to a no-op (O_NOFOLLOW + path component checks
/// are the best available defense).
///
/// # Errors
/// Returns an error string if the file escapes the repo root.
fn verify_fd_within_root(
    file: &std::fs::File,
    root: &std::path::Path,
    scope_path: &str,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let proc_path = format!("/proc/self/fd/{fd}");
        match std::fs::read_link(&proc_path) {
            Ok(real_path) => {
                let canon_root = root
                    .canonicalize()
                    .map_err(|e| format!("failed to canonicalize repo root: {e}"))?;
                if !real_path.starts_with(&canon_root) {
                    return Err(format!(
                        "scope path escapes repo root via symlink: {scope_path} \
                         (resolved to {})",
                        real_path.display()
                    ));
                }
            }
            Err(_) => {
                // /proc not available (e.g., macOS, FreeBSD).
                // Fall back to pre-open canonicalize check (TOCTOU possible
                // but best-effort on systems without /proc).
                let abs_path = root.join(scope_path);
                if let Ok(resolved) = abs_path.canonicalize() {
                    let canon_root = root
                        .canonicalize()
                        .map_err(|e| format!("failed to canonicalize repo root: {e}"))?;
                    if !resolved.starts_with(&canon_root) {
                        return Err(format!(
                            "scope path escapes repo root via symlink: {scope_path}"
                        ));
                    }
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (file, root, scope_path);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Symlink-safe file openers
// ---------------------------------------------------------------------------

/// Opens a file for reading, rejecting symlinks atomically via `O_NOFOLLOW`.
///
/// # Errors
///
/// Returns an error if the path is a symlink or cannot be opened.
fn open_nofollow_read(path: &std::path::Path) -> Result<std::fs::File, std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(path)
    }
    #[cfg(not(unix))]
    {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("path is a symlink: {}", path.display()),
                ));
            }
            Ok(_) | Err(_) => {}
        }
        std::fs::File::open(path)
    }
}

/// Opens a lock file safely, rejecting symlinks atomically via `O_NOFOLLOW`.
///
/// Uses `O_NOFOLLOW` to prevent symlink-following attacks without TOCTOU.
/// On non-Unix platforms, falls back to a symlink_metadata pre-check.
///
/// # Errors
///
/// Returns an error if the path is a symlink or cannot be opened.
fn open_lock_file_safe(path: &std::path::Path) -> Result<std::fs::File, std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        // Fallback: pre-check + open (TOCTOU possible but best-effort on non-Unix).
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("lock path is a symlink: {}", path.display()),
                ));
            }
            Ok(_) | Err(_) => {}
        }
        std::fs::OpenOptions::new().create(true).write(true).open(path)
    }
}

// ---------------------------------------------------------------------------
// RecordRoundProtocol
// ---------------------------------------------------------------------------

/// Atomic record-round protocol that writes to review.json via PrivateIndex.
///
/// On first invocation for a track, auto-creates a review cycle with the
/// expected groups. Subsequent calls append rounds to the existing cycle.
pub struct RecordRoundProtocolImpl {
    pub items_dir: std::path::PathBuf,
    pub group_display: String,
    pub base_ref: String,
}

impl RecordRoundProtocol for RecordRoundProtocolImpl {
    #[allow(clippy::too_many_lines)]
    fn execute(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError> {
        use domain::{
            GroupRoundVerdict, ReviewJson, ReviewJsonReader, ReviewJsonWriter, StoredFinding,
        };
        use usecase::review_workflow::scope::DiffScopeProvider;

        use crate::git_cli::{GitRepository, SystemGitRepo};
        use crate::review_json_store::FsReviewJsonStore;

        let git = SystemGitRepo::discover()
            .map_err(|e| RecordRoundProtocolError::Other(format!("git error: {e}")))?;

        // Acquire worktree-scoped exclusive advisory lock for the read-modify-write cycle.
        // Placed at worktree root (not .git/) to support linked worktrees where .git is a file.
        // Uses OpenOptions to reject symlinks (create_new fails if path exists as symlink).
        let lock_path = git.root().join(".sotp-record-round.lock");
        let lock_file = open_lock_file_safe(&lock_path).map_err(|e| {
            RecordRoundProtocolError::Other(format!(
                "failed to create lock file {}: {e}",
                lock_path.display()
            ))
        })?;
        {
            use fs4::fs_std::FileExt;
            lock_file.lock_exclusive().map_err(|e| {
                RecordRoundProtocolError::Other(format!(
                    "failed to acquire lock on {}: {e}",
                    lock_path.display()
                ))
            })?;
        }

        // Verify track exists and check escalation gate (fail-closed).
        {
            use crate::track::fs_store::FsTrackStore;
            use domain::TrackReader;
            let store = FsTrackStore::new(&self.items_dir);
            let track = store
                .find(track_id)
                .map_err(|e| {
                    RecordRoundProtocolError::Other(format!(
                        "track '{}' metadata.json read/decode failed: {e}",
                        track_id.as_ref()
                    ))
                })?
                .ok_or_else(|| {
                    RecordRoundProtocolError::Other(format!(
                        "track '{}' not found",
                        track_id.as_ref()
                    ))
                })?;

            // Escalation gate: reject if metadata.json has an active escalation block.
            if let Some(review_state) = track.review() {
                if let domain::EscalationPhase::Blocked(block) = review_state.escalation().phase() {
                    let concerns: Vec<_> =
                        block.concerns().iter().map(|c| c.as_ref().to_owned()).collect();
                    return Err(RecordRoundProtocolError::EscalationBlocked(concerns));
                }
            }
        }

        let rj_store = FsReviewJsonStore::new(&self.items_dir);

        // Load or create ReviewJson (under lock).
        let mut review = rj_store
            .find_review(track_id)
            .map_err(|e| RecordRoundProtocolError::Other(format!("read review.json: {e}")))?
            .unwrap_or_else(ReviewJson::new);

        // Auto-create cycle if none exists, using real frozen scope from DiffScopeProvider.
        if review.current_cycle().is_none() {
            let cycle_id = timestamp.to_string();

            // 1. Get changed files via DiffScopeProvider.
            let diff_scope = GitDiffScopeProvider
                .changed_files(&self.base_ref)
                .map_err(|e| RecordRoundProtocolError::Other(format!("diff scope: {e}")))?;

            // 2. Load group policy from review-scope.json (+ optional per-track override).
            let scope_json_path = git.root().join("track/review-scope.json");
            let base_groups = load_base_review_groups(&scope_json_path).map_err(|e| {
                RecordRoundProtocolError::Other(format!("load review-scope.json groups: {e}"))
            })?;
            let override_config =
                crate::review_group_policy::load_review_groups_override(&self.items_dir, track_id)
                    .map_err(|e| {
                        RecordRoundProtocolError::Other(format!("load review-groups override: {e}"))
                    })?;
            let policy = crate::review_group_policy::ResolvedReviewGroupPolicy::resolve(
                &base_groups,
                override_config.as_ref(),
            )
            .map_err(|e| RecordRoundProtocolError::Other(format!("resolve group policy: {e}")))?;

            // 3. Partition changed files into groups with real frozen scope.
            // Filter to expected_groups only (+ mandatory "other") to respect
            // the --expected-groups contract and avoid requiring review for
            // groups the caller didn't request.
            let diff_files: Vec<_> = diff_scope.files().into_iter().cloned().collect();
            let full_partition = policy
                .partition(&diff_files)
                .map_err(|e| RecordRoundProtocolError::Other(format!("partition: {e}")))?;
            let other_key = ReviewGroupName::try_new("other").map_err(|e| {
                RecordRoundProtocolError::Other(format!("invalid group name 'other': {e}"))
            })?;
            // Filter to expected_groups. Files from non-expected groups are
            // re-mapped to "other" so they are still covered by the review scope
            // (fail-closed: no files silently dropped).
            let mut filtered_groups = std::collections::BTreeMap::new();
            for (name, paths) in full_partition.groups() {
                if expected_groups.contains(name) || *name == other_key {
                    filtered_groups.insert(name.clone(), paths.clone());
                } else {
                    // Re-map to "other" so these files are not silently dropped.
                    filtered_groups
                        .entry(other_key.clone())
                        .or_default()
                        .extend(paths.iter().cloned());
                }
            }
            filtered_groups.entry(other_key).or_default();
            let partition =
                usecase::review_workflow::groups::GroupPartition::try_new(filtered_groups)
                    .map_err(|e| {
                        RecordRoundProtocolError::Other(format!("partition filter: {e}"))
                    })?;
            // Compute base policy hash separately (before override).
            let base_policy =
                crate::review_group_policy::ResolvedReviewGroupPolicy::resolve(&base_groups, None)
                    .map_err(|e| {
                        RecordRoundProtocolError::Other(format!("resolve base policy: {e}"))
                    })?;
            let snapshot = usecase::review_workflow::groups::ReviewPartitionSnapshot::new(
                base_policy.policy_hash(),
                policy.policy_hash(),
                partition,
            );

            // 4. Start cycle with real frozen scope via usecase function.
            usecase::review_workflow::cycle::start_review_cycle(
                &mut review,
                usecase::review_workflow::cycle::StartReviewCycleInput {
                    cycle_id,
                    started_at: timestamp.clone(),
                    base_ref: self.base_ref.clone(),
                    snapshot,
                },
            )
            .map_err(|e| RecordRoundProtocolError::Other(format!("start_cycle: {e}")))?;
        }

        // Build GroupRoundVerdict from the verdict + concerns (fail-closed validation).
        let group_verdict = match verdict {
            domain::Verdict::ZeroFindings => {
                if !concerns.is_empty() {
                    return Err(RecordRoundProtocolError::Other(format!(
                        "inconsistent input: zero_findings verdict with {} concerns",
                        concerns.len()
                    )));
                }
                GroupRoundVerdict::ZeroFindings
            }
            domain::Verdict::FindingsRemain => {
                if concerns.is_empty() {
                    return Err(RecordRoundProtocolError::Other(
                        "inconsistent input: findings_remain verdict with no concerns".to_owned(),
                    ));
                }
                let findings: Vec<StoredFinding> = concerns
                    .iter()
                    .map(|c| StoredFinding::new(c.as_ref(), None, None, None))
                    .collect();
                GroupRoundVerdict::findings_remain(findings).map_err(|e| {
                    RecordRoundProtocolError::Other(format!("verdict construction: {e}"))
                })?
            }
        };

        // Compute per-group scope hash from the frozen scope files in the cycle.
        let group_scope: Vec<String> = review
            .current_cycle()
            .and_then(|c| c.group(&group_name))
            .map(|g| g.scope().to_vec())
            .unwrap_or_default();
        let group_hash = SystemGitHasher
            .group_scope_hash(&group_scope)
            .map_err(|e| RecordRoundProtocolError::Other(format!("group scope hash error: {e}")))?;

        // Guard: final round requires a prior successful fast round for this group.
        if round_type == domain::RoundType::Final {
            if let Some(cycle) = review.current_cycle() {
                if let Some(group_state) = cycle.group(&group_name) {
                    let has_fast_zero = group_state
                        .latest_round(domain::RoundType::Fast)
                        .is_some_and(|r| r.is_successful_zero_findings());
                    if !has_fast_zero {
                        return Err(RecordRoundProtocolError::Other(
                            "final round requires a prior successful fast round".to_owned(),
                        ));
                    }
                }
            }
        }

        // Append round via usecase function (enforces timestamp monotonicity).
        usecase::review_workflow::cycle::record_cycle_group_round(
            &mut review,
            usecase::review_workflow::cycle::RecordCycleGroupRoundInput {
                group_name: group_name.clone(),
                round_type,
                timestamp: timestamp.clone(),
                outcome: usecase::review_workflow::cycle::RecordRoundOutcome::Success(
                    group_verdict,
                ),
                group_hash,
            },
        )
        .map_err(|e| RecordRoundProtocolError::Other(format!("record_cycle_group_round: {e}")))?;

        // Write review.json to disk atomically.
        // NOTE: review.json is NOT staged in the private index. It is a
        // review_operational file that should not affect the code hash.
        // Staging is handled by `cargo make add-all` at commit time.
        rj_store
            .save_review(track_id, &review)
            .map_err(|e| RecordRoundProtocolError::Other(format!("save review.json: {e}")))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GitDiffScopeProvider — Git-backed DiffScope adapter
// ---------------------------------------------------------------------------

use usecase::review_workflow::scope::{DiffScope, DiffScopeProviderError, RepoRelativePath};

/// Git-backed [`DiffScopeProvider`] using merge-base diff.
///
/// Computes the set of changed files by:
/// 1. Finding the merge-base between `HEAD` and `base_ref`.
/// 2. Diffing `HEAD` against that merge-base (`--diff-filter=ACDMRT`).
/// 3. Adding staged (cached) changes.
/// 4. Adding untracked (non-ignored) files.
pub struct GitDiffScopeProvider;

impl usecase::review_workflow::scope::DiffScopeProvider for GitDiffScopeProvider {
    fn changed_files(&self, base_ref: &str) -> Result<DiffScope, DiffScopeProviderError> {
        use crate::git_cli::{GitRepository, SystemGitRepo};

        let git = SystemGitRepo::discover()
            .map_err(|e| DiffScopeProviderError::Other(format!("git error: {e}")))?;

        // 1. Find merge-base between HEAD and base_ref.
        let merge_base_output = git
            .output(&["merge-base", "HEAD", base_ref])
            .map_err(|e| DiffScopeProviderError::Other(format!("merge-base failed: {e}")))?;

        if !merge_base_output.status.success() {
            return Err(DiffScopeProviderError::UnknownBaseRef { base_ref: base_ref.to_owned() });
        }

        let merge_base = String::from_utf8_lossy(&merge_base_output.stdout).trim().to_owned();

        let mut files = Vec::new();

        // Helper: collect paths from git output, propagating errors.
        let mut collect_paths =
            |output: std::process::Output, label: &str| -> Result<(), DiffScopeProviderError> {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                    return Err(DiffScopeProviderError::Other(format!(
                        "{label} failed (exit {}): {stderr}",
                        output.status.code().unwrap_or(-1)
                    )));
                }
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        if let Some(path) = RepoRelativePath::normalize(trimmed) {
                            files.push(path);
                        }
                    }
                }
                Ok(())
            };

        // 2. Files changed between merge-base and HEAD (committed, includes renames).
        let diff_output = git
            .output(&["diff", "--name-only", "--diff-filter=ACDMRT", &merge_base, "HEAD"])
            .map_err(|e| DiffScopeProviderError::Other(format!("diff failed: {e}")))?;
        collect_paths(diff_output, "diff merge-base..HEAD")?;

        // 3. Staged but uncommitted changes.
        let staged_output = git
            .output(&["diff", "--name-only", "--cached"])
            .map_err(|e| DiffScopeProviderError::Other(format!("staged diff failed: {e}")))?;
        collect_paths(staged_output, "diff --cached")?;

        // 4. Unstaged worktree modifications to tracked files.
        let worktree_output = git
            .output(&["diff", "--name-only"])
            .map_err(|e| DiffScopeProviderError::Other(format!("worktree diff failed: {e}")))?;
        collect_paths(worktree_output, "diff (worktree)")?;

        // 5. Untracked (non-ignored) files.
        let untracked_output = git
            .output(&["ls-files", "--others", "--exclude-standard"])
            .map_err(|e| DiffScopeProviderError::Other(format!("ls-files failed: {e}")))?;
        collect_paths(untracked_output, "ls-files --others")?;

        Ok(DiffScope::new(files))
    }
}

// ---------------------------------------------------------------------------
// GitDiffScopeProvider — contract tests with tempdir git fixtures
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::process::Command;
    use std::sync::Mutex;

    use usecase::review_workflow::scope::{DiffScopeProvider, DiffScopeProviderError};

    use super::*;

    // Tests that call `set_current_dir` MUST run serially to avoid interfering
    // with each other or with tests in other modules that depend on cwd.
    // We use a process-wide Mutex as a lightweight serial gate — any test that
    // changes cwd acquires this lock for the duration of the call.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Creates a temporary git repo with an initial commit on "main" and
    /// checks out a fresh "test-branch".  The returned `TempDir` must be kept
    /// alive for the duration of the test.
    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        let run = |args: &[&str]| {
            let out = Command::new("git").args(args).current_dir(path).output().unwrap();
            assert!(
                out.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        };

        run(&["init"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);

        // Initial file + commit.
        std::fs::write(path.join("README.md"), "initial").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "initial"]);
        // Ensure the default branch is named "main".
        run(&["branch", "-M", "main"]);
        // Create and switch to a test branch so that "main" is a valid base ref.
        run(&["checkout", "-b", "test-branch"]);

        dir
    }

    /// Runs `GitDiffScopeProvider::changed_files` with the cwd temporarily set
    /// to `dir`.  The `CWD_LOCK` must be held by the caller for the duration of
    /// this call.
    fn run_provider_in_dir(
        dir: &std::path::Path,
        base_ref: &str,
    ) -> Result<DiffScope, DiffScopeProviderError> {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        let result = GitDiffScopeProvider.changed_files(base_ref);
        std::env::set_current_dir(original).unwrap();
        result
    }

    /// Returns `true` if `scope` contains a [`RepoRelativePath`] for `raw`.
    fn scope_contains(scope: &DiffScope, raw: &str) -> bool {
        RepoRelativePath::normalize(raw).is_some_and(|p| scope.contains(&p))
    }

    // -----------------------------------------------------------------------
    // Contract tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_diff_scope_includes_committed_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Commit a new file on the test branch.
        std::fs::write(path.join("new_feature.rs"), "pub fn hello() {}").unwrap();
        Command::new("git").args(["add", "new_feature.rs"]).current_dir(path).output().unwrap();
        Command::new("git")
            .args(["commit", "-m", "add new_feature"])
            .current_dir(path)
            .output()
            .unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "new_feature.rs"), "committed file should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_staged_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Stage a new file without committing.
        std::fs::write(path.join("staged.rs"), "// staged").unwrap();
        Command::new("git").args(["add", "staged.rs"]).current_dir(path).output().unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "staged.rs"), "staged file should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_unstaged_worktree_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Modify a tracked file without staging.
        std::fs::write(path.join("README.md"), "modified").unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(
            scope_contains(&scope, "README.md"),
            "unstaged worktree modification should appear in scope"
        );
    }

    #[test]
    fn test_diff_scope_includes_untracked_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Create a new file without staging it.
        std::fs::write(path.join("untracked.txt"), "not staged").unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "untracked.txt"), "untracked file should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_renamed_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Rename a tracked file and commit it (tests the merge-base..HEAD diff path).
        Command::new("git")
            .args(["mv", "README.md", "RENAMED.md"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git").args(["commit", "-m", "rename"]).current_dir(path).output().unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "RENAMED.md"), "renamed destination should appear in scope");
    }

    #[test]
    fn test_diff_scope_includes_deleted_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Delete the tracked file and commit it (tests the merge-base..HEAD diff path).
        Command::new("git").args(["rm", "README.md"]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "delete"]).current_dir(path).output().unwrap();

        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope_contains(&scope, "README.md"), "deleted file should appear in scope");
    }

    #[test]
    fn test_diff_scope_error_on_invalid_base_ref() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let result = run_provider_in_dir(path, "nonexistent-branch-xyz-999");
        match result {
            Err(DiffScopeProviderError::UnknownBaseRef { base_ref }) => {
                assert_eq!(base_ref, "nonexistent-branch-xyz-999");
            }
            other => panic!("expected UnknownBaseRef, got {other:?}"),
        }
    }

    #[test]
    fn test_diff_scope_empty_for_no_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // No changes from base — scope should be empty.
        let scope = run_provider_in_dir(path, "main").unwrap();
        assert!(scope.is_empty(), "scope should be empty when there are no branch changes");
    }

    // -----------------------------------------------------------------------
    // SystemGitHasher::group_scope_hash contract tests
    // -----------------------------------------------------------------------

    use usecase::review_workflow::usecases::GitHasher;

    fn run_hasher_in_dir(dir: &std::path::Path, scope: &[String]) -> Result<String, String> {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        let result = SystemGitHasher.group_scope_hash(scope);
        std::env::set_current_dir(original).unwrap();
        result
    }

    #[test]
    fn test_group_scope_hash_empty_scope_returns_deterministic_hash() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let hash1 = run_hasher_in_dir(path, &[]).unwrap();
        let hash2 = run_hasher_in_dir(path, &[]).unwrap();
        assert!(hash1.starts_with("rvw1:sha256:"), "hash should have rvw1 prefix: {hash1}");
        assert_eq!(hash1, hash2, "empty scope hash must be deterministic");
    }

    #[test]
    fn test_group_scope_hash_deterministic_for_same_worktree_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        std::fs::write(path.join("feature.rs"), "pub fn feature() {}").unwrap();

        let scope = vec!["feature.rs".to_owned()];
        let hash1 = run_hasher_in_dir(path, &scope).unwrap();
        let hash2 = run_hasher_in_dir(path, &scope).unwrap();
        assert_eq!(hash1, hash2, "same file content must produce same hash");
        assert!(hash1.starts_with("rvw1:sha256:"));
    }

    #[test]
    fn test_group_scope_hash_changes_when_content_changes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let scope = vec!["feature.rs".to_owned()];

        std::fs::write(path.join("feature.rs"), "pub fn v1() {}").unwrap();
        let hash_v1 = run_hasher_in_dir(path, &scope).unwrap();

        std::fs::write(path.join("feature.rs"), "pub fn v2() {}").unwrap();
        let hash_v2 = run_hasher_in_dir(path, &scope).unwrap();

        assert_ne!(hash_v1, hash_v2, "different content must produce different hash");
    }

    #[test]
    fn test_group_scope_hash_only_includes_scope_files() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        std::fs::write(path.join("a.rs"), "pub fn a() {}").unwrap();
        std::fs::write(path.join("b.rs"), "pub fn b() {}").unwrap();

        let scope_a = vec!["a.rs".to_owned()];
        let scope_ab = vec!["a.rs".to_owned(), "b.rs".to_owned()];

        let hash_a = run_hasher_in_dir(path, &scope_a).unwrap();
        let hash_ab = run_hasher_in_dir(path, &scope_ab).unwrap();

        assert_ne!(hash_a, hash_ab, "different scope sets must produce different hashes");
    }

    #[test]
    fn test_group_scope_hash_deleted_file_produces_tombstone() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        std::fs::write(path.join("exists.rs"), "pub fn ok() {}").unwrap();

        let scope = vec!["exists.rs".to_owned(), "deleted.rs".to_owned()];
        let hash_with_deleted = run_hasher_in_dir(path, &scope).unwrap();

        let scope_only = vec!["exists.rs".to_owned()];
        let hash_without_deleted = run_hasher_in_dir(path, &scope_only).unwrap();

        // Missing files get DELETED tombstone, so the hashes differ.
        assert_ne!(
            hash_with_deleted, hash_without_deleted,
            "missing file should contribute a DELETED tombstone to the hash"
        );
    }

    #[test]
    fn test_group_scope_hash_rejects_absolute_path() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let scope = vec!["/etc/passwd".to_owned()];
        let result = run_hasher_in_dir(path, &scope);
        assert!(result.is_err(), "absolute path must be rejected");
        assert!(result.unwrap_err().contains("traversal or absolute"));
    }

    #[test]
    fn test_group_scope_hash_rejects_parent_traversal() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let scope = vec!["../../../etc/passwd".to_owned()];
        let result = run_hasher_in_dir(path, &scope);
        assert!(result.is_err(), "parent traversal must be rejected");
        assert!(result.unwrap_err().contains("traversal or absolute"));
    }

    #[test]
    fn test_group_scope_hash_allows_double_dot_in_filename() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // File with ".." in its name (not a traversal).
        std::fs::write(path.join("v1..v2.md"), "changelog").unwrap();

        let scope = vec!["v1..v2.md".to_owned()];
        let result = run_hasher_in_dir(path, &scope);
        assert!(result.is_ok(), "double-dot in filename must be allowed: {result:?}");
    }

    #[test]
    fn test_group_scope_hash_not_affected_by_staging() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Write a file but do NOT stage it.
        std::fs::write(path.join("unstaged.rs"), "pub fn unstaged() {}").unwrap();
        let scope = vec!["unstaged.rs".to_owned()];
        let hash_unstaged = run_hasher_in_dir(path, &scope).unwrap();

        // Now stage the same file — hash should not change.
        Command::new("git").args(["add", "unstaged.rs"]).current_dir(path).output().unwrap();
        let hash_staged = run_hasher_in_dir(path, &scope).unwrap();

        assert_eq!(hash_unstaged, hash_staged, "hash must not be affected by git staging state");
    }
}
