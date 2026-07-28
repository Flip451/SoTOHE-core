//! Convention-shipping contracts for the exported template tree.
//!
//! This module owns the request naming the two trees the shipping check
//! compares, the verdict that check produces, and the fail-closed conditions
//! under which it cannot produce one at all (spec `IN-11`, `AC-18`).
//!
//! The document identity both trees are inventoried as is
//! [`ConventionDocumentPath`], owned by [`crate::conventions_resolve`] and
//! shared unchanged: a convention document is the same concept whether it was
//! found under the overlay or inside an export.

use std::path::PathBuf;

use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::NonEmptyVec;

use crate::conventions_resolve::{ConventionDocumentPath, ConventionDocumentPathError};

#[cfg(test)]
mod shipping_contract_tests;

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
