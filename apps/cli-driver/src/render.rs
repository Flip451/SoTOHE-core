// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! Render helpers: `CommandOutcome`, constructor helpers, and shared output formatters.
//!
//! `CommandOutcome` is the unified return type for all driver command methods.
//! It mirrors `cli_composition::CommandOutcome` 1:1; T021 removes the
//! `cli_composition` definition when the live path is flipped.
//!
//! `render_outcome` mirrors `cli_composition::cmd_outcome::render_outcome` (lines 12-33).
//! T021 promotes this function to the primary implementation and removes the
//! `cli_composition::cmd_outcome` duplicate.

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

// ---------------------------------------------------------------------------
// Shared render helpers (staged from cli_composition/src/cmd_outcome.rs
// lines 12-33; T021 activates when Cargo.toml is materialized).
// ---------------------------------------------------------------------------

// TODO(T021): uncomment and wire once `infrastructure` crate is in scope:
//
// /// Render a `VerifyOutcome` into a `CommandOutcome`.
// ///
// /// Formats findings with a header/footer label and sets `exit_code = 1` when
// /// any finding has error severity.
// ///
// /// Mirrors `cli_composition::cmd_outcome::render_outcome` (lines 12-33).
// pub fn render_outcome(
//     label: &str,
//     outcome: &infrastructure::verify::VerifyOutcome,
// ) -> CommandOutcome {
//     let mut lines = vec![format!("--- {label} ---")];
//     if outcome.findings().is_empty() {
//         lines.push("[OK] All checks passed.".to_owned());
//         lines.push(format!("--- {label} PASSED ---"));
//         CommandOutcome::success(Some(lines.join("\n")))
//     } else {
//         for finding in outcome.findings() {
//             lines.push(finding.to_string());
//         }
//         if outcome.has_errors() {
//             lines.push(format!("--- {label} FAILED ---"));
//             CommandOutcome { stdout: Some(lines.join("\n")), stderr: None, exit_code: 1 }
//         } else {
//             lines.push(format!("--- {label} PASSED ---"));
//             CommandOutcome::success(Some(lines.join("\n")))
//         }
//     }
// }
