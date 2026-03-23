// Policy module uses bounded array indexing for argv traversal where
// preceding length/bounds checks guarantee in-bounds access.
#![allow(clippy::indexing_slicing)]

//! Guard policy for shell command checking.
//!
//! Determines whether a shell command should be allowed or blocked
//! based on git operation detection rules.

use super::types::SimpleCommand;
use super::verdict::{GuardVerdict, ParseError};

/// Command launchers that prefix the real command.
const COMMAND_LAUNCHERS: &[&str] = &[
    "nohup", "nice", "timeout", "stdbuf", "setsid", "chronic", "ionice", "chrt", "taskset",
    "command", "time", "exec", "sudo", "doas",
];

/// Launchers that consume a mandatory positional argument before the command.
const LAUNCHER_POSITIONAL_ARGS: &[(&str, usize)] = &[("timeout", 1), ("chrt", 1), ("taskset", 1)];

/// Launcher options that consume the next token as their argument.
///
/// Note: `-p` and `-c` are intentionally excluded because they have different
/// meanings for different launchers:
/// - `command -p`: no-arg flag (use default PATH)
/// - `command -v`, `command -V`: no-arg flags (describe command)
/// - `exec -c`: no-arg flag (pass argv[0] as command name)
/// - `exec -l`: no-arg flag
/// - `exec -a name`: takes argument (handled via EXEC_OPTIONS_WITH_ARG)
const LAUNCHER_OPTIONS_WITH_ARG: &[&str] = &[
    "-n",
    "--adjustment",
    "-k",
    "--kill-after",
    "-s",
    "--signal",
    "-i",
    "-o",
    "-e",
    "-f",
    "--format",
    "--output",
];

/// Launcher-specific options that consume the next token as their argument.
/// Maps (launcher_name, flag) pairs.
///
/// Note: `taskset -c` / `taskset -p` / `taskset --cpu-list` are intentionally
/// excluded. These are treated as no-arg flags, and the CPU mask/list is consumed
/// via `LAUNCHER_POSITIONAL_ARGS` (1 positional). This handles both `taskset MASK cmd`
/// and `taskset -c LIST cmd` uniformly.
const LAUNCHER_SPECIFIC_OPTIONS_WITH_ARG: &[(&str, &str)] = &[
    ("exec", "-a"),
    ("chrt", "-p"),
    ("chrt", "--pid"),
    ("ionice", "-c"),
    ("ionice", "--class"),
    ("sudo", "-u"),
    ("sudo", "--user"),
    ("sudo", "-g"),
    ("sudo", "--group"),
    ("sudo", "-C"),
    ("sudo", "--close-from"),
    ("sudo", "-p"),
    ("sudo", "--prompt"),
    ("sudo", "-D"),
    ("sudo", "--chdir"),
    ("sudo", "-r"),
    ("sudo", "--role"),
    ("sudo", "-t"),
    ("sudo", "--type"),
    ("sudo", "-h"),
    ("sudo", "--host"),
    ("doas", "-u"),
];

/// Git top-level options that consume the next token.
const GIT_OPTIONS_WITH_ARG: &[&str] = &[
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
    "--exec-path",
];

/// Policy messages.
const GIT_ADD_MESSAGE: &str = "[Git Policy] Direct `git add` is blocked. Use `/track:commit`, or write repo-relative paths \
     to `tmp/track-commit/add-paths.txt` and run `cargo make track-add-paths`.";

const GIT_COMMIT_MESSAGE: &str = "[Git Policy] Direct `git commit` is blocked. Use `/track:commit`, or write the message to \
     `tmp/track-commit/commit-message.txt` and run `cargo make track-commit-message`.";

const GIT_PUSH_MESSAGE: &str =
    "[Git Policy] Direct `git push` is blocked. Pushing must be done manually by the user.";

const GIT_BRANCH_DELETE_MESSAGE: &str = "[Git Policy] Direct `git branch -d/-D/--delete` is blocked. Branch deletion must be done \
     manually by the user.";

const GIT_SWITCH_MESSAGE: &str = "[Git Policy] Direct `git switch` / `git checkout -b` is blocked. \
     Use `cargo make track-branch-create '<track-id>'` or `cargo make track-branch-switch '<track-id>'`.";

const GIT_MERGE_MESSAGE: &str = "[Git Policy] Direct `git merge` is blocked. Merging must be done \
     manually by the user via PR workflow.";

const GIT_REBASE_MESSAGE: &str = "[Git Policy] Direct `git rebase` is blocked. Rebasing must be \
     done manually by the user.";

const GIT_CHERRY_PICK_MESSAGE: &str = "[Git Policy] Direct `git cherry-pick` is blocked. \
     Cherry-picking must be done manually by the user.";

const GIT_RESET_MESSAGE: &str = "[Git Policy] Direct `git reset` is blocked. Resetting must be \
     done manually by the user.";

const GIT_VARIABLE_BYPASS_MESSAGE: &str = "[Git Policy] Shell variable or command substitution is blocked. \
     Use literal values only. This pattern is not needed in the template workflow.";

const ENV_COMMAND_MESSAGE: &str = "[Git Policy] `env` command is blocked. \
     Use `VAR=val command` shell syntax instead. \
     The `env` command creates bypass vectors and is not needed in the template workflow.";

const NESTED_GIT_REFERENCE_MESSAGE: &str = "[Git Policy] Non-git command contains a git reference in its arguments. \
     Git operations must use literal `git` commands, not wrappers or nested invocations.";

