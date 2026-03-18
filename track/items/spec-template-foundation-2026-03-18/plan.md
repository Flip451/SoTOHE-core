<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Phase 1 残り: spec.md テンプレート基盤整備 (SURVEY-03/10, TSUMIKI-02, SURVEY-16)

Phase 1 remaining quick wins: test file deletion guard hook, task description immutability enforcement, source attribution template, and spec frontmatter signals placeholder. All tasks are independent and parallelizable.

## Test file deletion block hook

Implement sotp hook dispatch block-test-deletion as a Rust PreToolUse hook. Detects Edit|Write targeting test files (*_test.rs, tests/**/*.rs, **/tests.rs) and blocks deletion (empty content Write). Add hook entry to settings.json.

- [ ] Add block-test-deletion hook via sotp hook dispatch to prevent test file deletion (SURVEY-03/#5)

## Task description immutability

Wire existing validate_descriptions_unchanged() from domain layer into FsTrackStore::save() update path. Skip validation on new track creation (no previous to compare). Return TrackWriteError on mutation attempt.

- [ ] Wire validate_descriptions_unchanged() into FsTrackStore::save() update path for task description immutability (SURVEY-10/#14)

## Source attribution template

Update track-plan SKILL.md spec.md initialization to include [source: ...] tags on each requirement. Define 3 source types: [source: PRD/doc reference], [source: inference — reason], [source: discussion]. Create project-docs/conventions/source-attribution.md.

- [ ] Add [source: ...] attribution tags to spec.md template in track-plan SKILL.md and create source-attribution convention (TSUMIKI-02/#6)

## Spec frontmatter completion + signals placeholder

Update TODO-PLAN 1-8 to done. Add optional signals field to frontmatter parser (libs/infrastructure/src/verify/frontmatter.rs). verify-spec-frontmatter continues to pass without signals (Phase 2 TSUMIKI-01 will make it required).

- [ ] Mark SURVEY-16/#23 as done (basic frontmatter implemented) and add optional signals field to frontmatter parser for Phase 2 preparation
