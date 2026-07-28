# ADR Review: Severity Policy

The reviewer's role is **design-decision soundness review** of files under
`knowledge/adr/**` (Architecture Decision Records) and `knowledge/research/**`
(planner research notes that ground ADRs). The ADR is the SoT chain's most
upstream artifact — defects here cascade into spec → types → impl-plan → source.

**Principle: the review evaluates whether the ADR records its decisions
concisely and without contradiction（簡潔に矛盾なく決定を記録しているか）.
ADR review does NOT question specification completeness — spec.json, the next
SoT-chain layer, complements completeness by translating decisions into
observable acceptance criteria, and conventions / capability contracts /
workflow SSoTs carry the operational detail.** Findings that demand operational
or specification detail inside an ADR are out of scope.

**Mechanical checks** (YAML front-matter schema, `decisions[].id` uniqueness,
`adr_id` non-empty) are handled by `bin/sotp signal check-adr-user` /
`cargo make verify-*`, not the reviewer.

## What to report

Report findings ONLY for the following categories:

- **decision not actually recorded**: a `### Dn` decision whose intent is so
  vague that no decision is recorded (e.g., "適切に対応する", "必要に応じて検討"
  as the entire substance). This is a failure to decide, not a completeness
  gap — do NOT stretch this category into demanding operational procedures,
  verdict/schema formats, gate firing conditions, recovery flows, or edge-case
  enumeration; per the Principle above, completeness belongs to spec.json and
  the downstream documents.
- **decision obscured by excess detail**: operational procedures, schemas, gate
  wiring, or exhaustive enumerations embedded in the ADR body that belong to
  `knowledge/conventions/`, capability contracts, or workflow SSoTs and bury
  the decision. The test is removability: report it only when deleting the
  passage would leave every recorded decision unchanged. Report as a
  conciseness violation; the fix is deletion, or delegation to a document that
  already exists — never further elaboration inside the ADR, and never
  delegation to a document that has yet to be written, which deletes the
  decision instead of relocating it. Three things are outside this category:
  - The canonical status of content marked `<!-- illustrative, non-canonical -->`.
    Marking code and schema examples that way is required of ADR authors, so a
    marked example must not be reported as an unauthorized commitment. The
    marker settles that one question and no other: it does not license
    unbounded length. An example long enough to bury the decision it
    illustrates remains reportable under this category, on the same
    removability test.
  - A concrete identifier — a command, flag, path, or outcome name — that is
    itself the subject of the decision. When an ADR decides what a surface is,
    naming that surface is the decision, not detail wrapped around it.
  - A prohibition or a fail-closed condition that the ADR itself decides.
    Delegating one of those to a downstream document turns it back into an
    open implementation choice, so it cannot be relocated without being
    overturned. This does not extend to a prohibition the ADR merely restates
    from a workflow or capability SSoT that already records it: deleting a
    restatement leaves every recorded decision unchanged, so it stays
    reportable on the ordinary removability test.
- **inconsistent decisions within the same ADR**: two `### Dn` items inside one
  ADR that contradict each other, or a Decision section that contradicts the
  ADR's own Context / Rejected Alternatives narrative.
- **rejected alternative re-emerges in decisions**: a design path explicitly
  rejected in `## Rejected Alternatives` reappears as an implicit assumption
  in `## Decision` without acknowledging the prior rejection.
- **Reassess When trigger missing or vacuous**: an ADR with no
  `## Reassess When` section, or one whose triggers are tautologies ("when the
  decision no longer applies") that provide no operational signal for revisiting.
- **broken narrative reference**: a `## Related` link or in-prose ADR citation
  that is self-evidently wrong (cites an ADR whose title is unrelated to the
  context), or references a convention path that is clearly off-topic. Do NOT
  flag whether the file physically exists — that is `verify-doc-links` / CI.
- **research grounding mismatch**: a `knowledge/research/**` note cited as
  grounding for a decision but whose content contradicts or fails to support
  the decision being made.
- **scope leakage into ADR body**: the ADR claims to decide X but the body
  inadvertently constrains downstream Y (e.g., a decision about "review scope" silently
  prescribes a CI gate that belongs in a separate ADR).

## What NOT to report

- Specification-completeness demands against a decision record: operational
  procedure detail, machine-readable schema/verdict formats, gate wiring,
  recovery-flow enumeration, or restating consumer-derivable rules inside the
  ADR — completeness is complemented by spec.json and the downstream documents,
  never by inflating the ADR
- Wording nits (tone, word choice preference, heading depth). Style-level
  phrasing is out of scope; structural bloat is NOT a nit — it is the
  "decision obscured by excess detail" category above
- English/Japanese mixed writing (unless an explicit style rule is violated)
- Existence checks for file paths or ADR slugs (CI / `verify-doc-links`)
- Alternative design suggestions — the decision has been made; relitigating
  it during review is out of scope (the proper venue is a new ADR that
  supersedes or refines)
- Front-matter field nits when the schema validator already passes
  (`adr_id` formatting, status spelling, etc.)
- Backward-looking observations (how many rounds it took, history of edits)
- Convention overlap suggestions ("this should be a convention not an ADR")
  unless the artifact unambiguously fits the convention column of the ADR vs
  Convention table in `.harness/reference/adr-schema.md`
