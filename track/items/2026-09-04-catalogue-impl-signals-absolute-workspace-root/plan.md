<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Catalogue Impl Signals Absolute Workspace Root

## Summary

Coordinate TrackWorkspaceRootInput::try_new changes with catalogue-impl-signals regression tests (GO-01).

## Tasks (2/2 resolved)

### SECTION-01 — Workspace-root boundary normalization

> Update apps/cli-driver/src/track_resolution.rs (TrackWorkspaceRootInput::try_new) and add focused cli_driver tests for startup inputs and failure paths (IN-01, IN-02, IN-03, CN-01, CN-02, AC-03, AC-04; OS-02, OS-03).

- [x] **T1**: Update apps/cli-driver/src/track_resolution.rs (TrackWorkspaceRootInput::try_new) and add focused cli_driver tests for startup inputs and failure paths (IN-01, IN-02, IN-03, CN-01, CN-02, AC-03, AC-04; OS-02, OS-03).

### SECTION-02 — Relative-root target-exclusion regressions

> Extend tests in apps/cli/src/commands/track/tddd/catalogue_impl_signals.rs for default and explicit-relative workspace-root invocations and relative/absolute path variants (IN-02, IN-04, CN-01, AC-01, AC-02, AC-04; OS-01).

- [x] **T2**: Extend tests in apps/cli/src/commands/track/tddd/catalogue_impl_signals.rs for default and explicit-relative workspace-root invocations and relative/absolute path variants (IN-02, IN-04, CN-01, AC-01, AC-02, AC-04; OS-01).
