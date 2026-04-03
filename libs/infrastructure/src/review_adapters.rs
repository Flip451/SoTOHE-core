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
                    // Reject non-regular files (FIFO, device, etc.) to prevent
                    // hangs on read_to_end. Must check AFTER open to avoid TOCTOU.
                    let meta =
                        file.metadata().map_err(|e| format!("failed to stat {path}: {e}"))?;
                    if !meta.is_file() {
                        return Err(format!("scope path is not a regular file: {path}"));
                    }
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

fn filter_partition_to_group_names(
    full_partition: &usecase::review_workflow::groups::GroupPartition,
    group_names: &std::collections::BTreeSet<ReviewGroupName>,
) -> Result<usecase::review_workflow::groups::GroupPartition, RecordRoundProtocolError> {
    full_partition
        .remap_to_group_names(group_names)
        .map_err(|e| RecordRoundProtocolError::Other(format!("partition filter: {e}")))
}

fn ensure_expected_groups_supported(
    full_partition: &usecase::review_workflow::groups::GroupPartition,
    expected_groups: &[ReviewGroupName],
) -> Result<(), RecordRoundProtocolError> {
    let expected = normalized_expected_groups(expected_groups)?;
    let available: std::collections::BTreeSet<_> =
        full_partition.groups().keys().cloned().collect();
    if !expected.is_subset(&available) {
        let missing: Vec<_> = expected.difference(&available).map(ToString::to_string).collect();
        return Err(RecordRoundProtocolError::Other(format!(
            "expected_groups must be supported by the current partition (missing: {:?})",
            missing
        )));
    }
    Ok(())
}

fn normalized_expected_groups(
    expected_groups: &[ReviewGroupName],
) -> Result<std::collections::BTreeSet<ReviewGroupName>, RecordRoundProtocolError> {
    let other_key = ReviewGroupName::try_new("other")
        .map_err(|e| RecordRoundProtocolError::Other(format!("invalid group name 'other': {e}")))?;
    let mut normalized: std::collections::BTreeSet<_> = expected_groups.iter().cloned().collect();
    normalized.insert(other_key);
    Ok(normalized)
}

fn validate_expected_groups_against_cycle(
    cycle: &domain::ReviewCycle,
    expected_groups: &[ReviewGroupName],
) -> Result<(), RecordRoundProtocolError> {
    // This only verifies that the caller's requested review set fits within the
    // frozen cycle topology. Approval still keys off the cycle's full stored
    // group set via `ReviewCycle::all_groups_approved`, not `expected_groups`.
    let expected = normalized_expected_groups(expected_groups)?;
    let actual: std::collections::BTreeSet<_> = cycle.group_names().cloned().collect();
    let unexpected: Vec<_> = expected.difference(&actual).map(ToString::to_string).collect();
    if unexpected.is_empty() {
        return Ok(());
    }

    Err(RecordRoundProtocolError::Other(format!(
        "expected_groups must be a subset of the current cycle groups (unexpected: {:?})",
        unexpected
    )))
}

fn load_current_review_partition(
    git_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    diff_base: &str,
) -> Result<
    (
        crate::review_group_policy::ReviewScopeConfig,
        crate::review_group_policy::ResolvedReviewGroupPolicy,
        usecase::review_workflow::groups::GroupPartition,
    ),
    RecordRoundProtocolError,
> {
    use usecase::review_workflow::scope::DiffScopeProvider;

    let diff_scope = GitDiffScopeProvider
        .changed_files(diff_base)
        .map_err(|e| RecordRoundProtocolError::Other(format!("diff scope: {e}")))?;
    let scope_json_path = git_root.join("track/review-scope.json");
    let scope_config =
        crate::review_group_policy::load_review_scope_config(&scope_json_path, track_id)
            .map_err(|e| RecordRoundProtocolError::Other(format!("load review-scope.json: {e}")))?;
    let override_config = crate::review_group_policy::load_review_groups_override(
        items_dir, track_id,
    )
    .map_err(|e| RecordRoundProtocolError::Other(format!("load review-groups override: {e}")))?;
    let policy = crate::review_group_policy::ResolvedReviewGroupPolicy::resolve(
        &scope_config.groups,
        override_config.as_ref(),
    )
    .map_err(|e| RecordRoundProtocolError::Other(format!("resolve group policy: {e}")))?;
    let diff_files: Vec<_> = diff_scope.files().into_iter().cloned().collect();
    let filtered_files = crate::review_group_policy::filter_operational(
        &diff_files,
        &scope_config.operational_matchers,
    );
    let full_partition = policy
        .partition(&filtered_files)
        .map_err(|e| RecordRoundProtocolError::Other(format!("partition: {e}")))?;

    Ok((scope_config, policy, full_partition))
}

