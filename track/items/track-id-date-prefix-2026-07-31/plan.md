<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Track ID Date Prefix

## Summary

GO-01 → T001–T003; D1。

## Tasks (2/3 resolved)

### S1 — Init Workflow

- [x] **T001**: `.harness/workflows/track/init.md` Step 2 の track-id 導出を date-then-slug 順へ更新し、`apps/cli/tests/operational_reference_cutover.rs` に init-workflow derivation regression を追加する。D1; IN-01; CN-01; AC-01。 (`a57b350472aee1300cda507479c6be73413685bf`)

### S2 — Date-prefixed fixtures

- [x] **T002**: `apps/cli-composition/src/track/mod.rs` test module、`apps/cli-composition/src/track/branch_strategy.rs`、`apps/cli-composition/src/track/resolution.rs` に `TrackCompositionRoot::{track_init, track_branch_create, track_resolve_id}` の date-prefixed fixture 回帰試験を追加する。D1; IN-02、IN-03; CN-02; AC-02、AC-04。

### S3 — Suffix-form fixtures

- [ ] **T003**: `apps/cli-composition/src/track/mod.rs` test module、`apps/cli-composition/src/track/branch_strategy.rs`、`apps/cli-composition/src/track/resolution.rs` に `TrackCompositionRoot::{track_init, track_branch_create, track_resolve_id}` の suffix-form fixture 回帰試験を追加する。D1; OUT-01; CN-01、CN-02; AC-03。
