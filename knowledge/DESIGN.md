# Project Design Document

> This document tracks architecture decisions made during development.
> Updated by `/track:plan` workflow and specialist capability consultations.
> Track-facing docs (`spec.md`, `plan.md`, `verification.md`) stay in Japanese, but this design document stays in English for cross-provider compatibility.
> Diagrams in this document and in `plan.md` use Mermaid `flowchart TD`; do not use ASCII box art.

## Overview

SoTOHE-core is a CLI tool for managing specification-driven development workflows.
It implements a track state machine where task states drive track status derivation,
following DMMF (Domain Modeling Made Functional) principles to make illegal states
unrepresentable at the type level.

## Architecture

```mermaid
flowchart TD
    CLI[apps/cli<br>Composition Root / main.rs] --> Usecase[libs/usecase<br>Application Services + App Ports]
    CLI --> Infra[libs/infrastructure<br>Infrastructure Adapters]
    CLI --> Domain[libs/domain<br>Domain Model / Domain Ports]
    Usecase --> Domain
    Infra --> Domain
    Infra --> Usecase
```

## Module Structure

| Crate/Module | Role | Key Types |
|--------------|------|-----------|
| `domain` | Domain logic, Ports | `TrackId`, `TaskId`, `CommitHash`, `TrackMetadata`, `TrackTask`, `TaskStatus`, `TaskTransition`, `TrackStatus`, `StatusOverride`, `PlanView`, `PlanSection`, `TrackRepository`, `ConfidenceSignal`, `SignalBasis`, `SignalCounts`, `CoverageResult` |
| `domain::guard` | Shell command guard (pure computation, ports) | `Decision`, `GuardVerdict`, `ParseError`, `SimpleCommand`, `ShellParser` trait, `tokenize()`, `check_commands()` |
| `usecase` | Application services | `SaveTrackUseCase`, `LoadTrackUseCase`, `TransitionTaskUseCase` |
| `infrastructure` | Infrastructure adapters | `InMemoryTrackRepository`, `FsTrackStore`, `ConchShellParser` |
| `cli` | Composition Root | `main()` |

## Agent Roles

| Agent / Capability | Default Provider | Role |
|-------|-------|------|
| `orchestrator` | Claude Code | Overall orchestration, user interaction |
| `planner` | Claude Code (Opus) | Architecture design, implementation planning |
| `designer` | Claude Code | Domain type design (TDDD workflow) |
| `implementer` | Claude Code | Rust implementation |
| `reviewer` | Codex CLI | Code review, correctness analysis |
| `researcher` | Gemini CLI | Crate research, codebase analysis, external research |

Note: See `.harness/config/agent-profiles.json` for which provider handles each capability.

## Key Design Decisions

| Decision | ADR | Date |
|----------|-----|------|
| TrackStatus derived from tasks, not stored | [ADR](adr/2026-03-11-0000-track-status-derived.md) | 2026-03-11 |
| TaskStatus::Done owns Option\<CommitHash\> | [ADR](adr/2026-03-11-0010-done-owns-commit-hash.md) | 2026-03-11 |
| TaskTransition as explicit enum commands | [ADR](adr/2026-03-11-0020-task-transition-enum.md) | 2026-03-11 |
| StatusOverride auto-clears on all-resolved | [ADR](adr/2026-03-11-0030-status-override-auto-clear.md) | 2026-03-11 |
| Plan-task referential integrity at construction | [ADR](adr/2026-03-11-0040-plan-task-integrity.md) | 2026-03-11 |
| Fail-closed hook error handling | [ADR](adr/2026-03-11-0050-fail-closed-hooks.md) | 2026-03-11 |
| ~~Shell guard in domain layer (no trait)~~ | [ADR (superseded)](adr/2026-03-11-0060-shell-guard-in-domain.md) | 2026-03-11 |
| INF-20: ShellParser port + ConchShellParser adapter | [ADR](adr/2026-03-23-1000-shell-parser-port.md) | 2026-03-23 |
| conch-parser for shell AST (vendored, patched) | [ADR](adr/2026-03-11-0070-conch-parser-selection.md) | 2026-03-11 |
| Guard policy: ban edge-case-producing patterns | [ADR](adr/2026-03-11-0080-guard-policy-ban-patterns.md) | 2026-03-11 |
| Reviewer model_profiles in agent-profiles.json | [ADR](adr/2026-03-17-0000-reviewer-model-profiles.md) | 2026-03-17 |
| TSUMIKI-01/SPEC-05: 3-level signals with SignalBasis | [ADR](adr/2026-03-23-1010-three-level-signals.md) | 2026-03-23 |
| Two-stage signal architecture | [ADR](adr/2026-03-23-1020-two-stage-signals.md) | 2026-03-23 |
| CC-SDD-02: SpecStatus enum + content hash auto-demotion | — | 2026-03-24 |

## Crate Selection

| Crate | Version | Role | Notes |
|-------|---------|------|-------|
| thiserror | 2.x | Error derive macros | Domain layer only external dep |
| sha2 | 0.10.x | Content hash for spec approval | Infrastructure layer only |

## Canonical Blocks

```text
libs/domain/src/
├── lib.rs
├── error.rs
├── ids.rs
├── plan.rs
├── repository.rs
└── track.rs
```

```rust
// ids.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(String);

impl TrackId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError>;
    pub fn as_str(&self) -> &str;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError>;
    pub fn as_str(&self) -> &str;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitHash(String);

impl CommitHash {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError>;
    pub fn as_str(&self) -> &str;
}

// plan.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSection {
    id: String,
    title: String,
    description: Vec<String>,
    task_ids: Vec<TaskId>,
}

impl PlanSection {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Vec<String>,
        task_ids: Vec<TaskId>,
    ) -> Result<Self, ValidationError>;
    pub fn id(&self) -> &str;
    pub fn title(&self) -> &str;
    pub fn description(&self) -> &[String];
    pub fn task_ids(&self) -> &[TaskId];
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanView {
    summary: Vec<String>,
    sections: Vec<PlanSection>,
}

impl PlanView {
    pub fn new(summary: Vec<String>, sections: Vec<PlanSection>) -> Self;
    pub fn summary(&self) -> &[String];
    pub fn sections(&self) -> &[PlanSection];
}
```

```rust
// error.rs
#[derive(Debug, Error)]
pub enum DomainError {
    Validation(#[from] ValidationError),
    Transition(#[from] TransitionError),
    Repository(#[from] RepositoryError),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    InvalidTrackId(String),
    InvalidTaskId(String),
    InvalidCommitHash(String),
    EmptyTrackTitle,
    EmptyTaskDescription,
    EmptyPlanSectionId,
    EmptyPlanSectionTitle,
    DuplicateTaskId(String),
    DuplicatePlanSectionId(String),
    UnknownTaskReference(String),
    DuplicateTaskReference(String),
    UnreferencedTask(String),
    OverrideIncompatibleWithResolvedTasks(TrackStatus),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransitionError {
    TaskNotFound { task_id: String },
    InvalidTaskTransition {
        task_id: String,
        from: TaskStatusKind,
        to: TaskStatusKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    TrackNotFound(String),
    Message(String),
}

// repository.rs
pub trait TrackRepository: Send + Sync {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, RepositoryError>;
    fn save(&self, track: &TrackMetadata) -> Result<(), RepositoryError>;
}
```

