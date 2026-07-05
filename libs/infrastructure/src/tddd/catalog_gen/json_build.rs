//! Skeleton / entry JSON assembly for the catalogue adapter (D2 / D4).
//!
//! Builds the empty per-layer catalogue skeleton for `init` and the annotated
//! entry skeleton for `add`, seeding `$todo` holes for shape nodes the caller
//! did not supply.

use std::collections::BTreeSet;

use domain::plan_ref::SpecElementId;
use domain::tddd::LayerId;
use domain::tddd::catalog_gen::CatalogEntryKind;
use domain::tddd::catalogue_v2::CrateName;
use serde_json::{Map, Value, json};
use usecase::catalog_gen::{CatalogAddCommand, CatalogError};

use super::fragment::{
    parse_error, parse_field, parse_generic, parse_method, parse_trait_impl, parse_variant,
    parse_where,
};
use super::fs_access::schema_error;
use super::validate::{spec_refs_value, validate_role};
use crate::tddd::catalogue_document_codec::SCHEMA_VERSION;

/// A `{ "$todo": "..." }` hole node.
fn todo(instruction: &str) -> Value {
    json!({ "$todo": instruction })
}

/// Build the empty per-layer catalogue skeleton value (schema_version 5).
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] when `crate_name` or `layer` are not
/// valid identifiers.
pub(super) fn empty_catalogue_value(crate_name: &str, layer: &str) -> Result<Value, CatalogError> {
    CrateName::new(crate_name)
        .map_err(|err| schema_error(format!("invalid crate name `{crate_name}`: {err}")))?;
    LayerId::try_new(layer)
        .map_err(|err| schema_error(format!("invalid layer `{layer}`: {err}")))?;
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "crate_name": crate_name,
        "layer": layer,
        "types": {},
        "traits": {},
        "functions": {}
    }))
}

/// Build an annotated entry skeleton for `add`, plus any document-level
/// trait-impl declarations.
///
/// Supplied shape nodes are structured verbatim; omitted ones become `$todo`
/// holes.
///
/// # Errors
///
/// Returns a [`CatalogError`] on invalid role, unresolved anchor, or
/// unparseable shape fragment.
pub(super) fn build_add_entry(
    command: &CatalogAddCommand,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<(Value, Vec<Value>), CatalogError> {
    validate_shape_flag_compatibility(command)?;
    validate_role(command.kind, &command.role)?;
    let spec_refs = spec_refs_value(&command.anchors, spec_file, spec_anchors)?;
    let generics = parse_list(&command.generics, parse_generic)?;
    let where_predicates = parse_list(&command.where_predicates, parse_where)?;

    let mut entry = Map::new();
    entry.insert("action".to_owned(), json!("add"));
    entry.insert("role".to_owned(), role_value(command.kind, &command.role));

    match command.kind {
        CatalogEntryKind::Struct | CatalogEntryKind::Enum | CatalogEntryKind::TypeAlias => {
            entry.insert("kind".to_owned(), type_kind_value(command)?);
            entry.insert("methods".to_owned(), methods_value(command)?);
            insert_if_non_empty(&mut entry, "generics", generics);
            insert_if_non_empty(&mut entry, "where_predicates", where_predicates);
            entry.insert(
                "module_path".to_owned(),
                todo("module path where this type is declared, e.g. `tddd::foo`"),
            );
        }
        CatalogEntryKind::Trait => {
            entry.insert("methods".to_owned(), methods_value(command)?);
            insert_if_non_empty(&mut entry, "generics", generics);
            insert_if_non_empty(&mut entry, "where_predicates", where_predicates);
            entry
                .insert("module_path".to_owned(), todo("module path where this trait is declared"));
        }
        CatalogEntryKind::Function => {
            let signature = function_signature(command)?;
            entry.insert("params".to_owned(), signature.params);
            entry.insert("returns".to_owned(), signature.returns);
            entry.insert("is_async".to_owned(), json!(signature.is_async));
            // D6 (no silent drop): keep the generics / where predicates parsed
            // from the `--method` signature, appending any distinct `--generic` /
            // `--where` flags after them.
            let generics = union_values(signature.generics, generics);
            let where_predicates = union_values(signature.where_predicates, where_predicates);
            insert_if_non_empty(&mut entry, "generics", generics);
            insert_if_non_empty(&mut entry, "where_predicates", where_predicates);
        }
    }

    entry.insert("docs".to_owned(), todo("one-line doc comment describing this item"));
    entry.insert("spec_refs".to_owned(), spec_refs);
    entry.insert("informal_grounds".to_owned(), json!([]));

    let trait_impls = build_trait_impls(command)?;
    Ok((Value::Object(entry), trait_impls))
}

/// Reject shape flags that the selected entry kind cannot consume.
///
/// `catalog add` must either persist supplied shape information or fail before
/// writing; accepting a flag and then building a kind that has no slot for it
/// silently loses the caller's design intent.
fn validate_shape_flag_compatibility(command: &CatalogAddCommand) -> Result<(), CatalogError> {
    if !command.fields.is_empty() && command.kind != CatalogEntryKind::Struct {
        return Err(incompatible_flag_error("--field", command.kind, "struct entries"));
    }
    if !command.variants.is_empty() && command.kind != CatalogEntryKind::Enum {
        return Err(incompatible_flag_error("--variant", command.kind, "enum entries"));
    }
    if command.trait_impls.is_empty() {
        if !command.impl_generics.is_empty() {
            return Err(parse_error(
                "--impl-generic requires at least one --trait-impl; otherwise the impl generic \
                 would be dropped",
            ));
        }
        if !command.impl_where_predicates.is_empty() {
            return Err(parse_error(
                "--impl-where requires at least one --trait-impl; otherwise the impl where \
                 predicate would be dropped",
            ));
        }
    }
    Ok(())
}

/// Build a parse failure for a shape flag that does not apply to `kind`.
fn incompatible_flag_error(flag: &str, kind: CatalogEntryKind, expected: &str) -> CatalogError {
    parse_error(format!(
        "{flag} is incompatible with --kind {}; use it only for {expected}",
        kind_label(kind)
    ))
}

/// User-facing label for an entry kind.
fn kind_label(kind: CatalogEntryKind) -> &'static str {
    match kind {
        CatalogEntryKind::Struct => "struct",
        CatalogEntryKind::Enum => "enum",
        CatalogEntryKind::TypeAlias => "type-alias",
        CatalogEntryKind::Trait => "trait",
        CatalogEntryKind::Function => "function",
    }
}

