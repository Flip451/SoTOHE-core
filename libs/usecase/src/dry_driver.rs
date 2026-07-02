//! Driver-level service port for the `dry` command family.
//!
//! Defines a single `DryDriverService` trait that the `cli_driver::dry::DryDriver`
//! invokes, plus a pass-through `DryDriverInteractor` that delegates to
//! an injected `DryDriverPort`.
//!
//! The adapter that implements `DryDriverPort` lives in `cli_composition`
//! (where `infrastructure` is available) and delegates to the existing
//! `DryCompositionRoot` / `DryFixRunnerCompositionRoot` methods.

use std::path::PathBuf;
use std::sync::Arc;

// ── Input types ───────────────────────────────────────────────────────────────

/// Input for `sotp dry write` (driver boundary).
#[derive(Debug, Clone)]
pub struct DryWriteDriverInput {
    pub track_id: String,
    pub base_commit: Option<String>,
    pub db_path: PathBuf,
    pub threshold: Option<f32>,
    pub workspace_root: PathBuf,
    pub items_dir: PathBuf,
    pub model: Option<String>,
    pub capability_name: String,
}

/// Input for `sotp dry results` (driver boundary).
#[derive(Debug, Clone)]
pub struct DryResultsDriverInput {
    pub track_id: String,
    pub filter: String,
    pub items_dir: PathBuf,
}

/// Input for `sotp dry check-approved` (driver boundary).
#[derive(Debug, Clone)]
pub struct DryCheckApprovedDriverInput {
    pub track_id: String,
    pub base_commit: Option<String>,
    pub items_dir: PathBuf,
}

/// Input for `sotp dry fix-local` (driver boundary).
#[derive(Debug, Clone)]
pub struct DryFixLocalDriverInput {
    pub track_id: String,
    pub briefing_file: PathBuf,
    pub model: Option<String>,
}

// ── Output type ───────────────────────────────────────────────────────────────

/// Unified command outcome returned to the driver.
///
/// Mirrors `cli_driver::render::CommandOutcome`; defined here as a plain struct
/// so the usecase layer carries no dependency on `cli_driver`.
#[derive(Debug, Clone)]
pub struct DryDriverOutcome {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: u8,
}

impl DryDriverOutcome {
    /// Convenience constructor: success with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Convenience constructor: failure with optional stderr text.
    pub fn failure(msg: Option<String>) -> Self {
        Self { stdout: None, stderr: msg, exit_code: 1 }
    }
}

/// One rendered-finding row carried by `DryWriteOutcome::Success` (IN-13).
///
/// Fields mirror `domain::dry_check::DryCheckFinding`'s `changed_fragment_ref` /
/// `candidate_fragment_ref` / `refactor_proposal`, flattened to `String` because
/// `cli_driver` may only depend on `usecase` (architecture-rules.json) and these
/// values are opaque display text at this boundary, already validated upstream
/// in the domain layer.
#[derive(Debug, Clone)]
pub struct DryWriteFindingSummary {
    pub changed_path: String,
    pub changed_content_hash: String,
    pub candidate_path: String,
    pub candidate_content_hash: String,
    pub refactor_proposal: String,
}

/// Structured (pre-render) output for `sotp dry write` at the `cli_driver`
/// boundary (IN-13/AC-18).
///
/// Replaces the previous `DryDriverOutcome`-based pre-formatted stdout: the CLI
/// text rendering (the former `dry_write_outcome` helper) moves to `cli_driver`.
#[derive(Debug, Clone)]
pub enum DryWriteOutcome {
    Success {
        pairs_checked: usize,
        records_appended: usize,
        diff_fragments_processed: usize,
        findings: Vec<DryWriteFindingSummary>,
    },
    Failure {
        message: String,
    },
}

/// Structured (pre-render) output for `sotp dry check-approved` at the
/// `cli_driver` boundary (IN-13/AC-18).
///
/// Mirrors `domain::dry_check::DryCheckApprovalVerdict`'s Approved/Blocked shape
/// as a usecase-level DTO (`cli_driver` may only depend on `usecase`, so the
/// domain enum itself cannot cross this boundary) plus a `Failure` variant for
/// adapter-level errors.
#[derive(Debug, Clone)]
pub enum DryCheckApprovedOutcome {
    Approved,
    Blocked { unresolved_pair_count: usize },
    Failure { message: String },
}

// ── Port ──────────────────────────────────────────────────────────────────────

/// Secondary port for the `dry` command family.
///
/// Implemented by an adapter in `cli_composition` that delegates to
/// `DryCompositionRoot` / `DryFixRunnerCompositionRoot` methods.
pub trait DryDriverPort: Send + Sync {
    /// Run `sotp dry write`.
    fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome;

    /// Run `sotp dry results`.
    fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome;

    /// Run `sotp dry check-approved`.
    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome;

    /// Run `sotp dry fix-local`.
    fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome;
}

// ── Service ───────────────────────────────────────────────────────────────────

/// Application service trait for the `dry` command family.
pub trait DryDriverService: Send + Sync {
    /// Run `sotp dry write`.
    fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome;

    /// Run `sotp dry results`.
    fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome;

    /// Run `sotp dry check-approved`.
    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome;

    /// Run `sotp dry fix-local`.
    fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome;
}

// ── Interactor ────────────────────────────────────────────────────────────────

/// Interactor implementing [`DryDriverService`] by delegating to the port.
pub struct DryDriverInteractor {
    port: Arc<dyn DryDriverPort>,
}

impl DryDriverInteractor {
    /// Create a new interactor bound to the given port.
    #[must_use]
    pub fn new(port: Arc<dyn DryDriverPort>) -> Self {
        Self { port }
    }
}

impl DryDriverService for DryDriverInteractor {
    fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome {
        self.port.dry_write(input)
    }

    fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome {
        self.port.dry_results(input)
    }

    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome {
        self.port.dry_check_approved(input)
    }

    fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome {
        self.port.dry_fix_local(input)
    }
}
