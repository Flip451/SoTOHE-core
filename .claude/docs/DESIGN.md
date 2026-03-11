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
    CLI[apps/cli<br>Composition Root / main.rs] --> Usecase[libs/usecase<br>Application Services]
    CLI --> Infra[libs/infrastructure<br>Infrastructure Adapters]
    CLI --> Domain[libs/domain<br>Domain Model / Ports]
    Usecase --> Domain
    Infra --> Domain
```

## Module Structure

| Crate/Module | Role | Key Types |
|--------------|------|-----------|
| `domain` | Domain logic, Ports | `TrackId`, `TaskId`, `CommitHash`, `TrackMetadata`, `TrackTask`, `TaskStatus`, `TaskTransition`, `TrackStatus`, `StatusOverride`, `PlanView`, `PlanSection`, `TrackRepository` |
| `domain::lock` | File lock domain types, Ports | `FilePath`, `AgentId`, `LockMode`, `LockEntry`, `FileGuard`, `LockError`, `FileLockManager` |
| `domain::guard` | Shell command guard (pure computation) | `Decision`, `GuardVerdict`, `ParseError`, `SimpleCommand`, `split_shell()`, `check()` |
| `usecase` | Application services | `SaveTrackUseCase`, `LoadTrackUseCase`, `TransitionTaskUseCase` |
| `infrastructure` | Infrastructure adapters | `InMemoryTrackRepository` |
| `infrastructure::lock` | File lock infrastructure | `FsFileLockManager` |
| `cli` | Composition Root | `main()`, lock subcommands |

## Agent Roles

| Agent / Capability | Role |
|-------|------|
| Claude Code (main) | Overall orchestration, user interaction |
| `planner` / `reviewer` / `debugger` | Rust design, review, debugging |
| `researcher` / `multimodal_reader` | Crate research, codebase analysis, external document reading |

Note: See `.claude/agent-profiles.json` for which provider handles each capability.

## Key Design Decisions

| Decision | Rationale | Alternatives Considered | Date |
|----------|-----------|------------------------|------|
| TrackStatus derived from tasks, not stored | Eliminates status desync; matches Python reference | Stored status with manual sync | 2026-03-11 |
| TaskStatus::Done owns Option\<CommitHash\> | Commit hash data bound to done state at type level | Separate commit_hash field on TrackTask | 2026-03-11 |
| TaskTransition as explicit enum commands | Type-safe transition API; exhaustive match coverage | String-based transitions like Python | 2026-03-11 |
| StatusOverride auto-clears on all-resolved | Prevents stale override on completed tracks | Manual override management | 2026-03-11 |
| Plan-task referential integrity at construction | Catches invalid plans early; mirrors Python validation | Runtime validation on access | 2026-03-11 |
| File-based lock registry + flock | Inspectable, no daemon, flock auto-release on crash | Per-file sidecar, advisory locks, socket daemon | 2026-03-11 |
| FileGuard with boxed release callback | Domain layer stays I/O-free; RAII release on drop | Trait-based release, manual release only | 2026-03-11 |
| fd-lock for cross-process file locking | RwLock API maps to &/&mut semantics; RAII built-in | fs2 (no RAII), fslock (weak shared/exclusive) | 2026-03-11 |
| PID + TTL stale lock recovery | Auto-reap on crash; no manual intervention needed | Heartbeat daemon, manual cleanup only | 2026-03-11 |
| Lexicographic path ordering for deadlock prevention | Simple total ordering; no lock upgrading allowed | Wait-for graph, lock-free design | 2026-03-11 |
| Fail-closed hook error handling | Lock acquire hook blocks tool on any error (CLI not found, timeout, unexpected exception); never proceeds unlocked | Fail-open (silently skip locking on error) | 2026-03-11 |
| AlreadyHeld immediate rejection | Same-agent reacquire returns AlreadyHeld immediately even with timeout; logic errors should not be retried | Retry until timeout (masks the real error) | 2026-03-11 |
| Shell guard in domain layer (no trait) | Pure computation, no I/O, no implementation variability | tree-sitter-bash (C dep), domain trait (over-engineering) | 2026-03-11 |
| conch-parser for shell AST (vendored, patched) | Full POSIX AST, minimal deps (void only), structural env var/command separation | Hand-written parser (edge case proliferation), tree-sitter-bash (C dep), brush-parser (heavy deps) | 2026-03-11 |
| Guard policy: ban edge-case-producing patterns | Unconditionally block patterns that create bypass vectors but are unnecessary in the template workflow: (1) `env` command → immediate block, (2) `$VAR`/`$(cmd)`/`` `cmd` `` in **any position** (argv + redirect texts including heredoc bodies) → immediate block, (3) `.exe` suffix → stripped in basename, (4) if effective command ≠ `git` and any argv/redirect token contains "git" (case-insensitive) → block. Rules (2) and (4) together eliminate ALL per-tool nesting analysis with argv/redirect-level checks. | Per-pattern recursive parsing and validation (complex, error-prone, ~200 lines of per-tool option parsing) | 2026-03-11 |

## Crate Selection

| Crate | Version | Role | Notes |
|-------|---------|------|-------|
| thiserror | 2.x | Error derive macros | Domain layer only external dep |
| fd-lock | latest | Cross-process file locking (RwLock API) | Infrastructure layer; maps &/&mut to shared/exclusive |

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

### File Lock Manager (ownership-file-lock-2026-03-11)

```text
libs/domain/src/lock/
├── mod.rs              # re-exports
├── types.rs            # FilePath, AgentId, LockMode, LockEntry
├── guard.rs            # FileGuard (RAII)
├── error.rs            # LockError
└── port.rs             # FileLockManager trait

