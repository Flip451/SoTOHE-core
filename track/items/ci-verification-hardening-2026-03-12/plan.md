<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CI Verification Hardening — container entry points, placeholder false-positive fix, planning-phase bypass, i18n scaffold detection

Fix 4 CI verification issues: WF-23 (container CI entry points), WF-09 (TODO false positives in code blocks), WF-11 (tech-stack TODO blocking during planning), WF-13 (hardcoded English verification scaffold lines).
All changes are Python scripts + Makefile.toml + GitHub Actions workflow. No Rust code changes.
Backward compatible: existing passing CI must still pass after changes.
Fail-closed policy: metadata read failures must reject, not silently skip.

## Container CI Entry Points (WF-23)

Add public ci-container and ci-rust-container tasks to Makefile.toml.
Update .github/workflows/ci.yml to use ci-container instead of ci-local.
Update test assertions in test_make_wrappers.py and test_verify_scripts.py.

- [x] Add public ci-container and ci-rust-container tasks to Makefile.toml that delegate to private ci-local/ci-rust-local (WF-23)
- [x] Update .github/workflows/ci.yml to use ci-container, update test_make_wrappers.py and test_verify_scripts.py assertions (WF-23)

## Placeholder Detection Improvements (WF-09, WF-13)

Refactor placeholder_lines() to skip fenced code blocks (``` markers).
Add Japanese body-line equivalents to VERIFICATION_SCAFFOLD_LINES for i18n scaffold detection.
Add regression tests for both improvements.

- [x] Refactor placeholder_lines() in verify_latest_track_files.py to skip fenced code blocks; add Japanese body-line equivalents to VERIFICATION_SCAFFOLD_LINES for i18n scaffold detection (WF-09, WF-13)
- [x] Add regression tests for code-block TODO skip, fence-outside TODO detection, and Japanese verification headings in test_verify_scripts.py (WF-09, WF-13)

## Tech-stack Planning Phase Bypass (WF-11)

Change bypass logic in verify_tech_stack_ready.py: allow TODO when all tracks are planned.
Read metadata.json status for each track, fail-closed on unreadable metadata.
Add regression tests for new planning-phase rule.

- [x] Change verify_tech_stack_ready.py bypass logic: allow TODO when all tracks are planned status; fail-closed on unreadable metadata (WF-11)
- [x] Add regression tests for planning-phase bypass: pass when all planned, fail when in_progress/done exists, fail on missing metadata.json, fail on corrupt/non-UTF-8 metadata, fail on unreadable metadata (WF-11)

## Integration Validation

Run full test suite and CI to confirm all changes work together.

- [x] Full integration validation: run scripts-selftest and cargo make ci to confirm all changes pass together
