//! Format helpers for `rustdoc_types` values.
//!
//! Provides short-name string representations of `Type`, `GenericArgs`,
//! `GenericBound`, `WherePredicate`, and `Abi` values used in Phase 2
//! structural equality checks.  All formatting uses L1 resolution (only
//! the last path segment is kept for named types).

use std::collections::{BTreeSet, HashMap};

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

/// Applies a canonical generic-name map to a plain expression string by replacing
/// each whole-word occurrence of a generic name with its positional placeholder.
///
/// Used to canonicalize const generic default values, const generic arguments in
/// type paths (e.g. `Foo<N>` where `N` is a const generic parameter), and
/// associated-const default expressions that may reference parent generic
/// parameters.  Replacement is whole-word only (bounded by non-alphanumeric /
/// non-`_` / non-`.` characters) so that `"N"` in `"N + 1"` is replaced by `"#0"`
/// but `"Nested"` and `"crate.N"` are left intact.
///
/// **Path guard**: `.` (the normalized form of `::`) is treated as a continuation
/// character for identifiers so that a path-qualified name like `crate.N` is never
/// canonicalized.  Without this guard, `crate::N` (after `::`→`.` normalization)
/// would have `N` replaced by `#0`, making two distinct external constants like
/// `crate::N` and `crate::M` appear equal when the generic param also happens to
/// be named `N` or `M`.
///
/// **Literal guard**: replacements inside char literals (`'N'`) and string
/// literals (`"N"`) are skipped via a pre-pass that computes literal byte spans.
/// Without this guard, a whole-word check would match `N` inside `'N'` or `"N"`
/// (quotes are neither alphanumeric nor `_`, so both word-boundary conditions
/// pass) and produce `'#0'` / `"#0"`, causing two structurally different
/// concrete literal values (e.g. `"N"` vs `"M"`) to compare equal.
///
/// The pre-pass recognises:
/// - double-quoted string literals: `"..."` with `\"` escaping
/// - single-quoted char literals: `'...'` with `\'` escaping (covers single
///   identifier characters like `'N'` and multi-char escapes like `'\n'`)
///
/// If `canon` is empty or `s` is empty, the original string is returned unchanged.
pub(super) fn apply_canon_to_str(s: &str, canon: &HashMap<String, String>) -> String {
    if canon.is_empty() || s.is_empty() {
        return s.to_owned();
    }

    let mut result = s.to_owned();
    for (name, placeholder) in canon {
        if name.is_empty() {
            continue;
        }
        // --- Pre-pass: collect literal byte ranges in the *current* result ---
        // Spans are half-open [start, end) byte offsets into `result`.
        // Must be re-computed for each name because earlier replacements may have
        // shifted byte offsets — a stale span from the original `s` would no longer
        // cover the same bytes after a previous substitution widened the string.
        //
        // We recognise double-quoted strings (`"..."`) and single-quoted char literals
        // (`'...'`), both with backslash escaping.  Raw strings and byte strings are
        // rare in const generic expressions produced by rustdoc; if they appear the
        // guard does not cover them, but the worst case is symmetric distortion
        // (identical literals on both sides still produce the same canonicalized
        // string), which at worst yields a false equal, not a false unequal.
        // Structural-inequality is the D3 fail-closed direction, so this is the
        // safer failure mode.
        let literal_spans = collect_literal_spans(&result);

        // Scan for whole-word occurrences of `name` and replace them,
        // skipping any match whose byte range overlaps a literal span.
        let mut out = String::with_capacity(result.len());
        let mut remaining = result.as_str();
        // `base` tracks the byte offset of `remaining` within `result`.
        let mut base: usize = 0;
        while let Some(rel_pos) = remaining.find(name.as_str()) {
            let pos = base + rel_pos;
            let after_pos = pos + name.len();

            // Check word boundary: char before the match.
            let char_before = if rel_pos == 0 {
                None
            } else {
                remaining.get(..rel_pos).and_then(|s| s.chars().next_back())
            };
            // `.` (the normalized form of `::`) is treated as a continuation
            // character so that path-qualified names like `crate.N` are never
            // canonicalized (the `N` segment is part of a path, not a bare
            // reference to the generic parameter).
            let before_ok =
                char_before.is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '.');

            // Check word boundary: char after the match.
            let char_after = remaining.get(rel_pos + name.len()..).and_then(|s| s.chars().next());
            let after_ok = char_after.is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '.');

            // Literal guard: skip when the match overlaps any pre-computed literal span.
            let inside_literal =
                literal_spans.iter().any(|&(lstart, lend)| pos >= lstart && after_pos <= lend);

            if before_ok && after_ok && !inside_literal {
                out.push_str(&remaining[..rel_pos]);
                out.push_str(placeholder);
                let consumed = rel_pos + name.len();
                remaining = &remaining[consumed..];
                base += consumed;
            } else {
                // Not a whole-word match (or inside a literal); advance past this
                // occurrence.  Advance by the length of the next UTF-8 char at
                // `rel_pos` (not just +1 byte) to avoid splitting a multi-byte
                // character and panicking on the subsequent byte-slice.
                let char_len = remaining[rel_pos..].chars().next().map_or(1, char::len_utf8);
                let advance = rel_pos + char_len;
                out.push_str(&remaining[..advance]);
                remaining = &remaining[advance..];
                base += advance;
            }
        }
        out.push_str(remaining);
        result = out;
    }
    result
}

