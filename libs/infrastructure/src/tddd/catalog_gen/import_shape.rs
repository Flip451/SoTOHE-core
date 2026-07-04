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

use domain::plan_ref::SpecElementId;
use domain::schema::{FunctionInfo, SchemaExport, SchemaExporter, TypeInfo, TypeKind};
use domain::tddd::catalog_gen::CatalogImportAction;
use domain::tddd::catalogue::MemberDeclaration;
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

/// Split a crate-qualified type path into `(crate, module, name)`.
fn parse_type_path(type_path: &str) -> Result<(String, String, String), CatalogError> {
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
            let fields: Vec<Value> = type_info
                .members()
                .iter()
                .filter_map(|member| {
                    member.ty().map(|ty| json!({ "name": member.name(), "ty": ty }))
                })
                .collect();
            json!({
                "kind": "struct",
                "shape": { "kind": "plain", "fields": fields, "has_stripped_fields": false }
            })
        }
        TypeKind::Enum => {
            let variants: Vec<Value> = type_info.members().iter().map(variant_value).collect();
            json!({ "kind": "enum", "variants": variants })
        }
        TypeKind::TypeAlias => json!({
            "kind": "type_alias",
            "target": { "$todo": "the aliased type (not captured by extraction)" }
        }),
    }
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
/// contribute its methods. The impl's target module path is `None` when
/// extraction could not resolve it, in which case only a request whose module
/// is likewise unresolved matches.
fn collect_methods(schema: &SchemaExport, name: &str, module_path: Option<&str>) -> Vec<Value> {
    let expected = module_segments(module_path);
    let mut methods = Vec::new();
    for impl_info in schema.impls() {
        if impl_info.target_type() == name
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
}
