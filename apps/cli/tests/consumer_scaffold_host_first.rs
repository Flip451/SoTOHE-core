//! Exported-consumer regression tests for the host-first scaffold contract.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use tempfile::TempDir;

static EXPORTED_SCAFFOLD: OnceLock<TempDir> = OnceLock::new();

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn exported_scaffold() -> PathBuf {
    EXPORTED_SCAFFOLD
        .get_or_init(|| {
            let export_parent = tempfile::tempdir().unwrap();
            let output_dir = export_parent.path().join("scaffold");
            export_scaffold(&output_dir);
            export_parent
        })
        .path()
        .join("scaffold")
}

fn export_scaffold(output_dir: &Path) {
    let root = workspace_root();
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
}

fn isolated_git_command(program: &Path, global_config: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", global_config)
        .env("GIT_CONFIG_COUNT", "0");
    command
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

fn top_level_toml_string_value(content: &str, key: &str) -> String {
    let expected = format!("{key} = ");
    let mut in_table = false;

    for line in content.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_table = true;
        } else if !in_table && let Some(value) = line.strip_prefix(&expected) {
            return serde_json::from_str(value)
                .unwrap_or_else(|error| panic!("{key} must be a TOML basic string: {error}"));
        }
    }

    panic!("top-level TOML key {key} missing")
}

fn task_toml_value<'a>(makefile: &'a str, task_name: &str, key: &str) -> &'a str {
    let task_header = format!("[tasks.{task_name}]");
    let expected = format!("{key} = ");
    let mut in_task = false;

    for line in makefile.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_task = line == task_header;
        } else if in_task && let Some(value) = line.strip_prefix(&expected) {
            return value;
        }
    }

    panic!("TOML key {key} missing from task {task_name}")
}

fn task_toml_string_array(makefile: &str, task_name: &str, key: &str) -> Vec<String> {
    serde_json::from_str(task_toml_value(makefile, task_name, key)).unwrap_or_else(|error| {
        panic!("{key} for task {task_name} must be a TOML string array: {error}")
    })
}

fn toml_table_value<'a>(content: &'a str, table: &str, key: &str) -> &'a str {
    let table_header = format!("[{table}]");
    let expected = format!("{key} = ");
    let mut in_table = false;

    for line in content.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_table = line == table_header;
        } else if in_table && let Some(value) = line.strip_prefix(&expected) {
            return value;
        }
    }

    panic!("TOML key {key} missing from table {table}")
}

fn toml_table_string_value(content: &str, table: &str, key: &str) -> String {
    serde_json::from_str(toml_table_value(content, table, key)).unwrap_or_else(|error| {
        panic!("{key} in table {table} must be a TOML basic string: {error}")
    })
}

fn toml_table_string_array(content: &str, table: &str, key: &str) -> Vec<String> {
    serde_json::from_str(toml_table_value(content, table, key)).unwrap_or_else(|error| {
        panic!("{key} in table {table} must be a TOML string array: {error}")
    })
}

fn json_string_value(content: &str, key: &str) -> String {
    let document: serde_json::Value =
        serde_json::from_str(content).unwrap_or_else(|error| panic!("invalid JSON: {error}"));
    document
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("JSON string key {key} missing"))
        .to_owned()
}

fn gitignore_patterns(content: &str) -> BTreeSet<&str> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn trace_operation(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    let program = parts.next()?;
    let operation = match program {
        "git" => {
            let mut argument = parts.next()?;
            while argument.starts_with('-') {
                if argument == "-c" || argument == "--git-dir" {
                    parts.next()?;
                }
                argument = parts.next()?;
            }
            argument
        }
        "cargo" | "sotp" => parts.next()?,
        _ => return None,
    };
    Some((program, operation))
}

