//! Base (non-canon) type and bounds formatters.
//!
//! Provides `format_type`, `format_generic_args`, `format_hrtb_type_params`,
//! `format_generic_bounds`, and `format_type_common_arms` — the canon-unaware
//! rendering path and the shared traversal skeleton for canon-aware variants.

use std::collections::HashMap;

use rustdoc_types::{
    AssocItemConstraintKind, DynTrait, FunctionPointer, GenericArg, GenericArgs, GenericBound,
    GenericParamDef, GenericParamDefKind, Term, Type,
};

use super::abi::format_abi;
use super::canon::apply_canon_to_str;

/// Formats HRTB type params (`for<T: Foo, T: Bar>`) as a bracketed string.
///
/// Only type parameters (not lifetime parameters) are included in the output
/// because lifetime renaming is identity-preserving at L1.  Type parameters
/// are rendered as `T:<bound1>+<bound2>` and sorted so that equivalent bound
/// sets produce identical strings.  The result is wrapped in `[…]` when
/// non-empty and empty otherwise, so the caller can unconditionally append it.
/// Nested HRTB binders (inside a bound's own `generic_params`) are recursed so
/// that `for<T: for<U: Foo> Bar>` produces a distinct string from `for<T: Bar>`.
///
/// Example: `for<T: Foo, T: Bar>` → `[T:Bar,T:Foo]`
pub(crate) fn format_hrtb_type_params(generic_params: &[GenericParamDef]) -> String {
    let type_params: Vec<String> = generic_params
        .iter()
        .filter_map(|hp| {
            if let GenericParamDefKind::Type { bounds: hb, .. } = &hp.kind {
                let mut strs: Vec<String> = hb
                    .iter()
                    .filter_map(|b| {
                        if let GenericBound::TraitBound {
                            trait_: ht, generic_params: nested, ..
                        } = b
                        {
                            let short = ht.path.rsplit("::").next().unwrap_or(&ht.path).to_string();
                            // Recursively include nested HRTB so that distinct nested
                            // binders produce distinct strings.
                            let nested_str = format_hrtb_type_params(nested);
                            Some(format!("{short}{nested_str}"))
                        } else {
                            None
                        }
                    })
                    .collect();
                strs.sort_unstable();
                Some(format!("T:{}", strs.join("+")))
            } else {
                None
            }
        })
        .collect();
    if type_params.is_empty() { String::new() } else { format!("[{}]", type_params.join(",")) }
}

/// Policy hooks for the short-name `Type` traversal shared by plain renderers.
pub(crate) trait ShortTypeFormatPolicy {
    fn format_nested_type(&self, ty: &Type) -> String;
    fn format_generic_args(&self, args: &GenericArgs) -> String;
    fn format_impl_trait(&self, bounds: &[GenericBound]) -> String;
    fn format_dyn_trait(&self, dyn_trait: &DynTrait) -> String;
    fn format_function_pointer(&self, fp: &FunctionPointer) -> String;
    fn format_qualified_path(
        &self,
        name: &str,
        self_type: &Type,
        trait_: Option<&rustdoc_types::Path>,
        args: Option<&GenericArgs>,
    ) -> String;
}

/// Traverses the common short-name `Type` variants and delegates policy-specific
/// generic/bound formatting to the caller.
pub(crate) fn format_short_type_with_policy<P: ShortTypeFormatPolicy>(
    ty: &Type,
    policy: &P,
) -> String {
    match ty {
        Type::ResolvedPath(p) => {
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            if let Some(args) = &p.args {
                let rendered = policy.format_generic_args(args);
                if rendered.is_empty() { short } else { format!("{short}<{rendered}>") }
            } else {
                short
            }
        }
        Type::Generic(name) => name.clone(),
        Type::Primitive(name) => name.clone(),
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            format!("&{mut_str}{}", policy.format_nested_type(inner))
        }
        Type::Slice(inner) => format!("[{}]", policy.format_nested_type(inner)),
        Type::Array { type_: inner, len } => {
            let safe_len = len.replace("::", ".");
            format!("[{}; {}]", policy.format_nested_type(inner), safe_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(|t| policy.format_nested_type(t)).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", policy.format_nested_type(inner))
        }
        Type::ImplTrait(bounds) => policy.format_impl_trait(bounds),
        Type::DynTrait(dyn_trait) => policy.format_dyn_trait(dyn_trait),
        Type::FunctionPointer(fp) => policy.format_function_pointer(fp),
        Type::Pat { type_: inner, .. } => policy.format_nested_type(inner),
        Type::QualifiedPath { name, self_type, trait_, args } => {
            policy.format_qualified_path(name, self_type, trait_.as_ref(), args.as_deref())
        }
        _ => "_".to_string(),
    }
}

