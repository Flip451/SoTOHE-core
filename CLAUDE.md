# CLAUDE.md

Maintainer index for SoTOHE-core. First-time user onboarding lives in `README.md`. This file is a pointer map: rule content lives in the referenced files. Do not inline rules here — add a pointer, and keep pointers in sync when files move.

## Mental model: SoT Chain

All work is organized as a one-way reference chain of four Sources of Truth:

```
ADR (knowledge/adr/, permanent, cross-track)
  ↑ cites
spec.json (per track)
  ↑ cites
<layer>-types.json (per track, type contracts / TDDD)
  ↑ conforms to
implementation (libs/, apps/, permanent)
```

Each downstream artifact must cite its direct upstream (no layer skipping, no reverse references). References are machine-evaluated as signals — 🔵 grounded / 🟡 grounded but must be resolved before track end / 🔴 broken. Gates are built only from these signals plus binary checks; there are no artificial `approved` / `Status` states (`knowledge/conventions/workflow-ceremony-minimization.md`). How a gate consumes signals is declared per **chain × gate** in `.harness/config/signal-gates.json` — `interim` (🔴 blocks, 🟡 warns and passes) or `strict` (🟡 also blocks). Chains: `adr_user` (ADR decision grounding), `spec_adr` (spec → ADR), `catalog_spec` (catalogue → spec), `impl_catalog` (implementation → catalogue). Committed defaults:

- commit gate — `spec_adr` / `catalog_spec` strict; `adr_user` / `impl_catalog` interim (in-progress implementation may carry 🟡 into a commit, but design-chain references must already be grounded)
- merge gate — strict on all four chains
- pre-review task-contract gate (`bin/sotp task-contract check`, binary) — reads `impl_catalog` signals per task status via `task-contract.json`: entries attributed to done / in_progress tasks must be 🔵, todo-task entries may remain 🟡, 🔴 always blocks. Its precondition — every catalogue entry attributed to an existing task — is enforced separately by `bin/sotp task-contract coverage` (in `cargo make ci`). The gate asserts structural conformance only; body semantics remain the reviewer's lane.
- ADR-baseline freeze gate (`bin/sotp adr-baseline`) — at Phase 0, the orchestrator designates primary ADR source(s) exclusively by creating `init` snapshots; those ledger records are the designation, with no separate metadata or external primary pointer. Review wrappers run `check-review`, whose CLI requires a nonempty init-record designation set and verifies every recorded ledger copy; a current ADR that differs from its latest baseline is a normal Phase 0 draft state and does not block review dispatch. `--primary-source <file>` is only a direct-CLI override. Byte matching fires at the commit gate and track-aware CI: `cargo make ci-track` and guarded commits run `check-commit` for all recorded ADRs and separately enforce coverage for non-draft ADRs cited by `spec.json`. This is a separate byte-comparison gate; it does not change `adr_user` evaluation or `.harness/config/signal-gates.json`.

The unit of work is a **track** (one feature / fix / refactor) living on branch `track/<id>` with its artifacts under `track/items/<id>/`. The active track id is resolved from the current git branch name (branch-bound); on the base branch resolution fail-closes as `NotTrackBranch`, and only read commands with an explicit `--track-id` work there (`knowledge/conventions/branch-strategy.md`).

## Primary pipeline

The canonical implementation flow is **pre-track ADR → `/track:adr2pr`**:

1. Author a front-matter-complete ADR under `knowledge/adr/` **before** any track exists (`knowledge/conventions/pre-track-adr-authoring.md`). YAML front-matter with grounded `decisions[]` is mandatory (`knowledge/conventions/adr.md`); ungrounded decisions are 🔴.
2. `/track:adr2pr [<feature>] [--primary-adr <filename>.md]` — autonomous drive to a reviewed PR, no merge: init → ADR-baseline review + commit → Phase 1-3 (spec-design → type-design → impl-plan) → plan-artifacts review + commit → full-cycle (implement → DRY check → review → commit, batch-first) → pr-review. Both arguments are optional: an explicitly supplied value takes precedence, and a missing value is resolved from the conversation context and confirmed once with the user (candidate selection when resolution is not unique; a direct value request when context has no candidate) before both values are forwarded explicitly to `/track:init <feature> --primary-adr <filename>.md`. Stops with the PR open; the user merges (`/track:merge`).
3. `/track:done` — after merge, return to the configured base branch.

Every pipeline stage also exists as an individual `/track:*` command (`plan` orchestrates Phase 0-3 only; `status` / `catchup` are safe anywhere). `/track:diagnose` routes impl-phase structural inconsistencies back to the phase that owns the root cause.

Workflow logic SSoT for workflow-backed track commands is `.harness/workflows/track/*.md` (provider-agnostic). The corresponding `.claude/commands/track/*.md` files are thin Claude Code adapters (invocation form, tool constraints, report format only) — never duplicate workflow logic into an adapter. Utility commands without a workflow SSoT document their command-local behavior in their command file until a workflow SSoT exists.

## Source of Truth map

