//! Exported-consumer regression tests for the host-first scaffold contract.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};

use cli_driver::template_export::{TemplateDriver, TemplateExportInput, TemplateInput};
use domain::FreeText;
use infrastructure::template_export::{FsTemplateBoundaryManifestAdapter, FsTemplateExportAdapter};
use tempfile::TempDir;
use usecase::template_export::{
    SelfBinaryTransplantError, SelfBinaryTransplantPort, TemplateBoundaryManifestPort,
    TemplateExportInteractor, TemplateExportPort, TemplateExportService,
};

static EXPORTED_SCAFFOLD_PATH: OnceLock<PathBuf> = OnceLock::new();
static EXPORTED_SCAFFOLD_PARENT: OnceLock<PathBuf> = OnceLock::new();
static EXPORTED_SCAFFOLD_CLEANUP_REGISTERED: OnceLock<()> = OnceLock::new();

extern "C" fn cleanup_exported_scaffold() {
    if let Some(path) = EXPORTED_SCAFFOLD_PARENT.get()
        && let Err(error) = fs::remove_dir_all(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(
            stderr,
            "cannot remove template-export test directory {} at process exit: {error}",
            path.display()
        );
        std::process::abort();
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn template_export_temp_parent(
    cargo_target_tmpdir: Option<PathBuf>,
    workspace_root: &Path,
) -> PathBuf {
    cargo_target_tmpdir
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| workspace_root.join("target/tmp"))
}

fn template_export_tempdir() -> TempDir {
    let workspace_root = workspace_root();
    let parent = template_export_temp_parent(
        option_env!("CARGO_TARGET_TMPDIR").map(PathBuf::from),
        &workspace_root,
    );
    fs::create_dir_all(&parent).unwrap_or_else(|error| {
        panic!("cannot create template-export temporary parent {}: {error}", parent.display())
    });
    tempfile::tempdir_in(&parent).unwrap_or_else(|error| {
        panic!("cannot create template-export temporary directory in {}: {error}", parent.display())
    })
}

fn machine_home_directory() -> Option<PathBuf> {
    ["SOTP_MACHINE_HOME", "HOME", "USERPROFILE"].into_iter().find_map(|variable| {
        std::env::var_os(variable).filter(|value| !value.is_empty()).map(PathBuf::from)
    })
}

fn host_first_scaffold_parent(
    cargo_target_tmpdir: Option<PathBuf>,
    workspace_root: &Path,
    process_id: u32,
) -> PathBuf {
    template_export_temp_parent(cargo_target_tmpdir, workspace_root)
        .join(format!("consumer-scaffold-host-first-{process_id}"))
}

fn git_predicate(workspace_root: &Path, args: &[&str]) -> bool {
    let status = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|error| panic!("cannot run git {}: {error}", args.join(" ")));

    match status.code() {
        Some(0) => true,
        Some(1) => false,
        code => panic!("git {} failed with exit code {code:?}", args.join(" ")),
    }
}

fn gitignored_untracked(workspace_root: &Path, relative_path: &Path) -> bool {
    let relative_path = relative_path.to_str().unwrap_or_else(|| {
        panic!("workspace path is not valid UTF-8: {}", relative_path.display())
    });
    if git_predicate(workspace_root, &["ls-files", "--error-unmatch", "--", relative_path]) {
        return false;
    }
    git_predicate(workspace_root, &["check-ignore", "--quiet", "--", relative_path])
}

fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!("cannot create fixture directory {}: {error}", parent.display())
        });
    }
    fs::copy(source, destination).unwrap_or_else(|error| {
        panic!("cannot copy {} to {}: {error}", source.display(), destination.display())
    });
}

/// Test-only transplant adapter. The production export command keeps its
/// copy-based adapter; this fixture uses a hard link so creating the exported
/// scaffold does not duplicate the CLI binary.
#[derive(Debug)]
struct HardLinkSelfBinaryTransplantAdapter {
    source: PathBuf,
}

impl HardLinkSelfBinaryTransplantAdapter {
    fn new(source: PathBuf) -> Self {
        Self { source }
    }
}

fn hard_link_or_copy(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    match fs::hard_link(source, destination) {
        Ok(()) => Ok(()),
        // The test source and target normally share Cargo's filesystem. A
        // target directory mounted elsewhere still gets a usable fixture.
        Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
            fs::copy(source, destination).map(|_| ())
        }
        Err(error) => Err(error),
    }
}

