<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Phase 1: sotp CLI hardening (data safety, validation, hooks, spec verification)

Phase 1 hardening of sotp CLI: fix data-loss bugs (T001-T002), strengthen input validation (T003-T004), add test-file deletion guard hook (T005), enforce task description immutability (T006), and add spec.md structure verification (T007-T008).

## Data Safety Fixes

Fix TrackDocumentV2 silent field loss and collect_track_branch_claims abort-on-single-failure.

- [ ] SSoT-09: Add #[serde(flatten)] to TrackDocumentV2 to preserve unknown fields during round-trip
- [ ] SSoT-10: Change collect_track_branch_claims to skip-and-warn on broken metadata instead of aborting all

## Input Validation Hardening

Add TrackId validation to branch resolution and extend PR body findings parser format coverage.

- [ ] WF-33: Add TrackId::new() validation in resolve_track_id_from_branch after prefix strip
- [ ] WF-30: Extend parse_body_findings to recognize numbered lists and + prefix

## Test File Deletion Guard Hook

Add block-test-file-deletion PreToolUse hook in Rust (domain + usecase + cli layers).

- [ ] SURVEY-03: Add block-test-file-deletion PreToolUse hook to prevent test file removal

## Task Description Immutability

Validate that existing task descriptions are not mutated on save.

- [ ] SURVEY-10: Add task description immutability validation on save (detect description mutation of existing tasks)

## Spec Verification Subcommands

Add sotp verify spec-attribution and spec-frontmatter subcommands for spec.md quality gates.

- [ ] TSUMIKI-02: Add sotp verify spec-attribution subcommand to check [source: ...] tags in spec.md
- [ ] SURVEY-16: Add sotp verify spec-frontmatter subcommand to check YAML frontmatter in spec.md