```rust
// track.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackStatus {
    Planned,
    InProgress,
    Done,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatusKind {
    Todo,
    InProgress,
    Done,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done { commit_hash: Option<CommitHash> },
    Skipped,
}

impl TaskStatus {
    pub fn kind(&self) -> TaskStatusKind;
    pub fn is_resolved(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTransition {
    Start,
    Complete { commit_hash: Option<CommitHash> },
    ResetToTodo,
    Skip,
    Reopen,
}

impl TaskTransition {
    pub fn target_kind(&self) -> TaskStatusKind;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusOverride {
    Blocked { reason: String },
    Cancelled { reason: String },
}

impl StatusOverride {
    pub fn blocked(reason: impl Into<String>) -> Self;
    pub fn cancelled(reason: impl Into<String>) -> Self;
    pub fn reason(&self) -> &str;
    pub fn track_status(&self) -> TrackStatus;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackTask {
    id: TaskId,
    description: String,
    status: TaskStatus,
}

impl TrackTask {
    pub fn new(id: TaskId, description: impl Into<String>) -> Result<Self, ValidationError>;
    pub fn with_status(
        id: TaskId,
        description: impl Into<String>,
        status: TaskStatus,
    ) -> Result<Self, ValidationError>;
    pub fn id(&self) -> &TaskId;
    pub fn description(&self) -> &str;
    pub fn status(&self) -> &TaskStatus;
    pub fn transition(&mut self, transition: TaskTransition) -> Result<(), TransitionError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMetadata {
    id: TrackId,
    title: String,
    tasks: Vec<TrackTask>,
    plan: PlanView,
    status_override: Option<StatusOverride>,
}

impl TrackMetadata {
    pub fn new(
        id: TrackId,
        title: impl Into<String>,
        tasks: Vec<TrackTask>,
        plan: PlanView,
        status_override: Option<StatusOverride>,
    ) -> Result<Self, DomainError>;
    pub fn id(&self) -> &TrackId;
    pub fn title(&self) -> &str;
    pub fn tasks(&self) -> &[TrackTask];
    pub fn plan(&self) -> &PlanView;
    pub fn status_override(&self) -> Option<&StatusOverride>;
    pub fn status(&self) -> TrackStatus;
    pub fn set_status_override(
        &mut self,
        status_override: Option<StatusOverride>,
    ) -> Result<(), DomainError>;
    pub fn transition_task(
        &mut self,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<(), DomainError>;
    pub fn next_open_task(&self) -> Option<&TrackTask>;
}
```

```rust
// Canonical transition matrix from track_state_machine.py
match (&self.status, transition) {
    (TaskStatus::Todo, TaskTransition::Start) => TaskStatus::InProgress,
    (TaskStatus::Todo, TaskTransition::Skip) => TaskStatus::Skipped,
    (TaskStatus::InProgress, TaskTransition::Complete { commit_hash }) => {
        TaskStatus::Done { commit_hash }
    }
    (TaskStatus::InProgress, TaskTransition::ResetToTodo) => TaskStatus::Todo,
    (TaskStatus::InProgress, TaskTransition::Skip) => TaskStatus::Skipped,
    (TaskStatus::Done { .. }, TaskTransition::Reopen) => TaskStatus::InProgress,
    (TaskStatus::Skipped, TaskTransition::ResetToTodo) => TaskStatus::Todo,
    (_, transition) => {
        return Err(TransitionError::InvalidTaskTransition {
            task_id: self.id.to_string(),
            from: self.status.kind(),
            to: transition.target_kind(),
        });
    }
}
```

### Shell Command Guard (guard-cli-2026-03-11) — *will be superseded by INF-20*

> **Note**: The module tree and signatures below reflect the pre-INF-20 layout.
> INF-20 splits `parser.rs` into `types.rs` / `port.rs` / `text.rs`, moves
> conch-parser code to `infrastructure::shell`, and changes `policy::check(input)`
> to `policy::check_commands(&[SimpleCommand])`. Update this section during T6.

```text
libs/domain/src/guard/
├── mod.rs          # re-exports
├── verdict.rs      # Decision, GuardVerdict, ParseError
├── parser.rs       # split_shell() — shell command splitter
└── policy.rs       # check() — guard policy rules

apps/cli/src/commands/
├── mod.rs          # (existing, add guard module)
└── guard.rs        # guard check subcommand
```

```rust
// domain/src/guard/verdict.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardVerdict {
    pub decision: Decision,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("nesting depth exceeded maximum of {max}")]
    NestingDepthExceeded { max: usize },
    #[error("unmatched quote in command")]
    UnmatchedQuote,
}
```

```rust
// domain/src/guard/parser.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    pub argv: Vec<String>,
}

/// Splits a shell command string into individual simple commands.
///
/// # Errors
/// Returns `ParseError` on nesting depth exceeded or unmatched quotes.
pub fn split_shell(input: &str) -> Result<Vec<SimpleCommand>, ParseError>;
```

```rust
// domain/src/guard/policy.rs
/// Checks a shell command against the guard policy.
/// On parse failure, returns Block (fail-closed).
pub fn check(input: &str) -> GuardVerdict;
```

```rust
// CLI subcommand (apps/cli/src/commands/guard.rs)
#[derive(Debug, clap::Subcommand)]
pub enum GuardCommand {
    Check {
        #[arg(long)]
        command: String,
    },
}
```

```mermaid
flowchart TD
    A[Agent calls Bash tool] --> B[PreToolUse Hook fires]
    B --> C[Shell hook: extract command from JSON stdin]
    C --> D[sotp guard check\n--command 'git add .']
    D --> E{Rust CLI: parse & check}
    E -->|split_shell| F[Split by control operators]
    F --> G[For each SimpleCommand]
    G --> H[Skip env/launcher prefixes]
    H --> I{Effective command?}
    I -->|git| J[Extract git subcommand]
    J --> K{Protected subcommand?}
    K -->|add/commit/push| L[Block verdict]
    K -->|other| M[Allow verdict]
    I -->|non-git with 'git' in argv| L2[Block: git reference in args]
    I -->|other| M
    L2 --> O
    L --> O[JSON stdout + exit 1]
    M --> P[JSON stdout + exit 0]
    O --> Q[Hook: exit 2 — block tool]
    P --> R[Hook: exit 0 — allow tool]
```

## Security Hardening: Rust Migration

### Strategy: Rust for Security and Advisory Hooks (Python hook files fully removed by RV2-17)

Security hooks (`block-direct-git-ops`, `block-test-file-deletion`) and the advisory hook
(`skill-compliance`) are dispatched directly via `sotp hook dispatch` shell commands in
`.claude/settings.json`. Security hooks are fail-closed (exit 2 on error); the advisory hook
is fail-open (exit 0 on error, so a missing sotp binary never blocks the user). The deprecated
Python launcher files were removed in two phases: STRAT-03 Phase 1 (security launchers) and
RV2-17 (all remaining advisory hooks). There are no Python hook files under `.claude/hooks/`
anymore — the directory itself is gone. Other hooks (`TeammateIdle`, `PreCompact`,
`PermissionRequest`) use inline shell commands or prompt strings defined directly in
`.claude/settings.json` and are not Rust-dispatched.