const BIN_SOTP_OVERWRITE_MESSAGE: &str = "[Build Policy] Direct copy to `bin/sotp` is blocked. \
     Use `cargo make build-sotp` which includes runtime verification to prevent glibc mismatch.";

const OUTPUT_REDIRECT_MESSAGE: &str = "[File-Write Policy] Output redirect (>, >>, >|, <>) is blocked. \
     Use the Write/Edit tool for file modifications, or `cargo make` wrappers for controlled writes.";

const FILE_WRITE_COMMAND_MESSAGE: &str = "[File-Write Policy] File-write command is blocked. \
     Use the Write/Edit tool for file modifications, or `cargo make` wrappers for controlled writes.";

/// Commands that write to files (blocked in direct Bash).
const FILE_WRITE_COMMANDS: &[&str] = &["tee"];

/// Known shells for recursive `-c` payload inspection.
/// Used by the existing recursive parse in `split_shell` which already handles
/// `bash -c` / `sh -c`. Listed here for future generalization to other shells.
#[allow(dead_code)]
const KNOWN_SHELLS: &[&str] = &["bash", "sh", "dash", "zsh", "ash"];

/// Converts a parse error into a fail-closed block verdict.
///
/// Callers that parse shell commands via [`super::ShellParser`] should
/// use this helper to map parse failures to block verdicts.
pub fn block_on_parse_error(err: &ParseError) -> GuardVerdict {
    match err {
        ParseError::NestingDepthExceeded { .. } => {
            GuardVerdict::block("command nesting depth exceeded")
        }
        ParseError::UnmatchedQuote => GuardVerdict::block("unparseable command (unmatched quote)"),
    }
}

/// Checks pre-parsed simple commands against the guard policy.
///
/// Returns a `GuardVerdict` indicating whether the commands are allowed or blocked.
/// Parse errors should be handled by the caller using [`block_on_parse_error`].
pub fn check_commands(commands: &[SimpleCommand]) -> GuardVerdict {
    for cmd in commands {
        let verdict = check_simple_command(cmd);
        if verdict.is_blocked() {
            return verdict;
        }
    }

    GuardVerdict::allow()
}

/// Checks a single simple command against the policy.
fn check_simple_command(cmd: &SimpleCommand) -> GuardVerdict {
    // Check output redirects first — even redirect-only commands (empty argv)
    // like `> /tmp/file` must be blocked.
    if cmd.has_output_redirect {
        return GuardVerdict::block(OUTPUT_REDIRECT_MESSAGE);
    }

    let argv = &cmd.argv;
    if argv.is_empty() {
        return GuardVerdict::allow();
    }

    // Skip VAR=val assignments and command launchers to find the effective command
    let effective_start = skip_var_assignments(argv, 0);
    let effective_start = skip_command_launchers(argv, effective_start);

    if effective_start >= argv.len() {
        return GuardVerdict::allow();
    }

    let effective_cmd = basename(&argv[effective_start]).to_lowercase();

    // Block `env` command unconditionally.
    if effective_cmd == "env" {
        return GuardVerdict::block(ENV_COMMAND_MESSAGE);
    }

    // Block variable/command substitution ANYWHERE in argv or redirect texts.
    // The template workflow never needs $VAR, $(cmd), or `cmd` in any position.
    if command_contains_expansion(cmd) {
        return GuardVerdict::block(GIT_VARIABLE_BYPASS_MESSAGE);
    }

    // --- File-write guards (CON-07) ---
    // Note: output redirect check is at the top of check_simple_command
    // (before the empty-argv short-circuit) to catch redirect-only commands.

    // Block known file-write commands (tee).
    if FILE_WRITE_COMMANDS.contains(&effective_cmd.as_str()) {
        return GuardVerdict::block(FILE_WRITE_COMMAND_MESSAGE);
    }

    // Block `sed` with `-i` flag (in-place edit).
    if effective_cmd == "sed" && has_sed_inplace_flag(argv, effective_start) {
        return GuardVerdict::block(FILE_WRITE_COMMAND_MESSAGE);
    }

    // --- End file-write guards ---

    // Direct git command — check specific subcommands
    if effective_cmd == "git" {
        return check_git_command(argv, effective_start);
    }

    // Block `cp` (or `mv`) targeting `bin/sotp` — must use `cargo make build-sotp`.
    if is_bin_sotp_overwrite(argv, effective_start) {
        return GuardVerdict::block(BIN_SOTP_OVERWRITE_MESSAGE);
    }

    // Non-git command: block if any argv token or redirect text (including
    // heredoc bodies) contains "git" (case-insensitive). This catches ALL
    // nesting patterns (shell -c, python -c, find -exec, xargs, heredocs,
    // etc.) without per-tool option parsing.
    if command_contains_git(cmd) {
        return GuardVerdict::block(NESTED_GIT_REFERENCE_MESSAGE);
    }

    GuardVerdict::allow()
}

/// Checks a git command for protected subcommands.
fn check_git_command(argv: &[String], git_index: usize) -> GuardVerdict {
    let subcommand = extract_git_subcommand(argv, git_index);

    match subcommand.as_deref() {
        Some("add") => GuardVerdict::block(GIT_ADD_MESSAGE),
        Some("commit") => GuardVerdict::block(GIT_COMMIT_MESSAGE),
        Some("push") => GuardVerdict::block(GIT_PUSH_MESSAGE),
        Some("switch") => GuardVerdict::block(GIT_SWITCH_MESSAGE),
        Some("merge") => GuardVerdict::block(GIT_MERGE_MESSAGE),
        Some("rebase") => GuardVerdict::block(GIT_REBASE_MESSAGE),
        Some("cherry-pick") => GuardVerdict::block(GIT_CHERRY_PICK_MESSAGE),
        Some("reset") => GuardVerdict::block(GIT_RESET_MESSAGE),
        Some("checkout") => {
            if is_checkout_branch_create(argv, git_index) {
                GuardVerdict::block(GIT_SWITCH_MESSAGE)
            } else {
                GuardVerdict::allow()
            }
        }
        Some("branch") => {
            if is_branch_delete(argv, git_index) {
                GuardVerdict::block(GIT_BRANCH_DELETE_MESSAGE)
            } else {
                GuardVerdict::allow()
            }
        }
        _ => GuardVerdict::allow(),
    }
}

