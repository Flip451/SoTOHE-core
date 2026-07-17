<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ADR-baseline の review 入口検査を init 刻印の存在確認のみに縮小する

## Summary

GO-01: T001, T002, T003.

## Tasks (3/3 resolved)

### S1 — ADR-baseline review-gate behavior

> Update ADR-baseline review-entry components. IN-01/IN-02/CN-01/AC-01/AC-02/AC-03.

- [x] **T001**: Update ADR-baseline review-entry components and their tests in `apps/cli/src/commands/adr_baseline.rs`, `apps/cli-driver/src/adr_baseline.rs`, `apps/cli-composition/src/adr_baseline.rs`, and `libs/usecase/src/adr_baseline.rs`. IN-01/IN-02/CN-01/AC-01/AC-02/AC-03.

### S2 — Review workflow guardian routing

> Update `.harness/workflows/track/review.md`. IN-03/CN-02/CN-03/AC-04/AC-05.

- [x] **T002**: Update `.harness/workflows/track/review.md` for ADR-diagnoser routing and finding-origin relay. IN-03/CN-02/CN-03/AC-04/AC-05.

### S3 — Phase 0 workflow follow-through

> Update `.harness/workflows/track/plan.md` and `.harness/workflows/track/adr2pr.md`. IN-03/IN-04/CN-02/CN-03/AC-05/AC-06.

- [x] **T003**: Update `.harness/workflows/track/plan.md` and `.harness/workflows/track/adr2pr.md` for ADR-diagnoser routing, Phase 0 approval carve-out, and fresh-review flow. IN-03/IN-04/CN-02/CN-03/AC-05/AC-06.
