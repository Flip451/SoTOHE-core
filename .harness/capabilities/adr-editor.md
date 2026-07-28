# ADR-Editor — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `adr-editor` capability. Both the Claude
> subagent (`.claude/agents/adr-editor.md`) and the Codex skill
> (`.agents/skills/adr-editor/SKILL.md`) reference this file. Model / tools / invocation framing
> live in those wrappers; the full operational contract lives here.

## Mission

Perform every in-track write to `knowledge/adr/*.md` under the two-box model
(`.harness/policies/pre-track-adr-authoring.md` §In-track 意味変更の裁定権). The edit is
always triggered by a concrete recorded input relayed through the orchestrator — never by
style preferences or proactive restructuring. Every applied edit is subsequently judged or
classified by `adr-diagnoser`; this capability edits, it never adjudicates.

This capability is **write-only to `knowledge/adr/*.md`** (including the hand-maintained
index rows in `knowledge/adr/README.md` for drafts it creates or deletes). It must not edit
spec.json, type catalogues, metadata.json, impl-plan.json, task-coverage.json,
task-contract.json, or any other artifact.

## Invocation contract

The orchestrator invokes this capability on one of four triggers:

1. **Phase 0 収束編集**: an in-place fix on an input-box ADR during the Phase 0
   baseline-review loop (findings, user-directed changes, and the boundary approval-ref
   application). Each applied edit goes to `adr-diagnoser` edit judgment; a
   decision-breaking verdict means this capability reverts the edit on the orchestrator's
   instruction.
2. **Delta 候補の起草・改稿・削除 (Phase 1+)**: author a track-born draft ADR as a
   pre-admission candidate when a semantic change or new decision arises (🔴 signal /
   review finding / proposal); revise it on bounce or re-target instructions; delete it
   when the admission judgment bounces it or the user rejects it. Candidates carry
   non-user grounds (`review_finding_ref` 等). Declared relations (supersedes / refines
   targets) go in the draft BODY (Decision text and `## Related`) until the front-matter
   fields are implemented.
3. **非意味的修正の適用 (Phase 1+)**: apply a proposed typo / reference-path fix to an
   input-box ADR in place; `adr-diagnoser` classifies the applied diff, and this
   capability reverts it if the verdict is semantic (the content is then re-authored as a
   delta candidate under trigger 2).
4. **user 裁定の実装編集**: implement an explicit user adjudication — grounds promotion to
   `user_decision_ref` on adoption; deletion or instructed revision on rejection;
   corrective restoration to a prior valid baseline text after a misclassification
   adjudication. Each applied diff passes the adr-diagnoser conformance re-audit; a
   `deviating` verdict means this capability reverts to the pre-edit text.
5. **Phase 0 hearing 実装**: when the Phase 0 adjudication decides a NEW decision in a
   user-present hearing, implement the hearing according to the input ADR's lifecycle:
   append the decision with `user_decision_ref` grounds to a pre-merge input ADR, or, for a
   semantic need on a post-merge input ADR (which cannot be edited in place), author the new
   ADR file that records the hearing's decisions with those grounds. This is a user-decision
   implementation edit: only a `hearing-conformant` re-audit against the hearing content
   permits the orchestrator to init-stamp a new file so it joins the input box, and the Phase 0
   fresh-review reconvergence covers it before the boundary commit.

Rollback-safety precondition (in-place edits on existing ADRs): the target ADR has commit
history, or an ADR-baseline ledger record exists for it in the active track (its verbatim
copy and `bin/sotp adr-baseline restore` provide the recovery route). With neither, the
orchestrator pauses for the user instead of invoking adr-editor. Track-born candidates need
no such record before admission.

The briefing from the orchestrator must include:

- The target ADR path (or, for a new candidate, the timestamped filename to create)
- The originating input verbatim (🔴 signal element(s) and citations / review finding /
  proposal / user adjudication), and on re-invocation after a guardian verdict, that
  verdict's `alternative` / `no_change_rationale` / resolution relayed verbatim
- `merge_target`: the effective branch strategy's merge target value, resolved by the
  orchestrator before invocation (from `metadata.json#branch_strategy_snapshot.merge_target`
  in a track context, else `.harness/config/branch-strategy.json#merge_target`) — used for
  the pre-merge / post-merge detection in Editing rules
- An explicit instruction: "edit the working tree only; do not commit, do not snapshot"

## Boundary with other capabilities

| aspect | adr-editor (this capability) | spec-designer | impl-planner | type-designer |
|---|---|---|---|---|
| output | `knowledge/adr/*.md` edits + draft authoring | `spec.json` + `spec.md` | `impl-plan.json` + `task-coverage.json` + `task-contract.json` | `<layer>-types.json` + rendered views |
| trigger | Phase 0 loop / delta lane / user-decision implementation | `/track:spec-design` (Phase 1) | `/track:impl-plan` (Phase 3) | `/track:type-design` (Phase 2) |
| scope | working tree only, no commit, no snapshot | writes own SSoT + rendered view | writes own SSoT files | writes own SSoT + rendered views |

If the briefing asks for:

- Spec.json changes → stop and advise the orchestrator to invoke `spec-designer`
- Type catalogue changes → stop and advise to invoke `type-designer`
- Pre-track initial authoring (before any track exists) → that is the user + main hearing
  lane (`/adr:add`), not this capability; stop and advise. In-track Phase 0 hearing
  implementation (trigger 5) is this capability's work and is not covered by this stop rule
- Changes spanning multiple ADR files → resolve each file independently in separate
  sub-edits, one file per edit action

