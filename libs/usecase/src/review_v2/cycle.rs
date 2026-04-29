use std::collections::HashMap;

use domain::CommitHash;
use domain::review_v2::{
    FastVerdict, FilePath, NotRequiredReason, RequiredReason, ReviewApprovalVerdict, ReviewHash,
    ReviewOutcome, ReviewReader, ReviewScopeConfig, ReviewState, ReviewTarget, ScopeName, Verdict,
};

use super::error::ReviewCycleError;
use super::ports::{DiffGetter, ReviewHasher, Reviewer};

/// Review cycle orchestrator.
///
/// Coordinates diff retrieval, scope classification, hash computation, and
/// reviewer invocation. Does NOT persist results — callers use `ReviewWriter`
/// to write verdicts returned by `review()` / `fast_review()`.
///
/// Generic over port implementations for testability.
pub struct ReviewCycle<R, H, D> {
    base: CommitHash,
    scope_config: ReviewScopeConfig,
    reviewer: R,
    hasher: H,
    diff_getter: D,
}

impl<R: Reviewer, H: ReviewHasher, D: DiffGetter> ReviewCycle<R, H, D> {
    /// Constructs a new review cycle.
    #[must_use]
    pub fn new(
        base: CommitHash,
        scope_config: ReviewScopeConfig,
        reviewer: R,
        diff_getter: D,
        hasher: H,
    ) -> Self {
        Self { base, scope_config, reviewer, hasher, diff_getter }
    }

    /// Reviews a specific scope (final verdict).
    ///
    /// 1. Gets diff files and classifies into scopes
    /// 2. Computes hash_before
    /// 3. If empty → `Skipped`
    /// 4. Invokes reviewer
    /// 5. Computes hash_after and checks for changes during review
    /// 6. Returns `Reviewed { verdict, log_info, hash }`
    ///
    /// Callers must persist the result via `ReviewWriter::write_verdict`.
    ///
    /// # Errors
    /// - `UnknownScope` if the scope is not in the config
    /// - `FileChangedDuringReview` if hash changed between before/after
    /// - Propagated errors from diff, hash, or reviewer
    pub fn review(&self, scope: &ScopeName) -> Result<ReviewOutcome<Verdict>, ReviewCycleError> {
        if !self.scope_config.contains_scope(scope) {
            return Err(ReviewCycleError::UnknownScope(scope.clone()));
        }

        let target_before = self.get_scope_target(scope)?;
        let hash_before = self.hasher.calc(&target_before)?;
        if hash_before.is_empty() {
            return Ok(ReviewOutcome::Skipped);
        }

        let (verdict, log_info) = self.reviewer.review(&target_before)?;

        // Check for file changes during review
        let target_after = self.get_scope_target(scope)?;
        let hash_after = self.hasher.calc(&target_after)?;
        if hash_before != hash_after {
            return Err(ReviewCycleError::FileChangedDuringReview);
        }

        Ok(ReviewOutcome::Reviewed { verdict, log_info, hash: hash_after })
    }

    /// Reviews a specific scope (fast/advisory verdict).
    ///
    /// Same flow as `review()` but returns `FastVerdict`.
    /// Fast verdicts are not used for approval decisions.
    ///
    /// # Errors
    /// Same as `review()`.
    pub fn fast_review(
        &self,
        scope: &ScopeName,
    ) -> Result<ReviewOutcome<FastVerdict>, ReviewCycleError> {
        if !self.scope_config.contains_scope(scope) {
            return Err(ReviewCycleError::UnknownScope(scope.clone()));
        }

        let target_before = self.get_scope_target(scope)?;
        let hash_before = self.hasher.calc(&target_before)?;
        if hash_before.is_empty() {
            return Ok(ReviewOutcome::Skipped);
        }

        let (verdict, log_info) = self.reviewer.fast_review(&target_before)?;

        let target_after = self.get_scope_target(scope)?;
        let hash_after = self.hasher.calc(&target_after)?;
        if hash_before != hash_after {
            return Err(ReviewCycleError::FileChangedDuringReview);
        }

        Ok(ReviewOutcome::Reviewed { verdict, log_info, hash: hash_after })
    }

    /// Gets review targets for all scopes from the current diff.
    ///
    /// # Errors
    /// Propagated from diff and classification.
    pub fn get_review_targets(&self) -> Result<HashMap<ScopeName, ReviewTarget>, ReviewCycleError> {
        let files = self.diff_getter.list_diff_files(&self.base)?;
        let classified = self.scope_config.classify(&files);
        Ok(classified.into_iter().map(|(scope, files)| (scope, ReviewTarget::new(files))).collect())
    }