| Concern | SSoT |
|---|---|
| Workspace layers / dependency direction / module limits | `architecture-rules.json` |
| Tech stack / product-policy decisions | `knowledge/adr/` (pre-track ADRs; index: `knowledge/adr/README.md`) |
| Capability → provider routing | `.harness/config/agent-profiles.json` |
| Branch strategy (base / merge target / method) | `.harness/config/branch-strategy.json` (+ per-track `metadata.json#branch_strategy_snapshot`) |
| Review scopes / signal gates / DRY gate | `.harness/config/{review-scope,signal-gates,dry-check}.json` |
| Reviewer briefings (per layer / per SoT scope) | `.harness/custom/review-prompts/*.md` |
| Track identity (Phase 0) | `track/items/<id>/metadata.json` |
| Behavioral contract (Phase 1) | `track/items/<id>/spec.json` |
| Type contracts (Phase 2) | `track/items/<id>/<layer>-types.json` (schema: `knowledge/conventions/catalogue-schema-reference.md`) |
| Implementation plan / spec coverage / contract attribution (Phase 3) | `track/items/<id>/impl-plan.json` + `task-coverage.json` + `task-contract.json` |
| Architectural decisions | `knowledge/adr/` (index: `knowledge/adr/README.md`) |
| ADR baseline ledger and verbatim copies | `track/items/<id>/adr-baseline/` (written only by `bin/sotp adr-baseline snapshot`) |

Derived read-only views — never hand-edit, regenerate via `bin/sotp track views sync`: `spec.md`, `plan.md`, `track/registry.md` (gitignored), `contract-map.md`, `*-graph/`. Optional free-form manual log: `track/items/<id>/observations.md`.

## Hard invariants

- Public UI is `/track:*`; never use legacy aliases.
- No direct `git add` / `commit` / `merge` / `rebase` / `switch` — use `/track:*` or the guarded `bin/sotp git`, `bin/sotp track branch`, and `bin/sotp pr` workflow commands (`.claude/rules/10-guardrails.md`).
- The orchestrator never edits SoT files directly (1 file = 1 writer): ADR → `adr-editor`, `spec.json` → `spec-designer`, catalogues → `type-designer`, `impl-plan.json` → `impl-planner`. Task state transitions go through `bin/sotp track transition`.
- Every commit (plan artifacts included) is preceded by a reviewer-capability cycle to `zero_findings`; inline self-review is never a substitute (`knowledge/conventions/review-protocol.md`).
- Do not manually copy, edit, or remove ADR baseline records. A missing init snapshot blocks review and commit; a byte mismatch blocks commit and track-aware CI (not review — a Phase 0 draft divergence is normal). Use the sanctioned snapshot, restore, and diagnoser routes.
- Enforce rules by mechanism (type system > CI gate > hook > lint > docs), and prefer type-safe abstractions over lint/doc rules (`knowledge/conventions/{enforce-by-mechanism,prefer-type-safe-abstractions}.md`).
- Read `.claude/rules/08-orchestration.md`, `09-maintainer-checklist.md`, and `10-guardrails.md` before making changes.

## Rules and conventions

- `.claude/rules/` — Claude Code-specific operating rules (only these five): `01-language.md` (think in English, answer in Japanese), `07-dev-environment.md` (toolchain, cargo-make, `bin/sotp` build), `08-orchestration.md` (delegation), `09-maintainer-checklist.md` (co-update matrix), `10-guardrails.md` (guards, hooks, permissions).
- `knowledge/conventions/` — ~30 engineering conventions; the index in its `README.md` is auto-generated (`bin/sotp conventions update-index`) and lists them in reading order. Load-bearing for daily work: `track-lifecycle.md`, `branch-strategy.md`, `git-notes.md`, `task-completion-flow.md`, `pre-track-adr-authoring.md`, `adr.md`, `review-protocol.md`, `coding-principles.md`, `type-designer-kind-selection.md`, `no-upstream-restatement.md`.

## Workspace shape

Six crates. `architecture-rules.json` and `deny.toml` are the SSoT for permitted dependency direction and are enforced by `cargo make check-layers` / `deny`; `type-designer-kind-selection.md` R1 is the SSoT for role × layer placement. Delivery is split into `apps/cli` (bin), `apps/cli-driver` (primary adapter that depends on usecase only), and `apps/cli-composition` (composition root that wires all layers). Inspect with `bin/sotp arch tree` / `tree-full`.

## Quick commands

- `bin/sotp track resolve` — current phase / next command / blocker
- `bin/sotp track views sync` — regenerate `plan.md` + `registry.md`
- `cargo make ci` — full pre-commit gate (`ci-rust` = inner Rust-only loop)
- `cargo make ci-track` — active-track gates, including ADR-baseline `check-commit`
- `cargo make --list-all-steps` — task catalogue (details: `.claude/rules/07-dev-environment.md`)

## Delegation surfaces

- Capability specs: `.harness/capabilities/*.md`; provider routing: `.harness/config/agent-profiles.json` (orchestrator host may be Claude Code or Codex).
- Skills: `architecture-customizer` (layer migration) — definition under `.claude/skills/architecture-customizer/SKILL.md`.

## Maintenance

When changing workflow or architecture, update all affected layers together (`.claude/rules/09-maintainer-checklist.md`: README, conventions, `Makefile.toml`, `sotp verify` gates, `.claude/settings.json` hooks), then run `cargo make ci`.
