//! Review-fix dispatch helpers for `sotp make track-local-review-fix-codex`.
//!
//! Extracted from `make.rs` to keep that module under the 700-line production-code
//! cap. Declared via `#[path = "make_review_fix.rs"]` inside `make.rs`; uses
//! `super::` to access `raw_args_to_words` and `run_sotp` from the parent module.

use std::process::ExitCode;

use crate::CliError;

pub(super) fn dispatch_track_local_review_fix_codex(
    raw_args: &[String],
) -> Result<ExitCode, CliError> {
    let args = build_track_local_review_fix_codex_args(raw_args)?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    super::run_sotp(&refs)
}

pub(super) fn build_track_local_review_fix_codex_args(
    raw_args: &[String],
) -> Result<Vec<String>, CliError> {
    let words = raw_args_to_shell_words(raw_args)?;
    let filtered: Vec<&str> = words.iter().map(String::as_str).skip_while(|s| *s == "--").collect();
    let mut args: Vec<String> = vec!["review".to_owned(), "fix-local".to_owned()];
    args.extend(filtered.iter().map(|s| (*s).to_owned()));
    Ok(args)
}

fn raw_args_to_shell_words(raw_args: &[String]) -> Result<Vec<String>, CliError> {
    if raw_args.len() == 1 {
        let single = raw_args
            .first()
            .ok_or_else(|| CliError::Message("internal error: missing raw argument".to_owned()))?;
        split_shell_words(single)
    } else {
        Ok(raw_args.to_vec())
    }
}

fn split_shell_words(input: &str) -> Result<Vec<String>, CliError> {
    #[derive(Clone, Copy)]
    enum Quote {
        Single,
        Double,
    }

    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<Quote> = None;
    let mut in_word = false;
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        match quote {
            None if ch.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            None if ch == '\'' => {
                quote = Some(Quote::Single);
                in_word = true;
            }
            None if ch == '"' => {
                quote = Some(Quote::Double);
                in_word = true;
            }
            None if ch == '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push(ch);
                }
                in_word = true;
            }
            None => {
                current.push(ch);
                in_word = true;
            }
            Some(Quote::Single) if ch == '\'' => {
                quote = None;
            }
            Some(Quote::Single) => {
                current.push(ch);
            }
            Some(Quote::Double) if ch == '"' => {
                quote = None;
            }
            Some(Quote::Double) if ch == '\\' => {
                if let Some(next) = chars.next() {
                    if matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                        current.push(next);
                    } else {
                        current.push(ch);
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
            Some(Quote::Double) => {
                current.push(ch);
            }
        }
    }

    if quote.is_some() {
        return Err(CliError::Message("error: unterminated quoted argument".to_owned()));
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_track_local_review_fix_codex_args_preserves_quoted_paths() {
        let raw = vec![
            "--scope cli --briefing-file \"tmp/reviewer runtime/briefing cli.md\" \
             --track-id review-fix --round-type fast"
                .to_owned(),
        ];

        let args = build_track_local_review_fix_codex_args(&raw).unwrap();

        assert_eq!(
            args,
            vec![
                "review",
                "fix-local",
                "--scope",
                "cli",
                "--briefing-file",
                "tmp/reviewer runtime/briefing cli.md",
                "--track-id",
                "review-fix",
                "--round-type",
                "fast",
            ]
        );
    }

    #[test]
    fn test_build_track_local_review_fix_codex_args_preserves_direct_args() {
        let raw = vec![
            "--".to_owned(),
            "--scope".to_owned(),
            "cli".to_owned(),
            "--briefing-file".to_owned(),
            "tmp/reviewer runtime/briefing cli.md".to_owned(),
        ];

        let args = build_track_local_review_fix_codex_args(&raw).unwrap();

        assert_eq!(
            args,
            vec![
                "review",
                "fix-local",
                "--scope",
                "cli",
                "--briefing-file",
                "tmp/reviewer runtime/briefing cli.md"
            ]
        );
    }

    #[test]
    fn test_build_track_local_review_fix_codex_args_rejects_unclosed_quote() {
        let raw = vec!["--scope cli --briefing-file \"tmp/reviewer runtime/briefing.md".to_owned()];

        assert!(build_track_local_review_fix_codex_args(&raw).is_err());
    }

    #[test]
    fn test_build_track_local_review_fix_codex_args_preserves_backslash_in_double_quotes() {
        let raw = vec!["--briefing-file \"tmp/a\\b.md\"".to_owned()];

        let args = build_track_local_review_fix_codex_args(&raw).unwrap();

        assert_eq!(args, vec!["review", "fix-local", "--briefing-file", "tmp/a\\b.md"]);
    }
}