impl SelfBinaryTransplantPort for HardLinkSelfBinaryTransplantAdapter {
    fn transplant(&self, destination: &Path) -> Result<(), SelfBinaryTransplantError> {
        if let Some(parent) = destination.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                SelfBinaryTransplantError::DestinationWriteFailure {
                    path: destination.to_path_buf(),
                    reason: FreeText::new(error.to_string()),
                }
            })?;
        }

        hard_link_or_copy(&self.source, destination).map_err(|error| {
            SelfBinaryTransplantError::DestinationWriteFailure {
                path: destination.to_path_buf(),
                reason: FreeText::new(error.to_string()),
            }
        })
    }
}

fn test_template_export_driver() -> TemplateDriver {
    let manifest_port: Arc<dyn TemplateBoundaryManifestPort> =
        Arc::new(FsTemplateBoundaryManifestAdapter::new());
    let export_port: Arc<dyn TemplateExportPort> =
        Arc::new(FsTemplateExportAdapter::new(machine_home_directory()));
    let transplant_port: Arc<dyn SelfBinaryTransplantPort> = Arc::new(
        HardLinkSelfBinaryTransplantAdapter::new(PathBuf::from(env!("CARGO_BIN_EXE_sotp"))),
    );
    let service: Arc<dyn TemplateExportService> =
        Arc::new(TemplateExportInteractor::new(manifest_port, export_port, transplant_port));
    TemplateDriver::new(service)
}

fn workspace_cargo_target_root(
    workspace_root: &Path,
    cargo_target_tmpdir: &Path,
) -> Option<PathBuf> {
    let cargo_target_tmpdir = cargo_target_tmpdir.canonicalize().unwrap_or_else(|error| {
        panic!(
            "cannot canonicalize Cargo target temporary directory {}: {error}",
            cargo_target_tmpdir.display()
        )
    });
    let target_root = cargo_target_tmpdir.parent()?.to_path_buf();
    if target_root == workspace_root || !target_root.starts_with(workspace_root) {
        return None;
    }
    Some(target_root)
}

fn copy_workspace_input_tree(
    workspace_root: &Path,
    relative_source: &Path,
    destination: &Path,
    excluded_root: Option<&Path>,
) {
    let source = workspace_root.join(relative_source);
    if excluded_root.is_some_and(|root| source == root || source.starts_with(root)) {
        return;
    }
    let metadata = fs::symlink_metadata(&source)
        .unwrap_or_else(|error| panic!("cannot inspect {}: {error}", source.display()));
    if metadata.file_type().is_symlink() {
        panic!("workspace fixture source must not contain symlink {}", source.display());
    }

    if metadata.is_dir() {
        fs::create_dir_all(destination).unwrap_or_else(|error| {
            panic!("cannot create fixture directory {}: {error}", destination.display())
        });
        let mut entries = fs::read_dir(&source)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()))
            .map(|entry| {
                entry.unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let child = relative_source.join(entry.file_name());
            let child_source = workspace_root.join(&child);
            if child == Path::new(".git")
                || excluded_root
                    .is_some_and(|root| child_source == root || child_source.starts_with(root))
                || gitignored_untracked(workspace_root, &child)
            {
                continue;
            }
            copy_workspace_input_tree(
                workspace_root,
                &child,
                &destination.join(entry.file_name()),
                excluded_root,
            );
        }
    } else if metadata.is_file() {
        copy_file(&source, destination);
    } else {
        panic!("workspace fixture source is not a regular file or directory: {}", source.display());
    }
}

fn materialize_export_source(parent: &Path) -> PathBuf {
    let workspace_root = workspace_root();
    let source_root = parent.join("source");
    fs::create_dir_all(&source_root).unwrap_or_else(|error| {
        panic!("cannot create template-export source fixture {}: {error}", source_root.display())
    });

    let cargo_target_tmpdir = template_export_temp_parent(
        option_env!("CARGO_TARGET_TMPDIR").map(PathBuf::from),
        &workspace_root,
    );
    let excluded_root = workspace_cargo_target_root(&workspace_root, &cargo_target_tmpdir);
    copy_workspace_input_tree(
        &workspace_root,
        Path::new(""),
        &source_root,
        excluded_root.as_deref(),
    );
    source_root
}

