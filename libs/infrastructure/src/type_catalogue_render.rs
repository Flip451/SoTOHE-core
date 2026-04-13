//! Renderer for the per-layer type catalogue markdown view (e.g.
//! `domain-types.md`, a read-only view of `TypeCatalogueDocument`).
//!
//! Produces a markdown file with:
//! - A generated-view header comment
//! - A `## Type Declarations` section with a table: Name | Kind | Details | Signal
//!
//! The Details column summarises kind-specific payload:
//! - Typestate: `→ A, → B` (declared transitions)
//! - Enum / ErrorType: `A | B | C` (expected variants)
//! - TraitPort: `fn a, fn b` (expected methods)
//! - ValueObject: `—`
//!
//! The Signal column shows `🔵` / `🔴` / `—` (no signal yet).

use domain::{
    ConfidenceSignal, TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
    TypestateTransitions,
};

/// Renders the full `domain-types.md` document for a `TypeCatalogueDocument`.
///
/// Returns a markdown string suitable for writing to `domain-types.md`.
#[must_use]
pub fn render_type_catalogue(doc: &TypeCatalogueDocument) -> String {
    let mut out = String::new();

    out.push_str("<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->\n");

    out.push_str("\n## Type Declarations\n\n");

    out.push_str("| Name | Kind | Action | Details | Signal |\n");
    out.push_str("|------|------|--------|---------|--------|\n");

    // Track consumed signal indices to handle delete+add pairs that share the same
    // (name, kind_tag) key. The first matching signal is consumed by the first entry;
    // any second entry with the same key skips past it to the next match.
    let mut consumed: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for entry in doc.entries() {
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
        TypeDefinitionKind::TraitPort { expected_methods } => {
            if expected_methods.is_empty() {
                "\u{2014}".to_owned()
            } else {
                expected_methods.iter().map(|m| m.signature_string()).collect::<Vec<_>>().join(", ")
            }
        }
        TypeDefinitionKind::ValueObject => "\u{2014}".to_owned(),
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
        let output = render_type_catalogue(&doc);
        assert!(
            output.contains("<!-- Generated from domain-types.json"),
            "missing generated header"
        );
    }

    #[test]
    fn test_render_type_catalogue_includes_section_heading() {
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc);
        assert!(output.contains("## Type Declarations\n"), "missing section heading");
    }

    #[test]
    fn test_render_type_catalogue_includes_table_header() {
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc);
        assert!(
            output.contains("| Name | Kind | Action | Details | Signal |"),
            "missing table header"
        );
        assert!(
            output.contains("|------|------|--------|---------|--------|"),
            "missing table separator"
        );
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
        let output = render_type_catalogue(&doc);
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
        let output = render_type_catalogue(&doc);
        assert!(output.contains("\u{2205} (terminal)"), "missing terminal marker");
    }

    #[test]
    fn test_render_enum_entry_row() {
        let entry = make_entry(
            "TrackStatus",
            TypeDefinitionKind::Enum { expected_variants: vec!["Planned".into(), "Done".into()] },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc);
        assert!(output.contains("| TrackStatus | enum |"), "missing enum row");
        assert!(output.contains("Planned, Done"), "missing enum variants");
    }

    #[test]
    fn test_render_value_object_entry_row() {
        let entry = make_entry("TrackId", TypeDefinitionKind::ValueObject);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc);
        assert!(output.contains("| TrackId | value_object |"), "missing value_object row");
    }

    #[test]
    fn test_render_error_type_entry_row() {
        let entry = make_entry(
            "SchemaExportError",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NightlyNotFound".into()] },
        );
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc);
        assert!(output.contains("| SchemaExportError | error_type |"), "missing error_type row");
        assert!(output.contains("NightlyNotFound"), "missing error variant");
    }

    #[test]
    fn test_render_trait_port_entry_row() {
        let entry = make_entry(
            "SchemaExporter",
            TypeDefinitionKind::TraitPort {
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
        let output = render_type_catalogue(&doc);
        assert!(output.contains("| SchemaExporter | trait_port |"), "missing trait_port row");
        assert!(output.contains("fn export"), "missing method");
    }

    // ---------------------------------------------------------------------------
    // render_type_catalogue: Signal column
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_signal_column_shows_dash_when_no_signals() {
        let entry = make_entry("Draft", TypeDefinitionKind::ValueObject);
        let doc = make_doc(vec![entry]);
        let output = render_type_catalogue(&doc);
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
        let output = render_type_catalogue(&doc);
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
        let output = render_type_catalogue(&doc);
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
        let output = render_type_catalogue(&doc);
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
        let output = render_type_catalogue(&doc);
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
        let output = render_type_catalogue(&doc);
        assert!(output.contains("| delete |"), "Delete action should show 'delete'");
    }

    #[test]
    fn test_render_output_ends_with_newline() {
        let doc = make_doc(vec![]);
        let output = render_type_catalogue(&doc);
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }
}