/// Returns half-open `[start, end)` byte spans of double-quoted string literals
/// and single-quoted char literals found in `s`.
///
/// Both `"..."` and `'...'` forms are recognised; backslash escaping is handled
/// by advancing two bytes when a `\` is encountered inside a literal (which
/// skips the escaped character regardless of its width).  The spans are
/// **inclusive of the opening and closing quote** so that callers can test
/// `pos >= start && pos+len <= end` to detect matches that land entirely inside
/// a quoted literal.
fn collect_literal_spans(s: &str) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        if byte == b'"' || byte == b'\'' {
            let quote = byte;
            let start = i;
            i += 1;
            loop {
                match bytes.get(i) {
                    None => break,
                    Some(&b'\\') => {
                        // Skip escaped character (advances past both `\` and the
                        // escaped byte; for multi-byte UTF-8 sequences only the
                        // first byte is skipped, but that is sufficient to prevent
                        // the closing-quote detector from firing on `\'` or `\"`.
                        i += 2;
                    }
                    Some(&b) if b == quote => {
                        i += 1; // consume closing quote
                        break;
                    }
                    Some(_) => {
                        i += 1;
                    }
                }
            }
            spans.push((start, i));
        } else {
            i += 1;
        }
    }
    spans
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
            // For projection paths like `T::Item` (where `T` is a generic parameter),
            // preserve the full `<canon(T)>::Item` form so that `T::Item` and `U::Item`
            // produce distinct strings when `T` and `U` map to different positional indices.
            // For ordinary resolved paths like `std::vec::Vec` or `Clone`, take only the
            // last segment (current behaviour) because the prefix is a module path, not a
            // generic, and comparing short names is what the rest of the evaluator expects.
            let path_str: &str = &p.path;
            let display_base = if let Some(sep_pos) = path_str.find("::") {
                let prefix = &path_str[..sep_pos];
                let rest = &path_str[sep_pos..]; // starts with "::"
                if !canon.is_empty() && canon.contains_key(prefix) {
                    // The prefix is a generic parameter name — preserve qualified form and
                    // apply the canon map so `T::Item` and `U::Item` produce distinct keys.
                    let canon_prefix = canon.get(prefix).map(|s| s.as_str()).unwrap_or(prefix);
                    format!("{canon_prefix}{rest}")
                } else {
                    // Ordinary qualified path (e.g. `std::vec::Vec`) — use the last segment.
                    p.path.rsplit("::").next().unwrap_or(path_str).to_string()
                }
            } else {
                // No `::` — single-segment name (e.g. `Clone`, `String`).
                p.path.clone()
            };
            if let Some(args) = &p.args {
                let rendered = format_generic_args_with_canon(args, canon);
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
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            format!("&{mut_str}{}", format_type_with_canon(inner, canon))
        }
        Type::Slice(inner) => format!("[{}]", format_type_with_canon(inner, canon)),
        Type::Array { type_: inner, len } => {
            // Apply the canon map to the array length expression so that a const
            // generic rename (e.g. `N` → `M` in `trait A<const N>: Foo<[u8; N]>`)
            // does not produce a false structural mismatch.  Mirrors the treatment of
            // `GenericArg::Const` and `Term::Constant` equality bindings.
            let safe_len = apply_canon_to_str(&len.replace("::", "."), canon);
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
            // D3 fail-closed: Outlives, Use, and HRTB binders inside ImplTrait are outside ADR
            // `2026-05-13-1153` D3 scope.  Return a sentinel string so that ImplTrait
            // types carrying such bounds produce a unique, non-matching string rather
            // than silently comparing equal when both sides happen to be identical text.
            let has_unsupported = bounds.iter().any(|b| match b {
                rustdoc_types::GenericBound::Outlives(_) | rustdoc_types::GenericBound::Use(_) => {
                    true
                }
                // A TraitBound with a non-empty HRTB binder (`for<'a>`) is also
                // outside D3 scope.
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
                        // generic_params is empty here because the HRTB guard above already
                        // returned early.
                        let _ = generic_params;
                        Some(format!("{modifier_str}{short}{args_str}"))
                    }
                    // Outlives and Use are handled by the fail-closed guard above; this
                    // branch is unreachable when `has_unsupported` is true.
                    rustdoc_types::GenericBound::Outlives(_)
                    | rustdoc_types::GenericBound::Use(_) => None,
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
        }
        Type::DynTrait(dyn_trait) => {
            // D3 fail-closed: a `dyn Trait` whose `PolyTrait` entries carry a
            // non-empty HRTB binder (`for<'a> Trait<'a>`) is outside ADR
            // `2026-05-13-1153` D3 scope.  Return a sentinel so it never
            // compares equal to another type.
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
                            let s = format_generic_args_with_canon(a, canon);
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{args_str}")
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
            // D3 fail-closed: a function pointer with a HRTB binder (`for<'a> fn(...)`)
            // is outside ADR `2026-05-13-1153` D3 scope.  Return a sentinel so it never
            // compares equal to another type.
            if !fp.generic_params.is_empty() {
                return "<UNSUPPORTED:FunctionPointer>".to_string();
            }
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
            format!("{abi}{constness}{unsafety}fn({}{})->{ret}", params.join(","), variadic)
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
                    // Apply the canon map to const generic argument expressions so that
                    // a const param rename (e.g. `N` → `M` in `trait A<const N>: Foo<N>`)
                    // does not produce a false structural mismatch.  The expression is
                    // also `::`-normalized (same as in the canon-unaware path) before
                    // whole-word canon substitution is applied.
                    GenericArg::Const(c) => apply_canon_to_str(&c.expr.replace("::", "."), canon),
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
                        // Apply the canon map to the const expression so that a const generic
                        // rename (e.g. `N` → `M` in `trait A<const N>: Foo<LEN = N>`) does
                        // not produce a false structural mismatch.  Mirrors the treatment of
                        // `GenericArg::Const` above.
                        let rhs = apply_canon_to_str(&cv.expr.replace("::", "."), canon);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        // Use the canon-aware variant so that generic names inside the
                        // constraint bounds (e.g. `Iterator<Item: Into<T>>`) are also
                        // canonicalized.
                        let rhs = format_generic_bounds_with_canon(bounds, canon);
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

/// Formats a `rustdoc_types::Type` the same way as [`format_type`], but strips
/// generic args that are declared as type or lifetime parameters on the enclosing
/// `impl` block.
///
/// The primary use case is building the identity key for `impl` blocks in
/// [`build_impl_identity_map`]: `impl<S> TaskOperationInteractor<S>: TaskOperationService`
/// should produce the key `"TaskOperationInteractor: TaskOperationService"` (with `<S>`
/// removed), matching the catalogue A-codec key `"TaskOperationInteractor: TaskOperationService"`.
///
/// Stripping rules applied to `AngleBracketed` generic arg lists:
/// - `GenericArg::Type(Type::Generic(name))` where `name ∈ type_params` → removed.
/// - `GenericArg::Lifetime(lt)` where `lt` without its leading `'` is in `type_params`
///   → removed (impl-block lifetime params are identity-neutral).  Concrete lifetimes
///   such as `'static` whose bare name is NOT in `type_params` are preserved.
/// - `GenericArg::Type(t)` for composite types — recurse with
///   `format_type_strip_type_params(t, type_params)` so nested impl-block type params
///   inside `Vec<S>`, tuples, or borrowed refs are also stripped.
/// - All other args (const values, `_`) are preserved as-is.
/// - When all angle-bracketed args are stripped, the `<…>` brackets are also removed.
///
/// All `Type` variants recurse into `format_type_strip_type_params` (not
/// `format_type`) so that impl-block generics are stripped at every depth.
pub(super) fn format_type_strip_type_params(ty: &Type, type_params: &BTreeSet<String>) -> String {
    // Fast path: when there are no type params to strip, delegate to `format_type`
    // directly so the output is bit-for-bit identical for every supported variant
    // (including `ImplTrait` with `Outlives`/`Use`/HRTB bounds and `DynTrait` with
    // HRTB binders).  This guarantees that `format_type_strip_type_params(t, &[]) ==
    // format_type(t)` for all `t`, which prevents false identity mismatches when the
    // caller inadvertently passes an empty set.
    if type_params.is_empty() {
        return format_type(ty);
    }

    // Helper closure to reduce repetition for single-inner-type variants.
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
        // GenericArg-level filtering (format_generic_args_strip_type_params)
        // removes params from angle-bracketed lists, but bare `Type::Generic`
        // values also occur in non-list positions (BorrowedRef inner type,
        // Tuple elements, Array element type, FunctionPointer/Parenthesized
        // inputs after the explicit filter, etc.).  Return `_` so the
        // surrounding type string remains structurally valid (e.g. `&_`,
        // `[_; N]`, `(_, u32)`) rather than collapsing to an empty/broken form.
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
            // A bare const param (e.g. `N` in `impl<const N: usize>`) appears
            // as a plain identifier in `len`.  Replace it with `_` so that
            // `[u8; N]` normalizes to `[u8; _]`, matching the catalogue key.
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
            // Strip the object lifetime if it is an impl-block lifetime param
            // (e.g. `dyn Bar + 'a` where `'a` is declared on `impl<'a>`).
            // Concrete object lifetimes like `'static` are preserved.
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
            // Recurse with strip() so that impl-block type params in any
            // position (bare `S`, wrapped `&S`, nested `Vec<S>`, etc.) are
            // replaced with `_` by the `Type::Generic` arm above.
            let params: Vec<String> = fp.sig.inputs.iter().map(|(_, t)| strip(t)).collect();
            let ret = fp.sig.output.as_ref().map_or_else(|| "()".to_string(), &strip);
            let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
            let constness = if fp.header.is_const { "const " } else { "" };
            let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
            let abi = format_abi(&fp.header.abi);
            // Preserve HRTB binders (`for<'a, T>`) from the function type's own
            // `generic_params` — these are part of the structural identity and must
            // not be stripped.  Mirrors `format_type`'s binder rendering logic so
            // that `impl<S> Trait for for<'a> fn(&'a S)` produces the same key
            // prefix as the non-strip path (`for<'a>fn(&'a _)->()` vs `<UNSUPPORTED>`).
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
        // Unknown/future variants that cannot carry impl-block generics.
        other => format_type(other),
    }
}

/// Formats `GenericArgs`, filtering out angle-bracketed args that are
/// impl-block type parameters or lifetime parameters.
///
/// Returns the comma-joined rendered args **without** angle brackets; the
/// caller wraps with `<…>` only when the result is non-empty.
fn format_generic_args_strip_type_params(
    args: &GenericArgs,
    type_params: &BTreeSet<String>,
) -> String {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            // Retain only args that are NOT impl-block type/lifetime parameters.
            let positional: Vec<String> = args
                .iter()
                .filter_map(|arg| match arg {
                    // Strip bare type params declared on the impl block.
                    GenericArg::Type(Type::Generic(name))
                        if type_params.contains(name.as_str()) =>
                    {
                        None
                    }
                    // Strip impl-block lifetime params only.  A lifetime arg `'lt`
                    // in rustdoc stores the name WITH the leading `'`, while
                    // `GenericParamDef::name` stores it WITHOUT (e.g. `"a"` for
                    // `'a`).  Strip when the bare name (after removing `'`) is in
                    // `type_params`.  Concrete lifetimes like `'static` are
                    // preserved because they are not in `type_params`.
                    GenericArg::Lifetime(lt) => {
                        let bare = lt.trim_start_matches('\'');
                        if type_params.contains(bare) { None } else { Some(lt.clone()) }
                    }
                    // Recurse into composite types so nested impl-block type params
                    // (e.g. `S` inside `Vec<S>`) are also stripped.
                    GenericArg::Type(t) => Some(format_type_strip_type_params(t, type_params)),
                    // Strip const generic params declared on the impl block
                    // (e.g. `N` in `impl<const N: usize> Foo<N>`).  The
                    // `expr` for a bare const param is the param name itself.
                    GenericArg::Const(c) => {
                        let expr = c.expr.replace("::", ".");
                        if type_params.contains(expr.as_str()) { None } else { Some(expr) }
                    }
                    GenericArg::Infer => Some("_".to_string()),
                })
                .collect();
            // Associated-type constraints: recurse with the strip helper so that
            // impl-block type params nested inside constraint RHS types
            // (e.g. `Foo<Assoc<Item = Vec<T>>>`) are also stripped.
            let mut constraint_parts: Vec<String> = constraints
                .iter()
                .map(|c| match &c.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                        let rhs = format_type_strip_type_params(ty, type_params);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
                        let rhs = cv.expr.replace("::", ".");
                        // Strip const param names from const equality bindings.
                        let rhs =
                            if type_params.contains(rhs.as_str()) { "_".to_string() } else { rhs };
                        if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
                    }
                    // Use strip-aware bounds formatter so impl-block generics
                    // inside constraint bounds (e.g. `Foo<Assoc: Bar<T>>`) are stripped.
                    AssocItemConstraintKind::Constraint(bounds) => {
                        let rhs = format_generic_bounds_strip_type_params(bounds, type_params);
                        if rhs.is_empty() { c.name.clone() } else { format!("{}:{}", c.name, rhs) }
                    }
                })
                .collect();
            constraint_parts.sort();
            let mut parts = positional;
            parts.extend(constraint_parts);
            parts.join(", ")
        }
        // Parenthesized args (e.g. `Fn(T) -> R` for callable traits): recurse
        // with the strip helper so that impl-block generics nested inside
        // callable arg/return types (e.g. `Fn(S) -> S`, `Fn(&S) -> S` in
        // `impl<S>`) are replaced with `_` by the `Type::Generic` arm above.
        GenericArgs::Parenthesized { inputs, output } => {
            let params: Vec<String> =
                inputs.iter().map(|t| format_type_strip_type_params(t, type_params)).collect();
            let ret = output.as_ref().map_or_else(
                || "()".to_string(),
                |t| format_type_strip_type_params(t, type_params),
            );
            format!("({})->{}", params.join(","), ret)
        }
        _ => String::new(),
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

