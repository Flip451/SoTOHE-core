//! `sotp adr-baseline` command boundary.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::{AdrBaselineCompositionRoot, TrackCompositionRoot};
use cli_driver::adr_baseline::{
    AdrBaselineInput, AdrBaselineKindInput, AdrBaselineReasonInput, AdrBaselineSnapshotInput,
    AdrSourceFileNameInput, TrackIdInput,
};
use cli_driver::track_resolution::{
    TrackItemsDirectoryInput, TrackResolutionInput, TrackResolutionOutcome,
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
    let items_dir_input = TrackItemsDirectoryInput::try_new(items_dir.clone())
        .map_err(|error| CliError::Message(error.message().to_owned()))?;
    let project_root = items_dir_input.workspace_root().into_path();
    Ok(AdrBaselineCompositionRoot::new().adr_baseline_driver(project_root).handle(input))
}

fn resolve_for_write(
    track_root: &TrackCompositionRoot,
    track_id: Option<TrackIdInput>,
    items_dir: &Path,
) -> Result<TrackIdInput, CliError> {
    let items_dir = TrackItemsDirectoryInput::try_new(items_dir.to_path_buf())
        .map_err(|error| CliError::Message(error.message().to_owned()))?;
    match track_root
        .track_resolution_driver()
        .resolve(TrackResolutionInput::WriteFromItems { track_id, items_dir })
    {
        TrackResolutionOutcome::Resolved(track_id) => Ok(track_id),
        TrackResolutionOutcome::Inactive => Err(CliError::Message("no active track".to_owned())),
        TrackResolutionOutcome::Failed(error) => Err(CliError::Message(error.message().to_owned())),
    }
}

