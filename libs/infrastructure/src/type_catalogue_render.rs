//! Renderer for the per-layer type catalogue markdown view (e.g.
//! `domain-types.md`, a read-only view of the v3 `CatalogueDocument`).
//!
//! Produces a markdown file with:
//! - A generated-view header comment
//! - Per-kind section headers with per-section tables: Name | Kind | Action | Details | Signal
//!
//! Section order (non-empty sections only):
//! `## Typestates`, `## Enums`, `## Value Objects`, `## Error Types`,
//! `## Secondary Ports`, `## Application Services`, `## Use Cases`,
//! `## Interactors`, `## DTOs`, `## Commands`, `## Queries`, `## Factories`
//!
//! The Details column summarises kind-specific payload:
//! - Typestate: `→ A, → B` (declared transitions)
//! - Enum / ErrorType: `A | B | C` (expected variants)
//! - SecondaryPort / ApplicationService: `fn a, fn b` (expected methods)
//! - SecondaryAdapter: `impl Trait1, impl Trait2` (declared trait impls)
//! - ValueObject / UseCase / Interactor / Dto / Command / Query / Factory: `—`
//!
//! T002: `TraitPort` removed; `SecondaryPort` and `ApplicationService` added
//! (method-list details). Seven existence-check-only variants added with `—`
//! details. Section header `## Trait Ports` renamed to `## Secondary Ports`.
//! New section headers: `## Application Services`, `## Use Cases`,
//! `## Interactors`, `## DTOs`, `## Commands`, `## Queries`, `## Factories`.
//!
//! The Signal column shows `🔵` / `🔴` / `—` (no signal yet).
//!
//! T020 (ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D2.5 /
//! IN-17): when a `<layer>-catalogue-spec-signals.json` document is supplied
//! via the `catalogue_spec_signals` parameter of either `render_type_catalogue`
//! (v2) or `render_type_catalogue_v3` (v3), an additional `Cat-Spec` column is
//! appended to the per-section table — showing the SoT Chain ② catalogue-spec
//! grounding signal alongside the existing SoT Chain ③ type→implementation
//! `Signal`. When the parameter is `None` (layer not yet opted in, or signals
//! file absent/stale), the existing 5-column layout is preserved unchanged.

use std::path::{Path, PathBuf};

use domain::tddd::catalogue_v2::{
    CatalogueDocument, ContractRole, DataRole, FunctionRole, TypeKindV2,
};
use domain::{CatalogueSpecSignalsDocument, ConfidenceSignal, TypeSignal};
use thiserror::Error;

use crate::tddd::{catalogue_spec_signals_codec, type_signals_codec};

#[path = "type_catalogue_render/entry_details.rs"]
mod entry_details;
use entry_details::{v3_function_entry_details, v3_trait_entry_details, v3_type_entry_details};

/// Failure modes when loading a `<layer>-catalogue-spec-signals.json`
/// document for view rendering.
///
/// A layer that has opted in (`catalogue_spec_signal.enabled = true`) is
/// expected to carry a fresh signals file whenever a view is rendered. Any
/// missing / symlinked / malformed / stale state is a system-level error
/// the caller should surface fail-closed, typically with the remediation
/// `sotp signal calc-catalog-spec <track_id>` to regenerate the file.
#[derive(Debug, Error)]
pub enum LoadCatalogueSpecSignalsForViewError {
    /// The signals file is absent at the expected path.
    #[error("catalogue-spec-signals file not found at '{}'. Run `sotp signal calc-catalog-spec <track_id>` to generate it.", path.display())]
    NotFound { path: PathBuf },

    /// The signals path exists but is not a regular file (symlink /
    /// directory / submodule). Same fail-closed policy as the existing
    /// `reject_symlinks_below` guards elsewhere in the repo.
    #[error("catalogue-spec-signals path '{}' is not a regular file (symlink or other non-file entry rejected)", path.display())]
    NotRegularFile { path: PathBuf },

    /// The signals file could not be read.
    #[error("failed to read catalogue-spec-signals at '{}': {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The signals file is not valid JSON / schema / hash format.
    #[error("failed to decode catalogue-spec-signals at '{}': {source}", path.display())]
    Decode {
        path: PathBuf,
        #[source]
        source: catalogue_spec_signals_codec::CatalogueSpecSignalsCodecError,
    },

    /// The signals file is stale relative to the on-disk catalogue bytes.
    #[error("catalogue-spec-signals at '{}' is stale (declared={declared}, actual={actual}). Run `sotp signal calc-catalog-spec <track_id>` to regenerate.", path.display())]
    StaleHash { path: PathBuf, declared: String, actual: String },
}

