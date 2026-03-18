# Verification: WF-42+WF-43 Review Workflow Critical Fixes

## Scope Verified

- [ ] WF-42/1: `is_codex_bot("chatgpt-codex-connector[bot]")` returns true
- [ ] WF-42/1: case-insensitive match works for the new login
- [ ] WF-42/2: `poll_review` detects bot thumbs-up reaction as zero-findings completion (trigger timestamp filtered)
- [ ] WF-42/2: `poll_review` still detects COMMENTED review for findings-present case
- [ ] WF-42/2: `review_cycle` handles zero-findings without requiring review JSON
- [ ] WF-42/2: standalone `poll_review` outputs `{"verdict":"zero_findings","findings":[]}` for reaction-only completion
- [ ] WF-43: `index_tree_hash_normalizing` produces stable hash across re-stage of metadata.json
- [ ] WF-43: `record-round` -> re-stage -> `check-approved` succeeds with normalized hash
- [ ] WF-43: source code change correctly invalidates the normalized hash
- [ ] WF-43: metadata.json review.status tamper is detected by normalized hash
- [ ] WF-43: sentinel value is `"PENDING"` (not null)
- [ ] `cargo make ci` passes

## Manual Verification Steps

1. Run `cargo make test` and confirm all new tests pass
2. For WF-42/2: verify reaction detection with mock GhClient returning bot +1 reaction
3. For WF-43: manually verify the normalization approach:
   - Stage all files including metadata.json
   - Run `sotp review record-round` with a mock verdict (writes code_hash to metadata.json)
   - Re-stage metadata.json (now with code_hash)
   - Run `sotp review check-approved` and confirm it succeeds (normalized hash matches)
   - Tamper with review.status in metadata.json, re-stage, confirm check-approved fails

## Result

(未実施)

## Open Issues

(なし)

## Verified At

(未検証)