struct BaseShortTypeFormatPolicy;

impl ShortTypeFormatPolicy for BaseShortTypeFormatPolicy {
    fn format_nested_type(&self, ty: &Type) -> String {
        format_type(ty)
    }

    fn format_generic_args(&self, args: &GenericArgs) -> String {
        format_generic_args(args)
    }

    fn format_impl_trait(&self, bounds: &[GenericBound]) -> String {
        format_base_impl_trait(bounds)
    }

    fn format_dyn_trait(&self, dyn_trait: &DynTrait) -> String {
        format_base_dyn_trait(dyn_trait)
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
        format_qualified_path_with(name, self_type, trait_, args, format_type, format_generic_args)
    }
}

/// Formats a `rustdoc_types::Type` as a short-name string at L1 resolution.
///
/// Module paths are stripped (only the last segment is kept). Generic arguments
/// are preserved recursively. This function mirrors the private `format_type`
/// in `schema_export.rs` so that A-codec-derived types and rustdoc-derived types
/// compare symmetrically in Phase 2 structural equality checks.
///
/// # L1 short-name design rationale (why external crate paths are stripped)
///
/// The catalogue (A side) codec (`schema_export.rs::format_type`) also strips
/// module paths to short names.  S is built by seeding from B (rustdoc) then
/// applying A catalogue entries.  A-sourced items already carry short names
/// (e.g. the field type `"Serialize"` not `"serde::Serialize"`).  Preserving
/// full external paths on the C side but not on the A side would break symmetry
/// and cause false structural mismatches for all A-modified items.
///
/// As a consequence, two distinct external traits that share the same short name
/// (e.g. `serde::Serialize` and `other_crate::Serialize`) compare equal at L1.
/// This is an accepted trade-off of the L1 design (ADR 3 D2 / D3): the
/// 1-crate = 1-catalogue constraint (ADR 1 D6) makes same-short-name collisions
/// between external traits in any single catalogue scope practically impossible.
pub(crate) fn format_type(ty: &Type) -> String {
    format_short_type_with_policy(ty, &BaseShortTypeFormatPolicy)
}

pub(crate) fn format_impl_trait_with(
    bounds: &[GenericBound],
    sort_parts: bool,
    fmt_args: impl Fn(&GenericArgs) -> String,
    fmt_trait_bound: impl Fn(&str, &str, &str, &[GenericParamDef]) -> String,
    fmt_outlives: impl Fn(&str) -> Option<String>,
    fmt_use: impl Fn(&[rustdoc_types::PreciseCapturingArg]) -> Option<String>,
) -> String {
    let parts = format_generic_bound_parts_with(
        bounds,
        sort_parts,
        fmt_args,
        fmt_trait_bound,
        fmt_outlives,
        fmt_use,
    );
    let rendered = parts.join(" + ");
    if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
}

pub(crate) fn format_dyn_trait_with(
    dyn_trait: &DynTrait,
    sort_parts: bool,
    fmt_trait: impl Fn(&rustdoc_types::PolyTrait) -> String,
    fmt_lifetime: impl Fn(Option<&str>) -> String,
) -> String {
    let mut parts: Vec<String> = dyn_trait.traits.iter().map(fmt_trait).collect();
    if sort_parts {
        parts.sort_unstable();
    }
    let rendered = parts.join(" + ");
    let lifetime_str = fmt_lifetime(dyn_trait.lifetime.as_deref());
    if rendered.is_empty() {
        format!("dyn _{lifetime_str}")
    } else {
        format!("dyn {rendered}{lifetime_str}")
    }
}

fn format_base_impl_trait(bounds: &[GenericBound]) -> String {
    format_impl_trait_with(
        bounds,
        true,
        format_generic_args,
        |modifier_str, short, args_str, generic_params| {
            let hrtb_str = format_hrtb_type_params(generic_params);
            format!("{modifier_str}{short}{args_str}{hrtb_str}")
        },
        |lt| Some(lt.to_owned()),
        format_use_bounds,
    )
}