fn exported_scaffold() -> PathBuf {
    let export_parent = EXPORTED_SCAFFOLD_PARENT
        .get_or_init(|| {
            let workspace_root = workspace_root();
            host_first_scaffold_parent(
                option_env!("CARGO_TARGET_TMPDIR").map(PathBuf::from),
                &workspace_root,
                std::process::id(),
            )
        })
        .clone();
    EXPORTED_SCAFFOLD_CLEANUP_REGISTERED.get_or_init(|| {
        // Safety: the callback has the required C ABI, captures no state, and only
        // removes the process-isolated directory recorded in the OnceLock.
        let registration = unsafe { libc::atexit(cleanup_exported_scaffold) };
        if registration != 0 {
            panic!("cannot register template-export test directory cleanup");
        }
    });
    EXPORTED_SCAFFOLD_PATH
        .get_or_init(|| {
            // This fixed, process-isolated directory is deliberately stored as a
            // path rather than a `TempDir`: it remains under Cargo's cleanable
            // target temp root while allowing all tests in this binary to share
            // one expensive export. Remove leftovers from an interrupted prior
            // run before recreating the sibling source/scaffold trees.
            if export_parent.exists() {
                fs::remove_dir_all(&export_parent).unwrap_or_else(|error| {
                    panic!(
                        "cannot remove stale template-export test directory {}: {error}",
                        export_parent.display()
                    )
                });
            }
            fs::create_dir_all(&export_parent).unwrap_or_else(|error| {
                panic!(
                    "cannot create template-export test directory {}: {error}",
                    export_parent.display()
                )
            });
            let output_dir = export_parent.join("scaffold");
            let source_root = materialize_export_source(&export_parent);
            export_scaffold(&source_root, &output_dir);
            output_dir
        })
        .clone()
}

fn export_scaffold(source_root: &Path, output_dir: &Path) {
    let outcome =
        test_template_export_driver().handle(TemplateInput::Export(TemplateExportInput {
            workspace_root: source_root.to_path_buf(),
            manifest_path: source_root.join(".harness/config/template-boundary.json"),
            overlay_dir: source_root.join("overlay"),
            output_dir: output_dir.to_path_buf(),
        }));

    assert!(outcome.exit_code == 0, "template export failed: {outcome:?}",);

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        let source = fs::metadata(env!("CARGO_BIN_EXE_sotp")).unwrap();
        let transplanted = fs::metadata(output_dir.join("bin/sotp")).unwrap();
        if source.dev() == transplanted.dev() {
            assert_eq!(source.ino(), transplanted.ino());
        }
    }
}

#[test]
fn test_template_export_cli_dispatch_missing_manifest_returns_failure() {
    let temp_dir = template_export_tempdir();
    let workspace_root = temp_dir.path().join("workspace");
    let manifest_path = workspace_root.join("boundary.json");
    let overlay_dir = workspace_root.join("overlay");
    let output_dir = temp_dir.path().join("scaffold");
    fs::create_dir_all(&workspace_root).unwrap();

    // Stop at manifest loading so this subprocess smoke test covers the real
    // CLI/composition dispatch without invoking production's copy-based binary
    // transplant; the in-process path above owns the hard-link transplant check.
    let output = Command::new(env!("CARGO_BIN_EXE_sotp"))
        .env("SOTP_TELEMETRY", "0")
        .args(["template", "export", "--workspace-root"])
        .arg(&workspace_root)
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--overlay-dir")
        .arg(&overlay_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "sotp template export must surface the missing manifest\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("template boundary manifest not found"),
        "the CLI must dispatch through the composition-root manifest adapter"
    );
    assert!(!output_dir.exists(), "manifest failure must stop before creating an export");
}

#[test]
fn test_template_export_temp_parent_prefers_cargo_target_tmpdir() {
    let configured = PathBuf::from("/cargo/target/tmp");

    assert_eq!(
        template_export_temp_parent(Some(configured.clone()), Path::new("/workspace")),
        configured
    );
}

