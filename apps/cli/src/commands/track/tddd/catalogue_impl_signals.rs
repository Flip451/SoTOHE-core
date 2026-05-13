//! `sotp track catalogue-impl-signals` — diagnose SoT Chain ③ (catalogue ↔ implementation).
//!
//! Loads `<layer>-types.json` (CatalogueDocument / TypeGraph A), reads
//! `<layer>-types-baseline.json` as TypeGraph B (`rustdoc_types::Crate`), captures
//! TypeGraph C live via `cargo +nightly rustdoc`, and evaluates the 3-way
//! `SignalEvaluatorV2` producing a `ThreeWayEvaluationReport`.
//!
//! Outputs a markdown report to stdout. Exits with code 1 when any Red signals
//! are present.
//!
//! This command is an on-demand diagnostic tool (ADR 2026-05-11-2330 §D1 / D3):
//! it has no `--output` flag (no persisted view) and no Makefile wrapper.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use infrastructure::FsSymlinkGuard;
use infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
use infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter;
use infrastructure::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader;
use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use usecase::catalogue_impl_signals::{
    CatalogueImplSignalsInteractor, CatalogueImplSignalsService,
};

use crate::CliError;

/// Execute the `track catalogue-impl-signals` command.
///
/// Thin CLI adapter: constructs the concrete infrastructure adapters, wires up
/// `CatalogueImplSignalsInteractor`, and delegates all orchestration to the
/// usecase layer. Prints the returned report to stdout.
///
/// The track items directory is derived from `workspace_root` as
/// `<workspace_root>/track/items` (canonical layout). Symlink guards are
/// applied inside the interactor via the injected [`domain::SymlinkGuardPort`]
/// (`FsSymlinkGuard`), keeping filesystem I/O out of the usecase layer.
///
/// Exits with code 1 if any Red signals are found across all layers. Red
/// detection is done by scanning the returned report string for the "🔴 Red"
/// marker emitted by the interactor's formatter.
///
/// # Errors
///
/// Returns [`CliError`] when the track ID is invalid, any file is missing, or
/// the evaluation fails.
pub fn execute_catalogue_impl_signals(
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    // Build the concrete adapters at the composition root (apps/cli).
    let catalogue_loader = Arc::new(FsCatalogueDocumentLoader::new());
    let ext_crate_codec = Arc::new(CatalogueToExtendedCrateCodec::new());
    let evaluator = Arc::new(SignalEvaluatorV2::new());
    let rustdoc_crate_port = Arc::new(RustdocCrateAdapter::new(workspace_root.clone()));
    let layer_bindings_port = Arc::new(FsTdddLayerBindingsAdapter::new());
    let symlink_guard = Arc::new(FsSymlinkGuard::new());

    // Wire up the interactor via the usecase layer.
    // Symlink guards are now applied inside the interactor via the injected
    // SymlinkGuardPort (FsSymlinkGuard), keeping I/O out of the usecase layer.
    let interactor = CatalogueImplSignalsInteractor::new(
        catalogue_loader,
        ext_crate_codec,
        evaluator,
        rustdoc_crate_port,
        layer_bindings_port,
        symlink_guard,
    );

    // Delegate all orchestration to the usecase interactor.
    // items_dir is derived from workspace_root inside the interactor
    // (workspace_root/track/items) per CatalogueImplSignalsService contract.
    let report = interactor
        .run(track_id, workspace_root, layer)
        .map_err(|e| CliError::Message(e.to_string()))?;

    // D3: stdout-only output.
    let any_red = report_has_red_signals(&report);
    println!("{report}");

    if any_red { Ok(ExitCode::FAILURE) } else { Ok(ExitCode::SUCCESS) }
}

