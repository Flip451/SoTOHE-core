<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# external_guides 撤去と関連 helper の連鎖削除

## Summary

This track is deletion-only with zero Rust source code changes. Deletion targets span 6 categories: Python scripts, registry SSoT, Makefile tasks, slash command, doc references, and Roadmap ADR back-reference.
5-task structure: T001 (core scripts + registry deletion) -> T002 (conditional track_resolution evaluation + Makefile changes) -> T003 (doc ref cleanup) -> T004 (Roadmap ADR back-reference via adr-editor) -> T005 (CI gate verification).
The track_resolution.py / track_schema.py full-file-deletion decision in T002 is an impl-time judgment: if all remaining public functions become test-only after external_guides.py removal, delete entirely; otherwise perform function-level deletion only. This follows IN-04 and the ADR D1 hedge.
T004 is independent of T001-T003 and may be executed after the deletion work is confirmed. T005 must be last.

## Tasks (2/5 resolved)

### S001 — Core Python Scripts + Registry SSoT Deletion (T001)

> IN-01 + IN-02 + IN-03 deletions grouped into one commit: scripts/external_guides.py / scripts/test_external_guides.py / scripts/atomic_write.py / scripts/test_atomic_write.py / knowledge/external/POLICY.md / knowledge/external/guides.json / knowledge/external/ directory.
> Grouping these as one commit avoids a partially broken state where external_guides.py is deleted but its dependencies still exist.
> Zero Rust code changes. Diff is pure deletion only.

- [x] **T001**: core Python scripts and registry SSoT deletion. (1) Delete scripts/external_guides.py (IN-01 / AC-01). (2) Delete scripts/test_external_guides.py (IN-01 / AC-01). (3) Delete scripts/atomic_write.py (IN-03 / AC-03). (4) Delete scripts/test_atomic_write.py (IN-03 / AC-03). (5) Delete knowledge/external/POLICY.md (IN-02 / AC-02). (6) Delete knowledge/external/guides.json (IN-02 / AC-02). (7) Remove knowledge/external/ directory (IN-02 / AC-02 — directory becomes untracked once all files are deleted). All are independent file deletions with no implementation code changes. Diff is pure deletion only. (`7c73fbc7efe88d054f78704463e4398f3bdb0508`)

### S002 — track_resolution.py Evaluation + Makefile.toml Changes (T002)

> IN-04: After T001 deletion, evaluate whether track_resolution.py residual public functions are test-only. If so, delete the entire file (and test_track_resolution.py). Otherwise delete only latest_legacy_track_dir() function. Same evaluation for track_schema.py.
> IN-05: Delete all guides-* task definition blocks from Makefile.toml: guides-list / guides-fetch / guides-usage / guides-setup / guides-clean / guides-add / guides-selftest / guides-selftest-local.
> IN-06: Remove test_atomic_write.py and test_external_guides.py from scripts-selftest-local args. Also remove test_track_resolution.py if track_resolution.py was fully deleted.
> Makefile changes are surgical line-range deletions with no impact on other task definitions.

- [x] **T002**: Evaluate and modify scripts/track_resolution.py, then delete Makefile.toml guides tasks and update scripts-selftest-local args. (1) Open scripts/track_resolution.py and check whether latest_legacy_track_dir() has callers other than external_guides.py. If the remaining public functions become test-only after external_guides.py deletion, delete scripts/track_resolution.py and scripts/test_track_resolution.py entirely. Otherwise delete only the latest_legacy_track_dir() function and fix any call sites (IN-04 / AC-04). (2) Similarly evaluate scripts/track_schema.py residual usage after external_guides.py deletion; delete entirely if no remaining callers (IN-04 judgment basis). (3) Delete the following task definition blocks from Makefile.toml: [tasks.guides-list] / [tasks.guides-fetch] / [tasks.guides-usage] / [tasks.guides-setup] / [tasks.guides-clean] / [tasks.guides-add] / [tasks.guides-selftest] / [tasks.guides-selftest-local] (IN-05 / AC-05). (4) Remove scripts/test_atomic_write.py and scripts/test_external_guides.py from the args of [tasks.scripts-selftest-local] in Makefile.toml. Also remove scripts/test_track_resolution.py from args if track_resolution.py was fully deleted (IN-06 / AC-06). (`7c73fbc7efe88d054f78704463e4398f3bdb0508`)

