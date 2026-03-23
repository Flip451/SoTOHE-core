// Text module uses bounded array indexing extensively for character-by-character
// tokenization where loop conditions already guarantee in-bounds access.
#![allow(clippy::indexing_slicing)]

//! Hand-written shell text utilities (no external parser dependency).
//!
//! Provides quote-aware tokenization and command substitution extraction
//! as pure string-level operations.

use super::verdict::ParseError;

/// Maximum nesting depth for command substitution extraction.
const MAX_NESTING_DEPTH: usize = 16;

/// Tokenizes a single simple command string into an argv-like word list.
///
/// Handles single quotes, double quotes, and backslash escaping.
/// Does NOT split on control operators -- use [`super::ShellParser::split_shell`] first.
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

/// Extracts nested command strings from `$(...)` and backtick substitutions
/// found in the given input. Returns them as raw strings for recursive parsing.
///
/// # Errors
///
/// Returns `ParseError` on nesting depth exceeded or parse errors.
pub fn extract_command_substitutions(input: &str) -> Result<Vec<String>, ParseError> {
    extract_substitutions_inner(input, 0)
}

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
