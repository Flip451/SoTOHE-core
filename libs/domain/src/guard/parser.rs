// Parser module uses bounded array indexing extensively for character-by-character
// tokenization where loop conditions already guarantee in-bounds access.
#![allow(clippy::indexing_slicing)]

//! Shell command splitter backed by `conch-parser`.
//!
//! Thin adapter over `conch_parser` that parses a shell command string into
//! individual [`SimpleCommand`] values. Walks the conch-parser AST to extract
//! commands from pipelines, and/or lists, subshells, compound commands, and
//! command substitutions.

use conch_parser::ast;
use conch_parser::lexer::Lexer;
use conch_parser::parse::DefaultParser;

use super::verdict::ParseError;

/// Maximum nesting depth for command substitution extraction.
const MAX_NESTING_DEPTH: usize = 16;

/// A parsed simple command (argv list + redirect texts + output redirect flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    /// The argument vector of the command.
    pub argv: Vec<String>,
    /// Flattened text from redirect targets (including heredoc bodies).
    /// Used by policy to detect git references hidden in heredocs.
    pub redirect_texts: Vec<String>,
    /// Whether this command has any output redirect (Write/Append/Clobber).
    /// Does NOT include DupWrite (`>&fd`) or Read (`<`).
    pub has_output_redirect: bool,
}

/// Splits a shell command string into individual simple commands.
///
/// Handles control operators (`;`, `&&`, `||`, `|`), subshells,
/// compound commands, and command substitutions via conch-parser AST walking.
///
/// # Errors
///
/// Returns `ParseError::NestingDepthExceeded` if nesting exceeds 16 levels.
/// Returns `ParseError::UnmatchedQuote` on parse failures (fail-closed).
pub fn split_shell(input: &str) -> Result<Vec<SimpleCommand>, ParseError> {
    split_shell_inner(input, 0)
}

/// Extracts nested command strings from `$(...)` and backtick substitutions
/// found in the given input. Returns them as raw strings for recursive parsing.
///
/// # Errors
///
/// Returns `ParseError` on nesting depth exceeded or parse errors.
pub fn extract_command_substitutions(input: &str) -> Result<Vec<String>, ParseError> {
    extract_substitutions_inner(input, 0)
}