fn resolve_for_read(
    track_root: &TrackCompositionRoot,
    track_id: Option<TrackIdInput>,
    items_dir: &Path,
) -> Result<TrackIdInput, CliError> {
    let items_dir = TrackItemsDirectoryInput::try_new(items_dir.to_path_buf())
        .map_err(|error| CliError::Message(error.message().to_owned()))?;
    match track_root
        .track_resolution_driver()
        .resolve(TrackResolutionInput::ReadFromItems { track_id, items_dir })
    {
        TrackResolutionOutcome::Resolved(track_id) => Ok(track_id),
        TrackResolutionOutcome::Inactive => Err(CliError::Message("no active track".to_owned())),
        TrackResolutionOutcome::Failed(error) => Err(CliError::Message(error.message().to_owned())),
    }
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

        let resolved =
            resolve_for_read(&track_root, Some(explicit_id), Path::new("track/items")).unwrap();

        assert_eq!(resolved.as_ref(), "fixture-track");
    }

    #[test]
    fn test_adr_baseline_rejects_parent_traversal_with_typed_input_diagnostic() {
        let items_dir = PathBuf::from("nested/../track/items");
        let expected =
            "track items directory must not contain parent traversal: nested/../track/items";

        let input_error = TrackItemsDirectoryInput::try_new(items_dir.clone()).unwrap_err();
        assert_eq!(input_error.message(), expected);

        let (exit_code, diagnostic) =
            execute_with_error_chain(AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs {
                items_dir,
                track_id: Some("fixture-track".parse().unwrap()),
            }));
        assert_eq!(exit_code, ExitCode::FAILURE);
        assert_eq!(diagnostic.as_deref(), Some(expected));
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

        assert!(write_resolver_source.contains("TrackResolutionInput::WriteFromItems"));
        assert!(read_resolver_source.contains("TrackResolutionInput::ReadFromItems"));
        for resolver_source in [write_resolver_source, read_resolver_source] {
            assert!(resolver_source.contains("TrackItemsDirectoryInput::try_new"));
            assert!(resolver_source.contains("CliError::Message(error.message().to_owned())"));
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
        assert!(dispatch_source.contains("TrackItemsDirectoryInput::try_new(items_dir.clone())"));
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
        let expected = "--items-dir must point to '<project-root>/track/items'; got fixture/items";
        let (exit_code, diagnostic) =
            execute_with_error_chain(AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs {
                items_dir: PathBuf::from("fixture/items"),
                track_id: Some("fixture-track".parse().unwrap()),
            }));

        assert_eq!(exit_code, ExitCode::FAILURE);
        assert_eq!(diagnostic.as_deref(), Some(expected));

        let input_error =
            TrackItemsDirectoryInput::try_new(PathBuf::from("fixture/items")).unwrap_err();
        assert_eq!(input_error.message(), expected);
    }

    #[test]
    fn test_track_resolution_input_variants_preserve_cli_args_exit_and_persistence() {
        use cli_composition::TrackCompositionRoot;
        use cli_driver::track_resolution::{
            TrackItemsDirectoryInput, TrackResolutionInput, TrackResolutionOutcome,
            TrackWorkspaceRootInput,
        };

        let expected = "--items-dir must point to '<project-root>/track/items'; got fixture/items";
        let items_error =
            TrackItemsDirectoryInput::try_new(PathBuf::from("fixture/items")).unwrap_err();
        assert_eq!(items_error.message(), expected);

        let (exit_code, diagnostic) =
            execute_with_error_chain(AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs {
                items_dir: PathBuf::from("fixture/items"),
                track_id: Some("fixture-track".parse().unwrap()),
            }));
        assert_eq!(exit_code, ExitCode::FAILURE);
        assert_eq!(diagnostic.as_deref(), Some(expected));

        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        let run_git = |arguments: &[&str]| {
            let status = Command::new("git").args(arguments).current_dir(root).status().unwrap();
            assert!(status.success(), "git command failed: git {}", arguments.join(" "));
        };
        run_git(&["init"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test User"]);
        fs::create_dir_all(root.join("track/items")).unwrap();
        fs::create_dir_all(root.join("knowledge/adr")).unwrap();
        fs::write(root.join("track/items/.keep"), "").unwrap();
        fs::write(root.join("knowledge/adr/decision.md"), "# Decision\n").unwrap();
        run_git(&["add", "."]);
        run_git(&["commit", "-m", "fixture"]);
        run_git(&["checkout", "-b", "track/input-variants"]);

        let items_dir = TrackItemsDirectoryInput::try_new(root.join("track/items")).unwrap();
        let workspace_root = TrackWorkspaceRootInput::try_new(root.to_path_buf()).unwrap();
        let driver = TrackCompositionRoot::new().track_resolution_driver();
        let resolved = |outcome| match outcome {
            TrackResolutionOutcome::Resolved(track_id) => track_id.to_string(),
            other => panic!("expected resolved track, got {other:?}"),
        };

        assert_eq!(
            resolved(driver.resolve(TrackResolutionInput::ReadFromItems {
                track_id: None,
                items_dir: items_dir.clone(),
            })),
            "input-variants"
        );
        assert_eq!(
            resolved(
                driver.resolve(TrackResolutionInput::WriteFromItems { track_id: None, items_dir })
            ),
            "input-variants"
        );
        assert_eq!(
            resolved(driver.resolve(TrackResolutionInput::ReadFromRoot {
                track_id: None,
                workspace_root: workspace_root.clone(),
            })),
            "input-variants"
        );
        assert_eq!(
            resolved(driver.resolve(TrackResolutionInput::WriteFromRoot {
                track_id: None,
                workspace_root: workspace_root.clone(),
            })),
            "input-variants"
        );
        assert_eq!(
            resolved(driver.resolve(TrackResolutionInput::DetectActive { workspace_root })),
            "input-variants"
        );

        let outcome = dispatch(AdrBaselineCommand::Snapshot(AdrBaselineSnapshotArgs {
            items_dir: root.join("track/items"),
            track_id: Some("input-variants".parse().unwrap()),
            source: "decision.md".parse().unwrap(),
            kind: "init".parse().unwrap(),
            reason: None,
        }))
        .unwrap();
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr.as_deref(), None);
        let stdout = outcome.stdout.as_deref().unwrap();
        assert_eq!(
            stdout,
            stdout
                .strip_prefix("ADR baseline snapshot: SnapshotRecorded")
                .map(|suffix| format!("ADR baseline snapshot: SnapshotRecorded{suffix}"))
                .as_deref()
                .unwrap()
        );
        assert!(
            !stdout.contains('\n'),
            "snapshot success stdout must stay a single unchanged line"
        );
        let ledger =
            fs::read_to_string(root.join("track/items/input-variants/adr-baseline/ledger.jsonl"))
                .unwrap();
        assert!(ledger.contains("\"source\":\"decision.md\""));
        assert!(ledger.contains("\"kind\":\"init\""));
    }

    #[test]
    fn test_track_resolution_input_and_driver_preserve_adr_baseline_cli_contract() {
        let expected = "--items-dir must point to '<project-root>/track/items'; got fixture/items";
        let items_dir = match TrackItemsDirectoryInput::try_new(PathBuf::from("fixture/items")) {
            Ok(_) => {
                panic!("noncanonical items dir must fail before TrackResolutionDriver::resolve")
            }
            Err(error) => error,
        };
        assert_eq!(items_dir.message(), expected);

        let (exit_code, diagnostic) =
            execute_with_error_chain(AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs {
                items_dir: PathBuf::from("fixture/items"),
                track_id: Some("fixture-track".parse().unwrap()),
            }));
        assert_eq!(exit_code, ExitCode::FAILURE);
        assert_eq!(diagnostic.as_deref(), Some(expected));
        assert_eq!(format!("{exit_code:?}"), format!("{:?}", std::process::ExitCode::FAILURE));
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
