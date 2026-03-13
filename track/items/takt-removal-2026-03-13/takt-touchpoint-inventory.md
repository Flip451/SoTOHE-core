# Takt Touchpoint Inventory

## Purpose

This document fixes the current `takt` dependency surface before removal work starts.
It records what still depends on `takt`, why it matters, and what kind of replacement or
deletion path is required to keep `/track:*` working.

## Cutover Principles

1. Public UI stays `/track:*`.
2. `metadata.json` remains the SSoT for track state and rendered views.
3. `takt` is removed rather than replaced with another queue/orchestrator.
4. Pending commit/note/staging artifacts must converge on `tmp/track-commit/` or another
   explicit non-`takt` scratch path.
5. Guardrails, CI, and docs must move in the same phase as runtime changes; no mixed state
   where docs still direct users into removed `takt` paths.
6. Removal must be fail-closed for security-sensitive workflow paths:
   branch guard, direct-git blocking, commit note application, and CI gates.

## Inventory

### 1. Runtime and Queue Assets

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `.takt/config.yaml` | `takt` project/runtime config | `/track:setup` and maintainer guidance still point at it | remove after docs and command references are rewritten |
| `.takt/pieces/` | autonomous execution definitions | `/track:full-cycle` and direct `takt-*` wrappers depend on piece names | delete only after `/track:full-cycle` is redefined or retired |
| `.takt/personas/` | source personas for runtime generation | `takt-render-personas` and queue runs depend on them | remove with `scripts/takt_profile.py` |
| `.takt/runtime/personas/` | rendered runtime personas | host/runtime cache for `takt` execution | stop generating before deleting |
| `.takt/tasks.yaml` and `.takt/tasks/` | queue state | `takt-add` / `takt-run` depend on them | no migration; queue is removed |
| `.takt/handoffs/` | human handoff scratch on queue failure | transient automation dir excluded from staging | move any needed human handoff flow to `tmp/` or drop it |
| `.takt/last-failure.log`, `.takt/debug-report.md` | failure diagnostics | `takt-failure-report.py` and queue recovery docs depend on them | either remove or replace with generic workflow failure diagnostics |

### 2. Cargo Make Wrapper Surface

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `cargo make takt-add` | queue enqueue wrapper | user docs still advertise it | delete after docs/rules stop recommending queue usage |
| `cargo make takt-run` | queue execution wrapper | same as above | delete with queue assets |
| `cargo make takt-render-personas` | runtime persona generation | recovery docs and setup still mention it | replace docs first |
| `cargo make takt-full-cycle` | direct autonomous piece execution | `/track:full-cycle` currently shells into it | must be replaced before removal |
| `cargo make takt-spec-to-impl` | direct piece wrapper | direct docs and wrapper tests depend on it | remove with piece layer |
| `cargo make takt-impl-review` | direct review piece wrapper | direct docs and wrapper tests depend on it | remove with piece layer |
| `cargo make takt-tdd-cycle` | direct TDD piece wrapper | direct docs and wrapper tests depend on it | remove with piece layer |
| `cargo make takt-clean-queue` | queue maintenance | queue-specific only | remove with queue layer |
| `cargo make takt-failure-report` | queue failure report helper | docs/tests still cover it | decide remove vs generic rename explicitly |
| `cargo make add-pending-paths`, `commit-pending-message`, `note-pending` | pending artifact wrappers using `.takt/pending-*` | `/track:commit` fallback and git helper contracts still preserve them | replace with non-`takt` scratch path before deletion |

### 3. Python Runtime and Tests

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `scripts/takt_profile.py` | profile-aware `takt` launcher and queue manager | all `takt-*` wrappers and queue tests depend on it | remove after wrappers/docs are gone |
| `scripts/test_takt_profile.py` | regression suite for queue/pieces/runtime personas | CI still executes it | replace or delete in same phase as script removal |
| `scripts/takt_failure_report.py` | debug report generator | `takt-failure-report` wrapper and tests depend on it | decide whether to delete or generalize |
| `scripts/test_takt_failure_report.py` | regression tests for above | CI still executes it | remove with helper or rename with generic replacement |
| `scripts/test_takt_personas.py` | persona rendering checks | CI still executes it | remove with runtime persona path |

