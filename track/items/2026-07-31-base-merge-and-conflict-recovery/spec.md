<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 30, yellow: 0, red: 0 }
---

# Base Merge and Conflict Recovery

## Goal

- [GO-01] Permit an active track to safely incorporate the base branch recorded in its branch-strategy snapshot and complete its cleanup against the exact merged base commit, while preserving the prohibition on merging a track into its base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3]
- [GO-02] Make merge-conflict recovery, guarded worktree stashing, and type-signal freshness reliable without weakening repository safety gates. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2, knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D3, knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4]

## Scope

### In Scope
- [IN-01] Provide a guarded base-merge operation that runs only when the current branch is the active `track/<id>` branch and its source is that track's branch-strategy snapshot base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T001, T002, T003, T004]
- [IN-02] After every clean guarded base merge, regenerate derived views, replace the baseline from a commit-pinned isolated worktree without exposing a partial replacement, and then record synchronization of that exact merged base commit. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T003, T004, T010, T011]
- [IN-06] Persist the active track's synchronization state as an atomically replaced, schema-versioned record containing its track identity, base branch, and the exact base commit incorporated by a successful guarded merge. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2] [tasks: T010, T011]
- [IN-03] Provide orchestrator-owned conflict recovery through the canonical recovery workflow, with both `/track:recover` and `$track-recover` adapters shipping as provider-framing-only entry points. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2] [tasks: T005]
- [IN-04] Provide guarded stash and stash-pop support that explicitly includes untracked track artifacts while preserving repository safety gates. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D3] [tasks: T006]
- [IN-05] For the currently active track only, evaluate and self-heal type-signals cache entries using catalogue declaration, implementation input, and actual baseline hashes. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]

### Out of Scope
- [OS-01] Merging a track branch into its base branch, or permitting a merge source other than the active track snapshot's base branch, remains out of scope and rejected. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004]
- [OS-02] Provider adapters do not define independent recovery steps, gates, state transitions, or failure-recovery semantics. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2] [tasks: T005]
- [OS-03] Scanning, migrating, or validating type-signals caches for inactive or archived tracks, and routine manual deletion of type-signals artifacts during baseline recapture, are out of scope. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]

## Constraints
- [CN-01] The base-merge guard must reject reverse-direction merges, a non-track current branch, and any source branch that differs from the active track's recorded base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004]
- [CN-02] Conflict recovery must follow the canonical workflow through conflict editing, zero-findings review, and guarded commit under orchestrator ownership; conflict outcomes do not run clean-merge cleanup. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T005, T011]
- [CN-03] A type-signals cache may be reused only when its catalogue declaration hash, implementation input hash, and actual baseline hash all equal the current authoritative values. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]
- [CN-04] Cache misses caused by hash mismatch or any cache decode failure must reevaluate and atomically overwrite only after success; missing or unreadable baselines, other authoritative-input failures, evaluation failures, and cache-write failures must fail closed. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]
- [CN-05] Baseline recapture must generate and validate a complete replacement in an isolated worktree pinned to the already-authorized base commit, atomically publish it only after success, and preserve or restore the prior baseline on any failure; every failure, including failed restoration, fails closed. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1] [tasks: T010, T011]
- [CN-06] The sync-base record must use the exact commit incorporated by the guarded merge rather than a later branch-name resolution, be atomically written only after views and baseline recapture succeed, support idempotent retry for the same value, and fail closed on record generation, validation, write, or replacement failure. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011]
- [CN-07] Clean-merge cleanup may complete only in the order Views, Baseline, then SyncBaseStamp; conflict outcomes do not run that cleanup, and baseline replacement does not delete the type-signals cache. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011]

## Acceptance Criteria
- [ ] [AC-01] A guarded base merge succeeds only from the currently active `track/<id>` branch and only when the requested source equals its branch-strategy snapshot base branch. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-02] A reverse-direction merge, an invalid current branch, or a source that differs from the snapshot base branch is rejected without performing the merge. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-03] After a clean guarded base merge, derived views are regenerated; a complete, validated baseline replacement produced in an isolated worktree pinned to the exact merged base commit is atomically published; and only then is the sync-base record atomically written before the operation reports success. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T003, T004, T010, T011]
- [ ] [AC-04] When a guarded base merge conflicts, the orchestrator drives the canonical recovery workflow through editing, zero-findings review, and guarded commit without running clean-merge cleanup. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T002, T003, T004, T005, T011]
- [ ] [AC-05] Both `/track:recover` and `$track-recover` are available together and contain only provider-specific invocation framing, tool constraints, and reporting guidance; the canonical workflow owns all recovery semantics. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D2] [tasks: T005]
- [ ] [AC-06] A guarded stash records untracked track artifacts, and a successful guarded stash-pop restores them through the `bin/sotp git stash` command path without changing branch history, updating branch refs, or changing the active track branch; the stash-internal objects and `refs/stash` updates required by stash and stash-pop are permitted. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D3] [tasks: T006]
- [ ] [AC-07] For the active track, a type-signals cache is reused only when all three current hashes—catalogue declaration, implementation input, and actual baseline—match the cache record. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]
- [ ] [AC-08] Any hash mismatch, missing cache, schema mismatch, missing required cache field, invalid cache value, or malformed cache JSON is treated as a cache miss; successful reevaluation atomically replaces the cache with the new result and all current hashes. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]
- [ ] [AC-09] Missing or unreadable baselines, other authoritative-input failures, evaluation failures, and cache-write failures fail closed; inactive and archived tracks are untouched, and manual type-signals deletion is not part of ordinary baseline recapture. [adr: knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md#D4] [tasks: T007, T008, T009]
- [ ] [AC-10] If isolated baseline generation, validation, atomic replacement, or necessary restoration fails, the operation reports failure rather than cleanup completion and the previously valid baseline remains available whenever restoration succeeds. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D1] [tasks: T010, T011]
- [ ] [AC-11] Following successful views and baseline steps, the active track has a schema-versioned sync-base record with its track id, base branch, and the exact merged base commit; rerunning with that same commit is idempotent, while a later successful merge atomically replaces the record with its newly incorporated commit. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D2] [tasks: T010, T011]
- [ ] [AC-12] A conflict outcome enters the canonical recovery workflow without running clean-merge cleanup, and a failure in views, baseline recapture, or sync-base recording prevents a completed-cleanup success result. [adr: knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T010, T011]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 30  🟡 0  🔴 0

