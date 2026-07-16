<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 配布 scaffold の host-first 刷新と Makefile ゼロベース再構成

## Summary

T001: Rebuild the distributed common Makefile from workflow-required tasks and remove passthrough and duplicate wrappers.
T002: Supply symmetric host and Docker environment files with host-first default execution.
T003: Move canonical workflow and capability single operations to direct bin/sotp calls.
T004: Align provider adapters and permissions with the retired-wrapper contract.
T005: Refresh distributed consumer documentation for the retained host-first workflow.
T006: Pin toolchains and auxiliary tools, then move consumer CI and sotp provisioning to the host runner.
T007: Validate the exported scaffold contract and the removal of stale references.
Goal traceability: GO-01 -> T001/T003/T004/T005/T007; GO-02 -> T002/T003/T004/T005/T007; GO-03 -> T006/T007.

## Tasks (7/7 resolved)

### S1 — Distributed Makefile and environment split

> Run T001, then T002 (IN-01, IN-03, IN-04, IN-07, IN-08, CN-01, CN-02, CN-04, CN-06, AC-01, AC-03, AC-04, AC-07, AC-08).

- [x] **T001**: overlay/Makefile.toml — rebuild the common distributed task definitions; remove obsolete passthrough and duplicate track-views task definitions (IN-01, IN-02, IN-07, CN-01, CN-02, AC-01, AC-02, AC-07). (`d94cf7bcee5d3ceb553f4a3eccdf7585f69a2494`)
- [x] **T002**: overlay/Makefile.toml, overlay/Makefile.host.toml, overlay/Makefile.docker.toml, and .harness/config/template-boundary.json — split environment-dependent cargo-gate definitions into host and Docker peer files, declare the new peer overlays for export, update the common environment-selection target, and rewrite the `CODEX_BIN` resolution in `track-local-review`, `track-local-review-fix`, and `track-local-dry-fix` away from `asdf which codex` to the host-first tool-resolution contract (IN-03, IN-04, IN-08, CN-04, CN-06, AC-03, AC-04, AC-08). (`d94cf7bcee5d3ceb553f4a3eccdf7585f69a2494`)

### S2 — Workflow and adapter call-site migration

> Run T003, then T004 (IN-02, CN-01, CN-02, AC-02).

- [x] **T003**: .harness/workflows/track/ and .harness/capabilities/ — replace single-operation call sites with direct bin/sotp invocations and remove passthrough-wrapper references (IN-02, CN-01, CN-02, AC-02). (`d94cf7bcee5d3ceb553f4a3eccdf7585f69a2494`)
- [x] **T004**: .claude/commands/, .claude/settings.json, .agents/skills/, and distributed command-policy files — update provider-adapter and distributed Codex skill call sites and permission entries; remove passthrough-wrapper entries (IN-02, CN-01, CN-02, AC-02). (`d94cf7bcee5d3ceb553f4a3eccdf7585f69a2494`)

### S3 — Consumer guidance

> Run T005 (IN-09, CN-07, AC-09).

- [x] **T005**: README.md, CLAUDE.md, .claude/rules/, and affected distributed conventions — update consumer setup and workflow guidance; remove stale task, environment, tool-resolution, and wrapper references (IN-09, CN-07, AC-09). (`8df1374ac9ea59fb7444b7e3af039ee1a56fcf96`)

### S4 — Reproducible host provisioning and CI

> Run T006 after T002 so its export-boundary addition applies cleanly to the environment-file split (IN-05, IN-06, CN-03, AC-05, AC-06).

- [x] **T006**: overlay/rust-toolchain.toml, overlay/Makefile.toml bootstrap tasks, .github/workflows/ci.yml, and .harness/config/template-boundary.json — pin toolchain and auxiliary-tool versions, declare the pinned toolchain overlay for export, and update bootstrap and CI provisioning (IN-05, IN-06, CN-03, AC-05, AC-06). (`8df1374ac9ea59fb7444b7e3af039ee1a56fcf96`)

### S5 — Consumer-scaffold regression coverage

> Run T007 after T001 through T006 (IN-01, IN-02, IN-03, IN-04, IN-05, IN-06, IN-07, IN-08, IN-09, AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09).

- [x] **T007**: apps/cli/tests/consumer_scaffold_host_first.rs — add focused exported-overlay validation cases for the planned scaffold changes, including direct bin/sotp workflow calls and absence of retired passthrough-wrapper references (IN-01, IN-02, IN-03, IN-04, IN-05, IN-06, IN-07, IN-08, IN-09, AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09).
