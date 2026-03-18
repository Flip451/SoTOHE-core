# Verification: WF-42+WF-43 Review Workflow Critical Fixes

## Scope Verified

- [ ] WF-42/1: `is_codex_bot("chatgpt-codex-connector[bot]")` returns true
- [ ] WF-42/1: case-insensitive match works for the new login
- [ ] WF-42/2: `poll_review` detects bot thumbs-up reaction as zero-findings (trigger timestamp filtered)
- [ ] WF-42/2: `poll_review` falls back to bot comment text when reaction is stale
- [ ] WF-42/2: `poll_review` still detects COMMENTED review for findings-present case
- [ ] WF-42/2: `review_cycle` handles zero-findings without requiring review JSON
- [ ] WF-42/2: standalone `poll_review` outputs `{"verdict":"zero_findings","findings":[]}` for zero-findings
- [ ] WF-43: `index_tree_hash_normalizing` normalizes code_hash to "PENDING" and updated_at to epoch
- [ ] WF-43: normalizer inserts "PENDING" when code_hash is initially None/absent
- [ ] WF-43: `record-round` -> re-stage -> `check-approved` succeeds with normalized hash
- [ ] WF-43: pre-update freshness check detects code change between review rounds
- [ ] WF-43: first round (no prior code_hash) succeeds without freshness check error
- [ ] WF-43: updated_at variation between writes does not affect normalized hash
- [ ] WF-43: source code change correctly invalidates the normalized hash
- [ ] WF-43: metadata.json review.status tamper is detected by normalized hash
- [ ] WF-43: multiple review groups produce stable hash (BTreeMap ordering)
- [ ] `cargo make ci` passes

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

(未実施)

## Open Issues

(なし)

## Verified At

(未検証)
