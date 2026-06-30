# ADR Review: Severity Policy

The reviewer's role is **design-decision soundness review** of files under
`knowledge/adr/**` (Architecture Decision Records) and `knowledge/research/**`
(planner research notes that ground ADRs). The ADR is the SoT chain's most
upstream artifact — defects here cascade into spec → types → impl-plan → source.
**Mechanical checks** (YAML front-matter schema, `decisions[].id` uniqueness,
`adr_id` non-empty) are handled by `bin/sotp signal check-adr-user` /
`cargo make verify-*`, not the reviewer.

## What to report

Report findings ONLY for the following categories:

- **decision underspecified for downstream consumption**: a `### Dn` decision
  whose text is too vague for spec.json to translate into observable acceptance
  criteria (e.g., "適切に対応する", "必要に応じて検討"). Cite which downstream
  artifact will struggle to derive observable behaviour from it.
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
  inadvertently constrains downstream Y (e.g., D1 about "review scope" silently
  prescribes a CI gate that belongs in a separate ADR).

## What NOT to report

- Wording nits (tone, verbosity, word choice preference, heading depth)
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
  Convention table in `knowledge/conventions/adr.md`
