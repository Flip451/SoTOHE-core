//! `import` shape resolution (D3): reuse the existing rustdoc extraction to
//! resolve an existing type's current shape, and build a catalogue entry from
//! it.
//!
//! Reuse decision: there is no per-type rustdoc resolver in the tree — the only
//! extraction path is the whole-crate `cargo +nightly rustdoc` exporter behind
//! `sotp domain export-schema` ([`RustdocSchemaExporter`]). `resolve_shape`
//! reuses that exporter and indexes the resulting [`SchemaExport`] for the
//! requested type. That path needs the nightly toolchain on the host, so it is
//! not exercised by unit tests; the pure `SchemaExport`→entry mapping
//! ([`build_import_entry`]) is unit-tested with in-memory shapes instead.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use domain::plan_ref::SpecElementId;
use domain::schema::{
    FunctionInfo, SchemaExport, SchemaExporter, StructShapeKind, TypeInfo, TypeKind,
};
use domain::tddd::catalog_gen::CatalogImportAction;
use domain::tddd::catalogue::MemberDeclaration;
use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeName};
use serde_json::{Map, Value, json};
use usecase::catalog_gen::{CatalogError, CatalogImportCommand};

use super::fs_access::{port_error, schema_error};
use super::validate::spec_refs_value;
use crate::schema_export::RustdocSchemaExporter;

/// The rustdoc-resolved current shape of an existing type.
pub(super) struct ImportedShape {
    /// Crate-relative module path (e.g. `tddd::foo`).
    pub(super) module_path: String,
    /// Short type name (the catalogue entry key).
    pub(super) name: String,
    /// Pre-built `kind` node.
    pub(super) kind: Value,
    /// Pre-built inherent method-declaration nodes.
    pub(super) methods: Vec<Value>,
}

/// Resolve the current shape of `type_path` from rustdoc extraction.
///
/// # Errors
///
/// Returns [`CatalogError::Port`] on rustdoc extraction failure, or
/// [`CatalogError::SchemaInvalid`] when the type path is malformed or absent.
pub(super) fn resolve_shape(
    workspace_root: &Path,
    type_path: &str,
) -> Result<ImportedShape, CatalogError> {
    let (crate_name, module, name) = parse_type_path(type_path)?;
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let schema = exporter.export(&crate_name).map_err(|err| {
        port_error(format!("rustdoc extraction for crate `{crate_name}` failed: {err}"))
    })?;
    let type_info = select_type(&schema, &crate_name, &module, &name).ok_or_else(|| {
        schema_error(format!(
            "type `{type_path}` not found in crate `{crate_name}` at that exact module path"
        ))
    })?;
    let kind = kind_value(type_info);
    let methods = collect_methods(&schema, &name, type_info.module_path());
    Ok(ImportedShape { module_path: module, name, kind, methods })
}

/// Build a catalogue entry from a resolved shape and the import command.
///
/// The `role` and `docs` nodes are `$todo` holes (they are design annotations
/// not derivable from rustdoc); `action` reflects reference / modify / delete.
///
/// # Errors
///
/// Returns a [`CatalogError`] when an anchor does not resolve.
pub(super) fn build_import_entry(
    command: &CatalogImportCommand,
    shape: &ImportedShape,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<Value, CatalogError> {
    let spec_refs = spec_refs_value(&command.anchors, spec_file, spec_anchors)?;
    let action = match command.action {
        CatalogImportAction::Reference => "reference",
        CatalogImportAction::Modify => "modify",
        CatalogImportAction::Delete => "delete",
    };
    let mut entry = Map::new();
    entry.insert("action".to_owned(), json!(action));
    entry.insert(
        "role".to_owned(),
        json!({ "$todo": "assign the architectural role for this entry" }),
    );
    entry.insert("kind".to_owned(), shape.kind.clone());
    entry.insert("methods".to_owned(), Value::Array(shape.methods.clone()));
    entry.insert("module_path".to_owned(), json!(shape.module_path));
    entry
        .insert("docs".to_owned(), json!({ "$todo": "one-line doc comment describing this item" }));
    entry.insert("spec_refs".to_owned(), spec_refs);
    entry.insert("informal_grounds".to_owned(), json!([]));
    Ok(Value::Object(entry))
}

/// Build an identity-only deletion tombstone entry from `type_path`.
///
/// A `delete` import records only the removed type's identity (its name + module
/// path), so — unlike [`build_import_entry`] — it neither resolves the (expensive)
/// rustdoc shape nor emits `role` / `docs` `$todo` holes. The returned
/// `(name, entry)` is inserted into the `types` map with `action: "delete"`; the
/// catalogue codec routes it to `CatalogueDocument::deletions`. A crate-root type
/// omits `module_path` entirely. See spec IN-04, GO-03, AC-04.
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] when `type_path` is not crate-qualified.
pub(super) fn build_delete_entry(type_path: &str) -> Result<(String, Value), CatalogError> {
    let (_crate_name, module, name) = parse_type_path(type_path)?;
    let mut entry = Map::new();
    entry.insert("action".to_owned(), json!("delete"));
    if !module.is_empty() {
        entry.insert("module_path".to_owned(), json!(module));
    }
    Ok((name, Value::Object(entry)))
}

