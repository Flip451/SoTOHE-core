//! Renderer for the per-layer type catalogue markdown view (e.g.
//! `domain-types.md`, a read-only view of `TypeCatalogueDocument`).
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
//! via `render_type_catalogue`'s new `catalogue_spec_signals` parameter, an
//! additional `Cat-Spec` column is appended to the per-section table —
//! showing the SoT Chain ② catalogue-spec grounding signal alongside the
//! existing SoT Chain ③ type→implementation `Signal`. When the parameter is
//! `None` (layer not yet opted in, or signals file absent/stale), the
//! existing 5-column layout is preserved unchanged.

use std::path::{Path, PathBuf};

use domain::{
    CatalogueSpecSignalsDocument, ConfidenceSignal, TypeAction, TypeCatalogueDocument,
    TypeCatalogueEntry, TypeDefinitionKind, TypestateTransitions,
};
use thiserror::Error;

use crate::tddd::{catalogue_spec_signals_codec, type_signals_codec};

/// Failure modes when loading a `<layer>-catalogue-spec-signals.json`
/// document for view rendering.
///
/// A layer that has opted in (`catalogue_spec_signal.enabled = true`) is
/// expected to carry a fresh signals file whenever a view is rendered. Any
/// missing / symlinked / malformed / stale state is a system-level error
/// the caller should surface fail-closed, typically with the remediation
/// `sotp track catalogue-spec-signals <track_id>` to regenerate the file.
#[derive(Debug, Error)]
pub enum LoadCatalogueSpecSignalsForViewError {
    /// The signals file is absent at the expected path.
    #[error("catalogue-spec-signals file not found at '{}'. Run `sotp track catalogue-spec-signals <track_id>` to generate it.", path.display())]
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
    #[error("catalogue-spec-signals at '{}' is stale (declared={declared}, actual={actual}). Run `sotp track catalogue-spec-signals <track_id>` to regenerate.", path.display())]
    StaleHash { path: PathBuf, declared: String, actual: String },
}

/// Load a `<layer>-catalogue-spec-signals.json` document for view rendering.
///
/// **Fail-closed**: any missing / symlinked / malformed / stale state is
/// reported as an error — the caller surfaces it and blocks view rendering.
/// The remediation is to re-run `sotp track catalogue-spec-signals
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

/// Renders the full `domain-types.md` document for a `TypeCatalogueDocument`.
///
/// Returns a markdown string suitable for writing to `domain-types.md`.
/// Entries are grouped by kind into per-section tables in the canonical order
/// defined by D7 of ADR `2026-04-13-1813-tddd-taxonomy-expansion.md`.
/// Sections with no entries are omitted.
///
/// # Parameters
///
/// * `doc` — the type catalogue declaration.
/// * `source_file_name` — filename used in the `<!-- Generated from ... -->`
///   header comment (e.g. `"domain-types.json"`). Sanitised against HTML
///   comment injection (newline strip + `-->` → `-- >` replacement).
/// * `catalogue_spec_signals` — when `Some`, appends a `Cat-Spec` column
///   populated from the per-entry signals. When `None`, the existing
///   5-column layout (`Name | Kind | Action | Details | Signal`) is preserved
///   unchanged. See ADR `2026-04-23-0344-catalogue-spec-signal-activation.md`
///   §D2.5 / IN-17.
#[must_use]
pub fn render_type_catalogue(
    doc: &TypeCatalogueDocument,
    source_file_name: &str,
    catalogue_spec_signals: Option<&CatalogueSpecSignalsDocument>,
) -> String {
    let mut out = String::new();

    // Sanitize source_file_name for safe HTML comment interpolation:
    // - Strip newlines (a newline inside an HTML comment produces invalid markdown)
    // - Replace `-->` with `-- >` to prevent premature comment close
    let safe_name = source_file_name.replace(['\n', '\r'], "").replace("-->", "-- >");
    out.push_str(&format!("<!-- Generated from {safe_name} — DO NOT EDIT DIRECTLY -->\n"));

    // Track consumed signal indices to handle delete+add pairs that share the same
    // (name, kind_tag) key. The first matching signal is consumed by the first entry;
    // any second entry with the same key skips past it to the next match.
    let mut consumed: std::collections::HashSet<usize> = std::collections::HashSet::new();

    let has_spec_signals = catalogue_spec_signals.is_some();

    for section in SECTIONS {
        let section_entries: Vec<&TypeCatalogueEntry> =
            doc.entries().iter().filter(|e| e.kind().kind_tag() == section.kind_tag).collect();

        if section_entries.is_empty() {
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

        for entry in section_entries {
            let signal_col =
                signal_for_entry(doc, entry.name(), entry.kind().kind_tag(), &mut consumed);
            let details_col = render_details(entry);
            let action_col = render_action(entry.action());
            if let Some(spec_signals) = catalogue_spec_signals {
                // Use the entry's catalogue-declared index to look up the
                // corresponding catalogue-spec signal. `signals[i]` was
                // generated for `doc.entries()[i]`, so a positional lookup
                // is correct even when a delete+add pair shares a `type_name`
                // and the SECTIONS canonical order differs from catalogue-declared
                // order — name-only first-match would assign signals to the wrong
                // entry in that case.
                //
                // The `type_name` guard is a defensive cross-check: a fresh,
                // hash-verified `CatalogueSpecSignalsDocument` always satisfies
                // `signals[i].type_name == entries[i].name`. A mismatch here means
                // the signals doc is stale in a way the declaration-hash check
                // missed; fall back to `—` rather than showing the wrong signal.
                let cat_spec_col = doc
                    .entries()
                    .iter()
                    .position(|e| std::ptr::eq(e, entry))
                    .and_then(|i| spec_signals.signals.get(i))
                    .filter(|sig| sig.type_name == entry.name())
                    .map(|sig| catalogue_spec_signal_emoji(sig.signal))
                    .unwrap_or_else(|| "\u{2014}".to_owned()); // —
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    entry.name(),
                    entry.kind().kind_tag(),
                    action_col,
                    details_col,
                    signal_col,
                    cat_spec_col,
                ));
            } else {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    entry.name(),
                    entry.kind().kind_tag(),
                    action_col,
                    details_col,
                    signal_col,
                ));
            }
        }
    }

    out.push('\n');
    out
}

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

