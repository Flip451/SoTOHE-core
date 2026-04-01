# auto-state.json Schema Design

> Ephemeral session state for `/track:auto` runs.
> Placed at `track/items/<id>/auto-state.json`. Not git-tracked (add to `.gitignore`).
> This file is ephemeral session state — created on `/track:auto` start, deleted on
> successful completion or `--abort`. It is not SSoT for task status (metadata.json remains SSoT).

## JSON Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "AutoModeState",
  "type": "object",
  "required": ["version", "run_id", "track_id", "task_id", "phase", "round", "started_at", "updated_at"],
  "properties": {
    "version": { "type": "integer", "const": 1 },
    "run_id": { "type": "string", "format": "uuid", "description": "Unique ID for this auto-mode run, used for --resume" },
    "track_id": { "type": "string", "pattern": "^[a-z0-9][a-z0-9-]*[a-z0-9]$", "description": "Track ID being processed (validated as TrackId in domain layer)" },
    "task_id": { "type": "string", "pattern": "^T\\d+$", "description": "Current task being processed" },
    "phase": {
      "type": "string",
      "enum": ["plan", "plan_review", "type_design", "type_review", "implement", "code_review", "escalated", "committed"]
    },
    "round": { "type": "integer", "minimum": 1, "description": "Current round number (resets per task, increments on rollback)" },
    "started_at": { "type": "string", "format": "date-time" },
    "updated_at": { "type": "string", "format": "date-time" },
    "escalation": {
      "type": ["object", "null"],
      "description": "Required (non-null) when phase is 'escalated'. Must be null when phase is not 'escalated'.",
      "properties": {
        "reason": { "type": "string" },
        "resume_phase": {
          "type": "string",
          "enum": ["plan", "plan_review", "type_design", "type_review", "implement", "code_review"],
          "description": "The phase to return to on --resume. Records which phase triggered escalation."
        },
        "context_summary": { "type": "string", "description": "Brief summary of what was happening when escalation occurred" },
        "options": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Suggested choices for the human"
        },
        "pending_artifacts": {
          "type": "array",
          "items": { "type": "string" },
          "description": "File paths of in-progress artifacts"
        },
        "escalated_at": { "type": "string", "format": "date-time" }
      },
      "required": ["reason", "resume_phase", "context_summary", "options", "pending_artifacts", "escalated_at"]
    },
    "decision": {
      "type": ["object", "null"],
      "properties": {
        "text": { "type": "string", "description": "Human's decision text" },
        "decided_at": { "type": "string", "format": "date-time" }
      },
      "required": ["text", "decided_at"],
      "description": "Populated when resuming from escalation"
    },
    "completed_tasks": {
      "type": "array",
      "items": { "type": "string", "pattern": "^T\\d+$" },
      "description": "Task IDs completed in this auto-mode run"
    }
  },
  "if": { "properties": { "phase": { "const": "escalated" } } },
  "then": { "required": ["escalation"], "properties": { "escalation": { "type": "object" } } },
  "else": { "required": ["escalation"], "properties": { "escalation": { "const": null } } }
}
```

## Example

```json
{
  "version": 1,
  "run_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "track_id": "auto-mode-design-2026-03-16",
  "task_id": "T003",
  "phase": "escalated",
  "round": 2,
  "started_at": "2026-03-16T10:00:00Z",
  "updated_at": "2026-03-16T11:30:00Z",
  "escalation": {
    "reason": "Type reviewer found conflicting trait bounds that require architectural decision",
    "resume_phase": "type_review",
    "context_summary": "TypeReview flagged that AutoPhaseRunner trait requires Send+Sync but the escalation callback closure captures non-Send state",
    "options": [
      "Make escalation callback async and require Send+Sync",
      "Use a channel-based escalation instead of callbacks",
      "Remove Send+Sync requirement from AutoPhaseRunner"
    ],
    "pending_artifacts": [
      "libs/domain/src/auto_phase.rs",
      "libs/usecase/src/auto_runner.rs"
    ],
    "escalated_at": "2026-03-16T11:30:00Z"
  },
  "decision": null,
  "completed_tasks": ["T001", "T002"]
}
```

## Serialization Strategy

The `AutoPhase` enum uses `snake_case` string representation for JSON serialization,
matching the `Display` impl in `auto_phase.rs` and the JSON Schema `enum` values above.

When implementing serde support, use `#[serde(rename_all = "snake_case")]` on `AutoPhase`
to ensure PascalCase Rust variants map to the lowercase JSON values:
- `Plan` → `"plan"`, `PlanReview` → `"plan_review"`, `TypeDesign` → `"type_design"`, etc.

## Rust Type Definitions (Design Reference)

```rust
/// Persistent state for an auto-mode run.
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
pub enum ActivePhase {
    Plan,
    PlanReview,
    TypeDesign,
    TypeReview,
    Implement,
    CodeReview,
}

/// Information recorded when escalating to human.
pub struct EscalationInfo {
    pub reason: String,
    pub resume_phase: ActivePhase,
    pub context_summary: String,
    pub options: Vec<String>,
    pub pending_artifacts: Vec<String>,
    pub escalated_at: chrono::DateTime<chrono::Utc>,
}

/// Human decision recorded on resume.
pub struct HumanDecision {
    pub text: String,
    pub decided_at: chrono::DateTime<chrono::Utc>,
}
```
