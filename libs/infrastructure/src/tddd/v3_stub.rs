//! v3 [`CatalogueDocument`] → v2 [`TypeCatalogueDocument`] stub conversion.
//!
//! Converts a schema_version = 3 catalogue into a minimal stub
//! [`TypeCatalogueDocument`] for contract-map and `<layer>-types.md` rendering.
//! Cross-reference edges are OS-06 (out of scope) and are not reconstructed;
//! only entry names, role-mapped shapes, and grounding fields are preserved.

use std::path::Path;

use domain::tddd::catalogue::TypeDefinitionKind;
use domain::tddd::catalogue::{TypeAction, TypeCatalogueDocument, TypeCatalogueEntry};
use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Converts a v3 [`CatalogueDocument`] into a minimal stub [`TypeCatalogueDocument`]
/// for contract-map rendering and `<layer>-types.md` rendering.
///
/// Cross-reference edges are OS-06 (out of scope) and are not reconstructed —
/// the stub entries carry no `expected_methods`, `implements`, or variant
/// payload types, so no edges are emitted by the contract-map renderer. The node
/// shapes (mermaid shapes derived from `TypeDefinitionKind`) are preserved by
/// mapping v3 roles to the closest v2 kind variant.
///
/// Each entry's `ItemAction` is preserved (mapped to the corresponding `TypeAction`)
/// so that `render_type_catalogue` can render action markers (`modify`, `delete`,
/// etc.) in the `<layer>-types.md` output.
///
/// Returns `Err(reason)` if any entry name fails [`TypeCatalogueEntry::new`]
/// validation (empty/whitespace-only name). This should not occur in practice
/// because v3 `TypeName`/`TraitName`/`FunctionPath` are validated newtypes, but
/// the error is surfaced rather than silently dropped so that malformed catalogues
/// are diagnosed rather than producing truncated rendering output.
///
/// Exposed as `pub(crate)` so that `track::render` can use the same v3→stub
/// conversion when rendering `<layer>-types.md` for schema_version = 3 catalogues.
pub(crate) fn v3_doc_to_stub(
    doc: &domain::tddd::catalogue_v2::CatalogueDocument,
) -> Result<TypeCatalogueDocument, String> {
    let mut entries: Vec<TypeCatalogueEntry> = Vec::new();

    // Type entries: DataRole → TypeDefinitionKind stub
    // Grounding fields (spec_refs, informal_grounds) are copied to the stub so
    // that CatalogueSpecSignal evaluation uses the same data source as v2.
    for (type_name, type_entry) in &doc.types {
        let kind = data_role_to_kind(type_entry.role, &type_entry.kind);
        let action = item_action_to_type_action(type_entry.action);
        let entry = TypeCatalogueEntry::with_refs(
            type_name.as_str(),
            type_entry.docs.as_deref().unwrap_or(""),
            kind,
            action,
            true,
            type_entry.spec_refs.clone(),
            type_entry.informal_grounds.clone(),
        )
        .map_err(|e| format!("type entry '{type_name}': {e}"))?;
        entries.push(entry);
    }

    // Trait entries: ContractRole → TypeDefinitionKind stub
    for (trait_name, trait_entry) in &doc.traits {
        let kind = contract_role_to_kind(trait_entry.role);
        let action = item_action_to_type_action(trait_entry.action);
        let entry = TypeCatalogueEntry::with_refs(
            trait_name.as_str(),
            trait_entry.docs.as_deref().unwrap_or(""),
            kind,
            action,
            true,
            trait_entry.spec_refs.clone(),
            trait_entry.informal_grounds.clone(),
        )
        .map_err(|e| format!("trait entry '{trait_name}': {e}"))?;
        entries.push(entry);
    }

    // Function entries: FunctionRole → FreeFunction stub.
    // T012 ensures that all function paths in a decoded v3 document already carry
    // the catalogue's own crate_name prefix — cross-crate paths are rejected at
    // decode time.  No further filtering is needed here.
    for (fn_path, fn_entry) in &doc.functions {
        let kind = function_role_to_kind(fn_entry.role);
        let action = item_action_to_type_action(fn_entry.action);
        let entry = TypeCatalogueEntry::with_refs(
            fn_path.to_string(),
            fn_entry.docs.as_deref().unwrap_or(""),
            kind,
            action,
            true,
            fn_entry.spec_refs.clone(),
            fn_entry.informal_grounds.clone(),
        )
        .map_err(|e| format!("function entry '{fn_path}': {e}"))?;
        entries.push(entry);
    }

    Ok(TypeCatalogueDocument::new(3, entries))
}

