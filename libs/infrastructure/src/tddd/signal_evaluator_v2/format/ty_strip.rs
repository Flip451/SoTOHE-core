//! Impl-block type-param stripping formatters.
//!
//! Provides `format_type_strip_type_params` and `format_generic_args_strip_type_params`
//! which strip impl-block type/lifetime/const parameters from formatted type strings
//! so that `impl<S> TaskInteractor<S>` and `TaskInteractor` produce the same identity
//! key.

use std::collections::BTreeSet;

use rustdoc_types::{GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Type};

use super::abi::format_abi;
use super::ty_base::{
    format_generic_args_impl, format_generic_bounds_with, format_hrtb_type_params, format_type,
};

/// Formats a `rustdoc_types::Type` the same way as [`format_type`], but strips
/// generic args that are declared as type or lifetime parameters on the enclosing
/// `impl` block.
///
/// The primary use case is building the identity key for `impl` blocks in
/// `build_impl_identity_map`: `impl<S> TaskOperationInteractor<S>: TaskOperationService`
/// should produce the key `"TaskOperationInteractor: TaskOperationService"` (with `<S>`
/// removed), matching the catalogue A-codec key.
///
/// Stripping rules applied to `AngleBracketed` generic arg lists:
/// - `GenericArg::Type(Type::Generic(name))` where `name ∈ type_params` → removed.
/// - `GenericArg::Lifetime(lt)` where `lt ∈ type_params` OR `lt.trim_start_matches('\'') ∈
///   type_params` → removed.  Concrete lifetimes like `'static` are preserved.
/// - `GenericArg::Type(t)` for composite types — recurse with
///   `format_type_strip_type_params(t, type_params)` so nested impl-block type params
///   inside `Vec<S>`, tuples, or borrowed refs are also stripped.
/// - All other args (const values, `_`) are preserved as-is.
/// - When all angle-bracketed args are stripped, the `<…>` brackets are also removed.
pub(crate) fn format_type_strip_type_params(ty: &Type, type_params: &BTreeSet<String>) -> String {
    // Fast path: when there are no type params to strip, delegate to `format_type`
    // directly so the output is bit-for-bit identical for every supported variant.
    if type_params.is_empty() {
        return format_type(ty);
    }

    let strip = |inner: &Type| format_type_strip_type_params(inner, type_params);

    match ty {
        Type::ResolvedPath(p) => {
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            if let Some(args) = &p.args {
                let rendered = format_generic_args_strip_type_params(args, type_params);
                if rendered.is_empty() { short } else { format!("{short}<{rendered}>") }
            } else {
                short
            }
        }
        // Strip impl-block type/lifetime params wherever they appear.
        // Return `_` so the surrounding type string remains structurally valid.
        Type::Generic(name) => {
            if type_params.contains(name.as_str()) {
                "_".to_string()
            } else {
                name.clone()
            }
        }
        Type::Primitive(name) => name.clone(),
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            format!("&{mut_str}{}", strip(inner))
        }
        Type::Slice(inner) => format!("[{}]", strip(inner)),
        Type::Array { type_: inner, len } => {
            let safe_len = len.replace("::", ".");
            // Strip const param names from array length expressions.
            let stripped_len =
                if type_params.contains(safe_len.as_str()) { "_".to_string() } else { safe_len };
            format!("[{}; {}]", strip(inner), stripped_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(&strip).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", strip(inner))
        }
        Type::Pat { type_: inner, .. } => strip(inner),
        Type::ImplTrait(bounds) => {
            // Mirror `format_type`'s D3 fail-closed sentinel for unsupported bounds.
            let has_unsupported = bounds.iter().any(|b| match b {
                rustdoc_types::GenericBound::Outlives(_) | rustdoc_types::GenericBound::Use(_) => {
                    true
                }
                rustdoc_types::GenericBound::TraitBound { generic_params, .. } => {
                    !generic_params.is_empty()
                }
            });
            if has_unsupported {
                return "<UNSUPPORTED:ImplTrait>".to_string();
            }
            let mut parts: Vec<String> = bounds
                .iter()
                .filter_map(|b| match b {
                    rustdoc_types::GenericBound::TraitBound { trait_, modifier, .. } => {
                        let short =
                            trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                        let args_str = trait_
                            .args
                            .as_deref()
                            .map(|a| {
                                let s = format_generic_args_strip_type_params(a, type_params);
                                if s.is_empty() { String::new() } else { format!("<{s}>") }
                            })
                            .unwrap_or_default();
                        use rustdoc_types::TraitBoundModifier;
                        let modifier_str = match modifier {
                            TraitBoundModifier::None => "",
                            TraitBoundModifier::Maybe => "?",
                            TraitBoundModifier::MaybeConst => "~const ",
                        };
                        Some(format!("{modifier_str}{short}{args_str}"))
                    }
                    rustdoc_types::GenericBound::Outlives(_)
                    | rustdoc_types::GenericBound::Use(_) => None,
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
        }
        Type::DynTrait(dyn_trait) => {
            // Mirror `format_type`'s D3 fail-closed sentinel for HRTB binders.
            let has_hrtb = dyn_trait.traits.iter().any(|pt| !pt.generic_params.is_empty());
            if has_hrtb {
                return "<UNSUPPORTED:DynTrait>".to_string();
            }
            let mut parts: Vec<String> = dyn_trait
                .traits
                .iter()
                .map(|pt| {
                    let p = &pt.trait_;
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    let args_str = p
                        .args
                        .as_deref()
                        .map(|a| {
                            let s = format_generic_args_strip_type_params(a, type_params);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{args_str}")
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            // Strip the object lifetime if it is an impl-block lifetime param.
            let lifetime_str = dyn_trait
                .lifetime
                .as_deref()
                .and_then(|lt| {
                    let bare = lt.trim_start_matches('\'');
                    if type_params.contains(bare) { None } else { Some(format!(" + {lt}")) }
                })
                .unwrap_or_default();
            if rendered.is_empty() {
                format!("dyn _{lifetime_str}")
            } else {
                format!("dyn {rendered}{lifetime_str}")
            }
        }
        Type::FunctionPointer(fp) => {
            let params: Vec<String> = fp.sig.inputs.iter().map(|(_, t)| strip(t)).collect();
            let ret = fp.sig.output.as_ref().map_or_else(|| "()".to_string(), &strip);
            let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
            let constness = if fp.header.is_const { "const " } else { "" };
            let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
            let abi = format_abi(&fp.header.abi);
            // Preserve HRTB binders from the function type's own `generic_params`.
            let hrtb = if fp.generic_params.is_empty() {
                String::new()
            } else {
                let lt_strs: Vec<String> = fp
                    .generic_params
                    .iter()
                    .filter_map(|p| {
                        if matches!(p.kind, GenericParamDefKind::Lifetime { .. }) {
                            let bare = p.name.strip_prefix('\'').unwrap_or(&p.name);
                            Some(format!("'{bare}"))
                        } else {
                            None
                        }
                    })
                    .collect();
                let type_str = format_hrtb_type_params(&fp.generic_params);
                if lt_strs.is_empty() {
                    type_str
                } else {
                    format!("for<{}>{type_str}", lt_strs.join(","))
                }
            };
            format!("{hrtb}{abi}{constness}{unsafety}fn({}{})->{ret}", params.join(","), variadic)
        }
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let trait_str = trait_
                .as_ref()
                .map(|p| {
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    let trait_args_str = p
                        .args
                        .as_deref()
                        .map(|a| {
                            let s = format_generic_args_strip_type_params(a, type_params);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{trait_args_str}")
                })
                .unwrap_or_else(|| "_".to_string());
            let self_str = strip(self_type);
            let args_str = args.as_deref().map_or_else(String::new, |a| {
                format_generic_args_strip_type_params(a, type_params)
            });
            if args_str.is_empty() {
                format!("<{self_str} as {trait_str}>::{name}")
            } else {
                format!("<{self_str} as {trait_str}>::{name}<{args_str}>")
            }
        }
        other => format_type(other),
    }
}

/// Formats `GenericArgs`, filtering out angle-bracketed args that are
/// impl-block type parameters or lifetime parameters.
///
/// Returns the comma-joined rendered args **without** angle brackets.
///
/// Delegates to [`format_generic_args_impl`] with strip-aware callbacks:
/// - `fmt_arg` returns `None` for type/lifetime/const args matching `type_params`,
///   which causes those args to be omitted from the output.
/// - `fmt_type` / `fmt_const` / `fmt_bounds` apply the strip transformation to
///   constraint RHS values and `Parenthesized` inputs/output.
pub(crate) fn format_generic_args_strip_type_params(
    args: &GenericArgs,
    type_params: &BTreeSet<String>,
) -> String {
    format_generic_args_impl(
        args,
        &|arg| match arg {
            GenericArg::Type(Type::Generic(name)) if type_params.contains(name.as_str()) => None,
            GenericArg::Lifetime(lt) => {
                let bare = lt.trim_start_matches('\'');
                if type_params.contains(lt.as_str()) || type_params.contains(bare) {
                    None
                } else {
                    Some(lt.clone())
                }
            }
            GenericArg::Type(t) => Some(format_type_strip_type_params(t, type_params)),
            GenericArg::Const(c) => {
                let expr = c.expr.replace("::", ".");
                if type_params.contains(expr.as_str()) { None } else { Some(expr) }
            }
            GenericArg::Infer => Some("_".to_string()),
        },
        &|ty| format_type_strip_type_params(ty, type_params),
        &|s| {
            if type_params.contains(s) { "_".to_string() } else { s.to_string() }
        },
        &|bounds| format_generic_bounds_strip_type_params(bounds, type_params),
    )
}

/// Strip-aware variant of [`super::ty_base::format_generic_bounds`].
///
/// Differs from the plain variant in two ways:
///
/// 1. **Generic-arg stripping**: applies
///    [`format_generic_args_strip_type_params`] so that impl-block type
///    parameters appearing inside trait-bound generic args (e.g.
///    `Foo<Assoc: Bar<T>>`) are removed from the rendered string.
/// 2. **HRTB binder rendering**: for lifetime-only binders (D5 support)
///    emits a `#L{count}:` arity prefix (e.g. `#L1:Bar`) so that
///    `for<'a> Bar` and `Bar` produce distinct strings.  Type/const
///    binders fall back to the full HRTB bracket form.
///
/// Lifetime bounds (`Outlives`) and use-capture (`Use`) bounds are passed
/// through unchanged.
///
/// Delegates to [`format_generic_bounds_with`] with strip-aware callbacks.
pub(crate) fn format_generic_bounds_strip_type_params(
    bounds: &[GenericBound],
    type_params: &BTreeSet<String>,
) -> String {
    format_generic_bounds_with(
        bounds,
        |a| format_generic_args_strip_type_params(a, type_params),
        |modifier_str, short, args_str, generic_params| {
            let has_type_binders = generic_params.iter().any(|hp| {
                matches!(
                    hp.kind,
                    GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. }
                )
            });
            let binder_str = if has_type_binders {
                format_hrtb_type_params(generic_params)
            } else {
                let lt_count = generic_params
                    .iter()
                    .filter(|hp| matches!(hp.kind, GenericParamDefKind::Lifetime { .. }))
                    .count();
                if lt_count >= 1 { format!("#L{lt_count}:") } else { String::new() }
            };
            format!("{binder_str}{modifier_str}{short}{args_str}")
        },
    )
}

// ---------------------------------------------------------------------------
// Unit tests for `format_type_strip_type_params` const-generic edge cases
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use rustdoc_types::{
        AssocItemConstraint, AssocItemConstraintKind, Constant, GenericArg, GenericArgs, Path,
        Term, Type,
    };

    use super::{format_generic_args_strip_type_params, format_type_strip_type_params};

    fn params(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // --- Type::Array const-generic stripping ---

    #[test]
    fn test_array_len_const_param_is_stripped_to_underscore() {
        // `impl<const N: usize> Foo<[u8; N]>`: `N` is a const param.
        let ty =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "N".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["N"]));
        assert_eq!(result, "[u8; _]", "const param in array length must be replaced with '_'");
    }

    #[test]
    fn test_array_len_concrete_value_is_preserved() {
        let ty =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "16".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["N"]));
        assert_eq!(result, "[u8; 16]", "concrete array length must not be stripped");
    }

    #[test]
    fn test_array_len_const_param_not_in_type_params_is_preserved() {
        let ty =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "N".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["T", "S"]));
        assert_eq!(result, "[u8; N]", "array length not in type_params must not be stripped");
    }

    #[test]
    fn test_array_element_type_param_also_stripped() {
        let ty =
            Type::Array { type_: Box::new(Type::Generic("T".to_owned())), len: "N".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["T", "N"]));
        assert_eq!(
            result, "[_; _]",
            "both element type param and length param must be stripped; got: {result}"
        );
    }

    // --- GenericArg::Const stripping (angle-bracketed const params) ---

    #[test]
    fn test_angle_bracketed_const_generic_param_is_stripped() {
        let args = GenericArgs::AngleBracketed {
            args: vec![GenericArg::Const(Constant {
                expr: "N".to_owned(),
                value: None,
                is_literal: false,
            })],
            constraints: vec![],
        };
        let result = format_generic_args_strip_type_params(&args, &params(&["N"]));
        assert!(result.is_empty(), "const param in angle brackets must be stripped; got: {result}");
    }

    #[test]
    fn test_angle_bracketed_const_generic_not_in_params_is_preserved() {
        let args = GenericArgs::AngleBracketed {
            args: vec![GenericArg::Const(Constant {
                expr: "16".to_owned(),
                value: None,
                is_literal: true,
            })],
            constraints: vec![],
        };
        let result = format_generic_args_strip_type_params(&args, &params(&["N"]));
        assert_eq!(result, "16", "concrete const value must not be stripped; got: {result}");
    }

    // --- AssocItemConstraint Equality(Term::Constant) stripping ---

    #[test]
    fn test_const_equality_binding_const_param_is_stripped() {
        let args = GenericArgs::AngleBracketed {
            args: vec![],
            constraints: vec![AssocItemConstraint {
                name: "LEN".to_owned(),
                args: None,
                binding: AssocItemConstraintKind::Equality(Term::Constant(Constant {
                    expr: "N".to_owned(),
                    value: None,
                    is_literal: false,
                })),
            }],
        };
        let result = format_generic_args_strip_type_params(&args, &params(&["N"]));
        assert_eq!(
            result, "LEN=_",
            "const param in equality binding RHS must be stripped; got: {result}"
        );
    }

    #[test]
    fn test_const_equality_binding_concrete_value_preserved() {
        let args = GenericArgs::AngleBracketed {
            args: vec![],
            constraints: vec![AssocItemConstraint {
                name: "LEN".to_owned(),
                args: None,
                binding: AssocItemConstraintKind::Equality(Term::Constant(Constant {
                    expr: "16".to_owned(),
                    value: None,
                    is_literal: true,
                })),
            }],
        };
        let result = format_generic_args_strip_type_params(&args, &params(&["N"]));
        assert_eq!(
            result, "LEN=16",
            "concrete const value in equality binding must not be stripped; got: {result}"
        );
    }

    // --- Nested: ResolvedPath containing Array with const len ---

    #[test]
    fn test_resolved_path_with_array_const_len_stripped() {
        let inner_array =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "N".to_owned() };
        let ty = Type::ResolvedPath(Path {
            path: "mymodule::Foo".to_owned(),
            id: rustdoc_types::Id(0),
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Type(inner_array)],
                constraints: vec![],
            })),
        });
        let result = format_type_strip_type_params(&ty, &params(&["N"]));
        assert_eq!(
            result, "Foo<[u8; _]>",
            "const param inside nested array inside generic must be stripped; got: {result}"
        );
    }
}
