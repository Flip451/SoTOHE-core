//! Rendering of `plan.md` from `TrackMetadata` and an optional `ImplPlanDocument`.

use domain::{ImplPlanDocument, TrackMetadata};

/// Renders `plan.md` content from track identity metadata and an optional
/// `ImplPlanDocument`.
///
/// When `impl_plan` is `Some`, renders the full task list and plan sections
/// from the document. When `None`, emits a placeholder stub (used for
/// planning-only tracks that have not yet generated `impl-plan.json`).
#[must_use]
pub fn render_plan(track: &TrackMetadata, impl_plan: Option<&ImplPlanDocument>) -> String {
    let mut lines = Vec::new();
    lines.push(
        "<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->".to_owned(),
    );
    lines.push(format!("# {}", track.title()));
    lines.push(String::new());

    let Some(doc) = impl_plan else {
        lines.push(
            "> **Note**: `impl-plan.json` not yet generated. \
             Run `/track:impl-plan` to generate the implementation plan."
                .to_owned(),
        );
        lines.push(String::new());
        return lines.join("\n");
    };

    // Summary lines (if any).
    if !doc.plan().summary().is_empty() {
        lines.push("## Summary".to_owned());
        lines.push(String::new());
        for line in doc.plan().summary() {
            lines.push(line.clone());
        }
        lines.push(String::new());
    }

    // Task list per section.
    let total = doc.tasks().len();
    let done_count = doc.tasks().iter().filter(|t| t.status().is_resolved()).count();
    lines.push(format!("## Tasks ({done_count}/{total} resolved)"));
    lines.push(String::new());

    for section in doc.plan().sections() {
        lines.push(format!("### {} — {}", section.id(), section.title()));
        lines.push(String::new());
        if !section.description().is_empty() {
            for desc_line in section.description() {
                lines.push(format!("> {desc_line}"));
            }
            lines.push(String::new());
        }
        for task_id in section.task_ids() {
            if let Some(task) = doc.tasks().iter().find(|t| t.id() == task_id) {
                let status_label = match task.status() {
                    domain::TaskStatus::Todo => "[ ]",
                    domain::TaskStatus::InProgress => "[~]",
                    domain::TaskStatus::DonePending | domain::TaskStatus::DoneTraced { .. } => {
                        "[x]"
                    }
                    domain::TaskStatus::Skipped => "[-]",
                };
                let hash_note = match task.status() {
                    domain::TaskStatus::DoneTraced { commit_hash } => {
                        format!(" (`{}`)", commit_hash)
                    }
                    _ => String::new(),
                };
                lines.push(format!(
                    "- {} **{}**: {}{}",
                    status_label,
                    task_id,
                    task.description(),
                    hash_note
                ));
            }
        }
        lines.push(String::new());
    }

    lines.join("\n")
}
