//! Spec document operations.
//!
//! NOTE: `spec approve` is deprecated per ADR 2026-04-19-1242 §D3 / §D1.2.
//! The approved-lifecycle (status / approved_at / content_hash) was removed in
//! T003. `cargo make spec-approve` is removed in T009. This command now
//! returns an error explaining the migration path.

use std::process::ExitCode;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum SpecCommand {
    /// Approve a spec (DEPRECATED — approval gate removed, see ADR 2026-04-19-1242 §D3).
    Approve {
        /// Path to the track directory (e.g., track/items/<id>).
        track_dir: String,
    },
}

pub fn execute(cmd: SpecCommand) -> ExitCode {
    match cmd {
        SpecCommand::Approve { track_dir: _ } => {
            eprintln!(
                "[DEPRECATED] `sotp spec approve` has been removed per ADR 2026-04-19-1242 §D3.\n\
                 The spec approval gate is replaced by signal-based gating via `sotp track signals`.\n\
                 Run `sotp track signals <id>` to evaluate and store spec signals."
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approve_returns_deprecated_error() {
        let code = execute(SpecCommand::Approve { track_dir: "any-dir".to_owned() });
        assert_eq!(code, ExitCode::FAILURE, "deprecated approve must return FAILURE");
    }
}
