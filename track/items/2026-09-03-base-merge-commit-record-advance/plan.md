<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# base merge commit record advance

## Summary

GO-01 → T1 → T2.

## Tasks (0/2 resolved)

### SECTION-01 — Clean-merge advancement and caller migration

> Update libs/usecase/src/base_merge.rs (BaseMergeInteractor and PostMergeCleanupError) with the catalogue-declared commit-record dependency, CommitRecord(DiagnosticText) error variant, and commit-record update failure routing, plus the clean-merge completion implementation (IN-01, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04).
> Migrate apps/cli-composition/src/track/composition_root.rs and direct BaseMergeInteractor::new callers in libs/infrastructure/** and apps/cli-driver/** test code for the constructor change (IN-01, CN-01).

- [ ] **T1**: Update libs/usecase/src/base_merge.rs (BaseMergeInteractor and PostMergeCleanupError) to inject TrackCommitHashPort, add the catalogue-declared CommitRecord(DiagnosticText) error variant, and route commit-record update failures through it; update the clean-merge completion implementation and focused tests, and migrate apps/cli-composition/src/track/composition_root.rs plus every direct BaseMergeInteractor::new call in libs/infrastructure/** and apps/cli-driver/** test code to pass the existing adapter or a focused test double (IN-01, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04).

### SECTION-02 — Conflict-recovery advancement

> Update libs/usecase/src/base_merge.rs (BaseMergeInteractor) and focused tests to use the T1 dependency in the conflict-recovery completion path (IN-02, CN-01, CN-02, CN-03, AC-02, AC-03, AC-04).

- [ ] **T2**: Update libs/usecase/src/base_merge.rs (BaseMergeInteractor) and focused tests to wire the conflict-recovery completion path through the TrackCommitHashPort dependency established by T1 (IN-02, CN-01, CN-02, CN-03, AC-02, AC-03, AC-04).
