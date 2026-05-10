//! T008: `spec-code-consistency` verify subcommand is deleted.
//!
//! `execute_spec_code_consistency_str` and `evaluate_consistency_from_components`
//! depended on `TypeGraph`, `TypeBaseline`, and `check_consistency` which are
//! removed in T008.  The `sotp verify spec-code-consistency` CLI subcommand is
//! also deleted.
//!
//! This stub exports the minimum surface needed to keep the CLI verify.rs from
//! failing to compile while the command is phased out.

use domain::verify::{VerifyFinding, VerifyOutcome};

/// Stub — always returns an error explaining the command is removed.
///
/// T008: The real implementation is removed with `TypeGraph`.
pub fn execute_spec_code_consistency_str(
    _track_id_str: &str,
    _crate_name: &str,
    _project_root: &std::path::Path,
) -> VerifyOutcome {
    VerifyOutcome::from_findings(vec![VerifyFinding::error(
        "sotp verify spec-code-consistency is removed in T008. \
         Use `sotp track three-way-signals` instead."
            .to_owned(),
    )])
}