libs/infrastructure/src/lock/
├── mod.rs              # re-exports
└── fs_lock_manager.rs  # FsFileLockManager (file-based registry impl)

apps/cli/src/commands/
├── mod.rs
└── lock.rs             # lock acquire/release/status/cleanup/extend
```

```rust
// domain/src/lock/types.rs
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A canonicalized file path used as lock key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FilePath(PathBuf);

impl FilePath {
    /// Creates a new `FilePath` by canonicalizing the given path.
    ///
    /// # Errors
    /// Returns `LockError::InvalidPath` if canonicalization fails.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, super::error::LockError>;
    pub fn as_path(&self) -> &Path;
}

/// Identifies the agent holding or requesting a lock.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self;
    pub fn as_str(&self) -> &str;
}

/// Maps to Rust's borrow semantics:
/// - `Shared` ≈ `&T` — multiple readers allowed
/// - `Exclusive` ≈ `&mut T` — single writer, no concurrent readers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

/// A single lock entry in the registry.
#[derive(Debug, Clone)]
pub struct LockEntry {
    pub path: FilePath,
    pub mode: LockMode,
    pub agent: AgentId,
    pub pid: u32,
    pub acquired_at: SystemTime,
    pub expires_at: SystemTime,
}
```

```rust
// domain/src/lock/error.rs
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("path cannot be canonicalized: {path}")]
    InvalidPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("file is exclusively locked by agent {holder} (pid {pid})")]
    ExclusivelyHeld {
        holder: super::types::AgentId,
        pid: u32,
    },

    #[error("file has {count} shared lock(s); cannot acquire exclusive lock")]
    SharedLockConflict { count: usize },

    #[error("lock not found for path {path} held by agent {agent}")]
    NotFound {
        path: super::types::FilePath,
        agent: super::types::AgentId,
    },

    #[error("lock acquisition timed out after {elapsed_ms}ms")]
    Timeout { elapsed_ms: u64 },

    #[error("lock registry I/O error")]
    RegistryIo(#[source] std::io::Error),
}
```

```rust
// domain/src/lock/guard.rs

/// RAII guard that releases the lock on drop.
///
/// Holds a boxed release callback so the domain layer
/// does not depend on the infrastructure implementation.
pub struct FileGuard {
    path: FilePath,
    mode: LockMode,
    agent: AgentId,
    release_fn: Option<Box<dyn FnOnce(&FilePath, &AgentId) + Send>>,
}

impl FileGuard {
    pub fn new(
        path: FilePath,
        mode: LockMode,
        agent: AgentId,
        release_fn: Box<dyn FnOnce(&FilePath, &AgentId) + Send>,
    ) -> Self;
    pub fn path(&self) -> &FilePath;
    pub fn mode(&self) -> LockMode;
    pub fn agent(&self) -> &AgentId;
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        if let Some(f) = self.release_fn.take() {
            f(&self.path, &self.agent);
        }
    }
}
```

```rust
// domain/src/lock/port.rs
use std::time::Duration;

