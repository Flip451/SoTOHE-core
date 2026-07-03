//! `catalogue-lint` subcommands for the `sotp` CLI.
//!
//! Provides:
//! - `check-active-track`: run the default-config catalogue lint ruleset across
//!   every `tddd.enabled` layer of the active track and exit non-zero if any
//!   layer reports a violation. Reuses the same active-track-resolution
//!   mechanism already used by `sotp track lint` / `sotp signal
//!   calc-impl-catalog` (CN-07: no new track-scoping logic).
//!
//! ADR `knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md`
//! §D5: blocking from day one, no warn→block staged migration — this command
//! is wired into `track-active-gate` (see `Makefile.toml`) so it runs on every
//! commit/review cycle from the moment this track lands.
//!
//! All composition (adapter construction, interactor wiring, layer
//! enumeration) lives in `cli_composition`; this module is a thin
//! arg-parsing layer that hands off to the `track` primary-adapter driver
//! (`cli_driver::track::TrackDriver`) — it never calls `cli_composition`
//! workflow methods directly (CN-01 / CN-02).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TrackCompositionRoot;
use cli_driver::track::TrackInput;

use crate::commands::driver_outcome_to_exit;

// ── sotp catalogue-lint ─────────────────────────────────────────────────────

/// Subcommands for `sotp catalogue-lint`.
#[derive(Debug, Clone, Subcommand)]
pub enum CatalogueLintCommand {
    /// Run the catalogue lint ruleset across every `tddd.enabled` layer of the
    /// active track and exit non-zero if any layer reports a violation.
    ///
    /// Active-track resolution: when `--track-id` is omitted, the track is
    /// auto-resolved from the current git branch (`track/<id>`), the same
    /// mechanism `sotp track lint` already uses. Non-track branches fail
    /// closed (non-zero exit), matching the existing convention.
    ///
    /// Layers whose catalogue file does not exist yet (track has not
    /// finished Phase 2 `type-design` for that layer) cause the whole gate
    /// to be skipped for this run (exit 0) rather than erroring, since
    /// `CatalogueLoader::load_all` requires every `tddd.enabled` layer's
    /// catalogue file to be present.
    ///
    /// Exits 0 when zero violations are found (or the gate is skipped);
    /// exits 1 when one or more violations are found, or when the lint
    /// config is missing.
    CheckActiveTrack(CatalogueLintCheckActiveTrackArgs),
}

// ── sotp catalogue-lint check-active-track ──────────────────────────────────

/// Arguments for `sotp catalogue-lint check-active-track`.
///
/// `track_id` is optional; when omitted, the active track is auto-resolved
/// from the current git branch (`track/<id>`), matching the convention of
/// `sotp track lint` and other track-aware commands. `workspace_root`
/// defaults to `.` (the current directory). `rules_file` optionally
/// overrides the default lint config location
/// (`.harness/catalogue-lint/config.json`).
#[derive(Debug, Clone, Args)]
pub struct CatalogueLintCheckActiveTrackArgs {
    /// Active track identifier. When omitted, auto-resolved from the current
    /// git branch (only `track/<id>` branches are accepted).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Workspace root directory (contains `architecture-rules.json` and
    /// `track/items/`).
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    /// Optional override for the lint config file path (defaults to
    /// `.harness/catalogue-lint/config.json` under `workspace_root`).
    #[arg(long)]
    pub rules_file: Option<PathBuf>,
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

/// Dispatch `sotp catalogue-lint <subcommand>` to the appropriate execute_* handler.
pub fn execute(cmd: CatalogueLintCommand) -> ExitCode {
    match cmd {
        CatalogueLintCommand::CheckActiveTrack(args) => execute_check_active_track(args),
    }
}

/// Execute `sotp catalogue-lint check-active-track`.
///
/// Routes through the `track` primary-adapter driver
/// ([`cli_driver::track::TrackDriver`]) like every other `track`-family
/// subcommand, rather than calling the composition root directly. The driver
/// dispatches [`TrackInput::CatalogueLintCheckActiveTrack`] to
/// [`TrackCompositionRoot::catalogue_lint_check_active_track`] (via the
/// injected `TrackService`), which resolves the active track, enumerates
/// `tddd.enabled` layers from `architecture-rules.json`, and aggregates
/// violations across all of them.
pub fn execute_check_active_track(args: CatalogueLintCheckActiveTrackArgs) -> ExitCode {
    let outcome = TrackCompositionRoot::new().track_driver().handle(
        TrackInput::CatalogueLintCheckActiveTrack {
            track_id: args.track_id,
            workspace_root: args.workspace_root,
            rules_file: args.rules_file,
        },
    );
    driver_outcome_to_exit(outcome)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Thin clap wrapper for parsing `sotp catalogue-lint <subcmd>` in tests.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: CatalogueLintCommand,
    }

    fn parse_catalogue_lint(args: &[&str]) -> CatalogueLintCommand {
        TestCli::parse_from(args).cmd
    }

    #[test]
    fn test_check_active_track_parses_track_id_arg() {
        let cmd = parse_catalogue_lint(&[
            "catalogue-lint",
            "check-active-track",
            "--track-id",
            "my-track",
        ]);
        match cmd {
            CatalogueLintCommand::CheckActiveTrack(args) => {
                assert_eq!(args.track_id, Some("my-track".to_owned()));
                assert_eq!(args.workspace_root, PathBuf::from("."));
                assert_eq!(args.rules_file, None);
            }
        }
    }

    #[test]
    fn test_check_active_track_omitting_track_id_is_accepted() {
        // --track-id is optional; omitting it triggers auto-resolution from
        // the current git branch (`track/<id>`) at runtime. Clap-level parse
        // must accept this; resolution itself is exercised by integration tests.
        let result = TestCli::try_parse_from(["catalogue-lint", "check-active-track"]);
        assert!(result.is_ok(), "--track-id is optional; omitting it should be accepted");
        match result.unwrap().cmd {
            CatalogueLintCommand::CheckActiveTrack(args) => {
                assert_eq!(args.track_id, None, "omitting --track-id must yield None");
            }
        }
    }

    #[test]
    fn test_check_active_track_parses_custom_workspace_root_and_rules_file() {
        let cmd = parse_catalogue_lint(&[
            "catalogue-lint",
            "check-active-track",
            "--workspace-root",
            "custom/root",
            "--rules-file",
            "custom/rules.json",
        ]);
        match cmd {
            CatalogueLintCommand::CheckActiveTrack(args) => {
                assert_eq!(args.workspace_root, PathBuf::from("custom/root"));
                assert_eq!(args.rules_file, Some(PathBuf::from("custom/rules.json")));
            }
        }
    }

    #[test]
    fn test_catalogue_lint_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["catalogue-lint", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized catalogue-lint subcommand must be rejected by clap");
    }
}
