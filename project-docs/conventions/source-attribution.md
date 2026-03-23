# Source Attribution Convention

## Purpose

Every requirement, constraint, and acceptance criterion in `spec.md` must carry a
`[source: ...]` tag to make the provenance of each item traceable. This enables:

- Distinguishing verified facts from inferences
- Auditing why a requirement exists
- Phase 2 signal evaluation (TSUMIKI-01) to assess confidence per item

## Source Tag Types

| Tag | Meaning | Example |
|-----|---------|---------|
| `[source: <document> §<section>]` | Explicit reference to a document, section, or external standard | `[source: PRD §3.2]`, `[source: track/tech-stack.md]` |
| `[source: feedback — <context>]` | User feedback or correction recorded in memory/history | `[source: feedback — Rust-first policy]` |
| `[source: convention — <file>]` | Established project convention with specific file reference | `[source: convention — project-docs/conventions/security.md]` |
| `[source: inference — <reason>]` | Inferred from context, conventions, or common practice; not explicitly stated | `[source: inference — security best practice]` |
| `[source: discussion]` | Agreed upon in team discussion or user conversation | `[source: discussion]` |

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