/// Returns the signal emoji string for a named entry, or `"—"` if not evaluated.
///
/// `consumed` tracks signal indices already rendered so that a delete+add pair sharing
/// the same `(name, kind_tag)` identity does not show the same signal twice.
fn signal_for_entry(
    doc: &TypeCatalogueDocument,
    name: &str,
    kind_tag: &str,
    consumed: &mut std::collections::HashSet<usize>,
) -> String {
    let matched = doc.signals().and_then(|sigs| {
        sigs.iter()
            .enumerate()
            .find(|(idx, s)| {
                s.type_name() == name && s.kind_tag() == kind_tag && !consumed.contains(idx)
            })
            .map(|(idx, s)| {
                consumed.insert(idx);
                s
            })
    });
    matched
        .map(|sig| match sig.signal() {
            ConfidenceSignal::Blue => "\u{1f535}".to_owned(),
            ConfidenceSignal::Yellow => "\u{1f7e1}".to_owned(),
            ConfidenceSignal::Red => "\u{1f534}".to_owned(),
            _ => "?".to_owned(),
        })
        .unwrap_or_else(|| "\u{2014}".to_owned()) // —
}

/// Renders the Action column: `"—"` for the default `Add`, or the action tag otherwise.
fn render_action(action: TypeAction) -> &'static str {
    if action.is_default() { "\u{2014}" } else { action.action_tag() }
}

