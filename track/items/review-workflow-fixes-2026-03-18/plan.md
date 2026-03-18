<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-42+WF-43: review workflow critical fixes (bot login + completion detection + code_hash cycle)

Fix three critical review workflow issues: WF-42/1 (CODEX_BOT_LOGINS missing chatgpt-codex-connector[bot]), WF-42/2 (poll_review ignores thumbs-up reaction + comment fallback for zero-findings), and WF-43 (code_hash self-referential cycle). WF-43 uses method D: normalize review.code_hash to "PENDING" and updated_at to epoch before hashing. Pre-update freshness check is preserved. BTreeMap ensures deterministic group ordering. Sentinel is always inserted, never null.

## Phase 1: WF-42 bot login fix

Add chatgpt-codex-connector[bot] to the CODEX_BOT_LOGINS constant in apps/cli/src/commands/pr.rs. Add test cases for the new login including case-insensitive matching.

- [ ] Add chatgpt-codex-connector[bot] to CODEX_BOT_LOGINS + tests

## Phase 2: WF-42 completion detection via reaction + comment fallback

Extend poll_review and poll_review_for_cycle with 2-stage zero-findings detection: (1) check issues/{pr}/reactions API for bot +1 with created_at >= trigger_timestamp, (2) fallback: check bot issue comments for 'Didn't find any major issues' text with created_at >= trigger_timestamp (handles GitHub's reaction dedup where repeated +1 keeps old created_at). For standalone poll_review: output {"verdict":"zero_findings","findings":[]} JSON to stdout. For poll_review_for_cycle: return typed enum (ReviewFound/ZeroFindings/Timeout). review_cycle skips parse_review on ZeroFindings. Add list_reactions to GhClient trait.

- [ ] Add zero-findings detection to poll_review (reaction + comment text fallback) + update review_cycle to handle zero-findings without requiring a review object + tests

## Phase 3: WF-43 normalized hash infrastructure

Add index_tree_hash_normalizing to GitRepository trait (libs/infrastructure/src/git_cli.rs). Normalizes 2 fields: review.code_hash -> "PENDING", updated_at -> "1970-01-01T00:00:00Z". Implementation: (1) read metadata.json blob from staged index, (2) parse JSON, INSERT code_hash: "PENDING" if absent (never null/None), replace updated_at with epoch, (3) serialize deterministically, (4) git hash-object -w, (5) copy index to temp GIT_INDEX_FILE, (6) git update-index --cacheinfo, (7) git write-tree, (8) cleanup. Switch TrackReviewDocument.groups to BTreeMap. Add ReviewState::record_round_with_pending (freshness check + review state + PENDING sentinel) and set_code_hash (hash write-back) to libs/domain/src/review.rs. Keep existing record_round for compatibility.

- [ ] Add index_tree_hash_normalizing to GitRepository trait (libs/infrastructure/src/git_cli.rs): normalize review.code_hash to "PENDING" and updated_at to epoch in temp index, then git write-tree. Switch TrackReviewDocument.groups to BTreeMap. Add ReviewState::record_round_with_pending and set_code_hash domain methods (libs/domain/src/review.rs) + tests

## Phase 4: WF-43 integration into review commands

Update run_record_round in apps/cli/src/commands/review.rs: (1) compute pre-update normalized hash, (2) single store.update() calls record_round_with_pending(round_type, group, result, expected_groups, pre_update_hash) — does freshness check + review state + code_hash: PENDING in one write, (3) re-stage, (4) compute post-update normalized hash H1, (5) second store.update() calls set_code_hash(H1) — updated_at changes but normalized, (6) re-stage. Update run_check_approved: normalize staged metadata.json, compute hash, compare with stored code_hash.

- [ ] Use normalized hash in record-round and check-approved: record-round computes pre-update normalized hash, calls record_round_with_pending (freshness check + review state + PENDING), re-stages, computes post-update normalized hash H1, calls set_code_hash(H1), re-stages. check-approved normalizes and compares + tests

## Phase 5: End-to-end verification

Integration tests: (1) record-round -> re-stage -> check-approved succeeds, (2) source code change fails check-approved, (3) review.status tamper fails check-approved, (4) first round with no prior code_hash succeeds, (5) pre-update freshness check detects code change between rounds, (6) updated_at variation does not affect hash, (7) reaction + comment fallback detects zero-findings, (8) multi-group BTreeMap produces stable hash.

- [ ] Integration tests: full review->commit with re-stage, pre-update freshness check, first-round (no prior code_hash), updated_at stability, reaction+comment fallback detection, multi-group BTreeMap ordering, tamper detection