/// Tokenizes a single simple command string into an argv-like word list.
///
/// Handles single quotes, double quotes, and backslash escaping.
/// Does NOT split on control operators -- use [`split_shell`] first.
///
/// # Errors
///
/// Returns `ParseError::UnmatchedQuote` on unclosed quotes.
pub fn tokenize(input: &str) -> Result<Vec<String>, ParseError> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        match ch {
            // Backslash escape outside quotes
            '\\' if i + 1 < len => {
                current.push(chars[i + 1]);
                i += 2;
            }
            // Single quote -- everything until closing single quote is literal
            '\'' => {
                i += 1;
                while i < len && chars[i] != '\'' {
                    current.push(chars[i]);
                    i += 1;
                }
                if i >= len {
                    return Err(ParseError::UnmatchedQuote);
                }
                i += 1; // skip closing '
            }
            // Double quote -- allows backslash escape and $-expansion tracking
            '"' => {
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        // In double quotes, backslash only escapes: $ ` " \ newline
                        let next = chars[i + 1];
                        if matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                            current.push(next);
                            i += 2;
                        } else {
                            current.push('\\');
                            i += 1;
                        }
                    } else {
                        current.push(chars[i]);
                        i += 1;
                    }
                }
                if i >= len {
                    return Err(ParseError::UnmatchedQuote);
                }
                i += 1; // skip closing "
            }
            // Whitespace -- word boundary
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                i += 1;
            }
            // Everything else
            _ => {
                current.push(ch);
                i += 1;
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
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

// ---------------------------------------------------------------------------
// Word flattening: conch-parser word AST -> String
// ---------------------------------------------------------------------------

/// Flattens a `TopLevelWord` to a plain string.
fn flatten_top_level_word(word: &ast::TopLevelWord<String>) -> String {
    flatten_complex_word(&word.0)
}

type ConchComplexWord = ast::ComplexWord<ConchWord>;

/// Flattens a `ComplexWord` to a plain string.
fn flatten_complex_word(cw: &ConchComplexWord) -> String {
    match cw {
        ast::ComplexWord::Single(w) => flatten_word(w),
        ast::ComplexWord::Concat(words) => {
            let mut s = String::new();
            for w in words {
                s.push_str(&flatten_word(w));
            }
            s
        }
    }
}

type ConchSimpleWord = ast::SimpleWord<
    String,
    ast::Parameter<String>,
    Box<
        ast::ParameterSubstitution<
            ast::Parameter<String>,
            ast::TopLevelWord<String>,
            ast::TopLevelCommand<String>,
            ast::Arithmetic<String>,
        >,
    >,
>;

type ConchWord = ast::Word<String, ConchSimpleWord>;

/// Flattens a `Word` to a plain string.
fn flatten_word(word: &ConchWord) -> String {
    match word {
        ast::Word::Simple(sw) => flatten_simple_word(sw),
        ast::Word::SingleQuoted(s) => s.clone(),
        ast::Word::DoubleQuoted(parts) => {
            let mut s = String::new();
            for sw in parts {
                s.push_str(&flatten_simple_word(sw));
            }
            s
        }
    }
}

/// Flattens a `SimpleWord` to a plain string.
fn flatten_simple_word(sw: &ConchSimpleWord) -> String {
    match sw {
        ast::SimpleWord::Literal(s) | ast::SimpleWord::Escaped(s) => s.clone(),
        ast::SimpleWord::Param(p) => format!("{p}"),
        ast::SimpleWord::Subst(subst) => flatten_substitution(subst),
        ast::SimpleWord::Star => "*".to_string(),
        ast::SimpleWord::Question => "?".to_string(),
        ast::SimpleWord::SquareOpen => "[".to_string(),
        ast::SimpleWord::SquareClose => "]".to_string(),
        ast::SimpleWord::Tilde => "~".to_string(),
        ast::SimpleWord::Colon => ":".to_string(),
    }
}

/// Flattens a `ParameterSubstitution` to an approximate string representation.
fn flatten_substitution(
    subst: &ast::ParameterSubstitution<
        ast::Parameter<String>,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
        ast::Arithmetic<String>,
    >,
) -> String {
    match subst {
        ast::ParameterSubstitution::Command(cmds) => {
            // Reconstruct as $(...) placeholder -- the actual commands are
            // handled separately via command substitution extraction
            let mut inner = String::new();
            for cmd in cmds {
                if !inner.is_empty() {
                    inner.push(' ');
                }
                inner.push_str(&flatten_top_level_command_to_string(cmd));
            }
            format!("$({inner})")
        }
        ast::ParameterSubstitution::Len(p) => format!("${{#{p}}}"),
        ast::ParameterSubstitution::Arith(_) => String::new(),
        ast::ParameterSubstitution::Default(_, p, w) => {
            let val = w.as_ref().map(flatten_top_level_word).unwrap_or_default();
            format!("${{{p}:-{val}}}")
        }
        ast::ParameterSubstitution::Assign(_, p, w) => {
            let val = w.as_ref().map(flatten_top_level_word).unwrap_or_default();
            format!("${{{p}:={val}}}")
        }
        ast::ParameterSubstitution::Error(_, p, w) => {
            let val = w.as_ref().map(flatten_top_level_word).unwrap_or_default();
            format!("${{{p}:?{val}}}")
        }
        ast::ParameterSubstitution::Alternative(_, p, w) => {
            let val = w.as_ref().map(flatten_top_level_word).unwrap_or_default();
            format!("${{{p}:+{val}}}")
        }
        _ => String::new(),
    }
}

/// Produces a rough string representation of a top-level command (for embedding
/// in flattened substitution output).
fn flatten_top_level_command_to_string(tlc: &ast::TopLevelCommand<String>) -> String {
    // We only need a rough approximation -- the actual command is extracted
    // separately via the AST walker.
    let mut parts = Vec::new();
    let mut collect = |simple: &ast::SimpleCommand<
        String,
        ast::TopLevelWord<String>,
        ast::Redirect<ast::TopLevelWord<String>>,
    >| {
        for item in &simple.redirects_or_cmd_words {
            if let ast::RedirectOrCmdWord::CmdWord(word) = item {
                parts.push(flatten_top_level_word(word));
            }
        }
    };

    // Walk just the top level simply
    match &tlc.0 {
        ast::Command::List(and_or) | ast::Command::Job(and_or) => {
            walk_and_or_for_flatten(and_or, &mut collect);
        }
    }
    parts.join(" ")
}

/// Helper to walk an and-or list for flattening purposes only.
fn walk_and_or_for_flatten(
    list: &ast::CommandList<String, ast::TopLevelWord<String>, ast::TopLevelCommand<String>>,
    collect: &mut impl FnMut(
        &ast::SimpleCommand<
            String,
            ast::TopLevelWord<String>,
            ast::Redirect<ast::TopLevelWord<String>>,
        >,
    ),
) {
    walk_listable_for_flatten(&list.first, collect);
    for and_or in &list.rest {
        match and_or {
            ast::AndOr::And(cmd) | ast::AndOr::Or(cmd) => {
                walk_listable_for_flatten(cmd, collect);
            }
        }
    }
}

fn walk_listable_for_flatten(
    cmd: &ast::ListableCommand<
        ast::ShellPipeableCommand<String, ast::TopLevelWord<String>, ast::TopLevelCommand<String>>,
    >,
    collect: &mut impl FnMut(
        &ast::SimpleCommand<
            String,
            ast::TopLevelWord<String>,
            ast::Redirect<ast::TopLevelWord<String>>,
        >,
    ),
) {
    match cmd {
        ast::ListableCommand::Single(p) => walk_pipeable_for_flatten(p, collect),
        ast::ListableCommand::Pipe(_, ps) => {
            for p in ps {
                walk_pipeable_for_flatten(p, collect);
            }
        }
    }
}

fn walk_pipeable_for_flatten(
    cmd: &ast::ShellPipeableCommand<
        String,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
    >,
    collect: &mut impl FnMut(
        &ast::SimpleCommand<
            String,
            ast::TopLevelWord<String>,
            ast::Redirect<ast::TopLevelWord<String>>,
        >,
    ),
) {
    if let ast::PipeableCommand::Simple(s) = cmd {
        collect(s);
    }
}

// ---------------------------------------------------------------------------
// Command substitution extraction from AST words
// ---------------------------------------------------------------------------

/// Returns `true` if the redirect is an output redirect (Write/Append/Clobber).
///
/// Does NOT match DupWrite (`>&fd`) — that's FD duplication, not a file write.
/// Does NOT match Read (`<`), ReadWrite (`<>`), DupRead (`<&`), or Heredoc (`<<`).
/// Returns `true` if the redirect opens a writable file descriptor.
///
/// Matches Write (`>`), Append (`>>`), Clobber (`>|`), and ReadWrite (`<>`).
/// Does NOT match DupWrite (`>&fd`) — that's FD duplication, not a file open.
/// Does NOT match Read (`<`) or DupRead (`<&`).
fn is_output_redirect(redirect: &ast::Redirect<ast::TopLevelWord<String>>) -> bool {
    matches!(
        redirect,
        ast::Redirect::Write(..)
            | ast::Redirect::Append(..)
            | ast::Redirect::Clobber(..)
            | ast::Redirect::ReadWrite(..)
    )
}

/// Extracts the target word from a `Redirect` variant.
fn extract_redirect_word(
    redirect: &ast::Redirect<ast::TopLevelWord<String>>,
) -> Option<&ast::TopLevelWord<String>> {
    match redirect {
        ast::Redirect::Read(_, w)
        | ast::Redirect::Write(_, w)
        | ast::Redirect::ReadWrite(_, w)
        | ast::Redirect::Append(_, w)
        | ast::Redirect::Clobber(_, w)
        | ast::Redirect::Heredoc(_, w)
        | ast::Redirect::DupRead(_, w)
        | ast::Redirect::DupWrite(_, w) => Some(w),
    }
}

/// Collects `ParameterSubstitution::Command` references found in a word.
fn collect_command_substitutions_from_word(
    word: &ast::TopLevelWord<String>,
    out: &mut Vec<Vec<ast::TopLevelCommand<String>>>,
) {
    collect_subst_from_complex_word(&word.0, out);
}

fn collect_subst_from_complex_word(
    cw: &ConchComplexWord,
    out: &mut Vec<Vec<ast::TopLevelCommand<String>>>,
) {
    match cw {
        ast::ComplexWord::Single(w) => collect_subst_from_word(w, out),
        ast::ComplexWord::Concat(words) => {
            for w in words {
                collect_subst_from_word(w, out);
            }
        }
    }
}

fn collect_subst_from_word(word: &ConchWord, out: &mut Vec<Vec<ast::TopLevelCommand<String>>>) {
    match word {
        ast::Word::Simple(sw) => collect_subst_from_simple_word(sw, out),
        ast::Word::SingleQuoted(_) => {
            // No substitutions inside single quotes
        }
        ast::Word::DoubleQuoted(parts) => {
            for sw in parts {
                collect_subst_from_simple_word(sw, out);
            }
        }
    }
}

fn collect_subst_from_simple_word(
    sw: &ConchSimpleWord,
    out: &mut Vec<Vec<ast::TopLevelCommand<String>>>,
) {
    if let ast::SimpleWord::Subst(subst) = sw {
        if let ast::ParameterSubstitution::Command(cmds) = subst.as_ref() {
            out.push(cmds.clone());
        }
        // Also recurse into word parts of parameter substitutions
        collect_subst_from_param_subst(subst, out);
    }
}

fn collect_subst_from_param_subst(
    subst: &ast::ParameterSubstitution<
        ast::Parameter<String>,
        ast::TopLevelWord<String>,
        ast::TopLevelCommand<String>,
        ast::Arithmetic<String>,
    >,
    out: &mut Vec<Vec<ast::TopLevelCommand<String>>>,
) {
    match subst {
        ast::ParameterSubstitution::Default(_, _, Some(w))
        | ast::ParameterSubstitution::Assign(_, _, Some(w))
        | ast::ParameterSubstitution::Error(_, _, Some(w))
        | ast::ParameterSubstitution::Alternative(_, _, Some(w))
        | ast::ParameterSubstitution::RemoveSmallestSuffix(_, Some(w))
        | ast::ParameterSubstitution::RemoveLargestSuffix(_, Some(w))
        | ast::ParameterSubstitution::RemoveSmallestPrefix(_, Some(w))
        | ast::ParameterSubstitution::RemoveLargestPrefix(_, Some(w)) => {
            collect_command_substitutions_from_word(w, out);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// extract_command_substitutions (string-level API)
// ---------------------------------------------------------------------------

fn extract_substitutions_inner(input: &str, depth: usize) -> Result<Vec<String>, ParseError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(ParseError::NestingDepthExceeded { max: MAX_NESTING_DEPTH });
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // We use the hand-written extractor here because conch-parser requires
    // the input to be a valid complete command; but extract_command_substitutions
    // may be called on partial/word-level content.
    // The old hand-written logic is preserved for this function.
    let mut results = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < len {
        let ch = chars[i];

        // Escape
        if ch == '\\' && !in_single_quote && i + 1 < len {
            i += 2;
            continue;
        }

        // Quote tracking
        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }

        if in_single_quote {
            i += 1;
            continue;
        }

        // $(...) -- but not $((...))
        if ch == '$' && i + 1 < len && chars[i + 1] == '(' && !(i + 2 < len && chars[i + 2] == '(')
        {
            let (content, end) = extract_balanced_paren(&chars, i + 1)?;
            results.push(content);
            i = end;
            continue;
        }

        // Backtick
        if ch == '`' {
            let (content, end) = extract_backtick(&chars, i)?;
            results.push(content);
            i = end;
            continue;
        }

        i += 1;
    }

    Ok(results)
}

/// Extracts balanced parentheses content starting at `start` (which points to `(`).
/// Returns (inner_content, position_after_closing_paren).
fn extract_balanced_paren(chars: &[char], start: usize) -> Result<(String, usize), ParseError> {
    // Caller guarantees chars[start] == '('
    let mut depth = 1usize;
    let mut i = start + 1;
    let mut content = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '\\' && !in_single_quote && i + 1 < chars.len() {
            content.push(ch);
            content.push(chars[i + 1]);
            i += 2;
            continue;
        }

        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            content.push(ch);
            i += 1;
            continue;
        }
        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            content.push(ch);
            i += 1;
            continue;
        }

        if !in_single_quote && !in_double_quote {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Ok((content, i + 1));
                }
            }
        }

        content.push(ch);
        i += 1;
    }

    // Unmatched opening paren -- treat as parse error
    Err(ParseError::UnmatchedQuote)
}

