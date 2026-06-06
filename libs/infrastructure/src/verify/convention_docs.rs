//! Compatibility shim — delegates to `conventions::verify_convention_index`.
//!
//! Will be removed by T004 when CLI wiring is updated.

use std::path::Path;

use domain::verify::VerifyOutcome;

/// Verify that the conventions README index is in sync with actual files.
///
/// # Errors
///
/// Returns error findings when the README is missing, markers are absent,
/// or the index block does not match the expected content.
pub fn verify(root: &Path) -> VerifyOutcome {
    crate::conventions::verify_convention_index(root)
}