#[test]
fn test_template_export_temp_parent_falls_back_to_workspace_target_tmp() {
    assert_eq!(
        template_export_temp_parent(None, Path::new("/workspace")),
        PathBuf::from("/workspace/target/tmp")
    );
    assert_eq!(
        template_export_temp_parent(Some(PathBuf::new()), Path::new("/workspace")),
        PathBuf::from("/workspace/target/tmp")
    );
}

#[test]
fn test_host_first_scaffold_parent_is_process_isolated_under_target_tmp() {
    let configured = PathBuf::from("/cargo/target/tmp");

    assert_eq!(
        host_first_scaffold_parent(Some(configured), Path::new("/workspace"), 4242),
        PathBuf::from("/cargo/target/tmp/consumer-scaffold-host-first-4242")
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

fn task_toml_optional_value(makefile: &str, task_name: &str, key: &str) -> Option<toml::Value> {
    let document: toml::Value = toml::from_str(makefile)
        .unwrap_or_else(|error| panic!("Makefile.toml must be valid TOML: {error}"));

    document
        .get("tasks")
        .and_then(toml::Value::as_table)
        .and_then(|tasks| tasks.get(task_name))
        .and_then(toml::Value::as_table)
        .and_then(|task| task.get(key))
        .cloned()
}

fn task_toml_value(makefile: &str, task_name: &str, key: &str) -> toml::Value {
    task_toml_optional_value(makefile, task_name, key)
        .unwrap_or_else(|| panic!("TOML key {key} missing from task {task_name}"))
}

fn task_toml_optional_bool_value(makefile: &str, task_name: &str, key: &str) -> Option<bool> {
    task_toml_optional_value(makefile, task_name, key).map(|value| {
        value
            .as_bool()
            .unwrap_or_else(|| panic!("TOML key {key} in task {task_name} must be a boolean"))
    })
}

fn task_toml_string_array(makefile: &str, task_name: &str, key: &str) -> Vec<String> {
    let value = task_toml_value(makefile, task_name, key);
    value
        .as_array()
        .unwrap_or_else(|| panic!("{key} for task {task_name} must be a TOML string array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{key} for task {task_name} must be a TOML string array"))
        })
        .map(str::to_owned)
        .collect()
}

fn task_toml_string_value(makefile: &str, task_name: &str, key: &str) -> String {
    task_toml_value(makefile, task_name, key)
        .as_str()
        .unwrap_or_else(|| panic!("{key} for task {task_name} must be a TOML basic string"))
        .to_owned()
}

fn task_dependency_set(makefile: &str, task_name: &str) -> BTreeSet<String> {
    task_toml_string_array(makefile, task_name, "dependencies").into_iter().collect()
}