fn task_dependency_closure(makefile: &str, task_name: &str) -> BTreeSet<String> {
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    let mut current_task = None;

    for line in makefile.lines() {
        if let Some(name) = line.strip_prefix("[tasks.").and_then(|name| name.strip_suffix(']')) {
            current_task = Some(name);
        } else if let (Some(task), Some(values)) = (
            current_task,
            line.trim()
                .strip_prefix("dependencies = [")
                .and_then(|values| values.strip_suffix(']')),
        ) {
            dependencies.insert(
                task.to_owned(),
                values.split(',').map(|value| value.trim().trim_matches('"').to_owned()).collect(),
            );
        }
    }

    let mut closure = BTreeSet::new();
    let mut pending = vec![task_name.to_owned()];
    while let Some(task) = pending.pop() {
        if closure.insert(task.clone()) {
            pending.extend(dependencies.get(&task).into_iter().flatten().cloned());
        }
    }

    closure
}

#[test]
fn test_exported_scaffold_makefile_has_only_host_first_workflow_tasks() {
    let makefile = exported_file("Makefile.toml");
    let source_makefile = fs::read_to_string(workspace_root().join("Makefile.toml"))
        .expect("source Makefile.toml must be readable");
    let expected_tasks = BTreeSet::from([
        "cargo-make-lifecycle-init",
        "init",
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
        "task-contract-coverage-local",
        "task-contract-check-local",
        "ci-rust",
        "ci",
        "ci-track",
        "track-active-gate",
        "track-local-review",
        "track-local-review-fix",
        "track-local-dry-fix",
        "pr-audit-comment",
        "track-commit-message",
    ]);

    assert_eq!(task_names(&makefile), expected_tasks);
    assert!(
        !task_names(&source_makefile).contains("init"),
        "the init task must be available only in the exported overlay"
    );
    assert_eq!(top_level_toml_string_value(&makefile, "extend"), "Makefile.host.toml");
    assert_eq!(
        task_toml_string_array(&makefile, "verify-track-metadata", "args"),
        ["track", "views", "validate", "--project-root", "."]
    );
}

