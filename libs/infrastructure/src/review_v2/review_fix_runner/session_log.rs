use std::io::{Seek, SeekFrom, Write};

use super::spawn::RuntimeFile;
use usecase::review_v2::run_review_fix::ReviewFixRunnerError;

/// Names of environment variables that may carry provider authentication
/// credentials. Any non-empty value for these vars must be redacted before a
/// subprocess diagnostic is displayed or written to a persistent log
/// (`knowledge/conventions/security.md`).
///
/// The Codex fixer forwards only its allowlisted subset through
/// `build_safe_env`; reviewer adapters inherit the host provider environment.
pub(super) const CREDENTIAL_VARS: &[&str] = &[
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "OPENAI_ORG_ID",
    "OPENAI_BASE_URL",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "XAI_API_KEY",
];

/// The number of trailing bytes a streaming redactor must retain before
/// emitting a chunk. A credential can begin this many bytes before a pipe
/// boundary and finish in the next read.
pub(super) fn credential_redaction_overlap_bytes() -> usize {
    credential_values()
        .iter()
        .map(|(_, value)| value)
        .map(|value| value.len())
        .max()
        .unwrap_or_default()
}

pub(super) fn credential_values() -> Vec<(&'static str, String)> {
    CREDENTIAL_VARS
        .iter()
        .filter_map(|&var| std::env::var(var).ok().map(|value| (var, value)))
        .filter(|(_, value)| !value.is_empty())
        .collect()
}

/// Replaces every non-empty credential value found in `text` with a
/// `[REDACTED:<VAR_NAME>]` placeholder.  Empty values are never replaced —
/// replacing an empty string would corrupt the entire log.
pub(crate) fn redact_credentials(text: &str) -> String {
    redact_credential_values(text, credential_values())
}

pub(super) fn redact_credential_values<'a>(
    text: &str,
    values: impl IntoIterator<Item = (&'a str, String)>,
) -> String {
    let mut values: Vec<(&str, String)> =
        values.into_iter().filter(|(_, val)| !val.is_empty()).collect();
    values.sort_by(|(var_a, val_a), (var_b, val_b)| {
        val_b.len().cmp(&val_a.len()).then_with(|| var_a.cmp(var_b))
    });
    let mut result = text.to_owned();
    for (var, val) in values {
        let placeholder = format!("[REDACTED:{var}]");
        result = result.replace(&val, &placeholder);
    }
    result
}

pub(super) fn write_session_log(
    log_file: &mut RuntimeFile,
    bin: &std::ffi::OsStr,
    exit_status: &str,
    stdout: &str,
    stderr: &str,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) -> Result<(), ReviewFixRunnerError> {
    let bin_display = bin.to_string_lossy();
    let redacted_stdout = redact_credentials(stdout);
    let redacted_stderr = redact_credentials(stderr);
    let runtime_header = runtime.map(crate::codex_common::runtime_log_header).unwrap_or_default();
    let log_content = format!(
        "=== codex fixer session log ===\nbin: {bin_display}\n{runtime_header}exit_status: {exit_status}\n\n\
         === STDOUT ===\n{redacted_stdout}\n\
         === STDERR ===\n{redacted_stderr}"
    );
    let result = log_file
        .verify_path_identity()
        .and_then(|()| log_file.file.set_len(0))
        .and_then(|()| log_file.file.seek(SeekFrom::Start(0)))
        .and_then(|_| log_file.file.write_all(log_content.as_bytes()));
    result.map_err(|error| {
        ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(format!(
            "failed to write session log {}: {error}",
            log_file.path().display()
        )))
    })
}

/// Deletes the session log file on drop unless told to keep it.
///
/// Default behaviour is to remove the log when the guard is dropped (clean
/// successful run).  Call [`keep_for_diagnosis`] on the failure path so the
/// log survives for the caller to inspect.
///
/// [`keep_for_diagnosis`]: SessionLogCleanup::keep_for_diagnosis
pub(super) struct SessionLogCleanup {
    file: RuntimeFile,
    /// When `true` (the default), drop removes the file.
    /// Set to `false` via `keep_for_diagnosis` to retain the file.
    remove_on_drop: bool,
}

impl SessionLogCleanup {
    pub(super) fn new(file: RuntimeFile) -> Self {
        Self { file, remove_on_drop: true }
    }