fn format_base_dyn_trait(dyn_trait: &DynTrait) -> String {
    format_dyn_trait_with(
        dyn_trait,
        true,
        |pt| {
            let p = &pt.trait_;
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            let args_str = p
                .args
                .as_deref()
                .map(|a| {
                    let s = format_generic_args(a);
                    if s.is_empty() { String::new() } else { format!("<{s}>") }
                })
                .unwrap_or_default();
            let hrtb_str = format_hrtb_type_params(&pt.generic_params);
            format!("{hrtb_str}{short}{args_str}")
        },
        |lifetime| lifetime.map(|lt| format!(" + {lt}")).unwrap_or_default(),
    )
}

pub(crate) fn format_function_pointer_with(
    fp: &FunctionPointer,
    fmt_type: impl Fn(&Type) -> String,
) -> String {
    let params: Vec<String> = fp.sig.inputs.iter().map(|(_, t)| fmt_type(t)).collect();
    let ret = fp.sig.output.as_ref().map_or_else(|| "()".to_string(), fmt_type);
    let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
    let constness = if fp.header.is_const { "const " } else { "" };
    let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
    let abi = format_abi(&fp.header.abi);
    let hrtb = format_function_pointer_hrtb(&fp.generic_params);
    format!("{hrtb}{abi}{constness}{unsafety}fn({}{})->{ret}", params.join(","), variadic)
}

fn format_function_pointer_hrtb(generic_params: &[GenericParamDef]) -> String {
    if generic_params.is_empty() {
        return String::new();
    }
    let lt_strs: Vec<String> = generic_params
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
    let type_str = format_hrtb_type_params(generic_params);
    if lt_strs.is_empty() { type_str } else { format!("for<{}>{type_str}", lt_strs.join(",")) }
}

pub(crate) fn format_qualified_path_with(
    name: &str,
    self_type: &Type,
    trait_: Option<&rustdoc_types::Path>,
    args: Option<&GenericArgs>,
    mut fmt_type: impl FnMut(&Type) -> String,
    mut fmt_args: impl FnMut(&GenericArgs) -> String,
) -> String {
    let trait_str = trait_
        .map(|p| {
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            // Normalize an empty-path trait (rustdoc encodes Self-projection's resolved-but-
            // unnamed trait as `Path { path: "", .. }`) to the same token as `None` so that
            // `<Self as >::Input<'_>` (C-side) and `<Self as _>::Input<'_>` (A-side, trait:
            // None) compare as equal.
            if short.is_empty() {
                return "_".to_string();
            }
            let trait_args_str = p
                .args
                .as_deref()
                .map(|a| {
                    let s = fmt_args(a);
                    if s.is_empty() { String::new() } else { format!("<{s}>") }
                })
                .unwrap_or_default();
            format!("{short}{trait_args_str}")
        })
        .unwrap_or_else(|| "_".to_string());
    let self_str = fmt_type(self_type);
    let args_str = args.map_or_else(String::new, fmt_args);
    if args_str.is_empty() {
        format!("<{self_str} as {trait_str}>::{name}")
    } else {
        format!("<{self_str} as {trait_str}>::{name}<{args_str}>")
    }
}