/// Split a crate-qualified type path into `(crate, module, name)`.
pub(super) fn parse_type_path(type_path: &str) -> Result<(String, String, String), CatalogError> {
    let segments: Vec<&str> =
        type_path.split("::").map(str::trim).filter(|segment| !segment.is_empty()).collect();
    if segments.len() < 2 {
        return Err(schema_error(format!(
            "type path `{type_path}` must be crate-qualified, e.g. `domain::Foo`"
        )));
    }
    let crate_name = segments.first().copied().unwrap_or_default().to_owned();
    let name = segments.last().copied().unwrap_or_default().to_owned();
    let module = segments
        .get(1..segments.len().saturating_sub(1))
        .map(|middle| middle.join("::"))
        .unwrap_or_default();
    CrateName::new(crate_name.clone()).map_err(|e| {
        schema_error(format!("type path `{type_path}` has invalid crate name `{crate_name}`: {e}"))
    })?;
    if !module.is_empty() {
        ModulePath::from_str(&module).map_err(|e| {
            schema_error(format!("type path `{type_path}` has invalid module path `{module}`: {e}"))
        })?;
    }
    TypeName::new(name.clone()).map_err(|e| {
        schema_error(format!("type path `{type_path}` has invalid type name `{name}`: {e}"))
    })?;
    Ok((crate_name, module, name))
}

/// Select the [`TypeInfo`] whose short name is `name` and whose crate-qualified
/// module path matches `crate_name` + `module` exactly, segment for segment.
///
/// Rustdoc module paths are crate-qualified (e.g. `domain::tddd`), so the
/// requested crate and module are compared as one segment list against the
/// type's module path. There is no short-name fallback and no `ends_with`
/// suffix matching: a request that does not name the exact module resolves to
/// `None` rather than an arbitrary same-named type elsewhere in the crate
/// (which would otherwise let `foo::Order` collide with `bar_foo::Order`).
fn select_type<'a>(
    schema: &'a SchemaExport,
    crate_name: &str,
    module: &str,
    name: &str,
) -> Option<&'a TypeInfo> {
    let expected = expected_module_segments(crate_name, module);
    schema.types().iter().find(|type_info| {
        type_info.name() == name && module_segments(type_info.module_path()) == expected
    })
}

/// The crate-qualified module segments a requested `crate_name` + `module`
/// denotes (the crate is always the leading segment).
fn expected_module_segments<'a>(crate_name: &'a str, module: &'a str) -> Vec<&'a str> {
    let mut segments = vec![crate_name];
    segments.extend(path_segments(module));
    segments
}

/// The segments of a rustdoc module path, or empty when the path is unknown.
fn module_segments(module_path: Option<&str>) -> Vec<&str> {
    module_path.map(path_segments).unwrap_or_default()
}

/// Split a `::`-separated path into its non-empty segments.
fn path_segments(path: &str) -> Vec<&str> {
    path.split("::").filter(|segment| !segment.is_empty()).collect()
}

