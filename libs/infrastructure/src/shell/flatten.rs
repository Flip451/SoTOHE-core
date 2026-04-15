//! conch-parser AST flattening, redirect helpers, and command substitution extraction.
//!
//! These are internal helpers used by `super::conch`.
//! Separated from `conch.rs` to stay within the 700-line module limit.

use conch_parser::ast;

// ---------------------------------------------------------------------------
// Word flattening: conch-parser word AST -> String
// ---------------------------------------------------------------------------

/// Flattens a `TopLevelWord` to a plain string.
pub(super) fn flatten_top_level_word(word: &ast::TopLevelWord<String>) -> String {
    flatten_complex_word(&word.0)
}

pub(super) type ConchComplexWord = ast::ComplexWord<ConchWord>;

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

pub(super) type ConchSimpleWord = ast::SimpleWord<
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

pub(super) type ConchWord = ast::Word<String, ConchSimpleWord>;

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

fn flatten_top_level_command_to_string(tlc: &ast::TopLevelCommand<String>) -> String {
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

    match &tlc.0 {
        ast::Command::List(and_or) | ast::Command::Job(and_or) => {
            walk_and_or_for_flatten(and_or, &mut collect);
        }
    }
    parts.join(" ")
}

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
// Redirect helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the redirect opens a writable file descriptor.
pub(super) fn is_output_redirect(redirect: &ast::Redirect<ast::TopLevelWord<String>>) -> bool {
    matches!(
        redirect,
        ast::Redirect::Write(..)
            | ast::Redirect::Append(..)
            | ast::Redirect::Clobber(..)
            | ast::Redirect::ReadWrite(..)
    )
}

/// Extracts the target word from a `Redirect` variant.
pub(super) fn extract_redirect_word(
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

// ---------------------------------------------------------------------------
// Command substitution extraction from AST words
// ---------------------------------------------------------------------------

/// Collects `ParameterSubstitution::Command` references found in a word.
pub(super) fn collect_command_substitutions_from_word(
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
        ast::Word::SingleQuoted(_) => {}
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