/// Renders the Details column for a single entry based on its kind.
fn render_details(entry: &TypeCatalogueEntry) -> String {
    match entry.kind() {
        TypeDefinitionKind::Typestate { transitions, .. } => match transitions {
            TypestateTransitions::Terminal => "\u{2205} (terminal)".to_owned(), // ∅ (terminal)
            TypestateTransitions::To(targets) => {
                targets.iter().map(|t| format!("\u{2192} {t}")).collect::<Vec<_>>().join(", ")
            }
        },
        TypeDefinitionKind::Enum { expected_variants }
        | TypeDefinitionKind::ErrorType { expected_variants } => {
            if expected_variants.is_empty() {
                "\u{2014}".to_owned()
            } else {
                expected_variants.join(", ")
            }
        }
        TypeDefinitionKind::SecondaryPort { expected_methods }
        | TypeDefinitionKind::ApplicationService { expected_methods } => {
            if expected_methods.is_empty() {
                "\u{2014}".to_owned()
            } else {
                expected_methods
                    .iter()
                    .map(|m: &domain::tddd::catalogue::MethodDeclaration| m.signature_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        // Existence-check-only variants render as em-dash (no structural detail).
        TypeDefinitionKind::ValueObject { .. }
        | TypeDefinitionKind::UseCase { .. }
        | TypeDefinitionKind::Interactor { .. }
        | TypeDefinitionKind::Dto { .. }
        | TypeDefinitionKind::Command { .. }
        | TypeDefinitionKind::Query { .. }
        | TypeDefinitionKind::Factory { .. }
        | TypeDefinitionKind::DomainService { .. }
        | TypeDefinitionKind::FreeFunction { .. } => "\u{2014}".to_owned(),
        TypeDefinitionKind::SecondaryAdapter { implements, .. } => {
            if implements.is_empty() {
                "\u{2014}".to_owned()
            } else {
                implements
                    .iter()
                    .map(|d| format!("impl {}", d.trait_name()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use domain::{
        ConfidenceSignal, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal,
    };

    use super::*;

    fn make_entry(name: &str, kind: TypeDefinitionKind) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(name, "description", kind, domain::TypeAction::Add, true).unwrap()
    }

    fn make_doc(entries: Vec<TypeCatalogueEntry>) -> TypeCatalogueDocument {
        TypeCatalogueDocument::new(1, entries)
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue: header
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_type_catalogue_includes_generated_header() {
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(
            output.contains("<!-- Generated from domain-types.json"),
            "missing generated header"
        );
    }

    #[test]
    fn test_render_type_catalogue_header_reflects_source_file_name_argument() {
        // Regression test for T004 / D2: the generated header must reflect the
        // `source_file_name` argument, not a hardcoded `domain-types.json` string.
        // This ensures non-domain layer rendered views (e.g. `infrastructure-types.md`,
        // `usecase-types.md`) correctly attribute their source.
        let doc = make_doc(vec![]);

        let infra_output = render_type_catalogue(&doc, "infrastructure-types.json", None);
        assert!(
            infra_output.contains("<!-- Generated from infrastructure-types.json"),
            "header must contain 'infrastructure-types.json', got: {infra_output}"
        );
        assert!(
            !infra_output.contains("<!-- Generated from domain-types.json"),
            "header must NOT hardcode 'domain-types.json' for infrastructure layer"
        );

        let usecase_output = render_type_catalogue(&doc, "usecase-types.json", None);
        assert!(
            usecase_output.contains("<!-- Generated from usecase-types.json"),
            "header must contain 'usecase-types.json', got: {usecase_output}"
        );
    }

    #[test]
    fn test_render_type_catalogue_header_sanitizes_comment_injection_sequences() {
        // Security guard: source_file_name is interpolated into an HTML comment header.
        // A name containing `-->` or a newline must be sanitized so it cannot close
        // the comment prematurely or inject arbitrary markdown.
        let doc = make_doc(vec![]);

        // `-->` in the filename must be replaced with `-- >` so the name part of the
        // comment cannot close the comment prematurely. The test checks that the
        // rendered header contains the sanitized form `evil-- >suffix.json` rather
        // than the raw `evil-->suffix.json` sequence.
        let injection_output = render_type_catalogue(&doc, "evil-->suffix.json", None);
        assert!(
            injection_output.contains("evil-- >suffix.json"),
            "sanitized name must appear as `-- >` replacement, got: {injection_output}"
        );
        assert!(
            !injection_output.contains("evil-->"),
            "unsanitized `-->` from filename must not appear, got: {injection_output}"
        );

        // A newline in the name must be stripped so the comment stays on one line.
        // After stripping the `\n`, the name becomes `badname.json` with no embedded newline.
        let newline_output = render_type_catalogue(&doc, "bad\nname.json", None);
        let first_line = newline_output.lines().next().unwrap_or("");
        assert!(
            first_line.contains("badname.json"),
            "newline in source_file_name must be stripped, first line got: {first_line}"
        );
    }

    #[test]
    fn test_render_type_catalogue_no_type_declarations_heading() {
        // D7: the old flat "## Type Declarations" heading is replaced by per-kind
        // section headings.  An empty catalogue produces no section headings at all.
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(
            !output.contains("## Type Declarations"),
            "old flat heading must not appear after D7 rewrite"
        );
    }

    #[test]
    fn test_render_type_catalogue_table_header_present_when_entries_exist() {
        let doc = make_doc(vec![make_entry(
            "Foo",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        )]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(
            output.contains("| Name | Kind | Action | Details | Signal |"),
            "missing table header"
        );
        assert!(
            output.contains("|------|------|--------|---------|--------|"),
            "missing table separator"
        );
    }

    #[test]
    fn test_render_type_catalogue_section_headers_appear_for_present_kinds() {
        // D7: each present kind renders under its designated section header.
        let doc = make_doc(vec![
            make_entry(
                "Foo",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry("Bar", TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }),
        ]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("## Value Objects"), "missing ## Value Objects");
        assert!(output.contains("## Secondary Ports"), "missing ## Secondary Ports");
        // Other section headers must NOT appear when no entries exist for them
        assert!(!output.contains("## Typestates"), "unexpected ## Typestates");
        assert!(!output.contains("## Factories"), "unexpected ## Factories");
    }

    #[test]
    fn test_render_type_catalogue_trait_ports_heading_absent() {
        // D7: the old "## Trait Ports" heading was renamed to "## Secondary Ports".
        let doc = make_doc(vec![make_entry(
            "MyPort",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] },
        )]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(!output.contains("## Trait Ports"), "old ## Trait Ports heading must not appear");
        assert!(output.contains("## Secondary Ports"), "## Secondary Ports must appear");
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue: entry rows
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_typestate_entry_row() {
        let entry = make_entry(
            "Draft",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| Draft | typestate |"), "missing typestate row");
        assert!(output.contains("\u{2192} Published"), "missing transition arrow");
    }

    #[test]
    fn test_render_typestate_terminal_shows_empty_set() {
        let entry = make_entry(
            "Final",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("\u{2205} (terminal)"), "missing terminal marker");
    }

    #[test]
    fn test_render_enum_entry_row() {
        let entry = make_entry(
            "TrackStatus",
            TypeDefinitionKind::Enum { expected_variants: vec!["Planned".into(), "Done".into()] },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| TrackStatus | enum |"), "missing enum row");
        assert!(output.contains("Planned, Done"), "missing enum variants");
    }

    #[test]
    fn test_render_value_object_entry_row() {
        let entry = make_entry(
            "TrackId",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| TrackId | value_object |"), "missing value_object row");
    }

    #[test]
    fn test_render_error_type_entry_row() {
        let entry = make_entry(
            "SchemaExportError",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NightlyNotFound".into()] },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| SchemaExportError | error_type |"), "missing error_type row");
        assert!(output.contains("NightlyNotFound"), "missing error variant");
    }

    #[test]
    fn test_render_secondary_port_entry_row() {
        let entry = make_entry(
            "SchemaExporter",
            TypeDefinitionKind::SecondaryPort {
                expected_methods: vec![domain::tddd::catalogue::MethodDeclaration::new(
                    "export",
                    Some("&self".into()),
                    vec![domain::tddd::catalogue::ParamDeclaration::new("crate_name", "str")],
                    "Result<SchemaExport, SchemaExportError>",
                    false,
                )],
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(
            output.contains("| SchemaExporter | secondary_port |"),
            "missing secondary_port row"
        );
        assert!(output.contains("fn export"), "missing method");
    }

    #[test]
    fn test_render_application_service_entry_row() {
        let entry = make_entry(
            "HookHandler",
            TypeDefinitionKind::ApplicationService {
                expected_methods: vec![domain::tddd::catalogue::MethodDeclaration::new(
                    "handle",
                    Some("&self".into()),
                    vec![],
                    "Result<HookVerdict, HookError>",
                    false,
                )],
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(
            output.contains("| HookHandler | application_service |"),
            "missing application_service row"
        );
        assert!(output.contains("fn handle"), "missing method");
    }

    #[test]
    fn test_render_use_case_entry_row() {
        let entry = make_entry(
            "SaveTrackUseCase",
            TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| SaveTrackUseCase | use_case |"), "missing use_case row");
    }

    #[test]
    fn test_render_interactor_entry_row() {
        let entry = make_entry(
            "SaveTrackInteractor",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| SaveTrackInteractor | interactor |"), "missing interactor row");
    }

    #[test]
    fn test_render_dto_entry_row() {
        let entry = make_entry(
            "CreateUserDto",
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| CreateUserDto | dto |"), "missing dto row");
    }

    #[test]
    fn test_render_command_entry_row() {
        let entry = make_entry(
            "CreateUserCommand",
            TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| CreateUserCommand | command |"), "missing command row");
    }

    #[test]
    fn test_render_query_entry_row() {
        let entry = make_entry(
            "GetUserQuery",
            TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| GetUserQuery | query |"), "missing query row");
    }

    #[test]
    fn test_render_factory_entry_row() {
        let entry = make_entry(
            "UserFactory",
            TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| UserFactory | factory |"), "missing factory row");
    }

    #[test]
    fn test_render_all_12_variants_in_one_catalogue() {
        // Verifies that all 12 TypeDefinitionKind variants render in one catalogue.
        let entries = vec![
            make_entry(
                "Draft",
                TypeDefinitionKind::Typestate {
                    transitions: TypestateTransitions::To(vec!["Published".into()]),
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "TrackStatus",
                TypeDefinitionKind::Enum {
                    expected_variants: vec!["Planned".into(), "Done".into()],
                },
            ),
            make_entry(
                "TrackId",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "AppError",
                TypeDefinitionKind::ErrorType { expected_variants: vec!["NotFound".into()] },
            ),
            make_entry(
                "TrackRepo",
                TypeDefinitionKind::SecondaryPort {
                    expected_methods: vec![domain::tddd::catalogue::MethodDeclaration::new(
                        "save",
                        Some("&self".into()),
                        vec![],
                        "()",
                        false,
                    )],
                },
            ),
            make_entry(
                "UseHandler",
                TypeDefinitionKind::ApplicationService {
                    expected_methods: vec![domain::tddd::catalogue::MethodDeclaration::new(
                        "execute",
                        Some("&self".into()),
                        vec![],
                        "()",
                        false,
                    )],
                },
            ),
            make_entry(
                "SaveUseCase",
                TypeDefinitionKind::UseCase {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "SaveInteractor",
                TypeDefinitionKind::Interactor {
                    expected_members: Vec::new(),
                    declares_application_service: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "SaveDto",
                TypeDefinitionKind::Dto {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "SaveCommand",
                TypeDefinitionKind::Command {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "GetQuery",
                TypeDefinitionKind::Query {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "AggFactory",
                TypeDefinitionKind::Factory {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
        ];
        let doc = make_doc(entries);
        let output = render_type_catalogue(&doc, "domain-types.json", None);

        // All 12 kind tags must appear in the output
        assert!(output.contains("typestate"), "missing typestate");
        assert!(output.contains("enum"), "missing enum");
        assert!(output.contains("value_object"), "missing value_object");
        assert!(output.contains("error_type"), "missing error_type");
        assert!(output.contains("secondary_port"), "missing secondary_port");
        assert!(output.contains("application_service"), "missing application_service");
        assert!(output.contains("use_case"), "missing use_case");
        assert!(output.contains("interactor"), "missing interactor");
        assert!(output.contains("dto"), "missing dto");
        assert!(output.contains("command"), "missing command");
        assert!(output.contains("query"), "missing query");
        assert!(output.contains("factory"), "missing factory");

        // Existence-check variants render em-dash in details
        assert!(output.contains("| SaveUseCase | use_case |"), "missing use_case row");
        assert!(output.contains("| SaveInteractor | interactor |"), "missing interactor row");
        assert!(output.contains("| SaveDto | dto |"), "missing dto row");
        assert!(output.contains("| SaveCommand | command |"), "missing command row");
        assert!(output.contains("| GetQuery | query |"), "missing query row");
        assert!(output.contains("| AggFactory | factory |"), "missing factory row");

        // Method-bearing variants render method list in details
        assert!(output.contains("fn save"), "missing fn save for secondary_port");
        assert!(output.contains("fn execute"), "missing fn execute for application_service");

        // trait_port must not appear
        assert!(!output.contains("trait_port"), "trait_port must not appear after T002 rename");

        // D7: all 12 section headers must appear (one per kind present)
        assert!(output.contains("## Typestates"), "missing ## Typestates");
        assert!(output.contains("## Enums"), "missing ## Enums");
        assert!(output.contains("## Value Objects"), "missing ## Value Objects");
        assert!(output.contains("## Error Types"), "missing ## Error Types");
        assert!(output.contains("## Secondary Ports"), "missing ## Secondary Ports");
        assert!(output.contains("## Application Services"), "missing ## Application Services");
        assert!(output.contains("## Use Cases"), "missing ## Use Cases");
        assert!(output.contains("## Interactors"), "missing ## Interactors");
        assert!(output.contains("## DTOs"), "missing ## DTOs");
        assert!(output.contains("## Commands"), "missing ## Commands");
        assert!(output.contains("## Queries"), "missing ## Queries");
        assert!(output.contains("## Factories"), "missing ## Factories");

        // Old flat heading must not appear
        assert!(!output.contains("## Type Declarations"), "flat heading must not appear after D7");
        // Old Trait Ports heading must not appear
        assert!(!output.contains("## Trait Ports"), "old ## Trait Ports must not appear");
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue: Signal column
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_signal_column_shows_dash_when_no_signals() {
        let entry = make_entry(
            "Draft",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("\u{2014}"), "missing em-dash for unevaluated signal");
    }

    #[test]
    fn test_render_signal_column_shows_blue_when_signal_blue() {
        let entry = make_entry(
            "Draft",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let mut doc = make_doc(vec![entry]);
        doc.set_signals(vec![TypeSignal::new(
            "Draft",
            "value_object",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("\u{1f535}"), "missing blue circle for Blue signal");
    }

    #[test]
    fn test_render_signal_column_shows_red_when_signal_red() {
        let entry = make_entry(
            "Ghost",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let mut doc = make_doc(vec![entry]);
        doc.set_signals(vec![TypeSignal::new(
            "Ghost",
            "value_object",
            ConfidenceSignal::Red,
            false,
            vec![],
            vec![],
            vec![],
        )]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("\u{1f534}"), "missing red circle for Red signal");
    }

    #[test]
    fn test_render_multiple_entries_all_present() {
        let entries = vec![
            make_entry(
                "Draft",
                TypeDefinitionKind::Typestate {
                    transitions: TypestateTransitions::To(vec!["Published".into()]),
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "TrackStatus",
                TypeDefinitionKind::Enum { expected_variants: vec!["Planned".into()] },
            ),
            make_entry(
                "TrackId",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
        ];
        let doc = make_doc(entries);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("Draft"), "missing Draft");
        assert!(output.contains("TrackStatus"), "missing TrackStatus");
        assert!(output.contains("TrackId"), "missing TrackId");
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue: Action column
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_add_action_shows_dash() {
        let entry = make_entry(
            "Foo",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        // Add action renders as em-dash
        assert!(output.contains("| \u{2014} |"), "Add action should show em-dash");
    }

    #[test]
    fn test_render_delete_action_shows_delete() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "deleted",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            domain::TypeAction::Delete,
            true,
        )
        .unwrap();
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.contains("| delete |"), "Delete action should show 'delete'");
    }

    #[test]
    fn test_render_output_ends_with_newline() {
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }

    // ---------------------------------------------------------------------------
    // SECTIONS coverage (TDDD-Q01)
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // render_type_catalogue: Cat-Spec column (T020, ADR 2026-04-23-0344 §D2.5)
    // ---------------------------------------------------------------------------

    use domain::{CatalogueSpecSignal, CatalogueSpecSignalsDocument, ContentHash};

    fn make_spec_signals(per_entry: Vec<(&str, ConfidenceSignal)>) -> CatalogueSpecSignalsDocument {
        let hash = ContentHash::from_bytes([0u8; 32]);
        let signals = per_entry.into_iter().map(|(n, s)| CatalogueSpecSignal::new(n, s)).collect();
        CatalogueSpecSignalsDocument::new(hash, signals)
    }

    #[test]
    fn test_render_cat_spec_column_absent_when_signals_none() {
        // Backward-compat: when catalogue_spec_signals is None, the rendered
        // output matches the legacy 5-column layout — no `Cat-Spec` header
        // and no extra column. A pre-existing caller that passes `None`
        // must see byte-identical output to the pre-T020 version.
        let entry = make_entry(
            "Foo",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json", None);
        assert!(
            !output.contains("Cat-Spec"),
            "Cat-Spec header must not appear when signals is None: {output}"
        );
        assert!(
            output.contains("| Name | Kind | Action | Details | Signal |"),
            "legacy 5-column header must be preserved when signals is None: {output}"
        );
    }

    #[test]
    fn test_render_cat_spec_column_present_when_signals_some() {
        // When catalogue_spec_signals is Some, a sixth `Cat-Spec` column is
        // appended to the header and each entry row. Signal values map to
        // the same emoji set as the existing Signal column.
        let entries = vec![
            make_entry(
                "FooBlue",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "BarYellow",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "BazRed",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            make_entry(
                "QuxMissing",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
        ];
        let doc = make_doc(entries);
        let spec_signals = make_spec_signals(vec![
            ("FooBlue", ConfidenceSignal::Blue),
            ("BarYellow", ConfidenceSignal::Yellow),
            ("BazRed", ConfidenceSignal::Red),
            // QuxMissing is deliberately absent so the Cat-Spec column
            // shows the `—` fallback for that entry.
        ]);
        let output = render_type_catalogue(&doc, "domain-types.json", Some(&spec_signals));

        // Header has six columns including Cat-Spec.
        assert!(
            output.contains("| Name | Kind | Action | Details | Signal | Cat-Spec |"),
            "six-column header missing: {output}"
        );
        assert!(
            output.contains("|------|------|--------|---------|--------|----------|"),
            "six-column separator missing: {output}"
        );

        // Per-entry emoji rendering (Blue/Yellow/Red/— em-dash for missing).
        assert!(output.contains("\u{1f535}"), "Blue emoji (🔵) missing");
        assert!(output.contains("\u{1f7e1}"), "Yellow emoji (🟡) missing");
        assert!(output.contains("\u{1f534}"), "Red emoji (🔴) missing");
        // The em-dash fallback exists (also used by the Signal column for
        // unevaluated entries) so its presence is always expected.
        assert!(output.contains("\u{2014}"), "em-dash missing for absent signal");

        // Ensure the Cat-Spec column is the LAST column of each data row,
        // not a replacement for the existing Signal column. Check one row
        // ends with ` | <Cat-Spec> |` and contains two emoji-or-dash cells
        // after `value_object |`.
        let blue_row = output
            .lines()
            .find(|l| l.starts_with("| FooBlue | value_object |"))
            .expect("FooBlue row must be present");
        // Row format: `| Name | Kind | Action | Details | Signal | Cat-Spec |`
        // — i.e. 7 `|` characters total (including leading and trailing).
        let pipe_count = blue_row.chars().filter(|c| *c == '|').count();
        assert_eq!(pipe_count, 7, "expected 7 pipes in six-column row, got: {blue_row}");
    }

    #[test]
    fn test_render_cat_spec_column_entry_without_matching_signal_shows_dash() {
        // An entry whose name is not present in `signals.signals` renders
        // the em-dash fallback, not a panic or misaligned row.
        let entry = make_entry(
            "Unmapped",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );
        let doc = make_doc(vec![entry]);
        // Spec signals document with a DIFFERENT entry name so the lookup misses.
        let spec_signals = make_spec_signals(vec![("Other", ConfidenceSignal::Blue)]);
        let output = render_type_catalogue(&doc, "domain-types.json", Some(&spec_signals));

        let row = output
            .lines()
            .find(|l| l.starts_with("| Unmapped |"))
            .expect("Unmapped row must be present");
        // The Cat-Spec cell (last non-empty cell) must be the em-dash.
        assert!(
            row.ends_with("| \u{2014} |"),
            "Unmapped entry without signal must show em-dash Cat-Spec cell: {row}"
        );
    }

    #[test]
    fn test_render_cat_spec_column_uses_catalogue_position_not_name_order() {
        // Regression guard for the delete+add pair ordering bug (gpt-5.5 P1
        // finding): when a delete+add pair shares a `type_name` and the two
        // entries' kinds fall in different SECTIONS positions (different canonical
        // section order vs catalogue-declared order), a name-only first-match
        // approach would assign the wrong signal to each entry.
        //
        // The signals document is generated in catalogue-declared order, so
        // `signals[i]` is always for `doc.entries()[i]`. The renderer must use
        // a positional lookup (by catalogue index) rather than a name-only
        // first-match.
        //
        // Catalogue-declared order: [enum(0), value_object(1)]
        // SECTIONS canonical order: enum(1) comes before value_object(2) — same
        // here since both are adjacent, but the signals are generated in catalogue
        // order: signals[0] = Red (for enum entry), signals[1] = Blue (for value_object).
        //
        // With the position-based lookup:
        //   enum row → catalogue index 0 → signals[0] = Red ✓
        //   value_object row → catalogue index 1 → signals[1] = Blue ✓
        //
        // With the old name-only first-match (bug):
        //   enum walks SECTIONS first → picks signals[0] = Red (correct by coincidence)
        //   value_object picks signals[1] = Blue (also correct by coincidence here)
        //
        // The critical case: catalogue order is [value_object(0), enum(1)] but
        // SECTIONS renders enum section BEFORE value_object section. Then:
        //   Signals: [0]=Blue (for value_object), [1]=Red (for enum)
        //   Name-only: enum section renders first → finds "SameName" at index 0
        //     → picks Blue (WRONG: should be Red, index 1)
        //   Positional: enum entry is at catalogue index 1 → signals[1] = Red ✓
        //
        // This test covers that case explicitly.
        use domain::TypeAction;

        // Build catalogue with value_object FIRST (catalogue index 0), enum SECOND
        // (catalogue index 1). SECTIONS renders enum (index 1 in SECTIONS) before
        // value_object (index 2 in SECTIONS), so the render order differs from
        // catalogue order.
        let vo_entry = TypeCatalogueEntry::new(
            "SameName",
            "value object entry",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let enum_entry = TypeCatalogueEntry::new(
            "SameName",
            "enum entry",
            TypeDefinitionKind::Enum { expected_variants: vec!["A".into(), "B".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Catalogue-declared order: [value_object(0), enum(1)]
        let doc = make_doc(vec![vo_entry, enum_entry]);

        // Signals in catalogue-declared order:
        //   signals[0] = Blue  (for value_object entry at catalogue index 0)
        //   signals[1] = Red   (for enum entry at catalogue index 1)
        let spec_signals = make_spec_signals(vec![
            ("SameName", ConfidenceSignal::Blue),
            ("SameName", ConfidenceSignal::Red),
        ]);

        let output = render_type_catalogue(&doc, "domain-types.json", Some(&spec_signals));

        // SECTIONS renders enum before value_object. The positional lookup must
        // assign:
        //   - enum row (catalogue index 1) → signals[1] = Red
        //   - value_object row (catalogue index 0) → signals[0] = Blue
        let enum_row = output
            .lines()
            .find(|l| l.contains("| SameName | enum |"))
            .expect("enum SameName row must be present");
        assert!(
            enum_row.ends_with("| \u{1f534} |"),
            "enum SameName must show Red (signals[1]), got: {enum_row}"
        );

        let vo_row = output
            .lines()
            .find(|l| l.contains("| SameName | value_object |"))
            .expect("value_object SameName row must be present");
        assert!(
            vo_row.ends_with("| \u{1f535} |"),
            "value_object SameName must show Blue (signals[0]), got: {vo_row}"
        );
    }

    #[test]
    fn test_sections_covers_all_kind_tags() {
        // Guards against SECTIONS being out of sync with TypeDefinitionKind.
        //
        // When a new variant is added to TypeDefinitionKind, the compiler
        // forces TypeDefinitionKind::kind_tag() to handle it (exhaustive match).
        // This test then verifies SECTIONS contains an entry for every
        // kind_tag the enum can produce.
        //
        // Maintenance: adding a new TypeDefinitionKind variant requires:
        //   1. Update TypeDefinitionKind::kind_tag() (compiler enforces)
        //   2. Add a Section entry to SECTIONS (this test enforces)
        //   3. Add a sample construction below so kind_tag() is exercised.
        //
        // Using constructed enum values (rather than a hardcoded &str list)
        // couples this test to the real enum so a rename/removal surfaces as
        // a compile error before the assertion is even reached.
        use std::collections::HashSet;

        let samples = vec![
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::Enum { expected_variants: Vec::new() },
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::ErrorType { expected_variants: Vec::new() },
            TypeDefinitionKind::SecondaryPort { expected_methods: Vec::new() },
            TypeDefinitionKind::ApplicationService { expected_methods: Vec::new() },
            TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
            TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::SecondaryAdapter {
                implements: Vec::new(),
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::DomainService {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: Vec::new(),
                expected_returns: Vec::new(),
                expected_is_async: false,
            },
        ];
        let all_kind_tags: HashSet<&str> = samples.iter().map(|k| k.kind_tag()).collect();

        let section_kind_tags: HashSet<&str> = SECTIONS.iter().map(|s| s.kind_tag).collect();

        assert_eq!(
            all_kind_tags, section_kind_tags,
            "SECTIONS must cover every TypeDefinitionKind::kind_tag() value \
             (add a Section entry when adding a new variant)"
        );
    }
}