/// Build the `kind` node from a [`TypeInfo`].
fn kind_value(type_info: &TypeInfo) -> Value {
    match type_info.kind() {
        TypeKind::Struct => {
            json!({ "kind": "struct", "shape": struct_shape_value(type_info) })
        }
        TypeKind::Enum => {
            let variants: Vec<Value> = type_info.members().iter().map(variant_value).collect();
            json!({ "kind": "enum", "variants": variants })
        }
        TypeKind::TypeAlias => match type_info.alias_target() {
            Some(target) => json!({ "kind": "type_alias", "target": target }),
            None => json!({
                "kind": "type_alias",
                "target": { "$todo": "the aliased type (not captured by extraction)" }
            }),
        },
    }
}

/// Build the `shape` node for a struct.
///
/// Prefers the shape discriminant captured by rustdoc extraction
/// ([`TypeInfo::struct_shape`]): the member list alone cannot tell a unit struct
/// apart from a plain / tuple struct whose fields are all private, since both
/// extract to an empty member list. A rustdoc-resolved struct therefore carries
/// the authoritative shape plus its `has_stripped_fields` flag, so an all-private
/// struct is emitted as `plain` / `tuple` (not `unit`) and its hidden state is
/// surfaced. When the shape was not captured (`None` — e.g. an in-memory
/// `TypeInfo` built without extraction), the shape is reconstructed from the
/// member field names.
fn struct_shape_value(type_info: &TypeInfo) -> Value {
    let members = type_info.members();
    match type_info.struct_shape() {
        Some(StructShapeKind::Unit) => json!({ "kind": "unit" }),
        Some(StructShapeKind::Tuple { has_stripped_fields }) => json!({
            "kind": "tuple",
            "fields": tuple_field_values(members),
            "has_stripped_fields": *has_stripped_fields,
        }),
        Some(StructShapeKind::Plain { has_stripped_fields }) => json!({
            "kind": "plain",
            "fields": plain_field_values(members),
            "has_stripped_fields": *has_stripped_fields,
        }),
        None => struct_shape_from_members(members),
    }
}

/// Positional tuple-field type nodes (bare type strings, no `name` key).
fn tuple_field_values(members: &[MemberDeclaration]) -> Vec<Value> {
    members.iter().filter_map(|member| member.ty().map(|ty| json!(ty))).collect()
}

/// Named plain-field nodes (`{ "name", "ty" }`).
fn plain_field_values(members: &[MemberDeclaration]) -> Vec<Value> {
    members
        .iter()
        .filter_map(|member| member.ty().map(|ty| json!({ "name": member.name(), "ty": ty })))
        .collect()
}

/// Reconstruct a struct `shape` node from its members when the rustdoc shape
/// discriminant was not captured (e.g. an in-memory `TypeInfo`).
///
/// Tuple fields are extracted with positional names (`"0"`, `"1"`, …) that are
/// not valid Rust identifiers, plain fields keep their declared names, and an
/// empty member list is treated as a unit struct. Emitting the matching shape
/// keeps a tuple / unit struct from being written as a `plain` struct with
/// numeric field names, which the catalogue codec rejects because a plain
/// `FieldName` must be a Rust identifier. Without the captured discriminant an
/// all-private plain / tuple struct is indistinguishable from a unit struct;
/// `has_stripped_fields` therefore defaults to `false` on this fallback path.
fn struct_shape_from_members(members: &[MemberDeclaration]) -> Value {
    if members.is_empty() {
        return json!({ "kind": "unit" });
    }
    if members.iter().all(|member| member.name().parse::<usize>().is_ok()) {
        return json!({
            "kind": "tuple",
            "fields": tuple_field_values(members),
            "has_stripped_fields": false,
        });
    }
    json!({
        "kind": "plain",
        "fields": plain_field_values(members),
        "has_stripped_fields": false,
    })
}

/// Build a `VariantDecl` node from an enum member.
fn variant_value(member: &MemberDeclaration) -> Value {
    match member {
        MemberDeclaration::Variant(variant) if !variant.payload_types().is_empty() => {
            let fields: Vec<&str> = variant.payload_types().iter().map(String::as_str).collect();
            json!({ "name": member.name(), "payload": { "kind": "tuple", "fields": fields } })
        }
        _ => json!({ "name": member.name(), "payload": { "kind": "unit" } }),
    }
}