### S003 — Slash Command Deletion + Doc Reference Cleanup (T003)

> IN-07: Delete .claude/commands/guide/add.md.
> IN-08: Remove deleted-artifact references from CLAUDE.md / .claude/rules/09-maintainer-checklist.md / DEVELOPER_AI_WORKFLOW.md / LOCAL_DEVELOPMENT.md / .claude/settings.json / .claude/commands/track/catchup.md / track/workflow.md.
> Each file modification is limited to removing reference lines only. File structure and other content are unchanged.
> Best sequenced after T001 so the implementer can verify which references remain after deletion.

- [~] **T003**: Delete slash command and remove all doc references to deleted artifacts. (1) Delete .claude/commands/guide/add.md (IN-07 / AC-07). (2) Remove the knowledge/external/POLICY.md and knowledge/external/guides.json reference lines from the priority references section of CLAUDE.md (IN-08 / AC-08). (3) Remove the scripts/external_guides.py reference line from the enforcement section of .claude/rules/09-maintainer-checklist.md, and remove the 'external guides' mention from the Python helpers description (IN-08 / AC-08). (4) Remove all references to knowledge/external/guides.json / knowledge/external/POLICY.md / guides-fetch / guides-list etc. from DEVELOPER_AI_WORKFLOW.md (IN-08 / AC-08). (5) Remove the same kinds of references from LOCAL_DEVELOPMENT.md (IN-08 / AC-08). (6) Remove external guides references from .claude/settings.json (IN-08 / AC-08). (7) Remove external guides references from .claude/commands/track/catchup.md (IN-08 / AC-08). (8) Remove external guides references from track/workflow.md including Guiding Principles item 10 and any guides auto-injection description in the /track:plan section (IN-08 / AC-08). Each file modification is a surgical deletion of reference lines only; file structure and other content are not changed.

### S004 — Roadmap ADR Back-Reference (T004)

> IN-09: Append a back-reference blockquote note to the Phase 3 section of the Roadmap ADR indicating that D1 of 2026-04-28-1258-remove-external-guides.md supersedes the Phase 3 Rust migration plan.
> CN-01: Must be delegated to adr-editor capability; main orchestrator direct edit is prohibited.
> CN-02: Roadmap ADR YAML front-matter must not be changed; only a note in the body is appended.
> This task is independent of T001-T003 and can be executed in parallel with them if needed.

- [ ] **T004**: Append a back-reference note to the Roadmap ADR via adr-editor capability. Append a blockquote note to the Phase 3 section of knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md indicating that the direction of Phase 3 was changed from Rust migration to feature removal by decision D1 of 2026-04-28-1258-remove-external-guides.md (IN-09 / AC-09). The Roadmap ADR YAML front-matter must not be changed (CN-01 / CN-02). This task must be delegated to the adr-editor capability to comply with the 1-file-1-writer principle (CN-01).

### S005 — CI Gate Verification (T005)

> Run cargo make ci after T001-T004 are complete and verify all gates pass (AC-10).
> Zero Rust source changes means fmt-check / clippy / nextest / deny / check-layers should pass unchanged.
> Verify scripts-selftest, verify-*, and all CI gates pass after the deletion and doc changes.
> This is the final confirmation task before the track is done.

- [ ] **T005**: Run cargo make ci and verify all gates pass (AC-10). Execute after T001-T004 are complete. Since Rust source code is not changed, fmt-check / clippy / nextest / deny / check-layers should pass without difference. Confirm that scripts-selftest no longer references the removed test files (test_atomic_write.py / test_external_guides.py and optionally test_track_resolution.py) and does not produce false positives. Confirm verify-plan-artifact-refs / verify-adr-signals / verify-view-freshness and all other verify-* subcommands pass after the doc changes. Confirm full CI green before finalizing.
