//! Tempdir-backed adapter tests for the template export filesystem adapters
//! (spec IN-01, IN-02, IN-03, IN-12, AC-01, AC-02, AC-03, CN-02, CN-03).

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use usecase::template_export::{
    TemplateBoundaryManifestPort, TemplateBoundaryManifestReadError, TemplateExportCommand,
    TemplateExportPort, TemplateExportPortError,
};

use super::{FsTemplateBoundaryManifestAdapter, FsTemplateExportAdapter};

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

#[test]
fn test_export_applies_all_classifications() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    let manifest = full_manifest();

    let report = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap();

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

    FsTemplateExportAdapter::new().export(&first, &manifest).unwrap();
    FsTemplateExportAdapter::new().export(&second, &manifest).unwrap();

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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
    let expected = command.workspace_root.join("libs/does-not-exist");
    assert!(
        matches!(err, TemplateExportPortError::SourceMissing { ref path } if path == &expected),
        "unexpected error: {err}"
    );
}

#[test]
fn test_export_missing_overlay_anchor_returns_source_missing() {
    let dir = TempDir::new().unwrap();
    let command = export_fixture(&dir);
    // An `overlay` row whose workspace anchor is absent is drift, not a valid
    // overlay: the preflight rejects it before the OverlayMissing check runs.
    let manifest = manifest_from_json(
        r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "missing-overlay-anchor", "classification": "overlay" }
  ]
}"#,
    );

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
    let expected = command.workspace_root.join("missing-overlay-anchor");
    assert!(
        matches!(err, TemplateExportPortError::SourceMissing { ref path } if path == &expected),
        "unexpected error: {err}"
    );
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

    let err = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap_err();
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

    let report = FsTemplateExportAdapter::new().export(&command, &manifest).unwrap();
    assert_eq!(report.included_count, 1);
    assert!(command.output_dir.join("libs/domain/src/lib.rs").exists());
}