/// Collect inherent (non-trait) methods for the type named `name` living in
/// `module_path` from the schema's impls.
///
/// Impls are matched on both the target's short name and its crate-qualified
/// module path (segment for segment), so a same-short-named type in another
/// module (e.g. `shop::bar_foo::Order` vs `shop::foo::Order`) does not
/// contribute its methods. A generic type's impl target is recorded with its
/// generic arguments (`impl<T> Foo<T>` → `Foo<T>`), so the target is compared
/// after stripping that argument list against the bare catalogue entry name
/// `Foo`. The impl's target module path is `None` when extraction could not
/// resolve it, in which case only a request whose module is likewise unresolved
/// matches.
fn collect_methods(schema: &SchemaExport, name: &str, module_path: Option<&str>) -> Vec<Value> {
    let expected = module_segments(module_path);
    let mut methods = Vec::new();
    for impl_info in schema.impls() {
        if strip_generic_args(impl_info.target_type()) == name
            && impl_info.trait_name().is_none()
            && module_segments(impl_info.target_module_path()) == expected
        {
            for func in impl_info.methods() {
                methods.push(method_value(func));
            }
        }
    }
    methods
}

/// The bare type name of an impl target, with any trailing generic-argument
/// list stripped (`Foo<T, U>` → `Foo`).
///
/// The schema exporter renders a generic impl's target verbatim, including its
/// generic parameters, but a catalogue entry is keyed by the bare type name, so
/// the two are compared after normalisation.
fn strip_generic_args(target_type: &str) -> &str {
    match target_type.split_once('<') {
        Some((base, _)) => base.trim_end(),
        None => target_type,
    }
}

