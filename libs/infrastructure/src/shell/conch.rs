// Conch module uses bounded array indexing for AST traversal where
// preceding match arms guarantee the correct variant and in-bounds access.
#![allow(clippy::indexing_slicing)]

//! conch-parser backed shell command splitter.
//!
//! Walks the conch-parser AST to extract individual [`SimpleCommand`] values
//! from pipelines, and/or lists, subshells, compound commands, and
//! command substitutions.

use conch_parser::ast;
use conch_parser::lexer::Lexer;
use conch_parser::parse::DefaultParser;

use domain::guard::{ParseError, ShellParser, SimpleCommand};

use super::flatten::{
    collect_command_substitutions_from_word, extract_redirect_word, flatten_top_level_word,
    is_output_redirect,
};

/// Maximum nesting depth for command substitution extraction.
const MAX_NESTING_DEPTH: usize = 16;

/// conch-parser backed implementation of [`ShellParser`].
pub struct ConchShellParser;

impl ShellParser for ConchShellParser {
    fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError> {
        split_shell_inner(input, 0)
    }
}

// ---------------------------------------------------------------------------
// Internal: conch-parser based splitting
// ---------------------------------------------------------------------------

fn split_shell_inner(input: &str, depth: usize) -> Result<Vec<SimpleCommand>, ParseError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(ParseError::NestingDepthExceeded { max: MAX_NESTING_DEPTH });
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let lexer = Lexer::new(trimmed.chars());
    let parser = DefaultParser::new(lexer);

    let mut commands = Vec::new();

    for result in parser {
        let top_level_cmd = result.map_err(|_| ParseError::UnmatchedQuote)?;
        collect_from_top_level_command(&top_level_cmd, &mut commands, depth)?;
    }

    Ok(commands)
}

/// Collects simple commands from a `TopLevelCommand`.
fn collect_from_top_level_command(
    tlc: &ast::TopLevelCommand<String>,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(ParseError::NestingDepthExceeded { max: MAX_NESTING_DEPTH });
    }
    match &tlc.0 {
        ast::Command::List(and_or) | ast::Command::Job(and_or) => {
            collect_from_and_or_list(and_or, out, depth)?;
        }
    }
    Ok(())
}

/// Collects simple commands from an `AndOrList`.
fn collect_from_and_or_list(
    list: &ast::CommandList<String, ast::TopLevelWord<String>, ast::TopLevelCommand<String>>,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    collect_from_listable(&list.first, out, depth)?;
    for and_or in &list.rest {
        match and_or {
            ast::AndOr::And(cmd) | ast::AndOr::Or(cmd) => {
                collect_from_listable(cmd, out, depth)?;
            }
        }
    }
    Ok(())
}

/// Collects simple commands from a `ListableCommand`.
fn collect_from_listable(
    cmd: &ast::ListableCommand<
        ast::ShellPipeableCommand<String, ast::TopLevelWord<String>, ast::TopLevelCommand<String>>,
    >,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    match cmd {
        ast::ListableCommand::Single(pipeable) => {
            collect_from_pipeable(pipeable, out, depth)?;
        }
        ast::ListableCommand::Pipe(_, cmds) => {
            for pipeable in cmds {
                collect_from_pipeable(pipeable, out, depth)?;
            }
        }
    }
    Ok(())
}

/// Collects simple commands from a `PipeableCommand`.
fn collect_from_pipeable(
    cmd: &ast::ShellPipeableCommand<
        String,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
    >,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    match cmd {
        ast::PipeableCommand::Simple(simple) => {
            collect_from_conch_simple(simple, out, depth)?;
        }
        ast::PipeableCommand::Compound(compound) => {
            collect_from_compound(compound, out, depth)?;
        }
        ast::PipeableCommand::FunctionDef(_, body) => {
            collect_from_compound(body, out, depth)?;
        }
    }
    Ok(())
}

