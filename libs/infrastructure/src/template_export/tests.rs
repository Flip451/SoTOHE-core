//! Tempdir-backed adapter tests for the template export filesystem adapters
//! (spec IN-01, IN-02, IN-03, IN-12, AC-01, AC-02, AC-03, CN-02, CN-03).

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;
use usecase::template_export::{
    SelfBinaryTransplantError, SelfBinaryTransplantPort, TemplateBoundaryManifestPort,
    TemplateBoundaryManifestReadError, TemplateExportCommand, TemplateExportPort,
    TemplateExportPortError,
};

use super::{
    FsSelfBinaryTransplantAdapter, FsTemplateBoundaryManifestAdapter, FsTemplateExportAdapter,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn read_file(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git").args(args).current_dir(root).output().unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn initialize_git_repository(root: &Path) {
    run_git(root, &["init", "--quiet"]);
}

fn export_adapter() -> FsTemplateExportAdapter {
    FsTemplateExportAdapter::new(Some(PathBuf::from("/work-machine/home")))
}

fn relative_path_from(base: &Path, target: &Path) -> PathBuf {
    let base_components: Vec<_> = base.components().collect();
    let target_components: Vec<_> = target.components().collect();
    let shared_components = base_components
        .iter()
        .zip(&target_components)
        .take_while(|(base, target)| base == target)
        .count();

    let mut relative_path = PathBuf::new();
    for _ in &base_components[shared_components..] {
        relative_path.push("..");
    }
    for component in &target_components[shared_components..] {
        relative_path.push(component.as_os_str());
    }
    if relative_path.as_os_str().is_empty() {
        relative_path.push(".");
    }
    relative_path
}

/// Collects every file path (relative to `root`) in sorted order.
fn collect_files(root: &Path) -> Vec<PathBuf> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(Result::unwrap).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(base, &path, out);
            } else {
                out.push(path.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

const FULL_MANIFEST: &str = r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "libs/domain", "classification": "include" },
    { "pattern": "Makefile.toml", "classification": "overlay" },
    { "pattern": "vendor", "classification": "exclude" }
  ]
}"#;

// ---------------------------------------------------------------------------
// FsTemplateBoundaryManifestAdapter
// ---------------------------------------------------------------------------

#[test]
fn test_manifest_read_returns_decoded_manifest() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    std::fs::write(&manifest_path, FULL_MANIFEST).unwrap();

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let manifest = adapter.read(&manifest_path).unwrap();
    assert_eq!(manifest.entries().len(), 3);
}

