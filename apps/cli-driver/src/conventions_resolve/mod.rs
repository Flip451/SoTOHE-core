//! Read-only `conventions resolve` command — primary adapter (spec `IN-05`,
//! `IN-06`, `AC-06`, `AC-08`, `AC-09`).
//!
//! The module holds the boundary mirror of the requested capability, the typed
//! input the command is dispatched with, and the driver that resolves through
//! the usecase service and renders what comes back. The mirror lives here
//! rather than beside the `capability exec` argument so that the driver
//! consuming it can read its validated value without that value becoming
//! reachable from anywhere else.
//!
//! The tests live in the `driver_tests` sibling so that only the production
//! half adds to this module's length.

#[cfg(test)]
mod driver_tests;

use std::path::PathBuf;
use std::sync::Arc;

use usecase::conventions_resolve::{
    ConventionCapabilityId, ConventionCapabilityIdError, ConventionResolution,
    ConventionResolveService, ResolveConventionsQuery,
};

use crate::render::CommandOutcome;

/// CLI-boundary mirror of a validated [`ConventionCapabilityId`] (`IN-05`,
/// `IN-06`, `AC-09`).
///
/// It is a sibling of `capability::CapabilityNameArg` rather than a reuse of
/// it: that mirror wraps the `agent-profiles.json` lookup key, whose
/// constructor trims its input, and is shared with `capability exec`. Matching
/// through a trimming key would make a padded identifier match an unpadded
/// one, so the query side of an exact-match comparison carries the matching
/// term instead.
///
/// The mirror exists only because the validated value cannot itself implement
/// clap's parsing trait. It holds that value in a private field and checks
/// nothing of its own: [`ConventionCapabilityId::try_new`] is the single
/// enforcing site, reached through the [`FromStr`](core::str::FromStr)
/// implementation below, so the non-blank invariant is established once while
/// clap is still parsing and is never re-decided further in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionCapabilityIdArg(ConventionCapabilityId);

impl core::str::FromStr for ConventionCapabilityIdArg {
    type Err = ConventionCapabilityIdError;

    /// Parses the argument by delegating to the validated value's constructor.
    ///
    /// Nothing is normalized here, so the identifier reaches the resolver
    /// spelled exactly as the caller typed it, and nothing is looked up, so an
    /// identifier registered in neither `.harness/capabilities/` nor
    /// `agent-profiles.json` parses like any other (`AC-09`).
    ///
    /// # Errors
    ///
    /// Returns [`ConventionCapabilityIdError::Blank`] for an empty or
    /// whitespace-only argument, which is the constructor's only rejection and
    /// therefore this boundary's only one.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ConventionCapabilityId::try_new(value).map(Self)
    }
}

/// Typed input for the read-only `conventions resolve` command (`IN-05`,
/// `IN-06`, `AC-06`, `AC-09`).
///
/// The capability is the already-parsed [`ConventionCapabilityIdArg`] rather
/// than an argument string, so the driver receives a value whose invariant is
/// established and has nothing left to re-validate. Field order mirrors
/// `cli::ConventionsCommand::Resolve` and
/// [`ResolveConventionsQuery`] exactly; the CLI variant is the fixed point of
/// that chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionResolveInput {
    /// Capability whose required convention documents are wanted.
    pub capability: ConventionCapabilityIdArg,
    /// Root of the project whose `knowledge/conventions/` tree is scanned.
    pub project_root: PathBuf,
}

/// Primary adapter for `conventions resolve` (`IN-05`, `AC-06`, `AC-08`).
///
/// The injected [`ConventionResolveService`] is the only thing it holds, so
/// reading is the only thing the command can do: there is no route from here
/// to creating, updating, deleting, or indexing a convention document
/// (`AC-06`).
pub struct ConventionResolveDriver {
    service: Arc<dyn ConventionResolveService>,
}

impl ConventionResolveDriver {
    /// Wires the driver to the service it resolves through.
    #[must_use]
    pub fn new(service: Arc<dyn ConventionResolveService>) -> Self {
        Self { service }
    }

    /// Resolves the requested capability's conventions and renders the answer.
    ///
    /// Both renderings are taken from the one returned
    /// [`ConventionResolution`], so the machine-readable result and the human
    /// display cannot disagree (`IN-05`): the display is handed the very string
    /// the canonical result is, not a second rendering of the same resolution.
    /// Only the resolver's error reaches the failure arm, which is what leaves a
    /// resolution matching no document on the success path (`AC-08`).
    ///
    /// A resolution matching no document carries no canonical result at all
    /// rather than an empty one, because the emitting `bin` writes a present
    /// stdout as a line: an empty string there would put a blank line on a
    /// stream whose lines are the answer, making a zero-match result read as
    /// one unnamed document (`IN-05`).
    #[must_use]
    pub fn handle(&self, input: ConventionResolveInput) -> CommandOutcome {
        let query = ResolveConventionsQuery {
            capability: input.capability.0,
            project_root: input.project_root,
        };
        match self.service.resolve(query) {
            Ok(resolution) => {
                let documents = render_documents(&resolution);
                let summary = render_summary(&resolution, &documents);
                CommandOutcome {
                    stdout: (!resolution.is_empty()).then_some(documents),
                    stderr: Some(summary),
                    exit_code: 0,
                }
            }
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

/// Renders the canonical result: one repository-relative document path per
/// line, in the order the resolution holds them (`IN-05`, `AC-06`).
///
/// Deduplication and ordering are the resolution's own guarantee; this
/// rendering adds neither and reorders nothing, so a matched document appears
/// once and the lines read in stable path order. That a document path occupies
/// exactly one line is likewise not established here: `ConventionDocumentPath`
/// admits only paths that are valid UTF-8 and hold no line terminator, which is
/// what makes joining them on `\n` a lossless one-record-per-line encoding
/// rather than a merely usual-case one. A resolution matching no document
/// renders as no line at all.
fn render_documents(resolution: &ConventionResolution) -> String {
    resolution.documents().iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
}

/// Renders the human-readable display of the same resolution (`IN-05`).
///
/// `documents` is the canonical result already rendered by `render_documents`,
/// listed here verbatim rather than assembled a second time, so the display
/// cannot present a different set, a different order, or a different count from
/// the canonical result. Matching no document is displayed as such rather than
/// as a problem (`AC-08`).
fn render_summary(resolution: &ConventionResolution, documents: &str) -> String {
    if resolution.is_empty() {
        return "no convention document declares this capability".to_owned();
    }
    format!(
        "{} convention document(s) declare this capability:\n{documents}",
        resolution.documents().len(),
    )
}
