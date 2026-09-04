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
use serde_json::{Map, Value};
use usecase::catalog_gen::{
    CatalogAddCommand, CatalogCheckQuery, CatalogCheckReport, CatalogCiteCommand, CatalogError,
    CatalogImportCommand, CatalogInitReport, CatalogPort, CatalogWriteReport,
};

use crate::tddd::catalogue_document_codec::{
    CatalogueDocumentCodec, CatalogueDocumentCodecError, SCHEMA_VERSION,
};

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
mod adapter_tests;
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

/// Validate the schema of a draft's hole-free portion.
///
/// Unlike [`try_complete`] — which rejects *any* draft that still contains a
/// `$todo` hole — this prunes every entry that carries a hole and decodes the
/// remainder. It lets a gate that tolerates holes (the commit gate maps a
/// residual hole to an interim result) still surface a real schema violation in a
/// *hole-free* entry (an invalid role, an unknown field, a `crate_name` mismatch)
/// that an unrelated hole would otherwise mask as an interim / exit-0 outcome.
///
/// A residual hole that cannot be pruned — a `$todo` at a top-level scalar field
/// rather than inside an entry — is treated as tolerated (`Ok(())`); the hole
/// itself is still reported by the caller and blocks at the stricter merge /
/// phase2 gates.
///
/// # Errors
///
/// Returns [`CatalogDraftError::Codec`] when a hole-free entry violates the
/// catalogue schema.
pub(crate) fn validate_hole_free_schema(
    value: &Value,
    expected_stem: &str,
) -> Result<(), CatalogDraftError> {
    let pruned = prune_hole_bearing_entries(value, expected_stem);
    match try_complete(pruned, expected_stem) {
        // Hole-free portion decodes cleanly, or a non-prunable root/top-level
        // hole remains (tolerated here; still reported and blocked at stricter
        // gates). Any deeper hole that survives pruning is treated as a missed
        // validation case and fails closed instead of masking schema errors.
        Ok(_) => Ok(()),
        Err(CatalogDraftError::Incomplete { holes })
            if holes.iter().all(is_root_or_top_level_hole) =>
        {
            Ok(())
        }
        Err(err @ CatalogDraftError::Incomplete { .. }) => Err(err),
        Err(err @ CatalogDraftError::Codec { .. }) => Err(err),
    }
}

/// Clone `value` and drop every entry (from the `types` / `traits` / `functions`
/// maps) and every `trait_impls` / `inherent_impls` element whose subtree carries
/// a `$todo` hole, so the remainder can be schema-decoded without a hole marker
/// aborting the parse at its position.
fn prune_hole_bearing_entries(value: &Value, expected_stem: &str) -> Value {
    const ENTRY_SECTIONS: [&str; 3] = ["types", "traits", "functions"];
    const IMPL_SECTIONS: [&str; 2] = ["trait_impls", "inherent_impls"];

    let mut pruned = value.clone();
    if let Value::Object(root) = &mut pruned {
        remove_root_todo_marker(root);
        replace_top_level_hole(root, "schema_version", Value::Number(SCHEMA_VERSION.into()));
        replace_top_level_hole(root, "crate_name", Value::String(expected_stem.to_owned()));
        replace_top_level_hole(root, "layer", Value::String(expected_stem.to_owned()));
        for section in ENTRY_SECTIONS {
            match root.get_mut(section) {
                Some(Value::Object(entries)) => {
                    entries.remove(TODO_KEY);
                    entries.retain(|_, entry| !subtree_contains_todo_key(entry));
                }
                Some(value) if subtree_contains_todo_key(value) => {
                    *value = Value::Object(Map::new());
                }
                _ => {}
            }
        }
        for section in IMPL_SECTIONS {
            match root.get_mut(section) {
                Some(Value::Array(items)) => {
                    items.retain(|item| !subtree_contains_todo_key(item));
                }
                Some(value) if subtree_contains_todo_key(value) => {
                    *value = Value::Array(vec![]);
                }
                _ => {}
            }
        }
        remove_unknown_top_level_holes(root);
    }
    pruned
}