#[test]
fn test_manifest_read_missing_file_returns_not_found() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("missing.json");

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&manifest_path).unwrap_err();
    assert!(
        matches!(err, TemplateBoundaryManifestReadError::NotFound { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn test_manifest_read_invalid_json_returns_parse_error() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    std::fs::write(&manifest_path, "{not json}").unwrap();

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&manifest_path).unwrap_err();
    assert!(
        matches!(err, TemplateBoundaryManifestReadError::Parse { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn test_manifest_read_unsupported_schema_version_returns_parse_error() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    std::fs::write(
        &manifest_path,
        r#"{"schema_version": 2, "entries": [{"pattern": "a", "classification": "include"}]}"#,
    )
    .unwrap();

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&manifest_path).unwrap_err();
    assert!(
        matches!(err, TemplateBoundaryManifestReadError::Parse { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn test_manifest_read_invalid_pattern_returns_invalid_pattern_error() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    std::fs::write(
        &manifest_path,
        r#"{"schema_version": 1, "entries": [{"pattern": "../escape", "classification": "include"}]}"#,
    )
    .unwrap();

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&manifest_path).unwrap_err();
    assert!(
        matches!(err, TemplateBoundaryManifestReadError::InvalidPattern { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn test_manifest_read_empty_entries_returns_invalid_manifest_error() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    std::fs::write(&manifest_path, r#"{"schema_version": 1, "entries": []}"#).unwrap();

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&manifest_path).unwrap_err();
    assert!(
        matches!(err, TemplateBoundaryManifestReadError::InvalidManifest { .. }),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_manifest_read_symlink_path_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    let link_path = dir.path().join("boundary-link.json");
    std::fs::write(&manifest_path, FULL_MANIFEST).unwrap();
    std::os::unix::fs::symlink(&manifest_path, &link_path).unwrap();

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&link_path).unwrap_err();
    assert!(
        matches!(
            err,
            TemplateBoundaryManifestReadError::Io { ref path, .. } if path == &link_path
        ),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_manifest_read_symlink_parent_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let real_dir = dir.path().join("real-manifest");
    let link_dir = dir.path().join("manifest-link");
    std::fs::create_dir_all(&real_dir).unwrap();
    std::fs::write(real_dir.join("boundary.json"), FULL_MANIFEST).unwrap();
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();
    let manifest_path = link_dir.join("boundary.json");

    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let err = adapter.read(&manifest_path).unwrap_err();
    assert!(
        matches!(
            err,
            TemplateBoundaryManifestReadError::Io { ref path, .. } if path == &manifest_path
        ),
        "unexpected error: {err}"
    );
}

// ---------------------------------------------------------------------------
// FsTemplateExportAdapter
// ---------------------------------------------------------------------------

/// Builds a workspace + overlay fixture and returns a command pointing at a
/// not-yet-created output directory under `dir`.
fn export_fixture(dir: &TempDir) -> TemplateExportCommand {
    let root = dir.path();
    // Workspace tree.
    write_file(root, "workspace/libs/domain/src/lib.rs", "// domain\n");
    write_file(root, "workspace/libs/domain/Cargo.toml", "[package]\n");
    write_file(root, "workspace/Makefile.toml", "# real sotp tasks\n");
    write_file(root, "workspace/vendor/blob.bin", "excluded\n");
    // Overlay holds the template version of Makefile.toml.
    write_file(root, "overlay/Makefile.toml", "# template tasks\n");

    TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("out"),
    }
}

fn manifest_from_json(content: &str) -> domain::TemplateBoundaryManifest {
    let adapter = FsTemplateBoundaryManifestAdapter::new();
    let dir = TempDir::new().unwrap();
    let manifest_path = dir.path().join("boundary.json");
    std::fs::write(&manifest_path, content).unwrap();
    adapter.read(&manifest_path).unwrap()
}

fn full_manifest() -> domain::TemplateBoundaryManifest {
    manifest_from_json(FULL_MANIFEST)
}

fn gitignore_manifest() -> domain::TemplateBoundaryManifest {
    manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": ".git", "classification": "exclude" },
    { "pattern": ".gitignore", "classification": "include" }
  ]
}"#,
    )
}

#[test]
fn test_export_applies_all_classifications() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let manifest = full_manifest();

    let report = export_adapter().export(&command, &manifest).unwrap();

    assert_eq!(report.included_count, 1);
    assert_eq!(report.excluded_count, 1);
    assert_eq!(report.overlay_applied_count, 1);
    assert_eq!(report.output_dir, command.output_dir);

    // include: domain subtree copied verbatim.
    assert_eq!(read_file(&command.output_dir, "libs/domain/src/lib.rs"), "// domain\n");
    assert_eq!(read_file(&command.output_dir, "libs/domain/Cargo.toml"), "[package]\n");
    // overlay: template version replaces the real Makefile.toml.
    assert_eq!(read_file(&command.output_dir, "Makefile.toml"), "# template tasks\n");
    // exclude: vendor is absent.
    assert!(!command.output_dir.join("vendor").exists());
}

#[test]
fn test_export_is_deterministic() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let manifest = full_manifest();

    let mut first = command.clone();
    first.output_dir = dir.path().join("out-1");
    let mut second = command.clone();
    second.output_dir = dir.path().join("out-2");

    export_adapter().export(&first, &manifest).unwrap();
    export_adapter().export(&second, &manifest).unwrap();

    let files_first = collect_files(&first.output_dir);
    let files_second = collect_files(&second.output_dir);
    assert_eq!(files_first, files_second);
    for rel in files_first {
        let a = std::fs::read(first.output_dir.join(&rel)).unwrap();
        let b = std::fs::read(second.output_dir.join(&rel)).unwrap();
        assert_eq!(a, b, "content diverged for {}", rel.display());
    }
}

#[test]
fn test_export_existing_output_dir_returns_output_dir_exists() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let manifest = full_manifest();
    std::fs::create_dir_all(&command.output_dir).unwrap();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::OutputDirExists { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_output_inside_workspace_source_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let mut command = export_fixture(&dir);
    command.output_dir = command.workspace_root.join("libs/domain/out");
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.output_dir),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_output_inside_overlay_source_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_file(root, "workspace/docs/README.md", "# real\n");
    write_file(root, "overlay/docs/README.md", "# template\n");

    let command = TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("overlay/docs/out"),
    };
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "docs", "classification": "overlay" }
  ]
}"#,
    );

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.output_dir),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_export_output_parent_symlink_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let mut command = export_fixture(&dir);
    let real_out_parent = dir.path().join("real-out-parent");
    let link_out_parent = dir.path().join("out-parent-link");
    std::fs::create_dir_all(&real_out_parent).unwrap();
    std::os::unix::fs::symlink(&real_out_parent, &link_out_parent).unwrap();
    command.output_dir = link_out_parent.join("out");
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.output_dir),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_export_workspace_root_symlink_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let workspace_path = dir.path().join("workspace");
    let real_workspace_path = dir.path().join("real-workspace");
    std::fs::rename(&workspace_path, &real_workspace_path).unwrap();
    std::os::unix::fs::symlink(&real_workspace_path, &workspace_path).unwrap();
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.workspace_root),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_export_overlay_root_symlink_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let overlay_path = dir.path().join("overlay");
    let real_overlay_path = dir.path().join("real-overlay");
    std::fs::rename(&overlay_path, &real_overlay_path).unwrap();
    std::os::unix::fs::symlink(&real_overlay_path, &overlay_path).unwrap();
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.overlay_dir),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_missing_overlay_returns_overlay_missing() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // Remove the overlay file so the overlay classification cannot resolve.
    std::fs::remove_file(dir.path().join("overlay/Makefile.toml")).unwrap();
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(
            err,
            TemplateExportPortError::OverlayMissing { ref pattern, .. }
                if pattern.as_str() == "Makefile.toml"
        ),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_export_overlay_symlink_path_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let overlay_path = dir.path().join("overlay/Makefile.toml");
    let outside_path = dir.path().join("outside-Makefile.toml");
    std::fs::remove_file(&overlay_path).unwrap();
    std::fs::write(&outside_path, "# outside\n").unwrap();
    std::os::unix::fs::symlink(&outside_path, &overlay_path).unwrap();
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &overlay_path),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_unclassified_path_fails_closed() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // Add a workspace file the manifest does not classify.
    write_file(dir.path(), "workspace/README.md", "# unclassified\n");
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(
            err,
            TemplateExportPortError::UnclassifiedPath { ref path }
                if path.as_str() == "README.md"
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_gitignored_untracked_directory_is_skipped() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_file(root, "workspace/.gitignore", "generated/\n");
    write_file(root, "workspace/generated/cache.bin", "temporary\n");
    std::fs::create_dir_all(root.join("overlay")).unwrap();
    initialize_git_repository(&root.join("workspace"));
    let command = TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("out"),
    };

    let report = export_adapter().export(&command, &gitignore_manifest()).unwrap();

    assert_eq!(report.included_count, 1);
    assert!(!command.output_dir.join("generated").exists());
    assert_eq!(read_file(&command.output_dir, ".gitignore"), "generated/\n");
}

