//! Tests for [`make`] (split out to keep the main module under the 200-400 line guideline).

use super::*;

// --- raw_args_to_single tests ---

#[test]
fn test_raw_args_to_single_with_single_element() {
    let args = vec!["my-track-id".to_owned()];
    assert_eq!(raw_args_to_single(&args).unwrap(), "my-track-id");
}

#[test]
fn test_raw_args_to_single_with_spaced_string() {
    let args = vec!["commit message with spaces".to_owned()];
    assert_eq!(raw_args_to_single(&args).unwrap(), "commit message with spaces");
}

#[test]
fn test_raw_args_to_single_with_multiple_elements() {
    let args = vec!["part1".to_owned(), "part2".to_owned()];
    assert_eq!(raw_args_to_single(&args).unwrap(), "part1 part2");
}

#[test]
fn test_raw_args_to_single_empty_returns_error() {
    let args: Vec<String> = vec![];
    assert!(raw_args_to_single(&args).is_err());
}

#[test]
fn test_raw_args_to_single_whitespace_only_returns_error() {
    let args = vec!["  ".to_owned()];
    assert!(raw_args_to_single(&args).is_err());
}

// --- raw_args_to_words tests ---

#[test]
fn test_raw_args_to_words_single_element() {
    let args = vec!["my-id".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["my-id"]);
}

#[test]
fn test_raw_args_to_words_splits_single_string() {
    let args = vec!["track/items/xxx T001 done".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["track/items/xxx", "T001", "done"]);
}

#[test]
fn test_raw_args_to_words_multiple_elements_already_split() {
    let args = vec!["track/items/xxx".to_owned(), "T001".to_owned(), "done".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["track/items/xxx", "T001", "done"]);
}

#[test]
fn test_raw_args_to_words_empty() {
    let args: Vec<String> = vec![];
    let result: Vec<String> = raw_args_to_words(&args);
    assert!(result.is_empty());
}

#[test]
fn test_raw_args_to_words_with_extra_flags() {
    let args = vec!["track/items/xxx T001 done --commit-hash abc123".to_owned()];
    assert_eq!(
        raw_args_to_words(&args),
        vec!["track/items/xxx", "T001", "done", "--commit-hash", "abc123"]
    );
}

// --- build_forwarded_args tests ---

#[test]
fn test_build_forwarded_args_prepends_prefix() {
    let raw = vec!["--track-id my-track --round-type fast".to_owned()];
    let args = build_forwarded_args(&["review", "record-round"], &raw);
    assert_eq!(args[0], "review");
    assert_eq!(args[1], "record-round");
    assert_eq!(args[2], "--track-id");
}

#[test]
fn test_build_forwarded_args_strips_leading_double_dash() {
    let raw = vec!["-- --track-id my-track".to_owned()];
    let args = build_forwarded_args(&["review", "check-approved"], &raw);
    assert_eq!(args, vec!["review", "check-approved", "--track-id", "my-track"]);
}

#[test]
fn test_build_forwarded_args_empty_raw() {
    let raw: Vec<String> = vec![];
    let args = build_forwarded_args(&["review", "check-approved"], &raw);
    assert_eq!(args, vec!["review", "check-approved"]);
}

#[test]
fn test_raw_args_to_words_preserves_quoting_in_direct_call() {
    // Direct CLI: bin/sotp make track-add-task track-1 "fix parser bug"
    // Shell splits into two args, preserving the quoted group
    let args = vec!["track-1".to_owned(), "fix parser bug".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["track-1", "fix parser bug"]);
}
