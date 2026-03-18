# Verification: WF-42+WF-43 Review Workflow Critical Fixes

## Scope Verified

- [ ] WF-42/1: `is_codex_bot("chatgpt-codex-connector[bot]")` returns true
- [ ] WF-42/1: case-insensitive match works for the new login
- [ ] WF-42/2: `poll_review` detects bot thumbs-up reaction as zero-findings completion (trigger timestamp filtered)
- [ ] WF-42/2: `poll_review` still detects COMMENTED review for findings-present case
- [ ] WF-42/2: `review_cycle` handles zero-findings without requiring review JSON
- [ ] WF-42/2: standalone `poll_review` outputs `{"verdict":"zero_findings","findings":[]}` for reaction-only completion
- [ ] WF-43: `record-round` computes hash from staged index, writes to disk without re-staging
- [ ] WF-43: `check-approved` reads code_hash from disk, computes hash from staged index, matches
- [ ] WF-43: source code change correctly invalidates the hash
- [ ] WF-43: commit wrapper stages metadata.json after check-approved passes
- [ ] `cargo make ci` passes

## Manual Verification Steps

1. Run `cargo make test` and confirm all new tests pass
2. For WF-42/2: verify reaction detection with mock GhClient returning bot +1 reaction
3. For WF-43: manually verify the staging order control:
   - Stage all files including metadata.json (pre-review state)
   - Run `sotp review record-round` with a mock verdict (writes to disk only)
   - Do NOT re-stage metadata.json
   - Run `sotp review check-approved` and confirm it succeeds
   - Stage metadata.json and commit

## Result

(未実施)

## Open Issues

(なし)

## Verified At

(未検証)
