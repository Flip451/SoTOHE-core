//! Type-formatting helpers for the schema export infrastructure adapter.
//!
//! Contains `collect_type_names` and `format_type`, extracted from `schema_export`
//! to keep the parent module under the 700-line production-code limit.
//!
//! `format_type` preserves the schema exporter's historical TypeRef surface.
//! It intentionally differs from the TDDD structural-comparison formatter,
//! which canonicalizes order-independent constructs and includes extra
//! comparison-only detail.

use rustdoc_types::{DynTrait, FunctionPointer, GenericArg, GenericArgs, GenericBound, Type};

use crate::tddd::signal_evaluator_v2::format::{
    ShortTypeFormatPolicy, format_dyn_trait_with, format_function_pointer_with,
    format_impl_trait_with, format_qualified_path_with, format_short_type_with_policy,
};

/// Collects short type names from a rustdoc `Type` for transition-target
/// discovery.
///
/// This is a **name-extraction** helper — it collects candidate type names into
/// a `Vec<String>` for transition-target lookup.  It is explicitly **not** a
/// type renderer: it does not produce display strings, does not handle dyn
/// traits, function pointers, qualified paths, generics, primitives, slices,
/// tuples, or any of the other `Type` variants that a type renderer must
/// handle.  All non-transparent variants are silently ignored (`_ => {}`).
///
/// The structural similarity to `ty_base::format_type` (both match on `Type`
/// variants) is incidental.  `format_type` renders every variant into a
/// display string; this function only resolves a narrow set of transparent
/// wrappers to discover the underlying named type for transition-target lookup.
///
/// Traversal rules:
/// - `Result<T, _>` and `Option<T>`: recurse into the first generic argument
///   (the success/value type).
/// - `BorrowedRef { inner, .. }`: unwrap one reference layer and recurse.
/// - All other variants: push the short path name and stop (for resolved paths),
///   or silently ignore (for all other variants).
///   `Tuple`, `Vec<T>`, `Box<T>`, slice types, etc. are treated as opaque.
pub(super) fn collect_type_names(ty: &Type, out: &mut Vec<String>) {
    match ty {
        Type::ResolvedPath(p) => {
            let name = p.path.rsplit("::").next().unwrap_or(&p.path);
            match name {
                "Result" | "Option" => {
                    if let Some(args) = &p.args {
                        if let GenericArgs::AngleBracketed { args, .. } = args.as_ref() {
                            if let Some(GenericArg::Type(inner)) = args.first() {
                                collect_type_names(inner, out);
                            }
                        }
                    }
                }
                _ => {
                    out.push(name.to_string());
                }
            }
        }
        Type::BorrowedRef { type_: inner, .. } => {
            collect_type_names(inner, out);
        }
        _ => {}
    }
}

/// Recursive type formatter at L1 resolution.
///
/// Renders a rustdoc `Type` as a short-name string, preserving generic
/// structure verbatim. Module paths are stripped (last segment only). The
/// unit type `()` is rendered explicitly.
pub(super) fn format_type(ty: &Type) -> String {
    format_short_type_with_policy(ty, &SchemaTypeFormatPolicy)
}

struct SchemaTypeFormatPolicy;

impl ShortTypeFormatPolicy for SchemaTypeFormatPolicy {
    fn format_nested_type(&self, ty: &Type) -> String {
        format_type(ty)
    }

    fn format_generic_args(&self, args: &GenericArgs) -> String {
        format_args(args)
    }

    fn format_impl_trait(&self, bounds: &[GenericBound]) -> String {
        format_impl_trait_with(
            bounds,
            false,
            |_: &GenericArgs| String::new(),
            |_, short, _, _| short.to_owned(),
            |_: &str| None,
            |_: &[rustdoc_types::PreciseCapturingArg]| None,
        )
    }

    fn format_dyn_trait(&self, dyn_trait: &DynTrait) -> String {
        format_dyn_trait_with(
            dyn_trait,
            false,
            |pt| pt.trait_.path.rsplit("::").next().unwrap_or(&pt.trait_.path).to_string(),
            |_: Option<&str>| String::new(),
        )
    }

    fn format_function_pointer(&self, fp: &FunctionPointer) -> String {
        format_function_pointer_with(fp, format_type)
    }

    fn format_qualified_path(
        &self,
        name: &str,
        self_type: &Type,
        trait_: Option<&rustdoc_types::Path>,
        args: Option<&GenericArgs>,
    ) -> String {
        format_qualified_path_with(name, self_type, trait_, args, format_type, format_args)
    }
}

/// Render angle-bracketed generic argument lists. Lifetime and const
/// arguments are preserved in source order; type arguments are recursively
/// formatted via `format_type`.
fn format_args(args: &GenericArgs) -> String {
    match args {
        GenericArgs::AngleBracketed { args, .. } => args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(t) => format_type(t),
                GenericArg::Lifetime(lt) => lt.clone(),
                GenericArg::Const(c) => c.expr.replace("::", "."),
                GenericArg::Infer => "_".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        GenericArgs::Parenthesized { .. } | GenericArgs::ReturnTypeNotation => String::new(),
    }
}
