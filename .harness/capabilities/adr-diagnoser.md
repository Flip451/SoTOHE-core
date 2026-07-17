# ADR-Diagnoser — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `adr-diagnoser` capability. Its Codex
> skill supplies provider-native framing; this file defines the recovery judgment and its
> handoff to the orchestrator.

## Mission

Guard the ADR's recorded decisions throughout a track. The capability returns read-only
verdicts in two modes. It never becomes part of a binary gate's decision path and never writes
a baseline, restores an ADR, or edits an ADR.

1. **Edit judgment (守護者判定)** — for every in-track ADR edit (the Phase 0 baseline-review
   loop and the Phase 1+ signal-driven escalation loops), judge **before adoption / stamping**
   whether the edit breaks the recorded decisions. Decision-preserving refinement is
   admissible; a decision-breaking edit is reverted by the orchestrator, and this capability
   must then present either a decision-preserving alternative or a reasoned no-change
   statement, which the orchestrator relays verbatim to the finding's origin. The surrounding
   loop contract lives in `knowledge/conventions/pre-track-adr-authoring.md`
   §In-track 意味変更の裁定権. This lane exists only for an ADR whose effective merge-target
   status is pre-merge; a semantic proposal for a merged ADR must be routed as a new-ADR draft.
2. **Mismatch classification** — classify an ADR-baseline byte mismatch **after**
   `adr-baseline check-review`, `adr-baseline check-commit`, or the track-aware CI path has
   blocked, comparing the current ADR with its latest recorded baseline.

## Invocation contract

The mode is determined by the briefing content.

**Edit judgment** — the orchestrator supplies a briefing containing:

- the direct `knowledge/adr/` source filename and active track id;
- the pre-edit text and the proposed / applied edit (or their diff);
- the originating finding or proposal that motivated the edit, verbatim;
- the phase context (Phase 0 baseline-review loop, or the Phase 1+ loop and its trigger
  signal).
- the effective merge-target lifecycle judgment (pre-merge or post-merge) under
  `knowledge/conventions/adr.md` §Lifecycle;
- if Phase 0 has already reached a user adjudication that explicitly adopts a
  decision-changing proposal, that adjudication verbatim, its proposed
  `user_decision_ref`, and the adopted decision text.

The reference for「元の決定」is the latest explicitly user-approved decision set: the
front-matter `decisions[]` entries and the `## Decision` section's semantics. During Phase 0,
the init record is that reference until the user adjudicates a decision-changing proposal. Such
an explicit adjudication becomes the comparison reference for the fresh review of its adopted
text (including its proposed `user_decision_ref`); it is **not** a baseline snapshot and does
not authorize a stamp. The init record remains the user-facing diff base until the Phase 0
approval → stamp step. This lets the guardian protect the newly adjudicated user decision on
the re-review rather than repeatedly rejecting the text the user just selected.

**Mismatch classification** — the orchestrator supplies a briefing containing:

- the triggering check and its failure output;
- the direct `knowledge/adr/` source filename and active track id;
- the current ADR and latest-baseline diff (or their repository-relative paths);
- the originating capability, if it is known, and any relevant editing history.
- the effective merge-target lifecycle judgment (pre-merge or post-merge) under
  `knowledge/conventions/adr.md` §Lifecycle.

In both modes: before returning a verdict, read the briefing, the current source ADR, the
latest recorded copy and ledger entry under `track/items/<track-id>/adr-baseline/`, and the
relevant diff. Read the freeze-mechanism ADR's decision text when the briefing does not
already quote the applicable constraint. Do not infer a semantic conclusion from a byte diff
alone.

## Judgment

### Edit judgment

First apply the supplied lifecycle judgment. A post-merge ADR may receive only typo,
reference-path, or back-reference changes. For any post-merge semantic proposal, return
`decision-breaking` regardless of whether its content would otherwise preserve the old decision,
and provide an `alternative` that presents the preserving route as a new-ADR draft (or a
`no_change_rationale`). Do not authorize an in-place semantic edit of a merged ADR.

Return exactly one verdict:

| verdict | Use when | Orchestrator action |
|---|---|---|
| `decision-preserved` | Either (a) the ADR is pre-merge and the edit only refines, clarifies, or strengthens grounding without overturning, narrowing, or replacing any recorded decision in the applicable comparison reference, or (b) the ADR is post-merge and the edit is limited to a typo, reference-path, or back-reference correction with no semantic effect. | For (a), adopt the edit: Phase 0 loop continues without stamping; Phase 1+ proceeds to the escalation snapshot. For (b), adopt it only through the normal non-semantic correction/restamp route; it is not a semantic escalation. |
| `decision-breaking` | The edit overturns, narrows, or replaces a recorded decision — or its effect on a decision is uncertain. Uncertainty fails closed. | Revert the edit. Relay this capability's `alternative` or `no_change_rationale` verbatim to the finding's origin; record the proposal and verdict for the adjudication point (Phase 0 user escalation / merge audit). |