/// Strip-aware variant of [`format_generic_bounds`].
///
/// Applies `format_generic_args_strip_type_params` to inner trait-bound generic
/// args so that impl-block type parameters that appear inside constraint bounds
/// such as `Foo<Assoc: Bar<T>>` are stripped from the rendered string.
///
/// Lifetime bounds (`Outlives`) and use-capture (`Use`) bounds are passed
/// through unchanged (same as `format_generic_bounds`).
fn format_generic_bounds_strip_type_params(
    bounds: &[GenericBound],
    type_params: &BTreeSet<String>,
) -> String {
    let mut strs: Vec<String> = bounds
        .iter()
        .filter_map(|b| match b {
            GenericBound::TraitBound { trait_, modifier, generic_params } => {
                let short = trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
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

/// Canon-aware formatter for a `WherePredicate`. Applies `canon` to the
/// predicate LHS (`Type::Generic` names) and to any inner `Type::Generic`
/// occurrences inside trait-bound args so that renaming a type parameter
/// (`T → U`) does not change the formatted string. Pass an empty `HashMap`
/// when canonicalization is not desired.
///
/// Used by `build_where_form_view` (ADR `2026-05-13-1153` D1) so that A-side
/// (where-form, name = catalogue-author choice) and C-side (where-form virtual
/// view, name = source `T0`/`T1` for APIT) produce the same string when their
/// constraints are positionally identical.
pub(super) fn format_where_predicate_with_canon(
    pred: &WherePredicate,
    canon: &HashMap<String, String>,
) -> String {
    match pred {
        WherePredicate::BoundPredicate { type_: ty, bounds, generic_params } => {
            let ty_str = format_type_with_canon(ty, canon);
            let bounds_str = format_generic_bounds_with_canon(bounds, canon);
            // Include HRTB type params from the predicate's own binder (e.g. `for<T: Foo>
            // Fn(T): Bar`) so that predicates differing only by their HRTB binder produce
            // distinct strings.
            let hrtb_str = format_hrtb_type_params(generic_params);
            format!("{hrtb_str}{ty_str}:{bounds_str}")
        }
        // Both `LifetimePredicate` and `EqPredicate` are outside ADR `2026-05-13-1153`
        // D3 scope. `build_where_form_view` flags them via `has_unsupported`, but the
        // formatted string is still consumed by `build_generics_fingerprint` which keys
        // `build_trait_method_map`. To preserve distinctness across two methods whose
        // only difference is an unsupported clause, the prefix marker is followed by the
        // predicate's actual content. The `[UNSUPPORTED:` prefix never collides with a
        // well-formed `BoundPredicate` string (which starts with the formatted LHS type).
        WherePredicate::LifetimePredicate { lifetime, outlives } => {
            let bounds_str = outlives.join("+");
            format!("[UNSUPPORTED:Lifetime]{lifetime}:{bounds_str}")
        }
        WherePredicate::EqPredicate { lhs, rhs } => {
            let lhs_str = format_type_with_canon(lhs, canon);
            let rhs_str = match rhs {
                Term::Type(ty) => format_type_with_canon(ty, canon),
                Term::Constant(c) => c.expr.replace("::", "."),
            };
            format!("[UNSUPPORTED:Eq]{lhs_str}={rhs_str}")
        }
    }
}

/// Canon-aware variant of [`format_generic_bounds`]. Applies `canon` to inner
/// `Type::Generic` occurrences inside trait-bound generic args so that bounds
/// like `Into<T>` and `Into<U>` (with `canon["T"] = "#0"`, `canon["U"] = "#0"`)
/// produce the same string.
///
/// D3 scope:
/// - `TraitBound { generic_params: empty }` — fully supported, formatted verbatim.
/// - `Outlives(lt)` — supported: rendered as the lifetime string (e.g. `"'static"`),
///   enabling `F: 'static + Fn(...)` to compare correctly on both sides.
/// - `TraitBound { generic_params: non_empty }` (HRTB binder) — outside D3 scope,
///   returned as `<UNSUPPORTED:HRTB>`.
/// - `Use` — outside D3 scope, returned as `<UNSUPPORTED:Use>`.
pub(super) fn format_generic_bounds_with_canon(
    bounds: &[GenericBound],
    canon: &HashMap<String, String>,
) -> String {
    let mut strs: Vec<String> = bounds
        .iter()
        .map(|b| match b {
            GenericBound::TraitBound { trait_, modifier, generic_params } => {
                // D3 fail-closed: `TraitBound` with a non-empty HRTB binder
                // (`for<'a>` / `for<T: Foo>`) is outside ADR `2026-05-13-1153` D3 scope.
                // Return a sentinel so bounds containing HRTB binders produce a unique
                // non-matching string (including when they appear inside associated-type
                // constraints such as `Iterator<Item: for<'a> Foo<&'a str>>`).  Without
                // this check, `format_hrtb_type_params` silently drops lifetime params,
                // causing `for<'a> Foo<&'a str>` to compare equal to `Foo<&str>`.
                if !generic_params.is_empty() {
                    return "<UNSUPPORTED:HRTB>".to_owned();
                }
                let short = trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
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
                format!("{modifier_str}{short}{args_str}")
            }
            // Outlives bounds (e.g. `'static`, `'a`) are formatted verbatim so that
            // `F: 'static + Fn(...)` produces matching fingerprints on both A-codec
            // and C-side (rustdoc) paths.
            GenericBound::Outlives(lt) => lt.clone(),
            // Use is outside D3 scope.
            GenericBound::Use(_) => "<UNSUPPORTED:Use>".to_owned(),
        })
        .collect();
    strs.sort();
    strs.join("+")
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
        // `format_type_strip_type_params([u8; N], {N})` must yield `[u8; _]`.
        let ty =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "N".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["N"]));
        assert_eq!(result, "[u8; _]", "const param in array length must be replaced with '_'");
    }

    #[test]
    fn test_array_len_concrete_value_is_preserved() {
        // `[u8; 16]` — the literal `16` is NOT a const param and must be preserved.
        let ty =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "16".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["N"]));
        assert_eq!(result, "[u8; 16]", "concrete array length must not be stripped");
    }

    #[test]
    fn test_array_len_const_param_not_in_type_params_is_preserved() {
        // `[u8; N]` where `N` is NOT in `type_params` — preserve as-is.
        let ty =
            Type::Array { type_: Box::new(Type::Primitive("u8".to_owned())), len: "N".to_owned() };
        // `type_params` does NOT contain "N".
        let result = format_type_strip_type_params(&ty, &params(&["T", "S"]));
        assert_eq!(result, "[u8; N]", "array length not in type_params must not be stripped");
    }

    #[test]
    fn test_array_element_type_param_also_stripped() {
        // `impl<T, const N: usize> Foo<[T; N]>`: both the element type `T` and
        // the length `N` are impl-block params and must be stripped.
        // `T` is a `Type::Generic` that the `Type::Generic` arm now maps to `_`
        // when it is in `type_params`; `N` is stripped by the const-len path.
        let ty =
            Type::Array { type_: Box::new(Type::Generic("T".to_owned())), len: "N".to_owned() };
        let result = format_type_strip_type_params(&ty, &params(&["T", "N"]));
        // Both element type and length are stripped → `[_; _]`.
        assert_eq!(
            result, "[_; _]",
            "both element type param and length param must be stripped; got: {result}"
        );
    }

    // --- GenericArg::Const stripping (angle-bracketed const params) ---

    #[test]
    fn test_angle_bracketed_const_generic_param_is_stripped() {
        // `impl<const N: usize> Foo<N>` — `GenericArg::Const` with `expr = "N"`.
        // The positional arg must be stripped and the brackets removed.
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
        // `Foo<16>` — const value `16` is NOT a param and must be preserved.
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
        // `Trait<LEN = N>` where `N` is a const param: the RHS must be replaced with `_`.
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
        // Expect `LEN=_` (const param N stripped to `_` in the equality RHS).
        assert_eq!(
            result, "LEN=_",
            "const param in equality binding RHS must be stripped; got: {result}"
        );
    }

    #[test]
    fn test_const_equality_binding_concrete_value_preserved() {
        // `Trait<LEN = 16>` — concrete value; must not be stripped.
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
        // `Foo<[u8; N]>` — const param inside an array inside a generic arg of a path type.
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
        // `Foo<[u8; _]>` — inner array len stripped, path shortened to last segment.
        assert_eq!(
            result, "Foo<[u8; _]>",
            "const param inside nested array inside generic must be stripped; got: {result}"
        );
    }
}
