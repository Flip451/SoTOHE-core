//! `SignalService` — unified application-service facade for all `signal`
//! subcommands.
//!
//! Defines the primary port trait [`SignalService`] and the shared output DTO
//! [`SignalCommandOutput`] that the `cli_driver::signal::SignalDriver` consumes.
//! The composition root (`apps/cli-composition`) implements the trait by wiring
//! the appropriate infrastructure adapters and usecase interactors for each
//! subcommand.
//!
//! # Design rationale
//!
//! The `signal` family has nine subcommands that each require different
//! infrastructure setup (git discovery, ADR scan, spec.json resolution, TDDD
//! layer enumeration, type-signals executor, …).  Defining one wide service
//! trait lets the `SignalDriver` stay a simple dispatcher with a single
//! `Arc<dyn SignalService>` dependency, while the composition root retains
//! full control over wiring without leaking infrastructure types into
//! `cli_driver`.
//!
//! The output type [`SignalCommandOutput`] mirrors `cli_driver::CommandOutcome`
//! field-for-field so the driver can convert it in one expression, without
//! `usecase` needing to import `cli_driver`.

use std::path::PathBuf;

// ── Output DTO ────────────────────────────────────────────────────────────────

/// Unified output DTO for all `signal` subcommands.
///
/// Mirrors `cli_driver::render::CommandOutcome` field-for-field.  Defined here
/// (in the usecase layer) so that the `SignalService` trait does not import
/// `cli_driver`, preserving hexagonal layer order.
///
/// `cli_driver::signal` converts this to `CommandOutcome` in one expression.
#[derive(Debug, Clone)]
pub struct SignalCommandOutput {
    /// Optional text written to stdout.
    pub stdout: Option<String>,
    /// Optional text written to stderr.
    pub stderr: Option<String>,
    /// Process exit code (0 = success, non-zero = failure).
    pub exit_code: u8,
}

impl SignalCommandOutput {
    /// Construct a successful output with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Construct a failure output with optional stderr text.
    pub fn failure(stderr: Option<String>) -> Self {
        Self { stdout: None, stderr, exit_code: 1 }
    }
}

// ── Gate name ─────────────────────────────────────────────────────────────────

/// Selects the gate context when resolving strictness from `signal-gates.json`.
///
/// Re-defined here so that `cli_driver` can pass the value through the
/// `SignalService` port without importing `domain` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalGateName {
    /// CI commit gate — uses `commit_gate.*` cells.
    Commit,
    /// PR merge gate — uses `merge_gate.*` cells.
    Merge,
}

// ── Primary port ──────────────────────────────────────────────────────────────

/// Primary port for the `signal` command family.
///
/// Each method corresponds to one `sotp signal <subcommand>` invocation.
/// Return value is [`SignalCommandOutput`]; the driver converts it to
/// `CommandOutcome`.
pub trait SignalService: Send + Sync {
    /// `signal calc-adr-user` — compute ADR signal grounding from
    /// `project_root/knowledge/adr/`.
    fn calc_adr_user(&self, project_root: PathBuf) -> SignalCommandOutput;

    /// `signal check-adr-user` — evaluate chain ⓪ (ADR→user) gate.
    fn check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal calc-spec-adr` — compute and persist chain ① signals to
    /// `spec.json`.
    fn calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal check-spec-adr` — evaluate chain ① (spec→ADR) gate.
    fn check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal calc-catalog-spec` — compute and persist chain ② signals for
    /// all TDDD-enabled layers.
    fn calc_catalog_spec(&self) -> SignalCommandOutput;

    /// `signal check-catalog-spec` — evaluate chain ② (catalog→spec) gate.
    fn check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal calc-impl-catalog` — compute and persist chain ③ signals for
    /// all TDDD-enabled layers.
    fn calc_impl_catalog(&self) -> SignalCommandOutput;

    /// `signal check-impl-catalog` — evaluate chain ③ (impl↔catalog) gate.
    fn check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal check --gate` — evaluate commit/merge gate (chains ⓪①②③).
    fn check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;
}
