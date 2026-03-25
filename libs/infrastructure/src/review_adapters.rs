//! Infrastructure adapters for review workflow port traits.
//!
//! - `RecordRoundProtocolImpl`: the genuinely complex two-phase git index
//!   commit protocol (PrivateIndex + stage + hash + swap).
//! - `SystemGitHasher`: thin delegation to `SystemGitRepo` for normalised hash.

use std::path::Path;

use domain::{ReviewConcern, ReviewGroupName, RoundType, Timestamp, TrackId, Verdict};
use usecase::review_workflow::usecases::{
    GitHasher, RecordRoundProtocol, RecordRoundProtocolError,
};

// ---------------------------------------------------------------------------
// GitHasher — thin delegation
// ---------------------------------------------------------------------------

/// Computes normalised git tree hashes via `SystemGitRepo`.
pub struct SystemGitHasher;

impl GitHasher for SystemGitHasher {
    fn normalized_hash(&self, items_dir: &Path, track_id: &TrackId) -> Result<String, String> {
        use crate::git_cli::{GitRepository, SystemGitRepo};

        let git = SystemGitRepo::discover().map_err(|e| format!("git error: {e}"))?;
        let metadata_abs = items_dir.join(track_id.as_ref()).join("metadata.json");
        let metadata_rel = metadata_abs
            .strip_prefix(git.root())
            .unwrap_or(&metadata_abs)
            .to_string_lossy()
            .into_owned();

        git.index_tree_hash_normalizing(&metadata_rel).map_err(|e| format!("{e}"))
    }
}

// ---------------------------------------------------------------------------
// RecordRoundProtocol — two-phase git index commit
// ---------------------------------------------------------------------------

/// Atomic two-phase record-round protocol using PrivateIndex.
pub struct RecordRoundProtocolImpl {
    pub items_dir: std::path::PathBuf,
    pub group_display: String,
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
        use domain::{ReviewRoundResult, ReviewState};

        use crate::git_cli::private_index::PrivateIndex;
        use crate::git_cli::{GitRepository, SystemGitRepo};
        use crate::track::fs_store::FsTrackStore;

        let git = SystemGitRepo::discover()
            .map_err(|e| RecordRoundProtocolError::Other(format!("git error: {e}")))?;

        let metadata_abs = self.items_dir.join(track_id.as_ref()).join("metadata.json");
        let metadata_rel = metadata_abs
            .strip_prefix(git.root())
            .unwrap_or(&metadata_abs)
            .to_string_lossy()
            .into_owned();

        let store = FsTrackStore::new(&self.items_dir);

        let private_index =
            PrivateIndex::from_current(&git).map_err(RecordRoundProtocolError::Other)?;

        let pre_update_hash = private_index
            .normalized_tree_hash(&git, &metadata_rel)
            .map_err(|e| RecordRoundProtocolError::Other(format!("normalized hash error: {e}")))?;