### 4. Public Command and Workflow Docs

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `.claude/commands/track/full-cycle.md` | `/track:full-cycle` shells into `cargo make takt-full-cycle` | public interface breaks if wrapper disappears first | rewrite command contract before wrapper removal |
| `.claude/commands/track/setup.md` | setup checks `.takt/config.yaml` placeholder values | setup remains stale after `takt` removal | remove `.takt` prerequisite during doc rewrite |
| `.claude/commands/track/commit.md` | prefers `.takt/pending-note.md` | commit traceability keeps hidden `takt` dependency | switch preferred path to non-`takt` scratch |
| `.claude/docs/WORKFLOW.md` | describes `takt` as execution workflow | onboarding remains incorrect | rewrite with Claude Code + Rust CLI model |
| `track/workflow.md` | primary workflow guide still has “Integration with takt” and pending-note rules | user guidance and CI docs remain wrong | rewrite in same wave as command docs |
| `TAKT_TRACK_TRACEABILITY.md` | SSoT/traceability rules tied to `takt` progression and notes | source-of-truth docs stay inaccurate | replace with neutral track traceability doc or rewrite in place |
| `DEVELOPER_AI_WORKFLOW.md` | user guide advertises `takt-*` and Python/uv setup | onboarding remains tact-coupled | rewrite after replacement workflow is settled |
| `LOCAL_DEVELOPMENT.md` | local usage docs for `takt-*` | same as above | rewrite/remove together |
| `START_HERE_HUMAN.md` | mentions `takt` movement/runtime persona recovery | top-level onboarding remains wrong | rewrite in same doc wave |

### 5. Orchestration and Routing Rules

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `.claude/rules/08-orchestration.md` | defines `takt` as execution workflow layer | architecture guidance remains wrong | rewrite before removing commands |
| `.claude/rules/09-maintainer-checklist.md` | tells maintainers to update `.takt/*` definitions | maintenance checklist becomes stale | remove `takt` section with runtime removal |
| `.claude/rules/07-dev-environment.md` | lists `takt-*` and `.takt/pending-*` wrappers as standard commands | developer guidance stays wrong | rewrite with replacement scratch flow |
| `.claude/rules/02-codex-delegation.md`, `.claude/rules/03-gemini-delegation.md` | contain `takt`-specific delegation caveats | hidden coupling remains in provider rules | review and prune once no `takt` runner remains |
| `.claude/hooks/agent-router.py` | recommends `takt-*` automated workflow commands | user prompt routing still sends users to removed commands | rewrite in same wave as command docs |
| `.claude/hooks/_agent_profiles.py` | validates `takt_host_provider` / `takt_host_model` | profile config cannot be simplified while fields remain required | remove fields and validation together |
| `.claude/agent-profiles.json` | stores `takt_host_provider` and `takt_host_model` per profile | profile schema remains tact-specific | remove after hook helper changes |

### 6. Guardrails, Permissions, and CI

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `.claude/settings.json` | allowlists `cargo make takt-*` and deny rules for `.cache/takt-uv/**` | permissions drift if wrappers are removed but allowlist remains | clean in same phase as wrapper removal |
| `scripts/verify_orchestra_guardrails.py` | requires those permissions and deny rules | CI fails until verifier is updated | update verifier before deleting settings entries |
| `scripts/test_verify_scripts.py` | regression tests for expected guardrails | same as above | update/delete in same change as verifier |
| `scripts/test_make_wrappers.py` | smoke-tests `takt-*` wrappers | CI fails after wrapper deletion unless updated | rewrite or remove same phase |
| `cargo make ci` task list | still executes `scripts/test_takt_*` | CI continues enforcing removed features | remove test entries with runtime removal |

### 7. Guarded Git and Scratch Path Contracts

| Surface | Current role | Removal impact | Cutover principle |
|---|---|---|---|
| `libs/usecase/src/git_workflow.rs` | treats `.takt/pending-*` and `.takt/handoffs` as transient automation paths | branch/staging rules keep hidden tact assumptions | switch to replacement scratch contract before deleting paths |
| `scripts/git_ops.py` | mirrors the same transient file/dir contract for Python wrapper compatibility | legacy helper behavior diverges if not updated | keep parity until Python helper is retired |
| `scripts/test_git_ops.py` | regression coverage for `.takt/pending-*` and handoffs | CI fails after path removal unless updated | rewrite tests with replacement scratch semantics |
| `apps/cli/src/commands/git.rs` and `libs/infrastructure/src/git_cli.rs` tests | still mention `.takt/pending-note.md` path resolution | Rust git helper surface remains tact-aware | rewrite once replacement note path is chosen |

## Sequencing Constraints

1. Rewrite public `/track:*` docs and routing before deleting any user-facing `takt-*` command.
2. Replace pending-note / pending-message / pending-add-paths scratch contracts before removing `.takt/pending-*` wrappers.
3. Remove profile schema keys and routing text in the same phase as `scripts/takt_profile.py` retirement.
4. Update guardrail verifier/tests and `.claude/settings.json` in the same change as wrapper permission removal.
5. Only delete `.takt/` runtime and queue assets after CI no longer references their tests or docs.

## Exit Condition for T001

T001 is complete when this inventory is the agreed source of removal scope and every remaining
task in the track maps back to one or more sections above.
