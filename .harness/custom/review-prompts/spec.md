# Spec Review: Severity Policy

The reviewer's role is **behavioural-contract soundness review** of
`track/items/<track-id>/spec.json` (the Phase 1 SSoT) and its rendered view
`spec.md`. The spec translates ADR decisions into observable goal / scope /
constraints / acceptance_criteria; defects here mislead Phase 2 (type design)
and Phase 3 (impl-plan). **Mechanical checks** (schema_version, `adr_refs[]`
anchor validity, signal computation) are handled by `bin/sotp signal calc-spec-adr`
/ `ref-verify` / `cargo make verify-*`, not the reviewer.

The spec is a *refinement* of the ADR — more detailed and operational by design.
Refinement is not a defect.

## What to report

Report findings ONLY for the following categories:

- **acceptance criterion non-observable**: an `AC-NN` element whose text
  describes a state that cannot be checked by a deterministic procedure
  (CI command, grep, file-existence query, manual machine read). Example:
  "the design feels consistent" is non-observable; "`review-scope.json`
  declares a group named `adr`" is observable.
- **goal not anchored to a decision**: a `GO-NN` element whose intent has no
  corresponding `### Dn` in any cited ADR. Goal text expanding scope beyond the
  ADR's decision space is an unauthorized expansion (ref-verify Chain1 catches
  exact-pair semantic conflict; the reviewer flags the broader pattern).
- **scope / acceptance_criteria contradiction**: an `IN-NN` in_scope item that
  conflicts with an `OS-NN` out_of_scope item, or with a `CN-NN` constraint;
  an `AC-NN` that requires behaviour outside the declared in_scope set.
- **constraint vague or untestable**: a `CN-NN` element worded so loosely
  (e.g., "適切な精度", "necessary safeguards") that downstream type / impl-plan
  cannot derive a concrete contract from it. Distinguish from intentional
  qualitative constraints — flag only when downstream phases would be forced
  to invent the meaning.
- **missing AC coverage for a stated goal**: a `GO-NN` goal whose intent has
  no `AC-NN` that would deterministically prove it on completion. Stated as a
  coverage gap, not as a prescriptive "add AC-N here" suggestion.
- **out_of_scope leakage**: an `OS-NN` item whose exclusion is contradicted
  later by a hidden requirement in `AC-NN` or `CN-NN` (the spec promises not
  to do X but quietly requires X via an acceptance criterion).
- **broken cross-reference at narrative level**: a `related_conventions[]` or
  in-prose convention citation that is self-evidently off-topic. Do NOT
  verify path existence — that is CI's job.

## What NOT to report

- Wording nits (tone, verbosity, JP/EN mixing unless a style rule violated)
- ADR anchor validity / front-matter schema (ref-verify Chain1 and signal
  evaluators handle this deterministically)
- File-existence checks for cited paths (CI / `verify-doc-links`)
- Suggested additions to scope.in_scope or new goals — the planning gate
  has closed; expansion proposals belong in a follow-up ADR
- Backward-looking metrics (how many elements were rewritten, round counts)
- Type-level / structural prescriptions — those are Phase 2's domain and the
  reviewer of `types` scope owns them
- Implementation-task ordering / dependency hints — those are Phase 3 / the
  `impl-plan` scope reviewer's domain