        let mut stale_error: Option<String> = None;
        let mut escalation_error: Option<Vec<String>> = None;
        let with_locked_result = store.with_locked_document(track_id, |track, meta| {
            let review = track.review_mut().get_or_insert_with(ReviewState::new);
            let round_num = review
                .groups()
                .get(&group_name)
                .and_then(|g| match round_type {
                    domain::RoundType::Fast => g.fast().map(|r| r.round()),
                    domain::RoundType::Final => g.final_round().map(|r| r.round()),
                })
                .map(|n| n.saturating_add(1))
                .unwrap_or(1);

            let result = if concerns.is_empty() {
                ReviewRoundResult::new(round_num, verdict, timestamp.clone())
            } else {
                ReviewRoundResult::new_with_concerns(
                    round_num,
                    verdict,
                    timestamp.clone(),
                    concerns.clone(),
                )
            };
            match review.record_round_with_pending(
                round_type,
                &group_name,
                result,
                &expected_groups,
                &pre_update_hash,
            ) {
                Ok(()) => {}
                Err(domain::ReviewError::EscalationActive { concerns: blocked }) => {
                    escalation_error = Some(blocked);
                    return Err(domain::DomainError::Validation(
                        domain::ValidationError::InvalidTaskId(
                            "escalation-blocked-sentinel".to_owned(),
                        ),
                    ));
                }
                Err(domain::ReviewError::StaleCodeHash { expected, actual }) => {
                    stale_error = Some(format!(
                        "code hash mismatch: review recorded against {expected}, \
                         but current code is {actual} — review.status set to invalidated"
                    ));
                    meta.updated_at = timestamp.to_string();
                    return Ok(());
                }
                Err(e) => {
                    return Err(domain::DomainError::Validation(
                        domain::ValidationError::InvalidTaskId(e.to_string()),
                    ));
                }
            }

            meta.updated_at = timestamp.to_string();

            let pending_json = crate::track::codec::encode(track, meta).map_err(|e| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                    "codec encode error: {e}"
                )))
            })?;
            let pending_content = format!("{pending_json}\n");
            private_index.stage_bytes(&git, &metadata_rel, pending_content.as_bytes()).map_err(
                |e| domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(e)),
            )?;

            let h1 = private_index.normalized_tree_hash(&git, &metadata_rel).map_err(|e| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                    "post-pending normalized hash error: {e}"
                )))
            })?;

            if let Some(r) = track.review_mut().as_mut() {
                r.set_code_hash(h1).map_err(|e| {
                    domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(
                        format!("set_code_hash error: {e}"),
                    ))
                })?;
            }

            let final_json = crate::track::codec::encode(track, meta).map_err(|e| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                    "codec encode error (final): {e}"
                )))
            })?;
            let final_content = format!("{final_json}\n");
            private_index.stage_bytes(&git, &metadata_rel, final_content.as_bytes()).map_err(
                |e| domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(e)),
            )?;

            Ok(())
        });

        if let Some(blocked_concerns) = escalation_error {
            return Err(RecordRoundProtocolError::EscalationBlocked(blocked_concerns));
        }

        with_locked_result.map_err(|e| {
            let msg = e.to_string();
            let cleaned = if let Some(inner) = msg.strip_prefix("task id '") {
                inner.strip_suffix("' must match the pattern T<digits>").unwrap_or(inner).to_owned()
            } else {
                msg
            };
            RecordRoundProtocolError::Other(format!("record-round failed: {cleaned}"))
        })?;

        if let Some(err_msg) = stale_error {
            return Err(RecordRoundProtocolError::StaleHash(err_msg));
        }

        private_index.swap_into_real().map_err(RecordRoundProtocolError::Other)?;

        eprintln!(
            "[OK] Recorded {round_type} round for group '{}' (verdict: {verdict})",
            self.group_display
        );
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
// GitDiffScopeProvider — tests (Red phase written first)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use usecase::review_workflow::scope::{DiffScopeProvider, DiffScopeProviderError};

    use super::*;

    #[test]
    fn test_git_diff_scope_provider_against_head() {
        // Use HEAD as base_ref — always exists regardless of branch layout or shallow clone.
        let provider = GitDiffScopeProvider;
        let result = provider.changed_files("HEAD");
        assert!(result.is_ok(), "changed_files(HEAD) failed: {result:?}");
    }

    #[test]
    fn test_git_diff_scope_provider_unknown_base_ref() {
        let provider = GitDiffScopeProvider;
        let result = provider.changed_files("nonexistent-branch-xyz-999");
        assert!(result.is_err());
        match result {
            Err(DiffScopeProviderError::UnknownBaseRef { base_ref }) => {
                assert_eq!(base_ref, "nonexistent-branch-xyz-999");
            }
            other => panic!("expected UnknownBaseRef, got {other:?}"),
        }
    }

    #[test]
    fn test_git_diff_scope_provider_returns_diff_scope() {
        // Use HEAD — always available, even in shallow clones and CI.
        let provider = GitDiffScopeProvider;
        let result = provider.changed_files("HEAD");
        assert!(result.is_ok());
        let _scope = result.unwrap();
    }
}