/// Emit the role node: a tagged object for types/traits, a bare string for
/// functions (schema-5 wire form).
fn role_value(kind: CatalogEntryKind, role: &str) -> Value {
    match kind {
        CatalogEntryKind::Function => json!(role),
        _ => {
            let mut map = Map::new();
            map.insert(role.to_owned(), json!({}));
            Value::Object(map)
        }
    }
}

/// Build the `kind` node for a struct / enum / type-alias entry.
fn type_kind_value(command: &CatalogAddCommand) -> Result<Value, CatalogError> {
    match command.kind {
        CatalogEntryKind::Struct => {
            let shape = if command.fields.is_empty() {
                todo("describe the struct shape (unit / tuple / plain `name: Type` fields)")
            } else {
                let fields = parse_list(&command.fields, parse_field)?;
                json!({ "kind": "plain", "fields": fields, "has_stripped_fields": false })
            };
            Ok(json!({ "kind": "struct", "shape": shape }))
        }
        CatalogEntryKind::Enum => {
            let variants = if command.variants.is_empty() {
                todo("list the enum variants (e.g. `Idle`, `Pair(A, B)`)")
            } else {
                Value::Array(parse_list(&command.variants, parse_variant)?)
            };
            Ok(json!({ "kind": "enum", "variants": variants }))
        }
        CatalogEntryKind::TypeAlias => {
            Ok(json!({ "kind": "type_alias", "target": todo("the aliased type") }))
        }
        CatalogEntryKind::Trait | CatalogEntryKind::Function => {
            Err(schema_error("type kind requested for a non-type entry"))
        }
    }
}

/// Build the `methods` node (parsed array, or a `$todo` hole when omitted).
fn methods_value(command: &CatalogAddCommand) -> Result<Value, CatalogError> {
    if command.methods.is_empty() {
        Ok(todo("list method signatures as `fn name(...) -> R`, or `[]` if none"))
    } else {
        Ok(Value::Array(parse_list(&command.methods, parse_method)?))
    }
}

/// The pieces of a function entry derived from its `--method` signature.
struct FunctionSignatureParts {
    params: Value,
    returns: Value,
    is_async: bool,
    generics: Vec<Value>,
    where_predicates: Vec<Value>,
}

