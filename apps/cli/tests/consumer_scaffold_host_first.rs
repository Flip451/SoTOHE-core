//! Exported-consumer regression tests for the host-first scaffold contract.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use tempfile::TempDir;

const RETIRED_PASSTHROUGH_TASKS: &[&str] = &[
    "add-all",
    "sync",
    "track-branch-create",
    "track-branch-switch",
    "track-switch-base",
    "track-pr-push",
    "track-pr",
    "track-pr-review",
    "track-add-paths",
    "track-note",
];

static EXPORTED_SCAFFOLD: OnceLock<TempDir> = OnceLock::new();

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn exported_scaffold() -> PathBuf {
    EXPORTED_SCAFFOLD
        .get_or_init(|| {
            let export_parent = tempfile::tempdir().unwrap();
            let root = workspace_root();
            let output_dir = export_parent.path().join("scaffold");
            let output = Command::new(env!("CARGO_BIN_EXE_sotp"))
                .env("SOTP_TELEMETRY", "0")
                .args([
                    "template",
                    "export",
                    "--workspace-root",
                    root.to_str().unwrap(),
                    "--manifest-path",
                    root.join(".harness/config/template-boundary.json").to_str().unwrap(),
                    "--overlay-dir",
                    root.join("overlay").to_str().unwrap(),
                    "--output-dir",
                    output_dir.to_str().unwrap(),
                ])
                .output()
                .unwrap();

            assert!(
                output.status.success(),
                "template export failed\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            export_parent
        })
        .path()
        .join("scaffold")
}

fn exported_file(relative_path: &str) -> String {
    let path = exported_scaffold().join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn task_names(makefile: &str) -> BTreeSet<&str> {
    makefile
        .lines()
        .filter_map(|line| line.strip_prefix("[tasks.").and_then(|name| name.strip_suffix(']')))
        .collect()
}

fn collect_text_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("json" | "md" | "rules" | "toml" | "yml" | "yaml")
        ) {
            files.push(path.to_path_buf());
        }
        return;
    }

    for entry in fs::read_dir(path).unwrap() {
        collect_text_files(&entry.unwrap().path(), files);
    }
}

fn assert_no_retired_passthrough_calls(surface_paths: &[&str]) {
    let root = exported_scaffold();
    let mut stale_calls = Vec::new();

    for surface_path in surface_paths {
        let mut files = Vec::new();
        collect_text_files(&root.join(surface_path), &mut files);
        for file in files {
            let content = fs::read_to_string(&file).unwrap();
            for task in RETIRED_PASSTHROUGH_TASKS {
                let call = format!("cargo make {task}");
                if content.contains(&call) {
                    stale_calls.push(format!("{} contains {call}", file.display()));
                }
            }
        }
    }

    assert!(
        stale_calls.is_empty(),
        "exported scaffold retains retired passthrough calls:\n{}",
        stale_calls.join("\n")
    );
}

#[test]
fn test_exported_scaffold_makefile_has_only_host_first_workflow_tasks() {
    let makefile = exported_file("Makefile.toml");
    let expected_tasks = BTreeSet::from([
        "bootstrap",
        "install-aux-tools",
        "install-sotp",
        "check-layers",
        "verify-arch-docs",
        "verify-doc-links",
        "verify-track-metadata",
        "verify-hooks-path",
        "verify-canonical-modules",
        "verify-latest-track",
        "verify-module-size",
        "verify-domain-strings",
        "verify-domain-purity",
        "verify-usecase-purity",
        "verify-view-freshness",
        "verify-plan-artifact-refs",
        "verify-adr-signals",
        "verify-spec-states-current",
        "signal-check-impl-catalog",
        "verify-catalogue-spec-refs",
        "check-catalogue-spec-signals",
        "task-contract-refresh-impl-catalog",
        "task-contract-coverage",
        "task-contract-check",
        "ci-rust",
        "ci",
        "ci-track",
        "track-active-gate",
        "track-local-review",
        "track-local-review-fix",
        "track-local-dry-fix",
        "track-commit-message",
    ]);

    assert_eq!(task_names(&makefile), expected_tasks);
    assert!(makefile.starts_with("extend = \"Makefile.host.toml\""));
    assert!(makefile.contains("[tasks.verify-track-metadata]"));
    assert_eq!(
        makefile
            .matches("args = [\"track\", \"views\", \"validate\", \"--project-root\", \".\"]")
            .count(),
        1
    );
}

#[test]
fn test_exported_workflows_call_sotp_directly_without_retired_wrappers() {
    assert_no_retired_passthrough_calls(&[".harness", ".claude", ".agents"]);

    let adr2pr = exported_file(".harness/workflows/track/adr2pr.md");
    let done = exported_file(".harness/workflows/track/done.md");
    let pr = exported_file(".claude/commands/track/pr.md");

    assert!(adr2pr.contains("bin/sotp git add-all"));
    assert!(done.contains("bin/sotp track switch-base"));
    assert!(pr.contains("bin/sotp pr push") && pr.contains("bin/sotp pr ensure-pr"));
}

#[test]
fn test_exported_environment_overlays_are_symmetric_and_personal_environment_free() {
    let common = exported_file("Makefile.toml");
    let host = exported_file("Makefile.host.toml");
    let docker = exported_file("Makefile.docker.toml");
    let host_gate_tasks = task_names(&host);
    let docker_gate_tasks = task_names(&docker)
        .into_iter()
        .filter(|task| *task != "prepare-local-cache-dirs")
        .collect::<BTreeSet<_>>();

    assert_eq!(host_gate_tasks, docker_gate_tasks);
    assert!(host.contains("command = \"cargo\"") && !host.contains("docker compose"));
    assert!(docker.contains("docker") && docker.contains("CARGO_TARGET_DIR_RELATIVE"));
    assert!(common.contains("CODEX_BIN=\"${CODEX_BIN:-$(command -v codex)}\""));
    for content in [&common, &host, &docker] {
        assert!(!content.contains("asdf"));
        assert!(!content.contains("WORKER_ID"));
    }
}

#[test]
fn test_exported_toolchain_ci_and_consumer_guidance_are_host_first() {
    let toolchain = exported_file("rust-toolchain.toml");
    let ci = exported_file(".github/workflows/ci.yml");
    let gitignore = exported_file(".gitignore");

    assert!(toolchain.contains("channel = \"1.94.0\""));
    assert!(toolchain.contains("components = [\"clippy\", \"rustfmt\"]"));
    assert!(ci.contains("runs-on: ubuntu-latest") && !ci.contains("docker compose"));
    for tool in [
        "cargo-make --version 0.37.24",
        "cargo-nextest --version 0.9.129",
        "cargo-deny --version 0.19.0",
    ] {
        assert!(ci.contains(tool) && ci.contains("cargo install --locked"));
    }
    assert!(ci.contains("path: .cargo-install"));
    assert!(ci.contains("steps.sotp-version.outputs.tag"));
    assert!(ci.contains("cargo make install-sotp"));
    assert!(ci.contains("cargo make ci") && ci.contains("cargo make ci-track"));
    assert!(gitignore.lines().any(|line| line == "/bin/sotp"));

    assert_no_retired_passthrough_calls(&[
        "README.md",
        "CLAUDE.md",
        ".claude/rules",
        "knowledge/conventions",
    ]);
}