| Layer | Implementation |
|-------|----------------|
| Security hooks (`block-direct-git-ops`, `block-test-file-deletion`) | `sotp hook dispatch` (Rust, fail-closed: exit 2 on error) |
| Advisory hook (`skill-compliance`) | `sotp hook dispatch` (Rust, fail-open: exit 0 on error) |
| Orchestration hooks (`TeammateIdle`, `PreCompact`, `PermissionRequest`) | Inline shell/prompt in `.claude/settings.json` |
| Track state I/O | `metadata.json` read-modify-write |
| File writes | Atomic write for critical data |

### SOLID Design Principles Applied

| Principle | Decision |
|-----------|----------|
| SRP | `Decision` shared across subdomains; `TrackReader`/`TrackWriter` split; UseCase owns only business logic |
| OCP | `HookHandler` trait for extensible hook dispatch without modifying existing code |
| LSP | `TrackReader`/`TrackWriter` implementations interchangeable (InMemory, Fs) |
| ISP | Read-only consumers depend only on `TrackReader`; mutation consumers on `TrackWriter` |
| DIP | Domain defines ports; `clap` confined to CLI layer; infrastructure adapters confined to Infrastructure layer |

### New Module Structure

```text
libs/domain/src/
├── decision.rs          # Decision enum (shared across guard, hook)
├── guard/
│   ├── verdict.rs       # GuardVerdict (uses Decision)
│   ├── parser.rs        # split_shell()
│   └── policy.rs        # check()
├── hook/
│   ├── mod.rs           # re-exports
│   ├── types.rs         # HookName, HookContext, HookInput (framework-free)
│   ├── verdict.rs       # HookVerdict (uses Decision)
│   └── error.rs         # HookError
├── error.rs             # TrackReadError, TrackWriteError (typed port errors)
├── repository.rs        # TrackReader + TrackWriter ports (replaces TrackRepository)
└── ...

libs/usecase/src/
├── hook.rs              # HookHandler trait + dispatch logic
└── ...

libs/infrastructure/src/
├── track/
│   ├── mod.rs           # re-exports
│   ├── codec.rs         # TrackDocumentV2 serde types (metadata.json schema)
│   ├── fs_store.rs      # FsTrackStore: TrackReader + TrackWriter (atomic write)
│   └── atomic_write.rs  # atomic_write_file() utility
└── ...

apps/cli/src/commands/
├── hook.rs              # HookCommand + HookName clap::ValueEnum impl
└── ...
```

### Canonical Blocks

```rust
// domain/src/decision.rs — shared binary policy outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Block,
}
```

```rust
// domain/src/hook/types.rs — framework-free, NO serde/serde_json dependency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName {
    BlockDirectGitOps,
    BlockTestFileDeletion,
}

/// Context for hook execution. Built by the CLI layer from:
/// - `project_dir`: `$CLAUDE_PROJECT_DIR` env var (set by Claude Code)
#[derive(Debug, Clone)]
pub struct HookContext {
    pub project_dir: Option<std::path::PathBuf>,
}

/// Framework-free hook input extracted from Claude Code hook JSON.
/// Parsing from HookEnvelope (serde) happens in the CLI/infrastructure layer (DIP).
#[derive(Debug, Clone)]
pub struct HookInput {
    pub tool_name: String,
    pub command: Option<String>,
    pub file_path: Option<std::path::PathBuf>,
}
```

```rust
// apps/cli/src/hook_envelope.rs (or infrastructure layer) — serde types, NOT in domain
//
// HookEnvelope lives outside domain to keep serde/serde_json out of the domain layer.
// Security-critical fields (tool_name) must NOT use #[serde(default)] — parse failure
// is caught at the CLI boundary. For PreToolUse hooks this results in exit 2 (block,
// fail-closed). For PostToolUse hooks, errors result in stderr warning +
// exit 0 (PostToolUse cannot block).

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HookEnvelope {
    pub tool_name: String,           // required — no #[serde(default)]
    #[serde(default)]
    pub tool_input: HookToolInput,
    // tool_response intentionally omitted — not needed for security hooks
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct HookToolInput {
    pub command: Option<String>,
    pub file_path: Option<std::path::PathBuf>,
}

impl From<HookEnvelope> for domain::hook::HookInput {
    fn from(env: HookEnvelope) -> Self {
        Self {
            tool_name: env.tool_name,
            command: env.tool_input.command,
            file_path: env.tool_input.file_path,
        }
    }
}
```

```rust
// domain/src/hook/verdict.rs
use crate::Decision;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookVerdict {
    pub decision: Decision,
    pub reason: Option<String>,
    pub additional_context: Option<String>,
}
```

```rust
// domain/src/hook/error.rs
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("invalid hook input: {0}")]
    Input(String),

    #[error(transparent)]
    Guard(#[from] crate::guard::ParseError),

    #[error("unsupported hook: {0:?}")]
    Unsupported(super::types::HookName),
}
```

```rust
// domain/src/error.rs — typed port errors (DIP: domain owns the error boundary)

/// Error type for TrackReader port operations.
#[derive(Debug, thiserror::Error)]
pub enum TrackReadError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// Error type for TrackWriter port operations.
/// Captures both domain validation failures (from mutation closures)
/// and infrastructure errors (I/O, codec).
///
/// NOTE: The `Domain` variant wraps `DomainError` which currently contains
/// a `Repository` variant. This creates an ambiguous path:
/// `TrackWriteError::Domain(DomainError::Repository(_))`.
///
/// Migration plan:
/// 1. Remove `DomainError::Repository` variant from `DomainError`.
/// 2. `DomainError` keeps only `Validation` and `Transition`.
/// 3. Repository errors flow exclusively through `TrackReadError::Repository`
///    and `TrackWriteError::Repository`.
/// 4. Use cases return `TrackReadError`/`TrackWriteError` directly,
///    not `DomainError` (see use case migration note below).
#[derive(Debug, thiserror::Error)]
pub enum TrackWriteError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
```

```rust
// domain/src/repository.rs — ISP: read/write separation with typed errors

/// Read-only port for track retrieval.
pub trait TrackReader: Send + Sync {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError>;
}

/// Atomic mutation port for track persistence.
/// Implementations provide locking internally.
///
/// NOTE: `update<F>` makes this trait non-object-safe (generic method).
/// This is acceptable — use cases depend on concrete types or generics,
/// not `dyn TrackWriter`. If dyn dispatch is needed in the future,
/// extract a non-generic sub-trait.
pub trait TrackWriter: Send + Sync {
    /// Persists a track (insert or update — upsert semantics).
    /// Matches the current `TrackRepository::save` contract for backward compatibility.
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError>;

    /// Atomically loads, mutates, and persists a track.
    ///
    /// # Errors
    /// - `TrackWriteError::Repository(TrackNotFound)` if the track does not exist.
    /// - `TrackWriteError::Repository(Message)` on I/O failure.
    /// - `TrackWriteError::Domain` propagated from the mutation closure.
    fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>;
}
```

