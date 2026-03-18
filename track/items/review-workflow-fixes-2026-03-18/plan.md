<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-42+WF-43: review workflow critical fixes (bot login + code_hash cycle)

Fix two critical review workflow bugs: WF-42 (CODEX_BOT_LOGINS missing chatgpt-codex-connector[bot]) and WF-43 (code_hash self-referential cycle in metadata.json). WF-42 is a 1-line constant addition. WF-43 requires excluding metadata.json from git write-tree hash computation so that writing review state to metadata.json does not invalidate the hash.

## Phase 1: WF-42 bot login fix

Add chatgpt-codex-connector[bot] to the CODEX_BOT_LOGINS constant in apps/cli/src/commands/pr.rs. Add test cases for the new login including case-insensitive matching.

- [ ] Add chatgpt-codex-connector[bot] to CODEX_BOT_LOGINS + tests

## Phase 2: WF-43 hash exclusion infrastructure

Add index_tree_hash_excluding(&self, exclude_paths: &[&str]) method to the GitRepository trait in libs/domain and implement it in libs/infrastructure/src/git_cli.rs. Use git ls-files --stage + git mktree to compute a tree hash excluding specified paths without modifying the staging area.

- [ ] Add index_tree_hash_excluding method to GitRepository trait + infrastructure impl + tests

## Phase 3: WF-43 integration into review commands

Update run_record_round and run_check_approved in apps/cli/src/commands/review.rs to call index_tree_hash_excluding with the track's metadata.json path instead of index_tree_hash. Both commands must use the same exclusion so hashes are comparable.

- [ ] Use metadata.json-excluded hash in record-round and check-approved + tests

## Phase 4: End-to-end verification

Add integration test that simulates: stage all → record-round → re-stage metadata.json → check-approved succeeds. Also test that modifying a source file after record-round causes check-approved to fail (security invariant).

- [ ] Integration test: review -> commit flow with metadata.json hash exclusion
