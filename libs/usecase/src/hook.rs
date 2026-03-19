//! Hook dispatch use cases (OCP: each hook implements `HookHandler` independently).

use domain::guard::{SimpleCommand, split_shell};
use domain::hook::{HookContext, HookError, HookInput, HookName, HookVerdict};

/// Port for individual hook logic.
/// Receives framework-free HookInput (converted from HookEnvelope at CLI boundary).
///
/// ## Required Field Validation (fail-closed)
///
/// Each handler MUST validate hook-specific required fields from `HookInput`
/// and return `HookError::Input` if they are missing.
///
/// How the CLI maps `HookError::Input` depends on the hook event type:
/// - PreToolUse (guard): `HookError::Input` → exit 2 (block, fail-closed)
///
/// | Hook | Required fields | Missing → |
/// |------|----------------|-----------|
/// | `BlockDirectGitOps` | `tool_name` (always present), `command` | `HookError::Input("missing command")` |
///
/// Note: `tool_name` is guaranteed present (required in `HookEnvelope` serde).
/// `command` and `file_path` are `Option` in `HookInput` because different hooks
/// need different fields. The handler validates what it needs.
pub trait HookHandler: Send + Sync {
    /// Processes a hook event and returns a verdict.
    ///
    /// # Errors
    /// Returns `HookError` on invalid input or subsystem failure.
    fn handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError>;
}

/// Guard hook handler: delegates to `domain::guard::policy::check`.
pub struct GuardHookHandler;

impl HookHandler for GuardHookHandler {
    fn handle(&self, _ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        let command =
            input.command.as_deref().ok_or_else(|| HookError::Input("missing command".into()))?;

        let guard_verdict = domain::guard::policy::check(command);

        if guard_verdict.is_blocked() {
            Ok(HookVerdict::block(guard_verdict.reason))
        } else {
            Ok(HookVerdict::allow())
        }
    }
}

/// Test-file deletion guard handler: blocks `rm` commands targeting test files.
///
/// A file is considered a test file if:
/// - Path contains `tests/` directory segment
/// - Filename matches `*_test.rs`
/// - Filename matches `test_*.rs`
pub struct TestFileDeletionGuardHandler;

impl HookHandler for TestFileDeletionGuardHandler {
    fn handle(&self, _ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        match input.tool_name.as_str() {
            "Write" => {
                // Write with empty content to a test file is equivalent to truncation/deletion.
                // Write with non-empty content is a legitimate edit — allow it.
                // Write to a non-test file is always allowed regardless of content.
                let file_path = match input.file_path.as_ref().and_then(|p| p.to_str()) {
                    Some(p) => p,
                    None => {
                        // Fail-closed: missing or non-UTF-8 file_path on a Write
                        // could bypass the guard, so block it.
                        return Ok(HookVerdict::block(
                            "blocked: Write tool missing file_path (fail-closed)",
                        ));
                    }
                };
                // Fail-closed: None content or empty content both treated as
                // deletion attempts. A missing content field in a Write payload
                // should not silently bypass the guard.
                let has_content = input.content.as_deref().is_some_and(|s| !s.is_empty());
                if is_test_file(file_path) && !has_content {
                    return Ok(HookVerdict::block(format!(
                        "blocked: cannot overwrite test file '{file_path}' with empty or missing content"
                    )));
                }
                return Ok(HookVerdict::allow());
            }
            "Edit" => {
                // Edit operations are targeted replacements, not deletions — always allow.
                return Ok(HookVerdict::allow());
            }
            "Bash" => {
                // Fall through to the rm-detection logic below.
            }
            _ => {
                return Ok(HookVerdict::allow());
            }
        }

        let command =
            input.command.as_deref().ok_or_else(|| HookError::Input("missing command".into()))?;

        // Parse using conch-parser; fail-closed on parse error (block if command
        // mentions "rm" anywhere — unparseable commands are suspicious).
        let commands = match split_shell(command) {
            Ok(cmds) => cmds,
            Err(_) => {
                // Fail-closed: if we can't parse and the raw command mentions rm,
                // block it to be safe.
                if raw_mentions_rm(command) {
                    return Ok(HookVerdict::block("blocked: unparseable command containing rm"));
                }
                return Ok(HookVerdict::allow());
            }
        };

        // Check parsed commands for rm targeting test files, including shell re-entry
        if let Some(verdict) = check_commands_for_test_deletion(&commands, 0) {
            return Ok(verdict);
        }

        Ok(HookVerdict::allow())
    }
}