```rust
// usecase/src/hook.rs — OCP: each hook implements this trait

/// Port for individual hook logic.
/// Receives framework-free HookInput (converted from HookEnvelope at CLI boundary).
///
/// ## Required Field Validation (fail-closed)
///
/// Each handler MUST validate hook-specific required fields from `HookInput`
/// and return `HookError::Input` if they are missing.
///
/// How the CLI maps `HookError::Input` depends on the hook event type:
/// - PreToolUse (guard): `HookError::Input` → exit 2 (block, fail-closed)
///
/// | Hook | Required fields | Missing → |
/// |------|----------------|-----------|
/// | `BlockDirectGitOps` | `tool_name` (always present), `command` | `HookError::Input("missing command")` |
/// | `BlockTestFileDeletion` | `tool_name` (always present), `file_path` | `HookError::Input("missing file_path")` |
///
/// Note: `tool_name` is guaranteed present (required in `HookEnvelope` serde).
/// `command` and `file_path` are `Option` in `HookInput` because different hooks
/// need different fields. The handler validates what it needs.
pub trait HookHandler: Send + Sync {
    fn handle(
        &self,
        ctx: &domain::hook::HookContext,
        input: &domain::hook::HookInput,
    ) -> Result<domain::hook::HookVerdict, domain::hook::HookError>;
}
```

```rust
// infrastructure/src/track/codec.rs — metadata.json serde types

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackDocumentV2 {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub tasks: Vec<TrackTaskDocument>,
    pub plan: PlanDocument,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_override: Option<TrackStatusOverrideDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackTaskDocument {
    pub id: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanDocument {
    pub summary: Vec<String>,
    pub sections: Vec<PlanSectionDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanSectionDocument {
    pub id: String,
    pub title: String,
    pub description: Vec<String>,
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackStatusOverrideDocument {
    pub status: String,
    pub reason: String,
}
```

```rust
// infrastructure/src/track/fs_store.rs

/// File-system backed TrackReader + TrackWriter.
/// Uses atomic_write_file for crash-safe persistence.
pub struct FsTrackStore {
    root: std::path::PathBuf,
}

impl FsTrackStore {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self;
}
```

```rust
// infrastructure/src/track/atomic_write.rs

/// Atomically writes content to a file using tmp-in-same-dir + fsync + rename + parent fsync.
///
/// # Errors
/// Returns `std::io::Error` on any I/O failure. Cleans up temp file on error.
pub fn atomic_write_file(path: &std::path::Path, content: &[u8]) -> std::io::Result<()>;
```

```rust
// apps/cli/src/commands/hook.rs

#[derive(Debug, clap::Subcommand)]
pub enum HookCommand {
    /// Dispatch a security-critical hook via Rust logic.
    /// Reads Claude Code hook JSON from stdin.
    /// Exit 0 = allow, exit 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit 2 (fail-closed).
    Dispatch {
        #[arg(value_enum)]
        hook: CliHookName,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookName {
    BlockDirectGitOps,
    BlockTestFileDeletion,
}

impl From<CliHookName> for domain::hook::HookName { ... }
```

```rust
// apps/cli/src/commands/hook.rs — stdout mapping for Claude Code hooks
//
// PreToolUse hooks (block-direct-git-ops, block-test-file-deletion):
//   Allow:  exit 0, stdout = "" (empty)
//   Block:  exit 2, stdout = plain text reason
//
// Error (PreToolUse):
//   exit 2, stdout = plain text. All PreToolUse errors are fail-closed.
//
// The Rust CLI follows the Claude Code hook protocol (exit 0 = allow, exit 2 = block).
```

### UseCase Return Type Migration

When use cases migrate from `TrackRepository` to `TrackReader`/`TrackWriter`:

```rust
// BEFORE (current): all use cases return DomainError
pub fn execute(&self, track: &TrackMetadata) -> Result<(), DomainError>;

// AFTER: use cases return the port error type matching their operation
impl<W: TrackWriter> SaveTrackUseCase<W> {
    /// Delegates to TrackWriter::save (upsert semantics preserved).
    pub fn execute(&self, track: &TrackMetadata) -> Result<(), TrackWriteError>;
}

impl<R: TrackReader> LoadTrackUseCase<R> {
    pub fn execute(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError>;
}

impl<W: TrackWriter> TransitionTaskUseCase<W> {
    /// Uses TrackWriter::update (atomic read-modify-write).
    /// TrackWriteError captures both DomainError (from closure) and RepositoryError.
    pub fn execute(
        &self,
        track_id: &TrackId,
        task_id: &TaskId,
        transition: TaskTransition,
    ) -> Result<TrackMetadata, TrackWriteError>;
}
```

The CLI layer (composition root) maps `TrackReadError`/`TrackWriteError` to user-facing error messages and exit codes. No `From<TrackReadError> for DomainError` conversion — the old `DomainError::Repository` path is removed.

### Migration Path per Track

| Track | Approach |
|-------|----------|
| 1. container-git-readonly | Docker only — no Rust changes |
| 2. hook-fail-closed | Implement `domain::hook`, `usecase::hook`, `cli::commands::hook`; security hooks dispatched directly via `sotp hook dispatch` shell commands (Python launchers removed in STRAT-03 Phase 1) |
| 3. track-persistence | Implement `infrastructure::track::{codec, fs_store, atomic_write}`; replace `TrackRepository` with `TrackReader`+`TrackWriter`; Python `track_state_machine.py` delegates to `sotp track` |
| 4. per-worker-target-dir | Docker/Makefile only — no Rust changes |
| 5. atomic-write-standard | Reuse `atomic_write_file` from track 3; apply to remaining Python scripts or migrate them |
| 6. security-control-tests | Rust integration tests for hooks, atomic writes |

### Direct Shell Hook Invocation (STRAT-03 Phase 1 Complete)

Security-critical hooks are now invoked directly via shell commands in `.claude/settings.json`,
eliminating the Python launcher layer entirely. Example:

```bash
"${SOTP_CLI_BINARY:-sotp}" hook dispatch block-direct-git-ops || exit 2
```

Key design decisions:

