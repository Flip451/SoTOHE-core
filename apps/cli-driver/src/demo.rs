//! `demo` command family — primary adapter driver.
//!
//! `DemoDriver` holds an injected [`usecase::demo::DemoService`] and exposes
//! `handle(input) -> CommandOutcome`.

use std::sync::Arc;

use usecase::demo::DemoService;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `demo` command family.
pub enum DemoInput {
    /// Run the built-in demo / default stub (used when no subcommand is given).
    Run,
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `demo` command family.
///
/// Holds an injected [`DemoService`]; exposes `handle(input) -> CommandOutcome`.
pub struct DemoDriver {
    service: Arc<dyn DemoService>,
}

impl DemoDriver {
    /// Create a new `DemoDriver` with the given service.
    pub fn new(service: Arc<dyn DemoService>) -> Self {
        Self { service }
    }

    /// Handle a demo command.
    pub fn handle(&self, input: DemoInput) -> CommandOutcome {
        match input {
            DemoInput::Run => self.demo(),
        }
    }

    fn demo(&self) -> CommandOutcome {
        match self.service.run() {
            Ok(msg) => CommandOutcome::success(Some(msg)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}
