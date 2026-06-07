//! Helper functions for building permission maps/sets and extracting name fragments.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

use super::constants::{
    EXPECTED_CARGO_MAKE_ALLOW, EXPECTED_GIT_ALLOW, EXPECTED_OTHER_ALLOW, EXTRA_CARGO_MAKE_ALLOW_RE,
    EXTRA_GIT_ALLOW_RE, FORBIDDEN_ALLOW,
};

// ---------------------------------------------------------------------------
// Helper: build expected allow map (BTreeMap for determinism)
// ---------------------------------------------------------------------------

pub(crate) fn expected_allow_map() -> BTreeMap<&'static str, &'static str> {
    let mut map = BTreeMap::new();
    for (k, v) in
        EXPECTED_OTHER_ALLOW.iter().chain(EXPECTED_GIT_ALLOW).chain(EXPECTED_CARGO_MAKE_ALLOW)
    {
        map.insert(*k, *v);
    }
    map
}

pub(crate) fn forbidden_allow_set() -> BTreeSet<&'static str> {
    FORBIDDEN_ALLOW.iter().copied().collect()
}

// ---------------------------------------------------------------------------
// Helper: extract cargo make task name / git subcommand from allow entries
// ---------------------------------------------------------------------------

/// Extract a named capture group from an allow entry using the given regex.
///
/// Returns `None` when `re` is `None`, the entry does not match, or the named
/// capture group is absent.
fn named_capture_from_entry(entry: &str, re: Option<&Regex>, capture_name: &str) -> Option<String> {
    let re = re?;
    let caps = re.captures(entry)?;
    let m = caps.name(capture_name)?;
    entry.get(m.start()..m.end()).map(ToOwned::to_owned)
}

/// Extract the task name from `Bash(cargo make <task>)` or `Bash(cargo make <task>:*)`.
///
/// Returns `None` when the entry does not match the pattern.
pub(crate) fn cargo_make_task_name(entry: &str) -> Option<String> {
    named_capture_from_entry(entry, EXTRA_CARGO_MAKE_ALLOW_RE.as_ref(), "task")
}

/// Extract the subcommand from `Bash(git <subcommand>)` or `Bash(git <subcommand>:*)`.
///
/// Returns `None` when the entry does not match the pattern.
pub(crate) fn git_subcommand_name(entry: &str) -> Option<String> {
    named_capture_from_entry(entry, EXTRA_GIT_ALLOW_RE.as_ref(), "subcommand")
}

/// Returns `true` when `entry` is a `Bash(...)` permission that directly invokes a repo script
/// under `scripts/` rather than going through a `cargo make` wrapper.
///
/// Both `verify_allowlist` and `validate_permission_extensions` share this policy rule.
pub(crate) fn is_direct_repo_script_permission(entry: &str) -> bool {
    entry.starts_with("Bash(") && entry.contains("scripts/")
}

/// All cargo make task names that are already reserved by baseline expected or forbidden lists.
pub(crate) fn known_cargo_make_tasks() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in
        EXPECTED_CARGO_MAKE_ALLOW.iter().map(|(k, _)| *k).chain(FORBIDDEN_ALLOW.iter().copied())
    {
        if let Some(task) = cargo_make_task_name(entry) {
            set.insert(task);
        }
    }
    set
}

/// All git subcommands that are already reserved by baseline expected or forbidden lists.
pub(crate) fn known_git_subcommands() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in EXPECTED_GIT_ALLOW.iter().map(|(k, _)| *k).chain(FORBIDDEN_ALLOW.iter().copied()) {
        if let Some(sub) = git_subcommand_name(entry) {
            set.insert(sub);
        }
    }
    set
}