fn dependency_set(dependencies: &[&str]) -> BTreeSet<String> {
    dependencies.iter().map(|dependency| (*dependency).to_owned()).collect()
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
        "track-views-sync",
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
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let export_parent = template_export_tempdir();
    let real_git = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("Git must be available on PATH");
    let enclosing_repository = export_parent.path().to_path_buf();
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

    let source_root = materialize_export_source(export_parent.path());
    let scaffold = enclosing_repository.join("scaffold");
    export_scaffold(&source_root, &scaffold);
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
        "#!/bin/sh\ncase \"$1 $2\" in\n  'generate-lockfile ') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\" && printf 'generated lockfile\\n' > Cargo.lock ;;\n  'make install-sotp') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\"; ln \"$RUNNABLE_SOTP\" bin/sotp 2>/dev/null || cp \"$RUNNABLE_SOTP\" bin/sotp; chmod +x bin/sotp ;;\n  'make bootstrap') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\"; if [ \"${FAIL_BOOTSTRAP:-0}\" = 1 ]; then echo 'simulated bootstrap failure' >&2; exit 72; fi; exec \"$REAL_CARGO\" make bootstrap ;;\n  'make install-aux-tools'|'make ci') printf 'cargo %s\\n' \"$*\" >> \"$INIT_TRACE\" ;;\n  *) printf 'unexpected cargo command: %s\\n' \"$*\" >&2; exit 64 ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&cargo_shim, fs::Permissions::from_mode(0o755)).unwrap();

    let trace = shim_dir.join("init.trace");
    let installed_sotp = scaffold.join("bin/sotp");
    if installed_sotp.exists() {
        fs::remove_file(&installed_sotp).unwrap();
    }
    let sotp_shim = export_parent.path().join("runnable-sotp");
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
            .env("RUNNABLE_SOTP", &sotp_shim)
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
    assert!(!installed_sotp.exists(), "identity preflight must not install the transplanted CLI");
    let missing_identity_trace = fs::read_to_string(&trace).unwrap_or_default();
    assert!(
        !missing_identity_trace.contains("git -c core.hooksPath=/dev/null init -b main"),
        "identity validation must reject the host before Git initialization: {missing_identity_trace}"
    );
    for command_after_identity_check in [
        "cargo generate-lockfile",
        "cargo make install-sotp",
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
    if installed_sotp.exists() {
        fs::remove_file(&installed_sotp).unwrap();
    }

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
    fs::remove_file(&installed_sotp).unwrap();

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
    fs::remove_file(&installed_sotp).unwrap();

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
        "cargo make install-sotp",
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
    assert!(
        installed_sotp.is_file(),
        "a missing transplanted CLI must be restored before the convention index is generated"
    );
    let source_metadata = fs::metadata(&sotp_shim).unwrap();
    let installed_metadata = fs::metadata(&installed_sotp).unwrap();
    assert_eq!(
        source_metadata.dev(),
        installed_metadata.dev(),
        "test transplant must remain on the source filesystem",
    );
    assert_eq!(
        source_metadata.ino(),
        installed_metadata.ino(),
        "test install-sotp shim must use a hard link",
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
fn test_exported_ci_track_local_early_detection_preserves_track_gates() {
    let makefile = exported_file("Makefile.toml");
    let description = task_toml_string_value(&makefile, "ci-track", "description");
    let closure = task_dependency_closure(&makefile, "ci-track");
    let script = task_toml_string_array(&makefile, "ci-track", "script");

    assert!(description.contains("local early-detection"));
    assert!(description.contains("Requires a track/<id> branch"));
    assert!(
        !task_toml_optional_bool_value(&makefile, "ci-track", "private").unwrap_or(false),
        "ci-track must remain a public cargo-make route for local early detection"
    );
    assert!(
        !task_toml_optional_bool_value(&makefile, "ci-track", "disabled").unwrap_or(false),
        "ci-track must remain enabled for local early detection"
    );
    assert_eq!(script, ["bin/sotp adr-baseline check-commit"]);
    assert!(closure.contains("task-contract-check-local"));
    assert!(closure.contains("verify-spec-states-current"));
    assert!(closure.contains("signal-check-impl-catalog"));
    assert!(closure.contains("verify-catalogue-spec-refs"));
    assert!(closure.contains("check-catalogue-spec-signals"));
}

#[test]
fn test_exported_ci_track_remote_enforcement_is_documented() {
    let makefile = exported_file("Makefile.toml");
    let description = task_toml_string_value(&makefile, "ci-track", "description");

    assert!(description.contains("Remote CI plus branch protection enforce merge eligibility"));
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
fn test_root_aggregate_gate_wrappers_use_shared_summary_surface() {
    let makefile = fs::read_to_string(workspace_root().join("Makefile.toml")).unwrap();
    let local_wrappers = [
        ("ci-rust-local", "ci-rust-local-steps"),
        ("ci-track-local", "ci-track-local-steps"),
        ("ci-rust-container", "ci-rust-local-steps"),
        ("ci-track-container", "ci-track-local-steps"),
    ];
    for (task, child) in local_wrappers {
        assert_eq!(task_toml_string_value(&makefile, task, "command"), "cargo");
        assert_eq!(
            task_toml_string_array(&makefile, task, "args"),
            [
                "run",
                "--locked",
                "--quiet",
                "-p",
                "cli",
                "--",
                "gate-output",
                "--name",
                task,
                "--",
                "cargo",
                "make",
                "--allow-private",
                child,
            ]
        );
        assert!(
            task_toml_optional_value(&makefile, task, "script").is_none(),
            "{task} wrapper must not declare a script field"
        );
    }

    for aggregator in ["ci-local", "ci-container"] {
        assert!(
            task_toml_optional_value(&makefile, aggregator, "command").is_none(),
            "{aggregator} is the D9 dependency aggregator and must not wrap gate-output"
        );
        assert!(
            task_toml_optional_value(&makefile, aggregator, "script").is_none(),
            "{aggregator} must not declare a script field"
        );
    }

    let compose_wrappers = [
        ("ci-rust", "ci-rust-local-steps"),
        ("ci", "ci-local"),
        ("ci-track", "ci-track-local-steps"),
    ];
    for (task, child) in compose_wrappers {
        assert_eq!(task_toml_string_value(&makefile, task, "command"), "bin/sotp");
        assert_eq!(
            task_toml_string_array(&makefile, task, "args"),
            [
                "gate-output",
                "--name",
                task,
                "--",
                "docker",
                "compose",
                "run",
                "--rm",
                "tools",
                "cargo",
                "make",
                "--allow-private",
                child,
            ]
        );
        assert!(
            task_toml_optional_value(&makefile, task, "script").is_none(),
            "{task} wrapper must not declare a script field"
        );
    }

    assert_eq!(
        task_dependency_set(&makefile, "ci-rust-local-steps"),
        dependency_set(&[
            "fmt-check-local",
            "clippy-local",
            "test-local",
            "test-cli-feature-off-local",
            "build-sotp-default-local",
            "deny-local",
            "check-layers-local",
            "verify-canonical-modules-local",
        ])
    );
    let repo_gate_dependencies = dependency_set(&[
        "fmt-check-local",
        "clippy-local",
        "test-local",
        "test-cli-feature-off-local",
        "test-doc-local",
        "build-sotp-default-local",
        "deny-local",
        "check-layers-local",
        "verify-arch-docs-local",
        "verify-doc-links-local",
        "verify-plan-progress-local",
        "verify-track-metadata-local",
        "verify-track-registry-local",
        "verify-hooks-path-local",
        "verify-canonical-modules-local",
        "verify-latest-track-local",
        "verify-retention-gate-local",
        "verify-module-size-local",
        "verify-domain-strings-local",
        "verify-domain-purity-local",
        "verify-usecase-purity-local",
        "verify-view-freshness-local",
        "verify-plan-artifact-refs-local",
        "verify-adr-signals-local",
        "verify-machine-paths-local",
        "verify-template-refs-local",
        "template-export-smoke-local",
    ]);
    for task in ["ci-local", "ci-local-steps", "ci-container"] {
        assert_eq!(
            task_dependency_set(&makefile, task),
            repo_gate_dependencies,
            "{task} must retain the complete repo-wide quality-gate dependency set"
        );
    }
    assert_eq!(
        task_dependency_set(&makefile, "ci-track-local-steps"),
        dependency_set(&[
            "task-contract-coverage-local",
            "task-contract-check-local",
            "adr-baseline-check-commit-local",
            "verify-spec-states-current-local",
            "signal-check-impl-catalog-local",
            "verify-catalogue-spec-refs-local",
            "check-catalogue-spec-signals-local",
            "test-obligation-check-local",
        ])
    );
}

#[test]
fn test_repo_wide_ci_entry_points_wrap_dependency_aggregators() {
    let makefile = fs::read_to_string(workspace_root().join("Makefile.toml")).unwrap();
    let bootstrap_script = task_toml_string_array(&makefile, "bootstrap", "script").join("\n");
    let expected_bootstrap_ci = "bin/sotp gate-output --name ci -- docker compose run --rm tools cargo make --allow-private ci-local";
    assert!(
        bootstrap_script.lines().any(|line| line.trim() == expected_bootstrap_ci),
        "bootstrap Step 6 must persist the compose CI result through the shared summary surface"
    );
    assert!(
        !bootstrap_script.lines().any(|line| line.trim()
            == "docker compose run --rm tools cargo make --allow-private ci-local"),
        "bootstrap must not invoke the compose CI aggregator without gate-output"
    );

    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).unwrap();
    let expected_repo_wide_ci = "run: docker exec -e CARGO_INCREMENTAL=0 -e CARGO_HOME=/usr/local/cargo -e CARGO_TARGET_DIR=/cargo-target ci-runner cargo run --locked --quiet -p cli -- gate-output --name ci-container -- cargo make --allow-private ci-local-steps";
    assert!(
        workflow.lines().any(|line| line.trim() == expected_repo_wide_ci),
        "GitHub repo-wide CI must persist the in-container result through gate-output"
    );
    assert!(
        !workflow.contains("ci-runner cargo make --allow-private ci-container"),
        "GitHub repo-wide CI must not invoke the raw ci-container aggregator"
    );
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
