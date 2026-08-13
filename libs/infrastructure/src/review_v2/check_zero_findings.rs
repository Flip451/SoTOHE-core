//! Infrastructure adapter for the check-zero-findings review-state port.

use std::path::Path;

use domain::review_v2::{FastVerdict, LogInfo, ReviewState, ReviewTarget, ScopeName, Verdict};
use domain::{CommitHash, FreeText, TrackId};
use usecase::review_v2::{ReviewCheckZeroFindingsStatePort, ReviewCycle, Reviewer, ReviewerError};

use crate::git_cli::{SystemGitRepo, isolated_bounded_git_output};
use crate::track_artifact::{TrackArtifactReadError, read_track_artifact};

use super::{
    FsCommitHashStore, FsReviewStore, RootedGitDiffGetter, RootedSystemReviewHasher,
    load_v2_scope_config,
};

const MAX_BASE_RESOLUTION_OUTPUT_BYTES: usize = 1024;
const MAX_BRANCH_VALIDATION_OUTPUT_BYTES: usize = 8 * 1024;
const MAX_TRACK_METADATA_BYTES: u64 = 4 * 1024 * 1024;
const METADATA_FILE: &str = "metadata.json";

/// Reads one scope's persisted review state for the check-zero-findings use case.
///
/// The adapter owns the infrastructure assembly needed to compare the current
/// diff hash against the persisted final verdict. It deliberately does not run
/// a reviewer: the private placeholder below exists only because
/// [`ReviewCycle`] is generic over its reviewer port.
pub struct ReviewCheckZeroFindingsStateAdapter;

impl ReviewCheckZeroFindingsStatePort for ReviewCheckZeroFindingsStateAdapter {
    fn state_for(
        &self,
        track_id: &TrackId,
        items_dir: &Path,
        scope: &ScopeName,
    ) -> Result<Option<ReviewState>, FreeText> {
        let (git, canonical_items_dir) = crate::discover_isolated_repo_for_items_dir(items_dir)
            .map_err(|error| FreeText::new(format!("git discover: {error}")))?;
        let canonical_root = git
            .root()
            .canonicalize()
            .map_err(|error| FreeText::new(format!("canonicalize repository root: {error}")))?;
        if !canonical_items_dir.starts_with(&canonical_root) {
            return Err(FreeText::new(
                "the discovered repository does not enclose the items directory".to_owned(),
            ));
        }

        let track_dir = canonical_items_dir.join(track_id.as_ref());
        if !track_dir.is_dir() {
            return Err(FreeText::new(format!(
                "track directory '{}' does not exist. Check --track-id '{}' and --items-dir '{}'.",
                track_dir.display(),
                track_id.as_ref(),
                items_dir.display(),
            )));
        }

        let base = resolve_review_diff_base(track_id, items_dir)?;
        let scope_config = load_v2_scope_config(
            &canonical_root.join(".harness/config/review-scope.json"),
            track_id,
            &canonical_root,
        )
        .map_err(|error| FreeText::new(format!("load review-scope.json: {error}")))?;
        let review_store =
            FsReviewStore::new(track_dir.join("review.json"), canonical_root.clone());
        let cycle = ReviewCycle::new(
            base,
            scope_config,
            StateReaderReviewer,
            RootedGitDiffGetter::new(git.clone()),
            RootedSystemReviewHasher::new(git),
        );
        let states = cycle
            .get_review_states(&review_store)
            .map_err(|error| FreeText::new(error.to_string()))?;

        Ok(states.get(scope).cloned())
    }
}