// ---------------------------------------------------------------------------
// Filename stem helper
// ---------------------------------------------------------------------------

/// Extract the filename stem (portion before `-types.json`).
///
/// Returns `Some("domain")` for `"domain-types.json"`, `None` when the path
/// has no filename.
pub(crate) fn derive_filename_stem(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_str()?;
    Some(
        filename
            .strip_suffix("-types.json")
            .map(str::to_owned)
            .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned()),
    )
}

// ---------------------------------------------------------------------------
// Role → TypeDefinitionKind mapping helpers
// ---------------------------------------------------------------------------

/// Convert a v3 [`ItemAction`] to the corresponding v2 [`TypeAction`].
///
/// The two enums have identical semantics and variant names; this function
/// performs the structural mapping so the rendered `<layer>-types.md` table
/// shows the correct action markers (e.g. `modify`, `delete`) for v3 entries.
pub(crate) fn item_action_to_type_action(action: ItemAction) -> TypeAction {
    match action {
        ItemAction::Add => TypeAction::Add,
        ItemAction::Modify => TypeAction::Modify,
        ItemAction::Reference => TypeAction::Reference,
        ItemAction::Delete => TypeAction::Delete,
    }
}

/// Map a v3 `DataRole` (+ `TypeKindV2` for typestate detection) to the closest
/// v2 `TypeDefinitionKind` stub for node-shape rendering.
pub(crate) fn data_role_to_kind(role: DataRole, kind_v2: &TypeKindV2) -> TypeDefinitionKind {
    // Typestate detection: PlainStruct with a typestate marker → Typestate stub.
    if let TypeKindV2::PlainStruct { typestate: Some(_), .. } = kind_v2 {
        return TypeDefinitionKind::Typestate {
            transitions: domain::tddd::catalogue::TypestateTransitionsSpec::Terminal,
            expected_members: vec![],
            expected_methods: vec![],
        };
    }
    // Enum kind → Enum stub (role ignored for shape selection, ErrorType excepted).
    if matches!(kind_v2, TypeKindV2::Enum { .. }) && matches!(role, DataRole::ErrorType) {
        return TypeDefinitionKind::ErrorType { expected_variants: vec![] };
    }
    if matches!(kind_v2, TypeKindV2::Enum { .. }) {
        return TypeDefinitionKind::Enum { expected_variants: vec![] };
    }
    // UnitStruct / TupleStruct / PlainStruct / TypeAlias → role-based shape.
    match role {
        DataRole::ValueObject
        | DataRole::Entity
        | DataRole::AggregateRoot
        | DataRole::Specification => {
            TypeDefinitionKind::ValueObject { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::DomainService => {
            TypeDefinitionKind::DomainService { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::Factory => {
            TypeDefinitionKind::Factory { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::UseCase => {
            TypeDefinitionKind::UseCase { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::Interactor => TypeDefinitionKind::Interactor {
            expected_members: vec![],
            expected_methods: vec![],
            declares_application_service: vec![],
        },
        DataRole::Command => {
            TypeDefinitionKind::Command { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::Query => {
            TypeDefinitionKind::Query { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::Dto => {
            TypeDefinitionKind::Dto { expected_members: vec![], expected_methods: vec![] }
        }
        DataRole::ErrorType => TypeDefinitionKind::ErrorType { expected_variants: vec![] },
        DataRole::SecondaryAdapter => TypeDefinitionKind::SecondaryAdapter {
            expected_members: vec![],
            expected_methods: vec![],
            implements: vec![],
        },
    }
}

/// Map a v3 `ContractRole` to the closest v2 `TypeDefinitionKind` stub.
pub(crate) fn contract_role_to_kind(role: ContractRole) -> TypeDefinitionKind {
    match role {
        ContractRole::SecondaryPort | ContractRole::SpecificationPort => {
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }
        }
        ContractRole::ApplicationService => {
            TypeDefinitionKind::ApplicationService { expected_methods: vec![] }
        }
    }
}

/// Map a v3 `FunctionRole` to the v2 `FreeFunction` stub.
pub(crate) fn function_role_to_kind(_role: FunctionRole) -> TypeDefinitionKind {
    TypeDefinitionKind::FreeFunction {
        module_path: None,
        expected_params: vec![],
        expected_returns: vec![],
        expected_is_async: false,
    }
}