/// Common shell launchers that act as transparent prefixes before the real command.
const SHELL_LAUNCHERS: &[&str] =
    &["env", "command", "time", "exec", "nice", "nohup", "timeout", "stdbuf", "sudo", "doas"];

/// Shells that support `-c` for inline command execution.
const REENTRY_SHELLS: &[&str] = &["bash", "sh", "zsh", "dash", "ksh", "ash"];

/// Maximum recursion depth for shell re-entry detection to prevent infinite loops.
const MAX_REENTRY_DEPTH: u8 = 3;

/// Recursively checks a list of parsed commands for rm invocations targeting test files.
/// Handles direct rm commands and shell re-entry (`bash -c '...'`).
/// Returns `Some(HookVerdict)` if a blocking verdict is needed, `None` to allow.
fn check_commands_for_test_deletion(commands: &[SimpleCommand], depth: u8) -> Option<HookVerdict> {
    for cmd in commands {
        // Direct rm check
        if argv_has_rm(cmd) {
            let rm_args = extract_rm_args_from_argv(cmd);
            for arg in &rm_args {
                if is_test_file(arg) {
                    return Some(HookVerdict::block(format!(
                        "blocked: cannot delete test file '{arg}'"
                    )));
                }
            }
        }
        // Shell re-entry check (with depth limit)
        if let Some(inner) = extract_shell_reentry_arg(cmd) {
            if depth >= MAX_REENTRY_DEPTH {
                // Fail-closed: depth limit reached — block if inner payload mentions rm
                if raw_mentions_rm(&inner) {
                    return Some(HookVerdict::block(
                        "blocked: shell re-entry depth limit reached with rm in payload"
                            .to_string(),
                    ));
                }
                continue;
            }
            // Always parse the -c payload — do not pre-filter with raw_mentions_rm,
            // because shell escaping/quoting can obfuscate the rm token.
            match split_shell(&inner) {
                Ok(inner_cmds) => {
                    if let Some(verdict) = check_commands_for_test_deletion(&inner_cmds, depth + 1)
                    {
                        return Some(verdict);
                    }
                }
                Err(_) => {
                    // Fail-closed: unparseable inner command — block if it mentions rm
                    if raw_mentions_rm(&inner) {
                        return Some(HookVerdict::block(
                            "blocked: unparseable shell re-entry command containing rm".to_string(),
                        ));
                    }
                }
            }
        }
    }
    None
}

/// If `cmd` is a shell re-entry like `bash -c 'rm tests/foo.rs'`, returns the
/// inner command string. Scans all argv tokens for a REENTRY_SHELL followed by
/// `-c`, so it works even with launcher flags like `sudo -u root bash -c ...`
/// or `env -i bash -c ...`.
fn extract_shell_reentry_arg(cmd: &SimpleCommand) -> Option<String> {
    // Scan for any token that is a re-entry shell, followed somewhere by -c <arg>
    for (i, token) in cmd.argv.iter().enumerate() {
        let name = token.rsplit('/').next().unwrap_or(token).to_lowercase();
        if !REENTRY_SHELLS.contains(&name.as_str()) {
            continue;
        }
        // Look for -c in the remaining tokens (standalone or combined: -lc, -ce, etc.)
        let rest = cmd.argv.get(i + 1..)?;
        for (j, arg) in rest.iter().enumerate() {
            if arg == "-c" {
                // Standalone -c: next arg is the command
                return rest.get(j + 1).cloned();
            }
            // Combined short flags containing 'c' (e.g., -lc, -ce, -ec)
            // Per POSIX/Bash: `-c` consumes the next operand as a command string.
            // Characters after 'c' in combined flags are other flags, NOT the
            // command payload (e.g., `-ce` = `-c -e`, next arg is the command).
            // This applies regardless of where 'c' appears in the flags.
            if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
                let flags = &arg[1..]; // strip leading '-'
                if flags.contains('c') {
                    // Any combined flag with 'c': next arg is the command
                    return rest.get(j + 1).cloned();
                }
            }
        }
    }
    None
}

