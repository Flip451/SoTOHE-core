# Auto Mode Integration Design

> How `/track:auto` coexists with `/track:full-cycle` and the migration path.

## 1. Current State: /track:full-cycle

`/track:full-cycle` is the existing autonomous implementation path:

1. Resolve the current track and map task to approved scope
2. Mark target task `in_progress`
3. Read spec.md, plan.md, conventions (plus observations.md if present)
4. Implement with Agent Teams and focused validation
5. Run local review loop until zero findings
6. Run `cargo make ci`
7. Append to observations.md only when machine-non-verifiable observations arose or when `spec.json` acceptance_criteria explicitly requires it, then mark task `done`

**Limitations**:
- Single-task scope per invocation
- No state persistence between sessions (context lost on crash/timeout)
- No escalation mechanism (blocks on failure until user intervenes)
- No phase separation (design and implementation mixed)
- 2-phase cycle only (implement + review)

## 2. Comparison

| Aspect | /track:full-cycle | /track:auto |
|--------|------------------|-------------|
| **Scope** | Single task | Entire track (all tasks sequentially) |
| **Phases** | 2 (implement + review) | 6 per task (plan → plan-review → type-design → type-review → implement → code-review) |
| **State persistence** | None (in-memory only) | auto-state.json (cross-session) |
| **Escalation** | Blocks conversation | Persists state + exits cleanly |
| **Human interaction** | Synchronous (waits) | Asynchronous (exit + resume) |
| **Review model** | Fast + full model escalation | Same, per review phase |
| **Type design** | Mixed with implementation | Dedicated phase with separate review |
| **Commit granularity** | One commit per task | One commit per task (same) |
| **SSoT** | metadata.json | metadata.json (unchanged) |
| **Task transitions** | sotp track transition | sotp track transition (same) |

## 3. Migration Strategy

### Phase 1: Coexistence (this design spike)

- `/track:auto` is a new, independent command
- `/track:full-cycle` remains unchanged and fully functional
- Both commands use the same infrastructure:
  - `metadata.json` SSoT via `sotp track` CLI
  - `cargo make ci` for quality gates
  - Reviewer capability (resolved via `agent-profiles.json`: Codex CLI, Claude subagent, etc.)
  - Agent Teams for parallel execution
- `/track:auto` adds:
  - `auto-state.json` for cross-session persistence
  - `.claude/auto-mode-config.json` for phase configuration
  - 6-phase orchestration loop
  - Escalation/resume mechanism
- No breaking changes to existing workflow

### Phase 2: Feature Parity

- `/track:auto --task T001` supports single-task mode
- Feature parity with `/track:full-cycle` achieved
- `/track:full-cycle` can be implemented as:
  ```
  /track:auto --task {task_id}   # equivalent behavior
  ```
- Users can choose either command based on preference:
  - `/track:full-cycle` for quick single-task work (no overhead)
  - `/track:auto` for structured multi-task execution

### Phase 3: Deprecation

- `/track:full-cycle` skill definition updated to recommend `/track:auto --task`
- Warning message added to `/track:full-cycle` output
- `/track:full-cycle` remains functional but documented as deprecated
- New users directed to `/track:auto`

### Phase 4: Removal (optional, far future)

- `/track:full-cycle` skill removed
- All references updated to `/track:auto`

## 4. Shared Infrastructure

### Reused from existing codebase

| Component | Used By | No Changes Needed |
|-----------|---------|-------------------|
| `metadata.json` SSoT | Task state tracking | Yes |
| `sotp track transition` | Task state transitions | Yes |
| `sotp track next-task` | Finding next task to process | Yes |
| `sotp track task-counts` | Progress reporting | Yes |
| `cargo make ci` / `ci-rust` | Quality gates | Yes |
| `cargo make track-local-review` | Reviewer invocation | Yes |
| Agent Teams | Parallel implementation | Yes |
| `cargo make track-commit-message` | Guarded commit | Yes |
| `cargo make track-note` | Git note attachment | Yes |

### New components added by /track:auto

| Component | Purpose | Location |
|-----------|---------|----------|
| `auto-state.json` | Cross-session state persistence (ephemeral, not git-tracked) | `track/items/<id>/auto-state.json` |
| `auto-mode-config.json` | Phase configuration | `.claude/auto-mode-config.json` |
| `AutoPhase` enum | Phase state machine | `libs/domain/src/auto_phase.rs` |
| Phase orchestration | 6-phase loop | Skill definition (`.claude/skills/`) |
| Escalation UI | Human intervention | Skill definition |

## 5. Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| auto-state.json conflicts with metadata.json as dual SSoT | High | Low | auto-state.json is ephemeral session state only. metadata.json remains the durable SSoT for task status. auto-state.json references task IDs but never modifies task status directly. |
| Auto mode makes unwanted commits | Medium | Medium | `auto_commit` config flag (default true, can disable). Dry-run mode for testing. All commits go through existing guarded commit path. |
| Escalation state becomes stale | Low | Medium | Warn on --resume if auto-state.json is >24h old or if track metadata has changed. TTL-based expiry suggestion. |
| 6-phase overhead for simple tasks | Low | High | Single-task mode (`--task`) available. For trivial tasks, phases may be fast (plan = "implement as described", review = zero_findings). |
| Agent Teams resource contention | Medium | Low | Auto mode processes tasks sequentially (not in parallel). Agent Teams used within phases (e.g., parallel test execution), same as existing /track:full-cycle. |
| Phase configuration drift | Low | Low | auto-mode-config.json is version-controlled. CI can validate schema. |
