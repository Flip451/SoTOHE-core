//! Provider-facing adapters for generic capability dispatch.

pub mod agent_profiles;
pub mod claude;
pub mod codex;
pub(crate) mod front_matter;
pub(crate) mod path_guard;
pub(crate) mod process;
pub(crate) mod prompt;
pub(crate) mod read;
pub(crate) mod session;
pub mod source;

use usecase::capability_exec::{
    CapabilityDispatchRequest, CapabilityExecError, CapabilityFailureDetail, ProviderName,
};

pub(crate) use front_matter::{
    YAML_LINE_BREAKS, parse_provider_definition_front_matter, read_front_matter,
};
pub(crate) use process::{ProviderProcessRunner, system_process_runner};
pub(crate) use prompt::capability_prompt;
pub(crate) use read::{bounded_read_utf8_file, read_utf8_file};

#[cfg(test)]
pub(crate) use process::{run_provider_process_with_timeout, spawn_bounded_log_writer};

/// Maximum size of a repository-authored capability-dispatch text file.
///
/// Four MiB is intentionally generous for profile, provider-definition, briefing, and
/// discipline files while bounding memory consumed before their contents are validated.
pub(crate) const MAX_CAPABILITY_EXEC_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// Maximum stderr retained for one provider subprocess session log.
const MAX_CAPABILITY_EXEC_LOG_BYTES: usize = 4 * 1024 * 1024;

fn adapter_preflight_error(
    request: &CapabilityDispatchRequest,
    provider: &ProviderName,
    detail: impl Into<String>,
) -> CapabilityExecError {
    CapabilityExecError::AdapterPreflight {
        capability: request.request.capability.clone(),
        provider: provider.clone(),
        detail: CapabilityFailureDetail::new(detail),
    }
}

