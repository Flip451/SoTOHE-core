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
//! - ValueObject / UseCase / Interactor / Dto / Command / Query / Factory: `—`
//!
//! T002: `TraitPort` removed; `SecondaryPort` and `ApplicationService` added
//! (method-list details). Seven existence-check-only variants added with `—`
//! details. Section header `## Trait Ports` renamed to `## Secondary Ports`.
//! New section headers: `## Application Services`, `## Use Cases`,
//! `## Interactors`, `## DTOs`, `## Commands`, `## Queries`, `## Factories`.
//!
//! The Signal column shows `🔵` / `🔴` / `—` (no signal yet).

use domain::{
    ConfidenceSignal, TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
    TypestateTransitions,
};

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
];

/// Renders the full `domain-types.md` document for a `TypeCatalogueDocument`.
///
/// Returns a markdown string suitable for writing to `domain-types.md`.
/// Entries are grouped by kind into per-section tables in the canonical order
/// defined by D7 of ADR `2026-04-13-1813-tddd-taxonomy-expansion.md`.
/// Sections with no entries are omitted.
#[must_use]
pub fn render_type_catalogue(doc: &TypeCatalogueDocument, source_file_name: &str) -> String {
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

    for section in SECTIONS {
        let section_entries: Vec<&TypeCatalogueEntry> =
            doc.entries().iter().filter(|e| e.kind().kind_tag() == section.kind_tag).collect();

        if section_entries.is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(section.heading);
        out.push_str("\n\n");
        out.push_str("| Name | Kind | Action | Details | Signal |\n");
        out.push_str("|------|------|--------|---------|--------|\n");

        for entry in section_entries {
            let signal_col =
                signal_for_entry(doc, entry.name(), entry.kind().kind_tag(), &mut consumed);
            let details_col = render_details(entry);
            let action_col = render_action(entry.action());
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

    out.push('\n');
    out
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
        TypeDefinitionKind::Typestate { transitions } => match transitions {
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
        TypeDefinitionKind::ValueObject
        | TypeDefinitionKind::UseCase
        | TypeDefinitionKind::Interactor
        | TypeDefinitionKind::Dto
        | TypeDefinitionKind::Command
        | TypeDefinitionKind::Query
        | TypeDefinitionKind::Factory
        | TypeDefinitionKind::SecondaryAdapter { .. } => "\u{2014}".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
        let output = render_type_catalogue(&doc, "domain-types.json");
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

        let infra_output = render_type_catalogue(&doc, "infrastructure-types.json");
        assert!(
            infra_output.contains("<!-- Generated from infrastructure-types.json"),
            "header must contain 'infrastructure-types.json', got: {infra_output}"
        );
        assert!(
            !infra_output.contains("<!-- Generated from domain-types.json"),
            "header must NOT hardcode 'domain-types.json' for infrastructure layer"
        );

        let usecase_output = render_type_catalogue(&doc, "usecase-types.json");
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
        let injection_output = render_type_catalogue(&doc, "evil-->suffix.json");
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
        let newline_output = render_type_catalogue(&doc, "bad\nname.json");
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
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(
            !output.contains("## Type Declarations"),
            "old flat heading must not appear after D7 rewrite"
        );
    }

    #[test]
    fn test_render_type_catalogue_table_header_present_when_entries_exist() {
        let doc = make_doc(vec![make_entry("Foo", TypeDefinitionKind::ValueObject)]);
        let output = render_type_catalogue(&doc, "domain-types.json");
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
            make_entry("Foo", TypeDefinitionKind::ValueObject),
            make_entry("Bar", TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }),
        ]);
        let output = render_type_catalogue(&doc, "domain-types.json");
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
        let output = render_type_catalogue(&doc, "domain-types.json");
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
            },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| Draft | typestate |"), "missing typestate row");
        assert!(output.contains("\u{2192} Published"), "missing transition arrow");
    }

    #[test]
    fn test_render_typestate_terminal_shows_empty_set() {
        let entry = make_entry(
            "Final",
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("\u{2205} (terminal)"), "missing terminal marker");
    }

    #[test]
    fn test_render_enum_entry_row() {
        let entry = make_entry(
            "TrackStatus",
            TypeDefinitionKind::Enum { expected_variants: vec!["Planned".into(), "Done".into()] },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| TrackStatus | enum |"), "missing enum row");
        assert!(output.contains("Planned, Done"), "missing enum variants");
    }

    #[test]
    fn test_render_value_object_entry_row() {
        let entry = make_entry("TrackId", TypeDefinitionKind::ValueObject);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| TrackId | value_object |"), "missing value_object row");
    }

    #[test]
    fn test_render_error_type_entry_row() {
        let entry = make_entry(
            "SchemaExportError",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NightlyNotFound".into()] },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
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
        let output = render_type_catalogue(&doc, "domain-types.json");
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
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(
            output.contains("| HookHandler | application_service |"),
            "missing application_service row"
        );
        assert!(output.contains("fn handle"), "missing method");
    }

    #[test]
    fn test_render_use_case_entry_row() {
        let entry = make_entry("SaveTrackUseCase", TypeDefinitionKind::UseCase);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| SaveTrackUseCase | use_case |"), "missing use_case row");
    }

    #[test]
    fn test_render_interactor_entry_row() {
        let entry = make_entry("SaveTrackInteractor", TypeDefinitionKind::Interactor);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| SaveTrackInteractor | interactor |"), "missing interactor row");
    }

    #[test]
    fn test_render_dto_entry_row() {
        let entry = make_entry("CreateUserDto", TypeDefinitionKind::Dto);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| CreateUserDto | dto |"), "missing dto row");
    }

    #[test]
    fn test_render_command_entry_row() {
        let entry = make_entry("CreateUserCommand", TypeDefinitionKind::Command);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| CreateUserCommand | command |"), "missing command row");
    }

    #[test]
    fn test_render_query_entry_row() {
        let entry = make_entry("GetUserQuery", TypeDefinitionKind::Query);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| GetUserQuery | query |"), "missing query row");
    }

    #[test]
    fn test_render_factory_entry_row() {
        let entry = make_entry("UserFactory", TypeDefinitionKind::Factory);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
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
                },
            ),
            make_entry(
                "TrackStatus",
                TypeDefinitionKind::Enum {
                    expected_variants: vec!["Planned".into(), "Done".into()],
                },
            ),
            make_entry("TrackId", TypeDefinitionKind::ValueObject),
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
            make_entry("SaveUseCase", TypeDefinitionKind::UseCase),
            make_entry("SaveInteractor", TypeDefinitionKind::Interactor),
            make_entry("SaveDto", TypeDefinitionKind::Dto),
            make_entry("SaveCommand", TypeDefinitionKind::Command),
            make_entry("GetQuery", TypeDefinitionKind::Query),
            make_entry("AggFactory", TypeDefinitionKind::Factory),
        ];
        let doc = make_doc(entries);
        let output = render_type_catalogue(&doc, "domain-types.json");

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
        let entry = make_entry("Draft", TypeDefinitionKind::ValueObject);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("\u{2014}"), "missing em-dash for unevaluated signal");
    }

    #[test]
    fn test_render_signal_column_shows_blue_when_signal_blue() {
        let entry = make_entry("Draft", TypeDefinitionKind::ValueObject);
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
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("\u{1f535}"), "missing blue circle for Blue signal");
    }

    #[test]
    fn test_render_signal_column_shows_red_when_signal_red() {
        let entry = make_entry("Ghost", TypeDefinitionKind::ValueObject);
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
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("\u{1f534}"), "missing red circle for Red signal");
    }

    #[test]
    fn test_render_multiple_entries_all_present() {
        let entries = vec![
            make_entry(
                "Draft",
                TypeDefinitionKind::Typestate {
                    transitions: TypestateTransitions::To(vec!["Published".into()]),
                },
            ),
            make_entry(
                "TrackStatus",
                TypeDefinitionKind::Enum { expected_variants: vec!["Planned".into()] },
            ),
            make_entry("TrackId", TypeDefinitionKind::ValueObject),
        ];
        let doc = make_doc(entries);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("Draft"), "missing Draft");
        assert!(output.contains("TrackStatus"), "missing TrackStatus");
        assert!(output.contains("TrackId"), "missing TrackId");
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue: Action column
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_add_action_shows_dash() {
        let entry = make_entry("Foo", TypeDefinitionKind::ValueObject);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        // Add action renders as em-dash
        assert!(output.contains("| \u{2014} |"), "Add action should show em-dash");
    }

    #[test]
    fn test_render_delete_action_shows_delete() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "deleted",
            TypeDefinitionKind::ValueObject,
            domain::TypeAction::Delete,
            true,
        )
        .unwrap();
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.contains("| delete |"), "Delete action should show 'delete'");
    }

    #[test]
    fn test_render_output_ends_with_newline() {
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc, "domain-types.json");
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }
}