/// Shared structural traversal for `GenericArgs` formatting.
///
/// Owns the common algorithm: positional arg ordering, constraint sorting,
/// `"="` vs `":"` separators, and `Parenthesized` pattern.  Callers provide
/// policy-specific rendering via callbacks:
///
/// - `fmt_arg` — maps a single `GenericArg` to `Option<String>`.  Returning `None`
///   causes the arg to be dropped from the output (used by the strip-type-params path
///   to remove impl-block type/lifetime params).  For a plain rendering path, always
///   return `Some(…)`.
/// - `fmt_type` — format a `Type` value (used for constraint RHS types and
///   `Parenthesized` input/output).
/// - `fmt_const` — format a const-expression string (already `"::"→"."` normalized);
///   used for constraint RHS constants.
/// - `fmt_bounds` — format a `&[GenericBound]` slice for `Constraint` bindings.
///
/// `format_generic_args`, `format_generic_args_with_canon`, and
/// `format_generic_args_strip_type_params` all delegate to this helper, differing only
/// in which callbacks they supply.
pub(crate) fn format_generic_args_impl(
    args: &GenericArgs,
    fmt_arg: &dyn Fn(&GenericArg) -> Option<String>,
    fmt_type: &dyn Fn(&Type) -> String,
    fmt_const: &dyn Fn(&str) -> String,
    fmt_bounds: &dyn Fn(&[GenericBound]) -> String,
) -> String {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            // Type args and lifetimes are position-sensitive — preserve their order.
            // `fmt_arg` returns `None` to skip args (e.g. strip-type-params path).
            let positional: Vec<String> = args.iter().filter_map(fmt_arg).collect();
            // Associated-type/const constraints are named bindings (`Item = u8` or
            // `Item: Bound`) and are order-independent in Rust semantics. Sort them so
            // that two equivalent types with constraints in different orders compare as
            // equal.
            //
            // Use distinct separators for Equality (`=`) and Constraint (`:`) so that
            // `Iterator<Item = Copy>` and `Iterator<Item: Copy>` produce different
            // strings and are not incorrectly treated as equivalent.
            let mut constraint_parts: Vec<String> = constraints
                .iter()
                .map(|c| match &c.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                        let rhs = fmt_type(ty);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
                        let rhs = fmt_const(&cv.expr.replace("::", "."));
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        let rhs = fmt_bounds(bounds);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}:{}", c.name, rhs) }
                    }
                })
                .collect();
            constraint_parts.sort();

            let mut parts = positional;
            parts.extend(constraint_parts);
            parts.join(", ")
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let params: Vec<String> = inputs.iter().map(fmt_type).collect();
            let ret = output.as_ref().map_or_else(|| "()".to_string(), fmt_type);
            format!("({})->{}", params.join(","), ret)
        }
        _ => String::new(),
    }
}

/// Formats `GenericArgs` without canonicalization.
///
/// Used by `format_type` for the base (non-canon) rendering path.
/// Delegates to [`format_generic_args_impl`] with identity-preserving callbacks.
pub(crate) fn format_generic_args(args: &GenericArgs) -> String {
    format_generic_args_impl(
        args,
        &|arg| {
            Some(match arg {
                GenericArg::Type(t) => format_type(t),
                GenericArg::Lifetime(lt) => lt.clone(),
                GenericArg::Const(c) => c.expr.replace("::", "."),
                GenericArg::Infer => "_".to_string(),
            })
        },
        &|t| format_type(t),
        &|s| s.to_string(),
        &|bounds| format_generic_bounds(bounds),
    )
}

/// Shared helper for `GenericBound` slice formatting.
///
/// Owns the common iteration, `Outlives` / `Use` handling, TraitBound path
/// shortening and modifier formatting, sorting, and joining.  Callers supply
/// two policy callbacks:
///
/// - `fmt_args` — formats a `GenericArgs` value for the trait's generic
///   arguments (plain, strip-aware, or canon-aware).
/// - `fmt_trait_bound` — given `(modifier_str, short_path, args_str,
///   generic_params)` returns the final string for a single TraitBound entry.
///   This is where binder prefix / suffix policy is injected.
///
/// `format_generic_bounds` and `format_generic_bounds_strip_type_params` both
/// delegate here, differing only in which callbacks they supply.
pub(crate) fn format_generic_bounds_with(
    bounds: &[GenericBound],
    fmt_args: impl Fn(&GenericArgs) -> String,
    fmt_trait_bound: impl Fn(&str, &str, &str, &[GenericParamDef]) -> String,
) -> String {
    let strs = format_generic_bound_parts_with(
        bounds,
        true,
        fmt_args,
        fmt_trait_bound,
        |lt| Some(lt.to_owned()),
        format_use_bounds,
    );
    strs.join("+")
}

