//! Verify Git-tracked files do not contain work-machine home paths.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::template_export::machine_path_scan::{
    MachineHomeWorkspaceContainment, WORKSPACE_LOCAL_MACHINE_HOME_MESSAGE,
    file_contains_machine_home_path, machine_home_workspace_containment,
    normalized_machine_home_path_bytes,
};

use super::git_inventory::{checked_tracked_file_path, tracked_files};

/// Verifies that tracked repository files do not contain the injected machine home.
///
/// A home directory located inside the canonical project root is rejected as a
/// container-local home, so this verification gate cannot silently skip its
/// tracked-file scan. The verifier never reads environment variables to resolve
/// this value.
///
/// # Errors
///
/// Returns a failed [`VerifyOutcome`] when no machine home is supplied, its
/// validation or containment check fails, the Git inventory cannot be listed, a
/// tracked file cannot be read, or a tracked file contains the machine home.
pub fn verify(project_root: &Path, machine_home_dir: Option<&Path>) -> VerifyOutcome {
    let Some(machine_home_dir) = machine_home_dir else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "machine home directory must be explicitly supplied for machine-path verification",
        )]);
    };

    let home_bytes = match normalized_machine_home_path_bytes(project_root, machine_home_dir) {
        Ok(bytes) => bytes,
        Err(error) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot validate machine home directory: {error}"
            ))]);
        }
    };

    match machine_home_workspace_containment(project_root, machine_home_dir, project_root) {
        Ok(MachineHomeWorkspaceContainment::WithinWorkspace) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                WORKSPACE_LOCAL_MACHINE_HOME_MESSAGE,
            )]);
        }
        Ok(MachineHomeWorkspaceContainment::OutsideWorkspace) => {}
        Ok(MachineHomeWorkspaceContainment::Unresolved) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "cannot resolve machine home and project root for containment checking",
            )]);
        }
        Err(error) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot validate machine home directory: {error}"
            ))]);
        }
    }

    let tracked_files = match tracked_files(project_root) {
        Ok(files) => files,
        Err(message) => return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]),
    };

    let mut findings = Vec::new();
    for relative_path in tracked_files {
        let path = match checked_tracked_file_path(project_root, &relative_path) {
            Ok(path) => path,
            Err(error) => {
                findings.push(VerifyFinding::error(format!(
                    "cannot scan {} for work-machine paths: {error}",
                    relative_path.display()
                )));
                continue;
            }
        };
        match file_contains_machine_home_path(&path, &home_bytes) {
            Ok(true) => findings.push(VerifyFinding::error(format!(
                "{} contains a work-machine home path",
                relative_path.display()
            ))),
            Ok(false) => {}
            Err(error) => findings.push(VerifyFinding::error(format!(
                "cannot scan {} for work-machine paths: {error}",
                relative_path.display()
            ))),
        }
    }

    VerifyOutcome::from_findings(findings)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::verify;
    use crate::template_export::machine_path_scan::{
        MachineHomeWorkspaceContainment, machine_home_workspace_containment,
    };

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

    fn initialize_repository(project_root: &Path) {
        run_git(project_root, &["init", "--quiet", "--initial-branch=main"]);
    }

    fn add_all_files(project_root: &Path) {
        run_git(project_root, &["add", "."]);
    }

    #[test]
    fn test_verify_machine_path_in_tracked_file_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = temp_dir.path().join("machine-home");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&machine_home).unwrap();
        initialize_repository(&project_root);
        write_file(
            &project_root,
            "docs/host.md",
            &format!("workstation path: {}/project\n", machine_home.display()),
        );
        add_all_files(&project_root);

        assert!(verify(&project_root, Some(&machine_home)).has_errors());
    }

    #[test]
    fn test_verify_system_path_in_tracked_file_returns_pass() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = temp_dir.path().join("machine-home");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&machine_home).unwrap();
        initialize_repository(&project_root);
        write_file(&project_root, "scripts/check.sh", "cat /dev/null\n/bin/false\n");
        add_all_files(&project_root);

        assert!(verify(&project_root, Some(&machine_home)).is_ok());
    }

    #[test]
    fn test_verify_missing_machine_home_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();
        initialize_repository(&project_root);
        write_file(&project_root, "README.md", "fixture\n");
        add_all_files(&project_root);

        assert!(verify(&project_root, None).has_errors());
    }

    #[test]
    fn test_verify_workspace_local_home_fails_closed() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = project_root.join(".container/home");
        std::fs::create_dir_all(&machine_home).unwrap();
        initialize_repository(&project_root);
        write_file(
            &project_root,
            "docs/container.md",
            &format!("container home: {}\n", machine_home.display()),
        );
        add_all_files(&project_root);

        let outcome = verify(&project_root, Some(&machine_home));

        assert!(outcome.has_errors());
        assert!(outcome.to_string().contains("container-local home"));
    }

    #[test]
    fn test_verify_nonexistent_home_outside_workspace_scans_clean_and_detects_tracked_path() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = temp_dir.path().join("host-home");
        std::fs::create_dir_all(&project_root).unwrap();
        initialize_repository(&project_root);
        write_file(&project_root, "README.md", "fixture\n");
        add_all_files(&project_root);

        assert_eq!(
            machine_home_workspace_containment(&project_root, &machine_home, &project_root)
                .unwrap(),
            MachineHomeWorkspaceContainment::OutsideWorkspace
        );
        assert!(verify(&project_root, Some(&machine_home)).is_ok());

        write_file(
            &project_root,
            "docs/host.md",
            &format!("workstation path: {}/project\n", machine_home.display()),
        );
        add_all_files(&project_root);

        assert!(verify(&project_root, Some(&machine_home)).has_errors());
    }

    #[test]
    fn test_machine_home_workspace_containment_nonexistent_home_inside_workspace_returns_within() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = project_root.join("container/home");
        std::fs::create_dir_all(&project_root).unwrap();

        assert_eq!(
            machine_home_workspace_containment(&project_root, &machine_home, &project_root)
                .unwrap(),
            MachineHomeWorkspaceContainment::WithinWorkspace
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_nonexistent_home_below_symlinked_workspace_fails_closed() {
        let temp_dir = TempDir::new().unwrap();
        let real_project_root = temp_dir.path().join("project");
        let symlinked_project_root = temp_dir.path().join("project-link");
        let machine_home = symlinked_project_root.join(".container/home");
        std::fs::create_dir_all(&real_project_root).unwrap();
        std::os::unix::fs::symlink(&real_project_root, &symlinked_project_root).unwrap();

        assert_eq!(
            machine_home_workspace_containment(
                &symlinked_project_root,
                &machine_home,
                &symlinked_project_root
            )
            .unwrap(),
            MachineHomeWorkspaceContainment::WithinWorkspace
        );

        let outcome = verify(&symlinked_project_root, Some(&machine_home));

        assert!(outcome.has_errors());
        assert!(outcome.to_string().contains("container-local home"));
    }

    #[test]
    fn test_verify_home_with_parent_component_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = temp_dir.path().join("machine-home/../machine-home");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("machine-home")).unwrap();
        initialize_repository(&project_root);
        write_file(&project_root, "README.md", "fixture\n");
        add_all_files(&project_root);

        assert!(verify(&project_root, Some(&machine_home)).has_errors());
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_symlinked_tracked_file_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let machine_home = temp_dir.path().join("machine-home");
        let outside_file = temp_dir.path().join("outside.md");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&machine_home).unwrap();
        std::fs::write(&outside_file, "fixture\n").unwrap();
        initialize_repository(&project_root);
        std::fs::create_dir_all(project_root.join("docs")).unwrap();
        std::os::unix::fs::symlink(&outside_file, project_root.join("docs/host.md")).unwrap();
        add_all_files(&project_root);

        assert!(verify(&project_root, Some(&machine_home)).has_errors());
    }
}
