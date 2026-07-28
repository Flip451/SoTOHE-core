//! `sotp adr-baseline` command boundary.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::{AdrBaselineCompositionRoot, TrackCompositionRoot};
use cli_driver::adr_baseline::{
    AdrBaselineInput, AdrBaselineKindInput, AdrBaselineReasonInput, AdrBaselineSnapshotInput,
    AdrSourceFileNameInput, TrackIdInput,
};

use crate::CliError;

/// ADR baseline CLI subcommand family.
#[derive(Debug, Clone, Subcommand)]
pub enum AdrBaselineCommand {
    /// Atomically record an ADR baseline snapshot and ledger entry.
    Snapshot(AdrBaselineSnapshotArgs),
    /// Restore an ADR source from its latest recorded baseline.
    Restore(AdrBaselineRestoreArgs),
    /// Verify the primary ADR's init snapshot before review starts.
    CheckReview(AdrBaselineCheckReviewArgs),
    /// Verify recorded and required ADR baselines before commit.
    CheckCommit(AdrBaselineCheckCommitArgs),
}

/// ADR baseline snapshot CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineSnapshotArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
    /// Direct Markdown filename under `knowledge/adr`.
    #[arg(long)]
    pub source: AdrSourceFileNameInput,
    /// Snapshot kind: init, cite, new-adr, non-semantic-fix, or escalation.
    #[arg(long)]
    pub kind: AdrBaselineKindInput,
    /// Required for new-adr and escalation; forbidden for all other kinds.
    #[arg(long)]
    pub reason: Option<AdrBaselineReasonInput>,
}

/// ADR baseline restore CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineRestoreArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
    /// Direct Markdown filename under `knowledge/adr`.
    #[arg(long)]
    pub source: AdrSourceFileNameInput,
}

/// ADR baseline review-check CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineCheckReviewArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
    /// Optional primary ADR filename override; otherwise derives it from init records.
    #[arg(long)]
    pub primary_source: Option<AdrSourceFileNameInput>,
}

/// ADR baseline commit-check CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineCheckCommitArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
}

/// Executes the ADR baseline command family.
pub fn execute(cmd: AdrBaselineCommand) -> ExitCode {
    execute_with_error_chain(cmd).0
}

/// Executes the ADR baseline command while retaining a diagnostic for callers.
pub fn execute_with_error_chain(cmd: AdrBaselineCommand) -> (ExitCode, Option<String>) {
    match dispatch(cmd) {
        Ok(outcome) => {
            if let Some(stdout) = outcome.stdout {
                println!("{stdout}");
            }
            if let Some(stderr) = outcome.stderr {
                eprintln!("{stderr}");
                (ExitCode::from(outcome.exit_code), Some(stderr))
            } else {
                (ExitCode::from(outcome.exit_code), None)
            }
        }
        Err(error) => {
            eprintln!("{error}");
            (ExitCode::FAILURE, Some(error.to_string()))
        }
    }
}

fn dispatch(cmd: AdrBaselineCommand) -> Result<cli_driver::CommandOutcome, CliError> {
    let track_root = TrackCompositionRoot::new();
    let (items_dir, input) = match cmd {
        AdrBaselineCommand::Snapshot(AdrBaselineSnapshotArgs {
            items_dir,
            track_id,
            source,
            kind,
            reason,
        }) => {
            let input = AdrBaselineInput::Snapshot {
                track_id: resolve_for_write(&track_root, track_id, &items_dir)?,
                source,
                kind: AdrBaselineSnapshotInput::try_from((kind, reason))
                    .map_err(|error| CliError::Message(error.to_string()))?,
            };
            (items_dir, input)
        }
        AdrBaselineCommand::Restore(AdrBaselineRestoreArgs { items_dir, track_id, source }) => {
            let input = AdrBaselineInput::Restore {
                track_id: resolve_for_write(&track_root, track_id, &items_dir)?,
                source,
            };
            (items_dir, input)
        }
        AdrBaselineCommand::CheckReview(AdrBaselineCheckReviewArgs {
            items_dir,
            track_id,
            primary_source,
        }) => {
            let input = AdrBaselineInput::CheckReview {
                track_id: resolve_for_read(&track_root, track_id, &items_dir)?,
                primary_source,
            };
            (items_dir, input)
        }
        AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs { items_dir, track_id }) => {
            let input = AdrBaselineInput::CheckCommit {
                track_id: resolve_for_read(&track_root, track_id, &items_dir)?,
            };
            (items_dir, input)
        }
    };
    let project_root = track_root
        .track_resolve_project_root(items_dir)
        .map_err(|error| CliError::Message(error.to_string()))?;
    Ok(AdrBaselineCompositionRoot::new().adr_baseline_driver(project_root).handle(input))
}