/// Derive a function entry's signature parts from its single `--method`
/// signature, or hole `params` / `returns` when none is supplied.
///
/// A function entry maps to exactly one signature, so more than one `--method`
/// fragment is rejected rather than silently truncated to the first.
fn function_signature(command: &CatalogAddCommand) -> Result<FunctionSignatureParts, CatalogError> {
    if command.methods.len() > 1 {
        return Err(parse_error(format!(
            "function entry accepts at most one --method fragment, got {}",
            command.methods.len()
        )));
    }
    match command.methods.first() {
        Some(signature) => {
            let parsed = parse_method(signature)?;
            let parsed_name = parsed.get("name").and_then(Value::as_str).ok_or_else(|| {
                schema_error(format!("method `{signature}` did not produce a string `name`"))
            })?;
            validate_function_signature_name(&command.name, parsed_name)?;
            let params = parsed.get("params").cloned().unwrap_or_else(|| json!([]));
            let returns = parsed.get("returns").cloned().unwrap_or_else(|| json!("()"));
            let is_async = parsed.get("is_async").and_then(Value::as_bool).unwrap_or(false);
            Ok(FunctionSignatureParts {
                params,
                returns,
                is_async,
                generics: array_field(&parsed, "generics"),
                where_predicates: array_field(&parsed, "where_predicates"),
            })
        }
        None => Ok(FunctionSignatureParts {
            params: todo("parameter list — supply the signature via `--method`"),
            returns: todo("return type — supply the signature via `--method`"),
            is_async: false,
            generics: Vec::new(),
            where_predicates: Vec::new(),
        }),
    }
}

/// Ensure the parsed `fn` name is the function entry's identity tail.
fn validate_function_signature_name(
    entry_name: &str,
    signature_name: &str,
) -> Result<(), CatalogError> {
    let entry_tail = entry_name.rsplit("::").next().unwrap_or(entry_name);
    if entry_tail == signature_name {
        return Ok(());
    }
    Err(parse_error(format!(
        "function entry `{entry_name}` cannot use signature for `fn {signature_name}`; \
         the signature name must match `{entry_tail}`"
    )))
}

/// Clone the array stored at `key` in `value`, or an empty vec when absent.
fn array_field(value: &Value, key: &str) -> Vec<Value> {
    value.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Concatenate `primary` with the entries of `secondary` that are not already
/// present (value equality), preserving `primary`-first order.
fn union_values(mut primary: Vec<Value>, secondary: Vec<Value>) -> Vec<Value> {
    for value in secondary {
        if !primary.contains(&value) {
            primary.push(value);
        }
    }
    primary
}

/// Build the document-level trait-impl declarations for the entry, carrying the
/// impl-block generics / where predicates onto each declaration.
fn build_trait_impls(command: &CatalogAddCommand) -> Result<Vec<Value>, CatalogError> {
    let impl_generics = parse_list(&command.impl_generics, parse_generic)?;
    let impl_where_predicates = parse_list(&command.impl_where_predicates, parse_where)?;
    let mut impls = Vec::new();
    for fragment in &command.trait_impls {
        let mut value = parse_trait_impl(fragment, &command.name)?;
        attach_impl_bounds(&mut value, &impl_generics, &impl_where_predicates);
        impls.push(value);
    }
    Ok(impls)
}

/// Attach `impl_generics` / `impl_where_predicates` to a trait-impl value when
/// non-empty, matching the codec's `TraitImplDto` wire shape.
fn attach_impl_bounds(value: &mut Value, generics: &[Value], where_predicates: &[Value]) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if !generics.is_empty() {
        object.insert("impl_generics".to_owned(), Value::Array(generics.to_vec()));
    }
    if !where_predicates.is_empty() {
        object.insert("impl_where_predicates".to_owned(), Value::Array(where_predicates.to_vec()));
    }
}

/// Map each fragment through `parser`, collecting into a vec.
fn parse_list<F>(fragments: &[String], parser: F) -> Result<Vec<Value>, CatalogError>
where
    F: Fn(&str) -> Result<Value, CatalogError>,
{
    fragments.iter().map(|fragment| parser(fragment)).collect()
}