/// Returns `true` if `token` (already quote-stripped by conch-parser) is a known
/// shell launcher. Handles both bare names (`sudo`) and absolute paths (`/usr/bin/sudo`).
fn is_shell_launcher(token: &str) -> bool {
    let name = token.rsplit('/').next().unwrap_or(token);
    SHELL_LAUNCHERS.contains(&name)
}

/// Returns `true` if the raw command string mentions `rm` in any form that could
/// be an rm invocation. Used as a fail-closed fallback when conch-parser cannot
/// parse the command. Intentionally broad to avoid false negatives — catches
/// bare `rm`, quoted `"rm"`, `/bin/rm`, etc.
fn raw_mentions_rm(command: &str) -> bool {
    // Check for rm as a word (preceded by whitespace/start and followed by whitespace/end)
    // Also catches /bin/rm, "rm", 'rm', etc.
    let bytes = command.as_bytes();
    bytes.windows(2).enumerate().any(|(i, w)| {
        if w != b"rm" {
            return false;
        }
        // Check that 'r' is at a plausible command-name boundary
        // (start of string, after whitespace, after /, after quote)
        let before_ok = i == 0
            || bytes.get(i.wrapping_sub(1)).is_some_and(|b| {
                matches!(
                    b,
                    b' ' | b'\t' | b'/' | b'"' | b'\'' | b'`' | b';' | b'|' | b'&' | b'(' | b'\n'
                )
            });
        // Check that 'm' is at end or followed by non-alnum (space, quote, tab, etc.)
        let after_ok = bytes.get(i + 2).is_none_or(|b| !b.is_ascii_alphanumeric());
        before_ok && after_ok
    })
}

/// Returns `true` if `token` (already quote-stripped) is the `rm` command — either
/// bare (`rm`) or an absolute path ending in `/rm` (e.g., `/bin/rm`, `/usr/bin/rm`).
fn is_rm_token(token: &str) -> bool {
    token == "rm" || (token.starts_with('/') && token.ends_with("/rm"))
}

/// Checks whether a `SimpleCommand`'s argv contains `rm` at command position
/// (after env-var assignments and shell launchers).
fn argv_has_rm(cmd: &SimpleCommand) -> bool {
    let mut seen_launcher = false;
    for token in &cmd.argv {
        // Environment variable assignments (VAR=value) keep command position
        if token.contains('=') && !token.starts_with('=') {
            continue;
        }
        // Shell launchers are transparent prefixes
        if is_shell_launcher(token) {
            seen_launcher = true;
            continue;
        }
        // After a launcher, skip non-rm arguments (flags/positional args)
        if seen_launcher && !is_rm_token(token) {
            continue;
        }
        return is_rm_token(token);
    }
    false
}

/// Extracts file arguments from a single `SimpleCommand` whose argv contains `rm`.
/// Skips env-var assignments, launchers, the rm token itself, and flags.
/// Handles `--` end-of-options marker: all tokens after `--` are treated as file arguments.
fn extract_rm_args_from_argv(cmd: &SimpleCommand) -> Vec<String> {
    let mut args = Vec::new();
    let mut found_rm = false;
    let mut seen_launcher = false;
    let mut end_of_options = false;
    for token in &cmd.argv {
        if found_rm {
            if token == "--" && !end_of_options {
                end_of_options = true;
                continue;
            }
            if !end_of_options && token.starts_with('-') {
                continue; // skip flags like -f, -rf
            }
            if !token.is_empty() {
                args.push(token.clone());
            }
            continue;
        }
        if token.contains('=') && !token.starts_with('=') {
            continue;
        }
        if is_shell_launcher(token) {
            seen_launcher = true;
            continue;
        }
        if seen_launcher && !is_rm_token(token) {
            continue;
        }
        if is_rm_token(token) {
            found_rm = true;
        }
    }
    args
}

