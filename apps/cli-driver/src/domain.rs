// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `domain` command family — primary adapter driver.
//!
//! `DomainDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/domain.rs` (lines ~51-92); T021 removes the
//! `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use std::sync::Arc;
// use infrastructure::schema_export::RustdocSchemaExporter;
// use usecase::export_schema::{ExportSchemaCommand, ExportSchemaInteractor, ExportSchemaService};

use std::path::PathBuf;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Input DTO for the `domain export-schema` command.
#[derive(Debug, Clone)]
pub struct ExportSchemaInput {
    /// Crate name within the workspace.
    pub crate_name: String,
    /// Use indented JSON output.
    pub pretty: bool,
    /// Write output to a file instead of stdout.
    pub output: Option<PathBuf>,
}

/// Typed input for the `domain` command family.
pub enum DomainInput {
    /// Export the public API schema of a crate as JSON.
    ExportSchema(ExportSchemaInput),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `domain` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct DomainDriver {
    // TODO(T021): inject use-case interactors here.
    // export_schema_service: Arc<dyn usecase::export_schema::ExportSchemaService>,
}

impl DomainDriver {
    /// Create a new `DomainDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a domain command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: DomainInput) -> CommandOutcome {
        match input {
            DomainInput::ExportSchema(export_input) => self.domain_export_schema(export_input),
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/domain.rs
    // lines ~51-92; T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn domain_export_schema(&self, input: ExportSchemaInput) -> CommandOutcome {
        // TODO(T021): invoke ExportSchemaInteractor here.
        // Mirrors cli_composition/src/domain.rs DomainCompositionRoot::domain_export_schema.
        //
        // Output shape when `input.output` is Some:
        //   stderr = "[OK] Schema written to <path>"  exit_code = 0
        // Output shape when `input.output` is None:
        //   stdout = <json>  exit_code = 0
        let _ = (input.crate_name, input.pretty, input.output);
        CommandOutcome::success(None)
    }
}

impl Default for DomainDriver {
    fn default() -> Self {
        Self::new()
    }
}
