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
    let type_info = select_type(&schema, &module, &name).ok_or_else(|| {
        schema_error(format!("type `{type_path}` not found in crate `{crate_name}`"))
    })?;
    let kind = kind_value(type_info);
    let methods = collect_methods(&schema, &name);
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

/// Select the [`TypeInfo`] matching `name`, preferring a module match.
fn select_type<'a>(schema: &'a SchemaExport, module: &str, name: &str) -> Option<&'a TypeInfo> {
    let mut fallback = None;
    for type_info in schema.types() {
        if type_info.name() != name {
            continue;
        }
        match type_info.module_path() {
            Some(path) if module.is_empty() || path.ends_with(module) => return Some(type_info),
            _ => fallback = fallback.or(Some(type_info)),
        }
    }
    fallback
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

/// Collect inherent (non-trait) methods for `name` from the schema's impls.
fn collect_methods(schema: &SchemaExport, name: &str) -> Vec<Value> {
    let mut methods = Vec::new();
    for impl_info in schema.impls() {
        if impl_info.target_type() == name && impl_info.trait_name().is_none() {
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
}
