# Track Observations

## 2026-08-03 — infrastructure syntax retreat

- The infrastructure review loop spent 87 rounds on hand-written keyword restoration and
  Rust-syntax classification without converging.
- Per the revised handoff and the primary ADR, chain ③ is lexical. The restoration and
  classifier machinery was removed; expression parsing now delegates directly to `syn`.
- Generic declaration names are deliberately limited to plain, non-keyword Rust identifiers.
  Raw identifiers and keyword/weak-keyword spellings are rejected fail-closed, converting the
  accumulated syntax edge cases into rejection regression tests rather than a new parser.
- This keeps the implementation on the lexical-comparison side of the ADR boundary. If a
  future acceptance criterion requires grammar interpretation, it must be decided through a
  delta ADR rather than added as an implementation-side parser.

## 2026-08-05 — PR review spiral closed by a two-rule acceptance grammar

- PR #234 accumulated 33 review rounds / 47 inline findings (8/2–8/5) at a flat 1–2
  findings per round: the same non-convergence pathology as the local 87-round loop,
  recurring on the PR side. 74% of the findings hit the type_ref parser / comparison
  complex.
- The fix shape until then was a growing denylist: one reject function per reviewer
  finding (`bound_spelling.rs` reached 567 lines). A denylist over the open set of
  `syn`-parseable Rust is inherently fail-open — the reviewer could always produce a
  new counterexample.
- Per the fleet prescription, the denylist was replaced by a closed acceptance grammar
  (`type_ref_parser/closed_grammar.rs` + `canonical_render.rs`):
  - Rule A (canonical round-trip): a catalogue spelling is accepted only when it
    token-equals the canonical rendering of its own converted representation. Every
    normalized-away spelling variant dies with zero per-variant code.
  - Rule B (syntax allowlist): accepted AST classes are defined positively from what
    rustdoc can emit for a generic-alias declaration; everything else rejects by
    default, so novel constructs fail closed without new code.
- All 47 PR findings are fixed as a regression corpus
  (`test_pr234_findings_regression_corpus_*`); each grammar-relevant input is pinned
  to reject (A or B) or to accept-and-compare. Two spellings previously accepted by
  normalization (`Fn() -> ()`, `Fn(&'_ str)`) became rejections for consistency: their
  normalized forms are indistinguishable from the canonical spellings, which is
  exactly the reviewer's own turbofish argument.
- Shared rule with the 8/3 retreat: do not fight an open set — close the accepted set
  and reject the outside by default.
