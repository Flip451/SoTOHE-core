//! `sotp conventions` subcommand group.
//!
//! Each subcommand delegates to the corresponding `CliApp` method and
//! prints the outcome. Exits 0 on success, 1 on error.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;
use cli_composition::ConventionsCompositionRoot;
use cli_driver::conventions::ConventionsInput;
use cli_driver::conventions_resolve::{ConventionCapabilityIdArg, ConventionResolveInput};

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
    /// List the convention documents that declare a capability in `required_for`.
    ///
    /// The capability argument is the boundary mirror
    /// [`ConventionCapabilityIdArg`] rather than a `String`, so the only
    /// rejection this argument has — an empty or whitespace-only identifier — is
    /// made while clap is still parsing and no unvalidated identifier travels
    /// inward (`IN-06`). That mirror is also not `capability exec`'s
    /// `CapabilityNameArg`: this identifier is the query side of an exact-match
    /// comparison and reaches the resolver spelled exactly as it was typed,
    /// whereas the lookup key trims. Nothing here consults
    /// `.harness/capabilities/` or `agent-profiles.json`, so an identifier
    /// registered in neither is an ordinary argument (`AC-09`).
    ///
    /// Declaration order is deliberate: clap renders it in `--help`, and every
    /// sibling variant places `project_root` last.
    Resolve {
        /// Capability identifier matched exactly against `required_for`.
        #[arg(long)]
        capability: ConventionCapabilityIdArg,
        /// Project root directory.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
}

