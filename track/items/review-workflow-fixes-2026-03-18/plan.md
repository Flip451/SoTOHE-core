<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-42+WF-43: review workflow critical fixes (bot login + completion detection + code_hash cycle)

Fix three critical review workflow issues: WF-42/1 (CODEX_BOT_LOGINS missing chatgpt-codex-connector[bot]), WF-42/2 (poll_review completion detection ignores thumbs-up reaction for zero-findings), and WF-43 (code_hash self-referential cycle in metadata.json). WF-42/1 is a constant addition. WF-42/2 requires adding issues/{pr}/reactions API polling alongside existing review/comment checks. WF-43 requires excluding metadata.json from git write-tree hash computation.

## Phase 1: WF-42 bot login fix

Add chatgpt-codex-connector[bot] to the CODEX_BOT_LOGINS constant in apps/cli/src/commands/pr.rs. Add test cases for the new login including case-insensitive matching.

- [ ] Add chatgpt-codex-connector[bot] to CODEX_BOT_LOGINS + tests

## Phase 2: WF-42 completion detection via reaction API

Extend poll_review and poll_review_for_cycle to check issues/{pr}/reactions API for a bot thumbs-up (+1) reaction as the zero-findings completion signal. When the poller detects a reaction-only completion (no submitted review object), return a sentinel value (e.g. None for review JSON) so that review_cycle can distinguish zero-findings from timeout. Update review_cycle to skip parse_review when poll returns zero-findings and directly report success. Confirmed behavior from PR #2 and #36: findings-none -> bot posts thumbs-up reaction + 'Didn't find any major issues' comment (no review object); findings-present -> bot posts a COMMENTED review with inline findings. Add list_reactions method to GhClient trait.

- [ ] Add thumbs-up reaction detection to poll_review + update review_cycle to handle zero-findings (reaction-only) without requiring a review object + tests

## Phase 3: WF-43 hash exclusion infrastructure

Add index_tree_hash_excluding(&self, exclude_paths: &[&str]) method to the GitRepository trait in libs/domain and implement it in libs/infrastructure/src/git_cli.rs. Use git ls-files --stage + git mktree to compute a tree hash excluding specified paths without modifying the staging area.

- [ ] Add index_tree_hash_excluding method to GitRepository trait + infrastructure impl + tests

## Phase 4: WF-43 integration into review commands

Update run_record_round and run_check_approved in apps/cli/src/commands/review.rs to call index_tree_hash_excluding with the track's metadata.json path instead of index_tree_hash. Both commands must use the same exclusion so hashes are comparable.

- [ ] Use metadata.json-excluded hash in record-round and check-approved + tests

## Phase 5: End-to-end verification

Integration tests: (1) stage all -> record-round -> re-stage metadata.json -> check-approved succeeds, (2) source code change after record-round causes check-approved to fail (security invariant), (3) poll_review detects bot thumbs-up reaction as zero-findings completion.

- [ ] Integration test: review -> commit flow with metadata.json hash exclusion + pr-review polling with reaction detection