/// Load a `<layer>-catalogue-spec-signals.json` document for view rendering.
///
/// **Fail-closed**: any missing / symlinked / malformed / stale state is
/// reported as an error — the caller surfaces it and blocks view rendering.
/// The remediation is to re-run `sotp signal calc-catalog-spec
/// <track_id>` before the next view regeneration. Opt-out layers never
/// reach this function (callers gate on `catalogue_spec_signal_enabled()`).
///
/// Shared by `sync_rendered_views` (track-transition / sync-views path) and
/// the CLI type-signals refresh so both call sites error identically on
/// inconsistent state — without this helper the two paths diverged and
/// caused plan-artifacts review hashes to flap between 5-column and
/// 6-column renders across pre-commit steps.
///
/// # Errors
///
/// Returns [`LoadCatalogueSpecSignalsForViewError`] when the signals file
/// is absent, not a regular file, unreadable, malformed, or stale.
pub fn load_catalogue_spec_signals_for_view(
    signals_path: &Path,
    catalogue_bytes: &[u8],
) -> Result<CatalogueSpecSignalsDocument, LoadCatalogueSpecSignalsForViewError> {
    let meta = match signals_path.symlink_metadata() {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(LoadCatalogueSpecSignalsForViewError::NotFound {
                path: signals_path.to_path_buf(),
            });
        }
        Err(source) => {
            return Err(LoadCatalogueSpecSignalsForViewError::Io {
                path: signals_path.to_path_buf(),
                source,
            });
        }
    };
    if !meta.file_type().is_file() {
        return Err(LoadCatalogueSpecSignalsForViewError::NotRegularFile {
            path: signals_path.to_path_buf(),
        });
    }
    let json = std::fs::read_to_string(signals_path).map_err(|source| {
        LoadCatalogueSpecSignalsForViewError::Io { path: signals_path.to_path_buf(), source }
    })?;
    let doc = catalogue_spec_signals_codec::decode(&json).map_err(|source| {
        LoadCatalogueSpecSignalsForViewError::Decode { path: signals_path.to_path_buf(), source }
    })?;
    let actual = type_signals_codec::declaration_hash(catalogue_bytes);
    let declared = doc.catalogue_declaration_hash.to_hex();
    if declared != actual {
        return Err(LoadCatalogueSpecSignalsForViewError::StaleHash {
            path: signals_path.to_path_buf(),
            declared,
            actual,
        });
    }
    Ok(doc)
}

/// Section descriptor: a heading label paired with the predicate that selects entries.
struct Section {
    heading: &'static str,
    kind_tag: &'static str,
}

/// Canonical section order (D7).  Empty sections are skipped.
///
/// This list covers the v2-compat kind_tag values used by the v3 signal evaluator
/// and the `render_type_catalogue_v3` renderer.
const SECTIONS: &[Section] = &[
    Section { heading: "## Typestates", kind_tag: "typestate" },
    Section { heading: "## Enums", kind_tag: "enum" },
    Section { heading: "## Value Objects", kind_tag: "value_object" },
    Section { heading: "## Error Types", kind_tag: "error_type" },
    Section { heading: "## Secondary Ports", kind_tag: "secondary_port" },
    Section { heading: "## Application Services", kind_tag: "application_service" },
    Section { heading: "## Use Cases", kind_tag: "use_case" },
    Section { heading: "## Interactors", kind_tag: "interactor" },
    Section { heading: "## DTOs", kind_tag: "dto" },
    Section { heading: "## Commands", kind_tag: "command" },
    Section { heading: "## Queries", kind_tag: "query" },
    Section { heading: "## Factories", kind_tag: "factory" },
    Section { heading: "## Secondary Adapters", kind_tag: "secondary_adapter" },
    Section { heading: "## Domain Services", kind_tag: "domain_service" },
    Section { heading: "## Free Functions", kind_tag: "free_function" },
];

/// Additional section entries for v3-only role categories not present in the
/// v2 taxonomy covered by [`SECTIONS`].
///
/// `render_type_catalogue_v3` processes these after [`SECTIONS`]. The v3
/// renderer groups by the *real* v3 role (`type_entry_display_tag` /
/// `contract_role_display_tag` / `function_role_display_tag`), so a catalogue
/// entry whose role is `Entity` / `AggregateRoot` / `Specification` /
/// `SpecificationPort` / `UseCaseFunction` / `Repository` / `EventPolicy`
/// lands in its dedicated section here. Type-signal lookup within these
/// sections translates the heading tag back to the v2-compat storage key via
/// [`section_to_signal_kind_tag`]. Empty sections (no entry with that role)
/// are skipped at render time.
const V3_EXTRA_SECTIONS: &[Section] = &[
    Section { heading: "## Entities", kind_tag: "entity" },
    Section { heading: "## Aggregate Roots", kind_tag: "aggregate_root" },
    Section { heading: "## Specifications", kind_tag: "specification" },
    Section { heading: "## Specification Ports", kind_tag: "specification_port" },
    Section { heading: "## Use Case Functions", kind_tag: "use_case_function" },
    Section { heading: "## Repositories", kind_tag: "repository" },
    Section { heading: "## Event Policies", kind_tag: "event_policy" },
    Section { heading: "## Domain Events", kind_tag: "domain_event" },
];