/// Port for file lock management.
///
/// Implementations must be `Send + Sync` for use across threads
/// within the CLI process.
pub trait FileLockManager: Send + Sync {
    /// Acquires a lock on the given path.
    ///
    /// # Errors
    /// - `LockError::ExclusivelyHeld` if another agent holds an exclusive lock.
    /// - `LockError::SharedLockConflict` if shared locks exist and exclusive is requested.
    /// - `LockError::Timeout` if `timeout` elapses before the lock is available.
    /// - `LockError::RegistryIo` on I/O failure.
    fn acquire(
        &self,
        path: &FilePath,
        mode: LockMode,
        agent: &AgentId,
        pid: u32,
        timeout: Option<Duration>,
    ) -> Result<FileGuard, LockError>;

    /// Explicitly releases a lock. Used by CLI subcommand path
    /// where RAII drop is not practical (Python hook → CLI invoke → process exits).
    ///
    /// # Errors
    /// - `LockError::NotFound` if no matching lock exists.
    /// - `LockError::RegistryIo` on I/O failure.
    fn release(&self, path: &FilePath, agent: &AgentId) -> Result<(), LockError>;

    /// Queries all current locks. If `path` is `Some`, filters to that file.
    ///
    /// # Errors
    /// - `LockError::RegistryIo` on I/O failure.
    fn query(&self, path: Option<&FilePath>) -> Result<Vec<LockEntry>, LockError>;

    /// Removes stale entries (dead PIDs, expired timestamps).
    /// Returns the number of entries reaped.
    ///
    /// # Errors
    /// - `LockError::RegistryIo` on I/O failure.
    fn cleanup(&self) -> Result<usize, LockError>;

    /// Extends the expiry of an existing lock.
    ///
    /// # Errors
    /// - `LockError::NotFound` if no matching lock exists.
    /// - `LockError::RegistryIo` on I/O failure.
    fn extend(
        &self,
        path: &FilePath,
        agent: &AgentId,
        additional: Duration,
    ) -> Result<(), LockError>;
}
```

```mermaid
flowchart TD
    A[Agent calls Edit/Write tool] --> B[PreToolUse Hook fires]
    B --> C[Python: extract file path from JSON stdin]
    C --> D[Python invokes: sotp lock acquire\n--mode exclusive --path file.rs\n--agent agent-1 --pid PID]
    D --> E{CLI: acquire lock}
    E -->|flock .locks/LOCK| F[Read .locks/registry.json]
    F --> G{Conflict?}
    G -->|No conflict| H[Write new LockEntry to registry]
    H --> I[Release flock]
    I --> J[Exit 0]
    J --> K[Hook: exit 0 — allow tool]
    K --> L[Tool executes Edit/Write]
    L --> M[PostToolUse Hook fires]
    M --> N[Python invokes: sotp lock release\n--path file.rs --agent agent-1]
    N --> O[CLI removes entry from registry]
    G -->|Conflict: exclusive held| Q[Exit 1 + error message]
    Q --> R[Hook: exit 2 — block tool]
    G -->|Stale entry detected| T[Reap stale entry]
    T --> G
```

```rust
// CLI lock subcommands (apps/cli/src/commands/lock.rs)
#[derive(Debug, clap::Subcommand)]
pub enum LockCommand {
    Acquire {
        #[arg(long)]
        mode: String,  // "shared" or "exclusive"
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        pid: u32,
        #[arg(long, default_value = "5000")]
        timeout_ms: u64,
    },
    Release {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        agent: String,
    },
    Status {
        #[arg(long)]
        path: Option<PathBuf>,
    },
    Cleanup,
    Extend {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = "300000")]
        additional_ms: u64,
    },
}
```

### Shell Command Guard (guard-cli-2026-03-11)

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
    B --> C[Python: extract command from JSON stdin]
    C --> D[Python invokes: sotp guard check\n--command 'git add .']
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
    O --> Q[Python hook: exit 2 — block tool]
    P --> R[Python hook: exit 0 — allow tool]
```

## Security Hardening: Python-to-Rust Hybrid Migration (Phase 1-2)

### Strategy: Option C — Rust for Critical Paths, Python for Advisory

