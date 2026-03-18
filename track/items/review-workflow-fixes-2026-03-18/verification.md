# Verification: WF-42+WF-43 Review Workflow Critical Fixes

## Scope Verified

- [ ] WF-42: `is_codex_bot("chatgpt-codex-connector[bot]")` returns true
- [ ] WF-42: case-insensitive match works for the new login
- [ ] WF-43: `record-round` → re-stage → `check-approved` hash matches
- [ ] WF-43: source code change correctly invalidates the hash
- [ ] WF-43: `index_tree_hash()` (original) still works unchanged
- [ ] `cargo make ci` passes

## Manual Verification Steps

1. Run `cargo make test` and confirm all new tests pass
2. For WF-43: manually verify the hash cycle is broken by:
   - Stage all files including metadata.json
   - Run `sotp review record-round` with a mock verdict
   - Re-stage metadata.json
   - Run `sotp review check-approved` and confirm it succeeds

## Result

(未実施)

## Open Issues

(なし)

## Verified At

(未検証)
