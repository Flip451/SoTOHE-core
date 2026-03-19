# Verification: WF-42+WF-43 Review Workflow Critical Fixes

## Scope Verified

- [x] WF-42/1: `is_codex_bot("chatgpt-codex-connector[bot]")` returns true
- [x] WF-42/1: case-insensitive match works for the new login
- [x] WF-42/2: `poll_review` detects bot thumbs-up reaction as zero-findings (trigger timestamp filtered)
- [x] WF-42/2: `poll_review` falls back to bot comment text when reaction is stale
- [x] WF-42/2: `poll_review` still detects COMMENTED review for findings-present case
- [x] WF-42/2: `review_cycle` handles zero-findings without requiring review JSON
- [x] WF-42/2: standalone `poll_review` outputs `{"verdict":"zero_findings","findings":[]}` for zero-findings
- [x] WF-43: `index_tree_hash_normalizing` normalizes code_hash to "PENDING" and updated_at to epoch
- [x] WF-43: normalizer inserts "PENDING" when code_hash is initially None/absent
- [x] WF-43: `record-round` -> re-stage -> `check-approved` succeeds with normalized hash
- [x] WF-43: pre-update freshness check detects code change between review rounds
- [x] WF-43: first round (no prior code_hash) succeeds without freshness check error
- [x] WF-43: updated_at variation between writes does not affect normalized hash
- [x] WF-43: source code change correctly invalidates the normalized hash
- [x] WF-43: metadata.json review.status tamper is detected by normalized hash
- [x] WF-43: multiple review groups produce stable hash (BTreeMap ordering)
- [x] `cargo make ci` passes

## Manual Verification Steps

1. Run `cargo make test` and confirm all new tests pass
2. For WF-42/2: verify reaction detection and comment text fallback with mock GhClient
3. For WF-43: manually verify the normalization approach:
   - Stage all files including metadata.json
   - Run `sotp review record-round` with a mock verdict
   - Re-stage metadata.json (now with code_hash)
   - Run `sotp review check-approved` and confirm it succeeds
   - Tamper with review.status, re-stage, confirm check-approved fails
   - Run a second review round, confirm pre-update freshness check passes

## Result

All 1012 tests pass (1005 existing + 7 new integration tests). `cargo make ci` passes all gates including fmt-check, clippy, test, deny, check-layers, verify-arch-docs, and all verify-* scripts.

## Open Issues

- WF-42/2 reaction + comment fallback tests use mock GhClient only; end-to-end verification against live GitHub API requires a real PR with Codex Cloud installed.
- WF-43 integration tests simulate the record-round → check-approved flow at the domain+infrastructure level; full CLI-level end-to-end test blocked by WF-43 bug itself (which this track fixes).

## Verified At

2026-03-18
