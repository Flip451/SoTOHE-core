//! Verify shipped template files do not retain concrete ADR or track references.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use domain::TemplatePathClassification;
use domain::verify::{VerifyFinding, VerifyOutcome};
use regex::Regex;
use usecase::template_export::TemplateBoundaryManifestPort;

use crate::template_export::FsTemplateBoundaryManifestAdapter;

use super::git_inventory::{checked_tracked_file_path, tracked_files};

const BOUNDARY_MANIFEST_PATH: &str = ".harness/config/template-boundary.json";
const MAX_TEMPLATE_REF_SCAN_FILE_BYTES: u64 = 8 * 1024 * 1024;

static ADR_REFERENCE_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\d{4}-\d{2}-\d{2}-\d{4}-[a-z0-9][a-z0-9-]*").ok());
static TRACK_REFERENCE_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r#"track/items/([a-z0-9][a-z0-9-]*-\d{4}-\d{2}-\d{2})(?:/|$|[\s`'\"<>\[\](){}.,;:!?])"#,
    )
    .ok()
});

/// Verifies shipped template files for concrete ADR and dated track references.
///
/// The scan set is derived from the boundary manifest's `include` and `overlay`
/// classifications, then intersected with the Git-tracked-file inventory.  It
/// deliberately treats code, comments, and prose identically.
///
/// # Errors
///
/// Returns a failed [`VerifyOutcome`] when the boundary manifest or Git
/// inventory cannot be read, a selected file cannot be read, the patterns fail
/// to initialize, or a selected file contains a forbidden reference name key.
pub fn verify(project_root: &Path) -> VerifyOutcome {
    let manifest_path = project_root.join(BOUNDARY_MANIFEST_PATH);
    let manifest = match FsTemplateBoundaryManifestAdapter::new().read(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read template boundary manifest: {error}"
            ))]);
        }
    };

    let Some(adr_reference_re) = ADR_REFERENCE_RE.as_ref() else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "cannot initialize concrete ADR reference pattern",
        )]);
    };
    let Some(track_reference_re) = TRACK_REFERENCE_RE.as_ref() else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "cannot initialize concrete track reference pattern",
        )]);
    };

    let target_roots: Vec<PathBuf> = manifest
        .entries()
        .iter()
        .filter_map(|entry| match entry.classification() {
            TemplatePathClassification::Include => Some(PathBuf::from(entry.pattern().as_str())),
            TemplatePathClassification::Overlay => {
                Some(Path::new("overlay").join(entry.pattern().as_str()))
            }
            TemplatePathClassification::Exclude => None,
        })
        .collect();

    let tracked_files = match tracked_files(project_root) {
        Ok(files) => files,
        Err(message) => return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]),
    };

    let mut findings = Vec::new();
    for relative_path in tracked_files {
        if !target_roots.iter().any(|target_root| relative_path.starts_with(target_root)) {
            continue;
        }

        let path = match checked_tracked_file_path(project_root, &relative_path) {
            Ok(path) => path,
            Err(error) => {
                findings.push(VerifyFinding::error(format!(
                    "cannot read shipped template file {}: {error}",
                    relative_path.display()
                )));
                continue;
            }
        };
        let content = match read_shipped_template_file(&path) {
            Ok(content) => content,
            Err(error) => {
                findings.push(VerifyFinding::error(format!(
                    "cannot read shipped template file {}: {error}",
                    relative_path.display()
                )));
                continue;
            }
        };
        let content = String::from_utf8_lossy(&content);

        for reference in adr_reference_re.find_iter(&content) {
            findings.push(VerifyFinding::error(format!(
                "{} retains concrete ADR reference '{}'",
                relative_path.display(),
                reference.as_str()
            )));
        }
        for captures in track_reference_re.captures_iter(&content) {
            if let Some(track_id) = captures.get(1) {
                findings.push(VerifyFinding::error(format!(
                    "{} retains concrete track reference '{}'",
                    relative_path.display(),
                    track_id.as_str()
                )));
            }
        }
    }

    VerifyOutcome::from_findings(findings)
}

