const DRY_FIX_SENTINEL_PREFIX: &str = "DRY_FIX_STATUS: ";

pub(super) fn parse_dry_fix_sentinel(output: &str) -> Option<&'static str> {
    let last_line = output.lines().rev().find(|line| !line.trim().is_empty())?;
    if let Some(status) = last_line.strip_prefix(DRY_FIX_SENTINEL_PREFIX) {
        match status {
            "completed" => Some("completed"),
            "blocked" => Some("blocked"),
            "failed" => Some("failed"),
            _ => None,
        }
    } else {
        None
    }
}

pub(super) fn dry_fix_sentinel_to_exit_code(status: &str) -> i32 {
    match status {
        "completed" => 0,
        "blocked" => 2,
        _ => 1,
    }
}
