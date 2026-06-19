//! Shared test helpers for the `verify` module suite.
//!
//! Helpers in this module are compiled only in `#[cfg(test)]` contexts (see
//! the `mod.rs` declaration).  All functions here are `pub(crate)` so any
//! sub-module in `libs/infrastructure::verify` can import them without
//! duplicating the implementation.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

/// Write a minimal ADR YAML file with a single decision whose decision reference
/// is set to `ref_key: ref_value`.
///
/// Use this to create Yellow-signal ADRs (`review_finding_ref`) or Blue-signal
/// ADRs (`user_decision_ref`) without duplicating the fixture schema in every
/// test module.
pub(crate) fn write_minimal_adr(adr_dir: &Path, filename: &str, ref_key: &str, ref_value: &str) {
    let content = format!(
        "---\nadr_id: test-adr\ndecisions:\n  - id: D1\n    status: accepted\n    {ref_key}: {ref_value}\n---\n# Test ADR\n"
    );
    std::fs::write(adr_dir.join(filename), content).unwrap();
}

/// Initialise a minimal git repository in `dir`.
///
/// Configures a stub identity (`test@test.com` / `Test`) so git does not error
/// on missing global config.  Panics when `git` is not installed — these tests
/// require a working `git` binary on `PATH`.
pub(crate) fn git_init(dir: &Path) {
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir)
        .status()
        .expect("git must be installed for these tests");
    assert!(status.success(), "git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .status()
        .ok();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .status()
        .ok();
}
