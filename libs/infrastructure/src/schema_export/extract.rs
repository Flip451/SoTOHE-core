//! Extraction helpers for struct fields, enum variants, function params, and
//! module paths from rustdoc JSON index items.
//!
//! All functions operate on `rustdoc_types` values produced by `rustdoc --output-format json`
//! and convert them into domain `MemberDeclaration` / `ParamDeclaration` / `FunctionInfo`
//! values suitable for inclusion in a `SchemaExport`.

use domain::schema::{FunctionInfo, SchemaExportError};
use domain::tddd::catalogue::{MemberDeclaration, ParamDeclaration};
use domain::tddd::catalogue_v2::identifiers::{ParamName, TypeRef};
use rustdoc_types::{ItemEnum, Type, Variant, VariantKind, Visibility};

use super::format_helpers::{collect_type_names, format_type};

/// Extract public fields from a struct as `MemberDeclaration::Field`.
pub(super) fn extract_struct_fields(
    s: &rustdoc_types::Struct,
    krate: &rustdoc_types::Crate,
) -> Vec<MemberDeclaration> {
    match &s.kind {
        rustdoc_types::StructKind::Plain { fields, .. } => fields
            .iter()
            .filter_map(|id| krate.index.get(id))
            .filter(|item| matches!(item.visibility, Visibility::Public))
            .filter_map(|item| {
                let name = item.name.clone()?;
                if let ItemEnum::StructField(ty) = &item.inner {
                    Some(MemberDeclaration::field(name, format_type(ty)))
                } else {
                    None
                }
            })
            .collect(),
        rustdoc_types::StructKind::Tuple(fields) => fields
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| {
                let id = opt.as_ref()?;
                let item = krate.index.get(id)?;
                if let ItemEnum::StructField(ty) = &item.inner {
                    Some(MemberDeclaration::field(i.to_string(), format_type(ty)))
                } else {
                    None
                }
            })
            .collect(),
        rustdoc_types::StructKind::Unit => Vec::new(),
    }
}

/// Extract enum variants as `MemberDeclaration::Variant`.
///
/// Each variant's payload types are extracted from its `VariantKind`:
/// - `Plain` → unit variant, empty payload
/// - `Tuple(fields)` → tuple variant, payload types from struct-field items
/// - `Struct { fields, .. }` → struct variant, payload types from struct-field items
///
/// # Errors
/// Returns `Err(SchemaExportError::ParseFailed)` when a variant has stripped
/// (private/hidden) payload fields, since the resulting `payload_types` would be
/// silently incomplete.
pub(super) fn extract_enum_variants(
    e: &rustdoc_types::Enum,
    krate: &rustdoc_types::Crate,
) -> Result<Vec<MemberDeclaration>, SchemaExportError> {
    let mut out = Vec::new();
    for id in &e.variants {
        let item = krate.index.get(id).ok_or_else(|| {
            SchemaExportError::ParseFailed(format!(
                "enum variant id {id:?} not found in rustdoc index; \
                 the exported JSON may be partial or stripped"
            ))
        })?;
        let name = item.name.clone().ok_or_else(|| {
            SchemaExportError::ParseFailed(format!(
                "enum variant id {id:?} has no name in rustdoc index; \
                 the exported JSON may be partial or stripped"
            ))
        })?;
        if let ItemEnum::Variant(v) = &item.inner {
            let payload_types = extract_variant_payload_types(v, krate, &name)?;
            out.push(MemberDeclaration::variant(name, payload_types));
        } else {
            return Err(SchemaExportError::ParseFailed(format!(
                "enum variant id {id:?} (name '{name}') resolves to non-Variant item; \
                 the exported JSON may be malformed or partial — \
                 payload extraction must be fail-closed (T001 ADR 2026-05-02-0316)"
            )));
        }
    }
    Ok(out)
}

/// Extract the payload type list from a single enum variant.
///
/// Unit variants (`Plain`) produce an empty `Vec`. Tuple and struct variants
/// produce a `Vec` of L1 short-name type strings by looking up each field id
/// in `krate.index` and formatting its `StructField` type via `format_type`.
///
/// # Errors
/// Returns `Err(SchemaExportError::ParseFailed)` (fail-closed) when:
/// - A `Tuple` variant has a `None` slot — the corresponding field was stripped
///   from the rustdoc JSON (private or `#[doc(hidden)]`), so the payload list
///   would be silently shorter than the actual tuple arity.
/// - A `Struct` variant has `has_stripped_fields == true` — one or more fields
///   are hidden, so the field list is incomplete.
fn extract_variant_payload_types(
    v: &Variant,
    krate: &rustdoc_types::Crate,
    variant_name: &str,
) -> Result<Vec<String>, SchemaExportError> {
    match &v.kind {
        VariantKind::Plain => Ok(vec![]),
        VariantKind::Tuple(fields) => {
            let mut out = Vec::with_capacity(fields.len());
            for (i, opt_id) in fields.iter().enumerate() {
                let id = opt_id.as_ref().ok_or_else(|| {
                    SchemaExportError::ParseFailed(format!(
                        "variant '{}' tuple field at index {} is stripped (private/hidden); \
                         payload_types would be incomplete",
                        variant_name, i
                    ))
                })?;
                let item = krate.index.get(id).ok_or_else(|| {
                    SchemaExportError::ParseFailed(format!(
                        "variant '{}' tuple field at index {} id not found in rustdoc index",
                        variant_name, i
                    ))
                })?;
                if let ItemEnum::StructField(ty) = &item.inner {
                    out.push(format_type(ty));
                } else {
                    return Err(SchemaExportError::ParseFailed(format!(
                        "variant '{}' tuple field at index {} is not a StructField in rustdoc \
                         index",
                        variant_name, i
                    )));
                }
            }
            Ok(out)
        }
        VariantKind::Struct { fields, has_stripped_fields } => {
            if *has_stripped_fields {
                return Err(SchemaExportError::ParseFailed(format!(
                    "variant '{}' struct has stripped fields; payload_types would be incomplete",
                    variant_name
                )));
            }
            let mut out = Vec::with_capacity(fields.len());
            for (i, id) in fields.iter().enumerate() {
                let item = krate.index.get(id).ok_or_else(|| {
                    SchemaExportError::ParseFailed(format!(
                        "variant '{}' struct field at index {} id not found in rustdoc index",
                        variant_name, i
                    ))
                })?;
                if let ItemEnum::StructField(ty) = &item.inner {
                    out.push(format_type(ty));
                } else {
                    return Err(SchemaExportError::ParseFailed(format!(
                        "variant '{}' struct field at index {} is not a StructField in rustdoc \
                         index",
                        variant_name, i
                    )));
                }
            }
            Ok(out)
        }
    }
}

