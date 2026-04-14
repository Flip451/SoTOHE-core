# Source Attribution Convention

## Purpose

Every requirement, constraint, and acceptance criterion in `spec.md` must carry a
`[source: ...]` tag to make the provenance of each item traceable. This enables:

- Distinguishing verified facts from inferences
- Auditing why a requirement exists
- Phase 2 signal evaluation (TSUMIKI-01) to assess confidence per item

## Source Tag Types

| Tag | Meaning | Signal | Example |
|-----|---------|--------|---------|
| `[source: <document> §<section>]` | Explicit reference to a document, section, or external standard | Blue | `[source: PRD §3.2]`, `[source: track/tech-stack.md]` |
| `[source: convention — <file>]` | Established project convention with specific file reference | Blue | `[source: convention — knowledge/conventions/security.md]` |
| `[source: feedback — <context>]` | User feedback or correction (undocumented, not persisted) | Yellow | `[source: feedback — Rust-first policy]` |
| `[source: inference — <reason>]` | Inferred from context, conventions, or common practice; not explicitly stated | Yellow | `[source: inference — security best practice]` |
| `[source: discussion]` | Agreed upon in team discussion or user conversation | Yellow | `[source: discussion]` |

**Blue sources** reference persistent, version-controlled files (ADR, convention document, PRD, tech-stack, etc).
**Yellow sources** lack persistent documentation; they capture intent without an artifact.

### Strict gate semantics

The merge gate (invoked via `sotp pr wait-and-merge`) blocks merge when any
requirement still has a Yellow source. CI runs in interim mode and surfaces
Yellow as a `VerifyFinding::warning` (visible in `cargo make ci` output) without
blocking development iteration — see ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D8.

### Upgrading Yellow to Blue

To unblock merge, promote each Yellow requirement to Blue:

1. **Create persistent documentation**: Write an ADR (`knowledge/adr/<date>-<hhmm>-<slug>.md`) or convention (`knowledge/conventions/<topic>.md`) that records the decision.
2. **Reference the new document**: Update the spec requirement's `sources` array to point at the new ADR/convention via a `document` or `convention` source.
3. **Re-run signal evaluation**: `cargo make track-signals <track-id>` recomputes signals; previously-Yellow items should now be Blue.
4. **Re-approve the spec**: `cargo make spec-approve track/items/<id>` updates the `content_hash`.

This workflow is the structural incentive created by the strict signal gate:
design decisions accumulate as persistent artifacts rather than undocumented
feedback or inference.

## Placement

Tags appear inline at the end of the requirement statement:

```markdown
## Constraints

- New logic must be implemented in Rust, not Python [source: feedback — Rust-first policy]
- TDD workflow is mandatory [source: convention — .claude/rules/05-testing.md]
- Input validation uses domain types [source: track/tech-stack.md §Architecture]
```

For acceptance criteria:

```markdown
## Acceptance Criteria

- [ ] `sotp verify spec-frontmatter` passes for all spec.md files [source: inference — CI gate requirement]
```

## Rules

1. Every item in `Scope`, `Constraints`, and `Acceptance Criteria` sections should have a source tag.
2. `Goal` section does not require source tags (it is the feature's own definition).
3. When multiple sources apply, list them comma-separated: `[source: PRD §3.2, discussion]`.
4. When the source is unknown, use `[source: inference — reason]` with an honest reason.
   Do not fabricate document references.
5. Source tags are informational metadata used by signal evaluation (TSUMIKI-01) to assess confidence.
6. In `spec.json` (SSoT), sources are a JSON array: `"sources": ["PRD §3.2", "discussion"]`.
   In rendered `spec.md`, multi-source items display as `[source: PRD §3.2, discussion]`.
   Multi-source signal policy: the item's signal is the **highest confidence** among its sources.
