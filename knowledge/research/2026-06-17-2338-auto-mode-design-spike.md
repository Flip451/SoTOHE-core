# Auto Mode Design Spike — Archived Design Notes

> Source: `knowledge/designs/auto-mode-agent-briefings.md`,
> `knowledge/designs/auto-mode-escalation-ui.md`,
> `knowledge/designs/auto-mode-integration.md`,
> `knowledge/schemas/auto-mode-config-schema.md`,
> `knowledge/schemas/auto-state-schema.md`
>
> Original track: `track/items/auto-mode-design-2026-03-16/` (status: done, MEMO-15)
>
> These directories were removed as part of the knowledge/strategy cleanup (ADR
> `2026-06-17-1321-knowledge-strategy-cleanup.md`, D2/D3). This file salvages the
> design information not yet implemented in the codebase. The domain-layer type
> definitions (`AutoPhase`, `RollbackTarget`, `FindingSeverity`, etc.) are already
> encoded in `libs/domain/src/auto_phase.rs`. The information below covers the
> remaining unimplemented parts: config/state schemas, agent briefing templates, CLI
> interface, and integration strategy.

---

## 1. auto-mode-config.json Schema

Configuration for `/track:auto` phase-to-capability mapping and operational parameters.
Placed at `.claude/auto-mode-config.json`.