/// Checks if a path refers to a test file.
///
/// Normalizes path components (resolving `.` and `..`) before checking patterns,
/// so that relative traversals like `../tests/foo.rs` or `tests/../src/main.rs`
/// are handled correctly.
fn is_test_file(path: &str) -> bool {
    use std::path::Component;

    // Normalize path: resolve `.` and `..` without touching the filesystem.
    let mut normalized = Vec::new();
    for component in std::path::Path::new(path).components() {
        match component {
            Component::CurDir => {} // skip `.`
            Component::ParentDir => {
                // Pop the last Normal segment if any; ignore otherwise.
                if matches!(normalized.last(), Some(Component::Normal(_))) {
                    normalized.pop();
                }
            }
            other => normalized.push(other),
        }
    }

    // Rebuild a clean `/`-separated path string from the normalized components.
    let clean: String =
        normalized.iter().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");

    // Check if path contains a `tests` directory segment (not a substring like `mytests/`)
    if clean == "tests"
        || clean.starts_with("tests/")
        || clean.contains("/tests/")
        || clean.ends_with("/tests")
    {
        return true;
    }

    // Extract filename from path
    let filename = clean.rsplit('/').next().unwrap_or(&clean);

    // Check *_test.rs pattern
    if filename.ends_with("_test.rs") {
        return true;
    }

    // Check test_*.rs pattern
    if filename.starts_with("test_") && filename.ends_with(".rs") {
        return true;
    }

    // Check tests.rs module file (e.g., src/tests.rs, libs/domain/src/tests.rs)
    if filename == "tests.rs" {
        return true;
    }

    false
}

