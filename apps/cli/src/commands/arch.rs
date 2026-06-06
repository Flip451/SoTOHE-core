//! `sotp arch` subcommand group.
//!
//! Each subcommand delegates to the corresponding `CliApp` method and
//! prints the outcome. Exits 0 on success, 1 on error.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;
use cli_composition::CliApp;

use super::outcome_to_exit;

/// Architecture rules analysis subcommands.
#[derive(Debug, Subcommand)]
pub enum ArchCommand {
    /// Render the workspace tree (crate paths only).
    Tree {
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
    /// Render the workspace tree including extra_dirs.
    TreeFull {
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
    /// List workspace member paths (one per line).
    Members {
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
    /// Print the direct dependency check matrix.
    DirectChecks {
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
}

pub fn execute(cmd: ArchCommand) -> ExitCode {
    let app = CliApp::new();
    let result = match cmd {
        ArchCommand::Tree { project_root } => app.arch_tree(&project_root),
        ArchCommand::TreeFull { project_root } => app.arch_tree_full(&project_root),
        ArchCommand::Members { project_root } => app.arch_members(&project_root),
        ArchCommand::DirectChecks { project_root } => app.arch_direct_checks(&project_root),
    };
    outcome_to_exit(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;

    use clap::Parser;
    use tempfile::TempDir;

    use super::ArchCommand;
    use crate::commands::arch::execute;

    // ── CLI parsing tests ────────────────────────────────────────────────────

    /// Minimal parser wrapper for testing argument parsing in isolation.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: ArchCommand,
    }

    #[test]
    fn test_arch_tree_parses_with_default_project_root() {
        let cli = TestCli::try_parse_from(["test", "tree"]).unwrap();
        match cli.cmd {
            ArchCommand::Tree { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected Tree, got {other:?}"),
        }
    }

    #[test]
    fn test_arch_tree_parses_with_explicit_project_root() {
        let cli =
            TestCli::try_parse_from(["test", "tree", "--project-root", "/some/path"]).unwrap();
        match cli.cmd {
            ArchCommand::Tree { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), "/some/path");
            }
            other => panic!("expected Tree, got {other:?}"),
        }
    }

    #[test]
    fn test_arch_tree_full_parses_with_default_project_root() {
        let cli = TestCli::try_parse_from(["test", "tree-full"]).unwrap();
        match cli.cmd {
            ArchCommand::TreeFull { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected TreeFull, got {other:?}"),
        }
    }

    #[test]
    fn test_arch_members_parses_with_default_project_root() {
        let cli = TestCli::try_parse_from(["test", "members"]).unwrap();
        match cli.cmd {
            ArchCommand::Members { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected Members, got {other:?}"),
        }
    }

    #[test]
    fn test_arch_direct_checks_parses_with_default_project_root() {
        let cli = TestCli::try_parse_from(["test", "direct-checks"]).unwrap();
        match cli.cmd {
            ArchCommand::DirectChecks { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected DirectChecks, got {other:?}"),
        }
    }

    #[test]
    fn test_arch_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["test", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized arch subcommand must be rejected by clap");
    }

    // ── Integration tests (dispatch with minimal architecture-rules.json) ─────

    const MINIMAL_RULES: &str = r#"{
  "layers": [
    { "crate": "domain",  "path": "libs/domain",  "may_depend_on": [] },
    { "crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"] }
  ]
}"#;

    fn setup_dir(rules_json: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();
        dir
    }

    #[test]
    fn test_arch_tree_dispatch_succeeds_with_valid_rules() {
        let dir = setup_dir(MINIMAL_RULES);
        let exit = execute(ArchCommand::Tree { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_arch_tree_full_dispatch_succeeds_with_valid_rules() {
        let dir = setup_dir(MINIMAL_RULES);
        let exit = execute(ArchCommand::TreeFull { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_arch_members_dispatch_succeeds_with_valid_rules() {
        let dir = setup_dir(MINIMAL_RULES);
        let exit = execute(ArchCommand::Members { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_arch_direct_checks_dispatch_succeeds_with_valid_rules() {
        let dir = setup_dir(MINIMAL_RULES);
        let exit = execute(ArchCommand::DirectChecks { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_arch_tree_dispatch_fails_with_missing_rules_file() {
        let dir = TempDir::new().unwrap();
        let exit = execute(ArchCommand::Tree { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }

    #[test]
    fn test_arch_tree_full_dispatch_fails_with_missing_rules_file() {
        let dir = TempDir::new().unwrap();
        let exit = execute(ArchCommand::TreeFull { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }

    #[test]
    fn test_arch_members_dispatch_fails_with_missing_rules_file() {
        let dir = TempDir::new().unwrap();
        let exit = execute(ArchCommand::Members { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }

    #[test]
    fn test_arch_direct_checks_dispatch_fails_with_missing_rules_file() {
        let dir = TempDir::new().unwrap();
        let exit = execute(ArchCommand::DirectChecks { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }
}
