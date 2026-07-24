<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# scaffold の初期化列を単一タスクへ畳む

## Summary

Organizes ADR D1-D3 into three independently reviewable, non-Rust delivery-surface tasks.
T001 establishes the opt-in first-run flow; T002 makes the corresponding exported configuration and shipment checks coherent; T003 removes the host-specific progress-tracking stop condition.

## Tasks (3/3 resolved)

### S1 — Overlay first-run initialization

> Groups T001 updates for `overlay/Makefile.toml` and its Makefile-level checks. IN-01, IN-02, CN-01, CN-02, AC-01, AC-02.

- [x] **T001**: Add the `init` task to `overlay/Makefile.toml` and focused Makefile-level coverage for its first-run and repeat-invocation paths. IN-01, IN-02, OS-02, OS-03, OS-04, CN-01, CN-02, AC-01, AC-02. (`0156c5fc`)

### S2 — Exported branch defaults and shipment enforcement

> Groups T002 updates for overlay branch-strategy configuration and shipment checks. IN-03, CN-03, CN-04, AC-03, AC-04.

- [x] **T002**: Update the overlay branch-strategy configuration and the template-boundary, export, and shipped-Makefile smoke checks. IN-03, OS-01, OS-05, CN-03, CN-04, AC-03, AC-04. (`0156c5fc`)

### S3 — Host-independent command progress

> Groups T003 updates for the shipped `plan` and `adr2pr` command adapters. IN-04, CN-05, AC-05.

- [x] **T003**: Update the shipped `plan` and `adr2pr` command adapters' `TaskCreate` integration and their available/unavailable host-progress coverage. IN-04, CN-05, AC-05. (`0156c5fc`)