fn maybe_derive_stored_finding_concern(
    finding: &domain::StoredFinding,
) -> Result<ReviewConcern, RecordRoundProtocolError> {
    match finding.category() {
        Some(category) if category.trim().is_empty() => {
            return Err(RecordRoundProtocolError::Other(
                "derive finding concern: blank category is invalid".to_owned(),
            ));
        }
        Some(category) => {
            return ReviewConcern::try_new(category).map_err(|e| {
                RecordRoundProtocolError::Other(format!("derive finding concern: {e}"))
            });
        }
        None => {}
    }

    match finding.file() {
        Some(file) if file.trim().is_empty() => Err(RecordRoundProtocolError::Other(
            "derive finding concern: blank file is invalid".to_owned(),
        )),
        Some(file) => {
            let slug = domain::review::file_path_to_concern(file.trim()).to_lowercase();
            let concern = if slug.trim().is_empty() { "other" } else { slug.as_str() };
            ReviewConcern::try_new(concern).map_err(|e| {
                RecordRoundProtocolError::Other(format!("derive finding concern: {e}"))
            })
        }
        None => ReviewConcern::try_new("other")
            .map_err(|e| RecordRoundProtocolError::Other(format!("derive finding concern: {e}"))),
    }
}

fn derive_stored_finding_concerns(
    findings: &[domain::StoredFinding],
) -> Result<std::collections::BTreeSet<ReviewConcern>, RecordRoundProtocolError> {
    let mut set = std::collections::BTreeSet::new();
    for finding in findings {
        set.insert(maybe_derive_stored_finding_concern(finding)?);
    }
    Ok(set)
}

