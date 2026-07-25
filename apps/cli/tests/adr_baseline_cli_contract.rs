#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::Command;

fn sotp_bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sotp"));
    command.env("SOTP_TELEMETRY", "0");
    command
}

#[test]
fn test_adr_baseline_check_review_missing_track_preserves_failure_cli_contract() {
    let project = tempfile::tempdir().unwrap();
    let items_dir = project.path().join("track/items");
    let track_dir = items_dir.join("missing-track");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        r#"{
  "schema_version": 6,
  "id": "missing-track",
  "branch": null,
  "title": "Test Track",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  }
}
"#,
    )
    .unwrap();

    let output = sotp_bin()
        .current_dir(project.path())
        .args([
            "adr-baseline",
            "check-review",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            "missing-track",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "ADR baseline check blocked: AdrBaselineCheckViolations { violations: [PrimaryInitUnavailable] }\n"
    );
}
