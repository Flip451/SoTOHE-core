# Verification — Review Infrastructure Hardening

## Scope Verified

- [ ] T001: Codex stderr capture + verdict fallback
- [ ] T002: metadata.json review section
- [ ] T003: --round-type flag + sequential escalation enforcement
- [ ] T004: Commit guard

## Manual Verification Steps

### T001: Codex stderr capture + verdict fallback

1. (log creation) Run `sotp review codex-local`, verify `tmp/reviewer-runtime/codex-session-*.log` is created
2. (log content) Verify stderr log contains Codex session output (model, sandbox, file reads)
3. (fallback) Simulate empty `codex-last-message` with non-empty stderr containing verdict JSON, verify verdict is extracted
4. (normal path) When `codex-last-message` has valid verdict, verify stderr fallback is NOT used (primary path preferred)

### T002: metadata.json review section

5. (schema v3 extension) Create metadata.json with review section (schema_version stays 3), verify it parses correctly
6. (backward compat) Parse existing schema_version 3 metadata.json without review section, verify no error
7. (review.status enum) Verify explicit enum: `not_started` (default), `invalidated` (code changed), `fast_passed`, `approved` — no null. Transitions: not_started → fast_passed → approved, any state → invalidated on code change
8. (code_hash) Verify code_hash stores git tree hash
9. (groups persistence) After running `--round-type fast`, verify `review.groups.{name}.fast` is written with round number, verdict, and timestamp
10. (groups final) After running `--round-type final`, verify `review.groups.{name}.final` is written (key matches CLI flag name)

### T003: sotp review record-round + sequential escalation

11. (parallel safe) Verify `sotp review codex-local` does NOT write to metadata.json (stateless) AND still outputs verdict JSON on stdout
12. (record fast) Run `sotp review record-round --round-type fast --group infra-domain --expected-groups infra-domain,other --verdict '{"verdict":"zero_findings","findings":[]}'`, verify `review.groups.infra-domain.fast` is written
13. (multi-group aggregation) Record fast zero_findings for group A, then group B (both with `--expected-groups A,B`). Verify both `review.groups.A.fast` and `review.groups.B.fast` exist — second call must not overwrite first group's result
14. (partial group no promote) Record fast zero_findings for group A only (`--expected-groups A,B`), verify `review.status` does NOT promote to fast_passed (still not_started)
15. (findings block promote) Record fast findings_remain for group A, then zero_findings for group B (`--expected-groups A,B`), verify `review.status` does NOT promote (findings in A blocks)
16. (fast auto-promote) Record fast zero_findings for all expected groups, verify `review.status = fast_passed`
17. (record final after fast_passed) With review.status = fast_passed and current code_hash, run `record-round --round-type final --group infra-domain --expected-groups infra-domain,other --verdict ...`, verify it succeeds
18. (final without fast_passed) With review.status = not_started, run `record-round --round-type final`, verify **rejection**
19. (final stale code_hash) With review.status = fast_passed but stale code_hash, run `record-round --round-type final`, verify **rejection**
20. (final partial no promote) Record final zero_findings for group A only (`--expected-groups A,B`), verify `review.status` stays at fast_passed (not promoted to approved)
21. (final findings block promote) Record final findings_remain for group A, zero_findings for group B (`--expected-groups A,B`), verify `review.status` stays at fast_passed
22. (final auto-promote) Record final zero_findings for all expected groups, verify `review.status = approved`
23. (stale rejection at record time) With review.status = fast_passed and stale code_hash, run `record-round --round-type fast --expected-groups A`, verify the command **rejects without recording**, sets review.status to `invalidated`, and reports the stale state in its error output

### T004: Commit guard

24. (approved passes) Set review.status to approved with current code_hash, run track-commit-message, verify commit proceeds
25. (non-approved blocked) Set review.status to fast_passed, run track-commit-message, verify **rejection**
26. (stale at commit) Set review.status to approved with stale code_hash, run track-commit-message, verify **rejection** AND verify review.status is set to `invalidated` as side effect
27. (clean CI) `cargo make ci` passes on clean tree

## Result

- (pending)

## Open Issues

- (none yet)

## Verified At

- (pending)
