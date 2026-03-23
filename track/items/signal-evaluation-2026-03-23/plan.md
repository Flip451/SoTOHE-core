<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Spec Signal Evaluation — Stage 1 (TSUMIKI-01)

Stage 1 of two-stage signal evaluation: spec requirement provenance signals (TSUMIKI-01).
Evaluates [source: ...] tags in spec.md Scope/Constraints/Acceptance Criteria sections.
Stores aggregate SignalCounts in spec.md frontmatter signals field.
Stage 2 (Domain States signals + metadata.json domain_state_signals) is a separate follow-up track.

## Domain Layer — Shared Primitives

ConfidenceSignal enum, SignalBasis enum, source tag to signal mapping rules. These types are shared with future Stage 2.

- [x] Domain: ConfidenceSignal enum + SignalBasis enum + source tag mapping rules (pure functions, shared primitives for Stage 1 and future Stage 2)

## Infrastructure Layer — Source Tag Parser + Domain States Presence

spec.md source tag parser, signal evaluation engine, Domain States section presence check

- [x] Infrastructure: spec.md source tag parser + signal evaluation engine (Scope/Constraints/Acceptance Criteria sections only)
- [x] Infrastructure: sotp verify spec-states minimal — validate that spec.md contains a ## Domain States section with at least one data row in its table (signal tag parsing deferred to Track B)

## CLI Layer — Commands + Gates

sotp track signals command (spec.md frontmatter update), sotp verify spec-signals gate

- [x] CLI: sotp track signals command (evaluate spec.md source tags, update spec.md frontmatter signals field, display summary)
- [x] CLI + Infrastructure: sotp verify spec-signals gate (spec.md frontmatter signals vs actual source tag evaluation consistency, red == 0 policy)
