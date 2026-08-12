<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 31, yellow: 0, red: 0 }
---

# Base Merge and Conflict Recovery

## Goal

- [GO-01] Permit an active track to safely incorporate the base branch recorded in its branch-strategy snapshot and complete its cleanup against the exact merged base commit, while preserving the prohibition on merging a track into its base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3]
- [GO-02] Make merge-conflict recovery, guarded worktree stashing, and type-signal freshness reliable without weakening repository safety gates. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2, knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D3, knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4]

## Scope

### In Scope
- [IN-01] Provide a guarded base-merge operation that runs only when the current branch is the active `track/<id>` branch and its source is that track's branch-strategy snapshot base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T001, T002, T003, T004, T020, T025]
- [IN-02] After every clean guarded base merge, regenerate the baseline from the exact incorporated base commit, generate derived views, and then record synchronization of that commit. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T003, T004, T010, T011, T013, T023, T025]
- [IN-06] Persist synchronization of the exact base commit incorporated by a successful guarded merge only after its baseline and view stages succeed. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T023, T025]
- [IN-03] Provide orchestrator-owned conflict recovery through the canonical recovery workflow, with both `/track:recover` and `$track-recover` adapters shipping as provider-framing-only entry points. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2] [tasks: T005, T024, T025]
- [IN-04] Provide guarded stash push and stash-pop support that explicitly includes untracked track artifacts, pairs each pop exclusively with its push result, and preserves repository safety gates. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D3, knowledge/adr/2026-08-11-0910-guarded-stash-pop-pairing.md#D1] [tasks: T006, T021, T022, T025]
- [IN-05] For the currently active track only, reuse a type-signals evaluation cache only for a clean working tree whose HEAD matches the cache's recorded commit; otherwise reevaluate it. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4, knowledge/adr/2026-08-05-1035-type-signals-authority-availability-boundary.md#D1] [tasks: T007, T008, T009, T012, T015, T016, T017, T018, T019, T025]

### Out of Scope
- [OS-01] Merging a track branch into its base branch, or permitting a merge source other than the active track snapshot's base branch, remains out of scope and rejected. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004]
- [OS-02] Provider adapters do not define independent recovery steps, gates, state transitions, or failure-recovery semantics. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2] [tasks: T005]
- [OS-03] Scanning, migrating, or validating type-signals caches for inactive or archived tracks, and routine manual deletion of type-signals artifacts during baseline recapture, are out of scope. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009, T012]

## Constraints
- [CN-01] The base-merge guard must reject reverse-direction merges, a non-track current branch, and any source branch that differs from the active track's recorded base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004, T020, T025]
- [CN-02] Conflict recovery must follow the canonical workflow under orchestrator ownership after the guarded merge has completed its conflict-time baseline and view stages; conflicted merge outcomes do not write a sync-base record. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T005, T011, T024, T025]
- [CN-03] A type-signals evaluation cache may be reused only when the working tree is clean and HEAD matches the commit recorded with that cache; otherwise it must be reevaluated. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4, knowledge/adr/2026-08-05-1035-type-signals-authority-availability-boundary.md#D1] [tasks: T007, T008, T009, T012, T015, T016, T017, T018, T019, T025]
- [CN-04] A dirty working tree, a HEAD commit that differs from the recorded cache commit, or an unreadable authority must cause type-signals reevaluation rather than cache reuse. [adr: knowledge/adr/2026-08-05-1035-type-signals-authority-availability-boundary.md#D1] [tasks: T007, T008, T009, T012, T015, T016, T017, T018, T019, T025]
- [CN-05] Baseline regeneration must use the exact base commit incorporated by the guarded merge, and a failure must be reported. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T013, T023, T025]
- [CN-06] The sync-base record must use the exact base commit incorporated by the guarded merge and be written only after that clean merge's baseline and view stages succeed. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T023, T025]
- [CN-07] Clean-merge cleanup may complete only in the order Baseline, Views, then SyncBaseStamp. A conflicted merge executes Baseline then Views, with views reading only persisted signals, baseline, and track artifacts; it does not write a SyncBaseStamp and reports any stage failure. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T023, T025]

## Acceptance Criteria
- [ ] [AC-01] A guarded base merge succeeds only from the currently active `track/<id>` branch and only when the requested source equals its branch-strategy snapshot base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004, T020, T025]
- [ ] [AC-02] A reverse-direction merge, an invalid current branch, or a source that differs from the snapshot base branch is rejected without performing the merge. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004, T020, T025]
- [ ] [AC-03] After a clean guarded base merge, the baseline is regenerated from the exact base commit the merge incorporated, derived views are generated once after that baseline stage, and the sync-base record then records that exact commit. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T003, T004, T010, T011, T013, T023, T025]
- [ ] [AC-04] When a guarded base merge conflicts, its conflict outcome regenerates the baseline from the exact base commit it incorporated and then generates derived views from persisted signals, baseline, and track artifacts only; it reports any stage failure and surfaces the canonical recovery workflow entry point. After those stages, the canonical recovery workflow directs conflict-resolution editing, the normal review workflow through zero findings, and a guarded commit. It does not write a sync-base record or record cleanup completion. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T003, T004, T005, T011, T014, T020, T024, T025]
- [ ] [AC-05] Both `/track:recover` and `$track-recover` are available together and contain only provider-specific invocation framing, tool constraints, and reporting guidance; the canonical workflow owns all recovery semantics. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2] [tasks: T005]
- [ ] [AC-06] A guarded stash push records untracked track artifacts. Guarded stash operations do not change branch history, update branch refs, or change the active track branch; the stash-internal objects and `refs/stash` updates required by stash and stash-pop are permitted. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D3] [tasks: T006, T021, T022, T025]
- [ ] [AC-07] For the active track, a type-signals evaluation cache is reused only when the working tree is clean and HEAD equals the commit recorded with that cache; otherwise it is reevaluated. [adr: knowledge/adr/2026-08-05-1035-type-signals-authority-availability-boundary.md#D1] [tasks: T007, T008, T009, T012, T015, T016, T017, T018, T019, T025]
- [ ] [AC-08] During type-signals evaluation, a missing cache, schema mismatch, missing required cache field, invalid cache value, or malformed cache JSON is treated as a cache miss; successful reevaluation atomically replaces the cache with the new result. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009, T025]
- [ ] [AC-09] During type-signals evaluation, an unreadable authority, a dirty working tree, or a HEAD commit that differs from the recorded cache commit requires reevaluation rather than cache reuse. [adr: knowledge/adr/2026-08-05-1035-type-signals-authority-availability-boundary.md#D1] [tasks: T007, T008, T009, T012, T015, T016, T017, T018, T019, T025]
- [ ] [AC-10] If baseline regeneration fails, the operation reports failure rather than cleanup completion. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T013, T023, T025]
- [ ] [AC-11] Following successful baseline and view stages of a clean guarded merge, the sync-base record contains the exact base commit that merge incorporated. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T023, T025]
- [ ] [AC-12] A conflicted guarded merge runs the baseline stage from its exact incorporated base commit and then generates views from persisted signals, baseline, and track artifacts only; it does not write a sync-base record, and any stage failure is reported. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011, T025]
- [ ] [AC-13] A guarded stash push records the created stash commit OID or that no stash was created. A guarded stash-pop applies only the recorded OID and clears the record after success; for a no-stash record it applies nothing and clears the record. An absent record or missing OID reports failure and stops, and unrelated stash entries are untouched. [adr: knowledge/adr/2026-08-11-0910-guarded-stash-pop-pairing.md#D1] [tasks: T021, T022, T025]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 31  🟡 0  🔴 0

