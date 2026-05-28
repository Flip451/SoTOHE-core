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
    // New form: task_id and status without positional track_dir/track-id (D6).
    let args = vec!["T001 done".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["T001", "done"]);
}

#[test]
fn test_raw_args_to_words_multiple_elements_already_split() {
    // New form: task_id and status passed as separate args (D6).
    let args = vec!["T001".to_owned(), "done".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["T001", "done"]);
}

#[test]
fn test_raw_args_to_words_empty() {
    let args: Vec<String> = vec![];
    let result: Vec<String> = raw_args_to_words(&args);
    assert!(result.is_empty());
}

#[test]
fn test_raw_args_to_words_with_extra_flags() {
    // New form: task_id, status, and --commit-hash flag (no positional track_dir, D6).
    let args = vec!["T001 done --commit-hash abc123".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["T001", "done", "--commit-hash", "abc123"]);
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
    // Direct CLI: bin/sotp make track-add-task "fix parser bug" --section S1
    // Shell passes two already-split args; multi-element path returns them as-is
    // (D6: track-id is omitted, description and flags are separate args).
    let args = vec!["fix parser bug".to_owned(), "--section".to_owned(), "S1".to_owned()];
    assert_eq!(raw_args_to_words(&args), vec!["fix parser bug", "--section", "S1"]);
}

// --- dispatch argv construction tests (D6 passthrough, AC-17) ---
// These verify the argv vectors that dispatch_track_* would pass to run_sotp,
// without spawning a subprocess. We test build_forwarded_args directly because
// the dispatch functions delegate to it.

#[test]
fn test_dispatch_track_transition_passthrough_no_track_id() {
    // cargo make track-transition -- T001 done
    // Expected argv: track transition --items-dir track/items T001 done
    let raw = vec!["T001 done".to_owned()];
    let args = build_forwarded_args(&["track", "transition", "--items-dir", "track/items"], &raw);
    assert_eq!(args, vec!["track", "transition", "--items-dir", "track/items", "T001", "done"]);
}

#[test]
fn test_dispatch_track_transition_passthrough_with_track_id() {
    // cargo make track-transition -- --track-id my-track T001 done
    // Expected argv: track transition --items-dir track/items --track-id my-track T001 done
    let raw = vec!["--track-id my-track T001 done".to_owned()];
    let args = build_forwarded_args(&["track", "transition", "--items-dir", "track/items"], &raw);
    assert_eq!(
        args,
        vec![
            "track",
            "transition",
            "--items-dir",
            "track/items",
            "--track-id",
            "my-track",
            "T001",
            "done"
        ]
    );
}

#[test]
fn test_dispatch_track_transition_passthrough_with_commit_hash() {
    // cargo make track-transition -- T001 done --commit-hash abc123
    // Expected argv: track transition --items-dir track/items T001 done --commit-hash abc123
    let raw = vec!["T001 done --commit-hash abc123".to_owned()];
    let args = build_forwarded_args(&["track", "transition", "--items-dir", "track/items"], &raw);
    assert_eq!(
        args,
        vec![
            "track",
            "transition",
            "--items-dir",
            "track/items",
            "T001",
            "done",
            "--commit-hash",
            "abc123"
        ]
    );
}

#[test]
fn test_dispatch_track_add_task_passthrough_no_track_id() {
    // cargo make track-add-task -- "write docs"
    // Expected argv: track add-task --items-dir track/items write docs
    // (Note: multi-word description splits due to cargo-make ${@} single-string limitation)
    let raw = vec!["write docs".to_owned()];
    let args = build_forwarded_args(&["track", "add-task", "--items-dir", "track/items"], &raw);
    assert_eq!(args, vec!["track", "add-task", "--items-dir", "track/items", "write", "docs"]);
}

