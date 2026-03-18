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

Add index_tree_hash_normalizing to GitRepository trait. Normalizes 2 fields: review.code_hash -> "PENDING", updated_at -> "1970-01-01T00:00:00Z". Implementation: (1) read metadata.json blob from staged index, (2) parse JSON, INSERT code_hash: "PENDING" if absent (never leave as null/None — None skips the stale-hash guard), replace updated_at with epoch, (3) serialize deterministically with serde_json, (4) git hash-object -w for normalized blob, (5) copy index to temp GIT_INDEX_FILE, (6) git update-index --cacheinfo to swap blob, (7) git write-tree on temp index, (8) cleanup. Switch TrackReviewDocument.groups from HashMap to BTreeMap for key ordering.

- [ ] Add index_tree_hash_normalizing to GitRepository trait: normalize review.code_hash to "PENDING" and updated_at to epoch sentinel in temp index, then git write-tree. Switch TrackReviewDocument.groups to BTreeMap for deterministic serialization. Insert "PENDING" even when code_hash is initially None + tests

## Phase 4: WF-43 integration into review commands

Update run_record_round: (1) pre-update freshness check — compute normalized hash from current staged index, compare with stored code_hash (skip if None/first round), reject on mismatch (StaleCodeHash), (2) single store.update() writes review verdict/status/groups + code_hash: "PENDING" — this is a single write so updated_at changes only once, (3) re-stage metadata.json, (4) compute post-update normalized hash H1, (5) second store.update() sets code_hash: H1 only — updated_at changes again but is normalized so hash unaffected, (6) re-stage. Update run_check_approved: normalize staged metadata.json (code_hash -> PENDING, updated_at -> epoch), compute hash, compare with stored code_hash.

- [ ] Use normalized hash in record-round and check-approved: record-round does pre-update freshness check, then writes review state + code_hash "PENDING" in single store.update(), computes post-update normalized hash H1, writes code_hash H1 in second store.update(). check-approved normalizes and compares + tests

## Phase 5: End-to-end verification

Integration tests: (1) record-round -> re-stage -> check-approved succeeds, (2) source code change fails check-approved, (3) review.status tamper fails check-approved, (4) first round with no prior code_hash succeeds, (5) pre-update freshness check detects code change between rounds, (6) updated_at variation does not affect hash, (7) reaction + comment fallback detects zero-findings, (8) multi-group BTreeMap produces stable hash.

- [ ] Integration tests: full review->commit with re-stage, pre-update freshness check, first-round (no prior code_hash), updated_at stability, reaction+comment fallback detection, multi-group BTreeMap ordering, tamper detection
