# Verifier input diagnosis

Read-only researcher dispatch on 2026-09-11. No live LLM evaluation was run. This is implementation evidence, not architectural authority.

## Confirmed section loss

- `libs/usecase/src/test_obligation/evaluate/edges.rs:20`: `resolve_anchor_text` searches five sections but returns only requirement text.
- `libs/infrastructure/src/test_obligation/fulfillment_verifier.rs:106` and `waiver_verifier.rs:92`: both render the text under `Anchor promise`.
- `libs/usecase/src/test_obligation/check_support.rs:81,275`: existing `SpecElement` carries id, section and text; `anchor_texts` at line 247 drops section. Evaluate, check and results hash raw text and can reuse verdicts after same-text section moves.
- Both verifier fingerprint functions hash their preambles only. Rendering semantics also need a version/fingerprint invalidation mechanism.

## Responsibility-overreach evidence and limits

The cited cases were examined in the sibling consumer checkout through declarations, binding identities, test source and cached rejection text. No captured historical rendered prompt was found/observed; the input was reconstructed from current rendering code and those artifacts. Consequently model noncompliance is the supported explanation for those reconstructed inputs, not a claim that the historical request bytes were verified.

- The fulfillment preamble already restricts judgment to the target-owned portion and forbids demands belonging to another entry.
- Consumer `libs/usecase/src/port.rs:126` declares `SessionIdExtractor::extract` with a single `CapturedOutput` and a rule.
- Consumer `libs/infrastructure/src/extract_json.rs:20` parses selected output; it does not select streams. Bound tests cover rules naming either stream using a single capture.
- Consumer `libs/usecase/src/run.rs:818,851` binds interactor tests for selecting stderr over distinguishable stdout and not falling back when selected stderr is invalid.
- Consumer `libs/infrastructure/src/config_toml.rs:667` tests omitted stream defaulting to stdout.
- The failures demanding simultaneous distinct streams, stream selection, defaulting or fallback from the extractor conflict with that division of responsibility. They do not justify widening the consumer port or indiscriminately mixing other owners' tests.

## Regression directions

1. Same id/text moved between in_scope and out_of_scope changes both verifier inputs and anchor freshness in evaluate/check/results, for old Pass and Fail.
2. Both prompts preserve section and identity; nominal exclusions are not affirmative implementation requirements, and non-guarantees are not fabricated prohibitions.
3. Single-input parsing and caller-owned selection are distinguished in bounded calibration cases, including a genuinely wrong selection/fallback caller.
4. Target-owned contradiction, substitution and central-unverified remain failures; insufficient material remains pending.
5. Old verifier fingerprints are not reused after semantics updates; no cache purge.

## Source authority

The local test-obligation ADR's D3, D4, D6 and D16 already establish section priors, edge-locality and instruction-fingerprint invalidation. The new ADR refines those mechanisms. Consumer examples and temporary handoff files are diagnostic input only, not references for the new ADR body.