#[test]
fn test_export_include_subtree_with_ignored_untracked_entry_excludes_it() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_file(root, "workspace/.gitignore", ".claude/logs/\n");
    write_file(root, "workspace/.claude/rules/keep.md", "tracked rule\n");
    write_file(
        root,
        "workspace/.claude/logs/post-implementation-review-state.json",
        "{\"workspace\": \"/work-machine/home/project\"}\n",
    );
    std::fs::create_dir_all(root.join("overlay")).unwrap();
    initialize_git_repository(&root.join("workspace"));
    run_git(&root.join("workspace"), &["add", ".gitignore", ".claude/rules/keep.md"]);
    let command = TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("out"),
    };
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": ".claude", "classification": "include" },
    { "pattern": ".git", "classification": "exclude" },
    { "pattern": ".gitignore", "classification": "include" }
  ]
}"#,
    );

    let report = export_adapter().export(&command, &manifest).unwrap();

    assert_eq!(report.included_count, 2);
    assert_eq!(read_file(&command.output_dir, ".claude/rules/keep.md"), "tracked rule\n");
    assert!(!command.output_dir.join(".claude/logs").exists());
}

#[test]
fn test_export_gitignored_tracked_file_fails_closed() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_file(root, "workspace/.gitignore", "generated.tmp\n");
    write_file(root, "workspace/generated.tmp", "tracked despite ignore rule\n");
    std::fs::create_dir_all(root.join("overlay")).unwrap();
    initialize_git_repository(&root.join("workspace"));
    run_git(&root.join("workspace"), &["add", "--force", "generated.tmp"]);
    let command = TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("out"),
    };

    let err = export_adapter().export(&command, &gitignore_manifest()).unwrap_err();

    assert!(
        matches!(
            err,
            TemplateExportPortError::UnclassifiedPath { ref path }
                if path.as_str() == "generated.tmp"
        ),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_export_gitignored_path_with_symlinked_git_entry_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_file(root, "workspace/.gitignore", "generated/\n");
    write_file(root, "workspace/generated/cache.bin", "temporary\n");
    std::fs::create_dir_all(root.join("overlay")).unwrap();
    initialize_git_repository(&root.join("workspace"));
    let other_repository = root.join("other-repository");
    std::fs::create_dir_all(&other_repository).unwrap();
    initialize_git_repository(&other_repository);
    let git_dir = root.join("workspace/.git");
    std::fs::remove_dir_all(&git_dir).unwrap();
    std::os::unix::fs::symlink(other_repository.join(".git"), &git_dir).unwrap();
    let command = TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("out"),
    };

    let err = export_adapter().export(&command, &gitignore_manifest()).unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &git_dir),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_rejects_machine_path_in_exported_output() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = PathBuf::from("/work-machine/home");
    write_file(
        dir.path(),
        "workspace/libs/domain/src/lib.rs",
        &format!("source artifact from {}", home.display()),
    );

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(
            err,
            TemplateExportPortError::MachinePathDetected { ref path }
                if path.as_str() == "libs/domain/src/lib.rs"
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_workspace_home_path_fails_closed() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = command.workspace_root.join("libs/domain/.cache/home");
    std::fs::create_dir_all(&home).unwrap();
    let content = format!("container home: {}\n", home.display());
    write_file(dir.path(), "workspace/libs/domain/src/lib.rs", &content);

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, ref reason }
            if path == &command.output_dir && reason.as_str().contains("container-local home")),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_nonexistent_workspace_home_fails_closed() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = command.workspace_root.join("libs/domain/.cache/missing-home");
    write_file(
        dir.path(),
        "workspace/libs/domain/src/lib.rs",
        &format!("container home: {}\n", home.display()),
    );

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, ref reason }
            if path == &command.output_dir && reason.as_str().contains("container-local home")),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_relative_workspace_root_with_existing_machine_home_fails_closed() {
    let dir = TempDir::new().unwrap();
    let mut command = export_fixture(&dir);
    let home = command.workspace_root.join("libs/domain/.cache/home");
    std::fs::create_dir_all(&home).unwrap();
    let content = format!("container home: {}\n", home.display());
    write_file(dir.path(), "workspace/libs/domain/src/lib.rs", &content);

    let current_dir = std::env::current_dir().unwrap();
    let workspace_root = relative_path_from(&current_dir, &command.workspace_root);
    assert!(!workspace_root.is_absolute());
    assert_eq!(
        std::fs::canonicalize(&workspace_root).unwrap(),
        std::fs::canonicalize(&command.workspace_root).unwrap()
    );
    command.workspace_root = workspace_root;

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, ref reason }
            if path == &command.output_dir && reason.as_str().contains("container-local home")),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_machine_home_with_parent_component_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = command.workspace_root.join("../machine-home");
    write_file(
        dir.path(),
        "workspace/libs/domain/src/lib.rs",
        &format!("source artifact from {}\n", home.display()),
    );

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.output_dir),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_rejects_machine_path_when_home_has_trailing_separator() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = PathBuf::from("/work-machine/home/");
    write_file(
        dir.path(),
        "workspace/libs/domain/src/lib.rs",
        "source artifact from /work-machine/home/project\n",
    );

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(
            err,
            TemplateExportPortError::MachinePathDetected { ref path }
                if path.as_str() == "libs/domain/src/lib.rs"
        ),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_rejects_non_utf8_machine_home_path() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;

    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(
        output_dir.join("machine-path.txt"),
        b"source artifact from /work-machine/\xffhome/project\n",
    )
    .unwrap();
    let home = PathBuf::from(OsString::from_vec(b"/work-machine/\xffhome".to_vec()));

    let err = super::machine_path_scan::ensure_exported_output_has_no_machine_paths(
        &output_dir,
        Some(&home),
    )
    .unwrap_err();

    assert!(
        matches!(
            err,
            TemplateExportPortError::MachinePathDetected { ref path }
                if path.as_str() == "machine-path.txt"
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_scan_relative_machine_home_path_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let err = super::machine_path_scan::ensure_exported_output_has_no_machine_paths(
        &output_dir,
        Some(Path::new("work-machine/home")),
    )
    .unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &output_dir),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_unresolved_machine_home_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);

    let err = FsTemplateExportAdapter::new(None).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &command.output_dir),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_allows_machine_home_prefix_with_different_path_component() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = PathBuf::from("/work-machine/home");
    write_file(
        dir.path(),
        "workspace/libs/domain/src/lib.rs",
        &format!("decoy path: {}-archived\n", home.display()),
    );

    let report =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap();

    assert_eq!(report.included_count, 1);
}