/// Extracts the git subcommand from argv, skipping git global options.
fn extract_git_subcommand(argv: &[String], git_index: usize) -> Option<String> {
    let mut i = git_index + 1;

    while i < argv.len() {
        let token = &argv[i];

        if token == "--" {
            i += 1;
            break;
        }

        if GIT_OPTIONS_WITH_ARG.contains(&token.as_str()) {
            i += 2; // skip option + its argument
            continue;
        }

        if token.starts_with('-') {
            i += 1;
            continue;
        }

        return Some(token.to_lowercase());
    }

    if i < argv.len() { Some(argv[i].to_lowercase()) } else { None }
}

/// Checks if a `git branch` command includes a delete flag.
fn is_branch_delete(argv: &[String], git_index: usize) -> bool {
    // Find the "branch" subcommand position first
    let mut i = git_index + 1;
    let mut found_branch = false;

    while i < argv.len() {
        let token = &argv[i];
        if token == "--" {
            i += 1;
            break;
        }
        if GIT_OPTIONS_WITH_ARG.contains(&token.as_str()) {
            i += 2;
            continue;
        }
        if token.starts_with('-') {
            i += 1;
            continue;
        }
        if token.to_lowercase() == "branch" {
            found_branch = true;
            i += 1;
            break;
        }
        break; // not "branch"
    }

    if !found_branch {
        return false;
    }

    // Look for delete flags after "branch", stopping at `--` (end of options)
    for token in &argv[i..] {
        // `--` marks end of options; tokens after are branch names, not flags
        if token == "--" {
            break;
        }
        if matches!(token.as_str(), "-d" | "-D" | "--delete") {
            return true;
        }
        // Detect bundled short flags containing 'd' or 'D', e.g. `-dr`, `-rD`
        if token.starts_with('-') && !token.starts_with("--") && token.len() > 2 {
            let flag_chars = &token[1..];
            if flag_chars.contains('d') || flag_chars.contains('D') {
                return true;
            }
        }
    }

    false
}

/// Checks if a `git checkout` command includes a branch-create flag (-b or -B).
fn is_checkout_branch_create(argv: &[String], git_index: usize) -> bool {
    let mut i = git_index + 1;
    let mut found_checkout = false;

    while i < argv.len() {
        let token = &argv[i];
        if token == "--" {
            i += 1;
            break;
        }
        if GIT_OPTIONS_WITH_ARG.contains(&token.as_str()) {
            i += 2;
            continue;
        }
        if token.starts_with('-') {
            i += 1;
            continue;
        }
        if token.to_lowercase() == "checkout" {
            found_checkout = true;
            i += 1;
            break;
        }
        break;
    }

    if !found_checkout {
        return false;
    }

    for token in &argv[i..] {
        if token == "--" {
            break;
        }
        if matches!(token.as_str(), "-b" | "-B" | "--orphan") {
            return true;
        }
        // Detect bundled short flags containing 'b' or 'B', e.g. `-fb`, `-tB`
        if token.starts_with('-') && !token.starts_with("--") && token.len() > 2 {
            let flag_chars = &token[1..];
            if flag_chars.contains('b') || flag_chars.contains('B') {
                return true;
            }
        }
    }

    false
}

/// Skips leading `VAR=val` assignments.
fn skip_var_assignments(argv: &[String], start: usize) -> usize {
    let mut i = start;
    while i < argv.len() && is_var_assignment(&argv[i]) {
        i += 1;
    }
    i
}

/// Skips known command launchers (nohup, nice, timeout, etc.) and their options.
fn skip_command_launchers(argv: &[String], start: usize) -> usize {
    let mut i = start;

    while i < argv.len() && COMMAND_LAUNCHERS.contains(&basename(&argv[i]).to_lowercase().as_str())
    {
        let launcher = basename(&argv[i]).to_lowercase();
        i += 1;

        // Skip launcher options, tracking --key=value forms that embed a positional value
        let opts_start = i;
        while i < argv.len() && argv[i].starts_with('-') {
            if LAUNCHER_OPTIONS_WITH_ARG.contains(&argv[i].as_str())
                || LAUNCHER_SPECIFIC_OPTIONS_WITH_ARG
                    .iter()
                    .any(|(name, flag)| *name == launcher && *flag == argv[i].as_str())
            {
                i = (i + 2).min(argv.len());
            } else {
                i += 1;
            }
        }

        // Skip mandatory positional args.
        // For taskset: --cpu-list=VALUE embeds the CPU list, so reduce positional
        // count to avoid over-skipping the actual command. Only applies to taskset
        // because its positional (mask/list) can be embedded in --cpu-list=VALUE.
        let positional = LAUNCHER_POSITIONAL_ARGS
            .iter()
            .find(|(name, _)| *name == launcher)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        let opts_end = i.min(argv.len());
        let embedded = if launcher == "taskset" {
            argv[opts_start..opts_end].iter().filter(|t| t.starts_with("--cpu-list=")).count()
        } else {
            0
        };
        i += positional.saturating_sub(embedded);

        // After launcher, skip VAR=val assignments if present
        i = skip_var_assignments(argv, i);
    }

    i
}