/// Extracts backtick-delimited content starting at `start` (which points to `` ` ``).
/// Returns (inner_content, position_after_closing_backtick).
fn extract_backtick(chars: &[char], start: usize) -> Result<(String, usize), ParseError> {
    // Caller guarantees chars[start] == '`'
    let mut i = start + 1;
    let mut content = String::new();

    while i < chars.len() {
        let ch = chars[i];

        if ch == '\\' && i + 1 < chars.len() {
            content.push(chars[i + 1]);
            i += 2;
            continue;
        }

        if ch == '`' {
            return Ok((content, i + 1));
        }

        content.push(ch);
        i += 1;
    }

    Err(ParseError::UnmatchedQuote)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use rstest::rstest;

    use super::*;

    // -- tokenize: success cases --

    #[rstest]
    #[case::simple_command("git add .", vec!["git", "add", "."])]
    #[case::single_quotes("echo 'hello world'", vec!["echo", "hello world"])]
    #[case::double_quotes(r#"echo "hello world""#, vec!["echo", "hello world"])]
    #[case::backslash_escape(r"echo hello\ world", vec!["echo", "hello world"])]
    #[case::preserves_dollar_signs("echo $HOME", vec!["echo", "$HOME"])]
    fn test_tokenize_success(#[case] input: &str, #[case] expected: Vec<&str>) {
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_tokenize_empty_string() {
        let tokens = tokenize("").unwrap();
        assert!(tokens.is_empty());
    }

    // -- tokenize: error cases --

    #[rstest]
    #[case::unmatched_single_quote("echo 'hello")]
    #[case::unmatched_double_quote(r#"echo "hello"#)]
    fn test_tokenize_unmatched_quote_returns_error(#[case] input: &str) {
        assert!(matches!(tokenize(input), Err(ParseError::UnmatchedQuote)));
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
        let cmds = split_shell(input).unwrap();
        assert_eq!(cmds.len(), expected_count);
    }

    #[test]
    fn test_split_simple_command() {
        let cmds = split_shell("git status").unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].argv, vec!["git", "status"]);
    }

    #[test]
    fn test_split_semicolon() {
        let cmds = split_shell("echo a; echo b").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].argv, vec!["echo", "a"]);
        assert_eq!(cmds[1].argv, vec!["echo", "b"]);
    }

    #[test]
    fn test_split_pipe() {
        let cmds = split_shell("ls | grep foo").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].argv, vec!["ls"]);
        assert_eq!(cmds[1].argv, vec!["grep", "foo"]);
    }

    #[test]
    fn test_split_does_not_split_inside_quotes() {
        let cmds = split_shell("echo 'a && b'").unwrap();
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
        let cmds = split_shell(input).unwrap();
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
        let cmds = split_shell("echo hi > /tmp/file.txt").unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].argv, vec!["echo", "hi"]);
    }

    #[test]
    fn test_nesting_depth_exceeded() {
        let mut cmd = "echo hello".to_string();
        for _ in 0..20 {
            cmd = format!("echo $({})", cmd);
        }
        let result = split_shell(&cmd);
        assert!(matches!(result, Err(ParseError::NestingDepthExceeded { .. })));
    }

    // -- extract_command_substitutions tests --

    #[rstest]
    #[case::dollar_paren("echo $(git status) done", vec!["git status"])]
    #[case::backtick("echo `date` done", vec!["date"])]
    #[case::nested_dollar_paren("echo $(echo $(date))", vec!["echo $(date)"])]
    fn test_extract_command_substitutions_success(
        #[case] input: &str,
        #[case] expected: Vec<&str>,
    ) {
        let subs = extract_command_substitutions(input).unwrap();
        assert_eq!(subs, expected);
    }

    #[test]
    fn test_no_substitutions_in_single_quotes() {
        let subs = extract_command_substitutions("echo '$(git status)'").unwrap();
        assert!(subs.is_empty());
    }
}