/// Converts a conch-parser `SimpleCommand` into our `SimpleCommand` and
/// recursively extracts commands from any command substitutions in the words.
fn collect_from_conch_simple(
    simple: &ast::SimpleCommand<
        String,
        ast::TopLevelWord<String>,
        ast::Redirect<ast::TopLevelWord<String>>,
    >,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    let mut argv = Vec::new();

    // Collect env var assignments as "KEY=val" tokens in argv
    for item in &simple.redirects_or_env_vars {
        match item {
            ast::RedirectOrEnvVar::EnvVar(name, value) => {
                let val_str = value.as_ref().map(flatten_top_level_word).unwrap_or_default();
                argv.push(format!("{name}={val_str}"));
            }
            ast::RedirectOrEnvVar::Redirect(_) => {
                // Redirects before command -- skip for argv
                // (command substitutions in redirect targets are extracted below)
            }
        }
    }

    // Collect command words
    let mut cmd_words = Vec::new();
    for item in &simple.redirects_or_cmd_words {
        if let ast::RedirectOrCmdWord::CmdWord(word) = item {
            cmd_words.push(word);
            argv.push(flatten_top_level_word(word));
        }
    }

    // Collect flattened text from redirect targets (including heredoc bodies)
    let mut redirect_texts = Vec::new();
    let mut has_output_redirect = false;
    for item in &simple.redirects_or_env_vars {
        if let ast::RedirectOrEnvVar::Redirect(redirect) = item {
            if is_output_redirect(redirect) {
                has_output_redirect = true;
            }
            if let Some(word) = extract_redirect_word(redirect) {
                redirect_texts.push(flatten_top_level_word(word));
            }
        }
    }
    for item in &simple.redirects_or_cmd_words {
        if let ast::RedirectOrCmdWord::Redirect(redirect) = item {
            if is_output_redirect(redirect) {
                has_output_redirect = true;
            }
            if let Some(word) = extract_redirect_word(redirect) {
                redirect_texts.push(flatten_top_level_word(word));
            }
        }
    }

    // Emit a SimpleCommand if there are arguments OR if there are output redirects.
    // Redirect-only commands like `> /tmp/file` must reach policy evaluation
    // so the CON-07 file-write guard can block them.
    if !argv.is_empty() || has_output_redirect {
        out.push(SimpleCommand { argv, redirect_texts, has_output_redirect });
    }

    // Recursively extract commands from command substitutions in all words,
    // including redirect target words (Finding 1: redirects must not be ignored).
    let mut subst_commands = Vec::new();
    for item in &simple.redirects_or_env_vars {
        match item {
            ast::RedirectOrEnvVar::EnvVar(_, Some(word)) => {
                collect_command_substitutions_from_word(word, &mut subst_commands);
            }
            ast::RedirectOrEnvVar::Redirect(redirect) => {
                if let Some(word) = extract_redirect_word(redirect) {
                    collect_command_substitutions_from_word(word, &mut subst_commands);
                }
            }
            _ => {}
        }
    }
    for item in &simple.redirects_or_cmd_words {
        match item {
            ast::RedirectOrCmdWord::CmdWord(word) => {
                collect_command_substitutions_from_word(word, &mut subst_commands);
            }
            ast::RedirectOrCmdWord::Redirect(redirect) => {
                if let Some(word) = extract_redirect_word(redirect) {
                    collect_command_substitutions_from_word(word, &mut subst_commands);
                }
            }
        }
    }

    for sub_cmds in subst_commands {
        for sub_tlc in &sub_cmds {
            collect_from_top_level_command(sub_tlc, out, depth + 1)?;
        }
    }

    Ok(())
}

/// Collects simple commands from a compound command (subshell, brace group,
/// if/while/until/for/case).
///
/// Also inspects `compound.io` (the redirect list attached to the compound
/// command) for command substitutions in redirect target words.
fn collect_from_compound(
    compound: &ast::ShellCompoundCommand<
        String,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
    >,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    let before_len = out.len();
    collect_from_compound_kind(&compound.kind, out, depth)?;

    // Flatten redirect texts from compound.io (including heredoc bodies)
    // and propagate them to all commands collected from inside the compound.
    // This ensures `{ bash; } <<'SH'\ngit add .\nSH` is detected.
    // Also propagate has_output_redirect for CON-07 file-write guard.
    let mut compound_redirect_texts = Vec::new();
    let mut compound_has_output_redirect = false;
    for redirect in &compound.io {
        if is_output_redirect(redirect) {
            compound_has_output_redirect = true;
        }
        if let Some(word) = extract_redirect_word(redirect) {
            compound_redirect_texts.push(flatten_top_level_word(word));
        }
    }
    if !compound_redirect_texts.is_empty() || compound_has_output_redirect {
        for cmd in &mut out[before_len..] {
            cmd.redirect_texts.extend(compound_redirect_texts.iter().cloned());
            if compound_has_output_redirect {
                cmd.has_output_redirect = true;
            }
        }
    }

    // Walk redirects attached to the compound command for command substitutions
    let mut subst_commands = Vec::new();
    for redirect in &compound.io {
        if let Some(word) = extract_redirect_word(redirect) {
            collect_command_substitutions_from_word(word, &mut subst_commands);
        }
    }
    for sub_cmds in subst_commands {
        for sub_tlc in &sub_cmds {
            collect_from_top_level_command(sub_tlc, out, depth + 1)?;
        }
    }

    Ok(())
}