/// Checks if `sed` is invoked with the `-i` (in-place edit) flag.
///
/// Scans ALL tokens (not just until the first non-flag) because `-i` can
/// appear after `-e expr` or `-f script` which consume the next token.
fn has_sed_inplace_flag(argv: &[String], effective_start: usize) -> bool {
    let tokens = argv.get(effective_start + 1..).unwrap_or_default();
    let mut skip_next = false;
    for token in tokens {
        if skip_next {
            skip_next = false;
            continue;
        }
        // Stop at `--` end-of-options marker — anything after is a positional filename
        if token == "--" {
            break;
        }
        // `-e` and `-f` consume the next token (expression / script file).
        // Skip it to avoid treating the operand as a flag.
        if token == "-e" || token == "-f" {
            skip_next = true;
            continue;
        }
        if token == "-i"
            || token.starts_with("-i=")
            || token == "--in-place"
            || token.starts_with("--in-place=")
        {
            return true;
        }
        // Note: combined short flags like -ni/-Ei are NOT checked here to
        // avoid false positives (e.g., `sed -finit.sed` where `-f` consumes
        // the rest of the token as a filename). This is an accepted trade-off.
    }
    false
}

/// Checks if a command is `cp`, `mv`, or `install` targeting `bin/sotp` as
/// the **destination**.
///
/// Checks both the last non-flag argument (default destination) and explicit
/// target-directory options (`-t`/`--target-directory=`) that can also specify
/// the destination.
/// Commands that only *read from* `bin/sotp` (e.g., `cp bin/sotp /tmp/backup`)
/// are intentionally allowed.
fn is_bin_sotp_overwrite(argv: &[String], effective_start: usize) -> bool {
    if effective_start >= argv.len() {
        return false;
    }
    let cmd = basename(&argv[effective_start]).to_lowercase();
    if cmd != "cp" && cmd != "mv" && cmd != "install" {
        return false;
    }
    let args = &argv[effective_start + 1..];
    // Check explicit target-directory options (-t dir, --target-directory=dir, --target-directory dir)
    let mut has_target_dir_option = false;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-t" || args[i] == "--target-directory" {
            has_target_dir_option = true;
            if let Some(dir) = args.get(i + 1) {
                if is_bin_sotp_path(dir) {
                    return true;
                }
            }
            i += 2;
            continue;
        }
        if let Some(dir) = args[i].strip_prefix("--target-directory=") {
            has_target_dir_option = true;
            if is_bin_sotp_path(dir) {
                return true;
            }
        }
        // Handle clustered short options containing `t`.
        // GNU cp/mv/install allow clustering: `-at dir`, `-atbin/sotp`, `-ft dir`, etc.
        // We scan any arg starting with `-` (but not `--`) for the letter `t`.
        if args[i].starts_with('-') && !args[i].starts_with("--") {
            let flags = &args[i][1..]; // strip leading '-'
            if let Some(t_pos) = flags.find('t') {
                let after_t = &flags[t_pos + 1..];
                if !after_t.is_empty() {
                    // Attached form: -atbin/sotp or -tbin/sotp
                    has_target_dir_option = true;
                    if is_bin_sotp_path(after_t) {
                        return true;
                    }
                } else {
                    // Detached form: -at dir (next arg is the directory)
                    has_target_dir_option = true;
                    if let Some(dir) = args.get(i + 1) {
                        if is_bin_sotp_path(dir) {
                            return true;
                        }
                    }
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    // When -t/--target-directory is used, the destination is specified by that
    // option, not the last positional argument. Skip last-arg check to avoid
    // false positives (e.g., `cp -t /tmp bin/sotp` reads FROM bin/sotp).
    if has_target_dir_option {
        return false;
    }
    // The destination is the last non-flag argument.
    let last_arg = args.iter().rev().find(|arg| !arg.starts_with('-'));
    match last_arg {
        Some(arg) => is_bin_sotp_path(arg),
        None => false,
    }
}

/// Returns `true` if a path refers to the repo-relative `bin/sotp`.
///
/// Normalizes the path first to handle equivalent spellings like
/// `./bin/./sotp`, `bin//sotp`, `./bin/sotp`, `bin/tmp/../sotp`, etc.
/// Absolute paths (e.g., `/tmp/bin/sotp`) are NOT matched to avoid
/// false positives on unrelated destinations outside the repository.
fn is_bin_sotp_path(path: &str) -> bool {
    // Absolute paths cannot be repo-relative bin/sotp
    if path.starts_with('/') {
        return false;
    }
    let normalized = normalize_path(path);
    normalized == "bin/sotp"
}

/// Normalizes a Unix path by collapsing `/./`, `//`, `..`, and stripping leading `./`.
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            // Pop only if there is a real (non-`..`) parent to resolve against.
            // Leading `..` segments are preserved so that `../bin/sotp` stays
            // outside the repo and is NOT normalized to `bin/sotp`.
            if parts.last().is_some_and(|&p| p != "..") {
                parts.pop();
            } else {
                parts.push(seg);
            }
        } else {
            parts.push(seg);
        }
    }
    parts.join("/")
}

/// Checks if any token in a slice contains "git" (case-insensitive).
fn tokens_contain_git(tokens: &[String]) -> bool {
    tokens.iter().any(|token| token.to_lowercase().contains("git"))
}

