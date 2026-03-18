<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-42+WF-43: review workflow critical fixes (bot login + completion detection + code_hash cycle)

Fix three critical review workflow issues: WF-42/1 (CODEX_BOT_LOGINS missing chatgpt-codex-connector[bot]), WF-42/2 (poll_review completion detection ignores thumbs-up reaction for zero-findings), and WF-43 (code_hash self-referential cycle in metadata.json). WF-42/1 is a constant addition. WF-42/2 requires adding reaction API polling + review_cycle zero-findings path. WF-43 uses staging order control: compute hash from staged index before writing code_hash to disk, never re-stage metadata.json between record-round and check-approved.

## Phase 1: WF-42 bot login fix

Add chatgpt-codex-connector[bot] to the CODEX_BOT_LOGINS constant in apps/cli/src/commands/pr.rs. Add test cases for the new login including case-insensitive matching.

- [ ] Add chatgpt-codex-connector[bot] to CODEX_BOT_LOGINS + tests

## Phase 2: WF-42 completion detection via reaction API

Extend poll_review and poll_review_for_cycle to check issues/{pr}/reactions API for a bot thumbs-up (+1) reaction as the zero-findings completion signal. Filter reactions by created_at >= trigger_timestamp to avoid matching stale reactions from previous review cycles. For standalone poll_review: output {"verdict":"zero_findings","findings":[]} JSON to stdout (existing contract). For poll_review_for_cycle: return a typed enum (ReviewFound/ZeroFindings/Timeout) so review_cycle can distinguish zero-findings from timeout and skip parse_review accordingly. Add list_reactions method to GhClient trait.

- [ ] Add thumbs-up reaction detection to poll_review + update review_cycle to handle zero-findings (reaction-only) without requiring a review object + tests

## Phase 3: WF-43 staging order control in record-round

Enforce staging order in run_record_round (apps/cli/src/commands/review.rs): (1) compute hash from staged index via existing index_tree_hash(), (2) write review state + code_hash to metadata.json on disk via FsTrackStore::update(), (3) do NOT re-stage metadata.json. The key invariant: staged metadata.json remains in its pre-review state so the hash is stable. No new GitRepository methods needed — use existing index_tree_hash() as-is.

- [ ] Enforce staging order in record-round: compute hash from staged index before disk write, do not re-stage metadata.json + tests

## Phase 4: WF-43 check-approved + commit wrapper update

Update run_check_approved to read code_hash from the on-disk metadata.json (not from staged index) while computing the hash from staged index. Since staged metadata.json is still in pre-review state, the hash matches. Update the commit wrapper (track-commit-message / sotp make track-commit-message) to stage metadata.json AFTER check-approved passes but BEFORE git commit, so the committed version includes code_hash.

- [ ] Update check-approved to read code_hash from disk metadata.json while computing hash from staged index + update commit wrapper to stage metadata.json after check-approved + tests

## Phase 5: End-to-end verification

Integration tests: (1) add-all -> record-round (disk write only) -> check-approved (staged index hash matches disk code_hash) -> stage metadata.json -> commit succeeds, (2) source code change after record-round causes check-approved to fail (security invariant), (3) poll_review detects bot thumbs-up reaction as zero-findings completion with trigger timestamp filter.

- [ ] Integration test: full review -> commit flow with staging order control + pr-review polling with reaction detection
