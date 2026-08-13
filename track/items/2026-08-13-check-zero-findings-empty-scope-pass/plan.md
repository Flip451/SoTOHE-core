<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# check-zero-findings passes empty scopes

## Summary

GO-01: T1-T2.

## Tasks (2/2 resolved)

### empty-scope-outcome-and-delivery — Empty-scope outcome and command delivery

> Modify the `EmptyScope` outcome in `libs/usecase/src/review_v2/check_zero_findings.rs`; deliver it from `apps/cli-driver/src/review.rs`; and update focused usecase, driver, and composition regressions. [IN-01; OS-01; OS-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03]

- [x] **T1**: Modify the `EmptyScope` outcome in `libs/usecase/src/review_v2/check_zero_findings.rs`; deliver it from `apps/cli-driver/src/review.rs`; and update focused usecase, driver, and composition regressions. [IN-01; OS-01; OS-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03] (`1ecfadb99072155b7ed87fc7b771a2fe1a3e105e`)

### restore-spec-design-pre-entry — Restore the spec-design pre-entry declaration

> Verify that `.harness/config/phase-commands.json` declares the spec-design pre-entry command `check-zero-findings --scope adr --round final`. [IN-02; OS-03; AC-04]

- [x] **T2**: Verify that `.harness/config/phase-commands.json` declares the spec-design pre-entry command `check-zero-findings --scope adr --round final`. [IN-02; OS-03; AC-04] (`1ecfadb99072155b7ed87fc7b771a2fe1a3e105e`)