/// Reads one shipped file while retaining at most the template-reference scan limit.
fn read_shipped_template_file(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let file = File::open(path)?;
    let mut content = Vec::new();
    file.take(MAX_TEMPLATE_REF_SCAN_FILE_BYTES.saturating_add(1)).read_to_end(&mut content)?;
    if content.len() as u64 > MAX_TEMPLATE_REF_SCAN_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "file exceeds template reference scan size limit of {MAX_TEMPLATE_REF_SCAN_FILE_BYTES} bytes"
            ),
        ));
    }
    Ok(content)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::{MAX_TEMPLATE_REF_SCAN_FILE_BYTES, verify};

    const INCLUDE_MANIFEST: &str = r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "docs", "classification": "include" }
  ]
}"#;

    const OVERLAY_MANIFEST: &str = r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "docs", "classification": "overlay" }
  ]
}"#;

    fn write_file(root: &Path, relative_path: &str, content: &str) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
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

    fn project_with_manifest(manifest: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        write_file(temp_dir.path(), ".harness/config/template-boundary.json", manifest);
        run_git(temp_dir.path(), &["init", "--quiet", "--initial-branch=main"]);
        temp_dir
    }

    fn track_all_files(project_root: &Path) {
        run_git(project_root, &["add", "."]);
    }

    #[test]
    fn test_verify_actual_adr_name_returns_error() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        write_file(
            temp_dir.path(),
            "docs/reference.md",
            "knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md\n",
        );
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).has_errors());
    }

    #[test]
    fn test_verify_adr_readme_reference_returns_pass() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        write_file(temp_dir.path(), "docs/reference.md", "knowledge/adr/README.md\n");
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).is_ok());
    }

    #[test]
    fn test_verify_track_placeholder_reference_returns_pass() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        write_file(temp_dir.path(), "docs/reference.md", "track/items/<id>/\n");
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).is_ok());
    }

    #[test]
    fn test_verify_date_only_reference_returns_pass() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        write_file(temp_dir.path(), "docs/reference.md", "2026-07-13-0818\n");
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).is_ok());
    }

    #[test]
    fn test_verify_dated_track_id_returns_error() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        write_file(
            temp_dir.path(),
            "docs/reference.md",
            "track/items/public-template-blocker-cleanup-2026-07-13/\n",
        );
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).has_errors());
    }

    #[test]
    fn test_verify_overlay_reference_returns_error() {
        let temp_dir = project_with_manifest(OVERLAY_MANIFEST);
        write_file(
            temp_dir.path(),
            "overlay/docs/reference.md",
            "track/items/public-template-blocker-cleanup-2026-07-13/\n",
        );
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).has_errors());
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_symlinked_shipped_file_returns_error() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        let outside_file = temp_dir.path().join("outside.md");
        std::fs::write(&outside_file, "fixture\n").unwrap();
        std::fs::create_dir_all(temp_dir.path().join("docs")).unwrap();
        std::os::unix::fs::symlink(&outside_file, temp_dir.path().join("docs/reference.md"))
            .unwrap();
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).has_errors());
    }

    #[test]
    fn test_verify_oversized_shipped_file_returns_error() {
        let temp_dir = project_with_manifest(INCLUDE_MANIFEST);
        let oversized_path = temp_dir.path().join("docs/oversized.md");
        std::fs::create_dir_all(oversized_path.parent().unwrap()).unwrap();
        std::fs::File::create(&oversized_path)
            .unwrap()
            .set_len(MAX_TEMPLATE_REF_SCAN_FILE_BYTES.saturating_add(1))
            .unwrap();
        track_all_files(temp_dir.path());

        assert!(verify(temp_dir.path()).has_errors());
    }
}
