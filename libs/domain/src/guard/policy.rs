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

const SOTP_GUARDED_TOKEN_MESSAGE: &str = "[Git Policy] The guarded-git token is present in the Bash command string. \
     The token must not be passed inline — it is injected only by the sotp binary via its git_cli layer.";

const BIN_SOTP_OVERWRITE_MESSAGE: &str = "[Build Policy] Direct copy to `bin/sotp` is blocked. \
     Use `cargo make build-sotp` which includes runtime verification to prevent glibc mismatch.";

/// The exact token string scanned in argv to block inline token injection (D3/IN-03).
/// This must be an exact-match scan so that normal words containing substrings are not affected.
const GUARDED_GIT_TOKEN: &str = "SOTP_GUARDED_GIT";

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
///
/// Blanket blocks from ADR 0080 (D1/D2/D4) and CON-07 (output redirect, tee, sed -i)
/// have been retired per ADR 2026-06-10-1630-git-hooks-process-level-enforcement D4.
/// Enforcement of git write operations is now handled at the process level via
/// git hooks (reference-transaction / pre-push). The retained checks are:
///   - D3: SOTP_GUARDED_GIT exact-match scan over quote-stripped argv tokens
///   - D4 (maintained): direct git subcommand checks, launcher stripping, is_bin_sotp_overwrite
fn check_simple_command(cmd: &SimpleCommand) -> GuardVerdict {
    let argv = &cmd.argv;
    if argv.is_empty() {
        return GuardVerdict::allow();
    }

    // D3/IN-03 (b): argv-token exact-match scan for the guarded-git token.
    // Blocks attempts to inject the token via quote-splitting (e.g., SOTP_GUARDED_GI"T"=1).
    // The raw-string scan (stage a) is performed by GuardHookHandler (usecase layer) before
    // calling this function, since SimpleCommand does not carry the original raw string.
    if argv_contains_guarded_token(argv) {
        return GuardVerdict::block(SOTP_GUARDED_TOKEN_MESSAGE);
    }

    // Skip VAR=val assignments and command launchers to find the effective command
    let effective_start = skip_var_assignments(argv, 0);
    let effective_start = skip_command_launchers(argv, effective_start);

    if effective_start >= argv.len() {
        return GuardVerdict::allow();
    }

    let effective_cmd = basename(&argv[effective_start]).to_lowercase();

    // Direct git command — check specific subcommands
    if effective_cmd == "git" {
        return check_git_command(argv, effective_start);
    }

    // Block `cp` (or `mv`) targeting `bin/sotp` — must use `cargo make build-sotp`.
    if is_bin_sotp_overwrite(argv, effective_start) {
        return GuardVerdict::block(BIN_SOTP_OVERWRITE_MESSAGE);
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

/// Checks whether any quote-stripped argv token contains the guarded-git token
/// (D3/IN-03 stage b).
///
/// Blocks both the bare token (`SOTP_GUARDED_GIT`) and the assignment form
/// (`SOTP_GUARDED_GIT=...`), so that quote-splitting bypasses such as
/// `SOTP_GUARDED_GI"T"=1` — which the shell parser normalizes to `SOTP_GUARDED_GIT=1`
/// — are also caught before `skip_var_assignments` has a chance to silently skip them.
///
/// Exact-match semantics on the token (or the variable-name portion of an assignment)
/// prevent false positives on unrelated tokens whose names merely contain substrings
/// of the guarded token.
///
/// The raw-command-string scan (stage a) is performed upstream in `GuardHookHandler`
/// before `check_commands` is called, because `SimpleCommand` does not retain the
/// original raw string.
fn argv_contains_guarded_token(argv: &[String]) -> bool {
    argv.iter().any(|token| {
        // Bare token: exact match.
        if token == GUARDED_GIT_TOKEN {
            return true;
        }
        // Assignment form: the variable name (left of `=`) is exactly the guarded token.
        // This catches `SOTP_GUARDED_GIT=1` produced by quote-splitting of
        // `SOTP_GUARDED_GI"T"=1`.
        if let Some(rest) = token.strip_prefix(GUARDED_GIT_TOKEN) {
            if rest.starts_with('=') {
                return true;
            }
        }
        false
    })
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
                let mut output_redirect_texts = Vec::new();
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
                            output_redirect_texts.push((*target).to_string());
                            skip_next = true;
                        }
                    } else if tok.contains('>') && !tok.starts_with('-') && !tok.contains(">&") {
                        // e.g., "2>" — fd redirect (but NOT "2>&1" which is fd dup)
                        has_output_redirect = true;
                        if let Some(target) = tokens.get(i + 1) {
                            redirect_texts.push((*target).to_string());
                            output_redirect_texts.push((*target).to_string());
                            skip_next = true;
                        }
                    } else {
                        argv.push(tok.to_string());
                    }
                }
                SimpleCommand { argv, redirect_texts, output_redirect_texts, has_output_redirect }
            })
            .filter(|cmd| !cmd.argv.is_empty() || cmd.has_output_redirect)
            .collect();
        check_commands(&commands)
    }

    // -- AC-05: Blocked git subcommands (maintained precise checks) --

    #[rstest]
    #[case::git_add("git add .", "git add")]
    #[case::git_push("git push", "git push")]
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
    fn test_blocked_git_subcommands_with_direct_invocation_contain_reason(
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

    // AC-05: launcher-wrapped direct git subcommands are still blocked
    // Note: `&&`-chained commands (e.g. `cargo test && git add .`) are NOT tested here
    // because the test helper only splits on `;` and `|`. The ConchShellParser (infrastructure)
    // handles `&&` and produces two SimpleCommands; the policy-layer check correctly blocks
    // the `git add` command when it arrives as a separate SimpleCommand.
    #[rstest]
    #[case::git_push_plain("git push")]
    #[case::git_with_global_options_add("git -C /tmp add .")]
    #[case::semicolon_chained_git_commit("echo hi; git commit -m msg")]
    #[case::piped_git_push("echo y | git push")]
    #[case::timeout_git_commit("timeout 30 git commit -m msg")]
    #[case::nice_git_add("nice -n 10 git add .")]
    #[case::git_checkout_upper_b("git checkout -B new-branch")]
    #[case::git_checkout_orphan("git checkout --orphan new-branch")]
    #[case::git_exe_add("git.exe add .")]
    #[case::absolute_path_git_exe("/usr/bin/git.exe add .")]
    #[case::git_upper_exe("git.EXE add .")]
    #[case::timeout_signal_eq_kill_git_add("timeout --signal=KILL 30 git add .")]
    #[case::taskset_c_git_add("taskset -c 0 git add .")]
    #[case::taskset_plain_mask_git_add("taskset ff git add .")]
    #[case::taskset_cpu_list_eq_git_add("taskset --cpu-list=0 git add .")]
    #[case::taskset_cpu_list_git_add("taskset --cpu-list 0 git add .")]
    #[case::ionice_c_git_add("ionice -c 3 git add .")]
    #[case::command_p_git_add("command -p git add .")]
    #[case::exec_c_git_add("exec -c git add .")]
    #[case::exec_a_name_git_add("exec -a myname git add .")]
    fn test_blocked_direct_git_operations_with_launchers(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
    }

    // AC-05: absolute path git is still blocked
    #[test]
    fn test_absolute_path_git_is_blocked() {
        let v = check("/usr/bin/git add .");
        assert!(v.is_blocked());
    }

    // AC-05: git checkout branch-create variants are blocked; reason must mention "switch" or "checkout"
    #[rstest]
    #[case::checkout_b("git checkout -b new-branch")]
    #[case::checkout_upper_b("git checkout -B new-branch")]
    #[case::checkout_orphan("git checkout --orphan new-branch")]
    fn test_blocked_git_checkout_branch_create(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(v.is_blocked(), "expected blocked: {cmd}");
        assert!(
            v.reason.contains("switch") || v.reason.contains("checkout"),
            "reason {:?} should mention 'switch' or 'checkout' for cmd {:?}",
            v.reason,
            cmd
        );
    }

    // -- AC-03: SOTP_GUARDED_GIT argv token exact-match scan (D3/IN-03 stage b) --

    // AC-03/D3: exact-match argv-token scan — any argv position containing the bare
    // SOTP_GUARDED_GIT token must be blocked, with a reason mentioning "guarded-git token".
    #[rstest]
    #[case::bare_solo_token(vec!["SOTP_GUARDED_GIT".to_string()])]
    #[case::bare_token_as_first_arg(vec!["SOTP_GUARDED_GIT".to_string(), "git".to_string(), "commit".to_string()])]
    fn test_argv_containing_bare_guarded_token_is_blocked(#[case] argv: Vec<String>) {
        // Exact-match: only the bare token "SOTP_GUARDED_GIT" blocks.
        // A value-suffix variant (e.g., "SOTP_GUARDED_GIT=1") is tested separately below.
        let cmd = SimpleCommand {
            argv,
            redirect_texts: vec![],
            output_redirect_texts: vec![],
            has_output_redirect: false,
        };
        let v = check_commands(&[cmd]);
        assert!(v.is_blocked(), "bare SOTP_GUARDED_GIT token in argv must be blocked");
        assert!(v.reason.contains("guarded-git token"), "reason should mention guarded-git token");
    }

    #[rstest]
    #[case::assignment_with_value(vec!["SOTP_GUARDED_GIT=1".to_string(), "ls".to_string()])]
    #[case::assignment_empty_value(vec!["SOTP_GUARDED_GIT=".to_string()])]
    fn test_argv_containing_guarded_token_as_assignment_var_is_blocked(#[case] argv: Vec<String>) {
        // Regression for the quote-split bypass: SOTP_GUARDED_GI"T"=1 is parsed by the shell
        // into the token SOTP_GUARDED_GIT=1 — which must be blocked before skip_var_assignments
        // has a chance to silently discard it.
        let cmd = SimpleCommand {
            argv,
            redirect_texts: vec![],
            output_redirect_texts: vec![],
            has_output_redirect: false,
        };
        let v = check_commands(&[cmd]);
        assert!(v.is_blocked(), "SOTP_GUARDED_GIT=... assignment token in argv must be blocked");
        assert!(v.reason.contains("guarded-git token"), "reason should mention guarded-git token");
    }

    #[test]
    fn test_argv_containing_partial_guarded_token_is_allowed() {
        // Substring match must NOT block — exact match only
        let cmd = SimpleCommand {
            argv: vec!["SOTP_GUARDED".to_string(), "ls".to_string()],
            redirect_texts: vec![],
            output_redirect_texts: vec![],
            has_output_redirect: false,
        };
        let v = check_commands(&[cmd]);
        assert!(!v.is_blocked(), "partial match of SOTP_GUARDED_GIT token must be allowed");
    }

    // -- AC-04: Retired blanket blocks are now allowed --

    #[rstest]
    #[case::output_redirect("echo hello > /tmp/file.txt")]
    #[case::tee_pipeline("ls | tee output.txt")]
    #[case::sed_without_inplace("sed 's/a/b/' file.txt")]
    #[case::env_cargo_test("env cargo test")]
    #[case::command_substitution_pwd("echo $(pwd)")]
    #[case::dollar_home("echo $HOME")]
    #[case::redirect_append("echo hi >> /tmp/file.txt")]
    #[case::redirect_clobber("echo hi >| /tmp/file.txt")]
    #[case::tee_standalone("tee output.txt")]
    #[case::sed_inplace("sed -i 's/a/b/' file.txt")]
    #[case::env_ls("env ls -la")]
    fn test_retired_blanket_blocks_are_now_allowed(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(!v.is_blocked(), "expected allowed after D4 blanket block removal: {cmd}");
    }

    // -- AC-04 more: bash/python with git reference are now allowed (blanket gone) --

    #[rstest]
    #[case::bash_c_git_push_allowed("bash -c 'git push origin main'")]
    #[case::python_c_no_git_reference(r#"python3 -c "print('hello world')""#)]
    #[case::python_c_with_git_reference(
        r#"python3 -c "import subprocess; subprocess.run(['git', 'add', '.'])""#
    )]
    fn test_non_direct_git_invocations_are_allowed_after_d4(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(!v.is_blocked(), "expected allowed after D4: {cmd}");
    }

    // -- Allowed commands --

    #[rstest]
    #[case::git_status("git status")]
    #[case::git_diff("git diff")]
    #[case::git_log_oneline("git log --oneline")]
    #[case::git_branch_create("git branch feature-x")]
    #[case::cargo_make_test("cargo make test")]
    #[case::empty_command("")]
    #[case::git_checkout_file_restore("git checkout -- file.txt")]
    #[case::git_branch_double_dash_dev("git branch -- -dev")]
    #[case::taskset_git_status("taskset -c 0 git status")]
    #[case::taskset_plain_mask_git_status("taskset ff git status")]
    #[case::taskset_cpu_list_eq_git_status("taskset --cpu-list=0 git status")]
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

    // -- Allowed: redirects and file-write operations (D4 — CON-07 blanket blocks retired) --

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
    #[case::redirect_only("> /tmp/file.txt")]
    #[case::readwrite_redirect("cmd <> file.txt")]
    #[case::input_redirect("sort < input.txt")]
    #[case::fd_dup_stderr_to_stdout("ls 2>&1")]
    #[case::fd_dup_stdout_to_stderr("echo err 1>&2")]
    #[case::sed_without_inplace("sed 's/a/b/' file.txt")]
    #[case::sed_f_script("sed -finit.sed file.txt")]
    #[case::sed_e_with_i_as_expr("sed -e -i input.txt")]
    #[case::sed_f_with_inplace_as_script("sed -f --in-place input.txt")]
    fn test_allowed_redirect_and_file_write_operations_after_d4(#[case] cmd: &str) {
        let v = check(cmd);
        assert!(!v.is_blocked(), "expected allowed after D4: {cmd}");
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
}
