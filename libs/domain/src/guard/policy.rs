// Policy module uses bounded array indexing for argv traversal where
// preceding length/bounds checks guarantee in-bounds access.
#![allow(clippy::indexing_slicing)]

//! Guard policy for shell command checking.
//!
//! Determines whether a shell command should be allowed or blocked
//! based on git operation detection rules.

use super::parser::{self, SimpleCommand};
use super::verdict::{GuardVerdict, ParseError};

/// Command launchers that prefix the real command.
const COMMAND_LAUNCHERS: &[&str] = &[
    "nohup", "nice", "timeout", "stdbuf", "setsid", "chronic", "ionice", "chrt", "taskset",
    "command", "time", "exec",
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
const LAUNCHER_SPECIFIC_OPTIONS_WITH_ARG: &[(&str, &str)] =
    &[("exec", "-a"), ("chrt", "-p"), ("chrt", "--pid"), ("ionice", "-c"), ("ionice", "--class")];

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

/// Checks a shell command against the guard policy.
///
/// Returns a `GuardVerdict` indicating whether the command is allowed or blocked.
/// On parse failure, returns Block (fail-closed).
pub fn check(input: &str) -> GuardVerdict {
    // Parse the command into simple commands
    let commands = match parser::split_shell(input) {
        Ok(cmds) => cmds,
        Err(ParseError::NestingDepthExceeded { .. }) => {
            return GuardVerdict::block("command nesting depth exceeded");
        }
        Err(ParseError::UnmatchedQuote) => {
            return GuardVerdict::block("unparseable command (unmatched quote)");
        }
    };

    for cmd in &commands {
        let verdict = check_simple_command(cmd);
        if verdict.is_blocked() {
            return verdict;
        }
    }

    GuardVerdict::allow()
}

/// Checks a single simple command against the policy.
fn check_simple_command(cmd: &SimpleCommand) -> GuardVerdict {
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

    // Direct git command — check specific subcommands
    if effective_cmd == "git" {
        return check_git_command(argv, effective_start);
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

    // -- Acceptance criteria tests --

    #[test]
    fn test_git_add_is_blocked() {
        let v = check("git add .");
        assert!(v.is_blocked());
        assert!(v.reason.contains("git add"));
    }

    #[test]
    fn test_git_status_is_allowed() {
        let v = check("git status");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_env_nohup_git_commit_is_blocked() {
        // env is unconditionally blocked regardless of what follows
        let v = check("env VAR=val nohup git commit -m msg");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_bash_c_git_push_is_blocked() {
        let v = check("bash -c 'git push origin main'");
        assert!(v.is_blocked());
        assert!(v.reason.contains("git reference"));
    }

    #[test]
    fn test_variable_substitution_bypass_is_blocked() {
        let v = check("$CMD add");
        assert!(v.is_blocked());
        assert!(v.reason.contains("variable or command substitution"));
    }

    #[test]
    fn test_git_branch_delete_is_blocked() {
        let v = check("git branch -D feature");
        assert!(v.is_blocked());
        assert!(v.reason.contains("branch"));
    }

    #[test]
    fn test_cargo_make_test_is_allowed() {
        let v = check("cargo make test");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_find_exec_git_add_is_blocked() {
        let v = check("find . -exec git add {} \\;");
        assert!(v.is_blocked());
    }

    // -- Additional coverage tests --

    #[test]
    fn test_git_push_is_blocked() {
        let v = check("git push");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_git_diff_is_allowed() {
        let v = check("git diff");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_git_log_is_allowed() {
        let v = check("git log --oneline");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_git_with_global_options_add_is_blocked() {
        let v = check("git -C /tmp add .");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_semicolon_chained_git_commit() {
        let v = check("echo hi; git commit -m msg");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_and_chained_git_add() {
        let v = check("cargo test && git add .");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_piped_git_push() {
        let v = check("echo y | git push");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_timeout_git_commit() {
        let v = check("timeout 30 git commit -m msg");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_nice_git_add() {
        let v = check("nice -n 10 git add .");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_git_branch_create_is_allowed() {
        let v = check("git branch feature-x");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_git_branch_d_lowercase() {
        let v = check("git branch -d feature");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_git_branch_delete_long_flag() {
        let v = check("git branch --delete feature");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_xargs_git_add_is_blocked() {
        let v = check("echo file.txt | xargs git add");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_sh_c_git_add() {
        let v = check("sh -c 'git add .'");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_zsh_c_git_commit() {
        let v = check("zsh -c 'git commit -m test'");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_python_c_git_subprocess() {
        let v = check(r#"python3 -c "import subprocess; subprocess.run(['git', 'add', '.'])""#);
        assert!(v.is_blocked());
    }

    #[test]
    fn test_command_substitution_git() {
        let v = check("$(git_path) add .");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_empty_command_is_allowed() {
        let v = check("");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_absolute_path_git() {
        let v = check("/usr/bin/git add .");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_env_git_add() {
        let v = check("env git add .");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_var_assignment_git_commit() {
        let v = check("GIT_AUTHOR_NAME=test git commit -m msg");
        assert!(v.is_blocked());
    }

    // -- Review Fix: python versioned binary detection --

    #[test]
    fn test_python3_12_c_git_subprocess() {
        let v = check(r#"python3.12 -c "import subprocess; subprocess.run(['git', 'add', '.'])""#);
        assert!(v.is_blocked());
    }

    // -- Review Fix: absolute path git in Python code --

    #[test]
    fn test_python_c_absolute_path_git() {
        let v = check(r#"python3 -c "subprocess.run(['/usr/bin/git', 'push'])""#);
        assert!(v.is_blocked());
    }

    // -- Round 9 fixes: Python broad "git" match --

    #[test]
    fn test_python_c_git_exe_subprocess_is_blocked() {
        let v = check(r#"python3 -c "subprocess.run(['git.exe', 'add', '.'])""#);
        assert!(v.is_blocked(), "python git.exe subprocess should be blocked");
    }

    #[test]
    fn test_python_c_no_git_reference_is_allowed() {
        let v = check(r#"python3 -c "print('hello world')""#);
        assert!(!v.is_blocked(), "python without git should be allowed");
    }

    // -- env unconditional block --

    #[test]
    fn test_env_s_git_add() {
        let v = check("env -S 'git add .'");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_split_string_git_push() {
        let v = check("env --split-string 'git push'");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_cargo_test_is_blocked() {
        // env is unconditionally blocked regardless of payload
        let v = check("env cargo test");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_with_flags_is_blocked() {
        let v = check("env -i FOO=bar ls");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_timeout_env_is_blocked() {
        let v = check("timeout 30 env git status");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    // -- Review Fix Round 2: find -exec / xargs with launchers --

    #[test]
    fn test_find_exec_timeout_git_add() {
        let v = check("find . -exec timeout 30 git add {} \\;");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_find_exec_nice_git_commit() {
        let v = check("find . -exec nice -n 10 git commit -m msg {} \\;");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_xargs_nohup_git_push() {
        let v = check("echo file | xargs nohup git push");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_xargs_timeout_git_add() {
        let v = check("echo file | xargs timeout 30 git add");
        assert!(v.is_blocked());
    }

    // -- Finding 1: Redirect with command substitution --

    #[test]
    fn test_redirect_to_command_substitution_git_add() {
        // echo hi > $(git add .) should be blocked
        let v = check("echo hi > $(git add .)");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_redirect_to_command_substitution_git_push() {
        let v = check("cat foo >> $(git push)");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_redirect_to_normal_file_is_allowed() {
        let v = check("echo hi > /tmp/file.txt");
        assert!(!v.is_blocked());
    }

    // -- Finding 2: find -exec/xargs with python -c and variable bypass --

    #[test]
    fn test_find_exec_python_c_git_add() {
        let v = check(
            r#"find . -exec python3 -c "import subprocess; subprocess.run(['git', 'add', '.'])" \;"#,
        );
        assert!(v.is_blocked());
    }

    #[test]
    fn test_find_exec_variable_bypass() {
        // $CMD contains expansion marker — blocked regardless of position
        let v = check("find . -exec $CMD add \\;");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_xargs_python_c_git_push() {
        let v = check(r#"echo x | xargs python3 -c "import os; os.system('git push')""#);
        assert!(v.is_blocked());
    }

    #[test]
    fn test_xargs_variable_bypass() {
        // $CMD contains expansion marker — blocked regardless of position
        let v = check("echo x | xargs $CMD commit");
        assert!(v.is_blocked());
    }

    // -- Finding 3: Variable bypass with branch -d/-D --

    #[test]
    fn test_variable_bypass_branch_delete_uppercase() {
        let v = check("$CMD branch -D feature");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_variable_bypass_branch_delete_lowercase() {
        let v = check("$CMD branch -d feature");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_variable_bypass_branch_delete_long_flag() {
        let v = check("$CMD branch --delete feature");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_variable_bypass_branch_create_is_blocked() {
        // $CMD anything is blocked — variable expansion is not allowed
        let v = check("$CMD branch feature-x");
        assert!(v.is_blocked());
        assert!(v.reason.contains("variable or command substitution"));
    }

    // -- env with various flag combinations --

    #[test]
    fn test_env_with_combined_flags_is_blocked() {
        let v = check("env -iS'git add .'");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_with_escape_is_blocked() {
        let v = check("env -iSgit\\ push");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_with_separate_arg_is_blocked() {
        let v = check("env -iS 'git commit -m msg'");
        assert!(v.is_blocked());
        assert!(v.reason.contains("env"));
    }

    // -- taskset positional mask handling --

    #[test]
    fn test_taskset_c_git_add_is_blocked() {
        // taskset -c 0 git add . — -c is no-arg flag, 0 is positional (mask/list)
        let v = check("taskset -c 0 git add .");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_taskset_plain_mask_git_add_is_blocked() {
        // taskset ff git add . — ff is positional mask, git add is the command
        let v = check("taskset ff git add .");
        assert!(v.is_blocked(), "taskset ff git add . should be blocked");
    }

    #[test]
    fn test_taskset_git_status_is_allowed() {
        let v = check("taskset -c 0 git status");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_taskset_plain_mask_git_status_is_allowed() {
        let v = check("taskset ff git status");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_taskset_cpu_list_eq_git_add_is_blocked() {
        // --cpu-list=0 embeds the value; git add should still be detected
        let v = check("taskset --cpu-list=0 git add .");
        assert!(v.is_blocked(), "taskset --cpu-list=0 git add . should be blocked");
    }

    #[test]
    fn test_taskset_cpu_list_eq_git_status_is_allowed() {
        let v = check("taskset --cpu-list=0 git status");
        assert!(!v.is_blocked());
    }

    // -- Finding 1 (review round 2): For/Case iterator command substitutions --

    #[test]
    fn test_for_iterator_command_substitution_git_add_is_blocked() {
        let v = check("for x in $(git add .); do echo hi; done");
        assert!(v.is_blocked(), "for iterator with $(git add .) should be blocked");
    }

    #[test]
    fn test_case_subject_command_substitution_git_add_is_blocked() {
        let v = check("case $(git add .) in foo) echo hi;; esac");
        assert!(v.is_blocked(), "case subject with $(git add .) should be blocked");
    }

    // -- Finding 2 (review round 2): Incomplete launcher-specific arg table --

    #[test]
    fn test_ionice_c_git_add_is_blocked() {
        let v = check("ionice -c 3 git add .");
        assert!(v.is_blocked(), "ionice -c 3 git add . should be blocked");
    }

    #[test]
    fn test_taskset_cpu_list_git_add_is_blocked() {
        let v = check("taskset --cpu-list 0 git add .");
        assert!(v.is_blocked(), "taskset --cpu-list 0 git add . should be blocked");
    }

    // -- Finding 3 (review round 2): Python bundled branch-delete flags --

    #[test]
    fn test_python_c_git_branch_bundled_dr_is_blocked() {
        let v = check(r#"python3 -c "subprocess.run(['git','branch','-dr','feature'])""#);
        assert!(v.is_blocked(), "python git branch -dr should be blocked");
    }

    #[test]
    fn test_python_c_git_branch_bundled_r_upper_d_is_blocked() {
        let v = check(r#"python3 -c "subprocess.run(['git','branch','-rD','feature'])""#);
        assert!(v.is_blocked(), "python git branch -rD should be blocked");
    }

    // -- Finding 4 (review round 2): Branch-delete false positive on `git branch -- -dev` --

    #[test]
    fn test_git_branch_double_dash_dev_is_allowed() {
        let v = check("git branch -- -dev");
        assert!(!v.is_blocked(), "git branch -- -dev should be allowed (branch name, not flag)");
    }

    // -- Branch strategy: new blocked subcommands --

    #[test]
    fn test_git_switch_is_blocked() {
        let v = check("git switch feature");
        assert!(v.is_blocked());
        assert!(v.reason.contains("switch"));
    }

    #[test]
    fn test_git_switch_create_is_blocked() {
        let v = check("git switch -c new-branch");
        assert!(v.is_blocked());
        assert!(v.reason.contains("switch"));
    }

    #[test]
    fn test_git_checkout_b_is_blocked() {
        let v = check("git checkout -b new-branch");
        assert!(v.is_blocked());
        assert!(v.reason.contains("switch") || v.reason.contains("checkout"));
    }

    #[test]
    fn test_git_checkout_upper_b_is_blocked() {
        let v = check("git checkout -B new-branch");
        assert!(v.is_blocked());
    }

    #[test]
    fn test_git_checkout_file_restore_is_allowed() {
        let v = check("git checkout -- file.txt");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_git_merge_is_blocked() {
        let v = check("git merge feature");
        assert!(v.is_blocked());
        assert!(v.reason.contains("merge"));
    }

    #[test]
    fn test_git_rebase_is_blocked() {
        let v = check("git rebase main");
        assert!(v.is_blocked());
        assert!(v.reason.contains("rebase"));
    }

    #[test]
    fn test_git_cherry_pick_is_blocked() {
        let v = check("git cherry-pick abc1234");
        assert!(v.is_blocked());
        assert!(v.reason.contains("cherry-pick"));
    }

    #[test]
    fn test_git_reset_is_blocked() {
        let v = check("git reset HEAD~1");
        assert!(v.is_blocked());
        assert!(v.reason.contains("reset"));
    }

    #[test]
    fn test_git_reset_hard_is_blocked() {
        let v = check("git reset --hard HEAD~1");
        assert!(v.is_blocked());
        assert!(v.reason.contains("reset"));
    }

    #[test]
    fn test_git_checkout_orphan_is_blocked() {
        let v = check("git checkout --orphan new-branch");
        assert!(v.is_blocked());
        assert!(v.reason.contains("switch") || v.reason.contains("checkout"));
    }

    // -- Helper function tests --

    #[test]
    fn test_basename_extracts_name() {
        assert_eq!(basename("/usr/bin/git"), "git");
        assert_eq!(basename("git"), "git");
        assert_eq!(basename("C:\\Program Files\\git"), "git");
    }

    #[test]
    fn test_is_var_assignment() {
        assert!(is_var_assignment("FOO=bar"));
        assert!(is_var_assignment("_VAR=val"));
        assert!(!is_var_assignment("=bar"));
        assert!(!is_var_assignment("git"));
        assert!(!is_var_assignment("-c"));
    }

    #[test]
    fn test_has_expansion_marker() {
        assert!(has_expansion_marker("$HOME"));
        assert!(has_expansion_marker("$(cmd)"));
        assert!(has_expansion_marker("`cmd`"));
        assert!(!has_expansion_marker("git"));
    }

    // -- Finding 1: Compound-command redirects --

    #[test]
    fn test_subshell_redirect_command_substitution_git_add_is_blocked() {
        let v = check("(echo hi) > $(git add .)");
        assert!(v.is_blocked(), "subshell redirect with $(git add .) should be blocked");
    }

    // -- Finding 2: Launcher option parsing (command -p, exec -c) --

    #[test]
    fn test_command_p_git_add_is_blocked() {
        // `command -p` is a no-arg flag; git add should still be detected
        let v = check("command -p git add .");
        assert!(v.is_blocked(), "command -p git add . should be blocked");
    }

    #[test]
    fn test_exec_c_git_add_is_blocked() {
        // `exec -c` is a no-arg flag; git add should still be detected
        let v = check("exec -c git add .");
        assert!(v.is_blocked(), "exec -c git add . should be blocked");
    }

    #[test]
    fn test_exec_a_name_git_add_is_blocked() {
        // `exec -a name` takes an argument; git add should still be detected
        let v = check("exec -a myname git add .");
        assert!(v.is_blocked(), "exec -a myname git add . should be blocked");
    }

    // -- Finding 3: Branch-delete bundled short flags --

    #[test]
    fn test_git_branch_dr_is_blocked() {
        let v = check("git branch -dr feature");
        assert!(v.is_blocked(), "git branch -dr should be blocked as delete");
    }

    #[test]
    fn test_git_branch_r_upper_d_bundled_is_blocked() {
        let v = check("git branch -rD feature");
        assert!(v.is_blocked(), "git branch -rD should be blocked as delete");
    }

    #[test]
    fn test_variable_bypass_branch_dr_is_blocked() {
        let v = check("$CMD branch -dr feature");
        assert!(v.is_blocked(), "$CMD branch -dr should be blocked");
    }

    // -- Finding 4: Python git branch create should be allowed --

    #[test]
    fn test_python_c_git_branch_create_is_blocked() {
        // Any "git" reference in python -c is now blocked (broad match policy)
        let v = check(r#"python3 -c "subprocess.run(['git','branch','feature'])""#);
        assert!(v.is_blocked(), "python git branch create should be blocked (broad match)");
    }

    #[test]
    fn test_python_c_git_branch_delete_is_still_blocked() {
        let v = check(r#"python3 -c "subprocess.run(['git','branch','-D','feature'])""#);
        assert!(v.is_blocked(), "python git branch -D should still be blocked");
    }

    // -- Round 6 fixes --

    // env unconditional block
    #[test]
    fn test_env_unquoted_git_add_is_blocked() {
        let v = check("env git add .");
        assert!(v.is_blocked(), "env git add . should be blocked");
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_variable_expansion_in_command_position_always_blocked() {
        // Any expansion marker in any position is blocked
        let v = check("$CMD status");
        assert!(v.is_blocked(), "$CMD status should be blocked");
        let v = check("$(which ls) -la");
        assert!(v.is_blocked(), "$(which ls) should be blocked");
        let v = check("`which cat` file.txt");
        assert!(v.is_blocked(), "backtick should be blocked");
    }

    // Finding 2: .exe suffix bypass
    #[test]
    fn test_git_exe_add_is_blocked() {
        let v = check("git.exe add .");
        assert!(v.is_blocked(), "git.exe add . should be blocked");
    }

    #[test]
    fn test_bash_exe_c_git_push_is_blocked() {
        let v = check("bash.exe -c 'git push'");
        assert!(v.is_blocked(), "bash.exe -c git push should be blocked");
    }

    // -- Round 11 fixes --

    #[test]
    fn test_bash_rcfile_c_git_add_is_blocked() {
        // --rcfile takes a file argument; -c should still be found after it
        let v = check("bash --rcfile /dev/null -c 'git add .'");
        assert!(v.is_blocked(), "bash --rcfile /dev/null -c 'git add .' should be blocked");
    }

    #[test]
    fn test_bash_init_file_c_git_push_is_blocked() {
        let v = check("bash --init-file /etc/profile -c 'git push'");
        assert!(v.is_blocked(), "bash --init-file /etc/profile -c should be blocked");
    }

    #[test]
    fn test_python3_long_opt_c_git_is_blocked() {
        let v = check(
            r#"python3 --check-hash-based-pycs always -c "import subprocess; subprocess.run(['git','add','.'])""#,
        );
        assert!(v.is_blocked(), "python3 --check-hash-based-pycs always -c should be blocked");
    }

    #[test]
    fn test_timeout_signal_eq_kill_git_add_is_blocked() {
        // --signal=KILL should not regress timeout positional skip
        let v = check("timeout --signal=KILL 30 git add .");
        assert!(v.is_blocked(), "timeout --signal=KILL 30 git add . should be blocked");
    }

    #[test]
    fn test_absolute_path_git_exe_is_blocked() {
        let v = check("/usr/bin/git.exe add .");
        assert!(v.is_blocked(), "absolute path git.exe should be blocked");
    }

    #[test]
    fn test_basename_strips_exe_suffix() {
        assert_eq!(basename("git.exe"), "git");
        assert_eq!(basename("/usr/bin/git.exe"), "git");
        assert_eq!(basename("git"), "git");
    }

    // Finding 4: variable bypass + branch unconditionally blocked
    #[test]
    fn test_variable_bypass_branch_double_dash_is_blocked() {
        // $CMD branch -- -dev is blocked (variable bypass makes intent opaque)
        let v = check("$CMD branch -- -dev");
        assert!(v.is_blocked(), "$CMD branch -- -dev should be blocked");
    }

    // -- Round 7 fixes (env now unconditionally blocked) --

    #[test]
    fn test_env_u_then_s_git_add_is_blocked() {
        // env is blocked regardless of flags
        let v = check("env -u FOO -S 'git add .'");
        assert!(v.is_blocked(), "env should be blocked");
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_c_dir_is_blocked() {
        let v = check("env -C /tmp git status");
        assert!(v.is_blocked(), "env should be blocked");
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_u_shell_is_blocked() {
        // env -uSHELL is now blocked (env unconditional block)
        let v = check("env -uSHELL git status");
        assert!(v.is_blocked(), "env -uSHELL should be blocked");
        assert!(v.reason.contains("env"));
    }

    #[test]
    fn test_env_u_s_is_blocked() {
        // env -uS is now blocked (env unconditional block)
        let v = check("env -uS git status");
        assert!(v.is_blocked(), "env -uS should be blocked");
        assert!(v.reason.contains("env"));
    }

    // Finding 2: mixed-case .exe
    #[test]
    fn test_git_upper_exe_is_blocked() {
        let v = check("git.EXE add .");
        assert!(v.is_blocked(), "git.EXE add . should be blocked");
    }

    #[test]
    fn test_bash_mixed_exe_is_blocked() {
        let v = check("bash.Exe -c 'git push'");
        assert!(v.is_blocked(), "bash.Exe should be blocked");
    }

    #[test]
    fn test_basename_strips_mixed_case_exe() {
        assert_eq!(basename("git.EXE"), "git");
        assert_eq!(basename("git.Exe"), "git");
        assert_eq!(basename("git.exe"), "git");
        assert_eq!(basename("git"), "git");
    }

    // -- Heredoc bypass fix --

    #[test]
    fn test_bash_heredoc_git_add_is_blocked() {
        let v = check("bash <<'SH'\ngit add .\nSH");
        assert!(v.is_blocked(), "bash heredoc with git add should be blocked");
    }

    #[test]
    fn test_python_heredoc_git_subprocess_is_blocked() {
        let v = check("python3 - <<'PY'\nimport subprocess; subprocess.run(['git','add','.'])\nPY");
        assert!(v.is_blocked(), "python heredoc with git subprocess should be blocked");
    }

    #[test]
    fn test_bash_heredoc_without_git_is_allowed() {
        let v = check("bash <<'SH'\necho hello\nSH");
        assert!(!v.is_blocked(), "bash heredoc without git should be allowed");
    }

    #[test]
    fn test_redirect_to_file_without_git_is_allowed() {
        let v = check("echo hello > /tmp/output.txt");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_brace_group_heredoc_git_add_is_blocked() {
        let v = check("{ bash; } <<'SH'\ngit add .\nSH");
        assert!(v.is_blocked(), "brace group heredoc with git add should be blocked");
    }

    #[test]
    fn test_subshell_heredoc_git_add_is_blocked() {
        let v = check("( bash ) <<'SH'\ngit add .\nSH");
        assert!(v.is_blocked(), "subshell heredoc with git add should be blocked");
    }

    // -- Malformed launcher input (no panic) --

    #[test]
    fn test_taskset_trailing_option_no_panic() {
        // taskset -o has -o as option-with-arg but no following token
        let v = check("taskset -o");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_timeout_trailing_signal_no_panic() {
        // timeout -s has -s as option-with-arg but no following token
        let v = check("timeout -s");
        assert!(!v.is_blocked());
    }

    // -- UTF-8 safety --

    #[test]
    fn test_multibyte_utf8_command_no_panic() {
        // €aab is 6 bytes (€=3 bytes), len-4 would be inside € without safe handling
        let v = check("€aab");
        assert!(!v.is_blocked());
    }

    #[test]
    fn test_multibyte_utf8_with_exe_suffix_no_panic() {
        let v = check("日本語.exe add");
        assert!(!v.is_blocked());
    }
}
