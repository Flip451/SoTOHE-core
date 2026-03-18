<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-42+WF-43: review workflow critical fixes (bot login + completion detection + code_hash cycle)

Fix three critical review workflow issues: WF-42/1 (CODEX_BOT_LOGINS missing chatgpt-codex-connector[bot]), WF-42/2 (poll_review completion detection ignores thumbs-up reaction for zero-findings), and WF-43 (code_hash self-referential cycle in metadata.json). WF-42/1 is a constant addition. WF-42/2 requires adding reaction API polling + review_cycle zero-findings path. WF-43 uses code_hash normalization (method D): replace review.code_hash with "PENDING" sentinel before computing tree hash, ensuring post-update review state is included and serialization is deterministic (BTreeMap for groups ordering).

## Phase 1: WF-42 bot login fix

Add chatgpt-codex-connector[bot] to the CODEX_BOT_LOGINS constant in apps/cli/src/commands/pr.rs. Add test cases for the new login including case-insensitive matching.

- [ ] Add chatgpt-codex-connector[bot] to CODEX_BOT_LOGINS + tests

## Phase 2: WF-42 completion detection via reaction API

Extend poll_review and poll_review_for_cycle to check issues/{pr}/reactions API for a bot thumbs-up (+1) reaction as the zero-findings completion signal. Filter reactions by created_at >= trigger_timestamp to avoid matching stale reactions from previous review cycles. For standalone poll_review: output {"verdict":"zero_findings","findings":[]} JSON to stdout (existing contract). For poll_review_for_cycle: return a typed enum (ReviewFound/ZeroFindings/Timeout) so review_cycle can distinguish zero-findings from timeout and skip parse_review accordingly. Add list_reactions method to GhClient trait.

- [ ] Add thumbs-up reaction detection to poll_review + update review_cycle to handle zero-findings (reaction-only) without requiring a review object + tests

## Phase 3: WF-43 normalized hash infrastructure

Add index_tree_hash_normalizing method to GitRepository trait in libs/domain and implement in libs/infrastructure/src/git_cli.rs. The method: (1) reads metadata.json blob from staged index, (2) parses JSON and replaces review.code_hash with "PENDING" sentinel string (never null — null would become None and skip the stale-hash guard), (3) serializes deterministically via serde_json, (4) creates normalized blob with git hash-object -w, (5) copies current index to a temp GIT_INDEX_FILE, (6) swaps metadata.json blob via git update-index --cacheinfo, (7) runs git write-tree on temp index, (8) cleans up. IMPORTANT: switch TrackReviewDocument.groups from HashMap to BTreeMap (or sort keys before serialization) to guarantee deterministic JSON output across separate CLI invocations.

- [ ] Add index_tree_hash_normalizing method to GitRepository trait: replace review.code_hash with "PENDING" sentinel in a temp index, then git write-tree. Ensure deterministic serialization by switching TrackReviewDocument.groups from HashMap to BTreeMap + tests

## Phase 4: WF-43 integration into review commands

Update run_record_round and run_check_approved in apps/cli/src/commands/review.rs. CRITICAL ordering for record-round: (1) write review verdict/status/groups to metadata.json via store.update(), (2) re-stage metadata.json, (3) normalize code_hash to "PENDING" and compute tree hash H1 from post-update metadata, (4) write code_hash: H1 back to metadata.json, (5) re-stage. check-approved: (1) normalize code_hash to "PENDING" on staged metadata.json (which has code_hash: H1), (2) compute tree hash -> H1, (3) compare with stored code_hash H1 -> match. Both see identical post-update metadata (except code_hash itself) so hashes agree.

- [ ] Use normalized hash in record-round and check-approved: record-round writes review state first, then normalizes code_hash to "PENDING" and computes hash, then writes code_hash back. check-approved normalizes and compares. Both see post-update metadata + tests

## Phase 5: End-to-end verification

Integration tests: (1) record-round -> re-stage -> check-approved succeeds (normalized hash stable), (2) source code change causes check-approved to fail, (3) metadata.json review.status tamper causes check-approved to fail, (4) poll_review detects bot thumbs-up reaction with trigger timestamp filter, (5) multiple review groups produce consistent hash across separate CLI invocations (BTreeMap ordering).

- [ ] Integration test: full review -> commit flow with normalized hash (re-stage OK) + pr-review polling with reaction detection + tamper detection for non-code_hash metadata fields
