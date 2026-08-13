# Type-Design Workflow SSoT

> Provider-agnostic workflow SSoT for the `type-design` track workflow. Both the Claude adapter
> (`.claude/commands/track/type-design.md`) and the Codex skill adapter
> (`.agents/skills/track-type-design/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Author per-layer type catalogues for the current track via the `type-designer` capability
(Phase 2 TDDD workflow). The workflow is single-shot: invoke the capability once, receive its
per-layer signal-evaluation result, and return. Back-and-forth escalation (🔴 → spec-design
re-invocation or adr-editor) is owned by the caller (`plan` workflow). The `type-designer`
capability owns catalogue, baseline, and rendered-view writes plus signal computation
internally; Step 5's enrollment-artifact writes belong to the workflow orchestrator.

See `.harness/capabilities/type-designer.md` for the capability's full operational contract.

## Inputs

- **Current branch** — must match `track/<id>`. The track id is resolved from this branch.
- **`track/items/<track-id>/spec.json`** — must exist (Phase 1 completed). If absent, stop
  and instruct the caller to run the `spec-design` workflow first.
- **`architecture-rules.json`** — source of truth for TDDD-enabled layers; the capability
  uses this to determine which layers to process. Every TDDD-enabled layer is processed; the
  capability handles per-layer selection internally.
- **ADR path(s)** — paths under `knowledge/adr/` for the feature domain.

Conventions are **not** an input to this workflow. The `type-designer` capability reads exactly
the convention set the capability dispatcher resolves and delivers with the dispatch, and treats
that set as complete — including when it resolves to zero documents
(`.harness/capabilities/type-designer.md` § Compliance). This workflow neither selects nor
forwards convention paths.

## Sequence

**Step 1: Pre-check**

Confirm `track/items/<track-id>/spec.json` exists (Phase 1 output). If not, stop and
instruct the caller to run the `spec-design` workflow
(`.harness/workflows/track/spec-design.md`) first.

**Step 2: Review the spec scope**

1. Invoke the `review` workflow's single-scope re-entry round for `spec`
   (`.harness/workflows/track/review.md` §Single-scope re-entry round) to `zero_findings`.
2. Confirm the current Chain 1 semantic verification with
   `bin/sotp ref-verify check-approved --chain 1`. The preceding Phase 1 loop owns any required
   Chain 1 refresh; do not run the all-chain `ref-verify run` here because pending stale Chain 2
   pairs are regenerated only after this phase enters.

**Step 3: Enter the type-design phase**

Prepare the configured writer briefing at `tmp/type-designer-briefing.md`. It must include:

- Track id and the path `track/items/<track-id>/spec.json`
- Path to `architecture-rules.json` (source of truth for TDDD-enabled layers)
- Path(s) to the referenced ADR(s) under `knowledge/adr/`

The briefing must **not** carry convention paths. The capability's convention set comes solely
from the dispatcher's resolution; adding a hand-picked path here would make an unresolved
document an input and would leave a zero-document resolution non-authoritative.

Then run `bin/sotp phase enter type-design`. The phase engine runs the declared pre-entry
checks and, only when they all succeed, invokes the configured `type-designer` writer. The
workflow must not dispatch that writer directly. The writer owns baseline capture, each
`<layer>-types.json` write, all rendered views (type-graph md, contract-map.md,
`<layer>-type-signals.md`), and type → spec signal evaluation (🔵🟡🔴) per layer.

Before the capability starts its baseline-capture step, it must author
`track/items/<track-id>/tddd-features.json`: a schema-versioned, total mapping of every
TDDD-enabled layer to the Cargo features used for rustdoc extraction. Featureless layers are
explicit empty lists. The declaration is the only feature input route; no track command gains a
feature argument, flag, or subcommand. Baseline capture fail-closes if it is absent and freezes
its bytes for the later actual-capture check.

**Step 4: Receive and surface the per-layer signal result**

Receive the per-layer blue / yellow / red counts from the capability output. Surface the full
per-layer signal result as the workflow output without re-reading the catalogue files.

**Step 5: Materialize test-obligation enrollment artifacts (mandatory terminal step)**

After the capability returns, the workflow orchestrator materializes the track's
test-obligation enrollment (ADR 2026-07-23-0240 D1):

1. Run `bin/sotp test-obligation derive` to materialize
   `track/items/<track-id>/obligations.json`. A zero-obligation derivation still materializes
   the artifact — artifact absence and an empty derivation result must remain distinguishable.
2. If `track/items/<track-id>/test-bindings.json` does not exist, materialize it as an
   explicit records-empty authoring act: `{"track_id": "<track-id>", "records": []}`.
   An existing bindings file is left untouched.

Both artifacts belong to the same commit unit as the other plan artifacts. This step is not
conditional on any orchestrator judgment: every run of this workflow on a track with at least
one TDDD catalogue performs it. On re-entry (catalogue or spec re-generation through the
back-and-forth loops), re-run `bin/sotp test-obligation derive` so the obligations reflect
the regenerated upstream.

## Gates

| Gate | Verdict |
|------|---------|
| `spec.json` exists | ERROR if absent |
| Spec-scope fast and final reviews | `zero_findings` / ERROR |
| Chain 1 semantic verification after spec review | `check-approved --chain 1` succeeds / ERROR |
| Capability reports per-layer signal counts | OK (counts surfaced to caller) |
| `bin/sotp test-obligation derive` exits 0 and both enrollment artifacts exist | OK / ERROR |

The workflow itself does not enforce a minimum signal color — the caller (`plan` workflow)
applies the loop rule (🔵 proceed / 🟡 recover before downstream entry / 🔴 escalate). This
workflow is single-shot.

## Failure / recovery

- **Missing spec.json**: stop and instruct the caller to run the `spec-design` workflow first.
- **Spec-scope review findings**: do not enter the phase; return to the caller for the Phase 1
  recovery route, then re-run the spec-scope single-scope review.
- **Chain 1 semantic verification failure**: do not enter the phase; return to the caller for
  the Phase 1 recovery route.
- **Spec-scope review blocked_cross_scope**: return to the caller for the `plan` workflow's
  Phase 2 rollback route; use the `diagnose` workflow when the routing target is unclear.
- **Phase-entry failure**: retry up to 2 times for transient execution failures. A failed
  pre-entry check does not launch the writer; report it to the caller and stop.
- **Capability returns 🔴 on one or more layers**: surface the failing layer(s) and signal
  detail to the caller. The caller decides the escalation path (re-invoke the `spec-design`
  workflow,
  escalate to `adr-editor`, or pause for the user). This workflow does not trigger back-and-forth
  escalation on its own.

## Outputs

- `track/items/<id>/tddd-features.json` (authored by the type-designer before baseline capture)
- `track/items/<id>/<layer>-types.json` for every TDDD-enabled layer (written by the capability)
- `<layer>-types.json` baseline files (captured by the capability)
- Rendered views: type-graph md, contract-map.md, `<layer>-type-signals.md` (generated by the capability)
- `track/items/<id>/obligations.json` (derived by Step 5) and `test-bindings.json`
  (records-empty materialization when absent)
- Per-layer signal counts: blue / yellow / red
- No commit is created by this workflow
