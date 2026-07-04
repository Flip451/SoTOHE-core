//! Filesystem catalogue-generation adapter for the `sotp catalog` surface.
//!
//! This module hosts the infrastructure half of the "generate + annotate"
//! catalogue workflow (ADR 2026-07-02-1345):
//!
//! * the **draft layer** ([`scan_todo_holes`] / [`try_complete`] /
//!   [`CatalogDraftError`]) which sits in front of the typed catalogue DTO and
//!   locates `$todo` holes in a catalogue JSON tree (D4, IN-07, AC-05); and
//! * the [`FsCatalogAdapter`] secondary adapter implementing the usecase
//!   [`CatalogPort`] over on-disk `<layer>-types.json` files (IN-01..IN-06,
//!   AC-01..AC-12).
//!
//! The verb-level logic lives in the private `verb_*` submodules; the helper
//! layers (`fs_access`, `validate`, `fragment`, `json_build`, `import_shape`)
//! are shared across verbs. Catalogue schema authority (field layout,
//! `schema_version`, formatting, entry ordering) is centralised here rather than
//! in the writer prompt (CN-02).

use std::path::Path;

use domain::tddd::catalog_gen::{DraftHole, DraftHolePath, TodoInstruction};
use domain::tddd::catalogue_v2::CatalogueDocument;
use serde_json::Value;
use usecase::catalog_gen::{
    CatalogAddCommand, CatalogCheckQuery, CatalogCheckReport, CatalogCiteCommand, CatalogError,
    CatalogImportCommand, CatalogInitReport, CatalogPort, CatalogWriteReport,
};

use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, CatalogueDocumentCodecError};

mod fragment;
mod fs_access;
mod import_shape;
mod json_build;
mod validate;
mod verb_add;
mod verb_check;
mod verb_cite;
mod verb_import;
mod verb_init;

#[cfg(test)]
mod draft_tests;

// ---------------------------------------------------------------------------
// Draft layer (T005)
// ---------------------------------------------------------------------------

/// Reserved catalogue key marking an unfilled draft hole.
///
/// `$todo` is reserved across the whole catalogue schema and is never used as a
/// canonical field name (CN-01), so its presence anywhere in the JSON tree is a
/// deterministic hole marker.
pub(crate) const TODO_KEY: &str = "$todo";

/// Failure returned when a draft catalogue cannot be completed into a typed
/// [`CatalogueDocument`].
///
/// See IN-07, AC-05.
#[derive(Debug, thiserror::Error)]
pub enum CatalogDraftError {
    /// The draft still contains one or more `$todo` holes.
    #[error("draft catalogue has {} unfilled hole(s)", holes.len())]
    Incomplete {
        /// The remaining holes, each with its path and fill-in instruction.
        holes: Vec<DraftHole>,
    },
    /// A hole-free draft failed to decode into a typed [`CatalogueDocument`].
    #[error("catalogue codec error: {source}")]
    Codec {
        /// The underlying codec failure.
        #[from]
        source: CatalogueDocumentCodecError,
    },
}

/// Walk `value` as a JSON tree and collect every `$todo` hole with its path.
///
/// The path uses dotted object keys and bracketed array indices
/// (e.g. `types.Foo.methods[0].returns`). A hole node is any object carrying a
/// `$todo` key; the walk does not descend past it.
///
/// See IN-07, AC-05.
#[must_use]
pub fn scan_todo_holes(value: &Value) -> Vec<DraftHole> {
    let mut holes = Vec::new();
    collect_holes(value, "", &mut holes);
    holes
}

fn collect_holes(value: &Value, path: &str, out: &mut Vec<DraftHole>) {
    match value {
        Value::Object(map) => {
            if let Some(todo_value) = map.get(TODO_KEY) {
                let instruction = todo_value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or("(unspecified)");
                push_hole(path, instruction, out);
                return;
            }
            for (key, child) in map {
                let child_path =
                    if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                collect_holes(child, &child_path, out);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_holes(child, &format!("{path}[{index}]"), out);
            }
        }
        _ => {}
    }
}

fn push_hole(path: &str, instruction: &str, out: &mut Vec<DraftHole>) {
    let path_text = if path.is_empty() { "(root)" } else { path };
    if let (Ok(hole_path), Ok(hole_instruction)) = (
        DraftHolePath::try_new(path_text.to_owned()),
        TodoInstruction::try_new(instruction.to_owned()),
    ) {
        out.push(DraftHole::new(hole_path, hole_instruction));
    }
}

/// Complete a draft `value` into a typed [`CatalogueDocument`].
///
/// Returns [`CatalogDraftError::Incomplete`] (with the hole paths + instructions)
/// while any `$todo` remains, and otherwise decodes the JSON via the shared
/// catalogue codec, surfacing schema violations as [`CatalogDraftError::Codec`].
///
/// `expected_stem` is the crate name the caller derived from the catalogue's
/// filename (e.g. `"domain"` for `domain-types.json`); the decode step validates
/// the JSON `crate_name` field against it. Deriving it from the untrusted
/// `crate_name` field would make that check a tautology and let a tampered
/// catalogue decode where the canonical `CatalogueDocumentCodec::load` path
/// would reject it.
///
/// See IN-07, AC-05.
///
/// # Errors
///
/// Returns [`CatalogDraftError::Incomplete`] when holes remain, or
/// [`CatalogDraftError::Codec`] when the hole-free draft violates the schema
/// (including a `crate_name` that disagrees with `expected_stem`).
pub fn try_complete(
    value: Value,
    expected_stem: &str,
) -> Result<CatalogueDocument, CatalogDraftError> {
    let holes = scan_todo_holes(&value);
    if !holes.is_empty() {
        return Err(CatalogDraftError::Incomplete { holes });
    }
    let json = serde_json::to_string(&value).map_err(CatalogueDocumentCodecError::from)?;
    let document = CatalogueDocumentCodec::decode(&json, expected_stem)?;
    Ok(document)
}

// ---------------------------------------------------------------------------
// Filesystem adapter (T006 / T007)
// ---------------------------------------------------------------------------

/// Filesystem secondary adapter implementing [`CatalogPort`] over the track's
/// per-layer `<layer>-types.json` catalogue files.
///
/// See IN-01, IN-03, IN-04, IN-05, AC-01, AC-04, AC-12.
#[derive(Debug, Default)]
pub struct FsCatalogAdapter;

impl FsCatalogAdapter {
    /// Construct a new [`FsCatalogAdapter`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CatalogPort for FsCatalogAdapter {
    fn init(&self, track_id: &str, items_dir: &Path) -> Result<CatalogInitReport, CatalogError> {
        verb_init::run(track_id, items_dir)
    }

    fn add(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogAddCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        verb_add::run(track_id, items_dir, command)
    }

    fn import(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogImportCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        verb_import::run(track_id, items_dir, command)
    }

    fn cite(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogCiteCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        verb_cite::run(track_id, items_dir, command)
    }

    fn check(
        &self,
        track_id: &str,
        items_dir: &Path,
        query: CatalogCheckQuery,
    ) -> Result<CatalogCheckReport, CatalogError> {
        verb_check::run(track_id, items_dir, query)
    }
}