- **No Python intermediary**: `sotp` is called directly from the shell hook command. No subprocess overhead or Python runtime dependency.
- **Fail-closed**: `|| exit 2` ensures that if `sotp` is missing or fails, the hook blocks the operation.
- **Bootstrap guarantee**: `cargo make bootstrap` builds `sotp` before hooks can fire.
- **Claude Code hook protocol**: exit 0 = allow, exit 2 = block, exit 1 = non-blocking error (continues execution — NOT safe for security hooks).
- **PostToolUse**: errors are warnings only (exit 0), matching PostToolUse semantics.

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `TrackDocumentV2` schema drift vs Python `track_schema.py` | Incompatible metadata.json | Shared compatibility tests until Python writers removed |
| `TransitionTaskUseCase` still using `find/save` pattern | Data loss under contention | Migrate to `TrackWriter::update` in track 3 |
| Hook `tool_name` field missing from payload | JSON parse failure at CLI boundary | No `#[serde(default)]` on security fields. PreToolUse: parse error → exit 2 (block). |
| Hook-specific required fields (`command`, `file_path`) missing | Malformed payload bypasses control | Each `HookHandler` validates required fields → `HookError::Input` → PreToolUse: exit 2 (block) |
| Atomic rename cross-filesystem | Non-atomic write | Temp file in target directory; parent fsync mandatory |
| `TrackWriter::update<F>` is non-object-safe | Cannot use `dyn TrackWriter` | Acceptable: use cases use generics. Extract non-generic sub-trait if dyn needed later |
| `TrackStatus::Archived` missing in Rust domain | Python schema incompatibility | Add `Archived` variant to `TrackStatus` enum in domain layer |
| `DomainError::Repository` leaks into `TrackWriteError::Domain` | Ambiguous error path | Remove `DomainError::Repository` variant when migrating to `TrackReader`/`TrackWriter` |

## Feature Branch Strategy (branch-strategy-2026-03-12)

Full design with Canonical Blocks: `.claude/docs/research/planner-branch-strategy-2026-03-12.md`

### Canonical Blocks Reference

The following blocks are defined verbatim in the research artifact above (§ Canonical Blocks):

- **Branch naming convention**: `track/<track-id>`
- **`metadata.json` schema v3**: adds `branch` field binding track to feature branch
- **Python function signatures**: `current_git_branch()`, `find_track_by_branch()`, `resolve_track_dir()`, `latest_legacy_track_dir()`
- **Rust function signatures**: `allow_agent_git_operation()`, `is_protected_history_mutation()`
- **Mermaid flowchart**: branch lifecycle from `/track:plan` approval through PR merge

### Branch Lifecycle

```mermaid
flowchart TD
    A["/track:plan <feature> approved"] --> B["Create track/items/<id>/metadata.json"]
    B --> C["Create branch track/<id> from main"]
    C --> D["Switch workspace to track/<id>"]
    D --> E["Implement, review, /track:ci, /track:commit"]
    E --> F{"Track status done?"}
    F -->|No| E
    F -->|Yes| G["Push track/<id> and open PR to main"]
    G --> H["GitHub Actions runs on PR"]
    H --> I{"PR approved and green?"}
    I -->|No| E
    I -->|Yes| J["User merges PR to main"]
    J --> K["Update local main"]
    K --> L["Optional: archive track"]
    L --> M["Optional: delete merged branch"]
```

## Open Questions

_None at this time._

## Changelog

| Date | Changes |
|------|---------|
| 2026-03-11 | Initial design: DMMF track state machine domain model (Codex planner) |
| 2026-03-11 | Shell command guard: deterministic shell parsing + git operation blocking in domain layer |
| 2026-03-11 | Security hardening: Python-to-Rust hybrid migration design (SOLID, Codex planner) |
| 2026-03-11 | Codex review cycles R1-R10: typed port errors, HookInput DIP, exit code 2, required field validation, DomainError::Repository separation, UseCase return types, hook output JSON mapping |
| 2026-03-12 | Feature branch strategy: per-track branches, branch-aware resolution, guard policy extension (Codex planner) |
| 2026-03-16 | Auto mode design spike (MEMO-15): 6-phase state machine, auto-state.json persistence, escalation UI |
| 2026-03-23 | INF-20: ShellParser port in domain, ConchShellParser adapter in infrastructure, policy parse/evaluate split |
| 2026-03-23 | TSUMIKI-01/SPEC-05: Signal evaluation design — 3-level ConfidenceSignal + SignalBasis, two-stage architecture (Stage 1: spec signals in frontmatter, Stage 2: domain state signals in metadata.json) |

## Auto Mode (MEMO-15 Design Spike)

`/track:auto` provides autonomous track execution with a 6-phase cycle per commit unit
and human escalation for design decisions.

### Phase State Machine

```mermaid
flowchart TD
    Plan[Plan] --> PlanReview[PlanReview]
    PlanReview -->|zero_findings| TypeDesign[TypeDesign]
    PlanReview -->|P2+: design issue| Plan
    PlanReview -->|P1: fix in place| Plan
    TypeDesign --> TypeReview[TypeReview]
    TypeReview -->|zero_findings| Implement[Implement]
    TypeReview -->|P3: design issue| Plan
    TypeReview -->|P2: type change| TypeDesign
    TypeReview -->|P1: fix in place| TypeDesign
    Implement --> CodeReview[CodeReview]
    CodeReview -->|zero_findings| Committed[Committed]
    CodeReview -->|P3: design issue| Plan
    CodeReview -->|P2: type change| TypeDesign
    CodeReview -->|P1: impl fix| Implement
    Plan -.->|unresolvable| Escalated[Escalated]
    PlanReview -.->|unresolvable| Escalated
    TypeDesign -.->|unresolvable| Escalated
    TypeReview -.->|unresolvable| Escalated
    Implement -.->|unresolvable| Escalated
    CodeReview -.->|unresolvable| Escalated
    Escalated -->|"--resume (returns to phase that escalated)"| Plan
    Escalated -.->|or| TypeDesign
    Escalated -.->|or| Implement
    Escalated -.->|or| PlanReview
    Escalated -.->|or| TypeReview
    Escalated -.->|or| CodeReview
```

### Canonical Blocks

```rust
// auto_phase.rs — domain layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoPhase {
    Plan,
    PlanReview,
    TypeDesign,
    TypeReview,
    Implement,
    CodeReview,
    Escalated,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackTarget {
    Plan,
    TypeDesign,
    Implement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoPhaseTransition {
    Advance,
    Rollback(RollbackTarget),
    Escalate { reason: String },
    Resume { decision: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    P1, // Minor fix — rollback target is phase-specific (see rollback_target())
    P2, // Type-level issue — rollback target is phase-specific
    P3, // Design-level issue — always rolls back to Plan
}

// Phase-specific rollback rules:
// PlanReview:  P1/P2/P3 → Plan
// TypeReview:  P3 → Plan, P2/P1 → TypeDesign
// CodeReview:  P3 → Plan, P2 → TypeDesign, P1 → Implement
// See rollback_target(current_phase, severity) in auto_phase.rs

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AutoPhaseError {
    #[error("invalid auto-phase transition: {from} cannot {action}")]
    InvalidTransition { from: String, action: String },
    #[error("cannot resume: phase is '{phase}', not escalated")]
    NotEscalated { phase: String },
    #[error("rollback from '{from}' to '{to}' is not allowed")]
    InvalidRollback { from: String, to: String },
}
```

### Configuration

- **Phase config**: `.claude/auto-mode-config.json` — maps phases to capabilities from `agent-profiles.json`
- **Session state**: `track/items/<id>/auto-state.json` — ephemeral, not git-tracked; cross-session persistence for escalation/resume; deleted on completion/abort
- **Schema docs**: `.claude/docs/schemas/auto-state-schema.md`, `.claude/docs/schemas/auto-mode-config-schema.md`

