//! Format helpers for `rustdoc_types` values.
//!
//! Provides short-name string representations of `Type`, `GenericArgs`,
//! `GenericBound`, `WherePredicate`, and `Abi` values used in Phase 2
//! structural equality checks.  All formatting uses L1 resolution (only
//! the last path segment is kept for named types).

use std::collections::HashMap;

use rustdoc_types::{
    Abi, AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, GenericParamDef,
    GenericParamDefKind, Generics, Term, Type, WherePredicate,
};

/// Formats a `rustdoc_types::Abi` as an `extern "…"` string prefix.
///
/// Returns an empty string for `Abi::Rust` (implicit ABI requires no prefix).
/// All other ABIs render as `extern "<name>" ` with a trailing space so the
/// caller can unconditionally prepend it to the `fn` keyword.
pub(super) fn format_abi(abi: &Abi) -> String {
    match abi {
        Abi::Rust => String::new(),
        Abi::C { unwind: false } => "extern \"C\" ".to_string(),
        Abi::C { unwind: true } => "extern \"C-unwind\" ".to_string(),
        Abi::Cdecl { unwind: false } => "extern \"cdecl\" ".to_string(),
        Abi::Cdecl { unwind: true } => "extern \"cdecl-unwind\" ".to_string(),
        Abi::Stdcall { unwind: false } => "extern \"stdcall\" ".to_string(),
        Abi::Stdcall { unwind: true } => "extern \"stdcall-unwind\" ".to_string(),
        Abi::Fastcall { unwind: false } => "extern \"fastcall\" ".to_string(),
        Abi::Fastcall { unwind: true } => "extern \"fastcall-unwind\" ".to_string(),
        Abi::Aapcs { unwind: false } => "extern \"aapcs\" ".to_string(),
        Abi::Aapcs { unwind: true } => "extern \"aapcs-unwind\" ".to_string(),
        Abi::Win64 { unwind: false } => "extern \"win64\" ".to_string(),
        Abi::Win64 { unwind: true } => "extern \"win64-unwind\" ".to_string(),
        Abi::SysV64 { unwind: false } => "extern \"sysv64\" ".to_string(),
        Abi::SysV64 { unwind: true } => "extern \"sysv64-unwind\" ".to_string(),
        Abi::System { unwind: false } => "extern \"system\" ".to_string(),
        Abi::System { unwind: true } => "extern \"system-unwind\" ".to_string(),
        Abi::Other(name) => format!("extern \"{name}\" "),
    }
}

/// Builds a canonical name map from a `Generics` parameter list for use in
/// `format_type_with_canon`.
///
/// Maps each type and const generic parameter's source name to a positional
/// placeholder `"#0"`, `"#1"`, … (in declaration order, counting only
/// type/const params, not lifetime params).  Lifetime parameters are excluded
/// because `format_type` does not emit them as `Type::Generic` values.
///
/// Passing this map to `format_type_with_canon` ensures that renaming a generic
/// parameter (e.g. `T` → `U`) does not change the formatted string, so two
/// function signatures that differ only in generic parameter names still compare
/// as structurally equal — consistent with `generics_structurally_equal`'s
/// name-independent comparison.
pub(super) fn build_generic_canon_map(generics: &Generics) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut idx: usize = 0;
    for p in &generics.params {
        match &p.kind {
            GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. } => {
                map.insert(p.name.clone(), format!("#{idx}"));
                idx += 1;
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }
    map
}

