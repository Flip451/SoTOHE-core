//! Exported-consumer regression tests for the host-first scaffold contract.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
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

fn initial_setup_section(readme: &str) -> &str {
    let section =
        &readme[readme.find("### 初回セットアップ").expect("initial setup heading missing")..];
    section.find("\n### ").map_or(section, |end| &section[..end])
}

fn task_names(makefile: &str) -> BTreeSet<&str> {
    makefile
        .lines()
        .filter_map(|line| line.strip_prefix("[tasks.").and_then(|name| name.strip_suffix(']')))
        .collect()
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
fn test_exported_init_task_succeeds_with_global_hooks_path_and_repeat_rejects() {
    use std::os::unix::fs::PermissionsExt;

    let export_parent = tempfile::tempdir().unwrap();
    let real_git = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("Git must be available on PATH");
    let enclosing_repository = export_parent.path().join("enclosing-repository");
    let global_git_config = export_parent.path().join("gitconfig");
    fs::write(&global_git_config, "").unwrap();
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
    fs::write(&global_git_config, "[core]\n\thooksPath = .githooks\n").unwrap();
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
        "#!/bin/sh\ncase \"$1 $2\" in\n  'generate-lockfile ') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\" && printf 'generated lockfile\\n' > Cargo.lock ;;\n  'make bootstrap') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\"; exec \"$REAL_CARGO\" make bootstrap ;;\n  'make install-aux-tools'|'make ci') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\" ;;\n  *) printf 'unexpected cargo command: %s\\n' \"$*\" >&2; exit 64 ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&cargo_shim, fs::Permissions::from_mode(0o755)).unwrap();

    let trace = shim_dir.join("init.trace");
    let sotp_shim = scaffold.join("bin/sotp");
    fs::write(
        &sotp_shim,
        "#!/bin/sh\nprintf 'sotp %s\\n' \"$*\" >> \"$INIT_TRACE\"\nif [ \"$1 $2 $3\" = \"hook dispatch git-ref-update\" ]; then\n  echo 'initial commit must not run inherited hooks' >&2\n  exit 73\nfi\n",
    )
    .unwrap();
    fs::set_permissions(&sotp_shim, fs::Permissions::from_mode(0o755)).unwrap();

    let run = || {
        let shimmed_path =
            format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default());
        Command::new(&real_cargo)
            .args(["make", "init"])
            .current_dir(&scaffold)
            .env("PATH", shimmed_path)
            .env("INIT_TRACE", &trace)
            .env("REAL_CARGO", &real_cargo)
            .env("REAL_GIT", &real_git)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &global_git_config)
            .env("GIT_CONFIG_COUNT", "0")
            .env("GIT_AUTHOR_NAME", "Scaffold Test")
            .env("GIT_AUTHOR_EMAIL", "scaffold-test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Scaffold Test")
            .env("GIT_COMMITTER_EMAIL", "scaffold-test@example.invalid")
            .output()
            .unwrap()
    };

    let first = run();
    assert!(
        first.status.success(),
        "first initialization must succeed\nstderr: {}\ntrace: {}",
        String::from_utf8_lossy(&first.stderr),
        fs::read_to_string(&trace).unwrap_or_default(),
    );
    let trace_lines =
        fs::read_to_string(&trace).unwrap().lines().map(str::to_owned).collect::<Vec<_>>();
    let expected_sequence = [
        "git -c core.hooksPath=/dev/null init -b main",
        "cargo generate-lockfile",
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

    let repeat = run();
    assert!(!repeat.status.success(), "repeat initialization must fail closed");
    assert!(
        String::from_utf8_lossy(&repeat.stderr)
            .contains("before the repository has its first commit")
    );
    let repeat_trace = fs::read_to_string(&trace).unwrap_or_default();
    assert!(repeat_trace.contains("git --git-dir=.git rev-parse --verify HEAD"));
    for state_changing_command in ["git init", "git add", "git commit", "cargo "] {
        assert!(
            !repeat_trace.contains(state_changing_command),
            "repeat initialization must stop before {state_changing_command}: {repeat_trace}",
        );
    }
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
fn test_exported_readme_directs_first_setup_to_init() {
    let readme = exported_file("README.md");
    let initial_setup = initial_setup_section(&readme);

    assert!(
        initial_setup.lines().any(|line| {
            let command =
                line.trim().split_once('#').map_or(line.trim(), |(command, _)| command.trim_end());
            command == "cargo make init"
        }),
        "the exported initial-setup instructions must invoke the init workflow"
    );
    assert!(
        !initial_setup.lines().any(|line| line.trim_start().starts_with("cargo make bootstrap")),
        "the exported initial-setup instructions must not bypass init with bootstrap"
    );
}

#[test]
fn test_exported_branch_strategy_uses_overlay_main_defaults() {
    let source = fs::read_to_string(workspace_root().join(".harness/config/branch-strategy.json"))
        .expect("source branch strategy must be readable");
    let exported = exported_file(".harness/config/branch-strategy.json");

    assert!(source.contains("\"base_branch\": \"develop\""));
    assert!(source.contains("\"merge_target\": \"develop\""));
    assert!(exported.contains("\"base_branch\": \"main\""));
    assert!(exported.contains("\"merge_target\": \"main\""));
    assert!(!exported.contains("develop"));
}

#[test]
fn test_exported_command_adapters_fallback_when_progress_api_is_unavailable() {
    for (command, fallback) in [
        (
            ".claude/commands/track/plan.md",
            "When it is unavailable, report the same phase boundaries and\n  termination progress in text and continue the workflow.",
        ),
        (
            ".claude/commands/track/adr2pr.md",
            "When it is unavailable, report those transitions in text and continue the workflow.",
        ),
    ] {
        let adapter = exported_file(command);
        assert!(adapter.contains("when `TaskCreate` is available"));
        assert!(adapter.contains(fallback), "{command} must retain its text-progress fallback");
    }
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
fn test_exported_workflows_call_sotp_directly_without_retired_wrappers() {
    assert_no_retired_passthrough_calls(&[".harness", ".claude", ".agents", ".codex"]);

    let adr2pr = exported_file(".harness/workflows/track/adr2pr.md");
    let done = exported_file(".harness/workflows/track/done.md");
    let pr = exported_file(".claude/commands/track/pr.md");
    let codex_instructions = exported_file(".codex/instructions.md");
    let codex_rules = exported_file(".codex/rules/default.rules");

    assert!(adr2pr.contains("bin/sotp git add-all"));
    assert!(done.contains("bin/sotp track switch-base"));
    assert!(pr.contains("bin/sotp pr push") && pr.contains("bin/sotp pr ensure-pr"));
    assert!(
        codex_instructions.contains("bin/sotp git add-all")
            && codex_instructions.contains("bin/sotp pr review-cycle"),
        "Codex instructions must direct the guarded bin/sotp workflow commands"
    );
    assert!(
        codex_rules.contains(r#"pattern=["bin/sotp", "pr", "review-cycle"]"#)
            && !codex_rules.contains(r#"pattern=["cargo", "make", "track-pr"#),
        "Codex rules must allow the bin/sotp workflow commands instead of retired wrappers"
    );
}

#[test]
fn test_exported_environment_overlays_are_symmetric_and_personal_environment_free() {
    let source = fs::read_to_string(workspace_root().join("Makefile.toml"))
        .expect("source Makefile.toml must be readable");
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
    for makefile in [&source, &common] {
        assert!(
            makefile.contains("bin/sotp codex-runtime provision --project-root ."),
            "bootstrap must provision the repository-local Codex runtime link"
        );
        assert!(!makefile.contains("CODEX_BIN"));
    }
    assert!(
        common.contains("bin/sotp test-obligation check"),
        "the guarded commit chain must keep the test-obligation gate"
    );
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