fn format_generic_bound_parts_with(
    bounds: &[GenericBound],
    sort_parts: bool,
    fmt_args: impl Fn(&GenericArgs) -> String,
    fmt_trait_bound: impl Fn(&str, &str, &str, &[GenericParamDef]) -> String,
    fmt_outlives: impl Fn(&str) -> Option<String>,
    fmt_use: impl Fn(&[rustdoc_types::PreciseCapturingArg]) -> Option<String>,
) -> Vec<String> {
    let mut parts: Vec<String> = bounds
        .iter()
        .filter_map(|b| match b {
            GenericBound::TraitBound { trait_, modifier, generic_params } => {
                let short = trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                let args_str = trait_
                    .args
                    .as_deref()
                    .map(|a| {
                        let s = fmt_args(a);
                        if s.is_empty() { String::new() } else { format!("<{s}>") }
                    })
                    .unwrap_or_default();
                use rustdoc_types::TraitBoundModifier;
                let modifier_str = match modifier {
                    TraitBoundModifier::None => "",
                    TraitBoundModifier::Maybe => "?",
                    TraitBoundModifier::MaybeConst => "~const ",
                };
                Some(fmt_trait_bound(modifier_str, &short, &args_str, generic_params))
            }
            GenericBound::Outlives(lt) => fmt_outlives(lt),
            GenericBound::Use(use_bounds) => fmt_use(use_bounds),
        })
        .collect();
    if sort_parts {
        parts.sort();
    }
    parts
}

fn format_use_bounds(use_bounds: &[rustdoc_types::PreciseCapturingArg]) -> Option<String> {
    if use_bounds.is_empty() {
        return None;
    }
    let parts: Vec<String> = use_bounds
        .iter()
        .map(|b| match b {
            rustdoc_types::PreciseCapturingArg::Lifetime(lt) => lt.clone(),
            rustdoc_types::PreciseCapturingArg::Param(name) => name.clone(),
        })
        .collect();
    Some(format!("use<{}>", parts.join(",")))
}

/// Formats a slice of `GenericBound` values as a sorted, `+`-joined string.
///
/// Plain (non-canon, non-strip) rendering path.  Delegates to
/// [`format_generic_bounds_with`] with identity-preserving callbacks:
///
/// - `fmt_args`: delegates to [`format_generic_args`] (no canonicalization,
///   no type-param stripping).
/// - `fmt_trait_bound`: appends HRTB type-param brackets as a suffix so that
///   `for<T: Foo>` and `for<T: Bar>` produce distinct strings.  Lifetime
///   binders (`for<'a>`) are skipped since they are identity-preserving at L1.
///
/// Bounds are sorted alphabetically before joining so that semantically
/// equivalent bound sets (e.g. `A + B` vs `B + A`) produce identical strings.
/// Lifetime (`Outlives`) and use-capture (`Use`) bounds are included so that
/// `impl Copy + 'a` and `impl Copy` compare as structurally different.
pub(crate) fn format_generic_bounds(bounds: &[GenericBound]) -> String {
    format_generic_bounds_with(
        bounds,
        format_generic_args,
        // Binder suffix: HRTB type params distinguish `for<T: Foo>` from bare.
        // Lifetime binders are identity-preserving and omitted.
        |modifier_str, short, args_str, generic_params| {
            let hrtb_str = format_hrtb_type_params(generic_params);
            format!("{modifier_str}{short}{args_str}{hrtb_str}")
        },
    )
}

