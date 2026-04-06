# Planner Claude Migration — Design Review (Phase 2 Reference)

Date: 2026-04-07
Source: Codex planner (gpt-5.4) design review for Phase 2 hexagonal architecture
Context: Phase 1 is config/doc-only migration. This document captures Phase 2 design.

## Recommendation

Move planner execution out of the CLI-only wrapper into a real planner port in `usecase`,
add typed serde-free agent-profile domain types, and make `sotp plan auto` the only wrapper
entrypoint.

Current state: planner is CLI-bound and Codex-specific in `plan/mod.rs` and `codex_local.rs`,
while provider/model resolution is duplicated across Rust and Python in `agent_profiles.rs`,
`_agent_profiles.py`, and raw JSON walking in `pr_review.rs`.

## Canonical Blocks

```text
libs/domain/src/agent_profiles/
├── mod.rs
├── error.rs
└── types.rs

libs/usecase/src/planner/
├── mod.rs
├── error.rs
├── ports.rs
└── types.rs

libs/infrastructure/src/agent_profiles/
├── mod.rs
└── json_file.rs

libs/infrastructure/src/planner/
├── mod.rs
├── subprocess.rs
├── codex_planner.rs
└── claude_planner.rs

apps/cli/src/commands/plan/
├── mod.rs
├── auto.rs
├── codex_local.rs
├── claude_local.rs
└── tests.rs
```

```rust
// libs/domain/src/agent_profiles/types.rs
pub enum Capability {
    Planner,
    Reviewer,
    Researcher,
    Implementer,
    Debugger,
    Orchestrator,
    MultimodalReader,
}

pub enum ProviderName {
    Claude,
    Codex,
    Gemini,
}

pub struct AgentProfiles {
    active_profile: String,
    providers: std::collections::HashMap<ProviderName, ProviderDefinition>,
    profiles: std::collections::HashMap<String, CapabilityProfile>,
}

pub struct CapabilityProfile {
    routing: std::collections::HashMap<Capability, ProviderName>,
    provider_model_overrides: std::collections::HashMap<ProviderName, String>,
    workflow_host_provider: ProviderName,
    workflow_host_model: String,
}

pub struct ProviderDefinition {
    label: String,
    default_model: Option<String>,
    fast_model: Option<String>,
    nano_model: Option<String>,
    supported_capabilities: std::collections::BTreeSet<Capability>,
    model_profiles: std::collections::HashMap<String, ModelProfile>,
}

pub struct ModelProfile {
    full_auto: bool,
}

impl AgentProfiles {
    pub fn resolve_provider(&self, capability: Capability) -> Result<&ProviderName, AgentProfilesError>;
    pub fn resolve_provider_model(&self, provider: &ProviderName) -> Option<&str>;
}
```

```rust
// libs/usecase/src/planner/ports.rs
pub trait Planner {
    fn plan(&self, request: &PlanRequest) -> Result<PlanRunResult, PlannerError>;
}

pub trait AgentProfilesReader {
    fn load(&self) -> Result<domain::agent_profiles::AgentProfiles, PlannerError>;
}
```

```rust
// libs/usecase/src/planner/types.rs
pub enum PlanInput {
    BriefingFile(std::path::PathBuf),
    Prompt(String),
}

pub struct PlanRequest {
    pub input: PlanInput,
    pub timeout: std::time::Duration,
    pub model_override: Option<String>,
}

pub struct PlanRunResult {
    pub exit_code: u8,
    pub session_log_path: std::path::PathBuf,
}
```

## Dispatch Flow

```text
sotp plan auto
  -> AgentProfilesReader::load()
  -> profiles.resolve_provider(Capability::Planner)
  -> profiles.resolve_provider_model(resolved_provider)
  -> build Box<dyn Planner>
  -> planner.plan(request)
```

## Design Answers

1. `Planner` should live in `usecase`, not CLI (consistent with Reviewer pattern).
2. Use streaming passthrough plus exit-code result, not `Result<String, _>`.
3. Auto-dispatch should be `sotp plan auto` subcommand, then repoint `track-local-plan`.
4. Claude flags for v1: `claude --bare -p --max-turns 12 --permission-mode dontAsk --allowedTools Read,Grep,Glob`
5. Model resolution: `profiles.<active>.provider_model_overrides.<provider>` -> `providers.<provider>.default_model`
6. `ClaudePlanner` should support optional `--model`. Add `providers.claude.default_model`.

## Primary Risks

- Moving config file without shared resolver breaks Rust, hooks, PR flow, verifier checks.
  Hardcoded path in: `agent_profiles.rs`, `_agent_profiles.py`, `pr.rs`, `orchestra.rs`.
- Config parsing split between typed Rust, raw `serde_json::Value`, and Python creates
  inconsistent resolution.
- Keeping `track-local-plan` pinned to Codex defeats profile switch.
- Subprocess logic duplication: extract timeout/tee/process-group into `subprocess.rs`.

## Implementation Order

1. Add `domain::agent_profiles` with pure validation and resolution tests.
2. Add infrastructure JSON loader for `config/agent-profiles.json` with legacy fallback.
3. Add `usecase::planner` port/types and infrastructure `CodexPlanner`/`ClaudePlanner`.
4. Add `sotp plan auto`, repoint `track-local-plan`.
5. Update hooks/docs/verifier/tests, flip default planner to Claude.