/// Checks if any argv token or redirect text contains expansion markers ($, backtick).
fn command_contains_expansion(cmd: &SimpleCommand) -> bool {
    cmd.argv.iter().any(|t| has_expansion_marker(t))
        || cmd.redirect_texts.iter().any(|t| has_expansion_marker(t))
}

/// Checks if any argv token or redirect text contains "git" (case-insensitive).
///
/// Used to catch ALL nesting patterns (shell -c, python -c, find -exec, xargs,
/// heredocs, etc.) without per-tool option parsing. False positives on words
/// containing "git" (e.g. "digit", "legit") are acceptable — the template
/// workflow does not need to pass git references through wrapper commands.
fn command_contains_git(cmd: &SimpleCommand) -> bool {
    tokens_contain_git(&cmd.argv) || tokens_contain_git(&cmd.redirect_texts)
}

/// Returns the basename of a path-like token, stripping `.exe`/`.EXE`/`.Exe` suffix.
fn basename(token: &str) -> &str {
    let name = token
        .rsplit_once('/')
        .or_else(|| token.rsplit_once('\\'))
        .map(|(_, name)| name)
        .unwrap_or(token);
    // Case-insensitive .exe stripping (safe for multi-byte UTF-8)
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .or_else(|| name.strip_suffix(".Exe"))
        .or_else(|| {
            // General case-insensitive check for other mixed cases
            if name.len() > 4 && name.as_bytes()[name.len() - 4..].eq_ignore_ascii_case(b".exe") {
                Some(&name[..name.len() - 4])
            } else {
                None
            }
        })
        .unwrap_or(name)
}

/// Checks if a token looks like a `VAR=val` assignment.
fn is_var_assignment(token: &str) -> bool {
    if let Some(eq_pos) = token.find('=') {
        if eq_pos == 0 {
            return false;
        }
        let var_name = &token[..eq_pos];
        var_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && var_name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
    } else {
        false
    }
}