/// Maps a section `kind_tag` (a real v3 role tag, as produced by
/// `type_entry_display_tag` / `contract_role_display_tag` /
/// `function_role_display_tag`) to the v2-compatible key under which type
/// signals are stored.
///
/// The signal evaluator stores signals under the v2-collapsed `kind_tag`
/// (e.g. an `Entity` entry's signal is keyed `"value_object"`, a
/// `UseCaseFunction`'s `"free_function"`, a `Repository`'s `"secondary_port"`),
/// so a section rendered under the dedicated v3 heading must translate back to
/// that key when looking up its Signal column. v3-only tags collapse;
/// everything else is the identity.
fn section_to_signal_kind_tag(section_kind_tag: &'static str) -> &'static str {
    match section_kind_tag {
        "entity" | "aggregate_root" | "specification" => "value_object",
        "specification_port" | "repository" => "secondary_port",
        "use_case_function" => "free_function",
        other => other,
    }
}

/// Row type used by `render_type_catalogue_v3` to group entries by section.
///
/// Fields: `(name, action_tag, details_str, cat_spec_signal_idx)`.
/// `cat_spec_signal_idx` is `Some(i)` only when `catalogue_spec_signals.is_some()`,
/// where `i` is the positional index in the signals document (catalogue generation order).
type V3Row = (String, &'static str, String, Option<usize>);

/// Maps a [`ConfidenceSignal`] to its display emoji for the `Cat-Spec` column.
///
/// Returns `🔵` / `🟡` / `🔴` for the three standard values; any future
/// extended variant renders as `?` so the output remains legible rather than
/// panicking.
fn catalogue_spec_signal_emoji(signal: ConfidenceSignal) -> String {
    match signal {
        ConfidenceSignal::Blue => "\u{1f535}".to_owned(),
        ConfidenceSignal::Yellow => "\u{1f7e1}".to_owned(),
        ConfidenceSignal::Red => "\u{1f534}".to_owned(),
        _ => "?".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// V3-native renderer for <layer>-types.md (T025)
// ---------------------------------------------------------------------------

/// Section/display kind tag for a v3 `TypeEntry` — the *real* v3 role category.
///
/// Used by `render_type_catalogue_v3` for section grouping and the `Kind`
/// column so the rendered `<layer>-types.md` reflects the v3 schema's taxonomy
/// (`Entity` / `AggregateRoot` / `Specification` stay distinct rather than
/// collapsing to `value_object`). Type-signal lookup translates this back to
/// the v2-compat storage key via [`section_to_signal_kind_tag`]. Priority
/// order:
///
/// 1. `PlainStruct { typestate: Some(_) }` → `"typestate"` (structural shape wins)
/// 2. `Enum { .. }` with `ErrorType` role → `"error_type"` (enum shape + role combo)
/// 3. `Enum { .. }` with any other role → `"enum"` (structural shape wins)
/// 4. All other shapes → role-based mapping via [`data_role_display_tag`]
fn type_entry_display_tag(role: &DataRole, kind: &TypeKindV2) -> &'static str {
    match kind {
        TypeKindV2::Struct(sk) if sk.typestate.is_some() => "typestate",
        TypeKindV2::Enum { .. } if matches!(role, DataRole::ErrorType) => "error_type",
        TypeKindV2::Enum { .. } => "enum",
        _ => data_role_display_tag(role),
    }
}

/// Display kind tag for a v3 `DataRole` — the real role, not the v2 collapse.
fn data_role_display_tag(role: &DataRole) -> &'static str {
    match role {
        DataRole::ValueObject { .. } => "value_object",
        DataRole::Entity { .. } => "entity",
        DataRole::AggregateRoot { .. } => "aggregate_root",
        DataRole::Specification => "specification",
        DataRole::DomainService { .. } => "domain_service",
        DataRole::Factory => "factory",
        DataRole::UseCase { .. } => "use_case",
        DataRole::Interactor => "interactor",
        DataRole::Command => "command",
        DataRole::Query => "query",
        DataRole::Dto => "dto",
        DataRole::ErrorType => "error_type",
        DataRole::SecondaryAdapter => "secondary_adapter",
        DataRole::EventPolicy { .. } => "event_policy",
        DataRole::DomainEvent => "domain_event",
    }
}

/// Display kind tag for a v3 `ContractRole` — the real role, not the v2 collapse.
fn contract_role_display_tag(role: &ContractRole) -> &'static str {
    match role {
        ContractRole::SpecificationPort => "specification_port",
        ContractRole::SecondaryPort => "secondary_port",
        ContractRole::ApplicationService => "application_service",
        ContractRole::Repository { .. } => "repository",
    }
}

/// Display kind tag for a v3 `FunctionRole` — the real role, not the v2 collapse.
fn function_role_display_tag(role: FunctionRole) -> &'static str {
    match role {
        FunctionRole::UseCaseFunction => "use_case_function",
        FunctionRole::FreeFunction => "free_function",
    }
}

