//! Render helpers: `CommandOutcome`, constructor helpers, and shared output formatters.
//!
//! `CommandOutcome` is the unified return type for all driver command methods.
//! It currently mirrors `cli_composition::CommandOutcome` 1:1; consolidation
//! into a single canonical definition (here, in cli_driver) is T024 scope.

/// Unified return type for all driver command methods.
///
/// `bin` reads `stdout` / `stderr` and emits them, then exits with `exit_code`.
/// All fields are primitives so `bin` never needs to import domain types (CN-02).
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: u8,
}

impl CommandOutcome {
    /// Convenience constructor: success with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Convenience constructor: failure with optional stderr text.
    pub fn failure(stderr: Option<String>) -> Self {
        Self { stdout: None, stderr, exit_code: 1 }
    }
}

// Note: the previous staging placeholder for `render_outcome(label, VerifyOutcome)`
// was removed because its proposed signature took `infrastructure::verify::VerifyOutcome`,
// which would create a cli_driver → infrastructure dependency edge. Per
// `architecture-rules.json`, cli_driver may_depend_on = ["usecase"] only.
// If a generic verify-result render helper is needed in cli_driver, the
// `VerifyOutcome` type must first be lifted into the usecase layer (or a
// usecase DTO crafted) so cli_driver can format it without reaching into
// infrastructure. Today, per-family Drivers render their own command output
// using only usecase types they already consume.