/// Extract the module path for a type from the rustdoc `paths` table.
pub(super) fn extract_module_path(
    id: &rustdoc_types::Id,
    krate: &rustdoc_types::Crate,
) -> Option<String> {
    let summary = krate.paths.get(id)?;
    summary
        .path
        .get(..summary.path.len().saturating_sub(1))
        .filter(|parent| !parent.is_empty())
        .map(|parent| parent.join("::"))
}

/// Returns the self-receiver form (`"&self"` / `"&mut self"` / `"self"`), or
/// `None` if the first input is not a self receiver.
pub(super) fn extract_receiver(sig: &rustdoc_types::FunctionSignature) -> Option<String> {
    let (name, ty) = sig.inputs.first()?;
    if name != "self" {
        return None;
    }
    match ty {
        Type::BorrowedRef { is_mutable: false, .. } => Some("&self".to_string()),
        Type::BorrowedRef { is_mutable: true, .. } => Some("&mut self".to_string()),
        _ => Some("self".to_string()),
    }
}

/// Returns `true` if the function signature's first parameter is a self receiver.
pub(super) fn has_self_param(sig: &rustdoc_types::FunctionSignature) -> bool {
    sig.inputs.first().map(|(name, _)| name == "self").unwrap_or(false)
}

/// Extract the ordered parameter list from a function signature, excluding
/// the self receiver if present.
///
/// # Errors
/// Returns `SchemaExportError::ParseFailed` when a parameter name or type cannot be
/// converted to the V2 newtype (e.g. empty name or type string).
pub(super) fn extract_params(
    sig: &rustdoc_types::FunctionSignature,
) -> Result<Vec<ParamDeclaration>, SchemaExportError> {
    let mut out = Vec::new();
    for (name, ty) in sig.inputs.iter().filter(|(n, _)| n != "self") {
        let param_name = ParamName::new(name.as_str()).map_err(|e| {
            SchemaExportError::ParseFailed(format!(
                "param name '{}' is not a valid identifier: {e}",
                name
            ))
        })?;
        let ty_str = format_type(ty);
        let ty_ref = TypeRef::new(&ty_str).map_err(|e| {
            SchemaExportError::ParseFailed(format!(
                "param '{}' type '{}' is not valid: {e}",
                name, ty_str
            ))
        })?;
        out.push(ParamDeclaration::new(param_name, ty_ref));
    }
    Ok(out)
}

/// Format the return type. `Option<Type>::None` is rendered as `"()"`.
pub(super) fn format_return(sig: &rustdoc_types::FunctionSignature) -> String {
    sig.output.as_ref().map_or_else(|| "()".to_string(), format_type)
}

/// Extract the list of type names from the return type of a function signature.
pub(super) fn extract_return_type_names(sig: &rustdoc_types::FunctionSignature) -> Vec<String> {
    sig.output.as_ref().map_or_else(Vec::new, |ty| {
        let mut names = Vec::new();
        collect_type_names(ty, &mut names);
        names
    })
}

/// Extract method `FunctionInfo`s from a list of item Ids.
/// Accepts both `Public` and `Default` visibility (trait associated items use `Default`).
///
/// # Errors
/// Returns `SchemaExportError::ParseFailed` when `extract_params` fails for any method.
pub(super) fn extract_methods(
    ids: &[rustdoc_types::Id],
    krate: &rustdoc_types::Crate,
) -> Result<Vec<FunctionInfo>, SchemaExportError> {
    let mut out = Vec::new();
    for id in ids {
        let item = match krate.index.get(id) {
            Some(i) => i,
            None => continue,
        };
        if !matches!(item.visibility, Visibility::Public | Visibility::Default) {
            continue;
        }
        let name = match &item.name {
            Some(n) => n,
            None => continue,
        };
        if let ItemEnum::Function(f) = &item.inner {
            let return_type_names = extract_return_type_names(&f.sig);
            let has_self = has_self_param(&f.sig);
            let receiver = extract_receiver(&f.sig);
            let params = extract_params(&f.sig)?;
            let returns = format_return(&f.sig);
            out.push(FunctionInfo::new(
                name.clone(),
                item.docs.clone(),
                return_type_names,
                has_self,
                params,
                returns,
                receiver,
                f.header.is_async,
            ));
        }
    }
    Ok(out)
}