/// Shared inner traversal for canon-aware type formatting.
///
/// Handles all `Type` variants whose structure is identical across canon-unaware
/// and occurrence-aware rendering — namely every variant except `Type::Generic`
/// and `Type::ImplTrait`, which carry policy-specific placeholder / sentinel
/// logic that each caller must handle before delegating to this function.
///
/// Callers provide two callbacks:
///
/// - `fmt_rec` — formats a nested `Type` value recursively, threading any
///   extra state (e.g. occurrence cursor) the caller maintains.
/// - `fmt_generic_args` — formats a `GenericArgs` value, again threading extra
///   state as appropriate.
///
/// `canon` is required because several arms inspect it directly:
/// - `ResolvedPath`: to detect and preserve projection paths (`T::Item`).
/// - `BorrowedRef`: for HRTB `@BR:` sentinel-based lifetime rendering.
/// - `Array`: to canonicalize const-generic length expressions via
///   `apply_canon_to_str`.
///
/// This function is **not** a fallback — callers must not pass `Type::Generic`
/// or `Type::ImplTrait`; those are handled by the caller before the match
/// delegation.
pub(crate) fn format_type_common_arms(
    ty: &Type,
    canon: &HashMap<String, String>,
    mut fmt_rec: &mut impl FnMut(&Type) -> String,
    fmt_generic_args: &mut impl FnMut(&GenericArgs) -> String,
) -> String {
    match ty {
        Type::ResolvedPath(p) => {
            let path_str: &str = &p.path;
            let display_base = if let Some(sep_pos) = path_str.find("::") {
                let prefix = &path_str[..sep_pos];
                let rest = &path_str[sep_pos..];
                if !canon.is_empty() && canon.contains_key(prefix) {
                    let canon_prefix = canon.get(prefix).map(|s| s.as_str()).unwrap_or(prefix);
                    format!("{canon_prefix}{rest}")
                } else {
                    p.path.rsplit("::").next().unwrap_or(path_str).to_string()
                }
            } else {
                p.path.clone()
            };
            if let Some(args) = &p.args {
                let rendered = fmt_generic_args(args);
                if rendered.is_empty() {
                    display_base
                } else {
                    format!("{display_base}<{rendered}>")
                }
            } else {
                display_base
            }
        }
        Type::Primitive(name) => name.clone(),
        Type::BorrowedRef { lifetime, is_mutable, type_: inner } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            let in_hrtb_ctx = canon.contains_key("@BR:");
            let lt_str = match lifetime.as_deref() {
                None => String::new(),
                Some(lt) => {
                    if let Some(pos) = canon.get(&format!("@BR:{lt}")) {
                        if pos.is_empty() { String::new() } else { format!("{pos} ") }
                    } else if in_hrtb_ctx || lt == "'static" {
                        format!("{lt} ")
                    } else {
                        String::new()
                    }
                }
            };
            format!("&{lt_str}{mut_str}{}", fmt_rec(inner))
        }
        Type::Slice(inner) => format!("[{}]", fmt_rec(inner)),
        Type::Array { type_: inner, len } => {
            let safe_len = apply_canon_to_str(&len.replace("::", "."), canon);
            format!("[{}; {}]", fmt_rec(inner), safe_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(&mut fmt_rec).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", fmt_rec(inner))
        }
        Type::DynTrait(DynTrait { traits, lifetime }) => {
            let has_hrtb = traits.iter().any(|pt| !pt.generic_params.is_empty());
            if has_hrtb {
                return "<UNSUPPORTED:DynTrait>".to_string();
            }
            // Safety: neither `Type::ImplTrait` nor `Type::Generic("impl …")` can
            // appear nested inside a `dyn Trait`'s generic args in valid Rust or
            // rustdoc JSON.  Therefore `fmt_generic_args` will never increment any
            // occurrence cursor that the caller may maintain, so sorting the rendered
            // bounds after formatting them is safe.
            let mut parts: Vec<String> = traits
                .iter()
                .map(|pt| {
                    let p = &pt.trait_;
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    let args_str = p
                        .args
                        .as_deref()
                        .map(|a| {
                            let s = fmt_generic_args(a);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{args_str}")
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            let lifetime_str = lifetime.as_deref().map(|lt| format!(" + {lt}")).unwrap_or_default();
            if rendered.is_empty() {
                format!("dyn _{lifetime_str}")
            } else {
                format!("dyn {rendered}{lifetime_str}")
            }
        }
        Type::FunctionPointer(fp) => {
            if !fp.generic_params.is_empty() {
                return "<UNSUPPORTED:FunctionPointer>".to_string();
            }
            let params: Vec<String> = fp.sig.inputs.iter().map(|(_, t)| fmt_rec(t)).collect();
            let ret = fp.sig.output.as_ref().map_or_else(|| "()".to_string(), &mut fmt_rec);
            let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
            let constness = if fp.header.is_const { "const " } else { "" };
            let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
            let abi = format_abi(&fp.header.abi);
            format!("{abi}{constness}{unsafety}fn({}{})->{ret}", params.join(","), variadic)
        }
        Type::Pat { type_: inner, .. } => fmt_rec(inner),
        Type::QualifiedPath { name, self_type, trait_, args } => format_qualified_path_with(
            name,
            self_type,
            trait_.as_ref(),
            args.as_deref(),
            &mut fmt_rec,
            fmt_generic_args,
        ),
        _ => "_".to_string(),
    }
}