fn validate_stored_findings(
    findings: &[domain::StoredFinding],
) -> Result<(), RecordRoundProtocolError> {
    for finding in findings {
        if finding.message().trim().is_empty() {
            return Err(RecordRoundProtocolError::Other(
                "findings entries must include a non-empty `message`".to_owned(),
            ));
        }
        if finding.severity().is_some_and(|value| value.trim().is_empty()) {
            return Err(RecordRoundProtocolError::Other(
                "findings entries must use `severity: null` or a non-empty string".to_owned(),
            ));
        }
        if finding.file().is_some_and(|value| value.trim().is_empty()) {
            return Err(RecordRoundProtocolError::Other(
                "findings entries must use `file: null` or a non-empty string".to_owned(),
            ));
        }
        if finding.line() == Some(0) {
            return Err(RecordRoundProtocolError::Other(
                "findings entries must use `line: null` or a 1-based line number".to_owned(),
            ));
        }
        if finding.category().is_some_and(|value| value.trim().is_empty()) {
            return Err(RecordRoundProtocolError::Other(
                "findings entries must use `category: null` or a non-empty string".to_owned(),
            ));
        }
    }

    Ok(())
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
        findings: Vec<domain::StoredFinding>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError> {
        use crate::git_cli::{GitRepository, SystemGitRepo};
        use crate::review_json_store::FsReviewJsonStore;
        use domain::{GroupRoundVerdict, ReviewJson, ReviewJsonReader, ReviewJsonWriter};

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

        validate_stored_findings(&findings)?;
        if !expected_groups.contains(&group_name) {
            return Err(RecordRoundProtocolError::Other(format!(
                "group '{}' must be included in expected_groups",
                group_name.as_ref()
            )));
        }

        // Auto-create cycle if none exists, using real frozen scope from DiffScopeProvider.
        if review.current_cycle().is_none() {
            let cycle_id = timestamp.to_string();

            let (scope_config, policy, full_partition) = load_current_review_partition(
                git.root(),
                &self.items_dir,
                track_id,
                &self.base_ref,
            )?;
            // Compute base policy hash separately (before override).
            let base_policy = crate::review_group_policy::ResolvedReviewGroupPolicy::resolve(
                &scope_config.groups,
                None,
            )
            .map_err(|e| RecordRoundProtocolError::Other(format!("resolve base policy: {e}")))?;
            ensure_expected_groups_supported(&full_partition, &expected_groups)?;
            let snapshot = usecase::review_workflow::groups::ReviewPartitionSnapshot::new(
                base_policy.policy_hash(),
                policy.policy_hash(),
                full_partition,
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

        // Validate that the current group exists in the cycle (fail-fast).
        if let Some(cycle) = review.current_cycle() {
            validate_expected_groups_against_cycle(cycle, &expected_groups)?;
            if cycle.group(&group_name).is_none() {
                return Err(RecordRoundProtocolError::Other(format!(
                    "group '{}' not found in current cycle (available: {:?})",
                    group_name,
                    cycle.group_names().map(|n| n.to_string()).collect::<Vec<_>>()
                )));
            }
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
                if !findings.is_empty() {
                    return Err(RecordRoundProtocolError::Other(format!(
                        "inconsistent input: zero_findings verdict with {} findings",
                        findings.len()
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
                if findings.is_empty() {
                    return Err(RecordRoundProtocolError::Other(
                        "inconsistent input: findings_remain verdict with no findings".to_owned(),
                    ));
                }
                let derived_concerns = derive_stored_finding_concerns(&findings)?;
                let supplied_concerns: std::collections::BTreeSet<_> =
                    concerns.iter().cloned().collect();
                if !derived_concerns.is_subset(&supplied_concerns) {
                    return Err(RecordRoundProtocolError::Other(format!(
                        "inconsistent input: findings_remain concerns must include all findings-derived concerns (supplied: {:?}, derived: {:?})",
                        supplied_concerns, derived_concerns
                    )));
                }
                GroupRoundVerdict::findings_remain(findings).map_err(|e| {
                    RecordRoundProtocolError::Other(format!("verdict construction: {e}"))
                })?
            }
        };

        // Compute per-group scope hash from CURRENT partition (not frozen scope).
        // Uses effective_diff_base (approved_head if set, else base_ref) for
        // incremental scope computation to avoid re-reviewing previously committed files.
        let group_scope: Vec<String> = {
            let diff_base = review
                .current_cycle()
                .map(effective_diff_base)
                .unwrap_or_else(|| self.base_ref.clone());
            let (_, _, full_partition) =
                load_current_review_partition(git.root(), &self.items_dir, track_id, &diff_base)?;
            let filtered_partition = if let Some(cycle) = review.current_cycle() {
                let cycle_group_names: std::collections::BTreeSet<_> =
                    cycle.group_names().cloned().collect();
                filter_partition_to_group_names(&full_partition, &cycle_group_names)?
            } else {
                full_partition
            };

            filtered_partition
                .groups()
                .get(&group_name)
                .map(|paths| paths.iter().map(|p| p.as_str().to_owned()).collect())
                .unwrap_or_default()
        };
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
                concerns,
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
// Incremental diff base resolution
// ---------------------------------------------------------------------------

/// Resolves the effective diff base for review scope computation.
///
/// Prefers the cycle's `approved_head` (incremental base from the last approved commit)
/// over `base_ref` (typically "main"). Falls back to `base_ref` when:
/// - `approved_head` is `None` (first commit on the track branch)
/// - `approved_head` SHA is invalid (e.g., after a rebase)
///
/// This is fail-closed: fallback expands the scope to the cumulative branch diff.
pub fn effective_diff_base(cycle: &domain::ReviewCycle) -> String {
    match cycle.approved_head() {
        Some(head) => {
            // Verify approved_head is a valid commit AND an ancestor of HEAD.
            // Without ancestor check, a tampered approved_head could point to a
            // descendant/sibling commit, causing merge-base to collapse to HEAD
            // and silently shrinking the review scope (fail-open).
            let is_ancestor = std::process::Command::new("git")
                .args(["merge-base", "--is-ancestor", head.as_str(), "HEAD"])
                .output();
            match is_ancestor {
                Ok(output) if output.status.success() => head.as_str().to_owned(),
                _ => {
                    eprintln!(
                        "[incremental-scope] approved_head {} is not an ancestor of HEAD, \
                         falling back to base_ref {}",
                        head,
                        cycle.base_ref()
                    );
                    cycle.base_ref().to_owned()
                }
            }
        }
        None => cycle.base_ref().to_owned(),
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

    use domain::{ReviewJsonReader, ReviewJsonWriter, TrackWriter};
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

    fn sample_track(id: &str) -> domain::TrackMetadata {
        let task_id = domain::TaskId::try_new("T1").unwrap();
        let task = domain::TrackTask::new(task_id.clone(), "Implement feature").unwrap();
        let section = domain::PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = domain::PlanView::new(Vec::new(), vec![section]);

        domain::TrackMetadata::new(
            domain::TrackId::try_new(id).unwrap(),
            "Test Track",
            vec![task],
            plan,
            None,
        )
        .unwrap()
    }

    fn run_protocol_in_dir<F, T>(dir: &std::path::Path, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        let result = f();
        std::env::set_current_dir(original).unwrap();
        result
    }

    fn setup_record_round_fixture() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        domain::TrackId,
        RecordRoundProtocolImpl,
        domain::ReviewGroupName,
    ) {
        let repo = setup_test_repo();
        let path = repo.path().to_path_buf();

        std::fs::create_dir_all(path.join("track")).unwrap();
        std::fs::write(
            path.join("track/review-scope.json"),
            r#"{"groups": {}, "review_operational": ["track/items/<track-id>/review.json"]}"#,
        )
        .unwrap();

        let items_dir = path.join("track/items");
        let track_store = crate::track::fs_store::FsTrackStore::new(&items_dir);
        let track_id = domain::TrackId::try_new("test-track").unwrap();
        track_store.save(&sample_track(track_id.as_ref())).unwrap();

        let protocol = RecordRoundProtocolImpl {
            items_dir: items_dir.clone(),
            group_display: "Codex".to_owned(),
            base_ref: "main".to_owned(),
        };
        let group_name = domain::ReviewGroupName::try_new("other").unwrap();

        (repo, path, track_id, protocol, group_name)
    }

    fn setup_named_group_record_round_fixture(
        group_name: &str,
        scope_json: &str,
        changed_file: &str,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        domain::TrackId,
        RecordRoundProtocolImpl,
        domain::ReviewGroupName,
    ) {
        let repo = setup_test_repo();
        let path = repo.path().to_path_buf();

        std::fs::create_dir_all(path.join("track")).unwrap();
        std::fs::write(path.join("track/review-scope.json"), scope_json).unwrap();

        let changed_abs = path.join(changed_file);
        if let Some(parent) = changed_abs.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&changed_abs, "// changed").unwrap();

        let items_dir = path.join("track/items");
        let track_store = crate::track::fs_store::FsTrackStore::new(&items_dir);
        let track_id = domain::TrackId::try_new("test-track").unwrap();
        track_store.save(&sample_track(track_id.as_ref())).unwrap();

        let protocol = RecordRoundProtocolImpl {
            items_dir: items_dir.clone(),
            group_display: "Codex".to_owned(),
            base_ref: "main".to_owned(),
        };
        let group_name = domain::ReviewGroupName::try_new(group_name).unwrap();

        (repo, path, track_id, protocol, group_name)
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

    #[test]
    fn test_record_round_protocol_impl_findings_remain_persists_full_findings() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();
        let items_dir = path.join("track/items");
        let concerns = vec![domain::ReviewConcern::try_new("domain.review").unwrap()];
        let findings = vec![
            domain::StoredFinding::new(
                "P1: preserve me",
                Some("P1".to_owned()),
                Some("libs/domain/src/review.rs".to_owned()),
                Some(41),
            )
            .with_category(Some("domain.review".to_owned())),
        ];

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                concerns,
                findings.clone(),
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-02T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_ok(), "protocol should persist findings_remain verdict: {result:?}");

        let review = crate::review_json_store::FsReviewJsonStore::new(&items_dir)
            .find_review(&track_id)
            .unwrap()
            .unwrap();
        let round = review
            .current_cycle()
            .unwrap()
            .group(&group_name)
            .unwrap()
            .latest_round(domain::RoundType::Fast)
            .unwrap();

        match round.outcome() {
            domain::GroupRoundOutcome::Success(verdict) => {
                assert_eq!(verdict.findings(), findings.as_slice());
                assert_eq!(
                    round.concerns(),
                    &[domain::ReviewConcern::try_new("domain.review").unwrap()]
                );
            }
            outcome => panic!("expected successful round, got {outcome:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_zero_findings_with_concerns_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                Vec::new(),
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-02T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(
                    err.contains("zero_findings verdict with 1 concerns"),
                    "unexpected error: {err}"
                );
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_zero_findings_with_findings_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                Vec::new(),
                vec![domain::StoredFinding::new("P1: invalid", Some("P1".to_owned()), None, None)],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-02T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(
                    err.contains("zero_findings verdict with 1 findings"),
                    "unexpected error: {err}"
                );
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_without_concerns_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                Vec::new(),
                vec![domain::StoredFinding::new("P1: invalid", Some("P1".to_owned()), None, None)],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-02T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(
                    err.contains("findings_remain verdict with no concerns"),
                    "unexpected error: {err}"
                );
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_without_findings_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                Vec::new(),
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-02T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(
                    err.contains("findings_remain verdict with no findings"),
                    "unexpected error: {err}"
                );
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_mismatched_concerns_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                vec![
                    domain::StoredFinding::new(
                        "P1: invalid",
                        Some("P1".to_owned()),
                        Some("apps/cli/src/commands/review.rs".to_owned()),
                        None,
                    )
                    .with_category(Some("cli.review".to_owned())),
                ],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-02T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(
                    err.contains("must include all findings-derived concerns"),
                    "unexpected error: {err}"
                );
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_missing_metadata_derives_other_concern()
    {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();
        let items_dir = path.join("track/items");
        let concerns = vec![domain::ReviewConcern::try_new("other").unwrap()];
        let findings = vec![domain::StoredFinding::new(
            "P1: preserve free-form finding",
            Some("P1".to_owned()),
            None,
            None,
        )];

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                concerns,
                findings.clone(),
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(
            result.is_ok(),
            "protocol should preserve findings when opaque metadata falls back to `other`: {result:?}"
        );

        let review = crate::review_json_store::FsReviewJsonStore::new(&items_dir)
            .find_review(&track_id)
            .unwrap()
            .unwrap();
        let round = review
            .current_cycle()
            .unwrap()
            .group(&group_name)
            .unwrap()
            .latest_round(domain::RoundType::Fast)
            .unwrap();

        match round.outcome() {
            domain::GroupRoundOutcome::Success(verdict) => {
                assert_eq!(verdict.findings(), findings.as_slice());
                assert_eq!(round.concerns(), &[domain::ReviewConcern::try_new("other").unwrap()]);
            }
            outcome => panic!("expected successful round, got {outcome:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_accepts_mixed_case_file_concern() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("cli.foo").unwrap()],
                vec![domain::StoredFinding::new(
                    "P1: mixed case file path",
                    Some("P1".to_owned()),
                    Some("apps/CLI/src/Foo.rs".to_owned()),
                    Some(10),
                )],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_ok(), "mixed-case file-derived concern should normalize: {result:?}");
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_preserves_out_of_band_concerns() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();
        let items_dir = path.join("track/items");
        let concerns = vec![
            domain::ReviewConcern::try_new("cli.review").unwrap(),
            domain::ReviewConcern::try_new("domain.review").unwrap(),
            domain::ReviewConcern::try_new("other").unwrap(),
        ];
        let findings = vec![
            domain::StoredFinding::new(
                "P1: keep derived concern",
                Some("P1".to_owned()),
                Some("apps/cli/src/commands/review/codex_local.rs".to_owned()),
                Some(77),
            )
            .with_category(Some("cli.review".to_owned())),
            domain::StoredFinding::new(
                "P1: preserve free-form finding",
                Some("P1".to_owned()),
                None,
                None,
            ),
        ];

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                concerns.clone(),
                findings.clone(),
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_ok(), "extra out-of-band concerns should be preserved: {result:?}");

        let review = crate::review_json_store::FsReviewJsonStore::new(&items_dir)
            .find_review(&track_id)
            .unwrap()
            .unwrap();
        let round = review
            .current_cycle()
            .unwrap()
            .group(&group_name)
            .unwrap()
            .latest_round(domain::RoundType::Fast)
            .unwrap();

        match round.outcome() {
            domain::GroupRoundOutcome::Success(verdict) => {
                assert_eq!(verdict.findings(), findings.as_slice());
                assert_eq!(round.concerns(), concerns.as_slice());
            }
            outcome => panic!("expected successful round, got {outcome:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_blank_category_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                vec![
                    domain::StoredFinding::new(
                        "P1: invalid",
                        Some("P1".to_owned()),
                        Some("apps/cli/src/commands/review.rs".to_owned()),
                        None,
                    )
                    .with_category(Some(" ".to_owned())),
                ],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(err.contains("category"), "unexpected error: {err}");
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_blank_file_and_category_returns_error()
    {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                vec![
                    domain::StoredFinding::new(
                        "P1: invalid",
                        Some("P1".to_owned()),
                        Some(" ".to_owned()),
                        Some(1),
                    )
                    .with_category(Some("domain.review".to_owned())),
                ],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(err.contains("file"), "unexpected error: {err}");
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_blank_message_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                vec![domain::StoredFinding::new(
                    " ",
                    Some("P1".to_owned()),
                    Some("apps/cli/src/commands/review.rs".to_owned()),
                    Some(1),
                )],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(err.contains("message"), "unexpected error: {err}");
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_blank_severity_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                vec![domain::StoredFinding::new(
                    "P1: invalid",
                    Some(" ".to_owned()),
                    Some("apps/cli/src/commands/review.rs".to_owned()),
                    Some(1),
                )],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(err.contains("severity"), "unexpected error: {err}");
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_findings_remain_with_zero_line_returns_error() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::FindingsRemain,
                vec![domain::ReviewConcern::try_new("domain.review").unwrap()],
                vec![domain::StoredFinding::new(
                    "P1: invalid",
                    Some("P1".to_owned()),
                    Some("apps/cli/src/commands/review.rs".to_owned()),
                    Some(0),
                )],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T01:00:00Z").unwrap(),
            )
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(err.contains("line"), "unexpected error: {err}");
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    #[test]
    fn test_record_round_protocol_impl_excludes_review_json_from_round_hashes() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_record_round_fixture();
        let items_dir = path.join("track/items");

        std::fs::write(path.join("notes.txt"), "stays in other group").unwrap();

        let fast_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T02:00:00Z").unwrap(),
            )
        });
        assert!(fast_result.is_ok(), "fast round should succeed: {fast_result:?}");

        let review = crate::review_json_store::FsReviewJsonStore::new(&items_dir)
            .find_review(&track_id)
            .unwrap()
            .unwrap();
        let group_state = review.current_cycle().unwrap().group(&group_name).unwrap();
        assert!(
            !group_state.scope().iter().any(|path| path.ends_with("/review.json")),
            "review.json must be excluded from frozen scope: {:?}",
            group_state.scope()
        );
        let fast_hash =
            group_state.latest_round(domain::RoundType::Fast).unwrap().hash().to_owned();

        let final_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Final,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T02:05:00Z").unwrap(),
            )
        });
        assert!(final_result.is_ok(), "final round should succeed: {final_result:?}");

        let review = crate::review_json_store::FsReviewJsonStore::new(&items_dir)
            .find_review(&track_id)
            .unwrap()
            .unwrap();
        let final_hash = review
            .current_cycle()
            .unwrap()
            .group(&group_name)
            .unwrap()
            .latest_round(domain::RoundType::Final)
            .unwrap()
            .hash()
            .to_owned();
        assert_eq!(
            fast_hash, final_hash,
            "review.json write must not change the other-group hash between rounds"
        );
    }

    #[test]
    fn test_record_round_protocol_impl_rejects_expected_group_mismatch_for_existing_cycle() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_named_group_record_round_fixture(
            "infrastructure",
            r#"{
                "groups": {
                    "infrastructure": { "patterns": ["libs/infrastructure/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"]
            }"#,
            "libs/infrastructure/src/lib.rs",
        );

        let fast_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T03:00:00Z").unwrap(),
            )
        });
        assert!(fast_result.is_ok(), "fast round should succeed: {fast_result:?}");

        let mismatch_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Final,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                vec![domain::ReviewGroupName::try_new("cli").unwrap()],
                domain::Timestamp::new("2026-04-03T03:05:00Z").unwrap(),
            )
        });
        assert!(mismatch_result.is_err(), "mismatched expected_groups must fail closed");
        match mismatch_result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(
                    err.contains("must be included in expected_groups"),
                    "unexpected error: {err}"
                );
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // effective_diff_base tests
    // -----------------------------------------------------------------------

    fn make_cycle_with_approved_head(
        base_ref: &str,
        approved_head: Option<domain::ApprovedHead>,
    ) -> domain::ReviewCycle {
        let mut groups = std::collections::BTreeMap::new();
        let other = domain::ReviewGroupName::try_new("other").unwrap();
        groups.insert(other, domain::CycleGroupState::new(vec![]));
        domain::ReviewCycle::from_parts(
            "c1".into(),
            domain::Timestamp::new("2026-04-02T00:00:00Z").unwrap(),
            base_ref.into(),
            "sha256:abc".into(),
            "sha256:abc".into(),
            approved_head,
            groups,
        )
    }

    #[test]
    fn test_effective_diff_base_none_returns_base_ref() {
        let cycle = make_cycle_with_approved_head("main", None);
        assert_eq!(effective_diff_base(&cycle), "main");
    }

    #[test]
    fn test_effective_diff_base_valid_ancestor_returns_sha() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        // Get the initial commit SHA (which is an ancestor of HEAD on test-branch).
        let initial_sha =
            Command::new("git").args(["rev-parse", "main"]).current_dir(path).output().unwrap();
        let sha = String::from_utf8_lossy(&initial_sha.stdout).trim().to_owned();
        let approved = domain::ApprovedHead::try_new(&sha).unwrap();

        let cycle = make_cycle_with_approved_head("main", Some(approved));

        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let result = effective_diff_base(&cycle);
        std::env::set_current_dir(original).unwrap();

        assert_eq!(result, sha);
    }

    #[test]
    fn test_effective_diff_base_invalid_sha_falls_back() {
        let _guard = CWD_LOCK.lock().unwrap();
        let repo = setup_test_repo();
        let path = repo.path();

        let fake_sha =
            domain::ApprovedHead::try_new("0000000000000000000000000000000000000000").unwrap();
        let cycle = make_cycle_with_approved_head("main", Some(fake_sha));

        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let result = effective_diff_base(&cycle);
        std::env::set_current_dir(original).unwrap();

        assert_eq!(result, "main", "invalid SHA should fall back to base_ref");
    }

    #[test]
    fn test_filter_partition_to_expected_groups_preserves_remapped_paths_alongside_other_group() {
        let mut groups = std::collections::BTreeMap::new();
        groups.insert(
            ReviewGroupName::try_new("usecase").unwrap(),
            vec![RepoRelativePath::normalize("libs/usecase/src/lib.rs").unwrap()],
        );
        groups.insert(
            ReviewGroupName::try_new("harness-policy").unwrap(),
            vec![RepoRelativePath::normalize(".claude/commands/track/commit.md").unwrap()],
        );
        groups.insert(
            ReviewGroupName::try_new("other").unwrap(),
            vec![RepoRelativePath::normalize("Cargo.lock").unwrap()],
        );
        let full_partition =
            usecase::review_workflow::groups::GroupPartition::try_new(groups).unwrap();
        let expected = vec![
            ReviewGroupName::try_new("usecase").unwrap(),
            ReviewGroupName::try_new("other").unwrap(),
        ];
        let normalized_expected = normalized_expected_groups(&expected).unwrap();

        let filtered = filter_partition_to_group_names(&full_partition, &normalized_expected);

        assert!(filtered.is_ok(), "expected-group filter should succeed: {filtered:?}");
        let filtered = match filtered {
            Ok(filtered) => filtered,
            Err(err) => panic!("expected-group filter should succeed: {err:?}"),
        };
        let other_paths: Vec<_> = filtered.groups()[&ReviewGroupName::try_new("other").unwrap()]
            .iter()
            .map(|path| path.as_str())
            .collect();
        assert!(
            other_paths.contains(&".claude/commands/track/commit.md"),
            "remapped harness-policy path must be preserved in other: {other_paths:?}"
        );
        assert!(other_paths.contains(&"Cargo.lock"), "native other path must be preserved");
    }

    #[test]
    fn test_record_round_protocol_impl_starts_cycle_with_full_partition() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_named_group_record_round_fixture(
            "usecase",
            r#"{
                "groups": {
                    "cli": { "patterns": ["apps/cli/**"] },
                    "usecase": { "patterns": ["libs/usecase/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"]
            }"#,
            "libs/usecase/src/lib.rs",
        );
        let items_dir = path.join("track/items");
        let cli_group = ReviewGroupName::try_new("cli").unwrap();
        let expected_groups = vec![group_name.clone()];

        let fast_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                expected_groups.clone(),
                domain::Timestamp::new("2026-04-03T04:00:00Z").unwrap(),
            )
        });
        assert!(fast_result.is_ok(), "fast round should succeed: {fast_result:?}");

        let review = crate::review_json_store::FsReviewJsonStore::new(&items_dir)
            .find_review(&track_id)
            .unwrap()
            .unwrap();
        let cycle = review.current_cycle().unwrap();
        assert!(
            cycle.group(&cli_group).is_some(),
            "cycle should preserve empty groups from the active policy"
        );
        assert!(
            cycle.group(&ReviewGroupName::try_new("other").unwrap()).is_some(),
            "cycle must still preserve the mandatory other group"
        );
    }

    #[test]
    fn test_record_round_protocol_impl_allows_subset_expected_groups_against_subset_cycle() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_named_group_record_round_fixture(
            "usecase",
            r#"{
                "groups": {
                    "cli": { "patterns": ["apps/cli/**"] },
                    "usecase": { "patterns": ["libs/usecase/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"]
            }"#,
            "libs/usecase/src/lib.rs",
        );
        let expected_groups = vec![group_name.clone()];

        let fast_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                expected_groups.clone(),
                domain::Timestamp::new("2026-04-03T04:00:00Z").unwrap(),
            )
        });
        assert!(fast_result.is_ok(), "fast round should succeed: {fast_result:?}");

        let final_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Final,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                expected_groups.clone(),
                domain::Timestamp::new("2026-04-03T04:05:00Z").unwrap(),
            )
        });
        assert!(final_result.is_ok(), "final round should succeed: {final_result:?}");
    }

    #[test]
    fn test_ensure_expected_groups_supported_accepts_subset_of_partition_groups() {
        let mut groups = std::collections::BTreeMap::new();
        groups.insert(
            ReviewGroupName::try_new("cli").unwrap(),
            vec![RepoRelativePath::normalize("apps/cli/src/lib.rs").unwrap()],
        );
        groups.insert(
            ReviewGroupName::try_new("usecase").unwrap(),
            vec![RepoRelativePath::normalize("libs/usecase/src/lib.rs").unwrap()],
        );
        groups.insert(
            ReviewGroupName::try_new("other").unwrap(),
            vec![RepoRelativePath::normalize("Cargo.lock").unwrap()],
        );
        let full_partition =
            usecase::review_workflow::groups::GroupPartition::try_new(groups).unwrap();

        let result = ensure_expected_groups_supported(
            &full_partition,
            &[ReviewGroupName::try_new("usecase").unwrap()],
        );

        assert!(result.is_ok(), "subset support check should succeed: {result:?}");
    }

    #[test]
    fn test_record_round_protocol_impl_allows_truncated_expected_groups_for_active_cycle() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_named_group_record_round_fixture(
            "usecase",
            r#"{
                "groups": {
                    "cli": { "patterns": ["apps/cli/**"] },
                    "usecase": { "patterns": ["libs/usecase/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"]
            }"#,
            "libs/usecase/src/lib.rs",
        );
        let items_dir = path.join("track/items");
        let review_store = crate::review_json_store::FsReviewJsonStore::new(&items_dir);
        let mut review = domain::ReviewJson::new();
        let mut groups = std::collections::BTreeMap::new();
        groups.insert(
            ReviewGroupName::try_new("cli").unwrap(),
            domain::CycleGroupState::new(vec!["apps/cli/src/lib.rs".into()]),
        );
        groups.insert(
            group_name.clone(),
            domain::CycleGroupState::new(vec!["libs/usecase/src/lib.rs".into()]),
        );
        groups.insert(
            ReviewGroupName::try_new("other").unwrap(),
            domain::CycleGroupState::new(vec![]),
        );
        review
            .start_cycle(
                "2026-04-03T03:55:00Z",
                domain::Timestamp::new("2026-04-03T03:55:00Z").unwrap(),
                "main",
                "sha256:base",
                "sha256:effective",
                groups,
            )
            .unwrap();
        review_store.save_review(&track_id, &review).unwrap();

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                vec![group_name.clone()],
                domain::Timestamp::new("2026-04-03T04:00:00Z").unwrap(),
            )
        });
        assert!(result.is_ok(), "subset expected_groups should remain acceptable: {result:?}");
    }

    #[test]
    fn test_record_round_protocol_impl_accepts_existing_subset_cycle_after_upgrade() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_named_group_record_round_fixture(
            "usecase",
            r#"{
                "groups": {
                    "cli": { "patterns": ["apps/cli/**"] },
                    "usecase": { "patterns": ["libs/usecase/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"]
            }"#,
            "libs/usecase/src/lib.rs",
        );
        let items_dir = path.join("track/items");
        let review_store = crate::review_json_store::FsReviewJsonStore::new(&items_dir);
        let mut review = domain::ReviewJson::new();
        let mut groups = std::collections::BTreeMap::new();
        groups.insert(
            group_name.clone(),
            domain::CycleGroupState::new(vec!["libs/usecase/src/lib.rs".into()]),
        );
        groups.insert(
            ReviewGroupName::try_new("other").unwrap(),
            domain::CycleGroupState::new(vec![]),
        );
        review
            .start_cycle(
                "2026-04-03T03:55:00Z",
                domain::Timestamp::new("2026-04-03T03:55:00Z").unwrap(),
                "main",
                "sha256:base",
                "sha256:effective",
                groups,
            )
            .unwrap();
        review_store.save_review(&track_id, &review).unwrap();

        let expected_groups = vec![group_name.clone()];
        let fast_result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                expected_groups.clone(),
                domain::Timestamp::new("2026-04-03T04:00:00Z").unwrap(),
            )
        });
        assert!(
            fast_result.is_ok(),
            "existing subset cycle should remain recordable: {fast_result:?}"
        );
    }

    #[test]
    fn test_record_round_protocol_impl_rejects_unknown_expected_group() {
        let _guard = CWD_LOCK.lock().unwrap();
        let (_repo, path, track_id, protocol, group_name) = setup_named_group_record_round_fixture(
            "usecase",
            r#"{
                "groups": {
                    "cli": { "patterns": ["apps/cli/**"] },
                    "usecase": { "patterns": ["libs/usecase/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"]
            }"#,
            "libs/usecase/src/lib.rs",
        );
        let expected_groups =
            vec![group_name.clone(), ReviewGroupName::try_new("ghost-group").unwrap()];

        let result = run_protocol_in_dir(&path, || {
            protocol.execute(
                &track_id,
                domain::RoundType::Fast,
                group_name.clone(),
                domain::Verdict::ZeroFindings,
                vec![],
                vec![],
                expected_groups.clone(),
                domain::Timestamp::new("2026-04-03T04:00:00Z").unwrap(),
            )
        });
        assert!(result.is_err(), "unknown expected group must fail fast");
        match result.unwrap_err() {
            RecordRoundProtocolError::Other(err) => {
                assert!(err.contains("ghost-group"), "unexpected error: {err}");
            }
            err => panic!("expected Other error, got {err:?}"),
        }
    }
}
