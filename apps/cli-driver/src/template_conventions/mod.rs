//! Read-only `template check-convention-shipping` command — primary adapter
//! (spec `IN-11`, `AC-18`).
//!
//! The module holds the typed input the command is dispatched with and the
//! driver that checks through the usecase service and renders what comes back.
//!
//! It is a sibling of [`crate::template_export`] rather than a second arm of
//! `TemplateDriver`: that driver is constructed around the export service, and
//! widening it would change the public constructor of the export surface for a
//! check that is driven by its own application service.
//!
//! The tests live in the `driver_tests` sibling so that only the production half
//! adds to this module's length.

#[cfg(test)]
mod driver_tests;

use std::path::PathBuf;
use std::sync::Arc;

use usecase::conventions_resolve::ConventionDocumentPath;
use usecase::template_conventions::{
    CheckConventionShippingQuery, ConventionShippingCheckService, ConventionShippingVerdict,
};

use crate::render::CommandOutcome;

/// Typed input for the read-only convention non-shipping check (`IN-11`,
/// `AC-18`).
///
/// Both fields stay [`PathBuf`]: a tree root is a caller-chosen filesystem
/// location with no invariant of its own, while the invariant that does matter —
/// staying inside `knowledge/conventions/` — belongs to the document paths the
/// inventory produces and is owned by [`ConventionDocumentPath`].
///
/// Field names and order mirror `cli::TemplateConventionShippingArgs` and
/// [`CheckConventionShippingQuery`] exactly, so the two roots cannot be
/// transposed while travelling inward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionShippingCheckInput {
    /// Root of the exported template tree whose shipped conventions are checked.
    pub exported_root: PathBuf,
    /// Overlay directory that supplies the conventions a consumer may receive.
    pub overlay_dir: PathBuf,
}

/// Primary adapter for the convention non-shipping check (`IN-11`, `AC-18`).
///
/// The injected [`ConventionShippingCheckService`] is the only thing it holds,
/// so checking is the only thing the command can do: there is no route from here
/// to exporting, writing, or repairing a tree.
pub struct ConventionShippingCheckDriver {
    service: Arc<dyn ConventionShippingCheckService>,
}

impl ConventionShippingCheckDriver {
    /// Wires the driver to the service it checks through.
    #[must_use]
    pub fn new(service: Arc<dyn ConventionShippingCheckService>) -> Self {
        Self { service }
    }

    /// Checks the tree `input` names and renders the answer.
    ///
    /// The three arms are the three answers the service can give, mapped and
    /// nothing else: a conforming verdict is a success, a violating verdict is a
    /// failure that names every offending document, and a condition that
    /// prevented an answer is a failure carrying that condition's own message.
    /// No rule of the check is decided here — which documents count as
    /// unsupplied, in what order they are named, and which states are
    /// unanswerable are all settled before the outcome arrives.
    ///
    /// A violating verdict has to leave with a nonzero exit code, because the
    /// only consumer of this command is a gate: rendering the offending
    /// documents while exiting 0 would report the violation to a reader and hide
    /// it from the smoke run that has to fail on it.
    #[must_use]
    pub fn handle(&self, input: ConventionShippingCheckInput) -> CommandOutcome {
        let query = CheckConventionShippingQuery {
            exported_root: input.exported_root,
            overlay_dir: input.overlay_dir,
        };
        match self.service.check(query) {
            Ok(ConventionShippingVerdict::Conforming) => {
                CommandOutcome::success(Some(CONFORMING_MESSAGE.to_owned()))
            }
            Ok(ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents }) => {
                CommandOutcome::failure(Some(render_unsupplied(documents.as_slice())))
            }
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

/// Rendering of the conforming verdict.
const CONFORMING_MESSAGE: &str =
    "convention shipping check: the exported tree ships no convention the overlay does not supply";

/// Renders a violation by naming every offending document (`IN-11`, `AC-18`).
///
/// Each unsupplied document appears on a line of its own and none is summarised
/// away, so a reader of the failure can act on it without a second run: the
/// verdict carries the offending paths rather than a count precisely so that no
/// consumer can report the violation without being able to name it, and a count
/// rendered here would discard at the last step the property the verdict type
/// was shaped to guarantee.
///
/// Deduplication and ordering are the verdict's own guarantee; this rendering
/// adds neither and reorders nothing, so a failing run names the same documents
/// in the same order every time. That a document path occupies exactly one line
/// is likewise not established here: [`ConventionDocumentPath`] admits only
/// paths that are valid UTF-8 and hold no line terminator, which is what makes
/// joining them on `\n` a lossless one-record-per-line encoding.
fn render_unsupplied(documents: &[ConventionDocumentPath]) -> String {
    let named = documents.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
    format!(
        "convention shipping check: the exported tree ships convention documents the overlay does \
         not supply:\n{named}"
    )
}