fn remove_root_todo_marker(root: &mut Map<String, Value>) {
    if root.len() > 1 {
        root.remove(TODO_KEY);
    }
}

fn replace_top_level_hole(root: &mut Map<String, Value>, key: &str, replacement: Value) {
    if root.get(key).is_some_and(subtree_contains_todo_key) {
        root.insert(key.to_owned(), replacement);
    }
}

fn remove_unknown_top_level_holes(root: &mut Map<String, Value>) {
    const KNOWN_TOP_LEVEL_KEYS: [&str; 8] = [
        "schema_version",
        "crate_name",
        "layer",
        "types",
        "traits",
        "functions",
        "trait_impls",
        "inherent_impls",
    ];
    let keys_to_remove: Vec<String> = root
        .iter()
        .filter(|(key, value)| {
            !KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()) && subtree_contains_todo_key(value)
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in keys_to_remove {
        root.remove(&key);
    }
}

fn is_root_or_top_level_hole(hole: &DraftHole) -> bool {
    let path = hole.path().as_str();
    path == "(root)" || (!path.contains('.') && !path.contains('['))
}

/// Whether `value` carries the reserved `$todo` hole key anywhere in its subtree.
///
/// Distinct from [`scan_todo_holes`], which collects hole paths + instructions and
/// silently skips a hole whose path/instruction is unrepresentable: this is a
/// plain existence check, so a malformed hole still counts as hole-bearing for
/// pruning purposes.
fn subtree_contains_todo_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.contains_key(TODO_KEY) || map.values().any(subtree_contains_todo_key)
        }
        Value::Array(items) => items.iter().any(subtree_contains_todo_key),
        _ => false,
    }
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

#[cfg(test)]
type TestImportResolver = fn(&Path, &str) -> Result<import_shape::ImportedShape, CatalogError>;

#[cfg(test)]
thread_local! {
    /// Test-only import-shape resolver override. Kept outside the adapter so the
    /// production shape stays the declared unit struct; each test thread owns its
    /// own override.
    static IMPORT_RESOLVER_OVERRIDE: std::cell::Cell<Option<TestImportResolver>> =
        const { std::cell::Cell::new(None) };
}

impl FsCatalogAdapter {
    /// Construct a new [`FsCatalogAdapter`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[cfg(test)]
    fn with_import_resolver(resolver: TestImportResolver) -> Self {
        IMPORT_RESOLVER_OVERRIDE.with(|cell| cell.set(Some(resolver)));
        Self
    }

    #[cfg(test)]
    fn import_resolver_override() -> Option<TestImportResolver> {
        IMPORT_RESOLVER_OVERRIDE.with(std::cell::Cell::get)
    }
}

impl CatalogPort for FsCatalogAdapter {
    fn init(
        &self,
        track_id: &domain::TrackId,
        items_dir: &Path,
    ) -> Result<CatalogInitReport, CatalogError> {
        verb_init::run(track_id.as_ref(), items_dir)
    }

    fn add(
        &self,
        track_id: &domain::TrackId,
        items_dir: &Path,
        command: CatalogAddCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        verb_add::run(track_id.as_ref(), items_dir, command)
    }

    fn import(
        &self,
        track_id: &domain::TrackId,
        items_dir: &Path,
        command: CatalogImportCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        #[cfg(test)]
        if let Some(resolver) = Self::import_resolver_override() {
            return verb_import::run_with_resolver(track_id.as_ref(), items_dir, command, resolver);
        }
        verb_import::run(track_id.as_ref(), items_dir, command)
    }

    fn cite(
        &self,
        track_id: &domain::TrackId,
        items_dir: &Path,
        command: CatalogCiteCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        verb_cite::run(track_id.as_ref(), items_dir, command)
    }

    fn check(
        &self,
        track_id: &domain::TrackId,
        items_dir: &Path,
        query: CatalogCheckQuery,
    ) -> Result<CatalogCheckReport, CatalogError> {
        verb_check::run(track_id.as_ref(), items_dir, query)
    }
}