/// Resolves a `HookName` to the appropriate handler and dispatches.
///
/// This function is the single dispatch point (OCP: adding a new hook
/// only requires a new `HookHandler` impl and a match arm here).
///
/// # Errors
/// Returns `HookError` from the handler, or `HookError::Unsupported` for unknown hooks.
pub fn dispatch(
    _name: HookName,
    handler: &dyn HookHandler,
    ctx: &HookContext,
    input: &HookInput,
) -> Result<HookVerdict, HookError> {
    handler.handle(ctx, input)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_guard_handler_allows_safe_command() {
        let handler = GuardHookHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("git status".into()),
            file_path: None,
            content: None,
        };

        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_guard_handler_blocks_git_add() {
        let handler = GuardHookHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("git add .".into()),
            file_path: None,
            content: None,
        };

        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_guard_handler_returns_error_on_missing_command() {
        let handler = GuardHookHandler;
        let ctx = HookContext { project_dir: None };
        let input =
            HookInput { tool_name: "Bash".into(), command: None, file_path: None, content: None };

        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing command")));
    }

    #[test]
    fn test_test_file_guard_blocks_rm_tests_dir() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_test_file_guard_blocks_rm_underscore_test_rs() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/user_test.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_test_file_guard_blocks_rm_test_underscore_rs() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/test_user.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_test_file_guard_allows_non_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/lib.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_test_file_guard_blocks_rm_with_shell_separator() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/test_user.rs; echo done".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "rm with trailing `;` must still block test files");
    }

    #[test]
    fn test_test_file_guard_blocks_rm_with_double_ampersand_suffix() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm tests/foo.rs&& echo ok".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "rm with trailing `&&` must still block test files");
    }

    #[test]
    fn test_test_file_guard_blocks_chained_rm_after_separator() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/main.rs && rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "chained rm after && must still detect test files");
    }

    #[test]
    fn test_test_file_guard_blocks_quoted_operand() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm \"src/test_user.rs\"".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "quoted test file path must still be detected");
    }

    #[test]
    fn test_test_file_guard_blocks_glued_separator_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("echo ok;rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "glued ;rm must still detect test files");
    }

    #[test]
    fn test_test_file_guard_blocks_partial_quoting() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/\"test_user.rs\"".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "partially quoted test file path must still be detected");
    }

    #[test]
    fn test_test_file_guard_blocks_rm_with_background_operator() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm tests/foo.rs& echo ok".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "rm with background & must still block test files");
    }

    #[test]
    fn test_test_file_guard_allows_echo_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("echo rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked(), "echo rm should not be treated as an rm command");
    }

    #[test]
    fn test_test_file_guard_fail_closed_on_malformed_input() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        // Bash tool with missing command should error (fail-closed at CLI = exit 2)
        let input =
            HookInput { tool_name: "Bash".into(), command: None, file_path: None, content: None };
        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(_))));
    }

    #[test]
    fn test_test_file_guard_blocks_env_var_prefix_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("FOO=1 rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "env var prefix before rm must be detected");
    }

    #[test]
    fn test_test_file_guard_blocks_newline_separated_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("echo ok\nrm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "newline-separated rm must be detected");
    }

    #[test]
    fn test_test_file_guard_blocks_absolute_path_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("/bin/rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "/bin/rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_usr_bin_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("/usr/bin/rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "/usr/bin/rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_env_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("env rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "env rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_allows_env_non_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("env ls tests/".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked(), "env ls should not be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_time_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("time rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "time rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_exec_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("exec rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "exec rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_command_p_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("command -p rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "command -p rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_command_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("command rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "command rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_timeout_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("timeout 5 rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "timeout 5 rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_nice_n_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("nice -n 10 rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "nice -n 10 rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_stdbuf_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("stdbuf -oL rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "stdbuf -oL rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_sudo_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("sudo rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "sudo rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_doas_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("doas rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "doas rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_absolute_path_sudo_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("/usr/bin/sudo rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "/usr/bin/sudo rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_absolute_path_time_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("/usr/bin/time rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "/usr/bin/time rm must be detected as rm command");
    }

    #[test]
    fn test_test_file_guard_blocks_rm_with_redirect() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/test_user.rs>/dev/null".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            verdict.is_blocked(),
            "rm with redirect attached to filename must still be blocked"
        );
    }

    #[test]
    fn test_test_file_guard_blocks_rm_with_spaced_redirect() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm tests/foo.rs > /dev/null 2>&1".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "rm with spaced redirect must still be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_quoted_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("\"rm\" tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "quoted \"rm\" must still be detected");
    }

    #[test]
    fn test_test_file_guard_blocks_quoted_bin_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("\"/bin/rm\" tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "quoted \"/bin/rm\" must still be detected");
    }

    #[test]
    fn test_test_file_guard_blocks_leading_redirect_before_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some(">/tmp/out rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "leading redirect before rm must still be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_fd_redirect_before_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("2>/dev/null rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "fd redirect before rm must still be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_fd_dup_redirect_before_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("2>&1 rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            verdict.is_blocked(),
            "fd duplication redirect (2>&1) before rm must still be blocked"
        );
    }

    #[test]
    fn test_test_file_guard_blocks_read_write_redirect_before_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("<>/tmp/out rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "read-write redirect (<>) before rm must still be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_clobber_redirect_before_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some(">|/tmp/out rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "clobber redirect (>|) before rm must still be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_herestring_before_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        // Here-string before a chained rm
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("cat <<<word; rm tests/foo.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "here-string (<<<) before chained rm must still be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_quoted_path_with_space() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm \"src/test_user copy.rs\"".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            verdict.is_blocked(),
            "quoted path with space containing test_ pattern must be blocked"
        );
    }

    #[test]
    fn test_test_file_guard_blocks_rm_double_dash_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm -- -tests/foo_test.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            verdict.is_blocked(),
            "rm -- with dash-prefixed test file after end-of-options must be blocked"
        );
    }

    #[test]
    fn test_test_file_guard_blocks_rm_rf_double_dash_test_dir() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm -rf -- tests/".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "rm -rf -- tests/ must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_bash_c_rm_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("bash -c 'rm tests/foo.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "bash -c 'rm tests/foo.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_sh_c_rm_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("sh -c 'rm -rf tests/'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "sh -c 'rm -rf tests/' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_sudo_bash_c_rm_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("sudo bash -c 'rm tests/foo_test.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "sudo bash -c 'rm tests/foo_test.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_allows_bash_c_without_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("bash -c 'echo hello'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked(), "bash -c 'echo hello' must be allowed");
    }

    #[test]
    fn test_test_file_guard_blocks_env_i_bash_c_rm_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("env -i bash -c 'rm tests/foo.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "env -i bash -c 'rm tests/foo.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_timeout_bash_c_rm_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("timeout 5 bash -c 'rm tests/foo.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "timeout 5 bash -c 'rm tests/foo.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_nested_bash_c_sh_c_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some(r#"bash -c 'sh -c "rm tests/foo.rs"'"#.into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "nested bash -c 'sh -c \"rm ...\"' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_sudo_u_root_bash_c_rm() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("sudo -u root bash -c 'rm tests/foo_test.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "sudo -u root bash -c 'rm ...' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_bash_lc_rm_test_file() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("bash -lc 'rm tests/foo.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "bash -lc 'rm tests/foo.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_sh_ec_rm_test_file() {
        // -ec: -e flag (exit on error), -c takes next arg as command
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("sh -ec 'rm tests/foo.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "sh -ec 'rm tests/foo.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_sh_ce_rm_test_file() {
        // -ce: -c + -e flags, next arg 'rm tests/foo.rs' is the command
        // Per POSIX/Bash, `-ce` is equivalent to `-c -e`.
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("sh -ce 'rm tests/foo.rs'".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "sh -ce 'rm tests/foo.rs' must be blocked");
    }

    #[test]
    fn test_test_file_guard_blocks_deeply_nested_rm_at_depth_limit() {
        // 4 levels of nesting — depth limit is 3, so the innermost shell re-entry
        // must be blocked via fail-closed (raw_mentions_rm) at depth 3
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some(r#"bash -c 'sh -c "bash -c \"sh -c \\\"rm tests/foo.rs\\\"\"" '"#.into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            verdict.is_blocked(),
            "deeply nested rm beyond depth limit must be blocked (fail-closed)"
        );
    }

    // === Write tool: test file deletion guard ===

    #[test]
    fn test_test_file_guard_write_with_empty_content_to_test_file_is_blocked() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("tests/foo_test.rs")),
            content: Some("".into()),
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "Write with empty content to test file must be blocked");
    }

    #[test]
    fn test_test_file_guard_write_with_content_to_test_file_is_allowed() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("tests/foo_test.rs")),
            content: Some("#[test]\nfn test_something() {}".into()),
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked(), "Write with content to test file must be allowed");
    }

    #[test]
    fn test_test_file_guard_write_with_empty_content_to_non_test_file_is_allowed() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("src/lib.rs")),
            content: Some("".into()),
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked(), "Write with empty content to non-test file must be allowed");
    }

    #[test]
    fn test_test_file_guard_edit_tool_is_always_allowed() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Edit".into(),
            command: None,
            file_path: Some(PathBuf::from("tests/foo_test.rs")),
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            !verdict.is_blocked(),
            "Edit tool must always be allowed (edits are not deletions)"
        );
    }

    #[test]
    fn test_test_file_guard_write_with_missing_file_path_is_blocked() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: None,
            content: Some("content".into()),
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "Write with missing file_path must be blocked (fail-closed)");
    }

    #[test]
    fn test_test_file_guard_write_with_none_content_to_test_file_is_blocked() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("tests/foo_test.rs")),
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(
            verdict.is_blocked(),
            "Write with None content to test file must be blocked (fail-closed)"
        );
    }

    #[test]
    fn test_test_file_guard_write_with_none_content_to_non_test_file_is_allowed() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("src/lib.rs")),
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked(), "Write with None content to non-test file must be allowed");
    }

    #[test]
    fn raw_mentions_rm_catches_quoted_rm() {
        assert!(raw_mentions_rm(r#""rm" tests/foo.rs"#));
        assert!(raw_mentions_rm("/bin/rm tests/foo.rs"));
        assert!(raw_mentions_rm("rm tests/foo.rs"));
    }

    #[test]
    fn test_test_file_guard_blocks_rm_tests_rs_module() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("rm src/tests.rs".into()),
            file_path: None,
            content: None,
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "rm src/tests.rs must be blocked");
    }

    #[test]
    fn test_test_file_guard_write_empty_to_tests_rs_is_blocked() {
        let handler = TestFileDeletionGuardHandler;
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("src/tests.rs")),
            content: Some("".into()),
        };
        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked(), "Write empty to src/tests.rs must be blocked");
    }

    // --- is_test_file path normalization tests (GAP-05) ---

    #[test]
    fn test_is_test_file_dot_slash_prefix() {
        assert!(is_test_file("./tests/foo.rs"));
    }

    #[test]
    fn test_is_test_file_parent_traversal() {
        assert!(is_test_file("../tests/foo.rs"));
    }

    #[test]
    fn test_is_test_file_multi_level_traversal() {
        assert!(is_test_file("foo/../../tests/bar.rs"));
    }

    #[test]
    fn test_is_test_file_traversal_away_from_tests_is_not_test() {
        assert!(!is_test_file("tests/../src/main.rs"));
    }

    #[test]
    fn test_is_test_file_dot_slash_test_underscore_rs() {
        assert!(is_test_file("./src/test_user.rs"));
    }

    #[test]
    fn test_raw_mentions_rm_allows_non_rm_words() {
        assert!(!raw_mentions_rm("format something"));
        assert!(!raw_mentions_rm("firmware update"));
        assert!(!raw_mentions_rm("echo normal"));
    }
}
