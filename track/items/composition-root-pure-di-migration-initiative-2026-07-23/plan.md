<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Composition root pure DI migration initiative

## Summary

GO-01: Leaf command task scope and entry attribution: T001–T004 in task-contract.json.
GO-02: Leaf command acceptance and constraint coverage: T001–T004 cover AC-01–AC-05, IN-01–IN-02, and CN-01–CN-03 in task-coverage.json.

## Tasks (4/4 resolved)

### adr-baseline — ADR baseline command migration

> Remove cli::commands::adr_baseline::items_dir, the track-added AdrBaselineClockPort, and SystemAdrBaselineClockAdapter; dispatch AdrBaselineCommand through one move-based destructure; restore explicit validated Timestamp in AdrBaselineCommand::Snapshot and baseline AdrBaselineInteractor and AdrBaselineValidationError shapes; add reason-aware AdrBaselineSnapshotKind and carry it through AdrBaselineStorePort and FsAdrBaselineStore; add AdrBaselineSnapshotInput, including the cli_driver cross-crate snapshot-kind conversion attribution; use a composition-wired fallible timestamp provider in AdrBaselineDriver with the scoped ADR-baseline timestamp error, remove unreachable validation from AdrBaselineError, and update the ADR-baseline timestamp adapter; update CLI conversion, inputs, and composition wiring; add focused regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.
> Completion evidence: the task's attributed catalogue entries are implemented and its focused regression coverage passes. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

- [x] **T001**: Remove cli::commands::adr_baseline::items_dir, the track-added AdrBaselineClockPort, and SystemAdrBaselineClockAdapter; dispatch AdrBaselineCommand through one move-based destructure; restore explicit validated Timestamp in AdrBaselineCommand::Snapshot and baseline AdrBaselineInteractor and AdrBaselineValidationError shapes; add reason-aware AdrBaselineSnapshotKind and carry it through AdrBaselineStorePort and FsAdrBaselineStore; add AdrBaselineSnapshotInput, including the cli_driver cross-crate snapshot-kind conversion attribution; use a composition-wired fallible timestamp provider in AdrBaselineDriver with the scoped ADR-baseline timestamp error, remove unreachable validation from AdrBaselineError, and update the ADR-baseline timestamp adapter; update CLI conversion, inputs, and composition wiring; add focused regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

### catalog — Catalog command migration

> Update the Catalog command's attributed catalogue entries and regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.
> Completion evidence: the task's attributed catalogue entries are implemented and its focused regression coverage passes. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

- [x] **T002**: Update the Catalog command's catalogued cli_driver, usecase, infrastructure, and cli_composition entries; add focused regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

### template — Template command migration

> Update the Template command's attributed catalogue entries and regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.
> Completion evidence: the task's attributed catalogue entries are implemented and its focused regression coverage passes. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

- [x] **T003**: Update the Template command's catalogued cli_driver, usecase, infrastructure, and cli_composition entries; add focused regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

### task-contract — TaskContract command migration

> Update the TaskContract command's attributed catalogue entries and regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.
> Completion evidence: the task's attributed catalogue entries are implemented and its focused regression coverage passes. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.

- [x] **T004**: Update the TaskContract command's catalogued cli_driver, usecase, infrastructure, and cli_composition entries; add focused regression coverage. AC-01–AC-05; IN-01–IN-02; CN-01–CN-03.