/// Build a `MethodDeclaration` node from a [`FunctionInfo`].
fn method_value(func: &FunctionInfo) -> Value {
    let params: Vec<Value> = func
        .params()
        .iter()
        .map(|param| json!({ "name": param.name.as_str(), "ty": param.ty.as_str() }))
        .collect();
    let mut method = Map::new();
    method.insert("name".to_owned(), json!(func.name()));
    if let Some(receiver) = func.receiver() {
        method.insert("receiver".to_owned(), json!(receiver));
    }
    method.insert("params".to_owned(), Value::Array(params));
    method.insert("returns".to_owned(), json!(func.returns()));
    method.insert("is_async".to_owned(), json!(func.is_async()));
    method.insert("has_default_impl".to_owned(), json!(false));
    Value::Object(method)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use domain::tddd::LayerId;

    fn sample_shape() -> ImportedShape {
        ImportedShape {
            module_path: "tddd".to_owned(),
            name: "LayerId".to_owned(),
            kind: json!({
                "kind": "struct",
                "shape": { "kind": "plain", "fields": [{ "name": "value", "ty": "String" }], "has_stripped_fields": false }
            }),
            methods: vec![],
        }
    }

    fn import_command(action: CatalogImportAction) -> CatalogImportCommand {
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::tddd::LayerId".to_owned(),
            action,
            anchors: vec![],
        }
    }

    #[test]
    fn test_parse_type_path() {
        let (crate_name, module, name) = parse_type_path("domain::tddd::LayerId").unwrap();
        assert_eq!(crate_name, "domain");
        assert_eq!(module, "tddd");
        assert_eq!(name, "LayerId");
    }

    #[test]
    fn test_parse_type_path_two_segments() {
        let (crate_name, module, name) = parse_type_path("domain::LayerId").unwrap();
        assert_eq!(crate_name, "domain");
        assert_eq!(module, "");
        assert_eq!(name, "LayerId");
    }

    #[test]
    fn test_parse_type_path_rejects_bare_name() {
        assert!(parse_type_path("LayerId").is_err());
    }

    #[test]
    fn test_build_delete_entry_is_identity_only() {
        // A delete import emits an identity-only tombstone: action + module_path,
        // with no role / docs / kind / methods holes to fill in.
        let (name, entry) = build_delete_entry("domain::tddd::LayerId").unwrap();
        assert_eq!(name, "LayerId");
        assert_eq!(entry["action"], json!("delete"));
        assert_eq!(entry["module_path"], json!("tddd"));
        assert!(entry.get("role").is_none(), "delete tombstone must not carry a role");
        assert!(entry.get("docs").is_none(), "delete tombstone must not carry docs");
        assert!(entry.get("kind").is_none(), "delete tombstone must not carry a kind");
        assert!(entry.get("methods").is_none(), "delete tombstone must not carry methods");
    }

    #[test]
    fn test_build_delete_entry_root_module_omits_module_path() {
        let (name, entry) = build_delete_entry("domain::Foo").unwrap();
        assert_eq!(name, "Foo");
        assert_eq!(entry["action"], json!("delete"));
        assert!(entry.get("module_path").is_none(), "crate-root delete omits module_path");
    }

    #[test]
    fn test_build_delete_entry_rejects_bare_name() {
        assert!(build_delete_entry("LayerId").is_err());
    }

    #[test]
    fn test_build_delete_entry_rejects_invalid_module_path() {
        let result = build_delete_entry("domain::bad-segment::LayerId");
        assert!(result.is_err(), "invalid module_path must be rejected before writing");
    }

    #[test]
    fn test_build_delete_entry_rejects_invalid_type_name() {
        let result = build_delete_entry("domain::tddd::Layer-Id");
        assert!(result.is_err(), "invalid type name must be rejected before writing");
    }

    #[test]
    fn test_build_import_entry_reference_includes_shape() {
        let entry = build_import_entry(
            &import_command(CatalogImportAction::Reference),
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(entry["action"], json!("reference"));
        assert_eq!(entry["kind"]["kind"], json!("struct"));
        assert_eq!(entry["module_path"], json!("tddd"));
        assert!(entry["role"].get("$todo").is_some());
    }

    #[test]
    fn test_build_import_entry_modify_action() {
        let entry = build_import_entry(
            &import_command(CatalogImportAction::Modify),
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(entry["action"], json!("modify"));
        assert_eq!(entry["kind"]["shape"]["fields"][0]["name"], json!("value"));
    }

    #[test]
    fn test_build_import_entry_delete_action() {
        let entry = build_import_entry(
            &import_command(CatalogImportAction::Delete),
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(entry["action"], json!("delete"));
    }

    // Finding 3: exact segment-wise module match — `foo` must not resolve the
    // same-named type living in `bar_foo` via an `ends_with` suffix collision.
    #[test]
    fn test_select_type_exact_module_avoids_suffix_collision() {
        let in_foo = TypeInfo::with_module_path(
            "Order".to_owned(),
            TypeKind::Struct,
            None,
            vec![],
            "shop::foo".to_owned(),
        );
        let in_bar_foo = TypeInfo::with_module_path(
            "Order".to_owned(),
            TypeKind::Struct,
            None,
            vec![],
            "shop::bar_foo".to_owned(),
        );
        let schema =
            SchemaExport::new("shop".to_owned(), vec![in_bar_foo, in_foo], vec![], vec![], vec![]);
        let selected = select_type(&schema, "shop", "foo", "Order").unwrap();
        assert_eq!(selected.module_path(), Some("shop::foo"));
    }

    // Finding 3: an inexact request must not fall back to the sole same-named
    // type in another module.
    #[test]
    fn test_select_type_no_short_name_fallback() {
        let ti = TypeInfo::with_module_path(
            "LayerId".to_owned(),
            TypeKind::Struct,
            None,
            vec![],
            "domain::tddd".to_owned(),
        );
        let schema = SchemaExport::new("domain".to_owned(), vec![ti], vec![], vec![], vec![]);
        assert!(select_type(&schema, "domain", "review", "LayerId").is_none());
        assert!(select_type(&schema, "domain", "", "LayerId").is_none());
        assert!(select_type(&schema, "domain", "tddd", "LayerId").is_some());
    }

    // Finding 2: inherent methods must be attributed by the target type's exact
    // module path, not its short name — `shop::foo::Order` must not absorb the
    // methods of the same-short-named `shop::bar_foo::Order`.
    #[test]
    fn test_collect_methods_disambiguates_by_target_module_path() {
        use domain::schema::ImplInfo;

        let inherent = |method_name: &str, module: &str| {
            ImplInfo::with_target_details(
                "Order".to_owned(),
                None,
                vec![FunctionInfo::new(
                    method_name.to_owned(),
                    None,
                    vec![],
                    false,
                    vec![],
                    "()".to_owned(),
                    None,
                    false,
                )],
                None,
                Some(module.to_owned()),
            )
        };
        let schema = SchemaExport::new(
            "shop".to_owned(),
            vec![],
            vec![],
            vec![],
            vec![inherent("in_foo", "shop::foo"), inherent("in_bar", "shop::bar_foo")],
        );

        let methods = collect_methods(&schema, "Order", Some("shop::foo"));
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0]["name"], json!("in_foo"));
    }

    // Finding (round 4): a tuple struct (`struct Pair(u32, String)`) is extracted
    // with positional field names `"0"` / `"1"`. It must be emitted as a `tuple`
    // shape carrying bare type strings, not a `plain` shape whose numeric field
    // names the catalogue codec would reject.
    #[test]
    fn test_kind_value_tuple_struct_uses_tuple_shape() {
        let ti = TypeInfo::new(
            "Pair".to_owned(),
            TypeKind::Struct,
            None,
            vec![MemberDeclaration::field("0", "u32"), MemberDeclaration::field("1", "String")],
        );
        let kind = kind_value(&ti);
        assert_eq!(kind["kind"], json!("struct"));
        assert_eq!(kind["shape"]["kind"], json!("tuple"));
        assert_eq!(kind["shape"]["fields"], json!(["u32", "String"]));
        // Tuple fields are positional strings — no `name` key, so the invalid
        // `"0"` / `"1"` field names never reach the codec.
        assert!(kind["shape"]["fields"][0].get("name").is_none());
    }

    // Finding (round 4): a unit struct (`struct Marker;`) has no members and must
    // be emitted as a `unit` shape, not a `plain` shape with zero fields.
    #[test]
    fn test_kind_value_unit_struct_uses_unit_shape() {
        let ti = TypeInfo::new("Marker".to_owned(), TypeKind::Struct, None, vec![]);
        let kind = kind_value(&ti);
        assert_eq!(kind["shape"]["kind"], json!("unit"));
        assert!(kind["shape"].get("fields").is_none());
    }

    // A named-field struct is unchanged: it stays a `plain` shape keyed by field
    // name.
    #[test]
    fn test_kind_value_plain_struct_uses_plain_shape() {
        let ti = TypeInfo::new(
            "Config".to_owned(),
            TypeKind::Struct,
            None,
            vec![MemberDeclaration::field("path", "String")],
        );
        let kind = kind_value(&ti);
        assert_eq!(kind["shape"]["kind"], json!("plain"));
        assert_eq!(kind["shape"]["fields"][0]["name"], json!("path"));
        assert_eq!(kind["shape"]["fields"][0]["ty"], json!("String"));
    }

    // Finding F1 (round 12): a plain struct whose fields are ALL private is
    // extracted with an empty member list but a `StructShapeKind::Plain` hint.
    // It must be emitted as a `plain` shape with an empty field list and
    // `has_stripped_fields: true` — NOT misclassified as a `unit` struct, which
    // would wrongly claim the type has no hidden state.
    #[test]
    fn test_kind_value_stripped_plain_struct_uses_plain_shape() {
        let ti = TypeInfo::new("Opaque".to_owned(), TypeKind::Struct, None, vec![])
            .with_struct_shape(Some(StructShapeKind::Plain { has_stripped_fields: true }));
        let kind = kind_value(&ti);
        assert_eq!(kind["kind"], json!("struct"));
        assert_eq!(kind["shape"]["kind"], json!("plain"));
        assert_eq!(kind["shape"]["fields"], json!([]));
        assert_eq!(kind["shape"]["has_stripped_fields"], json!(true));
    }

    // Finding F1 (round 12): a tuple struct whose positional fields are all
    // private is likewise extracted with an empty member list but a
    // `StructShapeKind::Tuple` hint, and must be emitted as a `tuple` shape with
    // `has_stripped_fields: true` rather than collapsing to `unit`.
    #[test]
    fn test_kind_value_stripped_tuple_struct_uses_tuple_shape() {
        let ti = TypeInfo::new("Handle".to_owned(), TypeKind::Struct, None, vec![])
            .with_struct_shape(Some(StructShapeKind::Tuple { has_stripped_fields: true }));
        let kind = kind_value(&ti);
        assert_eq!(kind["shape"]["kind"], json!("tuple"));
        assert_eq!(kind["shape"]["fields"], json!([]));
        assert_eq!(kind["shape"]["has_stripped_fields"], json!(true));
    }

    // A partially public plain struct keeps its public field and still reports
    // `has_stripped_fields: true` from the captured shape hint (the member-only
    // fallback path hard-codes `false`, which would understate hidden state).
    #[test]
    fn test_kind_value_plain_struct_shape_hint_reports_stripped_flag() {
        let ti = TypeInfo::new(
            "Partial".to_owned(),
            TypeKind::Struct,
            None,
            vec![MemberDeclaration::field("visible", "String")],
        )
        .with_struct_shape(Some(StructShapeKind::Plain { has_stripped_fields: true }));
        let kind = kind_value(&ti);
        assert_eq!(kind["shape"]["kind"], json!("plain"));
        assert_eq!(kind["shape"]["fields"][0]["name"], json!("visible"));
        assert_eq!(kind["shape"]["has_stripped_fields"], json!(true));
    }

    // A `StructShapeKind::Unit` hint emits a `unit` shape with no `fields` node.
    #[test]
    fn test_kind_value_unit_shape_hint_uses_unit_shape() {
        let ti = TypeInfo::new("Marker".to_owned(), TypeKind::Struct, None, vec![])
            .with_struct_shape(Some(StructShapeKind::Unit));
        let kind = kind_value(&ti);
        assert_eq!(kind["shape"]["kind"], json!("unit"));
        assert!(kind["shape"].get("fields").is_none());
    }

    // Finding F1 (round 8): an existing type alias (`pub type MyAlias =
    // Vec<String>`) is resolved from rustdoc extraction, so its target is
    // emitted verbatim (`Vec<String>`) rather than left as a `$todo` hole that
    // would otherwise block the strict phase-2 / merge hole check.
    #[test]
    fn test_kind_value_type_alias_uses_rustdoc_target() {
        let ti = TypeInfo::new("MyAlias".to_owned(), TypeKind::TypeAlias, None, vec![])
            .with_alias_target(Some("Vec<String>".to_owned()));
        let kind = kind_value(&ti);
        assert_eq!(kind["kind"], json!("type_alias"));
        assert_eq!(kind["target"], json!("Vec<String>"));
        // The rustdoc-derived target replaces the `$todo` hole entirely.
        assert!(kind["target"].get("$todo").is_none());
    }

    // When extraction cannot resolve the alias target, the `$todo` hole is
    // retained so the annotation is still surfaced to the author.
    #[test]
    fn test_kind_value_type_alias_without_target_keeps_todo() {
        let ti = TypeInfo::new("MyAlias".to_owned(), TypeKind::TypeAlias, None, vec![]);
        let kind = kind_value(&ti);
        assert_eq!(kind["kind"], json!("type_alias"));
        assert!(kind["target"].get("$todo").is_some());
    }

    #[test]
    fn test_strip_generic_args_normalises_target() {
        assert_eq!(strip_generic_args("Foo"), "Foo");
        assert_eq!(strip_generic_args("Foo<T>"), "Foo");
        assert_eq!(strip_generic_args("Foo<T, U>"), "Foo");
        assert_eq!(strip_generic_args("Map<K, Vec<V>>"), "Map");
    }

    // Finding F2 (round 8): a generic type's impl target is recorded with its
    // generic arguments (`impl<T> Foo<T>` → target `Foo<T>`). Inherent methods
    // must still be attributed to the bare catalogue entry name `Foo`, so the
    // generic arguments are stripped before comparison; otherwise the rustdoc
    // methods are silently dropped from the generated catalogue.
    #[test]
    fn test_collect_methods_matches_generic_impl_target() {
        use domain::schema::ImplInfo;

        let generic_impl = ImplInfo::with_target_details(
            "Foo<T>".to_owned(),
            None,
            vec![FunctionInfo::new(
                "get".to_owned(),
                None,
                vec![],
                true,
                vec![],
                "&T".to_owned(),
                Some("&self".to_owned()),
                false,
            )],
            None,
            Some("crate_root::foo".to_owned()),
        );
        let schema =
            SchemaExport::new("crate_root".to_owned(), vec![], vec![], vec![], vec![generic_impl]);

        let methods = collect_methods(&schema, "Foo", Some("crate_root::foo"));
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0]["name"], json!("get"));
    }
}
