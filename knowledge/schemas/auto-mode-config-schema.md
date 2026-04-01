# auto-mode-config.json Schema Design

> Configuration for `/track:auto` phase-to-capability mapping and operational parameters.
> Placed at `.claude/auto-mode-config.json`. The capability names reference `agent-profiles.json` — actual provider resolution is delegated there.

## JSON Schema

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
          "description": "Per-phase time budget in seconds. Escalates when exceeded if escalation_on_timeout is true. Null means no timeout."
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

## Example

```json
{
  "version": 1,
  "phases": {
    "plan": {
      "capability": "planner",
      "max_rounds": 3,
      "timeout_seconds": 300,
      "description": "Task-level implementation planning"
    },
    "plan_review": {
      "capability": "reviewer",
      "max_rounds": 5,
      "timeout_seconds": 300,
      "description": "Review implementation plan for completeness and feasibility"
    },
    "type_design": {
      "capability": "planner",
      "max_rounds": 3,
      "timeout_seconds": 300,
      "description": "Design trait/struct/enum signatures"
    },
    "type_review": {
      "capability": "reviewer",
      "max_rounds": 5,
      "timeout_seconds": 300,
      "description": "Review type definitions for API ergonomics and correctness"
    },
    "implement": {
      "capability": "implementer",
      "max_rounds": 5,
      "timeout_seconds": 600,
      "description": "TDD implementation: Red → Green → Refactor"
    },
    "code_review": {
      "capability": "reviewer",
      "max_rounds": 5,
      "timeout_seconds": 300,
      "description": "Code review for correctness, performance, idiomatic Rust"
    }
  },
  "escalation_policy": {
    "max_consecutive_rollbacks": 3,
    "escalation_on_timeout": true
  },
  "settings": {
    "auto_commit": true,
    "verify_before_advance": true
  }
}
```

## Rust Type Definitions (Design Reference)

```rust
/// Top-level auto-mode configuration.
pub struct AutoModeConfig {
    pub version: u32,
    pub phases: PhaseMap,
    pub escalation_policy: EscalationPolicy,
    pub settings: AutoModeSettings,
}

/// Capability identifiers from agent-profiles.json.
/// Closed set to catch typos at deserialization time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Planner,
    Reviewer,
    Implementer,
    Researcher,
    Debugger,
}

/// Per-phase configuration.
pub struct PhaseConfig {
    pub capability: Capability,
    pub max_rounds: u32,
    pub timeout_seconds: Option<u32>,
    pub description: Option<String>,
}

/// Map of phase name to phase config.
pub struct PhaseMap {
    pub plan: PhaseConfig,
    pub plan_review: PhaseConfig,
    pub type_design: PhaseConfig,
    pub type_review: PhaseConfig,
    pub implement: PhaseConfig,
    pub code_review: PhaseConfig,
}

/// Global escalation policy.
pub struct EscalationPolicy {
    pub max_consecutive_rollbacks: u32,
    pub escalation_on_timeout: bool,
}

/// Global auto-mode settings.
pub struct AutoModeSettings {
    pub auto_commit: bool,
    pub verify_before_advance: bool,
}
```
