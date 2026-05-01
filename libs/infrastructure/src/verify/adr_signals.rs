//! Verify ADR decision signal grounds (`verify adr-signals` subcommand).
//!
//! All domain type handling is internal to this module. The CLI layer calls
//! `execute_verify_adr_signals` and receives a `VerifyOutcome` — no `domain::`
//! imports needed in `apps/cli/src/`.

use std::path::Path;
use std::sync::Arc;

use domain::AdrFilePort;
use domain::verify::{Severity, VerifyFinding, VerifyOutcome};
use usecase::verify_adr_signals::{
    VerifyAdrSignals, VerifyAdrSignalsCommand, VerifyAdrSignalsInteractor,
};

use crate::adr_decision::FsAdrFileAdapter;
use crate::track::symlink_guard::reject_symlinks_below;

/// Execute the `verify adr-signals` subcommand.
///
/// Composes [`FsAdrFileAdapter`] with [`VerifyAdrSignalsInteractor`] at the
/// composition root, runs the verification, and translates the resulting
/// `AdrVerifyReport` into a `VerifyOutcome`:
///
/// - `red_count >= 1` → error finding (drives exit 1) plus a stderr summary.
/// - `yellow_count >= 1` (no red) → warning finding (still exit 0).
/// - all blue (or empty directory) → info finding (exit 0).
///
/// # Errors
///
/// Returns a `VerifyOutcome` with an error finding when the interactor fails
/// (e.g., directory listing failure, I/O error).
pub fn execute_verify_adr_signals(project_root: &Path) -> VerifyOutcome {
    // Security: guard `project_root` itself before using it as the symlink-guard trusted root.
    // `reject_symlinks_below` only inspects descendants — a symlinked root would bypass it.
    match project_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            let msg = format!(
                "symlink guard: refusing to use symlinked project_root: {}",
                project_root.display()
            );
            eprintln!("{msg}");
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(msg)]);
        }
        Ok(_) => {}
        Err(e) => {
            let msg =
                format!("symlink guard: cannot stat project_root {}: {e}", project_root.display());
            eprintln!("{msg}");
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(msg)]);
        }
    }

    // Canonicalize project_root to resolve `..` traversal bypasses before using it
    // as the trusted root for all downstream symlink guards.
    let project_root_canonical = match project_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("cannot canonicalize project_root {}: {e}", project_root.display());
            eprintln!("{msg}");
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(msg)]);
        }
    };
    let project_root = project_root_canonical.as_path();

    let adr_dir = project_root.join("knowledge/adr");

    // Symlink guard on the ADR directory: reject symlinks at adr_dir or any ancestor
    // below `project_root` before traversal (fail-closed per ADR §D7).
    match reject_symlinks_below(&adr_dir, project_root) {
        Ok(_) => {}
        Err(e) => {
            let msg = format!("refusing to read ADR directory {}: {e}", adr_dir.display());
            eprintln!("{msg}");
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(msg)]);
        }
    }

    let adapter = FsAdrFileAdapter::new(adr_dir);
    let port: Arc<dyn AdrFilePort> = Arc::new(adapter);
    let interactor = VerifyAdrSignalsInteractor::new(port);

    let report = match interactor.verify(VerifyAdrSignalsCommand) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("verify-adr-signals failed: {e}");
            eprintln!("{msg}");
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(msg)]);
        }
    };

    let summary = format!(
        "ADR signal counts: blue={} yellow={} red={} grandfathered={}",
        report.blue_count(),
        report.yellow_count(),
        report.red_count(),
        report.grandfathered_count(),
    );

    if report.red_count() >= 1 {
        eprintln!("[verify-adr-signals] {summary} (red_count >= 1 → CI block)");
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(summary)]);
    }

    if report.yellow_count() >= 1 {
        return VerifyOutcome::from_findings(vec![VerifyFinding::warning(summary)]);
    }

    VerifyOutcome::from_findings(vec![VerifyFinding::new(Severity::Info, summary)])
}