#[test]
fn test_dispatch_track_next_task_passthrough_empty() {
    // cargo make track-next-task (no args: self-resolve from current branch, D1)
    // Expected argv: track next-task --items-dir track/items
    let raw: Vec<String> = vec![];
    let args = build_forwarded_args(&["track", "next-task", "--items-dir", "track/items"], &raw);
    assert_eq!(args, vec!["track", "next-task", "--items-dir", "track/items"]);
}

#[test]
fn test_dispatch_track_task_counts_passthrough_empty() {
    // cargo make track-task-counts (no args: self-resolve from current branch, D1)
    // Expected argv: track task-counts --items-dir track/items
    let raw: Vec<String> = vec![];
    let args = build_forwarded_args(&["track", "task-counts", "--items-dir", "track/items"], &raw);
    assert_eq!(args, vec!["track", "task-counts", "--items-dir", "track/items"]);
}

// --- build_set_override_args tests (dispatch_track_set_override routing, AC-17) ---

#[test]
fn test_build_set_override_args_blocked_no_flags() {
    // cargo make track-set-override -- blocked
    // Expected argv: track set-override --items-dir track/items blocked
    let raw = vec!["blocked".to_owned()];
    let args = build_set_override_args(&raw).unwrap();
    assert_eq!(args, vec!["track", "set-override", "--items-dir", "track/items", "blocked"]);
}

#[test]
fn test_build_set_override_args_clear_routes_to_clear_override() {
    // cargo make track-set-override -- clear
    // Expected argv: track clear-override --items-dir track/items
    let raw = vec!["clear".to_owned()];
    let args = build_set_override_args(&raw).unwrap();
    assert_eq!(args, vec!["track", "clear-override", "--items-dir", "track/items"]);
}

#[test]
fn test_build_set_override_args_status_after_flags() {
    // Flags before status: --track-id my-track blocked
    // First non-flag word is "blocked" even though --track-id precedes it.
    let raw = vec!["--track-id my-track blocked".to_owned()];
    let args = build_set_override_args(&raw).unwrap();
    assert_eq!(
        args,
        vec![
            "track",
            "set-override",
            "--items-dir",
            "track/items",
            "blocked",
            "--track-id",
            "my-track"
        ]
    );
}

#[test]
fn test_build_set_override_args_reason_with_same_word_as_status_not_dropped() {
    // --reason blocked should not be silently stripped when status is also "blocked".
    // Status word removed by index (0-th non-flag), not by value.
    let raw = vec!["blocked --reason blocked".to_owned()];
    let args = build_set_override_args(&raw).unwrap();
    assert_eq!(
        args,
        vec![
            "track",
            "set-override",
            "--items-dir",
            "track/items",
            "blocked",
            "--reason",
            "blocked"
        ]
    );
}

#[test]
fn test_build_set_override_args_clear_with_track_id() {
    // cargo make track-set-override -- clear --track-id my-track
    // Expected argv: track clear-override --items-dir track/items --track-id my-track
    let raw = vec!["clear --track-id my-track".to_owned()];
    let args = build_set_override_args(&raw).unwrap();
    assert_eq!(
        args,
        vec!["track", "clear-override", "--items-dir", "track/items", "--track-id", "my-track"]
    );
}

#[test]
fn test_build_set_override_args_missing_status_returns_error() {
    // No positional word provided — only flags.
    let raw = vec!["--track-id my-track".to_owned()];
    assert!(build_set_override_args(&raw).is_err());
}

#[test]
fn test_build_set_override_args_boolean_flag_before_status() {
    // An unknown boolean flag (e.g. --verbose) must not consume the next token as its value.
    // cargo make track-set-override -- --verbose blocked
    // Expected argv: track set-override --items-dir track/items blocked --verbose
    let raw = vec!["--verbose blocked".to_owned()];
    let args = build_set_override_args(&raw).unwrap();
    assert_eq!(
        args,
        vec!["track", "set-override", "--items-dir", "track/items", "blocked", "--verbose"]
    );
}