pub(crate) fn dispatch_error(
    provider: &ProviderName,
    detail: impl Into<String>,
) -> CapabilityExecError {
    CapabilityExecError::DispatchFailed {
        provider: provider.clone(),
        detail: CapabilityFailureDetail::new(detail),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{Cursor, ErrorKind};
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::{
        MAX_CAPABILITY_EXEC_LOG_BYTES, MAX_CAPABILITY_EXEC_TEXT_BYTES,
        parse_provider_definition_front_matter, read_front_matter, read_utf8_file,
        run_provider_process_with_timeout, spawn_bounded_log_writer,
    };
    use usecase::capability_exec::ProviderName;

    #[test]
    fn test_read_utf8_file_oversize_provider_definition_is_rejected() {
        let directory = tempfile::tempdir().expect("test directory is created");
        let definition = directory.path().join("definition.md");
        File::create(&definition)
            .expect("definition fixture is created")
            .set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)
            .expect("definition fixture size is set");

        let error = read_utf8_file(&definition, directory.path())
            .expect_err("oversize definition is rejected");

        assert!(error.contains("exceeds the maximum allowed size"));
    }

    #[test]
    fn test_bounded_read_utf8_file_directory_returns_regular_file_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;

        let error =
            super::bounded_read_utf8_file(directory.path()).expect_err("directory rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("not a regular file"));
        Ok(())
    }

    #[test]
    fn test_bounded_log_writer_over_limit_keeps_log_within_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider.log");
        let file = File::create(&path)?;
        let stderr = Cursor::new(vec![b'x'; MAX_CAPABILITY_EXEC_LOG_BYTES + 1]);

        let result = spawn_bounded_log_writer(stderr, file)
            .recv_timeout(Duration::from_secs(1))
            .map_err(|_| "bounded log writer did not complete")?;
        result?;

        assert!(std::fs::metadata(path)?.len() <= MAX_CAPABILITY_EXEC_LOG_BYTES as u64);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_bounded_read_utf8_file_fifo_is_rejected_before_opening()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let fifo = directory.path().join("definition.fifo");
        let status = Command::new("mkfifo").arg(&fifo).status()?;
        assert!(status.success(), "mkfifo must create the FIFO fixture");

        let started = Instant::now();
        let error = super::bounded_read_utf8_file(&fifo).expect_err("FIFO rejected before open");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
        assert!(started.elapsed() < Duration::from_secs(1), "FIFO rejection must not block");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_run_provider_process_symlinked_runtime_directory_returns_error_before_spawn()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let repository = workspace.path().join("repository");
        let outside = workspace.path().join("outside");
        std::fs::create_dir_all(&repository)?;
        std::fs::create_dir_all(&outside)?;
        std::os::unix::fs::symlink(&outside, repository.join("tmp"))?;
        let provider = ProviderName::try_new("codex")?;

        let error = run_provider_process_with_timeout(
            "unreachable-provider-binary",
            None,
            &[],
            &repository,
            &repository.join("tmp/capability-runtime"),
            &provider,
            Some(Duration::from_secs(1)),
            None,
        )
        .expect_err("symlinked runtime directory is rejected before process spawn");

        assert!(error.to_string().contains("refusing to follow symlink"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_run_provider_process_timeout_terminates_process_group()
    -> Result<(), Box<dyn std::error::Error>> {
        let repository = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex")?;
        let args = [OsString::from("-c"), OsString::from("sleep 60")];

        let error = run_provider_process_with_timeout(
            "sh",
            None,
            &args,
            repository.path(),
            &repository.path().join("tmp/capability-runtime"),
            &provider,
            Some(Duration::from_millis(1)),
            None,
        )
        .expect_err("provider process exceeds timeout");

        assert!(error.to_string().contains("timed out"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_run_provider_process_without_timeout_waits_for_completion()
    -> Result<(), Box<dyn std::error::Error>> {
        let repository = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex")?;
        let args = [OsString::from("-c"), OsString::from("sleep 0.2; exit 7")];

        let output = run_provider_process_with_timeout(
            "sh",
            None,
            &args,
            repository.path(),
            &repository.path().join("tmp/capability-runtime"),
            &provider,
            None,
            None,
        )?;

        assert_eq!(output.exit_code, 7);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_run_provider_process_detached_stderr_returns_after_bounded_drain()
    -> Result<(), Box<dyn std::error::Error>> {
        let repository = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex")?;
        let args = [OsString::from("-c"), OsString::from("(sleep 60) >&2 & exit 0")];

        let started = Instant::now();
        let error = run_provider_process_with_timeout(
            "sh",
            None,
            &args,
            repository.path(),
            &repository.path().join("tmp/capability-runtime"),
            &provider,
            Some(Duration::from_secs(1)),
            None,
        )
        .expect_err("a detached stderr holder must not hang dispatch");

        assert!(error.to_string().contains("stderr drain did not close"));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "logger drain timeout must bound dispatch completion"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_read_utf8_file_rejects_symlinked_component_before_parent() {
        let workspace = tempfile::tempdir().expect("workspace fixture is created");
        let repository = workspace.path().join("repository");
        std::fs::create_dir_all(&repository).expect("repository fixture is created");
        std::fs::write(workspace.path().join("definition.md"), "definition")
            .expect("external definition is written");
        std::os::unix::fs::symlink(workspace.path(), repository.join("internal-link"))
            .expect("internal symlink is created");
        let root = repository.join("internal-link/..");
        let path = root.join("definition.md");

        let error = read_utf8_file(&path, &root).expect_err("symlink before parent is rejected");

        assert!(error.contains("refusing to follow symlink"));
    }

    #[test]
    fn test_read_front_matter_closed_block_returns_contents() {
        let content = "---\nname: implementer\nsandbox: workspace-write\n---\nbody\n";

        assert_eq!(
            read_front_matter(content).expect("front matter parses"),
            Some("name: implementer\nsandbox: workspace-write")
        );
    }

    #[test]
    fn test_read_front_matter_reads_a_wholly_crlf_definition() {
        // The bytes a definition has in a checkout that uses CRLF: every break
        // is `\r\n`, both delimiters included. Nothing about it is exotic, so
        // answering `None` here would be this reader telling every caller that
        // an ordinary file declares nothing — silently for the convention
        // codec, which reads `None` as an empty declaration.
        let content = "---\r\nname: implementer\r\ndescription: Implements assigned tasks.\r\nsandbox: workspace-write\r\n---\r\nbody\r\n";

        let block = read_front_matter(content)
            .expect("a CRLF block is closed by its CRLF delimiter")
            .expect("a CRLF document opens a block");

        assert_eq!(
            block,
            "name: implementer\r\ndescription: Implements assigned tasks.\r\nsandbox: workspace-write",
            "the block is found and handed back whole, with the break before the closing \
             delimiter removed and the ones inside it left as the file spelled them"
        );
        let parsed = parse_provider_definition_front_matter(block)
            .expect("a YAML parser folds a CRLF break itself, so the block needs no rewriting");
        assert_eq!(parsed.validate_identity("implementer", "provider definition"), Ok(()));
        assert_eq!(
            parsed.sandbox(),
            Ok(Some("workspace-write")),
            "the declaration a CRLF definition would otherwise have lost is the one worth not \
             losing quietly: a silently defaulted sandbox is a read-only dispatch that was asked \
             to be able to write"
        );
    }

    #[test]
    fn test_read_front_matter_decides_a_block_by_its_content_and_not_its_line_endings() {
        // Every spelling of the same document, compared outcome for outcome:
        // an LF file, a CRLF file, one whose delimiters and body disagree, and
        // one closed at the end of the file. A fix reaching only the opening
        // delimiter passes the first of these and turns the rest into
        // unclosed-block rejections, which is a fail-closed regression on
        // ordinary files rather than a fix.
        let spellings = [
            "---\nname: implementer\n---\nbody\n",
            "---\r\nname: implementer\r\n---\r\nbody\r\n",
            "---\nname: implementer\r\n---\nbody\n",
            "---\r\nname: implementer\n---\r\nbody\n",
            "---\nname: implementer\n---",
            "---\r\nname: implementer\r\n---",
        ];

        for content in spellings {
            assert_eq!(
                read_front_matter(content),
                Ok(Some("name: implementer")),
                "{content:?} carries one block holding one declaration, so that is what it reads \
                 as — not an absent block and not an unclosed one"
            );
        }
    }

    #[test]
    fn test_read_front_matter_opens_and_closes_on_every_break_the_parser_recognises() {
        // The break form is the only thing that varies: same delimiters, same
        // single declaration, same body. So the only way a spelling can read as
        // one block holding `name: implementer` is if this reader opens,
        // scans, closes, and trims on that break — a set narrower anywhere
        // makes the file read as carrying no block, or as opening one nothing
        // closes, and either answer fails here.
        //
        // The set is the parser's own, established by feeding `serde_yaml` the
        // input rather than by reading a specification: NEL, LS, and PS are
        // YAML 1.1 breaks that 1.2 dropped, and this parser accepts them.
        for (name, line_break) in [
            ("LF", "\n"),
            ("CR", "\r"),
            ("CRLF", "\r\n"),
            ("NEL", "\u{85}"),
            ("LS", "\u{2028}"),
            ("PS", "\u{2029}"),
        ] {
            let content =
                format!("---{line_break}name: implementer{line_break}---{line_break}body");

            assert_eq!(
                read_front_matter(&content),
                Ok(Some("name: implementer")),
                "a definition broken by {name} carries the same block as any other spelling of it"
            );
        }
    }

    #[test]
    fn test_read_front_matter_reads_a_definition_written_with_a_byte_order_mark() {
        // An editor may put a mark at the head of a UTF-8 file. It sits in
        // front of the opening delimiter, so a reader matching `---` at byte
        // zero finds none and reports the block absent — and absent is the
        // silent answer, which for a provider definition means `sandbox`,
        // `model`, and `tools` all quietly taking their defaults.
        let content = "\u{FEFF}---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nbody\n";

        let block = read_front_matter(content)
            .expect("a mark makes a file no less well formed")
            .expect("the mark is dropped rather than hiding the block behind it");

        assert_eq!(
            block,
            "name: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write"
        );

        let parsed = parse_provider_definition_front_matter(block)
            .expect("the block a marked file carries parses like any other");
        assert_eq!(parsed.validate_identity("implementer", "provider definition"), Ok(()));
        assert_eq!(
            parsed.sandbox(),
            Ok(Some("workspace-write")),
            "the declaration a marked definition would otherwise have lost silently"
        );
    }

    #[test]
    fn test_read_front_matter_without_a_block_is_distinguished_from_an_unclosed_one() {
        // The two answers this reader must not confuse. A file with no opening
        // delimiter carries no front matter, and one that opens a block nothing
        // closes is malformed — and a file that is nothing but a delimiter is
        // the former, since a lone `---` opens nothing to leave unclosed.
        for absent in ["body only\n", "not front matter\n---\nname: implementer\n---\n", "---"] {
            assert_eq!(
                read_front_matter(absent).expect("no block is opened, so none is unclosed"),
                None,
                "{absent:?} carries no front matter"
            );
        }

        for unclosed in [
            "---\nname: implementer\n",
            "---\r\nname: implementer\r\n",
            "---\nname: implementer\n--- trailing\n",
        ] {
            let error =
                read_front_matter(unclosed).expect_err("an opened block that nothing closes");

            assert_eq!(
                error, "adapter definition has an unclosed YAML front matter block",
                "the rejection {unclosed:?} produces is the one this reader's callers report"
            );
        }
    }

    #[test]
    fn test_read_front_matter_keeps_an_empty_block_apart_from_an_absent_one() {
        // A block that closes immediately is empty rather than unclosed, and an
        // empty block is not the same answer as no block at all: the convention
        // codec reads the two the same way today, but only because it is
        // careful to, and reporting a closed block as unclosed would be this
        // reader misdescribing the file.
        for content in ["---\n---\n", "---\r\n---\r\n", "---\n\n---\n"] {
            assert_eq!(
                read_front_matter(content),
                Ok(Some("")),
                "{content:?} holds an empty block rather than an unclosed one"
            );
        }
    }

    #[test]
    fn test_provider_front_matter_top_level_tools_list_is_detected() {
        let front_matter = "name: implementer\ndescription: Implements assigned tasks.\ntools:\n  - Read\n  - Edit";
        let parsed = parse_provider_definition_front_matter(front_matter)
            .expect("top-level tools list parses");

        assert_eq!(parsed.tools(), Some(vec!["Read", "Edit"]));
        assert_eq!(parsed.validate_identity("implementer", "provider definition"), Ok(()));
    }

    #[test]
    fn test_provider_front_matter_identity_rejects_missing_empty_and_mismatched_fields() {
        let missing_name = parse_provider_definition_front_matter(
            "description: Implements assigned tasks.\nsandbox: read-only",
        )
        .expect("front matter without a name still decodes for validation");
        assert!(missing_name.validate_identity("implementer", "provider definition").is_err());

        let empty_name = parse_provider_definition_front_matter(
            "name: '  '\ndescription: Implements assigned tasks.",
        )
        .expect("front matter with an empty name still decodes for validation");
        assert!(empty_name.validate_identity("implementer", "provider definition").is_err());

        let mismatched_name = parse_provider_definition_front_matter(
            "name: researcher\ndescription: Researches the workspace.",
        )
        .expect("front matter with a different name still decodes for validation");
        assert!(mismatched_name.validate_identity("implementer", "provider definition").is_err());

        let missing_description = parse_provider_definition_front_matter("name: implementer")
            .expect("front matter without a description still decodes for validation");
        assert!(
            missing_description.validate_identity("implementer", "provider definition").is_err()
        );
    }

    #[test]
    fn test_provider_front_matter_malformed_name_is_rejected() {
        let front_matter = "name:\n  nested: implementer\ndescription: Implements assigned tasks.";

        assert!(parse_provider_definition_front_matter(front_matter).is_err());
    }

    #[test]
    fn test_provider_front_matter_nested_privileged_field_is_rejected() {
        let front_matter = "metadata:\n  sandbox: workspace-write";

        assert!(parse_provider_definition_front_matter(front_matter).is_err());
    }

    #[test]
    fn test_provider_front_matter_malformed_yaml_is_rejected() {
        let front_matter = "sandbox: [workspace-write";

        assert!(parse_provider_definition_front_matter(front_matter).is_err());
    }

    #[test]
    fn test_provider_front_matter_null_privileged_field_is_rejected() {
        let front_matter = "sandbox: null";

        assert!(parse_provider_definition_front_matter(front_matter).is_err());
    }
}