/// Resolves the exact diff base that review-state evaluation uses.
///
/// Review-results must display this resolved commit, rather than the metadata
/// branch label, because a valid `.commit_hash` can pin a different base.
pub(crate) fn resolve_review_diff_base(
    track_id: &TrackId,
    items_dir: &Path,
) -> Result<CommitHash, FreeText> {
    let (git, canonical_items_dir) = crate::discover_isolated_repo_for_items_dir(items_dir)
        .map_err(|error| FreeText::new(format!("git discover: {error}")))?;
    let canonical_root = git
        .root()
        .canonicalize()
        .map_err(|error| FreeText::new(format!("canonicalize repository root: {error}")))?;
    if !canonical_items_dir.starts_with(&canonical_root) {
        return Err(FreeText::new(
            "the discovered repository does not enclose the items directory".to_owned(),
        ));
    }
    let track_dir = canonical_items_dir.join(track_id.as_ref());
    if !track_dir.is_dir() {
        return Err(FreeText::new(format!(
            "track directory '{}' does not exist. Check --track-id '{}' and --items-dir '{}'.",
            track_dir.display(),
            track_id.as_ref(),
            items_dir.display(),
        )));
    }
    let base_branch = load_base_branch(&canonical_items_dir, track_id)?;
    let commit_hash_store = FsCommitHashStore::new(track_dir.join(".commit_hash"), canonical_root);
    resolve_diff_base(&commit_hash_store, &base_branch, &git)
}

fn load_base_branch(items_dir: &Path, track_id: &TrackId) -> Result<String, FreeText> {
    let metadata_json =
        read_track_artifact(items_dir, track_id, METADATA_FILE, MAX_TRACK_METADATA_BYTES).map_err(
            |error| match error {
                TrackArtifactReadError::NotFound => {
                    FreeText::new(format!("{METADATA_FILE} for '{}' not found", track_id.as_ref()))
                }
                TrackArtifactReadError::Failed(reason) => FreeText::new(format!(
                    "read {METADATA_FILE} for '{}': {reason}",
                    track_id.as_ref()
                )),
            },
        )?;
    let (metadata, _) = crate::track::codec::decode(&metadata_json)
        .map_err(|error| FreeText::new(format!("decode {METADATA_FILE}: {error}")))?;
    if metadata.id() != track_id {
        return Err(FreeText::new(format!(
            "{METADATA_FILE} track_id '{}' does not match requested track '{}'",
            metadata.id().as_ref(),
            track_id.as_ref()
        )));
    }
    Ok(metadata.branch_strategy_snapshot().base_branch().to_owned())
}

fn resolve_diff_base(
    store: &FsCommitHashStore,
    base_branch: &str,
    git: &SystemGitRepo,
) -> Result<CommitHash, FreeText> {
    match store.read_for_git(git) {
        Ok(Some(hash)) => return Ok(hash),
        Ok(None) => {}
        Err(error) => return Err(FreeText::new(format!("read .commit_hash: {error}"))),
    }

    let checked = isolated_bounded_git_output(
        git.root(),
        &["check-ref-format", "--branch", base_branch],
        MAX_BRANCH_VALIDATION_OUTPUT_BYTES,
    )
    .map_err(|error| FreeText::new(format!("validate base branch {base_branch}: {error}")))?;
    if !checked.status.success() {
        return Err(FreeText::new(format!(
            "git base branch {base_branch} is not a valid local branch name"
        )));
    }

    let revision = format!("refs/heads/{base_branch}^{{commit}}");
    let output = isolated_bounded_git_output(
        git.root(),
        &["rev-parse", "--verify", "--end-of-options", &revision],
        MAX_BASE_RESOLUTION_OUTPUT_BYTES,
    )
    .map_err(|error| FreeText::new(format!("isolated git rev-parse {base_branch}: {error}")))?;
    if !output.status.success() {
        return Err(FreeText::new(format!("git rev-parse {base_branch} failed")));
    }
    CommitHash::try_new(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .map_err(|error| FreeText::new(format!("git rev-parse {base_branch}: {error}")))
}

struct StateReaderReviewer;

impl Reviewer for StateReaderReviewer {
    fn review(&self, _target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected(
            "StateReaderReviewer: review() must not be called".to_owned(),
        ))
    }

    fn fast_review(&self, _target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        Err(ReviewerError::Unexpected(
            "StateReaderReviewer: fast_review() must not be called".to_owned(),
        ))
    }
}
