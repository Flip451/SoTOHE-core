#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::Command;

use cli_composition::AdrBaselineCompositionRoot;
use cli_driver::adr_baseline::{
    AdrBaselineInput, AdrBaselineSnapshotInput, AdrSourceFileNameInput, TrackIdInput,
};

fn initialize_git_repository(root: &std::path::Path) {
    for arguments in [
        &["init", "-q"][..],
        &["config", "user.email", "test@example.invalid"][..],
        &["config", "user.name", "Test User"][..],
    ] {
        let status = Command::new("git").args(arguments).current_dir(root).status().unwrap();
        assert!(status.success(), "git command must succeed: {}", arguments.join(" "));
    }
}

#[test]
fn test_adr_baseline_composed_clock_path_preserves_cli_contract() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    initialize_git_repository(root);
    std::fs::create_dir_all(root.join("knowledge/adr")).unwrap();
    std::fs::write(root.join("knowledge/adr/decision.md"), "# Decision\n").unwrap();

    let outcome = AdrBaselineCompositionRoot::new().adr_baseline_driver(root.into()).handle(
        AdrBaselineInput::Snapshot {
            track_id: "fixture-track".parse::<TrackIdInput>().unwrap(),
            source: "decision.md".parse::<AdrSourceFileNameInput>().unwrap(),
            kind: AdrBaselineSnapshotInput::Init,
        },
    );

    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.stderr, None);
    assert!(matches!(
        outcome.stdout.as_deref(),
        Some(output) if output.starts_with("ADR baseline snapshot: SnapshotRecorded")
    ));
}