| Layer | Rust (fail-closed by design) | Python (advisory, keep as-is) |
|-------|------------------------------|-------------------------------|
| Security hooks | `block-direct-git-ops`, `file-lock-acquire/release` | `suggest-*`, `lint-on-save`, `agent-router`, etc. |
| Track state I/O | `metadata.json` read-modify-write | `plan.md` / `registry.md` rendering |
| File writes | Atomic write for critical data | Log append (JSONL) |

### SOLID Design Principles Applied

| Principle | Decision |
|-----------|----------|
| SRP | `Decision` shared across subdomains; `TrackReader`/`TrackWriter` split; UseCase owns only business logic |
| OCP | `HookHandler` trait for extensible hook dispatch without modifying existing code |
| LSP | `TrackReader`/`TrackWriter` implementations interchangeable (InMemory, Fs) |
| ISP | Read-only consumers depend only on `TrackReader`; mutation consumers on `TrackWriter` |
| DIP | Domain defines ports; `clap`/`fd-lock` confined to CLI/Infrastructure layers |

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
│   ├── fs_store.rs      # FsTrackStore: TrackReader + TrackWriter with FileLockManager
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
    FileLockAcquire,
    FileLockRelease,
}

/// Context for hook execution. Built by the CLI layer from:
/// - `project_dir`: `$CLAUDE_PROJECT_DIR` env var (set by Claude Code)
/// - `locks_dir`: `$SOTP_LOCKS_DIR` env var or `--locks-dir` CLI arg
///   (default: `$CLAUDE_PROJECT_DIR/.locks` — must be project-root-anchored)
///   If neither is set → exit 2 (fail-closed, prevents split-registry)
/// - `agent`: `$SOTP_AGENT_ID` env var or `--agent` CLI arg — NO SAFE DEFAULT in sotp
///   (same reason as pid: sotp's ppid is the Python launcher, not Claude Code).
///   Python launcher MUST pass `--agent` explicitly (e.g., `f"pid-{os.getppid()}"`).
/// - `pid`: `--pid` CLI arg — NO SAFE DEFAULT in sotp.
///
/// ## PID / Agent Propagation for Lock Hooks
///
/// The Python launcher for lock hooks runs: Python → sotp (subprocess).
/// If sotp uses `getppid()`, it gets the Python launcher PID (short-lived),
/// not Claude Code's PID. This makes the lock immediately stale-reapable.
///
/// Therefore, lock-acquire launchers MUST compute pid/agent in Python and
/// pass them explicitly via `--pid` and `--agent` CLI args, exactly as
/// the current `file-lock-acquire.py` does:
///   pid = os.getppid()      # Claude Code PID (Python's parent)
///   agent = f"pid-{pid}"    # or $SOTP_AGENT_ID
///
/// Lock-release launchers MUST pass `--agent` but `--pid` is optional
/// (`FileLockManager::release` takes only `path` + `agent`).
///
/// For `block-direct-git-ops` (guard hook), pid/agent are irrelevant
/// and can be omitted.
///
/// Python launchers pass `--locks-dir` and `--agent` via CLI args or env vars.
/// `--pid` is CLI-arg-only (no env var — must be explicitly passed by launcher).
/// All fields are `Option` because different hooks need different subsets.
/// The CLI layer validates per-hook requirements:
/// - guard (block-direct-git-ops): only `project_dir` needed (for future use);
///   if `$CLAUDE_PROJECT_DIR` is unset, guard still works (it only inspects the command)
/// - lock-acquire: `project_dir` (for locks_dir default) + `locks_dir` + `agent` + `pid`
///   required — missing any → exit 2
/// - lock-release: `locks_dir` + `agent` required — `pid` NOT needed
///   (`FileLockManager::release` takes only `path` + `agent`, not `pid`)
#[derive(Debug, Clone)]
pub struct HookContext {
    pub project_dir: Option<std::path::PathBuf>,
    pub locks_dir: Option<std::path::PathBuf>,
    pub agent: Option<crate::lock::AgentId>,
    pub pid: Option<u32>,
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
// fail-closed). For PostToolUse hooks (lock-release) it results in stderr warning +
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
    Lock(#[from] crate::lock::LockError),

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
/// and infrastructure errors (I/O, lock, codec).
///
/// NOTE: The `Domain` variant wraps `DomainError` which currently contains
/// a `Repository` variant. This creates an ambiguous path:
/// `TrackWriteError::Domain(DomainError::Repository(_))`.
///
/// Migration plan (applied in filelock-migration track):
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

    /// Atomically loads, mutates, and persists a track under exclusive lock.
    ///
    /// # Errors
    /// - `TrackWriteError::Repository(TrackNotFound)` if the track does not exist.
    /// - `TrackWriteError::Repository(Message)` on I/O or lock failure.
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
/// - PreToolUse (guard, lock-acquire): `HookError::Input` → exit 2 (block, fail-closed)
/// - PostToolUse (lock-release): `HookError::Input` → stderr warning + exit 0 (cannot block)
///
/// | Hook | Required fields | Missing → |
/// |------|----------------|-----------|
/// | `BlockDirectGitOps` | `tool_name` (always present), `command` | `HookError::Input("missing command")` |
/// | `FileLockAcquire` | `tool_name`, `file_path` | `HookError::Input("missing file_path")` |
/// | `FileLockRelease` | `tool_name`, `file_path` | `HookError::Input("missing file_path")` |
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
/// Uses FileLockManager for exclusive access during mutations.
/// Uses atomic_write_file for crash-safe persistence.
pub struct FsTrackStore<L: domain::lock::FileLockManager> {
    root: std::path::PathBuf,
    lock_manager: std::sync::Arc<L>,
    lock_timeout: std::time::Duration,
}

impl<L: domain::lock::FileLockManager> FsTrackStore<L> {
    pub fn new(
        root: impl Into<std::path::PathBuf>,
        lock_manager: std::sync::Arc<L>,
        lock_timeout: std::time::Duration,
    ) -> Self;
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
    /// PostToolUse hooks (lock-release): any error → stderr warning + exit 0 (cannot block).
    Dispatch {
        #[arg(value_enum)]
        hook: CliHookName,

        /// Locks directory (for file-lock hooks).
        /// Default: `$CLAUDE_PROJECT_DIR/.locks` (project-root-anchored).
        /// Also read from `$SOTP_LOCKS_DIR`.
        /// If neither `--locks-dir` nor `$SOTP_LOCKS_DIR` nor `$CLAUDE_PROJECT_DIR`
        /// is set → exit 2 (fail-closed). No cwd fallback — prevents split-registry.
        #[arg(long, env = "SOTP_LOCKS_DIR")]
        locks_dir: Option<std::path::PathBuf>,

        /// Agent ID (for file-lock hooks). Required for lock hooks.
        /// MUST be passed explicitly by the Python launcher (same reason as pid:
        /// sotp's ppid is the Python launcher PID, not Claude Code's).
        /// Also read from `$SOTP_AGENT_ID`.
        #[arg(long, env = "SOTP_AGENT_ID")]
        agent: Option<String>,

        /// Process ID of the lock holder (required for lock-acquire only).
        /// MUST be the long-lived Claude Code PID, passed explicitly by
        /// the Python launcher. No safe default exists in the sotp process:
        /// - std::process::id() = sotp PID (dies immediately)
        /// - getppid() = Python launcher PID (dies immediately)
        /// Not needed for lock-release (release API uses path + agent only).
        /// Not needed for guard hooks (block-direct-git-ops).
        #[arg(long)]
        pid: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookName {
    BlockDirectGitOps,
    FileLockAcquire,
    FileLockRelease,
}

impl From<CliHookName> for domain::hook::HookName { ... }

// NOTE: For FileLockAcquire, the handler calls FileLockManager::acquire()
// and the returned FileGuard must be forgotten (std::mem::forget) to prevent
// the RAII drop from releasing the lock when the hook process exits.
// The lock is explicitly released by a separate PostToolUse hook
// (FileLockRelease). This matches the existing pattern in lock.rs:L108-112.
```

```rust
// apps/cli/src/commands/hook.rs — stdout JSON mapping for Claude Code hooks
//
// The Rust CLI outputs structured JSON that Python launchers forward to stdout.
// These formats match the existing codebase patterns (file-lock-acquire.py, etc.)
//
// PreToolUse hooks (block-direct-git-ops):
//   Allow:  exit 0, stdout = "" (empty)
//   Block:  exit 2, stdout = plain text reason (matches current block-direct-git-ops.py)
//
// PreToolUse hooks (file-lock-acquire):
//   Allow:  exit 0, stdout = "" (empty)
//   Block:  exit 2, stdout = JSON:
//     {"hookSpecificOutput": {
//       "decision": "block",
//       "reason": "<reason from HookVerdict>"
//     }}
//
// PostToolUse hooks (file-lock-release):
//   exit 0, stdout = "" or JSON:
//     {"hookSpecificOutput": {
//       "hookEventName": "PostToolUse",
//       "additionalContext": "<context from HookVerdict>"
//     }}
//   (PostToolUse cannot block — it runs after tool execution)
//
// Error:
//   PreToolUse hooks (guard, lock-acquire):
//     exit 2, stdout = plain text (guard) or block JSON (lock-acquire)
//     All PreToolUse errors are fail-closed (exit 2).
//   PostToolUse hooks (lock-release):
//     exit 0, stderr = warning message. PostToolUse CANNOT block —
//     the tool has already executed. Errors are logged but do not
//     prevent operation. This matches PostToolUse semantics.
//
// NOTE: The exact hookSpecificOutput schema may evolve with Claude Code versions.
// See https://docs.anthropic.com/en/docs/claude-code/hooks for authoritative spec.
// The Rust CLI should match the patterns established by existing Python hooks.
```

### UseCase Return Type Migration

When use cases migrate from `TrackRepository` to `TrackReader`/`TrackWriter` (filelock-migration track):

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
| 2. hook-fail-closed | Implement `domain::hook`, `usecase::hook`, `cli::commands::hook`; Python hooks become thin `sotp hook dispatch` launchers |
| 3. filelock-migration | Implement `infrastructure::track::{codec, fs_store, atomic_write}`; replace `TrackRepository` with `TrackReader`+`TrackWriter`; Python `track_state_machine.py` delegates to `sotp track` |
| 4. per-worker-target-dir | Docker/Makefile only — no Rust changes |
| 5. atomic-write-standard | Reuse `atomic_write_file` from track 3; apply to remaining Python scripts or migrate them |
| 6. security-control-tests | Rust integration tests for hooks, locking, atomic writes |

### Python Launcher Pattern (No Fallback, Fail-Closed)

```python
#!/usr/bin/env python3
"""Thin launcher for sotp hook dispatch. No Python fallback — fail-closed by design."""
import os, subprocess, sys

# Internal subprocess timeout (seconds). Matches existing file-lock-acquire.py pattern.
# If sotp hangs beyond this, the launcher exits 2 (block) without waiting for
# the outer Claude Code hook timeout.
_SUBPROCESS_TIMEOUT = 10

def main() -> int:
    cli = os.environ.get("SOTP_CLI_BINARY", "sotp")
    result = subprocess.run(
        [cli, "hook", "dispatch", "block-direct-git-ops"],
        input=sys.stdin.buffer.read(),
        capture_output=True,
        timeout=_SUBPROCESS_TIMEOUT,
    )
    sys.stdout.buffer.write(result.stdout)
    sys.stdout.buffer.flush()
    sys.stderr.buffer.write(result.stderr)
    sys.stderr.buffer.flush()
    return 0 if result.returncode == 0 else 2

if __name__ == "__main__":
    try:
        code = main()
    except BaseException:
        # Catches Exception (FileNotFoundError, TimeoutExpired, etc.),
        # KeyboardInterrupt, and SystemExit.
        # No sys.exit() inside the try block, so SystemExit cannot
        # accidentally swallow an intentional exit.
        code = 2
    os._exit(code)  # os._exit bypasses SystemExit entirely — guaranteed exit code
```

Key design decisions (PreToolUse launcher — guard, lock-acquire):

- **No Python fallback**: If `sotp` binary is missing, the launcher blocks (exit 2). No dual authority.
- **Exit code mapping**: Only Rust exit 0 passes through as allow. Everything else → exit 2 (block).
- **Subprocess timeout**: 10s internal timeout prevents hung `sotp` from blocking indefinitely. `TimeoutExpired` is caught by `except BaseException` → exit 2.
- **Explicit flush**: `sys.stdout/stderr.buffer.flush()` before `os._exit()` ensures forwarded output is not lost.
- **`os._exit()` not `sys.exit()`**: Guarantees the exact exit code without raising `SystemExit`.
- **`except BaseException`**: Catches `KeyboardInterrupt`, `SystemExit`, and all `Exception` subclasses. Safe because `main()` never calls `sys.exit()`.
- **Bootstrap guarantee**: `cargo make bootstrap` builds `sotp` before hooks can fire.
- **Claude Code hook protocol**: exit 0 = allow, exit 2 = block, exit 1 = non-blocking error (continues execution — NOT safe for security hooks).

PostToolUse launcher (lock-release) differences:

- **Cannot block**: PostToolUse fires after the tool has already executed. Exit 2 has no effect.
- **Error → warn + exit 0**: If `sotp` is missing, crashes, or times out, the launcher writes a warning to stderr and exits 0. This matches PostToolUse semantics.
- **`except BaseException` → exit 0** (not exit 2): All launcher-level errors are non-fatal.

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `TrackDocumentV2` schema drift vs Python `track_schema.py` | Incompatible metadata.json | Shared compatibility tests until Python writers removed |
| `TransitionTaskUseCase` still using `find/save` pattern | Data loss under contention | Migrate to `TrackWriter::update` in track 3 |
| Hook `tool_name` field missing from payload | JSON parse failure at CLI boundary | No `#[serde(default)]` on security fields. PreToolUse: parse error → exit 2 (block). PostToolUse: parse error → warn + exit 0 |
| Hook-specific required fields (`command`, `file_path`) missing | Malformed payload bypasses control | Each `HookHandler` validates required fields → `HookError::Input` → PreToolUse: exit 2 (block), PostToolUse: warn + exit 0 |
| Atomic rename cross-filesystem | Non-atomic write | Temp file in target directory; parent fsync mandatory |
| `TrackWriter::update<F>` is non-object-safe | Cannot use `dyn TrackWriter` | Acceptable: use cases use generics. Extract non-generic sub-trait if dyn needed later |
| `TrackStatus::Archived` missing in Rust domain | Python schema incompatibility | Add `Archived` variant to `TrackStatus` enum in domain layer |
| `DomainError::Repository` leaks into `TrackWriteError::Domain` | Ambiguous error path | Remove `DomainError::Repository` variant when migrating to `TrackReader`/`TrackWriter` |

## Open Questions

_None at this time._

## Changelog

| Date | Changes |
|------|---------|
| 2026-03-11 | Initial design: DMMF track state machine domain model (Codex planner) |
| 2026-03-11 | File lock manager: ownership-based concurrent file access control (Codex planner) |
| 2026-03-11 | Shell command guard: deterministic shell parsing + git operation blocking in domain layer |
| 2026-03-11 | Security hardening: Python-to-Rust hybrid migration design (SOLID, Codex planner) |
| 2026-03-11 | Codex review R1 fixes: typed port errors, HookInput DIP, exit code 2, no Python fallback, non-object-safe note |
| 2026-03-11 | Codex review R2 fixes: HookContext param supply, BaseException launcher, required field validation, DomainError::Repository separation, UseCase return types, hook output JSON mapping |
| 2026-03-11 | Codex review R3 fixes: hook JSON aligned with existing patterns, subprocess timeout, stdout flush before os._exit, create→save upsert semantics |
| 2026-03-11 | Codex review R4 fixes: pid default→getppid(), locks_dir→$CLAUDE_PROJECT_DIR/.locks, hook output per-hook spec alignment |
| 2026-03-11 | Codex review R5 fixes: explicit pid/agent propagation from launcher, CLAUDE_PROJECT_DIR unset→exit 2, PostToolUse error→warn+exit 0 |
| 2026-03-11 | Codex review R6 fixes: PostToolUse launcher exit 0 (not exit 2), agent no safe default in sotp, HookContext lock fields Optional |
| 2026-03-11 | Codex review R7 fixes: lock-release pid not required, per-hook context validation, acceptance criteria PreToolUse/PostToolUse split |
| 2026-03-11 | Codex review R8 fixes: HookError::Input exit code per event type, launcher pid guidance acquire-only, --pid doc scoped to lock-acquire |
| 2026-03-11 | Codex review R9 fixes: serde parse failure per event type, risk table per-event exit code, --pid CLI-arg-only (no env var) |
| 2026-03-11 | Codex review R10 fixes: risk table tool_name per-event, HookContext.project_dir Optional (guard works without CLAUDE_PROJECT_DIR) |
