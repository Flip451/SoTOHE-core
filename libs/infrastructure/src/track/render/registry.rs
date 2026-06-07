//! Rendering of `registry.md` from all track snapshots.

use super::snapshot::TrackSnapshot;

/// Parses a track status string into `domain::TrackStatus`.
/// Returns `TrackStatus::Planned` for unrecognized values.
pub(super) fn parse_track_status_str(s: &str) -> domain::TrackStatus {
    match s {
        "planned" => domain::TrackStatus::Planned,
        "in_progress" => domain::TrackStatus::InProgress,
        "done" => domain::TrackStatus::Done,
        "blocked" => domain::TrackStatus::Blocked,
        "cancelled" => domain::TrackStatus::Cancelled,
        "archived" => domain::TrackStatus::Archived,
        _ => domain::TrackStatus::Planned,
    }
}

pub(super) fn format_date(iso_timestamp: &str) -> &str {
    if iso_timestamp.len() >= 10 { &iso_timestamp[..10] } else { iso_timestamp }
}

pub(super) fn next_command_for_track(track: &TrackSnapshot) -> String {
    // Status is derived and cached in `derived_status`; use `resolve_phase_from_record`
    // to avoid re-loading impl-plan.json. Parse the status string into `TrackStatus`.
    let status = parse_track_status_str(track.derived_status.as_str());
    let override_reason = track.track.status_override().map(|o| o.reason()).filter(|_| {
        matches!(status, domain::TrackStatus::Blocked | domain::TrackStatus::Cancelled)
    });
    let info = domain::track_phase::resolve_phase_from_record(status, override_reason);
    format!("`{}`", info.next_command)
}

/// Renders `registry.md` content from all track snapshots.
#[must_use]
pub fn render_registry(tracks: &[TrackSnapshot]) -> String {
    let mut active: Vec<_> = tracks
        .iter()
        .filter(|track| {
            matches!(track.status().as_ref(), "planned" | "in_progress" | "blocked" | "cancelled")
        })
        .collect();
    // Sort active tracks so in-progress tracks precede planned ones.
    active.sort_by_key(|track| track.status() == "planned");
    let completed: Vec<_> = tracks.iter().filter(|track| track.status() == "done").collect();
    let archived: Vec<_> = tracks.iter().filter(|track| track.status() == "archived").collect();

    let mut lines = vec![
        "# Track Registry".to_owned(),
        String::new(),
        "> This file lists all tracks and their current status.".to_owned(),
        "> Auto-updated by `/track:plan` and `/track:commit`.".to_owned(),
        "> `/track:status` uses this file as an entry point to summarize progress.".to_owned(),
        "> Each track is expected to have `spec.md` (or `spec.json`) / `plan.md` / `metadata.json`; `observations.md` is optional.".to_owned(),
        String::new(),
        "## Current Focus".to_owned(),
        String::new(),
    ];

    if let Some(latest) = active.first() {
        lines.push(format!("- Latest active track: `{}`", latest.track.id()));
        lines.push(format!("- Next recommended command: {}", next_command_for_track(latest)));
        lines.push(format!("- Last updated: `{}`", format_date(latest.updated_at())));
    } else {
        lines.push("- Latest active track: `None yet`".to_owned());
        lines.push("- Next recommended command: `/track:plan <feature>`".to_owned());
        if let Some(latest) = tracks.first() {
            lines.push(format!("- Last updated: `{}`", format_date(latest.updated_at())));
        } else {
            lines.push("- Last updated: `YYYY-MM-DD`".to_owned());
        }
    }
    lines.push(String::new());

    lines.push("## Active Tracks".to_owned());
    lines.push(String::new());
    lines.push("| Track | Status | Next | Updated |".to_owned());
    lines.push("|------|--------|------|---------|".to_owned());
    if active.is_empty() {
        lines.push("| _No active tracks yet_ | - | `/track:plan <feature>` | - |".to_owned());
    } else {
        for track in &active {
            let status = track.status();
            lines.push(format!(
                "| {} | {} | {} | {} |",
                track.track.id(),
                status,
                next_command_for_track(track),
                format_date(track.updated_at())
            ));
        }
    }
    lines.push(String::new());

    lines.push("## Completed Tracks".to_owned());
    lines.push(String::new());
    lines.push("| Track | Result | Updated |".to_owned());
    lines.push("|------|--------|---------|".to_owned());
    if completed.is_empty() {
        lines.push("| _No completed tracks yet_ | - | - |".to_owned());
    } else {
        for track in &completed {
            lines.push(format!(
                "| {} | Done | {} |",
                track.track.id(),
                format_date(track.updated_at())
            ));
        }
    }
    lines.push(String::new());

    lines.push("## Archived Tracks".to_owned());
    lines.push(String::new());
    lines.push("| Track | Result | Archived |".to_owned());
    lines.push("|------|--------|----------|".to_owned());
    if archived.is_empty() {
        lines.push("| _No archived tracks yet_ | - | - |".to_owned());
    } else {
        for track in &archived {
            lines.push(format!(
                "| {} | Archived | {} |",
                track.track.id(),
                format_date(track.updated_at())
            ));
        }
    }
    lines.push(String::new());
    lines.push("---".to_owned());
    lines.push(String::new());
    lines.push("Use `/track:plan <feature>` to start a new track.".to_owned());
    lines.push(String::new());

    lines.join("\n")
}