/// Formats a `rustdoc_types::Type` as a short-name string at L1 resolution,
/// optionally canonicalizing generic parameter names via `canon`.
///
/// When `canon` is non-empty, `Type::Generic(name)` values are replaced by the
/// positional placeholder stored in the map (e.g. `"#0"`, `"#1"`).  This ensures
/// that two signatures that differ only in generic parameter names — such as
/// `fn f<T>(x: T)` vs `fn f<U>(x: U)` — produce identical formatted strings and
/// compare as structurally equal, consistent with `generics_structurally_equal`.
///
/// Pass an empty `HashMap` (or use `format_type` directly) when generic name
/// canonicalization is not desired.
pub(super) fn format_type_with_canon(ty: &Type, canon: &HashMap<String, String>) -> String {
    match ty {
        Type::Generic(name) => {
            if let Some(pos) = canon.get(name.as_str()) {
                pos.clone()
            } else {
                name.clone()
            }
        }
        Type::ResolvedPath(p) => {
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            if let Some(args) = &p.args {
                let rendered = format_generic_args_with_canon(args, canon);
                if rendered.is_empty() { short } else { format!("{short}<{rendered}>") }
            } else {
                short
            }
        }
        Type::Primitive(name) => name.clone(),
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            format!("&{mut_str}{}", format_type_with_canon(inner, canon))
        }
        Type::Slice(inner) => format!("[{}]", format_type_with_canon(inner, canon)),
        Type::Array { type_: inner, len } => {
            let safe_len = len.replace("::", ".");
            format!("[{}; {}]", format_type_with_canon(inner, canon), safe_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(|t| format_type_with_canon(t, canon)).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", format_type_with_canon(inner, canon))
        }
        Type::ImplTrait(bounds) => {
            let mut parts: Vec<String> = bounds
                .iter()
                .filter_map(|b| match b {
                    rustdoc_types::GenericBound::TraitBound {
                        trait_,
                        modifier,
                        generic_params,
                    } => {
                        let short =
                            trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                        let args_str = trait_
                            .args
                            .as_deref()
                            .map(|a| {
                                let s = format_generic_args_with_canon(a, canon);
                                if s.is_empty() { String::new() } else { format!("<{s}>") }
                            })
                            .unwrap_or_default();
                        use rustdoc_types::TraitBoundModifier;
                        let modifier_str = match modifier {
                            TraitBoundModifier::None => "",
                            TraitBoundModifier::Maybe => "?",
                            TraitBoundModifier::MaybeConst => "~const ",
                        };
                        let hrtb_str = format_hrtb_type_params(generic_params);
                        Some(format!("{modifier_str}{short}{args_str}{hrtb_str}"))
                    }
                    rustdoc_types::GenericBound::Outlives(lt) => Some(lt.clone()),
                    rustdoc_types::GenericBound::Use(use_bounds) => {
                        if use_bounds.is_empty() {
                            None
                        } else {
                            let parts: Vec<String> = use_bounds
                                .iter()
                                .map(|b| match b {
                                    rustdoc_types::PreciseCapturingArg::Lifetime(lt) => lt.clone(),
                                    rustdoc_types::PreciseCapturingArg::Param(name) => name.clone(),
                                })
                                .collect();
                            Some(format!("use<{}>", parts.join(",")))
                        }
                    }
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
        }
        Type::DynTrait(dyn_trait) => {
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
                            let s = format_generic_args_with_canon(a, canon);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    let hrtb_str = format_hrtb_type_params(&pt.generic_params);
                    format!("{hrtb_str}{short}{args_str}")
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            let lifetime_str =
                dyn_trait.lifetime.as_deref().map(|lt| format!(" + {lt}")).unwrap_or_default();
            if rendered.is_empty() {
                format!("dyn _{lifetime_str}")
            } else {
                format!("dyn {rendered}{lifetime_str}")
            }
        }
        Type::FunctionPointer(fp) => {
            let params: Vec<String> =
                fp.sig.inputs.iter().map(|(_, t)| format_type_with_canon(t, canon)).collect();
            let ret = fp
                .sig
                .output
                .as_ref()
                .map_or_else(|| "()".to_string(), |t| format_type_with_canon(t, canon));
            let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
            let constness = if fp.header.is_const { "const " } else { "" };
            let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
            let abi = format_abi(&fp.header.abi);
            let hrtb = if fp.generic_params.is_empty() {
                String::new()
            } else {
                let lt_strs: Vec<String> = fp
                    .generic_params
                    .iter()
                    .filter_map(|p| {
                        if matches!(p.kind, GenericParamDefKind::Lifetime { .. }) {
                            Some(format!("'{}", p.name))
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
        Type::Pat { type_: inner, .. } => format_type_with_canon(inner, canon),
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let trait_str = trait_
                .as_ref()
                .map(|p| {
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    let trait_args_str = p
                        .args
                        .as_deref()
                        .map(|a| {
                            let s = format_generic_args_with_canon(a, canon);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{trait_args_str}")
                })
                .unwrap_or_else(|| "_".to_string());
            let self_str = format_type_with_canon(self_type, canon);
            let args_str = args
                .as_deref()
                .map_or_else(String::new, |a| format_generic_args_with_canon(a, canon));
            if args_str.is_empty() {
                format!("<{self_str} as {trait_str}>::{name}")
            } else {
                format!("<{self_str} as {trait_str}>::{name}<{args_str}>")
            }
        }
        _ => "_".to_string(),
    }
}

/// Formats `GenericArgs` with generic parameter name canonicalization.
///
/// Mirrors `format_generic_args` but threads `canon` through all recursive
/// `format_type_with_canon` calls so that generic parameter names in argument
/// positions are also canonicalized.
fn format_generic_args_with_canon(args: &GenericArgs, canon: &HashMap<String, String>) -> String {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            let positional: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Type(t) => format_type_with_canon(t, canon),
                    GenericArg::Lifetime(lt) => lt.clone(),
                    GenericArg::Const(c) => c.expr.replace("::", "."),
                    GenericArg::Infer => "_".to_string(),
                })
                .collect();
            let mut constraint_parts: Vec<String> = constraints
                .iter()
                .map(|c| match &c.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                        let rhs = format_type_with_canon(ty, canon);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
                        let rhs = cv.expr.replace("::", ".");
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        let rhs = format_generic_bounds(bounds);
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
            let params: Vec<String> =
                inputs.iter().map(|t| format_type_with_canon(t, canon)).collect();
            let ret = output
                .as_ref()
                .map_or_else(|| "()".to_string(), |t| format_type_with_canon(t, canon));
            format!("({})->{}", params.join(","), ret)
        }
        _ => String::new(),
    }
}

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
pub(super) fn format_hrtb_type_params(generic_params: &[GenericParamDef]) -> String {
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
pub(super) fn format_type(ty: &Type) -> String {
    match ty {
        Type::ResolvedPath(p) => {
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            if let Some(args) = &p.args {
                let rendered = format_generic_args(args);
                if rendered.is_empty() { short } else { format!("{short}<{rendered}>") }
            } else {
                short
            }
        }
        // Generic type parameters (`T`, `U`, etc.) are rendered by name so that
        // positional differences in how generics are used are preserved.  For
        // example `fn f<T, U>(x: T, y: U)` and `fn f<T, U>(x: T, y: T)` must
        // compare as different.  The parameter-binding *value names* (e.g. the `x`
        // in `fn f(x: i32)`) are already excluded elsewhere; generic *type* names
        // are load-bearing structural tokens at L1 and must not be discarded.
        Type::Generic(name) => name.clone(),
        Type::Primitive(name) => name.clone(),
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            format!("&{mut_str}{}", format_type(inner))
        }
        Type::Slice(inner) => format!("[{}]", format_type(inner)),
        Type::Array { type_: inner, len } => {
            let safe_len = len.replace("::", ".");
            format!("[{}; {}]", format_type(inner), safe_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(format_type).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", format_type(inner))
        }
        Type::ImplTrait(bounds) => {
            // Sort bounds so that `impl A + B` and `impl B + A` produce the same string.
            // Include lifetime (`Outlives`) and use-capture (`Use`) bounds so that
            // `impl Copy + 'a` and `impl Copy` produce distinct strings.
            // Also include the modifier and HRTB type params so that `impl ?Sized` vs
            // `impl Sized` and `impl for<T: Foo> Fn(T)` vs `impl for<T: Bar> Fn(T)` differ.
            let mut parts: Vec<String> = bounds
                .iter()
                .filter_map(|b| match b {
                    rustdoc_types::GenericBound::TraitBound {
                        trait_,
                        modifier,
                        generic_params,
                    } => {
                        let short =
                            trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                        let args_str = trait_
                            .args
                            .as_deref()
                            .map(|a| {
                                let s = format_generic_args(a);
                                if s.is_empty() { String::new() } else { format!("<{s}>") }
                            })
                            .unwrap_or_default();
                        use rustdoc_types::TraitBoundModifier;
                        let modifier_str = match modifier {
                            TraitBoundModifier::None => "",
                            TraitBoundModifier::Maybe => "?",
                            TraitBoundModifier::MaybeConst => "~const ",
                        };
                        let hrtb_str = format_hrtb_type_params(generic_params);
                        Some(format!("{modifier_str}{short}{args_str}{hrtb_str}"))
                    }
                    rustdoc_types::GenericBound::Outlives(lt) => Some(lt.clone()),
                    rustdoc_types::GenericBound::Use(use_bounds) => {
                        // use<'a, T> capture bounds: render as `use<...>`.
                        if use_bounds.is_empty() {
                            None
                        } else {
                            let parts: Vec<String> = use_bounds
                                .iter()
                                .map(|b| match b {
                                    rustdoc_types::PreciseCapturingArg::Lifetime(lt) => lt.clone(),
                                    rustdoc_types::PreciseCapturingArg::Param(name) => name.clone(),
                                })
                                .collect();
                            Some(format!("use<{}>", parts.join(",")))
                        }
                    }
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
        }
        Type::DynTrait(dyn_trait) => {
            // Sort trait bounds so that `dyn A + B` and `dyn B + A` produce the same string.
            // Include HRTB type params from `PolyTrait.generic_params` so that
            // `dyn for<T: Foo> Bar` and `dyn for<T: Baz> Bar` produce distinct strings.
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
                            let s = format_generic_args(a);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    let hrtb_str = format_hrtb_type_params(&pt.generic_params);
                    format!("{hrtb_str}{short}{args_str}")
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            // Include the lifetime bound so `dyn Foo + 'a` and `dyn Foo + 'static`
            // produce distinct strings.
            let lifetime_str =
                dyn_trait.lifetime.as_deref().map(|lt| format!(" + {lt}")).unwrap_or_default();
            if rendered.is_empty() {
                format!("dyn _{lifetime_str}")
            } else {
                format!("dyn {rendered}{lifetime_str}")
            }
        }
        Type::FunctionPointer(fp) => {
            let params: Vec<String> = fp.sig.inputs.iter().map(|(_, t)| format_type(t)).collect();
            let ret = fp.sig.output.as_ref().map_or_else(|| "()".to_string(), format_type);
            let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
            let constness = if fp.header.is_const { "const " } else { "" };
            let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
            let abi = format_abi(&fp.header.abi);
            // Include higher-ranked lifetime and type params (e.g. `for<'a, T: Foo>`)
            // in the key.  Lifetime params are rendered as `'name`; type params use
            // `format_hrtb_type_params` so that `for<T: Foo>` and `for<T: Bar>` differ.
            // Both sets are joined into a single `for<…>[…]` prefix.
            let hrtb = if fp.generic_params.is_empty() {
                String::new()
            } else {
                let lt_strs: Vec<String> = fp
                    .generic_params
                    .iter()
                    .filter_map(|p| {
                        if matches!(p.kind, GenericParamDefKind::Lifetime { .. }) {
                            Some(format!("'{}", p.name))
                        } else {
                            None
                        }
                    })
                    .collect();
                let type_str = format_hrtb_type_params(&fp.generic_params);
                if lt_strs.is_empty() {
                    // No lifetime binders: emit type HRTB only (e.g. `[T:Foo]`).
                    type_str
                } else {
                    // Lifetime binders present: emit `for<'a,…>` followed by optional
                    // type HRTB suffix.
                    format!("for<{}>{type_str}", lt_strs.join(","))
                }
            };
            format!("{hrtb}{abi}{constness}{unsafety}fn({}{})->{ret}", params.join(","), variadic)
        }
        // Pattern types (RFC 3437): render as the underlying base type.
        Type::Pat { type_: inner, .. } => format_type(inner),
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let trait_str = trait_
                .as_ref()
                .map(|p| {
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    let trait_args_str = p
                        .args
                        .as_deref()
                        .map(|a| {
                            let s = format_generic_args(a);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{trait_args_str}")
                })
                .unwrap_or_else(|| "_".to_string());
            let self_str = format_type(self_type);
            let args_str = args.as_deref().map_or_else(String::new, format_generic_args);
            if args_str.is_empty() {
                format!("<{self_str} as {trait_str}>::{name}")
            } else {
                format!("<{self_str} as {trait_str}>::{name}<{args_str}>")
            }
        }
        _ => "_".to_string(),
    }
}

pub(super) fn format_generic_args(args: &GenericArgs) -> String {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            // Type args and lifetimes are position-sensitive — preserve their order.
            let positional: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Type(t) => format_type(t),
                    GenericArg::Lifetime(lt) => lt.clone(),
                    GenericArg::Const(c) => c.expr.replace("::", "."),
                    GenericArg::Infer => "_".to_string(),
                })
                .collect();
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
                        let rhs = format_type(ty);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
                        let rhs = cv.expr.replace("::", ".");
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        let rhs = format_generic_bounds(bounds);
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
            let params: Vec<String> = inputs.iter().map(format_type).collect();
            let ret = output.as_ref().map_or_else(|| "()".to_string(), format_type);
            format!("({})->{}", params.join(","), ret)
        }
        _ => String::new(),
    }
}

/// Formats a slice of `GenericBound` values as a sorted, `+`-joined string.
///
/// Bounds are sorted alphabetically before joining so that semantically
/// equivalent bound sets (e.g. `A + B` vs `B + A`) produce identical strings.
/// Includes trait generic arguments so that `Iterator<Item = u8>` and
/// `Iterator<Item = u16>` produce distinct strings.
/// Formats a slice of `GenericBound` values as a sorted, `+`-joined string.
///
/// Bounds are sorted alphabetically before joining so that semantically
/// equivalent bound sets (e.g. `A + B` vs `B + A`) produce identical strings.
/// Includes trait generic arguments so that `Iterator<Item = u8>` and
/// `Iterator<Item = u16>` produce distinct strings.
///
/// Lifetime (`Outlives`) and use-capture (`Use`) bounds are also included so
/// that `impl Copy + 'a` and `impl Copy` compare as structurally different.
pub(super) fn format_generic_bounds(bounds: &[GenericBound]) -> String {
    let mut strs: Vec<String> = bounds
        .iter()
        .filter_map(|b| match b {
            GenericBound::TraitBound { trait_, modifier, generic_params } => {
                let short = trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                let args_str = trait_
                    .args
                    .as_deref()
                    .map(|a| {
                        let s = format_generic_args(a);
                        if s.is_empty() { String::new() } else { format!("<{s}>") }
                    })
                    .unwrap_or_default();
                // Include the modifier so `T: Sized` and `T: ?Sized` produce distinct strings.
                use rustdoc_types::TraitBoundModifier;
                let modifier_str = match modifier {
                    TraitBoundModifier::None => "",
                    TraitBoundModifier::Maybe => "?",
                    TraitBoundModifier::MaybeConst => "~const ",
                };
                // Include HRTB type params so `for<T: Foo>` vs `for<T: Bar>` produce
                // distinct strings.  Lifetime binders (`for<'a>`) are skipped since
                // they are identity-preserving at L1.
                let hrtb_str = format_hrtb_type_params(generic_params);
                Some(format!("{modifier_str}{short}{args_str}{hrtb_str}"))
            }
            GenericBound::Outlives(lt) => Some(lt.clone()),
            GenericBound::Use(use_bounds) => {
                if use_bounds.is_empty() {
                    None
                } else {
                    let parts: Vec<String> = use_bounds
                        .iter()
                        .map(|b| match b {
                            rustdoc_types::PreciseCapturingArg::Lifetime(lt) => lt.clone(),
                            rustdoc_types::PreciseCapturingArg::Param(name) => name.clone(),
                        })
                        .collect();
                    Some(format!("use<{}>", parts.join(",")))
                }
            }
        })
        .collect();
    strs.sort();
    strs.join("+")
}

/// Formats a `WherePredicate` as a canonical string for structural comparison.
pub(super) fn format_where_predicate(pred: &WherePredicate) -> String {
    match pred {
        WherePredicate::BoundPredicate { type_: ty, bounds, generic_params } => {
            let ty_str = format_type(ty);
            let bounds_str = format_generic_bounds(bounds);
            // Include HRTB type params from the predicate's own binder (e.g. `for<T: Foo>
            // Fn(T): Bar`) so that predicates differing only by their HRTB binder produce
            // distinct strings.
            let hrtb_str = format_hrtb_type_params(generic_params);
            format!("{hrtb_str}{ty_str}:{bounds_str}")
        }
        WherePredicate::LifetimePredicate { .. } => String::new(),
        WherePredicate::EqPredicate { lhs, rhs } => {
            let lhs_str = format_type(lhs);
            let rhs_str = match rhs {
                Term::Type(ty) => format_type(ty),
                Term::Constant(c) => c.expr.replace("::", "."),
            };
            format!("{lhs_str}={rhs_str}")
        }
    }
}