#[test]
fn test_exported_init_task_succeeds_with_global_hooks_path_and_repeat_rejects() {
    use std::os::unix::fs::PermissionsExt;

    let export_parent = tempfile::tempdir().unwrap();
    let real_git = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("Git must be available on PATH");
    let enclosing_repository = export_parent.path().join("enclosing-repository");
    let global_git_config = export_parent.path().join("gitconfig");
    fs::write(&global_git_config, "[user]\n\tuseConfigOnly = true\n").unwrap();
    fs::create_dir_all(&enclosing_repository).unwrap();
    for (args, writes_outer_file) in [
        (["init", "-b", "main"].as_slice(), false),
        (["add", "."].as_slice(), true),
        (["commit", "-m", "Outer commit"].as_slice(), false),
    ] {
        if writes_outer_file {
            fs::write(enclosing_repository.join("outer.txt"), "outer repository\n").unwrap();
        }
        let output = isolated_git_command(&real_git, &global_git_config)
            .args(args)
            .current_dir(&enclosing_repository)
            .env("GIT_AUTHOR_NAME", "Scaffold Test")
            .env("GIT_AUTHOR_EMAIL", "scaffold-test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Scaffold Test")
            .env("GIT_COMMITTER_EMAIL", "scaffold-test@example.invalid")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "failed to create enclosing repository with {args:?}: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    let scaffold = enclosing_repository.join("scaffold");
    export_scaffold(&scaffold);
    fs::write(
        &global_git_config,
        "[user]\n\tuseConfigOnly = true\n[core]\n\thooksPath = .githooks\n",
    )
    .unwrap();
    let shim_dir = export_parent.path().join("init-test-shim");
    fs::create_dir_all(&shim_dir).unwrap();

    let real_cargo = std::env::var_os("CARGO").expect("Cargo must provide its executable path");

    let git_shim = shim_dir.join("git");
    fs::write(
        &git_shim,
        "#!/bin/sh\nprintf 'git %s\\n' \"$*\" >> \"$INIT_TRACE\"\nexec \"$REAL_GIT\" \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&git_shim, fs::Permissions::from_mode(0o755)).unwrap();

    let cargo_shim = shim_dir.join("cargo");
    fs::write(
        &cargo_shim,
        "#!/bin/sh\ncase \"$1 $2\" in\n  'generate-lockfile ') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\" && printf 'generated lockfile\\n' > Cargo.lock ;;\n  'make bootstrap') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\"; if [ \"${FAIL_BOOTSTRAP:-0}\" = 1 ]; then echo 'simulated bootstrap failure' >&2; exit 72; fi; exec \"$REAL_CARGO\" make bootstrap ;;\n  'make install-aux-tools'|'make ci') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\" ;;\n  *) printf 'unexpected cargo command: %s\\n' \"$*\" >&2; exit 64 ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&cargo_shim, fs::Permissions::from_mode(0o755)).unwrap();

    let trace = shim_dir.join("init.trace");
    let sotp_shim = scaffold.join("bin/sotp");
    fs::write(
        &sotp_shim,
        "#!/bin/sh\nprintf 'sotp %s\\n' \"$*\" >> \"$INIT_TRACE\"\nif [ \"$1 $2\" = \"conventions update-index\" ]; then\n  printf 'generated convention index\\n' > knowledge/conventions/README.md\nfi\nif [ \"$1 $2 $3\" = \"hook dispatch git-ref-update\" ]; then\n  echo 'initial commit must not run inherited hooks' >&2\n  exit 73\nfi\n",
    )
    .unwrap();
    fs::set_permissions(&sotp_shim, fs::Permissions::from_mode(0o755)).unwrap();

    let run = |with_identity: bool, fail_bootstrap: bool| {
        let shimmed_path =
            format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default());
        let mut command = Command::new(&real_cargo);
        command
            .args(["make", "init"])
            .current_dir(&scaffold)
            .env("PATH", shimmed_path)
            .env("INIT_TRACE", &trace)
            .env("REAL_CARGO", &real_cargo)
            .env("REAL_GIT", &real_git)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &global_git_config)
            .env("GIT_CONFIG_COUNT", "0")
            .env("FAIL_BOOTSTRAP", if fail_bootstrap { "1" } else { "0" });
        if with_identity {
            command
                .env("GIT_AUTHOR_NAME", "Scaffold Test")
                .env("GIT_AUTHOR_EMAIL", "scaffold-test@example.invalid")
                .env("GIT_COMMITTER_NAME", "Scaffold Test")
                .env("GIT_COMMITTER_EMAIL", "scaffold-test@example.invalid");
        } else {
            command
                .env_remove("GIT_AUTHOR_NAME")
                .env_remove("GIT_AUTHOR_EMAIL")
                .env_remove("GIT_COMMITTER_NAME")
                .env_remove("GIT_COMMITTER_EMAIL")
                .env_remove("EMAIL");
        }
        command.output().unwrap()
    };

    let missing_identity = run(false, false);
    assert!(!missing_identity.status.success(), "an unconfigured host must fail closed");
    assert!(
        String::from_utf8_lossy(&missing_identity.stderr)
            .contains("requires a Git author and committer identity")
    );
    assert!(
        !scaffold.join(".git").exists(),
        "identity failure must roll back the repository created for identity resolution"
    );
    assert!(
        !scaffold.join("Cargo.lock").exists(),
        "identity preflight must not generate a lockfile"
    );
    let missing_identity_trace = fs::read_to_string(&trace).unwrap_or_default();
    assert!(
        !missing_identity_trace.contains("git -c core.hooksPath=/dev/null init -b main"),
        "identity validation must reject the host before Git initialization: {missing_identity_trace}"
    );
    for command_after_identity_check in [
        "cargo generate-lockfile",
        "git add -A",
        "git -c core.hooksPath=/dev/null commit",
        "cargo make bootstrap",
    ] {
        assert!(
            !missing_identity_trace.contains(command_after_identity_check),
            "identity failure must stop before {command_after_identity_check}: {missing_identity_trace}"
        );
    }
    assert!(
        missing_identity_trace.contains("git var GIT_AUTHOR_IDENT"),
        "identity validation must defer to Git's own author resolution: {missing_identity_trace}"
    );
    fs::remove_file(&trace).unwrap();

    let failed_without_lockfile = run(true, true);
    assert!(
        !failed_without_lockfile.status.success(),
        "bootstrap failure must fail initialization"
    );
    assert!(
        !scaffold.join(".git").exists(),
        "bootstrap failure must remove the repository it created"
    );
    assert!(
        !scaffold.join("Cargo.lock").exists(),
        "bootstrap failure without a pre-existing lockfile must not leave one behind"
    );
    assert!(
        fs::read_to_string(&trace).unwrap_or_default().contains("cargo make bootstrap"),
        "the injected bootstrap failure must be reached"
    );
    fs::remove_file(&trace).unwrap();

    let original_lockfile = "pre-existing lockfile\n";
    fs::write(scaffold.join("Cargo.lock"), original_lockfile).unwrap();
    let failed_with_lockfile = run(true, true);
    assert!(!failed_with_lockfile.status.success(), "bootstrap failure must fail initialization");
    assert!(
        !scaffold.join(".git").exists(),
        "bootstrap failure must remove the repository it created"
    );
    assert_eq!(
        fs::read_to_string(scaffold.join("Cargo.lock")).unwrap(),
        original_lockfile,
        "bootstrap failure must restore the pre-existing lockfile exactly"
    );
    assert!(
        fs::read_to_string(&trace).unwrap_or_default().contains("cargo make bootstrap"),
        "the injected bootstrap failure must be reached"
    );
    fs::remove_file(&trace).unwrap();
    fs::remove_file(scaffold.join("Cargo.lock")).unwrap();

    let first = run(true, false);
    assert!(
        first.status.success(),
        "first initialization must succeed\nstderr: {}\ntrace: {}",
        String::from_utf8_lossy(&first.stderr),
        fs::read_to_string(&trace).unwrap_or_default(),
    );
    let trace_lines =
        fs::read_to_string(&trace).unwrap().lines().map(str::to_owned).collect::<Vec<_>>();
    let expected_sequence = [
        "git var GIT_AUTHOR_IDENT",
        "git var GIT_COMMITTER_IDENT",
        "git -c core.hooksPath=/dev/null init -b main",
        "cargo generate-lockfile",
        "sotp conventions update-index",
        "git add -A",
        "git -c core.hooksPath=/dev/null commit -m Initial commit",
        "cargo make bootstrap",
    ];
    assert!(
        trace_lines
            .windows(expected_sequence.len())
            .any(|window| { window.iter().map(String::as_str).eq(expected_sequence) }),
        "the public task must execute the initialization sequence before bootstrap: {trace_lines:?}",
    );
    assert!(
        !trace_lines.iter().any(|line| line == "sotp hook dispatch git-ref-update"),
        "the initial commit must bypass inherited hooks: {trace_lines:?}",
    );
    let branch_output = isolated_git_command(&real_git, &global_git_config)
        .args(["branch", "--show-current"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert!(branch_output.status.success());
    assert_eq!(String::from_utf8_lossy(&branch_output.stdout).trim(), "main");
    let committed_lockfile = isolated_git_command(&real_git, &global_git_config)
        .args(["show", "HEAD:Cargo.lock"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert!(committed_lockfile.status.success());
    assert_eq!(String::from_utf8_lossy(&committed_lockfile.stdout), "generated lockfile\n");
    let committed_convention_index = isolated_git_command(&real_git, &global_git_config)
        .args(["show", "HEAD:knowledge/conventions/README.md"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert!(committed_convention_index.status.success());
    let regenerated_convention_index =
        fs::read_to_string(scaffold.join("knowledge/conventions/README.md")).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&committed_convention_index.stdout),
        regenerated_convention_index
    );
    let worktree_status = isolated_git_command(&real_git, &global_git_config)
        .args(["status", "--porcelain"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert!(worktree_status.status.success());
    assert!(
        worktree_status.stdout.is_empty(),
        "initialization must not leave generated convention-index changes uncommitted: {}",
        String::from_utf8_lossy(&worktree_status.stdout)
    );
    let hooks_path = isolated_git_command(&real_git, &global_git_config)
        .args(["config", "--local", "core.hooksPath"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert!(hooks_path.status.success());
    assert_eq!(String::from_utf8_lossy(&hooks_path.stdout).trim(), ".githooks");

    let branch = String::from_utf8_lossy(&branch_output.stdout).into_owned();
    let committed = String::from_utf8_lossy(&committed_lockfile.stdout).into_owned();
    let lockfile = fs::read_to_string(scaffold.join("Cargo.lock")).unwrap();
    fs::remove_file(&trace).unwrap();

    let repeat = run(true, false);
    assert!(!repeat.status.success(), "repeat initialization must fail closed");
    assert!(
        String::from_utf8_lossy(&repeat.stderr)
            .contains("before the repository has its first commit")
    );
    let repeat_trace = fs::read_to_string(&trace).unwrap_or_default();
    let repeat_operations = repeat_trace.lines().filter_map(trace_operation).collect::<Vec<_>>();
    assert!(repeat_operations.contains(&("git", "rev-parse")));
    for state_changing_command in [("git", "init"), ("git", "add"), ("git", "commit")] {
        assert!(
            !repeat_operations.contains(&state_changing_command),
            "repeat initialization must stop before {state_changing_command:?}: {repeat_trace}",
        );
    }
    assert!(
        !repeat_operations.iter().any(|(program, _)| *program == "cargo"),
        "repeat initialization must stop before cargo: {repeat_trace}",
    );
    let repeat_branch = isolated_git_command(&real_git, &global_git_config)
        .args(["branch", "--show-current"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&repeat_branch.stdout), branch);
    let repeat_committed = isolated_git_command(&real_git, &global_git_config)
        .args(["show", "HEAD:Cargo.lock"])
        .current_dir(&scaffold)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&repeat_committed.stdout), committed);
    assert_eq!(fs::read_to_string(scaffold.join("Cargo.lock")).unwrap(), lockfile);
}

#[test]
fn test_exported_branch_strategy_uses_overlay_main_defaults() {
    let source = fs::read_to_string(workspace_root().join(".harness/config/branch-strategy.json"))
        .expect("source branch strategy must be readable");
    let exported = exported_file(".harness/config/branch-strategy.json");

    assert_eq!(json_string_value(&source, "base_branch"), "develop");
    assert_eq!(json_string_value(&source, "merge_target"), "develop");
    assert_eq!(json_string_value(&exported, "base_branch"), "main");
    assert_eq!(json_string_value(&exported, "merge_target"), "main");
}

#[test]
fn test_exported_ci_track_avoids_nightly_refresh_dependency() {
    let makefile = exported_file("Makefile.toml");
    let closure = task_dependency_closure(&makefile, "ci-track");

    assert!(closure.contains("task-contract-coverage-local"));
    assert!(closure.contains("task-contract-check-local"));
    assert!(
        !closure.contains("task-contract-refresh-impl-catalog"),
        "ci-track must not depend on the nightly-required implementation-catalogue refresh: {closure:?}"
    );
}

#[test]
fn test_exported_environment_overlays_define_the_same_gate_tasks() {
    let host = exported_file("Makefile.host.toml");
    let docker = exported_file("Makefile.docker.toml");
    let host_gate_tasks = task_names(&host);
    let docker_gate_tasks = task_names(&docker)
        .into_iter()
        .filter(|task| *task != "prepare-local-cache-dirs")
        .collect::<BTreeSet<_>>();

    assert_eq!(host_gate_tasks, docker_gate_tasks);
}

#[test]
fn test_exported_toolchain_and_gitignore_have_contract_values() {
    let toolchain = exported_file("rust-toolchain.toml");
    let gitignore = exported_file(".gitignore");

    assert_eq!(toml_table_string_value(&toolchain, "toolchain", "channel"), "1.94.0");
    assert_eq!(
        toml_table_string_array(&toolchain, "toolchain", "components"),
        ["clippy", "rustfmt"]
    );
    assert!(gitignore_patterns(&gitignore).contains("/bin/sotp"));
}

fn pr_audit_comment_script(makefile: &str) -> String {
    let section_start =
        makefile.find("[tasks.pr-audit-comment]").expect("pr-audit-comment task missing");
    let section = &makefile[section_start..];
    let marker = "script = ['''";
    let script_start =
        section.find(marker).expect("pr-audit-comment script missing") + marker.len();
    let script_len =
        section[script_start..].find("''']").expect("pr-audit-comment script unterminated");
    section[script_start..script_start + script_len].to_string()
}

#[test]
fn test_exported_pr_audit_comment_wrapper_validates_argv_and_reaches_gh_once() {
    use std::os::unix::fs::PermissionsExt;

    let scaffold = exported_scaffold();
    let script = pr_audit_comment_script(&exported_file("Makefile.toml"));
    let root_makefile = fs::read_to_string(workspace_root().join("Makefile.toml")).unwrap();
    assert_eq!(
        script,
        pr_audit_comment_script(&root_makefile),
        "exported pr-audit-comment wrapper drifted from the root Makefile task"
    );

    let shim_dir = scaffold.join("tmp/test-shim");
    fs::create_dir_all(&shim_dir).unwrap();
    let gh_log = shim_dir.join("gh.log");
    let gh_shim = shim_dir.join("gh");
    fs::write(
        &gh_shim,
        "#!/bin/sh\nprintf '%s|%s|%s\\n' \"${GH_REPO:-unset}\" \"${GH_HOST:-unset}\" \"$*\" >> \"$GH_LOG\"\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&gh_shim, fs::Permissions::from_mode(0o755)).unwrap();
    let wrapper = shim_dir.join("wrapper.sh");
    fs::write(&wrapper, &script).unwrap();

    fs::create_dir_all(scaffold.join("tmp/pr-audit")).unwrap();
    fs::write(scaffold.join("tmp/pr-audit/body.md"), "audit body\n").unwrap();
    let outside_secret = scaffold.join("outside-secret.txt");
    fs::write(&outside_secret, "secret\n").unwrap();
    std::os::unix::fs::symlink(&outside_secret, scaffold.join("tmp/pr-audit/link.md")).unwrap();

    let run = |args: &[&str]| {
        let shimmed_path =
            format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default());
        Command::new("sh")
            .arg(&wrapper)
            .args(args)
            .env("PATH", shimmed_path)
            .env("GH_LOG", &gh_log)
            .env("GH_REPO", "attacker/elsewhere")
            .env("GH_HOST", "github.evil.example")
            .current_dir(&scaffold)
            .output()
            .unwrap()
    };
    let gh_invocations = || {
        fs::read_to_string(&gh_log)
            .map(|log| log.lines().map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_default()
    };

    let rejected: &[&[&str]] = &[
        &[],
        &["--edit-last"],
        &["/etc/hostname"],
        &["tmp/pr-audit/../body.md"],
        &["tmp/pr-audit/body.md", "extra-arg"],
        &["tmp/pr-audit/link.md"],
        &["tmp/pr-audit/missing.md"],
    ];
    for args in rejected {
        let output = run(args);
        assert!(
            !output.status.success(),
            "wrapper must reject argv {args:?}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(gh_invocations().is_empty(), "rejected argv {args:?} must never reach gh");
    }

    let expected_call = "unset|unset|pr comment --body-file tmp/pr-audit/body.md";
    let plain = run(&["tmp/pr-audit/body.md"]);
    assert!(
        plain.status.success(),
        "valid invocation must succeed\nstderr: {}",
        String::from_utf8_lossy(&plain.stderr),
    );
    assert_eq!(gh_invocations(), vec![expected_call.to_owned()], "gh must be reached exactly once");

    let cargo_make_style = run(&["--", "tmp/pr-audit/body.md"]);
    assert!(
        cargo_make_style.status.success(),
        "`--`-prefixed invocation (cargo-make arg passthrough) must succeed\nstderr: {}",
        String::from_utf8_lossy(&cargo_make_style.stderr),
    );
    assert_eq!(
        gh_invocations(),
        vec![expected_call.to_owned(), expected_call.to_owned()],
        "each valid invocation must reach gh exactly once with the fixed posting form"
    );
}
