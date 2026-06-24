//! `sotp conventions` subcommand group.
//!
//! Each subcommand delegates to the corresponding `CliApp` method and
//! prints the outcome. Exits 0 on success, 1 on error.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;
use cli_composition::ConventionsCompositionRoot;
use cli_driver::conventions::ConventionsInput;

use super::driver_outcome_to_exit;

/// Convention document management subcommands.
#[derive(Debug, Subcommand)]
pub enum ConventionsCommand {
    /// Add a new convention document and update the README index.
    Add {
        /// Convention name or title.
        name: String,
        /// ASCII kebab-case file name.
        #[arg(long)]
        slug: Option<String>,
        /// Document title.
        #[arg(long)]
        title: Option<String>,
        /// One-line purpose text.
        #[arg(long)]
        summary: Option<String>,
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
    /// Regenerate README.md index from current convention documents.
    UpdateIndex {
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
    /// Verify that README.md indexes all convention documents.
    VerifyIndex {
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
}

pub fn execute(cmd: ConventionsCommand) -> ExitCode {
    let input = match cmd {
        ConventionsCommand::Add { name, slug, title, summary, project_root } => {
            ConventionsInput::Add { project_root, name, slug, title, summary }
        }
        ConventionsCommand::UpdateIndex { project_root } => {
            ConventionsInput::UpdateIndex { project_root }
        }
        ConventionsCommand::VerifyIndex { project_root } => {
            ConventionsInput::VerifyIndex { project_root }
        }
    };
    driver_outcome_to_exit(ConventionsCompositionRoot::new().conventions_driver().handle(input))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;

    use clap::Parser;
    use tempfile::TempDir;

    use super::ConventionsCommand;
    use crate::commands::conventions::execute;

    // ── CLI parsing tests ────────────────────────────────────────────────────

    /// Minimal parser wrapper for testing argument parsing in isolation.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: ConventionsCommand,
    }

    #[test]
    fn test_conventions_add_parses_with_required_name() {
        let cli = TestCli::try_parse_from(["test", "add", "testing"]).unwrap();
        match cli.cmd {
            ConventionsCommand::Add { name, slug, title, summary, project_root } => {
                assert_eq!(name, "testing");
                assert!(slug.is_none());
                assert!(title.is_none());
                assert!(summary.is_none());
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected Add, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_add_parses_with_all_options() {
        let cli = TestCli::try_parse_from([
            "test",
            "add",
            "My Convention",
            "--slug",
            "my-convention",
            "--title",
            "My Title",
            "--summary",
            "A summary.",
            "--project-root",
            "/some/path",
        ])
        .unwrap();
        match cli.cmd {
            ConventionsCommand::Add { name, slug, title, summary, project_root } => {
                assert_eq!(name, "My Convention");
                assert_eq!(slug.as_deref(), Some("my-convention"));
                assert_eq!(title.as_deref(), Some("My Title"));
                assert_eq!(summary.as_deref(), Some("A summary."));
                assert_eq!(project_root.to_str().unwrap(), "/some/path");
            }
            other => panic!("expected Add, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_update_index_parses_with_default_project_root() {
        let cli = TestCli::try_parse_from(["test", "update-index"]).unwrap();
        match cli.cmd {
            ConventionsCommand::UpdateIndex { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected UpdateIndex, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_update_index_parses_with_explicit_project_root() {
        let cli = TestCli::try_parse_from(["test", "update-index", "--project-root", "/some/path"])
            .unwrap();
        match cli.cmd {
            ConventionsCommand::UpdateIndex { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), "/some/path");
            }
            other => panic!("expected UpdateIndex, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_verify_index_parses_with_default_project_root() {
        let cli = TestCli::try_parse_from(["test", "verify-index"]).unwrap();
        match cli.cmd {
            ConventionsCommand::VerifyIndex { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected VerifyIndex, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_verify_index_parses_with_explicit_project_root() {
        let cli = TestCli::try_parse_from(["test", "verify-index", "--project-root", "/some/path"])
            .unwrap();
        match cli.cmd {
            ConventionsCommand::VerifyIndex { project_root } => {
                assert_eq!(project_root.to_str().unwrap(), "/some/path");
            }
            other => panic!("expected VerifyIndex, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_add_missing_name_is_rejected() {
        let result = TestCli::try_parse_from(["test", "add"]);
        assert!(result.is_err(), "add without name must be rejected by clap");
    }

    #[test]
    fn test_conventions_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["test", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized conventions subcommand must be rejected by clap");
    }

    // ── Integration tests (dispatch with temp dir) ────────────────────────────

    const INDEX_START: &str = "<!-- convention-docs:start -->";
    const INDEX_END: &str = "<!-- convention-docs:end -->";
    const EMPTY_BLOCK_BODY: &str =
        "- No convention documents yet. Add one with `/conventions:add <name>`.";

    fn setup_conventions_dir(root: &std::path::Path) {
        let dir = root.join("knowledge").join("conventions");
        fs::create_dir_all(&dir).unwrap();
        let readme = format!("# Conventions\n\n{INDEX_START}\n{EMPTY_BLOCK_BODY}\n{INDEX_END}\n");
        fs::write(dir.join("README.md"), readme).unwrap();
    }

    #[test]
    fn test_conventions_add_dispatch_succeeds_with_valid_conventions_dir() {
        let dir = TempDir::new().unwrap();
        setup_conventions_dir(dir.path());
        let exit = execute(ConventionsCommand::Add {
            name: "testing".to_owned(),
            slug: None,
            title: None,
            summary: None,
            project_root: dir.path().to_path_buf(),
        });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        assert!(dir.path().join("knowledge/conventions/testing.md").is_file());
    }

    #[test]
    fn test_conventions_add_dispatch_fails_without_conventions_dir() {
        let dir = TempDir::new().unwrap();
        // No conventions dir — README.md is missing.
        let exit = execute(ConventionsCommand::Add {
            name: "testing".to_owned(),
            slug: None,
            title: None,
            summary: None,
            project_root: dir.path().to_path_buf(),
        });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }

    #[test]
    fn test_conventions_update_index_dispatch_succeeds_with_valid_conventions_dir() {
        let dir = TempDir::new().unwrap();
        setup_conventions_dir(dir.path());
        let exit =
            execute(ConventionsCommand::UpdateIndex { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_conventions_update_index_dispatch_fails_without_readme() {
        let dir = TempDir::new().unwrap();
        // Conventions dir exists but no README.md.
        fs::create_dir_all(dir.path().join("knowledge/conventions")).unwrap();
        let exit =
            execute(ConventionsCommand::UpdateIndex { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }

    #[test]
    fn test_conventions_verify_index_dispatch_passes_on_empty_dir() {
        // An empty project root (no conventions dir) returns pass.
        let dir = TempDir::new().unwrap();
        let exit =
            execute(ConventionsCommand::VerifyIndex { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_conventions_verify_index_dispatch_passes_on_synced_index() {
        let dir = TempDir::new().unwrap();
        setup_conventions_dir(dir.path());
        let exit =
            execute(ConventionsCommand::VerifyIndex { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::SUCCESS);
    }

    #[test]
    fn test_conventions_verify_index_dispatch_fails_on_stale_index() {
        let dir = TempDir::new().unwrap();
        let conv_dir = dir.path().join("knowledge/conventions");
        fs::create_dir_all(&conv_dir).unwrap();
        // Write a convention doc.
        fs::write(conv_dir.join("security.md"), "# Security\n").unwrap();
        // Write a stale README that doesn't reference the new doc.
        fs::write(
            conv_dir.join("README.md"),
            format!("# Conventions\n\n{INDEX_START}\n- stale entry\n{INDEX_END}\n"),
        )
        .unwrap();

        let exit =
            execute(ConventionsCommand::VerifyIndex { project_root: dir.path().to_path_buf() });
        assert_eq!(exit, std::process::ExitCode::FAILURE);
    }
}