    /// Prevents the log from being deleted on drop so it can be used for diagnosis.
    pub(super) fn keep_for_diagnosis(mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for SessionLogCleanup {
    fn drop(&mut self) {
        if self.remove_on_drop {
            self.file.remove();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::super::spawn::create_runtime_file;
    use super::*;

    // ── redact_credentials ────────────────────────────────────────────────────

    /// Run `redact_credentials` in isolation without mutating the real env
    /// (Rust 2024 forbids `std::env::set_var` inside tests due to
    /// `#![forbid(unsafe_code)]`).  We test the pure helper directly by
    /// constructing a list of `(name, value)` pairs and applying the same
    /// replacement logic.
    fn apply_redaction_with(text: &str, pairs: &[(&str, &str)]) -> String {
        redact_credential_values(text, pairs.iter().map(|(var, val)| (*var, (*val).to_owned())))
    }

    #[test]
    fn test_redact_credentials_replaces_non_empty_value_in_captured_output() {
        let fake_key = "sk-FAKE-SECRET-123456";
        let captured = format!("Running codex...\nAuthorization: Bearer {fake_key}\nDone.");

        let redacted = apply_redaction_with(&captured, &[("OPENAI_API_KEY", fake_key)]);

        assert!(
            !redacted.contains(fake_key),
            "redacted output must not contain the original secret value"
        );
        assert!(
            redacted.contains("[REDACTED:OPENAI_API_KEY]"),
            "redacted output must contain the placeholder"
        );
    }

    #[test]
    fn test_redact_credentials_with_empty_value_does_not_corrupt_output() {
        let captured = "Running codex...\nNo secret here.\nDone.";

        // Empty value: must be skipped to avoid replacing every empty-string match.
        let redacted = apply_redaction_with(captured, &[("OPENAI_API_KEY", "")]);

        assert_eq!(
            redacted, captured,
            "empty credential value must leave the output completely unchanged"
        );
    }

    #[test]
    fn test_redact_credentials_handles_multiple_provider_vars_independently() {
        let key_val = "sk-FAKE-OPENAI-KEY";
        let codex_val = "ck-FAKE-CODEX-KEY";
        let org_val = "org-FAKE-ORG";
        let base_url_val = "https://token@example.invalid/v1";
        let anthropic_key_val = "sk-ant-FAKE-KEY";
        let anthropic_token_val = "anthropic-FAKE-TOKEN";
        let xai_key_val = "xai-FAKE-KEY";
        let captured = format!(
            "key={key_val} codex={codex_val} org={org_val} base={base_url_val} \
             anthropic_key={anthropic_key_val} anthropic_token={anthropic_token_val} \
             xai={xai_key_val} other=plaintext"
        );

        let redacted = apply_redaction_with(
            &captured,
            &[
                ("OPENAI_API_KEY", key_val),
                ("CODEX_API_KEY", codex_val),
                ("OPENAI_ORG_ID", org_val),
                ("OPENAI_BASE_URL", base_url_val),
                ("ANTHROPIC_API_KEY", anthropic_key_val),
                ("ANTHROPIC_AUTH_TOKEN", anthropic_token_val),
                ("XAI_API_KEY", xai_key_val),
            ],
        );

        assert!(!redacted.contains(key_val), "OPENAI_API_KEY value must be redacted");
        assert!(!redacted.contains(codex_val), "CODEX_API_KEY value must be redacted");
        assert!(!redacted.contains(org_val), "OPENAI_ORG_ID value must be redacted");
        assert!(!redacted.contains(base_url_val), "OPENAI_BASE_URL value must be redacted");
        assert!(!redacted.contains(anthropic_key_val), "ANTHROPIC_API_KEY value must be redacted");
        assert!(
            !redacted.contains(anthropic_token_val),
            "ANTHROPIC_AUTH_TOKEN value must be redacted"
        );
        assert!(!redacted.contains(xai_key_val), "XAI_API_KEY value must be redacted");
        assert!(redacted.contains("other=plaintext"), "non-credential content must be preserved");
        assert!(redacted.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(redacted.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(redacted.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(redacted.contains("[REDACTED:OPENAI_BASE_URL]"));
        assert!(redacted.contains("[REDACTED:ANTHROPIC_API_KEY]"));
        assert!(redacted.contains("[REDACTED:ANTHROPIC_AUTH_TOKEN]"));
        assert!(redacted.contains("[REDACTED:XAI_API_KEY]"));
    }

    #[test]
    fn test_redact_credentials_replaces_longest_overlapping_value_first() {
        let short_val = "sk-overlap";
        let long_val = "sk-overlap-secret";
        let captured = format!("short={short_val} long={long_val}");

        let redacted = apply_redaction_with(
            &captured,
            &[("OPENAI_API_KEY", short_val), ("CODEX_API_KEY", long_val)],
        );

        assert!(!redacted.contains(short_val), "short credential value must be redacted");
        assert!(!redacted.contains(long_val), "long credential value must be redacted");
        assert!(
            !redacted.contains("-secret"),
            "suffix of overlapping credential value must not leak"
        );
        assert!(redacted.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(redacted.contains("[REDACTED:CODEX_API_KEY]"));
    }

    #[test]
    fn test_credential_vars_include_all_provider_auth_vars() {
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"CODEX_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_ORG_ID"));
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_BASE_URL"));
        assert!(CREDENTIAL_VARS.contains(&"ANTHROPIC_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"ANTHROPIC_AUTH_TOKEN"));
        assert!(CREDENTIAL_VARS.contains(&"XAI_API_KEY"));
    }

    #[test]
    fn test_write_session_log_rejects_removed_runtime_path() {
        let repository = tempfile::tempdir().expect("repository fixture");
        let mut log_file = create_runtime_file(repository.path(), "session-log", "txt")
            .expect("create runtime file");
        log_file.remove();

        let error = write_session_log(
            &mut log_file,
            std::ffi::OsStr::new("codex"),
            "exit status: 1",
            "stdout",
            "stderr",
            None,
        )
        .expect_err("missing runtime path must be reported");

        assert!(error.to_string().contains("failed to write session log"));
    }

    #[cfg(unix)]
    #[test]
    fn test_write_session_log_rejects_replaced_runtime_path() {
        let repository = tempfile::tempdir().expect("repository fixture");
        let mut log_file = create_runtime_file(repository.path(), "session-log", "txt")
            .expect("create runtime file");
        log_file.remove();
        std::fs::write(log_file.path(), "replacement log entry").expect("replace runtime path");

        let error = write_session_log(
            &mut log_file,
            std::ffi::OsStr::new("codex"),
            "exit status: 1",
            "stdout",
            "stderr",
            None,
        )
        .expect_err("replacement runtime path must be reported");

        assert!(error.to_string().contains("failed to write session log"));
        assert_eq!(
            std::fs::read_to_string(log_file.path()).expect("read replacement path"),
            "replacement log entry"
        );
    }
}
