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