#[test]
fn test_export_allows_nonboundary_machine_home_across_scan_chunks() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = PathBuf::from("/work-machine/home");
    let home_path = home.to_string_lossy();
    let filler = "x".repeat(
        super::machine_path_scan::MACHINE_PATH_SCAN_CHUNK_SIZE
            .saturating_sub(home_path.len().saturating_add(2)),
    );
    let content = format!("{filler}x{home_path}\ntrailing\n");
    write_file(dir.path(), "workspace/libs/domain/src/lib.rs", &content);

    let report =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap();

    assert_eq!(report.included_count, 1);
    assert_eq!(read_file(&command.output_dir, "libs/domain/src/lib.rs"), content);
}

#[test]
fn test_export_detects_machine_path_across_scan_chunks() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let home = PathBuf::from("/work-machine/home");
    let filler =
        format!("{} ", "x".repeat(super::machine_path_scan::MACHINE_PATH_SCAN_CHUNK_SIZE - 2));
    let content = format!("{filler}{}\n", home.display());
    write_file(dir.path(), "workspace/libs/domain/src/lib.rs", &content);

    let err =
        FsTemplateExportAdapter::new(Some(home)).export(&command, &full_manifest()).unwrap_err();

    assert!(
        matches!(
            err,
            TemplateExportPortError::MachinePathDetected { ref path }
                if path.as_str() == "libs/domain/src/lib.rs"
        ),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_exported_output_with_symlink_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let output_dir = dir.path().join("out");
    let outside_path = dir.path().join("outside.txt");
    let symlink_path = output_dir.join("linked.txt");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(&outside_path, "outside\n").unwrap();
    std::os::unix::fs::symlink(&outside_path, &symlink_path).unwrap();

    let err = super::machine_path_scan::ensure_exported_output_has_no_machine_paths(
        &output_dir,
        Some(Path::new("/work-machine/home")),
    )
    .unwrap_err();

    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &symlink_path),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_allows_system_container_and_example_paths() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    write_file(
        dir.path(),
        "workspace/libs/domain/src/lib.rs",
        "system: /dev/null\ncommand: /bin/false\ncontainer: /workspace/app\nexample: /example/project\n",
    );

    let report = export_adapter().export(&command, &full_manifest()).unwrap();

    assert_eq!(report.included_count, 1);
    assert_eq!(
        read_file(&command.output_dir, "libs/domain/src/lib.rs"),
        "system: /dev/null\ncommand: /bin/false\ncontainer: /workspace/app\nexample: /example/project\n"
    );
}