/// Insert `values` under `key` when non-empty.
fn insert_if_non_empty(map: &mut Map<String, Value>, key: &str, values: Vec<Value>) {
    if !values.is_empty() {
        map.insert(key.to_owned(), Value::Array(values));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use domain::tddd::LayerId;
    use usecase::catalog_gen::CatalogAddCommand;

    fn anchor_set() -> BTreeSet<SpecElementId> {
        let mut set = BTreeSet::new();
        set.insert(SpecElementId::try_new("IN-01").unwrap());
        set
    }

    fn base_command(kind: CatalogEntryKind, role: &str) -> CatalogAddCommand {
        CatalogAddCommand {
            layer: LayerId::try_new("domain").unwrap(),
            kind,
            name: "Foo".to_owned(),
            role: role.to_owned(),
            anchors: vec![],
            fields: vec![],
            methods: vec![],
            variants: vec![],
            trait_impls: vec![],
            generics: vec![],
            where_predicates: vec![],
            impl_generics: vec![],
            impl_where_predicates: vec![],
        }
    }

    #[test]
    fn test_empty_catalogue_value() {
        let value = empty_catalogue_value("domain", "domain").unwrap();
        assert_eq!(value["schema_version"], json!(5));
        assert_eq!(value["crate_name"], json!("domain"));
        assert!(value["types"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_add_entry_struct_with_field() {
        let mut command = base_command(CatalogEntryKind::Struct, "ValueObject");
        command.fields = vec!["count: u32".to_owned()];
        command.anchors = vec!["IN-01".to_owned()];
        let (entry, trait_impls) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert!(trait_impls.is_empty());
        assert_eq!(entry["role"], json!({ "ValueObject": {} }));
        assert_eq!(entry["kind"]["shape"]["fields"], json!([{ "name": "count", "ty": "u32" }]));
        assert_eq!(entry["spec_refs"][0]["anchor"], json!("IN-01"));
        // Omitted shape nodes become holes.
        assert!(entry["docs"].get("$todo").is_some());
        assert!(entry["methods"].get("$todo").is_some());
        assert!(entry["module_path"].get("$todo").is_some());
    }

    #[test]
    fn test_build_add_entry_struct_without_field_holes_shape() {
        let command = base_command(CatalogEntryKind::Struct, "ValueObject");
        let (entry, _) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert!(entry["kind"]["shape"].get("$todo").is_some());
        let holes = super::super::scan_todo_holes(&entry);
        assert!(!holes.is_empty());
    }

    #[test]
    fn test_build_add_entry_enum_with_variant() {
        let mut command = base_command(CatalogEntryKind::Enum, "ErrorType");
        command.variants = vec!["NotFound".to_owned(), "Io(String)".to_owned()];
        let (entry, _) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(entry["kind"]["kind"], json!("enum"));
        assert_eq!(entry["kind"]["variants"][0]["name"], json!("NotFound"));
        assert_eq!(entry["kind"]["variants"][1]["payload"]["kind"], json!("tuple"));
    }

    #[test]
    fn test_build_add_entry_function_with_signature() {
        let mut command = base_command(CatalogEntryKind::Function, "FreeFunction");
        command.name = "domain::tddd::run".to_owned();
        command.methods = vec!["fn run(input: u32) -> bool".to_owned()];
        let (entry, _) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(entry["role"], json!("FreeFunction"));
        assert_eq!(entry["returns"], json!("bool"));
        assert_eq!(entry["params"], json!([{ "name": "input", "ty": "u32" }]));
    }

    // Finding 1: a function entry maps to exactly one signature; supplying more
    // than one `--method` is rejected rather than silently dropping the rest.
    #[test]
    fn test_build_add_entry_function_rejects_multiple_methods() {
        let mut command = base_command(CatalogEntryKind::Function, "FreeFunction");
        command.methods =
            vec!["fn run(input: u32) -> bool".to_owned(), "fn other() -> u8".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    // Finding 1: non-function kinds still accept multiple `--method` fragments.
    #[test]
    fn test_build_add_entry_trait_accepts_multiple_methods() {
        let mut command = base_command(CatalogEntryKind::Trait, "SecondaryPort");
        command.methods = vec!["fn a()".to_owned(), "fn b() -> u32".to_owned()];
        let (entry, _) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(entry["methods"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn test_build_add_entry_enum_rejects_field_flag() {
        let mut command = base_command(CatalogEntryKind::Enum, "ErrorType");
        command.fields = vec!["id: u64".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    #[test]
    fn test_build_add_entry_struct_rejects_variant_flag() {
        let mut command = base_command(CatalogEntryKind::Struct, "ValueObject");
        command.variants = vec!["Ready".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    #[test]
    fn test_build_add_entry_rejects_impl_bounds_without_trait_impl() {
        let mut command = base_command(CatalogEntryKind::Struct, "ValueObject");
        command.impl_generics = vec!["T: Clone".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));

        let mut command = base_command(CatalogEntryKind::Struct, "ValueObject");
        command.impl_where_predicates = vec!["T: Send".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    #[test]
    fn test_build_add_entry_function_rejects_signature_name_mismatch() {
        let mut command = base_command(CatalogEntryKind::Function, "FreeFunction");
        command.name = "domain::users::create_user".to_owned();
        command.methods = vec!["fn delete_user(id: UserId) -> Result<(), Error>".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    #[test]
    fn test_build_add_entry_rejects_invalid_role() {
        let command = base_command(CatalogEntryKind::Struct, "Bogus");
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::InvalidRole { .. }));
    }

    #[test]
    fn test_build_add_entry_rejects_unknown_anchor() {
        let mut command = base_command(CatalogEntryKind::Struct, "ValueObject");
        command.anchors = vec!["ZZ-99".to_owned()];
        let err = build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap_err();
        assert!(matches!(err, CatalogError::AnchorNotFound { .. }));
    }

    #[test]
    fn test_build_add_entry_with_trait_impl() {
        let mut command = base_command(CatalogEntryKind::Struct, "ErrorType");
        command.fields = vec!["message: String".to_owned()];
        command.trait_impls = vec!["From<CodecError>".to_owned()];
        let (_, trait_impls) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(trait_impls.len(), 1);
        assert_eq!(trait_impls[0]["for_type"], json!("Foo"));
    }

    // Finding 1: generics / where parsed from the `--method` signature must be
    // wired into the function entry, not silently dropped.
    #[test]
    fn test_build_add_entry_function_wires_signature_generics_and_where() {
        let mut command = base_command(CatalogEntryKind::Function, "FreeFunction");
        command.name = "domain::tddd::parse".to_owned();
        command.methods = vec!["fn parse<T: Clone>(input: T) -> T where T: Send".to_owned()];
        let (entry, _) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(entry["generics"], json!([{ "name": "T", "bounds": ["Clone"] }]));
        assert_eq!(
            entry["where_predicates"],
            json!([{ "lhs": "T", "rhs": ["Send"], "operator": "Bound" }])
        );
    }

    // Finding 1: signature-parsed generics come first; distinct `--generic`
    // flags are appended, and an identical flag is not duplicated.
    #[test]
    fn test_build_add_entry_function_unions_flag_generics_without_duplicates() {
        let mut command = base_command(CatalogEntryKind::Function, "FreeFunction");
        command.name = "domain::tddd::parse".to_owned();
        command.methods = vec!["fn parse<T: Clone>(input: T) -> T".to_owned()];
        command.generics = vec!["T: Clone".to_owned(), "U: Send".to_owned()];
        let (entry, _) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(
            entry["generics"],
            json!([
                { "name": "T", "bounds": ["Clone"] },
                { "name": "U", "bounds": ["Send"] }
            ])
        );
    }

    // Finding 2: `--impl-generic` / `--impl-where` must ride along on each
    // emitted trait-impl row (codec `TraitImplDto` shape).
    #[test]
    fn test_build_add_entry_trait_impl_carries_impl_generics_and_where() {
        let mut command = base_command(CatalogEntryKind::Struct, "ErrorType");
        command.fields = vec!["message: String".to_owned()];
        command.trait_impls = vec!["From<T>".to_owned()];
        command.impl_generics = vec!["T: Clone".to_owned()];
        command.impl_where_predicates = vec!["T: Send".to_owned()];
        let (_, trait_impls) =
            build_add_entry(&command, "track/items/t/spec.json", &anchor_set()).unwrap();
        assert_eq!(trait_impls.len(), 1);
        assert_eq!(trait_impls[0]["impl_generics"], json!([{ "name": "T", "bounds": ["Clone"] }]));
        assert_eq!(
            trait_impls[0]["impl_where_predicates"],
            json!([{ "lhs": "T", "rhs": ["Send"], "operator": "Bound" }])
        );
    }
}