/// Collects simple commands from a `CompoundCommandKind`.
fn collect_from_compound_kind(
    kind: &ast::CompoundCommandKind<
        String,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
    >,
    out: &mut Vec<SimpleCommand>,
    depth: usize,
) -> Result<(), ParseError> {
    match kind {
        ast::CompoundCommandKind::Brace(cmds) | ast::CompoundCommandKind::Subshell(cmds) => {
            for cmd in cmds {
                collect_from_top_level_command(cmd, out, depth)?;
            }
        }
        ast::CompoundCommandKind::While(pair) | ast::CompoundCommandKind::Until(pair) => {
            for cmd in &pair.guard {
                collect_from_top_level_command(cmd, out, depth)?;
            }
            for cmd in &pair.body {
                collect_from_top_level_command(cmd, out, depth)?;
            }
        }
        ast::CompoundCommandKind::If { conditionals, else_branch } => {
            for pair in conditionals {
                for cmd in &pair.guard {
                    collect_from_top_level_command(cmd, out, depth)?;
                }
                for cmd in &pair.body {
                    collect_from_top_level_command(cmd, out, depth)?;
                }
            }
            if let Some(else_cmds) = else_branch {
                for cmd in else_cmds {
                    collect_from_top_level_command(cmd, out, depth)?;
                }
            }
        }
        ast::CompoundCommandKind::For { words, body, .. } => {
            // Inspect iterator words for command substitutions
            if let Some(word_list) = words {
                let mut subst_commands = Vec::new();
                for w in word_list {
                    collect_command_substitutions_from_word(w, &mut subst_commands);
                }
                for sub_cmds in subst_commands {
                    for sub_tlc in &sub_cmds {
                        collect_from_top_level_command(sub_tlc, out, depth + 1)?;
                    }
                }
            }
            for cmd in body {
                collect_from_top_level_command(cmd, out, depth)?;
            }
        }
        ast::CompoundCommandKind::Case { word, arms } => {
            // Inspect the case subject word for command substitutions
            let mut subst_commands = Vec::new();
            collect_command_substitutions_from_word(word, &mut subst_commands);
            // Also inspect pattern words in each arm
            for arm in arms {
                for pattern in &arm.patterns {
                    collect_command_substitutions_from_word(pattern, &mut subst_commands);
                }
            }
            for sub_cmds in subst_commands {
                for sub_tlc in &sub_cmds {
                    collect_from_top_level_command(sub_tlc, out, depth + 1)?;
                }
            }
            // Walk arm bodies as before
            for arm in arms {
                for cmd in &arm.body {
                    collect_from_top_level_command(cmd, out, depth)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn parser() -> ConchShellParser {
        ConchShellParser
    }

    // -- split_shell: operator splitting --

    #[rstest]
    #[case::and_operator("cmd1 && cmd2", 2)]
    #[case::or_operator("cmd1 || cmd2", 2)]
    #[case::newline("echo a\necho b", 2)]
    fn test_split_shell_binary_operator_produces_two_commands(
        #[case] input: &str,
        #[case] expected_count: usize,
    ) {
        let cmds = parser().split_shell(input).unwrap();
        assert_eq!(cmds.len(), expected_count);
    }

    #[test]
    fn test_split_simple_command() {
        let cmds = parser().split_shell("git status").unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].argv, vec!["git", "status"]);
    }

    #[test]
    fn test_split_semicolon() {
        let cmds = parser().split_shell("echo a; echo b").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].argv, vec!["echo", "a"]);
        assert_eq!(cmds[1].argv, vec!["echo", "b"]);
    }

    #[test]
    fn test_split_pipe() {
        let cmds = parser().split_shell("ls | grep foo").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].argv, vec!["ls"]);
        assert_eq!(cmds[1].argv, vec!["grep", "foo"]);
    }

    #[test]
    fn test_split_does_not_split_inside_quotes() {
        let cmds = parser().split_shell("echo 'a && b'").unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].argv, vec!["echo", "a && b"]);
    }

    // -- split_shell: command substitution extraction --

    #[rstest]
    #[case::dollar_paren("echo $(git status)", vec!["git", "status"])]
    #[case::backtick("echo `git log`", vec!["git", "log"])]
    #[case::redirect_target("echo hi > $(git add .)", vec!["git", "add", "."])]
    #[case::subshell_redirect("(echo hi) > $(git add .)", vec!["git", "add", "."])]
    #[case::for_iterator("for x in $(git add .); do echo hi; done", vec!["git", "add", "."])]
    #[case::case_subject("case $(git add .) in foo) echo hi;; esac", vec!["git", "add", "."])]
    fn test_split_shell_nested_command_is_extracted(
        #[case] input: &str,
        #[case] expected_nested_argv: Vec<&str>,
    ) {
        let cmds = parser().split_shell(input).unwrap();
        assert!(
            cmds.len() >= 2,
            "expected at least 2 commands (outer + nested) for {:?}, got {}",
            input,
            cmds.len()
        );
        let nested = cmds.iter().find(|c| c.argv == expected_nested_argv);
        assert!(
            nested.is_some(),
            "expected nested command {:?} in {:?}",
            expected_nested_argv,
            input
        );
    }

    #[test]
    fn test_split_redirect_without_substitution() {
        let cmds = parser().split_shell("echo hi > /tmp/file.txt").unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].argv, vec!["echo", "hi"]);
    }

    #[test]
    fn test_nesting_depth_exceeded() {
        let mut cmd = "echo hello".to_string();
        for _ in 0..20 {
            cmd = format!("echo $({})", cmd);
        }
        let result = parser().split_shell(&cmd);
        assert!(matches!(result, Err(ParseError::NestingDepthExceeded { .. })));
    }
}