### Design Docs

- Agent briefings: `.claude/docs/designs/auto-mode-agent-briefings.md`
- Escalation UI: `.claude/docs/designs/auto-mode-escalation-ui.md`
- Integration with /track:full-cycle: `.claude/docs/designs/auto-mode-integration.md`

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| auto-state.json is ephemeral session state, metadata.json remains SSoT | Prevents dual-SSoT conflict; auto-state references task IDs but never modifies task status directly |
| 6 phases with severity-based rollback | P1→Implement, P2→TypeDesign, P3→Plan; minimizes rework while ensuring design issues are caught early |
| Separate config file (.claude/auto-mode-config.json) | Decouples auto-mode parameters from agent-profiles.json; phase→capability mapping with delegated provider resolution |
| Escalation exits process cleanly (exit 1) | Conversation not blocked; state persisted for async human decision; --resume with decision text |
| Domain layer AutoPhase enum (no method bodies in spike) | Type signatures only; implementation deferred to follow-up track |

## Review Escalation Threshold (WF-36)

The review workflow needs a circuit breaker for repeated bug classes. The trigger is:
the same normalized concern appears in 3 consecutive closed review cycles. A "closed
review cycle" means one `round_type` + `round` after all `expected_groups` have
recorded that same round number. This avoids double-counting parallel reviewer groups
inside a single fix-review wave.

### Recommended Decisions

| Design question | Recommendation | Why |
|-----------------|----------------|-----|
| Category determination | Option C: Hybrid | Reviewer-provided `category` is the best semantic signal, but the system cannot depend on every reviewer emitting it correctly on day 1. File-path fallback keeps the feature backward compatible and deterministic. Final fallback is `other`. |
| State location | Option B: `ReviewEscalationState` composed into `ReviewState` | Escalation is review state, not a separate workflow aggregate. Keeping it under `review` preserves metadata.json as SSoT while isolating circuit-breaker concerns from the existing approval status enum. |
| Schema extension | Extend `review.groups.*.{fast,final}` with `concerns`, add `review.escalation` object, preserve last 10 closed cycles (FIFO trim on insert) | The latest per-group result must carry concerns so the domain can close a cycle without persisting full findings. 10 cycles is enough to audit streaks (3x threshold + margin) while bounding metadata.json growth. |
| State-machine interaction | Escalation is orthogonal to `ReviewStatus` and acts as a hard gate | The triggering round should still be recorded, then subsequent `record-round` and `check_commit_ready` calls must fail with `EscalationActive`. Resolving the block invalidates review and clears `code_hash` so the next review starts fresh. |
| Findings persistence | Option B: category list per closed cycle, not full findings history | Counts-only is too opaque for debugging and migration. Full findings history is heavy and duplicates reviewer artifacts. Concern history keeps metadata small while preserving the recurrence signal. |

### Category Normalization

1. If a reviewer finding includes `category`, normalize that slug and use it.
2. Otherwise, derive a concern from `file` using a stable repo-local rule such as:
   `libs/domain/src/review.rs -> domain.review`, `apps/cli/src/commands/review.rs -> cli.review`.
3. If neither is available, fall back to `other`.

The domain layer should only see validated `ReviewConcern` values. All schema parsing
and fallback logic stays in CLI/usecase/infra.

### State-Machine Notes

- Triggering escalation does not rewrite `ReviewStatus`; it transitions `escalation.phase` to `EscalationPhase::Blocked(ReviewEscalationBlock)`.
- `record_round`, `record_round_with_pending`, and `check_commit_ready` must reject while blocked.
- `resolve-escalation` is explicit. It requires references to a workspace search artifact and a reinvention-check artifact.
- Successful resolution clears streak state, stores the resolution record, sets `ReviewStatus::Invalidated`, and clears `code_hash`.

### Canonical Blocks

```rust
// libs/usecase/src/review_workflow.rs
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFinding {
    pub message: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub line: Option<u64>,
}
```

```rust
// libs/domain/src/review.rs
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewConcern(String);

impl ReviewConcern {
    pub fn new(value: impl Into<String>) -> Result<Self, ReviewError>;
    pub fn as_str(&self) -> &str;
}

// --- Algebraic data types: use enum variants with data to make illegal states
// unrepresentable. EscalationPhase::Blocked carries the block data directly,
// so there is no way to have status=Blocked with blocked=None or vice versa.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCycleSummary {
    round_type: RoundType,
    round: u32,
    timestamp: String,
    concerns: Vec<ReviewConcern>,
    groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewConcernStreak {
    consecutive_rounds: u8,
    last_round_type: RoundType,
    last_round: u32,
    last_seen_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationBlock {
    concerns: Vec<ReviewConcern>,
    blocked_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEscalationDecision {
    AdoptWorkspaceSolution,
    AdoptExternalCrate,
    ContinueSelfImplementation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationResolution {
    blocked_concerns: Vec<ReviewConcern>,
    workspace_search_ref: String,
    reinvention_check_ref: String,
    decision: ReviewEscalationDecision,
    summary: String,
    resolved_at: String,
}

/// Algebraic data type: escalation phase carries its associated data in each variant.
/// `Clear` has no block data. `Blocked` carries the block details.
/// This eliminates impossible states like "status=Blocked but blocked=None".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationPhase {
    Clear,
    Blocked(ReviewEscalationBlock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationState {
    threshold: u8,
    phase: EscalationPhase,
    recent_cycles: Vec<ReviewCycleSummary>,
    concern_streaks: BTreeMap<ReviewConcern, ReviewConcernStreak>,
    last_resolution: Option<ReviewEscalationResolution>,
}

// ReviewRoundResult gains a `concerns` field but preserves backward compatibility
// by keeping the existing 3-arg `new()` and adding `new_with_concerns()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRoundResult {
    round: u32,
    verdict: String,
    timestamp: String,
    concerns: Vec<ReviewConcern>,
}