/// Returns `true` when the formatted report string contains at least one Red signal.
///
/// Scans for the exact table-cell marker `| 🔴 Red |` emitted by the
/// interactor's formatter (`libs/usecase/src/catalogue_impl_signals.rs`).
/// This marker only appears in the signal column of a table row; item/region
/// names (Rust identifiers) cannot contain the 🔴 emoji, so there are no
/// false positives. Using the full `| 🔴 Red |` cell form (rather than bare
/// `🔴 Red`) makes the scan independent of the summary-line format
/// (`🔴 {N} Red` where N may be 0).
pub(crate) fn report_has_red_signals(report: &str) -> bool {
    report.contains("| 🔴 Red |")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Symlinked workspace_root must be rejected before any I/O.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_workspace_root_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link_dir = tmp.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let result =
            execute_catalogue_impl_signals("test-track-2026-01-01".to_owned(), link_dir, None);
        assert!(result.is_err(), "symlinked workspace_root must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("symlink guard"), "error message must mention symlink guard: {msg}");
    }

    /// An invalid track ID must be rejected by the interactor's validation.
    #[test]
    fn test_invalid_track_id_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        // No architecture-rules.json — but invalid track ID fails before file I/O.
        let result = execute_catalogue_impl_signals(
            "bad track id!!".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "invalid track ID must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("invalid track id") || msg.contains("invalid track ID"),
            "error must mention invalid track id: {msg}"
        );
    }

    /// Missing architecture-rules.json at workspace_root must produce an error
    /// (fail-closed: the layer-bindings port cannot enumerate TDDD layers without it).
    #[test]
    fn test_missing_architecture_rules_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        // workspace_root is a real directory (not a symlink) but has no architecture-rules.json.
        let result = execute_catalogue_impl_signals(
            "test-track-2026-01-01".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "missing architecture-rules.json must return Err");
    }

    /// Symlinked `track/items` directory must be rejected by the items_dir guard.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_items_dir_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_items = tmp.path().join("real_items");
        std::fs::create_dir_all(&real_items).unwrap();
        // Create workspace_root/track/ and symlink items → real_items.
        let track_dir = tmp.path().join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let items_link = track_dir.join("items");
        std::os::unix::fs::symlink(&real_items, &items_link).unwrap();

        let result = execute_catalogue_impl_signals(
            "test-track-2026-01-01".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "symlinked track/items must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("symlink guard"), "error message must mention symlink guard: {msg}");
    }

    // ── report_has_red_signals unit tests ──────────────────────────────────

    /// A report with no signal table rows must return false.
    #[test]
    fn test_report_has_red_signals_empty_report_returns_false() {
        assert!(!report_has_red_signals(""), "empty report must not indicate red signals");
    }

    /// A report with only Blue/Yellow table rows must return false.
    #[test]
    fn test_report_has_red_signals_blue_yellow_only_returns_false() {
        let report = "\n## Layer: `domain`\n\n\
                      | Item | Region | Signal |\n\
                      |------|--------|--------|\n\
                      | Foo | SCaptureDCaptureIntersect | 🔵 Blue |\n\
                      | Bar | SCaptureDCaptureIntersect | 🟡 Yellow |\n\
                      \nSummary: 🔵 1 Blue | 🟡 1 Yellow | 🔴 0 Red\n";
        assert!(
            !report_has_red_signals(report),
            "report with only Blue/Yellow signals must return false"
        );
    }

    /// A report with at least one Red table row must return true.
    #[test]
    fn test_report_has_red_signals_with_red_row_returns_true() {
        let report = "\n## Layer: `domain`\n\n\
                      | Item | Region | Signal |\n\
                      |------|--------|--------|\n\
                      | Foo | SCaptureDCaptureIntersect | 🔵 Blue |\n\
                      | Baz | SCaptureDCaptureIntersect | 🔴 Red |\n\
                      \nSummary: 🔵 1 Blue | 🟡 0 Yellow | 🔴 1 Red\n";
        assert!(report_has_red_signals(report), "report with a Red table row must return true");
    }

    /// Verify that the summary line `🔴 0 Red` does NOT trigger a false positive.
    #[test]
    fn test_report_has_red_signals_summary_zero_red_is_not_false_positive() {
        // The summary line has `🔴 0 Red` (no space before 0), which must not
        // match the cell marker `| 🔴 Red |`.
        let report = "Summary: 🔵 2 Blue | 🟡 1 Yellow | 🔴 0 Red\n";
        assert!(
            !report_has_red_signals(report),
            "summary-only `🔴 0 Red` must not be treated as a red signal"
        );
    }
}
