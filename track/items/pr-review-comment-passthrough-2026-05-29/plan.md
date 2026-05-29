<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# PR レビュー結果を解釈せず最新ラウンドのコメントを agent に渡す

## Tasks (6/6 resolved)

### S1 — Usecase layer type cleanup

> Remove interpretation fields from the two data types in libs/usecase/src/pr_review.rs.
> T001 shrinks PrReviewFinding (drop severity, rule_id) and PrReviewResult (drop actionable_count, passed).
> T002 deletes the two interpretation functions (classify_severity, parse_body_findings) and their tests.
> These two tasks are the foundation for all downstream changes: cli-composition cannot compile until the struct shapes are fixed.

- [x] **T001**: Shrink PrReviewFinding and PrReviewResult in libs/usecase/src/pr_review.rs: remove severity and rule_id fields from PrReviewFinding; remove actionable_count, passed fields from PrReviewResult. Update derive impls (Debug/Clone/PartialEq/Eq) and doc comments to match the passthrough shape declared in the type catalogue. (`64b1c921a9e3d52d11281bd6641279ad0a360c18`)
- [x] **T002**: Delete classify_severity and parse_body_findings functions from libs/usecase/src/pr_review.rs, along with all their test cases (classify_severity — 5 tests, parse_body_findings — 14 tests). Update the module-level doc comment to remove references to the deleted functions. (`64b1c921a9e3d52d11281bd6641279ad0a360c18`)

### S2 — cli-composition passthrough rewrite

> Rewrite the parsing and presentation layer in apps/cli-composition to match the passthrough contract.
> T003 rewrites parse_review (inline-comments-only findings, no severity) and format_review_summary (passthrough output), and fixes the polling loop to select the latest-round review.
> T004 updates pr_review_cycle exit-code logic (ReviewFound is always exit 0) and keeps the cli test_helpers in sync with the new struct shapes.

- [x] **T003**: Rewrite parse_review in apps/cli-composition/src/pr/poll.rs: drop classify_severity and parse_body_findings call sites; build PrReviewResult.findings from inline comments only (path + line/end_line + sanitized body); include sanitized review.body in PrReviewResult.body; remove actionable_count and passed computation. Rewrite format_review_summary to a passthrough format (review body section + numbered inline comment list with path:line prefix). Fix the main polling loop in poll_review_for_cycle to select the latest Codex bot review by submitted_at (collect all qualifying reviews in the loop iteration, then pick the maximum) rather than returning the first match, ensuring only the latest round is surfaced (AC-05, CN-02). (`64b1c921a9e3d52d11281bd6641279ad0a360c18`)
- [x] **T004**: Update pr_review_cycle in apps/cli-composition/src/pr.rs: the ReviewFound branch must emit passthrough output and always exit with code 0 (no pass/fail gate on review-found path); zero-findings path keeps PASS with exit code 0. Update the duplicated poll logic in apps/cli/src/commands/pr.rs test_helpers module (poll_review_for_cycle, parse_review references) to match the new PrReviewFinding/PrReviewResult shape, removing classify_severity and parse_body_findings call sites. Remove or update pr_tests.rs test assertions that reference removed fields (severity, actionable_count, passed). (`64b1c921a9e3d52d11281bd6641279ad0a360c18`)

### S3 — Test coverage for passthrough behavior

> T005 adds new tests that verify the behavioral requirements introduced by this track:
> COMMENTED review does not produce FAIL (AC-09); latest-round selection (AC-05); review.body in output (AC-03); inline comment path:line in output (AC-04); sanitize_text applied (AC-08); zero-findings PASS via reaction (AC-06) and comment (AC-07).
> These tests are written after T001-T004 so they can compile against the new shapes.

- [x] **T005**: Add new tests covering passthrough behavior: COMMENTED review yields ReviewFound not FAIL (AC-09); multiple Codex reviews — only latest submitted_at is surfaced (AC-05); ReviewFound output contains sanitized review.body (AC-03); ReviewFound output contains inline comment path:line and sanitized body (AC-04); sanitize_text applied to both body and inline comments (AC-08); zero-findings via +1 reaction after trigger yields PASS (AC-06); zero-findings via Didn't find any major issues comment after trigger yields PASS (AC-07). Tests live in the appropriate module (cli-composition poll tests or usecase pr_review tests). (`64b1c921a9e3d52d11281bd6641279ad0a360c18`)

### S4 — Command documentation update

> T006 aligns .claude/commands/track/pr-review.md with the passthrough model.
> Removes P0/P1 severity language and pass/fail-finding framing from Step 3 and Behavior sections.
> This is a doc-only task and can be done independently of S1-S3, but is placed last so the text reflects the finished implementation.

- [x] **T006**: Update .claude/commands/track/pr-review.md: remove P0/P1 severity framing and pass/fail-finding language from Step 3 and the Behavior section; rewrite to passthrough model where ReviewFound means comments are surfaced for agent judgment (not auto-graded), and only zero-findings detection produces a machine PASS. Align with AC-11. (`0beba4ffb59a62d2b3506150bf516277fc44b851`)
