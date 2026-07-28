//! Convention-shipping contracts for the exported template tree.
//!
//! This module owns the request naming the two trees the shipping check
//! compares, the verdict that check produces, the fail-closed conditions under
//! which it cannot produce one at all, the port that inventories a tree's
//! convention documents, and the pure comparison between two such inventories
//! (spec `IN-11`, `AC-18`).
//!
//! The document identity both trees are inventoried as is
//! [`ConventionDocumentPath`], owned by [`crate::conventions_resolve`] and
//! shared unchanged: a convention document is the same concept whether it was
//! found under the overlay or inside an export.

use std::path::{Path, PathBuf};

use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::NonEmptyVec;

use crate::conventions_resolve::{ConventionDocumentPath, ConventionDocumentPathError};

#[cfg(test)]
mod inventory_port_tests;
#[cfg(test)]
mod shipping_contract_tests;
#[cfg(test)]
mod unsupplied_selection_tests;

/// Read-only request naming the two trees the shipping check compares
/// (`IN-11`, `AC-18`).
///
/// Both fields stay [`PathBuf`]: a tree root is a caller-chosen filesystem
/// location with no invariant of its own, so it is the truly-opaque case the
/// kind-selection rule permits. The constrained concept in this slice is the
/// identity of a document *inside* a tree, and that is
/// [`ConventionDocumentPath`], not the root it was found under.
///
/// A separate request from `TemplateExportCommand` because it names an
/// already-produced tree plus its overlay, not the four inputs an export
/// consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckConventionShippingQuery {
    /// Root of the exported template tree whose shipped conventions are checked.
    pub exported_root: PathBuf,
    /// Overlay directory that supplies the conventions a consumer may receive.
    pub overlay_dir: PathBuf,
}

/// Outcome of the shipping comparison (`IN-11`, `AC-18`).
///
/// An enum rather than a report struct holding a possibly-empty list, so a
/// violating verdict that names no offending document is unrepresentable. The
/// violating variant therefore carries a [`NonEmptyVec`] — the workspace's
/// existing non-empty collection — rather than a second one minted here.
///
/// The offending documents travel as the typed paths themselves rather than as
/// a count, so no consumer can report the violation without being able to name
/// what caused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConventionShippingVerdict {
    /// The exported tree ships exactly what the overlay supplies.
    Conforming,
    /// The exported tree ships documents the overlay does not supply.
    UnsuppliedDocumentsShipped {
        /// The shipped documents with no overlay counterpart, at least one.
        documents: NonEmptyVec<ConventionDocumentPath>,
    },
}

/// Structural fail-closed conditions of the shipping check (`IN-11`, `AC-18`).
///
/// A detected violation is deliberately not among them: non-overlay documents
/// in the export are the check's *normal finding* and are carried by
/// [`ConventionShippingVerdict`], so this enum stays reserved for states in
/// which the question could not be answered at all.
///
/// Membership is not a count of the conditions the anchors describe: an enum is
/// closed and that category is not, so a variant may compose more than one
/// condition and a new condition need not add one.
#[derive(Debug, thiserror::Error)]
pub enum ConventionShippingCheckError {
    /// A tree that must hold conventions has no convention root at all.
    ///
    /// This variant exists so that an exported tree with no conventions
    /// directory fails rather than passing vacuously: that is exactly the shape
    /// the tree takes if the boundary classification is edited away from
    /// `overlay`, which is the mutation this check is here to catch.
    #[error("tree holds no convention root: {}", tree_root.display())]
    ConventionRootMissing {
        /// Root of the tree whose convention directory is absent.
        tree_root: PathBuf,
    },
    /// A path under one of the trees could not be read or listed.
    #[error("convention tree is unreadable at '{}': {reason}", path.display())]
    TreeUnreadable {
        /// Path the inventory walk could not read or list.
        path: PathBuf,
        /// Human-readable diagnostic describing the failure.
        reason: FreeText,
    },
    /// The inventory walk resolved a path that is not a convention document path.
    ///
    /// Composes [`ConventionDocumentPathError`] rather than the whole
    /// `ConventionResolveError`: this check reads no document contents, so the
    /// content-parse conditions are unreachable on this path and must not be
    /// representable in its error type.
    ///
    /// The rejection rule itself belongs to `ConventionDocumentPath::try_new`
    /// and is not restated here — the specific reason is read from the source.
    /// What the walk adds is `tree_root`, without which a rejection could not be
    /// attributed to the exported tree rather than the overlay.
    #[error("the inventory of '{}' resolved a path that is not a convention document path", tree_root.display())]
    DocumentPathRejected {
        /// Root of the tree the rejected path was inventoried from.
        tree_root: PathBuf,
        /// Constructor rejection this variant composes.
        source: ConventionDocumentPathError,
    },
}