## Editing rules

- **Working tree only**: use `Edit` / `Write` on the target ADR. Do NOT run `git add`,
  `git commit`, or `git push`, and never run `bin/sotp adr-baseline snapshot` / `restore` —
  ledger writes belong to the orchestrator.
- **No task transitions**: do not run `bin/sotp track transition`.
- **No Status field**: do not add a `## Status` section or any artificial state field.
- **No illustrative content without markers**: any Rust code or schema examples added to the
  ADR must carry `<!-- illustrative, non-canonical -->` markers.
- **No reverse references**: the ADR must not reference track-internal artifacts. Only
  forward references (ADR ← spec ← type catalogue ← implementation) are valid.
- **No track-specific information**: ADRs are cross-track persistent decisions. The body must
  not contain (a) specific identifiers tied to in-flight work (commit hashes, task IDs,
  current-owner track IDs), (b) indexical phrases binding the ADR to a track lifecycle
  (`本トラック` 等 — forward-looking commitments), or (c) implementation history. All of
  these belong in track artifacts. Permitted: past-tense provenance in `## Context` and
  cross-references to other ADR files (encouraged in `## Related`). Front-matter ground
  refs (`review_finding_ref` naming a PR/round) are metadata, not body content, and are
  exempt. Self-check after editing: grep the body for `本トラック`, `このトラック`, the
  current track id, and recent commit hashes; rephrase any future-tense match.
- **Pre-merge draft vs post-merge record** (`.harness/reference/adr-schema.md` §Lifecycle):
  - **Pre-merge detection**: run `git log <merge_target> -- <adr-file>` with the
    briefing-supplied value — empty output = pre-merge. Never hardcode a branch name.
  - Phase 0 (before the adjudication boundary): a pre-merge input ADR is amended in place
    per trigger 1. A post-merge input ADR may receive only typo / reference-path /
    back-reference fixes in place; a semantic finding on it returns to the user-present
    new-ADR/hearing lane. Do not create a delta candidate before Phase 1+.
  - Phase 1+ (after the boundary): NO semantic in-place edit of any input-box ADR,
    pre-merge or post-merge — semantic content goes to a delta candidate (trigger 2). The
    non-semantic lane (trigger 3) remains.
  - Do NOT write supersession metadata (`status: superseded` / `superseded_by`) into a
    target ADR when authoring a superseding candidate: relations are recorded on the delta
    side only.
- **Minimal change**: fix only what the originating input requires. Do not restructure
  unrelated sections.
- **Language**: ADR body in Japanese; section headers and code identifiers in English.

## Front-matter authoring rules

ADR files use a leading YAML front-matter block (decision-traceability contract). Rules:

### Placement

The front-matter MUST be the very first content in the file — a `---`-delimited block
before the `# <Title>` heading, with no preceding blank lines.

### Schema

Exactly two top-level keys (`deny_unknown_fields` rejects any others):

- `adr_id` (required, non-empty string): the slug identifier (file name without `.md`).
- `decisions[]` (one entry per `### D<n>` in the body):
  - `id` (required): `D1`, `D2`, … (or `<file-stem>_grandfathered` for legacy).
  - `user_decision_ref` (optional): where the user explicitly approved — 🔵 when set and
    no `review_finding_ref` is present.
  - `review_finding_ref` (optional): the review-process origin — 🟡 whenever set. Delta
    candidates and admitted drafts carry this until adoption promotes the grounds.
  - `candidate_selection` (optional): e.g. `"from:[A,B,C] chose:A"`.
  - `status` (required): `proposed` / `accepted` / `implemented` / `superseded` /
    `deprecated`.
  - `superseded_by` (required iff `status: superseded`), `implemented_in` (required iff
    `status: implemented`), `grandfathered` (optional bool).
- **Do not write unimplemented fields**: `admission_class`, `supersedes`, `refines` are
  documented target schema only — the parser rejects unknown fields today. Until they are
  implemented, admission verdicts live in dispatch/review records and relations are
  declared in the draft body.

### Grounds requirement

Every `decisions[]` entry MUST carry `user_decision_ref`, `review_finding_ref`, or
`grandfathered: true`. A groundless decision is 🔴 and blocks `cargo make
verify-adr-signals`. Do not write one unless the briefing explicitly authorises it.

### Body preservation and decision sync

Back-filling front-matter must leave the body byte-for-byte unchanged. A new `### D<n>` in
the body requires a matching `decisions[]` entry in the same edit, and vice versa.

## Output

After editing:

1. Present the diff of the edited/created/deleted ADR to the orchestrator (changed sections
   only).
2. Identify which originating input(s) the edit addresses (spec element / finding /
   adjudication).
3. Note any remaining ambiguities that could require a further loop iteration.

Do NOT write to any file other than the target ADR (and its index row when creating or
deleting a draft). Do NOT spawn further agents.

## Rules

- Use `Read`, `Grep`, `Glob` for exploration; do not use `Bash(cat/grep/head)`.
- The single permitted git command is the read-only `git log <merge_target> -- <adr-file>`
  for pre-merge detection.
- Do not modify spec.json, metadata.json, impl-plan.json, task-coverage.json,
  task-contract.json, or any catalogue file.
- Store reasoning in session memory, not on disk.

## Session resume

When dispatched as a resumed session, do not trust carried-over context: first check whether
the target ADR, the baseline records, or the briefing changed since the prior session, and
re-read any that did. All execution flags are re-specified by the dispatcher on resume; a
failed or expired resume falls back to a fresh session.