/// Checks if a token contains shell expansion markers ($, backtick).
fn has_expansion_marker(token: &str) -> bool {
    token.contains('$') || token.contains('`')
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Test-only helper: splits a command string into `SimpleCommand`s and
    /// evaluates them via `check_commands`. Handles semicolons, pipes,
    /// and simple redirects (`>`, `>>`, `>|`, `<>`, `2>`). Sufficient for
    /// policy unit tests. Full shell parsing (quoting, subshells, command
    /// substitutions) is tested at the infrastructure layer (ConchShellParser).
    fn check(input: &str) -> GuardVerdict {
        let commands: Vec<SimpleCommand> = input
            .split(';')
            .flat_map(|segment| segment.split('|'))
            .map(|part| {
                let tokens: Vec<&str> = part.split_whitespace().collect();
                let mut argv = Vec::new();
                let mut redirect_texts = Vec::new();
                let mut has_output_redirect = false;
                let mut skip_next = false;
                for (i, &tok) in tokens.iter().enumerate() {
                    if skip_next {
                        skip_next = false;
                        continue;
                    }
                    if tok == ">" || tok == ">>" || tok == ">|" || tok == "<>" {
                        has_output_redirect = true;
                        if let Some(target) = tokens.get(i + 1) {
                            redirect_texts.push((*target).to_string());
                            skip_next = true;
                        }
                    } else if tok.contains('>') && !tok.starts_with('-') && !tok.contains(">&") {
                        // e.g., "2>" — fd redirect (but NOT "2>&1" which is fd dup)
                        has_output_redirect = true;
                        if let Some(target) = tokens.get(i + 1) {
                            redirect_texts.push((*target).to_string());
                            skip_next = true;
                        }
                    } else {
                        argv.push(tok.to_string());
                    }
                }
                SimpleCommand { argv, redirect_texts, has_output_redirect }
            })
            .filter(|cmd| !cmd.argv.is_empty() || cmd.has_output_redirect)
            .collect();
        check_commands(&commands)
    }

    // -- Blocked git subcommands --

    #[rstest]
    #[case::git_add("git add .", "git add")]
    #[case::git_push("git push", "git push")]
    #[case::git_commit_via_env_nohup("env VAR=val nohup git commit -m msg", "env")]
    #[case::git_branch_delete_upper_d("git branch -D feature", "branch")]
    #[case::git_branch_d_lowercase("git branch -d feature", "branch")]
    #[case::git_branch_delete_long_flag("git branch --delete feature", "branch")]
    #[case::git_branch_dr_bundled("git branch -dr feature", "branch")]
    #[case::git_branch_r_upper_d_bundled("git branch -rD feature", "branch")]
    #[case::git_switch("git switch feature", "switch")]
    #[case::git_switch_create("git switch -c new-branch", "switch")]
    #[case::git_merge("git merge feature", "merge")]
    #[case::git_rebase("git rebase main", "rebase")]
    #[case::git_cherry_pick("git cherry-pick abc1234", "cherry-pick")]
    #[case::git_reset("git reset HEAD~1", "reset")]
    #[case::git_reset_hard("git reset --hard HEAD~1", "reset")]
    fn test_blocked_git_subcommands_contain_reason(
        #[case] cmd: &str,
        #[case] reason_fragment: &str,
    ) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
        assert!(
            v.reason.contains(reason_fragment),
            "reason {:?} should contain {:?} for cmd {:?}",
            v.reason,
            reason_fragment,
            cmd
        );
    }

    #[rstest]
    #[case::git_push_plain("git push")]
    #[case::git_with_global_options_add("git -C /tmp add .")]
    #[case::semicolon_chained_git_commit("echo hi; git commit -m msg")]
    #[case::and_chained_git_add("cargo test && git add .")]
    #[case::piped_git_push("echo y | git push")]
    #[case::timeout_git_commit("timeout 30 git commit -m msg")]
    #[case::nice_git_add("nice -n 10 git add .")]
    #[case::xargs_git_add("echo file.txt | xargs git add")]
    #[case::sh_c_git_add("sh -c 'git add .'")]
    #[case::zsh_c_git_commit("zsh -c 'git commit -m test'")]
    #[case::find_exec_git_add("find . -exec git add {} \\;")]
    #[case::find_exec_timeout_git_add("find . -exec timeout 30 git add {} \\;")]
    #[case::find_exec_nice_git_commit("find . -exec nice -n 10 git commit -m msg {} \\;")]
    #[case::xargs_nohup_git_push("echo file | xargs nohup git push")]
    #[case::xargs_timeout_git_add("echo file | xargs timeout 30 git add")]
    #[case::git_checkout_upper_b("git checkout -B new-branch")]
    #[case::git_checkout_orphan("git checkout --orphan new-branch")]
    #[case::git_exe_add("git.exe add .")]
    #[case::absolute_path_git_exe("/usr/bin/git.exe add .")]
    #[case::git_upper_exe("git.EXE add .")]
    #[case::bash_exe_c_git_push("bash.exe -c 'git push'")]
    #[case::bash_mixed_exe("bash.Exe -c 'git push'")]
    #[case::bash_rcfile_c_git_add("bash --rcfile /dev/null -c 'git add .'")]
    #[case::bash_init_file_c_git_push("bash --init-file /etc/profile -c 'git push'")]
    #[case::timeout_signal_eq_kill_git_add("timeout --signal=KILL 30 git add .")]
    #[case::taskset_c_git_add("taskset -c 0 git add .")]
    #[case::taskset_plain_mask_git_add("taskset ff git add .")]
    #[case::taskset_cpu_list_eq_git_add("taskset --cpu-list=0 git add .")]
    #[case::taskset_cpu_list_git_add("taskset --cpu-list 0 git add .")]
    #[case::ionice_c_git_add("ionice -c 3 git add .")]
    #[case::command_p_git_add("command -p git add .")]
    #[case::exec_c_git_add("exec -c git add .")]
    #[case::exec_a_name_git_add("exec -a myname git add .")]
    #[case::bash_heredoc_git_add("bash <<'SH'\ngit add .\nSH")]
    #[case::python_heredoc_git_subprocess(
        "python3 - <<'PY'\nimport subprocess; subprocess.run(['git','add','.'])\nPY"
    )]
    #[case::brace_group_heredoc_git_add("{ bash; } <<'SH'\ngit add .\nSH")]
    #[case::subshell_heredoc_git_add("( bash ) <<'SH'\ngit add .\nSH")]
    fn test_blocked_git_operations(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
    }

    // Special case: absolute_path_git has a label and different cmd value
    #[test]
    fn test_absolute_path_git_is_blocked() {
        let v = check("/usr/bin/git add .");
        assert!(v.is_blocked());
    }

    // -- Blocked: bash -c with git reference --

    #[test]
    fn test_bash_c_git_push_is_blocked() {
        let v = check("bash -c 'git push origin main'");
        assert!(v.is_blocked());
        assert!(v.reason.contains("git reference"));
    }

    // -- Blocked: variable/command substitution --

    #[rstest]
    #[case::dollar_cmd_add("$CMD add")]
    #[case::command_substitution_git("$(git_path) add .")]
    #[case::dollar_cmd_status("$CMD status")]
    #[case::which_ls_substitution("$(which ls) -la")]
    #[case::backtick_substitution("`which cat` file.txt")]
    #[case::redirect_to_cmd_sub_git_add("echo hi > $(git add .)")]
    #[case::redirect_to_cmd_sub_git_push("cat foo >> $(git push)")]
    #[case::find_exec_variable_bypass("find . -exec $CMD add \\;")]
    #[case::xargs_variable_bypass("echo x | xargs $CMD commit")]
    #[case::variable_bypass_branch_delete_uppercase("$CMD branch -D feature")]
    #[case::variable_bypass_branch_delete_lowercase("$CMD branch -d feature")]
    #[case::variable_bypass_branch_delete_long_flag("$CMD branch --delete feature")]
    #[case::variable_bypass_branch_dr("$CMD branch -dr feature")]
    #[case::variable_bypass_branch_double_dash("$CMD branch -- -dev")]
    #[case::subshell_redirect_cmd_sub_git_add("(echo hi) > $(git add .)")]
    #[case::for_iterator_cmd_sub_git_add("for x in $(git add .); do echo hi; done")]
    #[case::case_subject_cmd_sub_git_add("case $(git add .) in foo) echo hi;; esac")]
    fn test_blocked_variable_substitution(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
    }

    #[rstest]
    #[case::dollar_cmd_add("$CMD add")]
    #[case::command_substitution_git("$(git_path) add .")]
    #[case::dollar_cmd_status("$CMD status")]
    #[case::which_ls_substitution("$(which ls) -la")]
    #[case::backtick_substitution("`which cat` file.txt")]
    #[case::variable_bypass_branch_create("$CMD branch feature-x")]
    fn test_blocked_variable_substitution_contains_reason(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
        assert!(
            v.reason.contains("variable or command substitution"),
            "reason {:?} should contain 'variable or command substitution' for cmd {:?}",
            v.reason,
            cmd
        );
    }

    // -- Blocked: env command (unconditional) --

    #[rstest]
    #[case::env_git_add("env git add .")]
    #[case::env_s_git_add("env -S 'git add .'")]
    #[case::env_split_string_git_push("env --split-string 'git push'")]
    #[case::env_cargo_test("env cargo test")]
    #[case::env_with_flags("env -i FOO=bar ls")]
    #[case::timeout_env("timeout 30 env git status")]
    #[case::env_with_combined_flags("env -iS'git add .'")]
    #[case::env_with_escape("env -iSgit\\ push")]
    #[case::env_with_separate_arg("env -iS 'git commit -m msg'")]
    #[case::env_u_then_s_git_add("env -u FOO -S 'git add .'")]
    #[case::env_c_dir("env -C /tmp git status")]
    #[case::env_u_shell("env -uSHELL git status")]
    #[case::env_u_s("env -uS git status")]
    fn test_blocked_env_command_contains_reason(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
        assert!(
            v.reason.contains("env"),
            "reason {:?} should contain 'env' for cmd {:?}",
            v.reason,
            cmd
        );
    }

    // -- Blocked: python/shell with git reference --

    #[rstest]
    #[case::python3_c_git_subprocess(
        r#"python3 -c "import subprocess; subprocess.run(['git', 'add', '.'])""#
    )]
    #[case::python3_12_c_git_subprocess(
        r#"python3.12 -c "import subprocess; subprocess.run(['git', 'add', '.'])""#
    )]
    #[case::python_c_absolute_path_git(r#"python3 -c "subprocess.run(['/usr/bin/git', 'push'])""#)]
    #[case::python_c_git_exe_subprocess(r#"python3 -c "subprocess.run(['git.exe', 'add', '.'])""#)]
    #[case::xargs_python_c_git_push(
        r#"echo x | xargs python3 -c "import os; os.system('git push')""#
    )]
    #[case::find_exec_python_c_git_add(
        r#"find . -exec python3 -c "import subprocess; subprocess.run(['git', 'add', '.'])" \;"#
    )]
    #[case::python_c_git_branch_bundled_dr(
        r#"python3 -c "subprocess.run(['git','branch','-dr','feature'])""#
    )]
    #[case::python_c_git_branch_bundled_r_upper_d(
        r#"python3 -c "subprocess.run(['git','branch','-rD','feature'])""#
    )]
    #[case::python_c_git_branch_create(
        r#"python3 -c "subprocess.run(['git','branch','feature'])""#
    )]
    #[case::python_c_git_branch_delete(
        r#"python3 -c "subprocess.run(['git','branch','-D','feature'])""#
    )]
    #[case::python3_long_opt_c_git(
        r#"python3 --check-hash-based-pycs always -c "import subprocess; subprocess.run(['git','add','.'])""#
    )]
    fn test_blocked_shell_with_git_reference(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
    }

    // -- Blocked: git checkout -b (branch create) --

    #[rstest]
    #[case::checkout_b("git checkout -b new-branch")]
    #[case::checkout_upper_b("git checkout -B new-branch")]
    #[case::checkout_orphan("git checkout --orphan new-branch")]
    fn test_blocked_git_checkout_branch_create(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
    }

    #[test]
    fn test_git_checkout_b_reason_contains_switch_or_checkout() {
        let v = check("git checkout -b new-branch");
        assert!(v.is_blocked());
        assert!(v.reason.contains("switch") || v.reason.contains("checkout"));
    }

    #[test]
    fn test_git_checkout_orphan_reason_contains_switch_or_checkout() {
        let v = check("git checkout --orphan new-branch");
        assert!(v.is_blocked());
        assert!(v.reason.contains("switch") || v.reason.contains("checkout"));
    }

    // -- Allowed commands --

    #[rstest]
    #[case::git_status("git status")]
    #[case::git_diff("git diff")]
    #[case::git_log_oneline("git log --oneline")]
    #[case::git_branch_create("git branch feature-x")]
    #[case::cargo_make_test("cargo make test")]
    #[case::empty_command("")]
    // redirect_to_normal_file and redirect_to_file_without_git moved to blocked
    // tests — output redirects are now blocked by CON-07 file-write guard.
    #[case::git_checkout_file_restore("git checkout -- file.txt")]
    #[case::git_branch_double_dash_dev("git branch -- -dev")]
    #[case::taskset_git_status("taskset -c 0 git status")]
    #[case::taskset_plain_mask_git_status("taskset ff git status")]
    #[case::taskset_cpu_list_eq_git_status("taskset --cpu-list=0 git status")]
    #[case::python_c_no_git_reference(r#"python3 -c "print('hello world')""#)]
    #[case::bash_heredoc_without_git("bash <<'SH'\necho hello\nSH")]
    #[case::taskset_trailing_option_no_panic("taskset -o")]
    #[case::timeout_trailing_signal_no_panic("timeout -s")]
    #[case::multibyte_utf8_command_no_panic("€aab")]
    #[case::multibyte_utf8_with_exe_suffix_no_panic("日本語.exe add")]
    fn test_allowed_commands(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(!v.is_blocked(), "expected allowed: {cmd}");
    }

    // -- Blocked: cp/mv to bin/sotp --

    #[rstest]
    #[case::cp_bin_sotp("cp target/release/sotp bin/sotp")]
    #[case::cp_full_path("cp /tmp/sotp ./bin/sotp")]
    #[case::mv_bin_sotp("mv target/release/sotp bin/sotp")]
    #[case::install_bin_sotp("install target/release/sotp bin/sotp")]
    #[case::cp_with_flags("cp -f target/release/sotp bin/sotp")]
    #[case::cp_target_dir_option("cp -t bin/sotp target/release/sotp")]
    #[case::cp_target_directory_long("cp --target-directory=bin/sotp target/release/sotp")]
    #[case::install_target_dir("install -t bin/sotp target/release/sotp")]
    #[case::sudo_cp_bin_sotp("sudo cp target/release/sotp bin/sotp")]
    #[case::cp_target_directory_space("cp --target-directory bin/sotp target/release/sotp")]
    #[case::sudo_u_root_cp_bin_sotp("sudo -u root cp target/release/sotp bin/sotp")]
    #[case::cp_attached_t_option("cp -tbin/sotp target/release/sotp")]
    #[case::cp_clustered_at("cp -at bin/sotp target/release/sotp")]
    #[case::cp_clustered_ft("cp -ft bin/sotp target/release/sotp")]
    #[case::cp_clustered_at_attached("cp -atbin/sotp target/release/sotp")]
    #[case::cp_dot_slash_dot_path("cp target/release/sotp ./bin/./sotp")]
    #[case::cp_double_slash_path("cp target/release/sotp bin//sotp")]
    #[case::cp_dotdot_path("cp target/release/sotp bin/tmp/../sotp")]
    #[case::cp_dotdot_deep("cp target/release/sotp foo/../bin/sotp")]
    #[case::sudo_p_prompt_cp_bin_sotp("sudo -p Password: cp target/release/sotp bin/sotp")]
    fn test_blocked_bin_sotp_overwrite(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
        assert!(v.reason.contains("bin/sotp"), "reason should mention bin/sotp: {:?}", v.reason);
    }

    #[rstest]
    #[case::cp_other_file("cp target/release/sotp /tmp/sotp")]
    #[case::cp_unrelated("cp file.txt other.txt")]
    #[case::cargo_make_build_sotp("cargo make build-sotp")]
    #[case::cp_from_bin_sotp("cp bin/sotp /tmp/sotp-backup")]
    #[case::cp_from_full_path_bin_sotp("cp ./bin/sotp /tmp/backup")]
    #[case::cp_target_dir_read_from_sotp("cp -t /tmp bin/sotp")]
    #[case::sudo_u_root_cp_elsewhere("sudo -u root cp target/release/sotp /tmp/sotp")]
    #[case::cp_absolute_bin_sotp("/usr/bin/cp target/release/sotp /tmp/bin/sotp")]
    #[case::cp_dotdot_leading_bin_sotp("cp target/release/sotp ../bin/sotp")]
    #[case::sudo_p_prompt_cp_elsewhere("sudo -p Password: cp target/release/sotp /tmp/sotp")]
    fn test_allowed_bin_sotp_unrelated(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(!v.is_blocked(), "expected allowed: {cmd}");
    }

    // -- Blocked: file-write operations (CON-07) --

    #[rstest]
    #[case::redirect_stdout("echo hi > /tmp/file.txt")]
    #[case::redirect_append("echo hi >> /tmp/file.txt")]
    #[case::redirect_stderr("cmd 2> err.log")]
    #[case::redirect_clobber("cmd >| force.txt")]
    #[case::tee_standalone("tee output.txt")]
    #[case::tee_pipeline("ls | tee output.txt")]
    #[case::sed_inplace("sed -i 's/a/b/' file.txt")]
    #[case::sed_inplace_suffix("sed --in-place=.bak 's/a/b/' file.txt")]
    #[case::sed_e_then_inplace("sed -e 's/a/b/' -i file.txt")]
    #[case::sed_f_then_inplace("sed -f script.sed -i file.txt")]
    #[case::compound_redirect("{ echo hi; } > file.txt")]
    #[case::subshell_redirect("(echo hi) >> file.txt")]
    #[case::redirect_only("> /tmp/file.txt")]
    #[case::readwrite_redirect("cmd <> file.txt")]
    fn test_blocked_file_write_operations(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
    }

    #[rstest]
    #[case::input_redirect("sort < input.txt")]
    #[case::fd_dup_stderr_to_stdout("ls 2>&1")]
    #[case::fd_dup_stdout_to_stderr("echo err 1>&2")]
    #[case::sed_without_inplace("sed 's/a/b/' file.txt")]
    #[case::sed_f_script("sed -finit.sed file.txt")]
    #[case::sed_e_with_i_as_expr("sed -e -i input.txt")]
    #[case::sed_f_with_inplace_as_script("sed -f --in-place input.txt")]
    fn test_allowed_non_write_redirects(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(!v.is_blocked(), "expected allowed: {cmd}");
    }

    // -- Helper: basename --

    #[rstest]
    #[case::absolute_path("/usr/bin/git", "git")]
    #[case::plain("git", "git")]
    #[case::windows_path("C:\\Program Files\\git", "git")]
    #[case::exe_suffix("git.exe", "git")]
    #[case::exe_suffix_with_path("/usr/bin/git.exe", "git")]
    #[case::upper_exe_suffix("git.EXE", "git")]
    #[case::mixed_exe_suffix("git.Exe", "git")]
    fn test_basename(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(basename(input), expected);
    }

    // -- Helper: is_var_assignment --

    #[rstest]
    #[case::foo_bar("FOO=bar", true)]
    #[case::underscore_var("_VAR=val", true)]
    #[case::leading_eq("=bar", false)]
    #[case::plain_word("git", false)]
    #[case::flag("-c", false)]
    fn test_is_var_assignment(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(is_var_assignment(input), expected);
    }

    // -- Helper: has_expansion_marker --

    #[rstest]
    #[case::dollar_var("$HOME", true)]
    #[case::dollar_paren("$(cmd)", true)]
    #[case::backtick("`cmd`", true)]
    #[case::plain_word("git", false)]
    fn test_has_expansion_marker(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(has_expansion_marker(input), expected);
    }
}