/// Maps a v3 `ItemAction` display string to the rendered action column value.
fn v3_action_tag(action: domain::tddd::catalogue_v2::ItemAction) -> &'static str {
    use domain::tddd::catalogue_v2::ItemAction;
    match action {
        ItemAction::Add => "\u{2014}", // — (default; omitted)
        ItemAction::Modify => "modify",
        ItemAction::Reference => "reference",
        ItemAction::Delete => "delete",
    }
}

/// Renders a v3 `CatalogueDocument` as a `<layer>-types.md` markdown view.
///
/// T025: v3-native renderer — accepts `CatalogueDocument` directly (no
/// intermediate stub conversion). Entries from `types`,
/// `traits`, and `functions` are merged and grouped into sections by kind tag
/// (derived from `DataRole` / `ContractRole` / `FunctionRole`). Sections with
/// no entries are omitted.
///
/// The Signal column is populated from `type_signals` when supplied. Signal
/// lookup is by `(name, signal_kind_tag)` — the signal kind tag is derived
/// from the section kind tag via `section_to_signal_kind_tag`, which maps
/// v3-only section tags to their v2-compat collapsed equivalents (e.g.
/// `"specification_port"` → `"secondary_port"`, `"use_case_function"` →
/// `"free_function"`). This ensures signals stored under the compat tag are
/// matched correctly even when entries are rendered in a dedicated v3 section.
///
/// The Cat-Spec column is appended when `catalogue_spec_signals` is `Some`
/// (i.e. the layer has opted in and the caller loaded and validated the
/// `<layer>-catalogue-spec-signals.json`). Lookup is by positional index
/// assigned during the types→traits→functions BTreeMap traversal — the signal
/// generator uses the same order, so `signals[i]` always corresponds to the
/// i-th entry encountered during catalogue iteration. Name-only first-match
/// would assign the wrong signal when a type and a trait share the same
/// display name (they live in separate BTreeMaps but can have identical
/// short names). When `catalogue_spec_signals` is `None`, the existing
/// 5-column layout is preserved unchanged (matching the v2 renderer's opt-in
/// semantics).
///
/// # Parameters
///
/// * `doc` — the v3 type catalogue document.
/// * `source_file_name` — filename used in the `<!-- Generated from ... -->`
///   header comment. Sanitised against HTML comment injection.
/// * `type_signals` — optional slice of evaluated type signals (from
///   `<layer>-type-signals.json`) for Signal column rendering.
/// * `catalogue_spec_signals` — when `Some`, appends a `Cat-Spec` column
///   populated from the per-entry signals. When `None`, the existing
///   5-column layout is preserved unchanged. See ADR
///   `2026-04-23-0344-catalogue-spec-signal-activation.md` §D2.5 / IN-17.
#[must_use]
pub fn render_type_catalogue_v3(
    doc: &CatalogueDocument,
    source_file_name: &str,
    type_signals: Option<&[TypeSignal]>,
    catalogue_spec_signals: Option<&CatalogueSpecSignalsDocument>,
) -> String {
    let mut out = String::new();

    let safe_name = source_file_name.replace(['\n', '\r'], "").replace("-->", "-- >");
    out.push_str(&format!("<!-- Generated from {safe_name} \u{2014} DO NOT EDIT DIRECTLY -->\n"));

    let has_spec_signals = catalogue_spec_signals.is_some();

    // Build a combined list of (name, action, details, cat_spec_signal_idx) per
    // kind_tag. Entries are collected in catalogue generation order
    // (types BTreeMap → traits BTreeMap → functions BTreeMap, all sorted by key)
    // so that `cat_spec_signal_idx` matches the positional index in the signals
    // document — the signal generator uses the same traversal order, and a v3
    // catalogue may have a type and a trait sharing the same display name (in
    // separate BTreeMaps), so name-only matching would assign the wrong signal.
    //
    // Row type: `V3Row` = (name, action_tag, details_str, cat_spec_signal_idx).
    // `cat_spec_signal_idx` is `Some(i)` only when `catalogue_spec_signals.is_some()`.
    let mut rows_by_kind: std::collections::BTreeMap<&'static str, Vec<V3Row>> =
        std::collections::BTreeMap::new();

    // Monotonically increasing index tracking position in catalogue generation order
    // (types BTreeMap → traits BTreeMap → functions BTreeMap). This matches the
    // signal generator's traversal order, ensuring `cat_spec_signal_idx` for each
    // row corresponds to the correct positional entry in the signals document.
    let mut spec_idx: usize = 0;

    for (type_name, type_entry) in &doc.types {
        // Group by the real v3 role (display tag) so the rendered view reflects
        // the v3 taxonomy; structural shapes (enum, typestate) still take
        // precedence over the semantic DataRole. Signal lookup within the
        // section translates this back to the v2-compat key via
        // section_to_signal_kind_tag.
        let kind_tag = type_entry_display_tag(&type_entry.role, &type_entry.kind);
        let action = v3_action_tag(type_entry.action);
        let details = v3_type_entry_details(
            type_entry,
            type_name.as_str(),
            doc.crate_name.as_str(),
            &doc.trait_impls,
        );
        let sig_idx = has_spec_signals.then_some(spec_idx);
        spec_idx += 1;
        rows_by_kind.entry(kind_tag).or_default().push((
            type_name.as_str().to_owned(),
            action,
            details,
            sig_idx,
        ));
    }

    for (trait_name, trait_entry) in &doc.traits {
        let kind_tag = contract_role_display_tag(&trait_entry.role);
        let action = v3_action_tag(trait_entry.action);
        let details = v3_trait_entry_details(trait_entry);
        let sig_idx = has_spec_signals.then_some(spec_idx);
        spec_idx += 1;
        rows_by_kind.entry(kind_tag).or_default().push((
            trait_name.as_str().to_owned(),
            action,
            details,
            sig_idx,
        ));
    }

    for (fn_path, fn_entry) in &doc.functions {
        let kind_tag = function_role_display_tag(fn_entry.role);
        let action = v3_action_tag(fn_entry.action);
        let details = v3_function_entry_details(fn_entry);
        let sig_idx = has_spec_signals.then_some(spec_idx);
        spec_idx += 1;
        rows_by_kind.entry(kind_tag).or_default().push((
            fn_path.to_string(),
            action,
            details,
            sig_idx,
        ));
    }

    // Emit sections in canonical SECTIONS order (same order as the v2 renderer),
    // followed by V3_EXTRA_SECTIONS for roles that only exist in the v3 schema.
    let all_v3_sections = SECTIONS.iter().chain(V3_EXTRA_SECTIONS.iter());
    for section in all_v3_sections {
        let Some(entries) = rows_by_kind.get(section.kind_tag) else {
            continue;
        };
        if entries.is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(section.heading);
        out.push_str("\n\n");
        if has_spec_signals {
            out.push_str("| Name | Kind | Action | Details | Signal | Cat-Spec |\n");
            out.push_str("|------|------|--------|---------|--------|----------|\n");
        } else {
            out.push_str("| Name | Kind | Action | Details | Signal |\n");
            out.push_str("|------|------|--------|---------|--------|\n");
        }

        // Sort by name within each section for deterministic output.
        let mut sorted_entries = entries.clone();
        sorted_entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, action, details, cat_spec_idx) in &sorted_entries {
            // Signal column: look up by (name, signal_kind_tag).
            let sig_kind_tag = section_to_signal_kind_tag(section.kind_tag);
            let signal_col = type_signals
                .and_then(|sigs| {
                    sigs.iter()
                        .find(|s| s.type_name() == name.as_str() && s.kind_tag() == sig_kind_tag)
                })
                .map(|sig| match sig.signal() {
                    ConfidenceSignal::Blue => "\u{1f535}".to_owned(),
                    ConfidenceSignal::Yellow => "\u{1f7e1}".to_owned(),
                    ConfidenceSignal::Red => "\u{1f534}".to_owned(),
                    _ => "?".to_owned(),
                })
                .unwrap_or_else(|| "\u{2014}".to_owned()); // —
            if let Some(spec_signals) = catalogue_spec_signals {
                // Cat-Spec column: look up by the positional index assigned during
                // types→traits→functions traversal. This index corresponds to the
                // position in the signals document (same generation order), so it
                // is correct even when a type and a trait share the same display name.
                //
                // The `type_name` guard is a defensive cross-check: a fresh,
                // hash-verified signals document always satisfies
                // `signals[i].type_name == entry name at index i`. A mismatch here
                // means the signals doc is stale in a way the declaration-hash check
                // missed; fall back to `—` rather than painting the wrong signal.
                let cat_spec_col = cat_spec_idx
                    .and_then(|i| spec_signals.signals.get(i))
                    .filter(|sig| sig.type_name == name.as_str())
                    .map(|sig| catalogue_spec_signal_emoji(sig.signal))
                    .unwrap_or_else(|| "\u{2014}".to_owned()); // —
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    name, section.kind_tag, action, details, signal_col, cat_spec_col,
                ));
            } else {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    name, section.kind_tag, action, details, signal_col,
                ));
            }
        }
    }

    // Any kind tags not in SECTIONS or V3_EXTRA_SECTIONS (truly unknown roles)
    // are appended as an "## Other" section so they are not silently dropped.
    let known_kind_tags: std::collections::BTreeSet<&str> =
        SECTIONS.iter().chain(V3_EXTRA_SECTIONS.iter()).map(|s| s.kind_tag).collect();
    let mut other_entries = Vec::new();
    for (kind_tag, entries) in &rows_by_kind {
        if !known_kind_tags.contains(kind_tag) {
            for (name, action, details, cat_spec_idx) in entries {
                other_entries.push((
                    kind_tag,
                    name.clone(),
                    action,
                    details.clone(),
                    *cat_spec_idx,
                ));
            }
        }
    }
    if !other_entries.is_empty() {
        other_entries.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(&b.1)));
        out.push('\n');
        out.push_str("## Other\n\n");
        if has_spec_signals {
            out.push_str("| Name | Kind | Action | Details | Signal | Cat-Spec |\n");
            out.push_str("|------|------|--------|---------|--------|----------|\n");
        } else {
            out.push_str("| Name | Kind | Action | Details | Signal |\n");
            out.push_str("|------|------|--------|---------|--------|\n");
        }
        for (kind_tag, name, action, details, cat_spec_idx) in &other_entries {
            let signal_col = type_signals
                .and_then(|sigs| {
                    sigs.iter()
                        .find(|s| s.type_name() == name.as_str() && s.kind_tag() == **kind_tag)
                })
                .map(|sig| match sig.signal() {
                    ConfidenceSignal::Blue => "\u{1f535}".to_owned(),
                    ConfidenceSignal::Yellow => "\u{1f7e1}".to_owned(),
                    ConfidenceSignal::Red => "\u{1f534}".to_owned(),
                    _ => "?".to_owned(),
                })
                .unwrap_or_else(|| "\u{2014}".to_owned()); // —
            if let Some(spec_signals) = catalogue_spec_signals {
                // Defensive `type_name` cross-check — see the main-section loop above.
                let cat_spec_col = cat_spec_idx
                    .and_then(|i| spec_signals.signals.get(i))
                    .filter(|sig| sig.type_name == name.as_str())
                    .map(|sig| catalogue_spec_signal_emoji(sig.signal))
                    .unwrap_or_else(|| "\u{2014}".to_owned()); // —
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    name, kind_tag, action, details, signal_col, cat_spec_col,
                ));
            } else {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    name, kind_tag, action, details, signal_col,
                ));
            }
        }
    }

    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use domain::{ConfidenceSignal, TypeSignal};

    use super::entry_details::for_type_local_bare_name;
    use super::*;

    // (T027: v2 render_type_catalogue tests removed — render_type_catalogue deleted)

    // (T027: test_sections_covers_all_kind_tags deleted)

    // ---------------------------------------------------------------------------
    // render_type_catalogue_v3: Cat-Spec column
    // ---------------------------------------------------------------------------

    fn make_v3_doc_with_value_object(type_name: &str) -> CatalogueDocument {
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CrateName, DataRole, ItemAction, ModulePath, StructKind,
            StructShape, TypeEntry, TypeKindV2, TypeName,
        };
        use domain::tddd::layer_id::LayerId;
        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new(type_name).unwrap(), entry);
        doc
    }

    fn make_dummy_hash() -> domain::ContentHash {
        domain::ContentHash::try_from_hex("a".repeat(64).as_str()).unwrap()
    }

    fn make_spec_signals_doc(
        signals: Vec<(String, ConfidenceSignal)>,
    ) -> CatalogueSpecSignalsDocument {
        use domain::{CatalogueSpecSignal, CatalogueSpecSignalsDocument};
        let sigs = signals
            .into_iter()
            .map(|(name, sig)| {
                CatalogueSpecSignal::new(name, sig, domain::ContentHash::from_bytes([0u8; 32]))
            })
            .collect();
        CatalogueSpecSignalsDocument::new(make_dummy_hash(), sigs)
    }

    #[test]
    fn test_render_type_catalogue_v3_without_cat_spec_emits_five_columns() {
        let doc = make_v3_doc_with_value_object("TrackId");
        let output = render_type_catalogue_v3(&doc, "domain-types.json", None, None);
        assert!(
            output.contains("| Name | Kind | Action | Details | Signal |"),
            "expected 5-column header, got: {output}"
        );
        assert!(
            !output.contains("Cat-Spec"),
            "should not have Cat-Spec column when catalogue_spec_signals is None"
        );
    }

    #[test]
    fn test_render_type_catalogue_v3_with_cat_spec_emits_six_columns() {
        let doc = make_v3_doc_with_value_object("TrackId");
        let spec_signals =
            make_spec_signals_doc(vec![("TrackId".to_owned(), ConfidenceSignal::Blue)]);
        let output = render_type_catalogue_v3(&doc, "domain-types.json", None, Some(&spec_signals));
        assert!(
            output.contains("| Name | Kind | Action | Details | Signal | Cat-Spec |"),
            "expected 6-column header, got: {output}"
        );
    }

    #[test]
    fn test_render_type_catalogue_v3_cat_spec_column_shows_blue_signal() {
        let doc = make_v3_doc_with_value_object("TrackId");
        let spec_signals =
            make_spec_signals_doc(vec![("TrackId".to_owned(), ConfidenceSignal::Blue)]);
        let output = render_type_catalogue_v3(&doc, "domain-types.json", None, Some(&spec_signals));
        // Blue = 🔵
        assert!(
            output.contains("| TrackId | value_object |") && output.contains("\u{1f535}"),
            "expected Blue \u{1f535} in Cat-Spec column, got: {output}"
        );
    }

    #[test]
    fn test_render_type_catalogue_v3_cat_spec_column_shows_dash_when_name_not_found() {
        // Build a catalogue with two types: "AType" (index 0) and "ZType" (index 1).
        // BTreeMap iterates in sorted order, so "AType" < "ZType".
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CrateName, DataRole, ItemAction, ModulePath, StructKind,
            StructShape, TypeEntry, TypeKindV2, TypeName,
        };
        use domain::tddd::layer_id::LayerId;
        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        let plain_entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("AType").unwrap(), plain_entry.clone());
        doc.types.insert(TypeName::new("ZType").unwrap(), plain_entry);
        // Signals doc has only 1 entry (for index 0 = "AType").
        // "ZType" at index 1 has no corresponding signal → should show "—".
        let spec_signals =
            make_spec_signals_doc(vec![("AType".to_owned(), ConfidenceSignal::Blue)]);
        let output = render_type_catalogue_v3(&doc, "domain-types.json", None, Some(&spec_signals));
        // "AType" (index 0) gets 🔵
        let row_a =
            output.lines().find(|l| l.contains("AType")).expect("AType row must be present");
        assert!(
            row_a.ends_with("| \u{1f535} |"),
            "AType (index 0) should show \u{1f535}, row: {row_a}"
        );
        // "ZType" (index 1) has no signal in the doc → "—"
        let row_z =
            output.lines().find(|l| l.contains("ZType")).expect("ZType row must be present");
        assert!(
            row_z.ends_with("| \u{2014} |"),
            "Cat-Spec column should be '\u{2014}' when position exceeds signals length, row: {row_z}"
        );
    }

    #[test]
    fn test_render_type_catalogue_v3_cat_spec_column_shows_dash_when_signal_name_mismatches() {
        // Index 0 = "AType", but the signal at index 0 names "Bogus" — a stale
        // signals doc the declaration-hash check did not catch. The defensive
        // `type_name` guard must fall back to "—" rather than painting 🔵 onto AType.
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CrateName, DataRole, ItemAction, ModulePath, StructKind,
            StructShape, TypeEntry, TypeKindV2, TypeName,
        };
        use domain::tddd::layer_id::LayerId;
        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        doc.types.insert(
            TypeName::new("AType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],

                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        let spec_signals =
            make_spec_signals_doc(vec![("Bogus".to_owned(), ConfidenceSignal::Blue)]);
        let output = render_type_catalogue_v3(&doc, "domain-types.json", None, Some(&spec_signals));
        let row_a =
            output.lines().find(|l| l.contains("AType")).expect("AType row must be present");
        assert!(
            row_a.ends_with("| \u{2014} |"),
            "Cat-Spec column must be '\u{2014}' when the positional signal names a different entry, row: {row_a}"
        );
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue_v3: v3-only role sections + signal-key translation
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_type_catalogue_v3_entity_renders_under_entities_with_value_object_signal() {
        // A v3 `Entity` role keeps its own `## Entities` section (not collapsed
        // into `## Value Objects`), and its type-signal — stored by the signal
        // evaluator under the v2-compat `"value_object"` key — is still matched
        // via `section_to_signal_kind_tag`.
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CrateName, DataRole, ItemAction, ModulePath, StructKind,
            StructShape, TypeEntry, TypeKindV2, TypeName,
        };
        use domain::tddd::layer_id::LayerId;
        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        doc.types.insert(
            TypeName::new("UserAccount").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::entity().unwrap(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],

                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        let signals = vec![TypeSignal::new(
            "UserAccount",
            "value_object",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )];
        let output = render_type_catalogue_v3(&doc, "domain-types.json", Some(&signals), None);
        assert!(
            output.contains("## Entities"),
            "an Entity role must render under '## Entities', got:\n{output}"
        );
        assert!(
            !output.contains("## Value Objects"),
            "an Entity-only catalogue must not emit a '## Value Objects' section, got:\n{output}"
        );
        let row = output
            .lines()
            .find(|l| l.contains("| UserAccount | entity |"))
            .expect("UserAccount row with 'entity' Kind must be present");
        assert!(
            row.contains("\u{1f535}"),
            "the type-signal stored under the v2-compat 'value_object' key must still paint \u{1f535}, row: {row}"
        );
    }

    #[test]
    fn test_render_type_catalogue_v3_use_case_function_renders_under_use_case_functions() {
        // A v3 `UseCaseFunction` keeps its own `## Use Case Functions` section
        // (not collapsed into `## Free Functions`), and its type-signal —
        // stored under the v2-compat `"free_function"` key — is still matched.
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::{FunctionName, FunctionPath};
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CrateName, FunctionRole, ItemAction, TypeRef,
        };
        use domain::tddd::layer_id::LayerId;
        let layer = LayerId::try_new("usecase".to_owned()).unwrap();
        let crate_name = CrateName::new("usecase").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name.clone(), layer);
        let fn_path =
            FunctionPath::at_root(crate_name, FunctionName::new("register_user").unwrap());
        doc.functions.insert(
            fn_path.clone(),
            FunctionEntry {
                action: ItemAction::Add,
                role: FunctionRole::UseCaseFunction,
                params: vec![],
                returns: TypeRef::new("()").unwrap(),
                is_async: false,
                generics: vec![],
                where_predicates: vec![],
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        let signals = vec![TypeSignal::new(
            fn_path.to_string(),
            "free_function",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )];
        let output = render_type_catalogue_v3(&doc, "usecase-types.json", Some(&signals), None);
        assert!(
            output.contains("## Use Case Functions"),
            "a UseCaseFunction role must render under '## Use Case Functions', got:\n{output}"
        );
        assert!(
            !output.contains("## Free Functions"),
            "a UseCaseFunction-only catalogue must not emit a '## Free Functions' section, got:\n{output}"
        );
        let row = output
            .lines()
            .find(|l| l.contains("use_case_function"))
            .expect("a row with 'use_case_function' Kind must be present");
        assert!(
            row.contains("\u{1f535}"),
            "the type-signal stored under the v2-compat 'free_function' key must still paint \u{1f535}, row: {row}"
        );
    }

    #[test]
    fn test_render_includes_domain_event_section() {
        // A DomainEvent role entry must appear under "## Domain Events" in the
        // rendered view. This is a regression guard for the T012/T013 bug where
        // `DataRole::DomainEvent` was added without updating SECTIONS /
        // V3_EXTRA_SECTIONS, causing DomainEvent entries to be silently dropped.
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CrateName, DataRole, ItemAction, ModulePath, StructKind,
            StructShape, TypeEntry, TypeKindV2, TypeName,
        };
        use domain::tddd::layer_id::LayerId;
        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        doc.types.insert(
            TypeName::new("UserRegistered").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::DomainEvent,
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        let output = render_type_catalogue_v3(&doc, "domain-types.json", None, None);
        assert!(
            output.contains("## Domain Events"),
            "a DomainEvent entry must render under '## Domain Events', got:\n{output}"
        );
        assert!(
            output.lines().any(|l| l.contains("| UserRegistered | domain_event |")),
            "UserRegistered row with 'domain_event' Kind must be present, got:\n{output}"
        );
    }

    // ---------------------------------------------------------------------------
    // for_type_local_bare_name: ADR 2026-05-20-0048 D2 convention
    // ---------------------------------------------------------------------------

    #[test]
    fn test_for_type_local_bare_name_bare_name_returns_some() {
        // Per ADR D2, a bare name (no `::`) is the convention for self-crate types.
        assert_eq!(for_type_local_bare_name("MyAdapter", "my_crate"), Some("MyAdapter"));
    }

    #[test]
    fn test_for_type_local_bare_name_bare_name_with_generic_args_strips_args() {
        assert_eq!(for_type_local_bare_name("MyType<T>", "my_crate"), Some("MyType"));
    }

    #[test]
    fn test_for_type_local_bare_name_crate_prefix_returns_last_segment() {
        assert_eq!(for_type_local_bare_name("crate::MyAdapter", "my_crate"), Some("MyAdapter"));
    }

    #[test]
    fn test_for_type_local_bare_name_self_prefix_returns_last_segment() {
        assert_eq!(for_type_local_bare_name("self::Inner", "my_crate"), Some("Inner"));
    }

    #[test]
    fn test_for_type_local_bare_name_super_prefix_returns_last_segment() {
        assert_eq!(for_type_local_bare_name("super::Parent", "my_crate"), Some("Parent"));
    }

    #[test]
    fn test_for_type_local_bare_name_self_crate_prefix_returns_last_segment() {
        assert_eq!(for_type_local_bare_name("my_crate::LocalType", "my_crate"), Some("LocalType"));
    }

    #[test]
    fn test_for_type_local_bare_name_external_qualified_path_returns_none() {
        // Per ADR D2, external types must use crate-prefixed qualified paths.
        assert_eq!(for_type_local_bare_name("std::vec::Vec<i32>", "my_crate"), None);
    }

    #[test]
    fn test_for_type_local_bare_name_other_crate_prefix_returns_none() {
        assert_eq!(for_type_local_bare_name("other_crate::Foo", "my_crate"), None);
    }

    #[test]
    fn test_for_type_local_bare_name_std_prelude_bare_name_returns_some() {
        // A bare name that matches a std prelude type (e.g. "Vec") is treated as a local
        // type per ADR D2 convention. This function returns Some("Vec") regardless of
        // whether there is a local TypeEntry named "Vec". The caller then checks if the
        // returned name equals the current TypeEntry name: if there is a local TypeEntry
        // named "Vec", the impl is shown; if not, the impl simply does not appear in any
        // row. Per ADR D2, for external std types use qualified paths (e.g.
        // "std::vec::Vec<i32>"), which this function correctly maps to None (external).
        assert_eq!(for_type_local_bare_name("Vec", "my_crate"), Some("Vec"));
        assert_eq!(for_type_local_bare_name("Option", "my_crate"), Some("Option"));
    }

    #[test]
    fn test_for_type_local_bare_name_nested_self_crate_path_returns_last_segment() {
        assert_eq!(
            for_type_local_bare_name("my_crate::module::Adapter", "my_crate"),
            Some("Adapter")
        );
    }
}
