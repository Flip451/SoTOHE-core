<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# base merge commit record advance

## Summary

GO-01 is implemented by the clean-merge T1 path and the conflict-recovery T2 path.

## Tasks (2/2 resolved)

### SECTION-01 — Clean-merge commit-record integration

> Update libs/usecase/src/base_merge.rs at BaseMergeInteractor and PostMergeCleanupError to use the injected TrackCommitHashPort on the clean-completion branch (IN-01, CN-02, CN-03, AC-01, AC-03, AC-04).
> Migrate BaseMergeInteractor::new callers in apps/cli-composition/src/track/composition_root.rs and focused libs/infrastructure/** and apps/cli-driver/** tests to supply the existing adapter or a test double (IN-01, AC-01, AC-03).

- [x] **T1**: Complete the clean-merge implementation in libs/usecase/src/base_merge.rs by injecting TrackCommitHashPort into BaseMergeInteractor, routing persistence failures through PostMergeCleanupError::CommitRecord, and extending focused tests; migrate BaseMergeInteractor::new callers in apps/cli-composition/src/track/composition_root.rs and focused libs/infrastructure/** and apps/cli-driver/** tests to provide the existing adapter or a test double (IN-01, CN-02, CN-03, AC-01, AC-03, AC-04).

### SECTION-02 — Conflict-recovery commit-record integration

> Update .harness/workflows/track/recover.md at the post-guarded-commit completion step to invoke bin/sotp track set-commit-hash through the existing TrackSetCommitHashService, TrackSetCommitHashInteractor, and TrackCommitHashPort path in libs/usecase/src/track_lifecycle/track_set_commit_hash.rs (IN-02, CN-01, CN-02, CN-03, AC-02, AC-03, AC-04).

- [x] **T2**: Update .harness/workflows/track/recover.md at the post-guarded-commit completion step to invoke bin/sotp track set-commit-hash through the existing TrackSetCommitHashService, TrackSetCommitHashInteractor, and TrackCommitHashPort path in libs/usecase/src/track_lifecycle/track_set_commit_hash.rs (IN-02, CN-01, CN-02, CN-03, AC-02, AC-03, AC-04).