#[test]
fn test_export_missing_include_source_returns_source_missing() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // The manifest classifies an include path that does not exist in the
    // workspace; the walk would never reach it, so the preflight must catch it.
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "libs/does-not-exist", "classification": "include" }
  ]
}"#,
    );

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    let expected = command.workspace_root.join("libs/does-not-exist");
    assert!(
        matches!(err, TemplateExportPortError::SourceMissing { ref path } if path == &expected),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_overlay_row_missing_both_sources_returns_source_missing() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // An `overlay` row with neither a workspace anchor nor overlay content is
    // drift: the preflight rejects it before the walk runs. (The fixture overlay
    // dir holds only Makefile.toml, so this pattern has no overlay content.)
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "missing-overlay-anchor", "classification": "overlay" }
  ]
}"#,
    );

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    let expected = command.workspace_root.join("missing-overlay-anchor");
    assert!(
        matches!(err, TemplateExportPortError::SourceMissing { ref path } if path == &expected),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_overlay_only_anchor_emits_overlay_content() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // The workspace has no `track/registry.md` (a gitignored generated view that
    // lives only under overlay/), so the walk never sees it. The overlay-only
    // emission pass must still ship the overlay content and count it.
    write_file(dir.path(), "overlay/track/registry.md", "# generated view\n");
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "libs/domain", "classification": "include" },
    { "pattern": "Makefile.toml", "classification": "overlay" },
    { "pattern": "track/registry.md", "classification": "overlay" },
    { "pattern": "vendor", "classification": "exclude" }
  ]
}"#,
    );

    let report = export_adapter().export(&command, &manifest).unwrap();

    // Makefile.toml (anchor present, via walk) + track/registry.md (overlay-only).
    assert_eq!(report.overlay_applied_count, 2);
    assert_eq!(read_file(&command.output_dir, "track/registry.md"), "# generated view\n");
    assert_eq!(read_file(&command.output_dir, "Makefile.toml"), "# template tasks\n");
}

