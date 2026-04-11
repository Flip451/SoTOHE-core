# Strict Spec Signal Gate — Yellow blocks merge

## Status

Accepted

## Context

SoTOHE-core uses a 3-level confidence signal system (Blue/Yellow/Red) to evaluate the provenance of spec requirements. Prior to this ADR, the signal gate only blocked Red (missing sources) at CI time, while Yellow (inference, discussion, feedback) was allowed at all stages including merge.

This meant requirements could reach main with no persistent documentation of their rationale. A developer could write `[source: discussion]` or `[source: feedback — user said so]` and pass all gates.

### Problem: feedback as Blue

The `feedback` source type was mapped to Blue (same confidence level as `document` and `convention`), despite having no persistent artifact. This created a trivial bypass: any Yellow item could be upgraded to Blue by rewriting the source tag to `feedback — approved` without creating any documentation.

### Fail-closed principle

SoTOHE-core follows a fail-closed design philosophy across all gates:
- Hook errors → block (ADR 2026-03-11-0050)
- review.json unreadable → bypass denied
- Baseline absent → signal evaluation error
- The merge gate should follow the same principle

## Decision

### D1: Yellow blocks merge

`wait-and-merge` reads `spec.json` from the PR head ref (via `git show origin/branch:path`) and blocks merge when:
- `signals` is absent → BLOCKED (unevaluated)
- `signals.red > 0` → BLOCKED (missing sources)
- `signals.yellow > 0` → BLOCKED (undocumented sources)

Only `signals.yellow == 0 && signals.red == 0` (all Blue) allows merge.

This creates a structural incentive to record design decisions before merge:
- `inference` / `discussion` / `feedback` → Yellow → merge blocked
- Write an ADR or convention document → reference as `document` source → Blue → merge allowed

### D2: Downgrade feedback from Blue to Yellow

`SignalBasis::Feedback` is remapped from `ConfidenceSignal::Blue` to `ConfidenceSignal::Yellow`.

Blue sources must reference persistent files:
- `document` → references a file (ADR, spec, PRD)
- `convention` → references a convention file

Yellow sources lack persistent documentation:
- `feedback` → "user said so" (no file)
- `inference` → "I think because..." (no file)
- `discussion` → "we agreed" (no file)

To upgrade `feedback` to Blue, the developer must persist the decision in an ADR or convention document and reference it as a `document` source.

### D3: spec.json required (fail-closed)

All new tracks created by `/track:plan` include `spec.json`. Legacy tracks without `spec.json` are already completed and will not be re-merged. When `git show` cannot find `spec.json` on the PR head ref, merge is blocked.

### D4: Pattern follows check_tasks_resolved

The implementation uses the same `git show origin/branch:path → decode → check` pattern as the existing task completion guard, ensuring the gate validates the remote state rather than stale local files.

## Rejected Alternatives

### A. Delegate to verify_from_spec_json via temp file

Writing remote `spec.json` to a temp file and calling `verify_from_spec_json(path, strict=true)`. Rejected because:
- Requires temp file management (creation, cleanup)
- `verify_from_spec_json` also checks domain-types.json as a sibling file, which isn't available from the remote ref
- The simple `git show → decode → check signals` pattern is sufficient for the merge gate since CI already runs the full `verify spec-states` (default mode)

### B. Keep feedback as Blue

Keep the existing Blue mapping for `feedback` sources. Rejected because:
- Provides a trivial bypass for the strict gate
- No persistent artifact is created
- Undermines the purpose of requiring documented rationale

### C. Skip gate for legacy tracks without spec.json

Return SUCCESS when `spec.json` is not found. Rejected because:
- Violates fail-closed principle
- All new tracks have `spec.json`
- Legacy tracks are completed and won't merge again

## Consequences

### Good

- **ADR creation is structurally incentivized**: The only way to upgrade Yellow to Blue is to write a persistent document. This creates a natural workflow where design decisions are recorded before merge.
- **Fail-closed**: Missing or unevaluated specs block merge.
- **Consistent with existing gates**: Same pattern as task completion guard.

### Bad

- **Higher friction for small changes**: Even trivial features need spec sources that reference persistent files. This can feel disproportionate for 5-line changes.
- **Domain-type signals not checked**: The merge gate only checks Stage 1 (spec signals), not Stage 2 (domain-type signals). CI covers Stage 2 in default mode (Red blocked), but Yellow domain-type signals can reach main. This is an accepted limitation — replicating the full verify pipeline from remote refs is complex.
- **Race condition**: Guards run once before the poll loop, not at merge time. A post-validation push could bypass the gate. Tracked as SEC-10 in TODO.md.

## Reassess When

- A lightweight "micro-track" workflow is introduced that reduces planning overhead for small changes
- Domain-type signal strict gate is needed at merge time (would require reading domain-types.json from remote refs)
- `feedback` needs to be re-elevated to Blue (would need a persistent artifact requirement, e.g., feedback must reference a memory file or PR comment)