fn resolve_for_write(
    track_root: &TrackCompositionRoot,
    track_id: Option<TrackIdInput>,
    items_dir: &std::path::Path,
) -> Result<TrackIdInput, CliError> {
    track_root
        .track_resolve_id_for_write(track_id.map(|id| id.to_string()), items_dir.to_path_buf())
        .map_err(|error| CliError::Message(error.to_string()))?
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(error.to_string()))
}

fn resolve_for_read(
    track_root: &TrackCompositionRoot,
    track_id: Option<TrackIdInput>,
    items_dir: &std::path::Path,
) -> Result<TrackIdInput, CliError> {
    track_root
        .track_resolve_id(track_id.map(|id| id.to_string()), items_dir.to_path_buf())
        .map_err(|error| CliError::Message(error.to_string()))?
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(error.to_string()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::process::Command;

    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: AdrBaselineCommand,
    }

    #[test]
    fn test_adr_baseline_snapshot_parses_typed_arguments() {
        let cli = TestCli::parse_from([
            "adr-baseline",
            "snapshot",
            "--source",
            "decision.md",
            "--kind",
            "new-adr",
            "--reason",
            "user approved this ADR",
        ]);
        assert!(matches!(
            cli.command,
            AdrBaselineCommand::Snapshot(AdrBaselineSnapshotArgs { track_id: None, .. })
        ));
    }

    #[test]
    fn test_adr_baseline_check_review_parses_optional_primary_source() {
        let derived = TestCli::parse_from(["adr-baseline", "check-review"]);
        assert!(matches!(
            derived.command,
            AdrBaselineCommand::CheckReview(AdrBaselineCheckReviewArgs {
                primary_source: None,
                ..
            })
        ));

        let explicit = TestCli::parse_from([
            "adr-baseline",
            "check-review",
            "--primary-source",
            "decision.md",
        ]);
        assert!(matches!(
            explicit.command,
            AdrBaselineCommand::CheckReview(AdrBaselineCheckReviewArgs {
                primary_source: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn test_adr_baseline_arguments_reject_invalid_filename_and_kind() {
        assert!(
            TestCli::try_parse_from([
                "adr-baseline",
                "snapshot",
                "--source",
                "../decision.md",
                "--kind",
                "init"
            ])
            .is_err()
        );
        assert!(
            TestCli::try_parse_from([
                "adr-baseline",
                "snapshot",
                "--source",
                "decision.md",
                "--kind",
                "unknown"
            ])
            .is_err()
        );
    }

    #[test]
    fn test_resolve_for_read_explicit_track_id_returns_typed_id() {
        let track_root = TrackCompositionRoot::new();
        let explicit_id: TrackIdInput = "fixture-track".parse().unwrap();

        let resolved = resolve_for_read(
            &track_root,
            Some(explicit_id),
            std::path::Path::new("noncanonical/items-dir"),
        )
        .unwrap();

        assert_eq!(resolved.as_ref(), "fixture-track");
    }

    #[test]
    fn test_resolvers_map_invalid_items_dir_to_cli_error() {
        let track_root = TrackCompositionRoot::new();
        let invalid_items_dir = std::path::Path::new("fixture/items");
        let expected = "--items-dir must point to '<project-root>/track/items'; got fixture/items";

        let read_result = resolve_for_read(&track_root, None, invalid_items_dir);
        assert!(matches!(
            read_result,
            Err(CliError::Message(message)) if message == expected
        ));

        let write_result = resolve_for_write(
            &track_root,
            Some("fixture-track".parse().unwrap()),
            invalid_items_dir,
        );
        assert!(matches!(
            write_result,
            Err(CliError::Message(message)) if message == expected
        ));
    }

    #[test]
    fn test_resolvers_and_dispatch_preserve_pure_di_path() {
        let source = include_str!("adr_baseline.rs");
        let dispatch_source = source
            .split("fn dispatch(cmd: AdrBaselineCommand)")
            .nth(1)
            .unwrap()
            .split("fn resolve_for_write(")
            .next()
            .unwrap();
        let write_resolver_source = source
            .split("fn resolve_for_write(")
            .nth(1)
            .unwrap()
            .split("fn resolve_for_read(")
            .next()
            .unwrap();
        let read_resolver_source = source
            .split("fn resolve_for_read(")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(write_resolver_source.contains("track_resolve_id_for_write("));
        assert!(read_resolver_source.contains("track_resolve_id("));
        for resolver_source in [write_resolver_source, read_resolver_source] {
            assert!(resolver_source.contains("items_dir.to_path_buf()"));
            assert!(resolver_source.contains(".parse::<TrackIdInput>()"));
            assert!(resolver_source.contains("CliError::Message(error.to_string())"));
            for forbidden_runtime_path in [
                "AdrBaselineCompositionRoot",
                "CommandOutcome",
                ".handle(",
                "std::fs::",
                "std::process::",
                "std::net::",
                "std::io::",
                "println!",
                "eprintln!",
                "print!",
                "eprint!",
                "ServiceImpl",
                "CompatibilityShim",
                "CompatService",
                "usecase::",
            ] {
                assert!(
                    !resolver_source.contains(forbidden_runtime_path),
                    "track resolution must not execute or delegate through {forbidden_runtime_path}"
                );
            }
        }

        for command_destructure in [
            "AdrBaselineCommand::Snapshot(AdrBaselineSnapshotArgs {",
            "AdrBaselineCommand::Restore(AdrBaselineRestoreArgs { items_dir, track_id, source })",
            "AdrBaselineCommand::CheckReview(AdrBaselineCheckReviewArgs {",
            "AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs { items_dir, track_id })",
        ] {
            assert!(
                dispatch_source.contains(command_destructure),
                "dispatch must destructure each command variant in its single move"
            );
        }
        assert!(dispatch_source.contains("resolve_for_write(&track_root, track_id, &items_dir)?"));
        assert!(dispatch_source.contains("resolve_for_read(&track_root, track_id, &items_dir)?"));
        assert!(dispatch_source.contains("track_resolve_project_root(items_dir)"));
        assert!(!dispatch_source.contains("fn items_dir("));
        assert!(dispatch_source.contains(
            "AdrBaselineCompositionRoot::new().adr_baseline_driver(project_root).handle(input)"
        ));
        for forbidden_runtime_path in [
            "std::fs::",
            "std::process::",
            "std::net::",
            "std::io::",
            "println!",
            "eprintln!",
            "print!",
            "eprint!",
            "ServiceImpl",
            "CompatibilityShim",
            "CompatService",
            "usecase::",
        ] {
            assert!(
                !dispatch_source.contains(forbidden_runtime_path),
                "dispatch must remain a CLI-to-driver boundary without {forbidden_runtime_path}"
            );
        }
    }

    #[test]
    fn test_adr_baseline_execution_preserves_resolution_error_diagnostic_and_exit_code() {
        let (exit_code, diagnostic) =
            execute_with_error_chain(AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs {
                items_dir: PathBuf::from("fixture/items"),
                track_id: Some("fixture-track".parse().unwrap()),
            }));

        assert_eq!(exit_code, ExitCode::FAILURE);
        assert_eq!(
            diagnostic.as_deref(),
            Some("--items-dir must point to '<project-root>/track/items'; got fixture/items")
        );
    }

    #[test]
    fn test_adr_baseline_snapshot_valid_repository_persists_ledger_and_success_outcome() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        let run_git = |arguments: &[&str]| {
            let status = Command::new("git").args(arguments).current_dir(root).status().unwrap();
            assert!(status.success(), "git command failed: git {}", arguments.join(" "));
        };

        run_git(&["init"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test User"]);
        fs::create_dir_all(root.join("knowledge/adr")).unwrap();
        fs::write(root.join("knowledge/adr/decision.md"), "# Decision\n").unwrap();
        run_git(&["add", "."]);
        run_git(&["commit", "-m", "initial fixture"]);
        run_git(&["checkout", "-b", "track/fixture-track"]);

        let outcome = dispatch(AdrBaselineCommand::Snapshot(AdrBaselineSnapshotArgs {
            items_dir: root.join("track/items"),
            track_id: Some("fixture-track".parse().unwrap()),
            source: "decision.md".parse().unwrap(),
            kind: "init".parse().unwrap(),
            reason: None,
        }))
        .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(matches!(
            outcome.stdout.as_deref(),
            Some(output) if output.starts_with("ADR baseline snapshot: SnapshotRecorded")
        ));
        let baseline_dir = root.join("track/items/fixture-track/adr-baseline");
        let ledger = fs::read_to_string(baseline_dir.join("ledger.jsonl")).unwrap();
        assert!(ledger.contains("\"source\":\"decision.md\""));
        assert!(ledger.contains("\"kind\":\"init\""));
        assert!(
            fs::read_dir(&baseline_dir)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with("decision.")),
            "snapshot copy must be persisted beside the ledger"
        );
    }
}