pub fn execute(cmd: ConventionsCommand) -> ExitCode {
    let root = ConventionsCompositionRoot::new();
    let input = match cmd {
        // The read-only variant leaves through its own driver. Folding it into
        // `ConventionsInput` would route resolution through the handler that
        // creates, updates, and indexes documents, which is exactly the reach
        // `AC-06` denies it.
        ConventionsCommand::Resolve { capability, project_root } => {
            let input = ConventionResolveInput { capability, project_root };
            return driver_outcome_to_exit(root.conventions_resolve_driver().handle(input));
        }
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
    driver_outcome_to_exit(root.conventions_driver().handle(input))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
    use std::os::fd::AsRawFd as _;

    use clap::Parser;
    use tempfile::TempDir;

    use cli_driver::conventions_resolve::ConventionCapabilityIdArg;

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

    // ── `resolve` variant ─────────────────────────────────────────────────────

    const DECLARING_DOC: &str = "---\nrequired_for:\n  - implementer\n---\n\n# Declaring\n";

    /// Writes a convention tree holding one document that declares
    /// `implementer`, one that declares an identifier registered nowhere, and
    /// one that declares nothing.
    fn setup_resolvable_conventions(root: &std::path::Path) {
        let dir = root.join("knowledge").join("conventions");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("adr.md"), DECLARING_DOC).unwrap();
        fs::write(
            dir.join("obscure.md"),
            "---\nrequired_for:\n  - not-a-registered-capability\n---\n\n# Obscure\n",
        )
        .unwrap();
        fs::write(dir.join("git-notes.md"), "# Git notes\n").unwrap();
    }

    /// Runs `run` with the process's stdout redirected into a temporary file and
    /// returns its result alongside everything written there.
    ///
    /// `execute` answers the caller by printing, so what a `resolve` dispatch
    /// hands back is only observable at the descriptor. The stderr counterpart of
    /// this helper lives beside the track commands that need it.
    ///
    /// The redirection is **process-wide**, so two callers running at once would
    /// each retarget the other's stdout and read the other's output. The guard
    /// below serialises the whole redirect-run-restore sequence, because it is
    /// the sequence and not the file that must not interleave.
    ///
    /// These tests require a process-per-test runner, which is what
    /// `cargo make ci` uses. Under `cargo test` they run as threads in one
    /// process and libtest installs a thread-local capture, so `println!` never
    /// reaches descriptor 1 and this helper reads an empty file — a limitation
    /// of the technique rather than a race, and one the guard cannot address.
    /// Observing the answer at the descriptor is what makes these tests exercise
    /// what a caller actually sees; the alternative is injecting a writer into
    /// `execute`, which would move an output-shaping decision into the bin.
    fn capture_stdout<T>(run: impl FnOnce() -> T) -> (T, String) {
        static STDOUT_REDIRECT: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _serialised = STDOUT_REDIRECT.lock().unwrap_or_else(|poisoned| {
            // A panic inside another capture leaves stdout restored by its own
            // guard drop, so the descriptor state is sound and only the mutex
            // is poisoned. Recovering keeps one failing test from cascading.
            poisoned.into_inner()
        });
        let mut capture = tempfile::tempfile().unwrap();
        let stdout_fd = std::io::stdout().as_raw_fd();
        let capture_fd = capture.as_raw_fd();
        std::io::stdout().flush().unwrap();

        // Safety: `stdout_fd` is a valid process file descriptor for stdout.
        let saved_fd = unsafe { libc::dup(stdout_fd) };
        assert!(saved_fd >= 0, "dup(stdout) failed");
        // Safety: both descriptors are valid; this redirects stdout to the temp file.
        let redirect_result = unsafe { libc::dup2(capture_fd, stdout_fd) };
        assert_eq!(redirect_result, stdout_fd, "dup2(capture, stdout) failed");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));

        std::io::stdout().flush().unwrap();
        // Safety: `saved_fd` was returned by `dup`; this restores stdout.
        let restore_result = unsafe { libc::dup2(saved_fd, stdout_fd) };
        assert_eq!(restore_result, stdout_fd, "dup2(saved, stdout) failed");
        // Safety: `saved_fd` is no longer needed after restoring stdout.
        let close_result = unsafe { libc::close(saved_fd) };
        assert_eq!(close_result, 0, "close(saved stdout) failed");

        capture.seek(SeekFrom::Start(0)).unwrap();
        let mut output = String::new();
        capture.read_to_string(&mut output).unwrap();

        match result {
            Ok(value) => (value, output),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Reads the convention tree back as sorted `(file name, contents)` pairs.
    fn convention_tree(root: &std::path::Path) -> Vec<(String, String)> {
        let dir = root.join("knowledge").join("conventions");
        let mut documents = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                (name, fs::read_to_string(&path).unwrap())
            })
            .collect::<Vec<_>>();
        documents.sort();
        documents
    }

    #[test]
    fn test_conventions_resolve_parses_the_capability_and_the_project_root() {
        let cli =
            TestCli::try_parse_from(["test", "resolve", "--capability", "implementer"]).unwrap();
        match cli.cmd {
            ConventionsCommand::Resolve { capability, project_root } => {
                assert_eq!(capability, "implementer".parse().unwrap());
                assert_eq!(project_root.to_str().unwrap(), ".");
            }
            other => panic!("expected Resolve, got {other:?}"),
        }

        let cli = TestCli::try_parse_from([
            "test",
            "resolve",
            "--capability",
            "implementer",
            "--project-root",
            "/some/path",
        ])
        .unwrap();
        match cli.cmd {
            ConventionsCommand::Resolve { project_root, .. } => {
                assert_eq!(project_root.to_str().unwrap(), "/some/path");
            }
            other => panic!("expected Resolve, got {other:?}"),
        }
    }

    /// The argument's type, not the command body, is what refuses a blank
    /// identifier: clap never produces a `Resolve` variant for one, so no
    /// unvalidated identifier travels inward.
    #[test]
    fn test_conventions_resolve_rejects_a_blank_capability_at_parse() {
        for blank in ["", "   "] {
            let result = TestCli::try_parse_from(["test", "resolve", "--capability", blank]);
            assert!(result.is_err(), "blank capability {blank:?} must be rejected while parsing");
        }
        assert!(
            TestCli::try_parse_from(["test", "resolve"]).is_err(),
            "resolve without a capability must be rejected by clap"
        );
    }

    /// A padded identifier survives parsing padded. Had this argument been
    /// `capability exec`'s trimming lookup key, it would have arrived equal to
    /// the unpadded one and matched documents the caller did not name.
    #[test]
    fn test_conventions_resolve_keeps_the_capability_spelled_as_typed() {
        let cli =
            TestCli::try_parse_from(["test", "resolve", "--capability", " implementer "]).unwrap();
        match cli.cmd {
            ConventionsCommand::Resolve { capability, .. } => {
                assert_eq!(capability, " implementer ".parse().unwrap());
                assert_ne!(
                    capability,
                    "implementer".parse::<ConventionCapabilityIdArg>().unwrap(),
                    "the boundary must not trim the identifier into a different match term"
                );
            }
            other => panic!("expected Resolve, got {other:?}"),
        }
    }

    #[test]
    fn test_conventions_resolve_help_lists_the_capability_before_the_project_root() {
        // `--help` is reported as a clap error carrying the rendered help text.
        let Err(rendered) = TestCli::try_parse_from(["test", "resolve", "--help"]) else {
            panic!("--help must be reported as a clap error carrying the help text");
        };
        let help = rendered.to_string();

        let capability = help.find("--capability").expect("help must list --capability");
        let project_root = help.find("--project-root").expect("help must list --project-root");
        assert!(
            capability < project_root,
            "declaration order must render the capability first:\n{help}"
        );
    }

    /// The `Resolve` arm reaches the read-only driver rather than the handler
    /// the other three variants share: dispatch succeeds and the convention tree
    /// is byte-identical afterwards, so no document was created, updated,
    /// deleted, or indexed. What the dispatch hands back is asserted by the
    /// sibling test below.
    #[test]
    fn test_conventions_resolve_dispatch_leaves_the_convention_tree_untouched() {
        let dir = TempDir::new().unwrap();
        setup_resolvable_conventions(dir.path());
        let before = convention_tree(dir.path());

        let exit = execute(ConventionsCommand::Resolve {
            capability: "implementer".parse().unwrap(),
            project_root: dir.path().to_path_buf(),
        });

        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        assert_eq!(convention_tree(dir.path()), before);
    }

    /// The other half of what a `resolve` dispatch owes its caller: the answer
    /// it prints is the repository-relative path of each document declaring the
    /// requested capability, one per line.
    #[test]
    fn test_conventions_resolve_dispatch_prints_the_declaring_document_paths() {
        let dir = TempDir::new().unwrap();
        setup_resolvable_conventions(dir.path());

        let (exit, stdout) = capture_stdout(|| {
            execute(ConventionsCommand::Resolve {
                capability: "implementer".parse().unwrap(),
                project_root: dir.path().to_path_buf(),
            })
        });

        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        // Repository-relative, not rooted at the project directory the command
        // was pointed at, and carrying no document that declares something else
        // or declares nothing.
        assert_eq!(
            stdout.lines().collect::<Vec<_>>(),
            vec!["knowledge/conventions/adr.md"],
            "the dispatch must print the declaring document's repository-relative path"
        );
        // Whole-value matching of the identifier, the absence of duplicates, and
        // the order the lines arrive in are guarantees of the resolution this
        // dispatch is handed: it prints that value through and establishes none
        // of the three, so each is asserted where it is decided rather than
        // claimed a second time here.
    }

    /// Writes a populated convention tree in which nothing declares
    /// `implementer`: one document declares a different capability and one
    /// carries no front matter at all.
    ///
    /// Both file names occur nowhere else in the repository, so a dispatch
    /// reading a convention tree other than the one it was pointed at could not
    /// produce this tree's answer by accident.
    fn setup_conventions_declaring_nothing_matching(root: &std::path::Path) {
        let dir = root.join("knowledge").join("conventions");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("zz-fixture-other-declaration.md"),
            "---\nrequired_for:\n  - zz-fixture-some-other-capability\n---\n\n# Other\n",
        )
        .unwrap();
        fs::write(dir.join("zz-fixture-undeclared.md"), "# Undeclared\n").unwrap();
    }

    /// A capability that no document declares is an ordinary success whose
    /// answer is the empty path list, and an empty path list is zero lines
    /// rather than one blank one (`AC-08`).
    ///
    /// The distinction is the whole assertion: stdout is written as a line when
    /// it is present, so representing "no documents" as an empty string instead
    /// of no string at all would make this result read as one unnamed document
    /// to anything consuming the stream line by line. The tree is populated
    /// rather than empty so that the emptiness comes from the resolution and
    /// not from there being nothing to resolve against.
    #[test]
    fn test_conventions_resolve_dispatch_prints_no_line_for_an_empty_result() {
        let dir = TempDir::new().unwrap();
        setup_conventions_declaring_nothing_matching(dir.path());

        let (exit, stdout) = capture_stdout(|| {
            execute(ConventionsCommand::Resolve {
                capability: "implementer".parse().unwrap(),
                project_root: dir.path().to_path_buf(),
            })
        });

        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        assert_eq!(
            stdout.lines().count(),
            0,
            "an empty result must print no line at all, got {stdout:?}"
        );
        assert!(stdout.is_empty(), "an empty result must print nothing, got {stdout:?}");
    }

    /// Writes a convention tree whose single document declares a
    /// whitespace-only capability identifier, which the front-matter codec
    /// refuses.
    ///
    /// The file name occurs nowhere else in the repository, so a dispatch that
    /// somehow read a convention tree other than the one it was pointed at
    /// could not find this document and pass by accident.
    fn setup_unresolvable_conventions(root: &std::path::Path) {
        let dir = root.join("knowledge").join("conventions");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("zz-fixture-blank-declaration.md"),
            "---\nrequired_for:\n  - \"   \"\n---\n\n# Blank declaration\n",
        )
        .unwrap();
    }

    /// The failure the resolver decides arrives at the caller as a failing exit
    /// status carrying no result, rather than as a panic or as a partial answer
    /// printed beside the error.
    ///
    /// This is the only dispatch path in this module reaching the resolver's
    /// error arm; the sibling dispatch tests all resolve successfully.
    #[test]
    fn test_conventions_resolve_dispatch_fails_without_partial_output_on_a_resolver_error() {
        let dir = TempDir::new().unwrap();
        setup_unresolvable_conventions(dir.path());

        let (exit, stdout) = capture_stdout(|| {
            execute(ConventionsCommand::Resolve {
                capability: "implementer".parse().unwrap(),
                project_root: dir.path().to_path_buf(),
            })
        });

        assert_eq!(exit, std::process::ExitCode::FAILURE);
        assert!(
            stdout.trim().is_empty(),
            "a failed resolution must print no result line, got {stdout:?}"
        );
    }

    /// An identifier registered in neither `.harness/capabilities/` nor
    /// `agent-profiles.json` parses like any other and resolves like any other:
    /// the command looks nothing up before dispatching, so the document that
    /// declares such an identifier comes back rather than being passed over.
    #[test]
    fn test_conventions_resolve_accepts_a_capability_registered_in_no_registry() {
        let dir = TempDir::new().unwrap();
        setup_resolvable_conventions(dir.path());

        let cli = TestCli::try_parse_from([
            "test",
            "resolve",
            "--capability",
            "not-a-registered-capability",
            "--project-root",
            dir.path().to_str().unwrap(),
        ])
        .expect("an unregistered capability id must parse like any other");

        let (exit, stdout) = capture_stdout(|| execute(cli.cmd));

        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        // Not erroring is the weaker half. The document whose `required_for`
        // names the unregistered identifier is what the caller is owed, and it
        // is here, so no registry stood between the identifier and its match.
        assert_eq!(
            stdout.lines().collect::<Vec<_>>(),
            vec!["knowledge/conventions/obscure.md"],
            "an identifier in no registry must resolve to the document declaring it"
        );
    }
}