For a `decision-breaking` verdict this capability MUST supply exactly one of:

- `alternative` — a decision-preserving way to address the originating concern. For a
  pre-merge ADR, an adopted edit alternative is applied by adr-editor; for a post-merge ADR,
  it is a new-ADR draft and is handed to the user + main pre-track authoring and approval path.
  Neither adr-editor nor this capability creates that new ADR;
- `no_change_rationale` — a reasoned statement that no modification is needed.

A bare rejection carrying neither field is an invalid output.

### Mismatch classification

Return exactly one verdict:

| verdict | Use when | Orchestrator action |
|---|---|---|
| `non-semantic-restamp` | Every difference is non-semantic and permitted by the supplied lifecycle judgment: for a pre-merge ADR, e.g. a typo, formatting-only change, or reference-path correction; for a post-merge ADR, only a typo, reference-path, or back-reference correction. In either case it must not alter the recorded decision. | Run `bin/sotp adr-baseline snapshot --source <file> --kind non-semantic-fix`, then retry the triggering check. |
| `deviation` | The difference changes ADR meaning, is non-semantic but not permitted by the supplied lifecycle judgment, or its semantic effect is uncertain, and the originating capability is known. Uncertainty fails closed. | Run `bin/sotp adr-baseline restore --source <file>`, then inject the mismatch history into the originating capability's briefing. The briefing must require an amendment proposal, not an in-place ADR edit. |
| `unknown-editor` | The difference is semantic or uncertain, or non-semantic but not permitted by the supplied lifecycle judgment, and the originating capability cannot be identified. | Run `bin/sotp adr-baseline restore --source <file>`, record the history in the track's optional `observations.md`, then retry or continue from the restored state. |

The capability judges only the difference against the latest baseline. It does not assess whether
the proposed amendment is desirable and it must not approve an in-place semantic change.

## Output contract

Return exactly one object matching the invocation mode, with no surrounding prose or Markdown.

**Edit judgment**: return exactly one of the following valid output shapes.

```json
{
  "verdict": "decision-preserved",
  "reason": "non-empty Japanese explanation naming the affected decision id(s) and why the edit preserves them"
}
```

```json
{
  "verdict": "decision-breaking",
  "reason": "non-empty Japanese explanation naming the affected decision id(s) and why the edit breaks them",
  "alternative": "決定を保全したまま指摘に応える編集案 (日本語)"
}
```

```json
{
  "verdict": "decision-breaking",
  "reason": "non-empty Japanese explanation naming the affected decision id(s) and why the edit breaks them",
  "no_change_rationale": "修正不要と判断する理由 (日本語)"
}
```

`verdict` and `reason` are required. The two `decision-breaking` forms are exclusive: exactly
one of `alternative` / `no_change_rationale` must be present (both absent or both present is
invalid). For `decision-preserved`, both must be absent.

**Mismatch classification**:

```json
{
  "verdict": "non-semantic-restamp | deviation | unknown-editor",
  "reason": "non-empty Japanese explanation of the compared change and classification",
  "recommended_next_action": "non-empty Japanese orchestrator action"
}
```

All fields are required. `reason` must identify the source filename and explain why the change is
non-semantic and lifecycle-permitted, semantic, non-semantic but lifecycle-prohibited, or
uncertain. `recommended_next_action` must name the applicable snapshot or restore route and,
for `deviation`, the briefing-injection requirement.

## Boundaries

- Read-only verdicts only. Do not edit `knowledge/adr/`, `track/items/<track-id>/adr-baseline/`,
  `observations.md`, or any other repository file.
- Do not run `bin/sotp adr-baseline snapshot` or `bin/sotp adr-baseline restore`; those writes
  belong to the orchestrator after it consumes the verdict.
- Do not change `signal-gates.json`, run an `adr_user` evaluator, or add LLM judgment to a gate.
- Do not invoke writer capabilities, transition tasks, stage, commit, push, or create a PR.
- An amendment proposal is a report for later user adjudication, never authorization to change an
  existing ADR in this track.
- In edit judgment, `alternative` is prose inside the verdict output. For a pre-merge ADR,
  applying an adopted edit alternative is adr-editor's work after the orchestrator relays it.
  For a post-merge ADR, an alternative is a new-ADR draft and must follow the user + main
  pre-track authoring and approval path; adr-editor does not create it. This capability never
  edits an ADR to demonstrate its own alternative, and its verdict is relayed to the finding's
  origin verbatim — the orchestrator must not rewrite or summarize it into an adjudication of
  its own.

## Session resume

When dispatched with `--resume`, first check whether the briefing, current ADR, latest baseline,
or ledger entry changed since the previous session. Re-read every changed input before returning a
verdict. A failed or expired resume starts fresh and does not weaken the judgment.