### JSON Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "AutoModeConfig",
  "type": "object",
  "required": ["version", "phases", "escalation_policy", "settings"],
  "properties": {
    "version": { "type": "integer", "const": 1 },
    "phases": {
      "type": "object",
      "required": ["plan", "plan_review", "type_design", "type_review", "implement", "code_review"],
      "properties": {
        "plan": { "$ref": "#/$defs/phase_config" },
        "plan_review": { "$ref": "#/$defs/phase_config" },
        "type_design": { "$ref": "#/$defs/phase_config" },
        "type_review": { "$ref": "#/$defs/phase_config" },
        "implement": { "$ref": "#/$defs/phase_config" },
        "code_review": { "$ref": "#/$defs/phase_config" }
      }
    },
    "escalation_policy": {
      "type": "object",
      "required": ["max_consecutive_rollbacks", "escalation_on_timeout"],
      "properties": {
        "max_consecutive_rollbacks": {
          "type": "integer", "minimum": 1, "default": 3,
          "description": "Max consecutive rollbacks before auto-escalation"
        },
        "escalation_on_timeout": {
          "type": "boolean", "default": true,
          "description": "Escalate to human when a phase times out"
        }
      }
    },
    "settings": {
      "type": "object",
      "required": ["auto_commit", "verify_before_advance"],
      "properties": {
        "auto_commit": {
          "type": "boolean", "default": true,
          "description": "Automatically commit after CodeReview passes"
        },
        "verify_before_advance": {
          "type": "boolean", "default": true,
          "description": "Run cargo make ci-rust before advancing from Implement"
        }
      }
    }
  },
  "$defs": {
    "phase_config": {
      "type": "object",
      "required": ["capability", "max_rounds"],
      "properties": {
        "capability": {
          "type": "string",
          "enum": ["planner", "reviewer", "implementer", "researcher", "debugger"],
          "description": "Capability name from agent-profiles.json"
        },
        "max_rounds": {
          "type": "integer", "minimum": 1, "default": 5,
          "description": "Max rounds for this phase before escalation"
        },
        "timeout_seconds": {
          "type": ["integer", "null"], "minimum": 60, "default": null,
          "description": "Per-phase time budget in seconds. Null means no timeout."
        },
        "description": {
          "type": "string",
          "description": "Human-readable description of what this phase does"
        }
      }
    }
  }
}
```

### Example auto-mode-config.json

```json
{
  "version": 1,
  "phases": {
    "plan":        { "capability": "planner",    "max_rounds": 3, "timeout_seconds": 300, "description": "Task-level implementation planning" },
    "plan_review": { "capability": "reviewer",   "max_rounds": 5, "timeout_seconds": 300, "description": "Review implementation plan for completeness and feasibility" },
    "type_design": { "capability": "planner",    "max_rounds": 3, "timeout_seconds": 300, "description": "Design trait/struct/enum signatures" },
    "type_review": { "capability": "reviewer",   "max_rounds": 5, "timeout_seconds": 300, "description": "Review type definitions for API ergonomics and correctness" },
    "implement":   { "capability": "implementer","max_rounds": 5, "timeout_seconds": 600, "description": "TDD implementation: Red -> Green -> Refactor" },
    "code_review": { "capability": "reviewer",   "max_rounds": 5, "timeout_seconds": 300, "description": "Code review for correctness, performance, idiomatic Rust" }
  },
  "escalation_policy": { "max_consecutive_rollbacks": 3, "escalation_on_timeout": true },
  "settings": { "auto_commit": true, "verify_before_advance": true }
}
```

### Rust Type Definitions

```rust
pub struct AutoModeConfig {
    pub version: u32,
    pub phases: PhaseMap,
    pub escalation_policy: EscalationPolicy,
    pub settings: AutoModeSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability { Planner, Reviewer, Implementer, Researcher, Debugger }

pub struct PhaseConfig {
    pub capability: Capability,
    pub max_rounds: u32,
    pub timeout_seconds: Option<u32>,
    pub description: Option<String>,
}

pub struct PhaseMap {
    pub plan: PhaseConfig,
    pub plan_review: PhaseConfig,
    pub type_design: PhaseConfig,
    pub type_review: PhaseConfig,
    pub implement: PhaseConfig,
    pub code_review: PhaseConfig,
}

pub struct EscalationPolicy {
    pub max_consecutive_rollbacks: u32,
    pub escalation_on_timeout: bool,
}

pub struct AutoModeSettings {
    pub auto_commit: bool,
    pub verify_before_advance: bool,
}
```

---

## 2. auto-state.json Schema

Ephemeral session state for `/track:auto` runs. Placed at
`track/items/<id>/auto-state.json`. Not git-tracked — add to `.gitignore`. Created on
`/track:auto` start, deleted on successful completion or `--abort`. Not SSoT for task
status (metadata.json remains SSoT).

### JSON Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "AutoModeState",
  "type": "object",
  "required": ["version", "run_id", "track_id", "task_id", "phase", "round", "started_at", "updated_at"],
  "properties": {
    "version": { "type": "integer", "const": 1 },
    "run_id": { "type": "string", "format": "uuid", "description": "Unique ID for --resume" },
    "track_id": { "type": "string", "pattern": "^[a-z0-9][a-z0-9-]*[a-z0-9]$" },
    "task_id": { "type": "string", "pattern": "^T\\d+$" },
    "phase": {
      "type": "string",
      "enum": ["plan", "plan_review", "type_design", "type_review", "implement", "code_review", "escalated", "committed"]
    },
    "round": { "type": "integer", "minimum": 1 },
    "started_at": { "type": "string", "format": "date-time" },
    "updated_at": { "type": "string", "format": "date-time" },
    "escalation": {
      "type": ["object", "null"],
      "properties": {
        "reason": { "type": "string" },
        "resume_phase": { "type": "string", "enum": ["plan","plan_review","type_design","type_review","implement","code_review"] },
        "context_summary": { "type": "string" },
        "options": { "type": "array", "items": { "type": "string" } },
        "pending_artifacts": { "type": "array", "items": { "type": "string" } },
        "escalated_at": { "type": "string", "format": "date-time" }
      },
      "required": ["reason", "resume_phase", "context_summary", "options", "pending_artifacts", "escalated_at"]
    },
    "decision": {
      "type": ["object", "null"],
      "properties": {
        "text": { "type": "string" },
        "decided_at": { "type": "string", "format": "date-time" }
      },
      "required": ["text", "decided_at"]
    },
    "completed_tasks": { "type": "array", "items": { "type": "string", "pattern": "^T\\d+$" } }
  },
  "if": { "properties": { "phase": { "const": "escalated" } } },
  "then": { "required": ["escalation"], "properties": { "escalation": { "type": "object" } } },
  "else": { "required": ["escalation"], "properties": { "escalation": { "const": null } } }
}
```

### Serialization Note

`AutoPhase` uses `#[serde(rename_all = "snake_case")]`: `Plan` → `"plan"`,
`PlanReview` → `"plan_review"`, etc. (consistent with `Display` impl in `auto_phase.rs`).

### Rust Type Definitions

```rust
pub struct AutoRunState {
    pub version: u32,
    pub run_id: uuid::Uuid,
    pub track_id: TrackId,
    pub task_id: TaskId,
    pub phase: AutoPhase,
    pub round: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub escalation: Option<EscalationInfo>,
    pub decision: Option<HumanDecision>,
    pub completed_tasks: Vec<TaskId>,
}

/// The six active (resumable) phases. Excludes Escalated and Committed
/// to make illegal resume targets unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivePhase { Plan, PlanReview, TypeDesign, TypeReview, Implement, CodeReview }

pub struct EscalationInfo {
    pub reason: String,
    pub resume_phase: ActivePhase,
    pub context_summary: String,
    pub options: Vec<String>,
    pub pending_artifacts: Vec<String>,
    pub escalated_at: chrono::DateTime<chrono::Utc>,
}

pub struct HumanDecision {
    pub text: String,
    pub decided_at: chrono::DateTime<chrono::Utc>,
}
```

---

## 3. Agent Briefing Templates (6 Phases)

Common base context injected into all phases:

```
Track: {track_id}
Task: {task_id} — {task_description}
Spec summary: {spec.md first 3 paragraphs}
Tech stack: {track/tech-stack.md key constraints}
Conventions: {relevant knowledge/conventions/*.md}
```

### Phase 1: Plan (planner capability)

Input: spec.md (full), plan.md task description, tech-stack.md, conventions, existing
type signatures, previous task artifacts (if sequential dependency).

Prompt template:
```
You are planning the implementation of task {task_id}: {task_description}

## Context
{base_context}

## Existing Types
{DESIGN.md or architecture-rules.json canonical blocks}

## Instructions
Create an implementation plan with:
1. Files to create or modify (with paths)
2. Types to define (trait/struct/enum signatures)
3. Test cases to write (test names and intent)
4. Dependencies between steps
5. Risks and mitigation

Output format: structured markdown with numbered steps.
```

### Phase 2: Plan Review (reviewer capability)

Review criteria:
- Covers all acceptance criteria
- File paths consistent with workspace structure
- Layer dependencies correct (domain <- usecase <- infrastructure <- cli)
- Test cases sufficient (happy path + error cases)
- Scope appropriate (not over-engineered)

Verdict JSON: `{"verdict":"zero_findings","findings":[]}` or
`{"verdict":"findings_remain","findings":[{"message":"...","severity":"P1|P2|P3","file":null,"line":null}]}`

Rollback trigger: P2+ → rollback to Plan. P1 → re-enter Plan (authoring phase) to fix.

### Phase 3: Type Design (planner capability)

Rules:
- No method bodies — signatures only
- No `todo!()` or `unimplemented!()`
- `/// doc comments` with `# Errors` sections
- Respect layer boundaries

### Phase 4: Type Review (reviewer capability)

Review criteria:
- Object safety (if traits used as `dyn`)
- `Send + Sync` bounds (if used across threads/tasks)
- API ergonomics (builder pattern, `Into<T>`)
- Naming consistency
- Error type granularity
- Doc comment completeness

Rollback trigger: P3 → Plan, P2 → TypeDesign, P1 → re-enter TypeDesign.

### Phase 5: Implement (implementer capability)

Follow TDD: RED (failing tests first) → GREEN (minimal code) → REFACTOR.

Rules:
- No `unwrap()`/`expect()` outside `#[cfg(test)]`
- Use `?` for error propagation
- Test naming: `test_{target}_{condition}_{expected_result}`

### Phase 6: Code Review (reviewer capability)

Review criteria:
- Logic errors, edge cases, race conditions
- No panics in library code
- Proper error propagation (`thiserror`, `#[source]`, `#[from]`)
- Architecture layer dependency direction
- Idiomatic Rust (naming, patterns, clippy compliance)
- Test coverage (happy path + error cases)
- Security (input validation, error information leakage)
- Performance (unnecessary clones, allocation patterns)

DO NOT report: test code using unwrap/expect, unchanged pre-existing code.

Rollback mapping: P3 → Plan, P2 → TypeDesign, P1 → Implement.

---

## 4. CLI Interface Design

```
/track:auto [track-id]                              # Start auto mode for all tasks
/track:auto [track-id] --task T001                  # Process a single task only
/track:auto --resume <run-id> --decision "..."      # Resume from escalation
/track:auto --status [track-id]                     # Show current auto-mode state
/track:auto --abort [run-id]                        # Abort an in-progress auto run
/track:auto [track-id] --dry-run                    # Simulate without committing
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | All tasks completed successfully |
| 1 | Escalation — human intervention needed (auto-state.json written) |
| 2 | CI failure — fixable, re-run after manual fix |
| 3 | Abort — user requested abort |
| 4 | Fatal error — unexpected failure |

### Escalation Trigger Conditions

1. Reviewer rollback count exceeds `max_consecutive_rollbacks` (default 3)
2. Phase timeout when `escalation_on_timeout` is true
3. Context overflow (task scope too large for single-pass)
4. Conflicting reviewer findings on direction
5. External dependency requires architectural decision not covered by spec

### Escalation Terminal UI

```
┌─────────────────────────────────────────────┐
│ ESCALATION: Human decision required          │
│                                               │
│ Track: auto-mode-design-2026-03-16           │
│ Task:  T003 — Implement repository trait     │
│ Phase: TypeReview (round 3)                  │
│ Run:   a1b2c3d4-e5f6-7890-abcd-ef1234567890 │
│                                               │
│ Reason: Conflicting trait bounds              │
│                                               │
│ Options:                                      │
│   1. Make callback async + Send+Sync         │
│   2. Use channel-based escalation            │
│   3. Remove Send+Sync requirement            │
│                                               │
│ Resume with:                                  │
│   /track:auto --resume a1b2c3d4 --decision   │
│     "Option 2: use channels"                  │
└─────────────────────────────────────────────┘
```

### Progress Display Format

```
[auto] T003/T007 | TypeReview | Round 2 | 0 findings | 4m23s
```

Phase transition log example:
```
[auto] T003 → Plan (round 1)
[auto] T003 → PlanReview (round 1) ✓ zero_findings
[auto] T003 → TypeDesign (round 1)
[auto] T003 → TypeReview (round 1) ✗ 2 findings (P2: type change needed)
[auto] T003 → TypeDesign (round 2) <- rollback
[auto] T003 → TypeReview (round 2) ✓ zero_findings
[auto] T003 → Implement (round 2)
[auto] T003 → CodeReview (round 2) ✓ zero_findings
[auto] T003 → Committed ✓ (abc1234)
```

### State Lifecycle

```
/track:auto start → CREATED → RUNNING → COMPLETED → CLEANED (auto-state.json deleted)
                                      ↘ ESCALATED (exit 1) → RESUMED → RUNNING
/track:auto --abort → ABORTED (auto-state.json deleted, task states unchanged)
```

Cleanup rules:
- Successful completion: delete auto-state.json after all tasks committed
- Escalation: preserve auto-state.json for --resume
- Abort: delete auto-state.json, leave metadata.json task states as-is (in_progress stays in_progress)
- Stale state: warn on --resume if auto-state.json is older than 24 hours or track metadata changed

### Error Handling

| Error Type | Behavior |
|-----------|----------|
| Network/API error | Retry with exponential backoff (1s, 2s, 4s), max 3 retries |
| CI failure (`cargo make ci`) | Stop, report error, exit code 2 |
| Reviewer timeout | If `escalation_on_timeout`: escalate. Otherwise: retry once. |
| Context overflow | Split task into subtasks, escalate with suggested split |
| Reviewer verdict extraction failure | Retry once. If still fails, escalate. |
| File conflict | Stop, report conflicting files, exit code 2 |

---

## 5. Integration with /track:full-cycle

### Comparison Table

| Aspect | /track:full-cycle | /track:auto |
|--------|------------------|-------------|
| Scope | Single task | Entire track (all tasks sequentially) |
| Phases | 2 (implement + review) | 6 per task |
| State persistence | None (in-memory only) | auto-state.json (cross-session) |
| Escalation | Blocks conversation | Persists state + exits cleanly |
| Human interaction | Synchronous (waits) | Asynchronous (exit + resume) |
| Type design | Mixed with implementation | Dedicated phase with separate review |
| Commit granularity | One commit per task | One commit per task (same) |
| SSoT | metadata.json | metadata.json (unchanged) |

### Shared Infrastructure (no changes needed)

- `metadata.json` SSoT via `sotp track` CLI
- `sotp track transition` for task state transitions
- `cargo make ci` / `ci-rust` for quality gates
- `cargo make track-local-review` for reviewer invocation
- Agent Teams for parallel execution
- `cargo make track-commit-message` / `cargo make track-note` for guarded commit

### New Components for /track:auto

| Component | Purpose | Location |
|-----------|---------|----------|
| `auto-state.json` | Cross-session state persistence (ephemeral, not git-tracked) | `track/items/<id>/auto-state.json` |
| `auto-mode-config.json` | Phase configuration | `.claude/auto-mode-config.json` |
| `AutoPhase` enum | Phase state machine | `libs/domain/src/auto_phase.rs` (**already implemented**) |
| Phase orchestration | 6-phase loop | Skill definition (`.claude/skills/`) |
| Escalation UI | Human intervention | Skill definition |

### Migration Strategy

- Phase 1 (Coexistence): `/track:auto` is new, independent. `/track:full-cycle` remains unchanged.
- Phase 2 (Feature Parity): `/track:auto --task T001` achieves single-task parity with `/track:full-cycle`.
- Phase 3 (Deprecation): `/track:full-cycle` skill updated to recommend `/track:auto --task`.
- Phase 4 (Removal, optional): `/track:full-cycle` skill removed.