    /// Gets the review state for all scopes.
    ///
    /// Compares current hashes against stored final verdicts to determine
    /// which scopes need review. Includes configured-but-empty scopes as `Empty`.
    ///
    /// # Errors
    /// Propagated from diff, hash, or reader.
    pub fn get_review_states(
        &self,
        reader: &impl ReviewReader,
    ) -> Result<HashMap<ScopeName, ReviewState>, ReviewCycleError> {
        // 1. Current diff → scope hashes
        let files = self.diff_getter.list_diff_files(&self.base)?;
        let classified = self.scope_config.classify(&files);
        let mut current_hashes: HashMap<ScopeName, ReviewHash> = HashMap::new();
        for (scope, scope_files) in &classified {
            let target = ReviewTarget::new(scope_files.clone());
            current_hashes.insert(scope.clone(), self.hasher.calc(&target)?);
        }

        // 2. Stored latest finals
        let latest_finals = reader.read_latest_finals()?;

        // 3. Determine state for each scope
        let mut states = HashMap::new();

        // 3a. Scopes with files in the diff
        for (scope, current_hash) in &current_hashes {
            let state = match current_hash {
                ReviewHash::Empty => ReviewState::NotRequired(NotRequiredReason::Empty),
                ReviewHash::Computed(_) => match latest_finals.get(scope) {
                    None => ReviewState::Required(RequiredReason::NotStarted),
                    Some((Verdict::FindingsRemain(_), _)) => {
                        ReviewState::Required(RequiredReason::FindingsRemain)
                    }
                    Some((Verdict::ZeroFindings, stored_hash)) => {
                        if stored_hash == current_hash {
                            ReviewState::NotRequired(NotRequiredReason::ZeroFindings)
                        } else {
                            ReviewState::Required(RequiredReason::StaleHash)
                        }
                    }
                },
            };
            states.insert(scope.clone(), state);
        }

        // 3b. Configured scopes with no files → Empty
        for scope in self.scope_config.all_scope_names() {
            states.entry(scope).or_insert(ReviewState::NotRequired(NotRequiredReason::Empty));
        }

        Ok(states)
    }

    /// Evaluates whether the review cycle is approved for the current track.
    ///
    /// Collects all `Required(*)` scopes via `get_review_states` and classifies:
    /// - No Required scopes → `Approved`
    /// - All Required scopes are `Required(NotStarted)` **and** `review_json_exists == false`
    ///   → `ApprovedWithBypass { not_started_count }`
    /// - Otherwise → `Blocked { required_scopes }`
    ///
    /// The `review_json_exists` flag is provided by the caller (CLI composition root)
    /// via `FsReviewStore::review_json_exists()`; this method performs no file I/O.
    ///
    /// # Errors
    /// Propagated from `get_review_states` (diff, hash, or reader errors).
    pub fn evaluate_approval(
        &self,
        reader: &impl ReviewReader,
        review_json_exists: bool,
    ) -> Result<ReviewApprovalVerdict, ReviewCycleError> {
        let states = self.get_review_states(reader)?;

        // Collect Required(*) scopes as (ScopeName, RequiredReason) pairs.
        let required: Vec<(ScopeName, RequiredReason)> = states
            .into_iter()
            .filter_map(|(name, state)| match state {
                ReviewState::Required(reason) => Some((name, reason)),
                ReviewState::NotRequired(_) => None,
            })
            .collect();

        if required.is_empty() {
            return Ok(ReviewApprovalVerdict::Approved);
        }

        // Bypass: all Required scopes are NotStarted AND review.json is absent.
        let all_not_started =
            required.iter().all(|(_, reason)| matches!(reason, RequiredReason::NotStarted));
        if all_not_started && !review_json_exists {
            return Ok(ReviewApprovalVerdict::ApprovedWithBypass {
                not_started_count: required.len(),
            });
        }

        let mut required_scopes: Vec<ScopeName> =
            required.into_iter().map(|(name, _)| name).collect();
        // Sort by display representation for deterministic output across HashMap iteration.
        required_scopes.sort_by_key(|a| a.to_string());
        Ok(ReviewApprovalVerdict::Blocked { required_scopes })
    }

    /// Helper: gets the classified files for a single scope from the current diff.
    fn get_scope_target(&self, scope: &ScopeName) -> Result<ReviewTarget, ReviewCycleError> {
        let files = self.diff_getter.list_diff_files(&self.base)?;
        let classified = self.scope_config.classify(&files);
        let scope_files: Vec<FilePath> = classified.get(scope).cloned().unwrap_or_default();
        Ok(ReviewTarget::new(scope_files))
    }
}