/// Read-only driven port listing the convention documents a tree ships
/// (`IN-11`, `AC-18`).
///
/// The documents are identified tree-relative — [`ConventionDocumentPath`] is a
/// repository-relative path — so an inventory of the exported tree and one of
/// the overlay tree yield comparable values. Absolute paths would carry each
/// tree's root into the identity and leave
/// [`select_unsupplied_conventions`] comparing values that can never be equal.
///
/// Deliberately not the existing
/// [`ConventionRequirementPort`](crate::conventions_resolve::ConventionRequirementPort):
/// that port's contract is to decode `required_for` front matter and it fails
/// closed on malformed YAML, which would let an unrelated document defect
/// masquerade as a shipping violation. This check reads no document contents at
/// all, and [`ConventionShippingCheckError`] accordingly cannot name a
/// content-parse condition.
///
/// It lives in `usecase` rather than `domain` by R1's port tie-break: listing a
/// tree's documents is a technical capability the application orchestration
/// needs, not an invariant of a domain aggregate.
///
/// The method is synchronous by decision, not by omission. This workspace has
/// no ambient async runtime, the port is called twice in a short-lived CLI
/// process with nothing to overlap, and every sibling filesystem port in this
/// layer is synchronous, so an async color would force a runtime into the
/// composition root for concurrency no caller could use.
pub trait ConventionInventoryPort: Send + Sync {
    /// Lists the convention documents `tree_root` ships, each identified
    /// relative to that tree.
    ///
    /// The result is what the walk found, in the order it found it: making it
    /// canonical is [`select_unsupplied_conventions`]' decision, not an
    /// adapter's.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionShippingCheckError`] when the tree holds no
    /// convention root at all, when a path under it cannot be read or listed,
    /// or when the walk resolves a path that is not a convention document path.
    fn list_conventions(
        &self,
        tree_root: &Path,
    ) -> Result<Vec<ConventionDocumentPath>, ConventionShippingCheckError>;
}

/// Selects the shipped documents the overlay does not supply (`IN-11`,
/// `AC-18`).
///
/// A free function rather than an interactor method because it holds no state
/// and needs no injected dependency, which is also what makes the comparison
/// exercisable without either tree on disk: everything it decides on is already
/// in its two arguments. It sits at the same seam, and for the same reasons, as
/// [`select_required_conventions`](crate::conventions_resolve::select_required_conventions)
/// — inventorying documents on one side, choosing among them on the other.
///
/// The offending documents are deduplicated and placed in stable lexicographic
/// path order. The spec requires neither; both are committed to here so that a
/// failing smoke run names the same documents in the same order on every run,
/// and is therefore reproducible and diffable rather than dependent on the
/// order a directory walk happened to produce.
///
/// The comparison is deliberately one-directional. A document the overlay
/// supplies but the exported tree does not ship is outside this function's
/// contract: reporting it would be a second finding class
/// [`ConventionShippingVerdict`] has no variant for and that this track states
/// no rule about.
///
/// The return type is not a `Result`: an export shipping nothing beyond the
/// overlay's supply is [`ConventionShippingVerdict::Conforming`], not a
/// failure. Because the violating variant carries a [`NonEmptyVec`], that
/// verdict is returned exactly when the selection is empty — there is no
/// representation of "violating but naming no document" to fall into.
#[must_use]
pub fn select_unsupplied_conventions(
    shipped: &[ConventionDocumentPath],
    supplied: &[ConventionDocumentPath],
) -> ConventionShippingVerdict {
    let mut unsupplied: Vec<ConventionDocumentPath> =
        shipped.iter().filter(|&document| !supplied.contains(document)).cloned().collect();
    unsupplied.sort();
    unsupplied.dedup();

    let Ok(documents) = NonEmptyVec::try_new(unsupplied) else {
        return ConventionShippingVerdict::Conforming;
    };
    ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents }
}
