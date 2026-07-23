<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 出荷面を最小化し、workflow と出荷物の乖離クラスを閉じる

## Summary

GO-01 -> T001-T004.
GO-02 -> T005-T006.
T001-T002 classify convention export entries and update the scaffold bootstrap.
T003-T004 update the shipped skills and rules references.
T005-T006 align scaffold workflow integration and extend template-export smoke coverage.

## Tasks (6/6 resolved)

### export-boundary — Convention export boundary and bootstrap

> Classify convention export entries in `.harness/config/template-boundary.json`. IN-01/AC-01. T001.
> Add `bin/sotp conventions update-index` to `overlay/Makefile.toml` bootstrap. IN-02/AC-02. T002.

- [x] **T001**: Classify every convention in `.harness/config/template-boundary.json` as an explicit `include` or `exclude`, then run `cargo make template-export-smoke` against the manifest. GO-01/IN-01/OS-01/AC-01/AC-02. (`ff2a4a7498db610d462387fb32949f866408aeae`)
- [x] **T002**: Add `bin/sotp conventions update-index` to the exported-scaffold bootstrap in `overlay/Makefile.toml` after `bin/sotp` provisioning, then inspect the bootstrap order and run `cargo make template-export-smoke`. GO-01/IN-02/AC-02. (`ff2a4a7498db610d462387fb32949f866408aeae`)

### consumer-instructions — Consumer skills and rules surface

> Update `.claude/skills/.gitignore`, remove redundant skill sources, and update skill-compliance mapping. IN-03/AC-03. T003.
> Migrate `.claude/rules` paths and update `CLAUDE.md`, rule, and workflow references. IN-04/IN-05/AC-04. T004.

- [x] **T003**: Update `.claude/skills/.gitignore`, remove `.claude/skills/track-plan` and `.claude/skills/diagnose`, and update the skill-compliance hook mapping; run `cargo make template-export-smoke` and inspect the hook mapping. GO-01/IN-03/OS-02/AC-03. (`ff2a4a7498db610d462387fb32949f866408aeae`)
- [x] **T004**: Rename numbered `.claude/rules` files, add the neutral language overlay, exclude `maintainer-checklist` from shipped rules, and update references in `CLAUDE.md`, rules, and workflows; run `cargo make template-export-smoke` and scan the migrated references. GO-01/IN-04/IN-05/OS-03/AC-04. (`ff2a4a7498db610d462387fb32949f866408aeae`)

### scaffold-smoke — Scaffold workflow alignment and smoke coverage

> Update `.harness/workflows/track/review.md` and `overlay/Makefile.toml` integration. IN-06/AC-07. T005.
> Add workflow-task and workspace-CLI cases to `template-export-smoke`. IN-06/CO-01/CO-02/AC-05/AC-06/AC-07. T006.

- [x] **T005**: Update `.harness/workflows/track/review.md` and `overlay/Makefile.toml` to remove the stale baseline wrapper step and workspace-CLI invocation, and add the `ci-track` step. GO-02/IN-06/OS-04/AC-07. (`ff2a4a7498db610d462387fb32949f866408aeae`)
- [x] **T006**: Add workflow-task-existence and workspace-CLI-invocation cases to `template-export-smoke`. GO-02/IN-06/OS-04/CO-01/CO-02/AC-05/AC-06/AC-07. (`ff2a4a7498db610d462387fb32949f866408aeae`)
