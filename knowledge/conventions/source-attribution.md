# Source Attribution Convention

## Purpose

Every requirement, constraint, and acceptance criterion in `spec.md` must carry a
`[source: ...]` tag to make the provenance of each item traceable. This enables:

- Distinguishing verified facts from inferences
- Auditing why a requirement exists
- Phase 2 signal evaluation (TSUMIKI-01) to assess confidence per item

> **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-attribution

## Source Tag Types

| Tag | Meaning | Signal | Example |
|-----|---------|--------|---------|
| `[source: <document> §<section>]` | Explicit reference to a document, section, or external standard | Blue | `[source: PRD §3.2]`, `[source: knowledge/adr/<adr-id>.md#<decision-id>]` |
| `[source: convention — <file>]` | Established project convention with specific file reference | Blue | `[source: convention — knowledge/conventions/security.md]` |
| `[source: feedback — <context>]` | User feedback or correction (undocumented, not persisted) | Yellow | `[source: feedback — Rust-first policy]` |
| `[source: inference — <reason>]` | Inferred from context, conventions, or common practice; not explicitly stated | Yellow | `[source: inference — security best practice]` |
| `[source: discussion]` | Agreed upon in team discussion or user conversation | Yellow | `[source: discussion]` |

> **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-attribution / bin/sotp verify spec-signals

**Blue sources** reference persistent, version-controlled files (ADR, convention document, PRD, etc).
**Yellow sources** lack persistent documentation; they capture intent without an artifact.

> **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-attribution / bin/sotp verify spec-signals

### Strict gate semantics

The merge gate (invoked via `sotp pr wait-and-merge`) blocks merge when any
requirement still has a Yellow source. CI runs in interim mode and surfaces
Yellow as a `VerifyFinding::warning` (visible in `cargo make ci` output) without
blocking development iteration. This keeps development feedback visible while reserving the
strict Blue-only requirement for the merge gate.

> **強制先**: 宣言突合 (catalogue + verify) — bin/sotp pr wait-and-merge / cargo make ci

### Upgrading Yellow to Blue

To unblock merge, promote each Yellow requirement to Blue:

> **強制先**: 宣言突合 (catalogue + verify) — bin/sotp pr wait-and-merge

1. **Create persistent documentation**: Write an ADR (`knowledge/adr/<date>-<hhmm>-<slug>.md`) or convention (`knowledge/conventions/<topic>.md`) that records the decision.
   > **強制先**: review 観点 — adr / harness-policy scope
2. **Reference the new document**: Update the spec requirement's `sources` array to point at the new ADR/convention via a `document` or `convention` source.
   > **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-attribution
3. **Re-run signal evaluation**: run the signal command required by the active track workflow; previously-Yellow items should then become Blue.
   > **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-signals

This workflow is the structural incentive created by the strict signal gate:
design decisions accumulate as persistent artifacts rather than undocumented
feedback or inference.

## Placement

Tags appear inline at the end of the requirement statement:

> **強制先**: review 観点 — spec scope

```markdown
## Constraints

- New logic must be implemented in Rust, not Python [source: feedback — Rust-first policy]
- Fake-first test doubles; mock only when the interaction is the specification [source: convention — knowledge/conventions/testing.md]
- Input validation uses domain types [source: knowledge/conventions/prefer-type-safe-abstractions.md]
```

For acceptance criteria:

```markdown
## Acceptance Criteria

- [ ] `sotp verify spec-frontmatter` passes for all spec.md files [source: inference — CI gate requirement]
```

## Rules

1. Every item in `Scope`, `Constraints`, and `Acceptance Criteria` sections should have a source tag.
   > **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-attribution
2. `Goal` section does not require source tags (it is the feature's own definition).
   > **強制先**: review 観点 — spec scope
3. When multiple sources apply, list them comma-separated: `[source: PRD §3.2, discussion]`.
   > **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-attribution
4. When the source is unknown, use `[source: inference — reason]` with an honest reason.
   Do not fabricate document references.
   > **強制先**: review 観点 — spec scope
5. Source tags are informational metadata used by signal evaluation (TSUMIKI-01) to assess confidence.
   > **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-signals
6. In `spec.json` (SSoT), sources are a JSON array: `"sources": ["PRD §3.2", "discussion"]`.
   In rendered `spec.md`, multi-source items display as `[source: PRD §3.2, discussion]`.
   Multi-source signal policy: the item's signal is the **highest confidence** among its sources.
   > **強制先**: 宣言突合 (catalogue + verify) — bin/sotp verify spec-frontmatter / bin/sotp verify spec-attribution / bin/sotp verify spec-signals