#[test]
fn test_export_absent_exclude_row_succeeds() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // An `exclude` row whose workspace path does not exist is a no-op, not drift:
    // the export must still succeed.
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "libs/domain", "classification": "include" },
    { "pattern": "Makefile.toml", "classification": "overlay" },
    { "pattern": "vendor", "classification": "exclude" },
    { "pattern": "already-gone", "classification": "exclude" }
  ]
}"#,
    );

    let report = export_adapter().export(&command, &manifest).unwrap();

    assert_eq!(report.included_count, 1);
    assert_eq!(report.overlay_applied_count, 1);
    // Only the present `vendor` exclude is counted; the absent one is a silent no-op.
    assert_eq!(report.excluded_count, 1);
}

#[cfg(unix)]
#[test]
fn test_export_include_symlink_path_returns_io_error() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let include_path = dir.path().join("workspace/libs/domain");
    let outside_path = dir.path().join("outside-domain");
    std::fs::remove_dir_all(&include_path).unwrap();
    std::fs::create_dir_all(&outside_path).unwrap();
    std::fs::write(outside_path.join("secret.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink(&outside_path, &include_path).unwrap();
    let manifest = full_manifest();

    let err = export_adapter().export(&command, &manifest).unwrap_err();
    assert!(
        matches!(err, TemplateExportPortError::Io { ref path, .. } if path == &include_path),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_descends_into_unclassified_directory() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // `libs` is unclassified; the export must descend and classify `libs/domain`.
    write_file(root, "workspace/libs/domain/src/lib.rs", "// domain\n");
    write_file(root, "workspace/Makefile.toml", "# real\n");
    write_file(root, "workspace/vendor/blob.bin", "excluded\n");
    write_file(root, "overlay/Makefile.toml", "# template\n");

    let command = TemplateExportCommand {
        workspace_root: root.join("workspace"),
        manifest_path: root.join("boundary.json"),
        overlay_dir: root.join("overlay"),
        output_dir: root.join("out"),
    };
    let manifest = full_manifest();

    let report = export_adapter().export(&command, &manifest).unwrap();
    assert_eq!(report.included_count, 1);
    assert!(command.output_dir.join("libs/domain/src/lib.rs").exists());
}

// ---------------------------------------------------------------------------
// FsSelfBinaryTransplantAdapter
// ---------------------------------------------------------------------------

#[test]
fn test_transplant_copies_running_binary_verbatim() {
    let dir = TempDir::new().unwrap();
    let destination = dir.path().join("bin/sotp");

    FsSelfBinaryTransplantAdapter::new().transplant(&destination).unwrap();

    // Byte-identity to the running binary (spec CN-01, AC-01). In the test
    // harness `env::current_exe` returns the running test binary; copying it
    // and comparing the two proves the adapter emits a byte-for-byte copy of
    // whatever binary is executing — the same property the CLI relies on.
    let source = std::env::current_exe().unwrap();
    let source_bytes = std::fs::read(&source).unwrap();
    let destination_bytes = std::fs::read(&destination).unwrap();
    assert_eq!(source_bytes, destination_bytes, "transplant must be byte-identical");
}

#[cfg(unix)]
#[test]
fn test_transplant_preserves_executable_permission() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = TempDir::new().unwrap();
    let destination = dir.path().join("bin/sotp");

    FsSelfBinaryTransplantAdapter::new().transplant(&destination).unwrap();

    // The destination must carry the executable bit for at least the owner
    // (spec CN-02, AC-01) — matching the running binary's mode.
    let mode = std::fs::metadata(&destination).unwrap().permissions().mode();
    assert!(mode & 0o100 != 0, "destination must have owner-execute bit: mode={mode:o}");
}

#[test]
fn test_transplant_destination_write_failure_when_parent_is_a_file() {
    let dir = TempDir::new().unwrap();
    // Place a file where the transplant will try to create a directory: the
    // destination's parent (`bin/`) cannot be created because `bin` is a file.
    let blocking_file = dir.path().join("bin");
    std::fs::write(&blocking_file, b"blocking").unwrap();
    let destination = blocking_file.join("sotp");

    let err = FsSelfBinaryTransplantAdapter::new().transplant(&destination).unwrap_err();
    assert!(
        matches!(
            err,
            SelfBinaryTransplantError::DestinationWriteFailure { ref path, .. }
                if path == &destination
        ),
        "unexpected error: {err}"
    );
}