impl ReviewRoundResult {
    /// Existing 3-arg constructor — backward compatible, empty concerns.
    pub fn new(
        round: u32,
        verdict: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self;
    /// New constructor with concerns for escalation tracking.
    pub fn new_with_concerns(
        round: u32,
        verdict: impl Into<String>,
        timestamp: impl Into<String>,
        concerns: Vec<ReviewConcern>,
    ) -> Self;
    pub fn round(&self) -> u32;
    pub fn verdict(&self) -> &str;
    pub fn timestamp(&self) -> &str;
    pub fn concerns(&self) -> &[ReviewConcern];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewState {
    status: ReviewStatus,
    code_hash: Option<String>,
    groups: HashMap<String, ReviewGroupState>,
    escalation: ReviewEscalationState,
}

// Note: thiserror is already a dependency of libs/domain (see domain/Cargo.toml).
// The #[derive(Error)] below does NOT violate the "std-only" constraint.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewError {
    #[error("final round requires review status fast_passed, but current status is {0}")]
    FinalRequiresFastPassed(ReviewStatus),

    #[error("code hash mismatch: review recorded against {expected}, but current code is {actual}")]
    StaleCodeHash { expected: String, actual: String },

    #[error("review status is {0}, not approved")]
    NotApproved(ReviewStatus),

    #[error("review escalation is active for concerns: {concerns:?}")]
    EscalationActive { concerns: Vec<String> },

    #[error("review escalation is not active")]
    EscalationNotActive,

    #[error("invalid review concern: {0}")]
    InvalidConcern(String),

    #[error("resolution evidence is required: {0}")]
    ResolutionEvidenceMissing(&'static str),

    #[error("resolution concerns do not match blocked concerns")]
    ResolutionConcernMismatch { expected: Vec<String>, actual: Vec<String> },
}

impl ReviewState {
    pub fn escalation(&self) -> &ReviewEscalationState;

    pub fn record_round(
        &mut self,
        round_type: RoundType,
        group: &str,
        result: ReviewRoundResult,
        expected_groups: &[String],
        current_code_hash: &str,
    ) -> Result<(), ReviewError>;

    pub fn record_round_with_pending(
        &mut self,
        round_type: RoundType,
        group: &str,
        result: ReviewRoundResult,
        expected_groups: &[String],
        pre_update_hash: &str,
    ) -> Result<(), ReviewError>;

    /// Checks if the review state is ready for commit.
    /// Returns `Err(EscalationActive)` if escalation is blocked.
    pub fn check_commit_ready(&mut self, current_code_hash: &str) -> Result<(), ReviewError>;

    pub fn resolve_escalation(
        &mut self,
        resolution: ReviewEscalationResolution,
    ) -> Result<(), ReviewError>;

    /// Extended constructor for codec deserialization — includes escalation state.
    pub fn with_fields(
        status: ReviewStatus,
        code_hash: Option<String>,
        groups: HashMap<String, ReviewGroupState>,
        escalation: ReviewEscalationState,
    ) -> Self;
}
```

```mermaid
flowchart TD
    NS[NotStarted / Invalidated] -->|fast zero across all expected groups| FP[FastPassed]
    NS -->|fast findings remain| NS
    FP -->|final zero across all expected groups| AP[Approved]
    FP -->|final findings remain| FP
    AP -->|stale code hash| IV[Invalidated]

    NS -. closed cycle reaches threshold .-> BL[Escalation Blocked]
    FP -. closed cycle reaches threshold .-> BL
    AP -. closed cycle reaches threshold .-> BL
    IV -. closed cycle reaches threshold .-> BL

    BL -->|record_round / record_round_with_pending| RJ1[Reject: EscalationActive]
    BL -->|check_commit_ready| RJ2[Reject: EscalationActive]
    BL -->|resolve_escalation| IV
```

The `phase` field uses an internally tagged union (`#[serde(tag = "type")]`) to preserve the ADT invariant in JSON:
- `"phase": {"type": "clear"}` — no block data
- `"phase": {"type": "blocked", "concerns": [...], "blocked_at": "..."}` — block data inline

Codec deserializes this into `EscalationPhase::Clear` or `EscalationPhase::Blocked(...)`.
The illegal state `type=clear + block data present` cannot be represented.

```json
{
  "review": {
    "status": "not_started",
    "code_hash": "315c9b21...",
    "groups": {
      "infra-domain": {
        "fast": {
          "round": 3,
          "timestamp": "2026-03-19T01:52:41Z",
          "verdict": "findings_remain",
          "concerns": ["typed_deserialization"]
        },
        "final": null
      },
      "other": {
        "fast": {
          "round": 3,
          "timestamp": "2026-03-19T01:53:10Z",
          "verdict": "zero_findings",
          "concerns": []
        },
        "final": null
      }
    },
    "escalation": {
      "threshold": 3,
      "phase": {
        "type": "blocked",
        "concerns": ["typed_deserialization"],
        "blocked_at": "2026-03-19T01:53:10Z"
      },
      "recent_cycles": [
        {
          "round_type": "fast",
          "round": 1,
          "timestamp": "2026-03-19T01:20:00Z",
          "groups": ["infra-domain", "other"],
          "concerns": ["typed_deserialization"]
        },
        {
          "round_type": "fast",
          "round": 2,
          "timestamp": "2026-03-19T01:36:00Z",
          "groups": ["infra-domain", "other"],
          "concerns": ["typed_deserialization"]
        },
        {
          "round_type": "fast",
          "round": 3,
          "timestamp": "2026-03-19T01:53:10Z",
          "groups": ["infra-domain", "other"],
          "concerns": ["typed_deserialization"]
        }
      ],
      "concern_streaks": {
        "typed_deserialization": {
          "consecutive_rounds": 3,
          "last_round_type": "fast",
          "last_round": 3,
          "last_seen_at": "2026-03-19T01:53:10Z"
        }
      },
      "last_resolution": null
    }
  }
}
```

## Spec Approval Gate (CC-SDD-02)

### Overview

`SpecDocument.status` was promoted from `String` to `SpecStatus` enum (`Draft` / `Approved`).
Two new optional fields enable explicit approval tracking with auto-demotion on content change.

### Domain Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecStatus {
    Draft,
    Approved,
}

// On SpecDocument:
pub fn approve(&mut self, timestamp: Timestamp, content_hash: String);
pub fn demote(&mut self);
pub fn is_approval_valid(&self, current_hash: &str) -> bool;
pub fn effective_status(&self, current_hash: &str) -> SpecStatus;
```

### Content Hash

SHA-256 of canonical JSON serialization of substantive fields:
`title`, `version`, `goal`, `scope`, `constraints`, `domain_states`, `acceptance_criteria`.

Excluded (changes do not invalidate approval):
`status`, `signals`, `domain_state_signals`, `additional_sections`, `related_conventions`,
`approved_at`, `content_hash`.

Computed in infrastructure layer (`spec::codec::compute_content_hash`) using `sha2` crate.
Domain layer stores hash as opaque `String`.

### Auto-Demotion

When the codec decodes a spec.json with `status: "approved"`, the caller can use
`effective_status(current_hash)` to detect content changes since approval.
If the hash does not match, the effective status is `Draft`.

### spec.json Schema (v1, backward compatible)

New optional fields:
```json
{
  "status": "approved",
  "approved_at": "2026-03-24T10:00:00Z",
  "content_hash": "sha256:abc123..."
}
```

### CLI

`sotp spec approve <track-dir>` — reads spec.json, computes content hash, sets
status=approved + approved_at + content_hash, writes back.

## Tamper-Proof Review Verdict (tamper-proof-review-2026-03-26)

Full design artifact: `knowledge/adr/2026-03-26-0010-tamper-proof-review-verdict.md`

### Threat Model

Claude Code (orchestrator) is untrusted. It can:
1. Inject fabricated verdicts via `sotp review record-round --verdict`
2. Write directly to `review.json` via Write/Edit tools
3. Bypass auto-record and use manual record-round path

The Rust CLI (`sotp`) is the trusted component, protected by the guard hook.

### Architecture

| Layer | Protection | Attack Vector |
|-------|-----------|---------------|
| CLI sealing | Remove `record-round` subcommand | Verdict injection |
| Hook guard | `BlockProtectedReviewStateWrite` PreToolUse hook | Direct file tampering |
| Provenance | `VerdictProvenance::TrustedSubprocess` with SHA-256 digests | Post-hoc tampering detection |
| Verification | `check-approved --require-provenance` | CI gate |

### Artifact Layout

```text
track/items/<id>/
  review.json                        # schema_version: 2, rounds have provenance
  review-artifacts/
    <invocation-id>/
      final-message.json             # canonical filtered payload
      session.log                    # captured reviewer stderr
      attestation.json               # sotp-generated attestation document
```

### Canonical Blocks

```rust
// domain/src/review/provenance.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    pub fn new(value: impl Into<String>) -> Result<Self, ReviewError> {
        let value = value.into();
        let is_hex = value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit());
        if !is_hex {
            return Err(ReviewError::InvalidProvenance(
                "sha256 digest must be 64 hex chars".to_owned(),
            ));
        }
        Ok(Self(value.to_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReviewInvocationId(NonEmptyString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDigest(Sha256Hex);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadDigest(Sha256Hex);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationDigest(Sha256Hex);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedArtifactRef {
    path: NonEmptyString,
    bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum ReviewerKind {
    CodexLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
/// Where the verdict JSON was extracted from.
/// - `OutputLastMessage`: Codex `--output-last-message` file (primary path)
/// - `SessionLogFallback`: session log (captured stderr + Codex diagnostic output),
///   scanned bottom-up for JSON verdict blocks via `extract_verdict_from_content`.
///   This is a fallback when the primary output-last-message file is empty.
pub enum VerdictSource {
    OutputLastMessage,
    SessionLogFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedSubprocessProvenance {
    invocation_id: ReviewInvocationId,
    reviewer: ReviewerKind,
    captured_at: Timestamp,
    source: VerdictSource,
    session_log: ProtectedArtifactRef,
    session_digest: SessionDigest,
    final_message: ProtectedArtifactRef,
    payload_digest: PayloadDigest,
    attestation: ProtectedArtifactRef,
    attestation_digest: AttestationDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictProvenance {
    LegacyUnverified,
    TrustedSubprocess(TrustedSubprocessProvenance),
}
```

```rust
// domain/src/review/types.rs — ReviewRoundResult gains provenance

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRoundResult {
    round: u32,
    verdict: Verdict,
    timestamp: crate::Timestamp,
    concerns: Vec<super::concern::ReviewConcern>,
    provenance: VerdictProvenance,
}
```

```rust
// usecase/src/review_workflow/usecases.rs — attested recording

pub struct AttestedReviewRound {
    pub round_type: RoundType,
    pub group_name: ReviewGroupName,
    pub expected_groups: Vec<ReviewGroupName>,
    pub verdict: Verdict,
    pub concerns: Vec<ReviewConcern>,
    pub timestamp: Timestamp,
    pub provenance: VerdictProvenance,
    pub final_message_json: Vec<u8>,
    pub session_log_bytes: Vec<u8>,
    pub attestation_json: Vec<u8>,
}

pub trait RecordRoundProtocol {
    /// Existing entrypoint — retained for backward compatibility during migration.
    /// Will be removed after all callers migrate to `execute_attested`.
    fn execute(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError>;

    /// New attested entrypoint — records verdict with provenance and evidence artifacts.
    /// Introduced additively; becomes the sole entrypoint after migration Phase 2.
    fn execute_attested(
        &self,
        track_id: &TrackId,
        round: AttestedReviewRound,
    ) -> Result<(), RecordRoundProtocolError>;
}
```

```rust
// usecase/src/review_workflow/usecases.rs — evidence verification

pub trait ReviewEvidenceVerifier {
    fn verify_round(
        &self,
        track_id: &TrackId,
        group: &ReviewGroupName,
        round_type: RoundType,
        round: &ReviewRoundResult,
    ) -> Result<ReviewEvidenceStatus, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewEvidenceStatus {
    Verified,
    LegacyUnverified,
    MissingArtifact { path: String },
    DigestMismatch { path: String, expected: String, actual: String },
    VerdictMismatch,
}
```

```rust
// infrastructure/src/review_store.rs — serde documents

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewArtifactRefDocument {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReviewRoundProvenanceDocument {
    LegacyUnverified,
    TrustedSubprocess {
        invocation_id: String,
        reviewer: String,
        captured_at: String,
        source: String,
        session_log: ReviewArtifactRefDocument,
        final_message: ReviewArtifactRefDocument,
        attestation: ReviewArtifactRefDocument,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewAttestationDocument {
    pub schema_version: u32,
    pub invocation_id: String,
    pub reviewer: String,
    pub model: String,
    pub track_id: String,
    pub round_type: String,
    pub group: String,
    pub expected_groups: Vec<String>,
    pub captured_at: String,
    pub source: String,
    pub review_hash: String,
    pub final_payload_sha256: String,
    pub session_log_sha256: String,
}
```

```rust
// apps/cli/src/commands/review/mod.rs — sealed CLI (RecordRound removed)

#[derive(Debug, clap::Subcommand)]
pub enum ReviewCommand {
    CodexLocal(CodexLocalArgs),
    CheckApproved(CheckApprovedArgs),
    ResolveEscalation(ResolveEscalationArgs),
    Status(StatusArgs),
}
```

```rust
// domain/src/hook/types.rs — new hook variant

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName {
    BlockDirectGitOps,
    BlockTestFileDeletion,
    BlockProtectedReviewStateWrite,
}
```

## Domain Types Registry (domain-types.json)

Domain type declarations are stored in a separate `domain-types.json` file per track,
independent from `spec.json`. This separation reflects different lifecycles: specs are
frozen after approval while type declarations evolve with implementation.

See ADR: `knowledge/adr/2026-04-07-0045-domain-types-separation.md`

### DomainTypeKind Categories

| Kind | Verification | Data |
|------|-------------|------|
| `typestate` | transition functions exist | `TypestateTransitions::Terminal` or `To(targets)` |
| `enum` | variant names match exactly | `expected_variants: Vec<String>` |
| `value_object` | type exists | (none) |
| `error_type` | expected variants covered | `expected_variants: Vec<String>` |
| `trait_port` | methods present | `expected_methods: Vec<String>` |

### CodeProfile (Pre-indexed Evaluation Interface)

`CodeProfile` is the domain-layer query interface for type evaluation.
Infrastructure builds it from `SchemaExport` via `build_code_profile()`.

| Type | Layer | Purpose |
|------|-------|---------|
| `CodeProfile` | domain | HashMap-indexed view: types + traits |
| `CodeType` | domain | kind + members + method_return_types (HashSet) |
| `CodeTrait` | domain | method_names |
| `build_code_profile()` | infrastructure | Transforms SchemaExport → CodeProfile |

The evaluation function `evaluate_domain_type_signals(entries, profile)` takes
`&CodeProfile` — no raw string parsing in the domain layer.

### Signal Rules (Blue/Red Binary)

Blue = spec and code fully match. Red = everything else.
Yellow is not used in domain type evaluation (Stage 2).
