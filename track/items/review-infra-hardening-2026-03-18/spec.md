---
status: draft
version: "1.0"
---

# Spec: Review Infrastructure Hardening

## Goal

Solve three operational problems discovered during phase1-sotp-hardening and review-quality-quick-wins:
1. Codex verdict extraction failures (exit 105) with no diagnostic information
2. No enforcement of sequential model escalation (fast → final)
3. No gate preventing commits without completed review

## Scope

- `apps/cli/src/commands/review.rs` — stderr capture, verdict fallback, --round-type flag
- `libs/domain/src/track.rs` — review section in TrackMetadata
- `libs/infrastructure/src/track/codec.rs` — review section added to schema v3 via `#[serde(default)]` (no schema version bump)
- `apps/cli/src/commands/make.rs` (or commit wrapper) — review status guard
- `tmp/reviewer-runtime/` — session log files (not committed)

## Constraints

- No new crate dependencies
- Backward compatible: metadata.json without `review` section must still parse (use `#[serde(default)]`)
- Existing tracks with schema_version 3 must not break
- stderr capture must not break existing stdout verdict output

## Acceptance Criteria

- [ ] Codex stderr is captured to `tmp/reviewer-runtime/codex-session-{pid}.log`
- [ ] When `codex-last-message` is empty, verdict is extracted from stderr log (fallback)
- [ ] `metadata.json` supports `review` section with `status`, `code_hash`, `groups` (keys: `fast`/`final`)
- [ ] `sotp review codex-local` remains stateless — outputs verdict on stdout only, does not write metadata.json (parallel-safe)
- [ ] `sotp review record-round --round-type fast|final --group <name> --verdict <json> --expected-groups <comma-separated>` writes results to metadata.json (called by orchestrator after parallel reviewers complete)
- [ ] `--expected-groups` defines the authoritative set of groups; promotion to `fast_passed`/`approved` only occurs when ALL expected groups have recorded `zero_findings` for that round type
- [ ] `sotp review record-round --round-type final` fails if `review.status != fast_passed` OR `code_hash` doesn't match current git tree hash
- [ ] If any group reports `findings_remain`, `review.status` stays at current state (no promotion)
- [ ] `track-commit-message` refuses commit when `review.status != approved` OR `code_hash` is stale
- [ ] Code changes (git tree hash mismatch) detected at both `record-round` and `track-commit-message` time, setting `review.status` to `invalidated`. `record-round` with stale code_hash **rejects without recording** — caller must re-run the reviewer on current code first
- [ ] `review.status` uses explicit enum: `not_started` (default/initial), `invalidated` (code changed after review), `fast_passed`, `approved` — no null
- [ ] `cargo make ci` passes
- [ ] Backward compatibility: schema_version 3 metadata.json still parses without error
