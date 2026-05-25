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

/// Builds a canonical name map and synthetic-param occurrence list from a `Generics`
/// parameter list for use in `format_type_with_canon`.
///
/// Returns `(name_map, synthetic_order)` where:
/// - `name_map`: maps each **non-synthetic** type and const generic parameter name to a
///   positional placeholder `"#0"`, `"#1"`, … (in declaration order, counting only
///   type/const params, not lifetime params).  Used by `Type::Generic(name)` lookups for
///   normal named params.
/// - `synthetic_order`: an occurrence-ordered `Vec<String>` of synthetic `impl Trait`
///   occurrence keys (those where `is_synthetic == true`), in declaration order.  Each key
///   starts with its positional placeholder (`"#0"`, `"#1"`, …) and includes the canonical
///   bound fingerprint, so equal positions with different bounds do not collapse.
///
/// Rustdoc desugars `fn(a: impl Into<String>, b: impl Into<String>)` into two synthetic
/// `GenericParamDef` entries both named `"impl Into<String>"`.  Storing them in a HashMap
/// by name causes the second to overwrite the first (key collision).  Separating them into
/// an ordered list avoids this collision: the A-side codec (`Type::ImplTrait`) and the
/// C-side rustdoc output (`Type::Generic("impl Into<String>")`) both consume the list
/// positionally, ensuring the k-th `impl Trait` parameter in the signature maps to the
/// same placeholder on both sides.
///
/// Lifetime parameters are excluded because `format_type` does not emit them as
/// `Type::Generic` values.
///
/// When the input `Generics` has no synthetic params (normal explicit generics only),
/// `synthetic_order` is empty and the behaviour is identical to the previous
/// name-map-only version.
pub(super) fn build_generic_canon_map(
    generics: &Generics,
) -> (HashMap<String, String>, Vec<String>) {
    let mut map = HashMap::new();
    let mut idx: usize = 0;
    for p in &generics.params {
        match &p.kind {
            GenericParamDefKind::Type { is_synthetic, .. } => {
                let placeholder = format!("#{idx}");
                if !*is_synthetic {
                    // Normal named type param: insert into name_map for Generic lookup.
                    map.insert(p.name.clone(), placeholder);
                }
                idx += 1;
            }
            GenericParamDefKind::Const { .. } => {
                // Const params are never synthetic (no impl Trait desugaring).
                map.insert(p.name.clone(), format!("#{idx}"));
                idx += 1;
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }
    let mut synthetic_order: Vec<String> = Vec::new();
    idx = 0;
    for p in &generics.params {
        match &p.kind {
            GenericParamDefKind::Type { bounds, is_synthetic, .. } => {
                if *is_synthetic {
                    // Synthetic impl Trait param: record in occurrence order only.
                    // Do NOT insert into name_map — duplicate names would collide.
                    let placeholder = format!("#{idx}");
                    let bound_sig = format_type_with_canon(&Type::ImplTrait(bounds.clone()), &map);
                    synthetic_order
                        .push(format_impl_trait_occurrence_key(&placeholder, &bound_sig));
                }
                idx += 1;
            }
            GenericParamDefKind::Const { .. } => {
                idx += 1;
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }
    (map, synthetic_order)
}

fn format_impl_trait_occurrence_key(placeholder: &str, impl_sig: &str) -> String {
    format!("{placeholder}:{impl_sig}")
}

fn occurrence_placeholder(occurrence_key: &str) -> &str {
    occurrence_key.split_once(':').map_or(occurrence_key, |(placeholder, _)| placeholder)
}

/// Formats a `rustdoc_types::Type` as a short-name string at L1 resolution with an
/// occurrence cursor for positional `impl Trait` placeholder resolution.
///
/// This is the occurrence-aware variant of [`format_type_with_canon`].  It must be used
/// when the caller needs A-side (`Type::ImplTrait`) and C-side (`Type::Generic("impl ...")`)
/// representations to produce the same placeholder for the same argument position.
///
/// `canon` is the **non-synthetic** name→placeholder map from `build_generic_canon_map`.
/// `synthetic_order` is the occurrence-ordered synthetic occurrence-key list (also from
/// `build_generic_canon_map`) for synthetic `impl Trait` params.  This list is populated
/// on the C-side (rustdoc); on the A-side (catalogue codec) it may be empty.
/// `cursor` is a shared mutable counter incremented each time an `impl Trait` occurrence
/// is consumed; the k-th C-side invocation maps to `synthetic_order[k]`, while the A-side
/// generates the same placeholder prefix and appends its own canonical bound fingerprint
/// when `use_positional_fallback` is true.
///
/// **`use_positional_fallback`:** controls how `Type::ImplTrait` and
/// `Type::Generic("impl ...")` are rendered when `synthetic_order` is empty:
/// - `true` (A-side in an A/C asymmetric comparison): generates on-the-fly positional
///   placeholders as `#(canon.len() + cursor)`, mirroring the placeholder that the C-side
///   assigns to its synthetic params.  Use this when the COUNTERPART has a non-empty
///   `synthetic_order`.
/// - `false` (A-A symmetric comparison or explicit opt-out): falls back to
///   `format_type_with_canon` which renders `Type::ImplTrait` as its literal bound
///   string (e.g. `"impl Display"`), preserving bound-content distinction between
///   different `impl Trait` signatures.
///
/// **Supported bounds check:** if an `ImplTrait` carries unsupported bounds
/// (`Outlives`, `Use`, or HRTB `TraitBound`), the cursor is NOT consumed and the call
/// falls through to `format_type_with_canon` which returns the `<UNSUPPORTED:ImplTrait>`
/// sentinel.  This preserves the D3 fail-closed guarantee.
pub(super) fn format_type_with_canon_occ(
    ty: &Type,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    match ty {
        Type::Generic(name) => {
            // Check if this is a rustdoc-synthetic impl Trait param (name starts with "impl ").
            // Consume one occurrence from synthetic_order and return the corresponding placeholder.
            if name.starts_with("impl ") {
                if !synthetic_order.is_empty() {
                    let cur = *cursor;
                    if let Some(occurrence_key) = synthetic_order.get(cur) {
                        *cursor += 1;
                        return occurrence_key.clone();
                    }
                    // cursor past the end of the list — fall through.
                }
                if use_positional_fallback {
                    // A-side in A/C comparison: generate placeholder using normal-param count as offset.
                    let placeholder = format!("#{}", canon.len() + *cursor);
                    *cursor += 1;
                    let bound_sig = apply_canon_to_str(name, canon);
                    return format_impl_trait_occurrence_key(&placeholder, &bound_sig);
                }
                // A-A comparison: render as literal (fall through to name_map lookup or literal).
            }
            // Normal named generic param: look up in name_map.
            if let Some(pos) = canon.get(name.as_str()) { pos.clone() } else { name.clone() }
        }
        Type::ImplTrait(bounds) => {
            // A-side: impl Trait declared in catalogue.
            // Check bounds are supported before consuming the cursor (D3 fail-closed guard).
            let has_unsupported = bounds.iter().any(|b| match b {
                rustdoc_types::GenericBound::Outlives(_) | rustdoc_types::GenericBound::Use(_) => {
                    true
                }
                rustdoc_types::GenericBound::TraitBound { generic_params, .. } => {
                    !generic_params.is_empty()
                }
            });
            if has_unsupported {
                // Unsupported bounds: do NOT consume cursor — fall through to sentinel.
                return format_type_with_canon(ty, canon);
            }
            // Supported bounds: assign positional placeholder.
            let bound_sig = format_type_with_canon(ty, canon);
            if !synthetic_order.is_empty() {
                // C-side has synthetic params: use the pre-built occurrence list.
                let cur = *cursor;
                if let Some(occurrence_key) = synthetic_order.get(cur) {
                    *cursor += 1;
                    let placeholder = occurrence_placeholder(occurrence_key);
                    return format_impl_trait_occurrence_key(placeholder, &bound_sig);
                }
                // cursor past the end of the list — fall back to on-the-fly generation.
            }
            if use_positional_fallback {
                // A-side in A/C comparison (counterpart has synthetic params).
                // Generate placeholder using normal-param count as offset, mirroring C-side assignment.
                let placeholder = format!("#{}", canon.len() + *cursor);
                *cursor += 1;
                return format_impl_trait_occurrence_key(&placeholder, &bound_sig);
            }
            // A-A symmetric comparison: fall back to literal bound rendering so that
            // fn(Display, Into<String>) and fn(Into<String>, Display) produce different strings.
            bound_sig
        }
        // All other variants: delegate to the occurrence-aware inner formatter, threading
        // the cursor so that any nested ImplTrait or Generic("impl ...") values inside
        // composite types (e.g. `Vec<impl Trait>`) also consume the shared cursor.
        other => format_type_with_canon_occ_inner(
            other,
            canon,
            synthetic_order,
            use_positional_fallback,
            cursor,
        ),
    }
}

/// Inner recursive helper for [`format_type_with_canon_occ`].
///
/// Handles all `Type` variants except `Type::Generic` and `Type::ImplTrait` (which are
/// handled by the outer function).  Mirrors the matching arms in `format_type_with_canon`
/// but threads the occurrence cursor and `use_positional_fallback` flag through recursive
/// calls so that nested `ImplTrait` or `Generic("impl ...")` values inside composite types
/// (e.g. `Vec<impl Trait>`) also consume the shared cursor with the correct behavior.
fn format_type_with_canon_occ_inner(
    ty: &Type,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    // Macro-like closure to reduce boilerplate in recursive cases.
    let rec = |t: &Type, c: &mut usize| {
        format_type_with_canon_occ(t, canon, synthetic_order, use_positional_fallback, c)
    };
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
                let rendered = format_generic_args_with_canon_occ(
                    args,
                    canon,
                    synthetic_order,
                    use_positional_fallback,
                    cursor,
                );
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
            format!("&{lt_str}{mut_str}{}", rec(inner, cursor))
        }
        Type::Slice(inner) => format!("[{}]", rec(inner, cursor)),
        Type::Array { type_: inner, len } => {
            let safe_len = apply_canon_to_str(&len.replace("::", "."), canon);
            format!("[{}; {}]", rec(inner, cursor), safe_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(|t| rec(t, cursor)).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", rec(inner, cursor))
        }
        Type::DynTrait(dyn_trait) => {
            let has_hrtb = dyn_trait.traits.iter().any(|pt| !pt.generic_params.is_empty());
            if has_hrtb {
                return "<UNSUPPORTED:DynTrait>".to_string();
            }
            // Safety: the occurrence cursor (`cursor`) is shared across the whole
            // type-formatting call and is consumed whenever a `Type::ImplTrait` (A-side)
            // or `Type::Generic("impl …")` (C-side synthetic rustdoc param) is encountered.
            //
            // Neither variant can appear nested inside a `dyn Trait`'s generic args in
            // valid Rust or valid rustdoc JSON output:
            //
            //   • `Type::ImplTrait` — `impl Trait` in a type-argument position (e.g.
            //     `dyn SomeTrait<impl Foo>` or `dyn SomeTrait<Assoc = impl Foo>`) is not
            //     accepted by the Rust compiler; the grammar does not allow `impl Trait`
            //     as a type argument or associated-type binding value inside `dyn Trait`.
            //
            //   • `Type::Generic("impl …")` — these are the synthetic generic parameters
            //     that rustdoc emits when it desugars function-argument-position `impl Trait`
            //     (e.g. `fn f(x: impl Display)`).  This desugaring is exclusively
            //     function-level: rustdoc replaces the `impl Trait` arg type with
            //     `Type::Generic("impl Display")` and adds an `is_synthetic = true`
            //     `GenericParamDef` to the function's generic list.  This form never
            //     appears inside the generic args of a `dyn Trait` object type.
            //
            // Consequence: `format_generic_args_with_canon_occ` called for each bound
            // below will never increment `cursor`, so the post-loop `sort_unstable()` does
            // not reorder any cursor-consuming renderings.  The consume-then-sort pattern
            // is therefore harmless for this arm.
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
                            let s = format_generic_args_with_canon_occ(
                                a,
                                canon,
                                synthetic_order,
                                use_positional_fallback,
                                cursor,
                            );
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
            if !fp.generic_params.is_empty() {
                return "<UNSUPPORTED:FunctionPointer>".to_string();
            }
            let params: Vec<String> = fp.sig.inputs.iter().map(|(_, t)| rec(t, cursor)).collect();
            let ret = fp.sig.output.as_ref().map_or_else(|| "()".to_string(), |t| rec(t, cursor));
            let variadic = if fp.sig.is_c_variadic { ", ..." } else { "" };
            let constness = if fp.header.is_const { "const " } else { "" };
            let unsafety = if fp.header.is_unsafe { "unsafe " } else { "" };
            let abi = format_abi(&fp.header.abi);
            format!("{abi}{constness}{unsafety}fn({}{})->{ret}", params.join(","), variadic)
        }
        Type::Pat { type_: inner, .. } => rec(inner, cursor),
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let trait_str = trait_
                .as_ref()
                .map(|p| {
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    let trait_args_str = p
                        .args
                        .as_deref()
                        .map(|a| {
                            let s = format_generic_args_with_canon_occ(
                                a,
                                canon,
                                synthetic_order,
                                use_positional_fallback,
                                cursor,
                            );
                            if s.is_empty() { String::new() } else { format!("<{s}>") }
                        })
                        .unwrap_or_default();
                    format!("{short}{trait_args_str}")
                })
                .unwrap_or_else(|| "_".to_string());
            let self_str = rec(self_type, cursor);
            let args_str = args.as_deref().map_or_else(String::new, |a| {
                format_generic_args_with_canon_occ(
                    a,
                    canon,
                    synthetic_order,
                    use_positional_fallback,
                    cursor,
                )
            });
            if args_str.is_empty() {
                format!("<{self_str} as {trait_str}>::{name}")
            } else {
                format!("<{self_str} as {trait_str}>::{name}<{args_str}>")
            }
        }
        _ => "_".to_string(),
    }
}

/// Formats `GenericArgs` with occurrence-aware canonicalization.
///
/// Mirrors `format_generic_args_with_canon` but threads the occurrence cursor and
/// `use_positional_fallback` flag through recursive `format_type_with_canon_occ` calls
/// so that `impl Trait` argument positions inside generic args are also tracked with
/// the correct fallback behavior.
fn format_generic_args_with_canon_occ(
    args: &GenericArgs,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            let positional: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Type(t) => format_type_with_canon_occ(
                        t,
                        canon,
                        synthetic_order,
                        use_positional_fallback,
                        cursor,
                    ),
                    GenericArg::Lifetime(lt) => {
                        canon.get(lt.as_str()).cloned().unwrap_or_else(|| lt.clone())
                    }
                    GenericArg::Const(c) => apply_canon_to_str(&c.expr.replace("::", "."), canon),
                    GenericArg::Infer => "_".to_string(),
                })
                .collect();
            let mut sorted_constraints: Vec<_> = constraints.iter().collect();
            sorted_constraints
                .sort_by_key(|c| format_assoc_constraint_sort_key_with_canon(c, canon));
            let constraint_parts: Vec<String> = sorted_constraints
                .into_iter()
                .map(|c| {
                    format_assoc_constraint_with_canon_occ(
                        c,
                        canon,
                        synthetic_order,
                        use_positional_fallback,
                        cursor,
                    )
                })
                .collect();
            let mut parts = positional;
            parts.extend(constraint_parts);
            parts.join(", ")
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let params: Vec<String> = inputs
                .iter()
                .map(|t| {
                    format_type_with_canon_occ(
                        t,
                        canon,
                        synthetic_order,
                        use_positional_fallback,
                        cursor,
                    )
                })
                .collect();
            let ret = output.as_ref().map_or_else(
                || "()".to_string(),
                |t| {
                    format_type_with_canon_occ(
                        t,
                        canon,
                        synthetic_order,
                        use_positional_fallback,
                        cursor,
                    )
                },
            );
            format!("({})->{}", params.join(","), ret)
        }
        _ => String::new(),
    }
}

fn format_assoc_constraint_sort_key_with_canon(
    c: &rustdoc_types::AssocItemConstraint,
    canon: &HashMap<String, String>,
) -> String {
    match &c.binding {
        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
            let rhs = format_type_with_canon(ty, canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
        }
        AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
            let rhs = apply_canon_to_str(&cv.expr.replace("::", "."), canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
        }
        AssocItemConstraintKind::Constraint(bounds) => {
            let rhs = format_generic_bounds_with_canon(bounds, canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}:{}", c.name, rhs) }
        }
    }
}

fn format_assoc_constraint_with_canon_occ(
    c: &rustdoc_types::AssocItemConstraint,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    match &c.binding {
        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
            let rhs = format_type_with_canon_occ(
                ty,
                canon,
                synthetic_order,
                use_positional_fallback,
                cursor,
            );
            if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
        }
        AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
            let rhs = apply_canon_to_str(&cv.expr.replace("::", "."), canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
        }
        AssocItemConstraintKind::Constraint(bounds) => {
            let rhs = format_generic_bounds_with_canon(bounds, canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}:{}", c.name, rhs) }
        }
    }
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
/// For `Type::ImplTrait` in the absence of occurrence context, this function renders
/// the bound strings literally (e.g. `"impl Into<String>"`).  When occurrence-aware
/// formatting is needed (A-side vs C-side `impl Trait` symmetry), use
/// [`format_type_with_canon_occ`] instead.
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
        Type::BorrowedRef { lifetime, is_mutable, type_: inner } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            // For HRTB binders (1 or more lifetime params), the canon map contains
            // `@BR:lt_name` entries (added by `format_generic_bounds_with_canon`).
            // Three cases:
            //
            //  1. Binder lifetime: `@BR:{lt}` is in the map.
            //     - 2+-binder: value is `"#L{i}"` (non-empty) → emit positional label.
            //       Distinguishes which binder param each reference uses, preventing
            //       `for<'a,'b> Fn(&'a str, &'b str)` and `for<'a,'b> Fn(&'a str, &'a str)`
            //       from collapsing.
            //     - 1-binder: value is `""` (empty) → drop.  Preserves A-side ≡ C-side
            //       symmetry for Fn-trait desugaring: A-side has no lifetime annotation
            //       (`None`, handled above), C-side has the binder lifetime `'_` or `'a`
            //       (dropped here so both sides produce the same fingerprint).
            //
            //  2. Concrete lifetime (e.g. `'static`) in an HRTB context: `@BR:{lt}` is
            //     NOT in the map, but `@BR:` sentinel IS present.  Emit the literal
            //     lifetime string so that `&'static str` and binder-lifetime references
            //     produce distinct fingerprints, preventing false Blue.
            //
            //  3. No HRTB context (no `@BR:` sentinel): only `'static` is semantically
            //     distinct from "no lifetime annotation".  Named lifetime params like
            //     `'a` and `'b` are alpha-equivalent (`generics_structurally_equal`
            //     ignores lifetime param names), so they are dropped.  `'_` (elided) is
            //     also dropped as equivalent to `None`.  This prevents false mismatches
            //     for signatures that differ only in lifetime parameter names
            //     (e.g. `fn f<'a>(&'a str)` vs `fn f<'b>(&'b str)`).
            let in_hrtb_ctx = canon.contains_key("@BR:");
            let lt_str = match lifetime.as_deref() {
                None => String::new(),
                Some(lt) => {
                    if let Some(pos) = canon.get(&format!("@BR:{lt}")) {
                        // Case 1: binder lifetime → positional label (or drop when empty).
                        // For 2+-binder HRTBs, `pos` is `"#L{i}"` (non-empty) → emit label.
                        // For 1-binder Fn-desugaring HRTBs, `pos` is `""` (empty) → drop
                        // (A-side ≡ C-side symmetry: A has no lifetime, C has `'_`/`'a`).
                        // For 1-binder non-Fn HRTBs, `pos` is `"#L{i}"` (non-empty) → emit.
                        if pos.is_empty() { String::new() } else { format!("{pos} ") }
                    } else if in_hrtb_ctx {
                        // Case 2: concrete (non-binder) lifetime in HRTB context.
                        // Emit verbatim so `&'static str` and `&'a str` (binder ref) are
                        // distinguished — preventing false Blue for `for<'a> Fn(&'static str)`
                        // vs `for<'a> Fn(&'a str)`.
                        format!("{lt} ")
                    } else {
                        // Case 3: no HRTB context (no `@BR:` sentinel).
                        // Only `'static` is semantically distinct from "no lifetime"; emit
                        // it verbatim.  All other named lifetimes (`'a`, `'b`, `'_`, etc.)
                        // are alpha-equivalent and dropped so that `fn f<'a>(&'a str)` and
                        // `fn f<'b>(&'b str)` produce the same fingerprint.
                        if lt == "'static" { format!("{lt} ") } else { String::new() }
                    }
                }
            };
            format!("&{lt_str}{mut_str}{}", format_type_with_canon(inner, canon))
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
                    // When the canon map contains a binder-lifetime entry (added for HRTB
                    // binders with 2+ params), normalize the lifetime to its positional label
                    // so that `for<'a> Foo<'a>` and `for<'b> Foo<'b>` produce the same
                    // fingerprint.  For the 0/1-binder elision case no entry exists, so the
                    // raw lifetime value is emitted (and sorted/deduped at a higher level).
                    GenericArg::Lifetime(lt) => {
                        canon.get(lt.as_str()).cloned().unwrap_or_else(|| lt.clone())
                    }
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
                            // Normalize lifetime names to always carry exactly one leading `'`.
                            // rustdoc stores lifetime names WITH the `'` (e.g. `"'a"`), while
                            // older or hand-crafted `GenericParamDef` values may omit it
                            // (e.g. `"a"`).  Using `strip_prefix('\'').unwrap_or(&p.name)`
                            // then re-prepending `'` ensures both forms produce `"'a"`.
                            let bare = p.name.strip_prefix('\'').unwrap_or(&p.name);
                            Some(format!("'{bare}"))
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
/// - `GenericArg::Lifetime(lt)` where `lt ∈ type_params` OR `lt.trim_start_matches('\'') ∈
///   type_params` → removed (impl-block lifetime params are identity-neutral).  Both forms
///   are checked because `GenericArg::Lifetime` always includes the leading `'` (e.g.
///   `"'a"`), while `GenericParamDef::name` may or may not (rustdoc C-side omits it, the
///   A-codec via `type_ref_parser` includes it).  Concrete lifetimes like `'static` are
///   preserved because neither form appears in `type_params`.
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
                            // Normalize lifetime names: strip any leading `'` then re-prepend,
                            // so both `"a"` and `"'a"` produce `"'a"` (avoids `"''a"`).
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
                    // Strip impl-block lifetime params only.  `GenericArg::Lifetime` values
                    // always carry the leading `'` (e.g. `"'a"`).  `GenericParamDef::name`
                    // may carry `'` (A-codec via `type_ref_parser`) or may omit it (rustdoc
                    // C-side), so `type_params` can contain either `"'a"` or `"a"`.  Check
                    // both forms so that the strip is robust to whichever convention produced
                    // the `type_params` set.  Concrete lifetimes like `'static` are preserved
                    // because they are not in `type_params`.
                    GenericArg::Lifetime(lt) => {
                        let bare = lt.trim_start_matches('\'');
                        if type_params.contains(lt.as_str()) || type_params.contains(bare) {
                            None
                        } else {
                            Some(lt.clone())
                        }
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
/// For lifetime-only HRTB binders (D5 support), emits a `#L{count}:` arity
/// prefix so that `for<'a> Bar` and `Bar` produce distinct strings, preventing
/// false equality between HRTB-qualified and non-HRTB constraint bounds.
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
                // For type-param binders, use format_hrtb_type_params (pre-D5 path).
                // For lifetime-only binders (D5), emit an arity prefix `#L{n}:` so that
                // `for<'a> Bar` and `Bar` (and `for<'a,'b> Bar`) produce distinct strings.
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
                Some(format!("{binder_str}{modifier_str}{short}{args_str}"))
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

/// Counts how many `BorrowedRef::lifetime` values in `types` match any name in
/// `binder_names` (with or without the leading `'`).
///
/// Used to determine whether a single-binder HRTB lifetime appears more than once in
/// the Parenthesized Fn-trait args: if the count is > 1 the binder introduces a
/// shared-lifetime constraint (semantically distinct from elided independent lifetimes)
/// and must be retained rather than dropped.
fn count_binder_lifetime_in_types(types: &[Type], binder_names: &[&str]) -> usize {
    types.iter().map(|t| count_binder_lifetime_in_type(t, binder_names)).sum()
}

fn count_binder_lifetime_in_type(ty: &Type, binder_names: &[&str]) -> usize {
    match ty {
        Type::BorrowedRef { lifetime, type_: inner, .. } => {
            let matched = lifetime.as_deref().is_some_and(|lt| {
                binder_names.iter().any(|name| {
                    // Compare with and without leading `'` to handle both conventions.
                    *name == lt
                        || name.strip_prefix('\'') == Some(lt)
                        || lt.strip_prefix('\'') == Some(*name)
                })
            });
            (matched as usize) + count_binder_lifetime_in_type(inner, binder_names)
        }
        Type::Slice(inner)
        | Type::Array { type_: inner, .. }
        | Type::RawPointer { type_: inner, .. } => {
            count_binder_lifetime_in_type(inner, binder_names)
        }
        Type::Tuple(tys) => {
            tys.iter().map(|t| count_binder_lifetime_in_type(t, binder_names)).sum()
        }
        // Recurse into generic args of resolved paths so that
        // `for<'a> Fn(Vec<&'a str>, Vec<&'a str>)` is counted as 2 binder uses (not 0),
        // preventing the single-binder drop from producing a false Blue.
        Type::ResolvedPath(path) => {
            if let Some(args) = path.args.as_deref() {
                count_binder_lifetime_in_generic_args(args, binder_names)
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Counts binder lifetime occurrences in angle-bracketed `GenericArgs`.
/// Used by `count_binder_lifetime_in_type` to recurse into resolved-path args.
fn count_binder_lifetime_in_generic_args(args: &GenericArgs, binder_names: &[&str]) -> usize {
    match args {
        GenericArgs::AngleBracketed { args, .. } => args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(t) => count_binder_lifetime_in_type(t, binder_names),
                _ => 0,
            })
            .sum(),
        GenericArgs::Parenthesized { inputs, output } => {
            let in_count: usize =
                inputs.iter().map(|t| count_binder_lifetime_in_type(t, binder_names)).sum();
            let out_count =
                output.as_ref().map_or(0, |t| count_binder_lifetime_in_type(t, binder_names));
            in_count + out_count
        }
        // ReturnTypeNotation and other future variants contain no embedded types.
        _ => 0,
    }
}

/// Canon-aware variant of [`format_generic_bounds`]. Applies `canon` to inner
/// `Type::Generic` occurrences inside trait-bound generic args so that bounds
/// like `Into<T>` and `Into<U>` (with `canon["T"] = "#0"`, `canon["U"] = "#0"`)
/// produce the same string.
///
/// D5 scope (ADR 2026-05-18-1223 D5):
/// - `TraitBound { generic_params: empty }` — fully supported, formatted verbatim.
/// - `TraitBound { generic_params: lifetime-only }` (HRTB-on-TraitBound) — supported;
///   binder lifetime names are normalized so that the A-side (no binder) and C-side
///   (elision `'_` or explicit `'a`) produce the same fingerprint for the binder-introduced
///   lifetime references.  Concrete non-binder lifetimes (e.g. `'static`) in BorrowedRef
///   positions are preserved verbatim (even in 1-binder context) to prevent false Blue for
///   `for<'a> Fn(&'static str)` vs `for<'a> Fn(&'a str)`.  For 2+-binder HRTBs, binder
///   lifetimes are mapped to positional labels (`#L0`, `#L1`) to distinguish which binder
///   param each reference annotation uses.  A `#L{n}:` arity prefix distinguishes binder
///   arities ≥ 2 from each other.
/// - `TraitBound { generic_params: contains type params }` (HRTB with type binders) —
///   outside D5 scope; returned as `<UNSUPPORTED:HRTB>` (same as before D5).
/// - `Outlives(lt)` — supported: rendered as the lifetime string (e.g. `"'static"`),
///   enabling `F: 'static + Fn(...)` to compare correctly on both sides.
/// - `Use` — outside D5 scope, returned as `<UNSUPPORTED:Use>`.
pub(super) fn format_generic_bounds_with_canon(
    bounds: &[GenericBound],
    canon: &HashMap<String, String>,
) -> String {
    let mut strs: Vec<String> = bounds
        .iter()
        .map(|b| match b {
            GenericBound::TraitBound { trait_, modifier, generic_params } => {
                // D5 (ADR 2026-05-18-1223): HRTB-on-TraitBound with lifetime-only binder
                // params is now supported. Rustdoc desugars elided-lifetime Fn trait bounds
                // (`Fn(&str)`) into HRTB form (`for<'_> Fn(&'_ str)`), so C-side always
                // has a non-empty `generic_params` while A-side (catalogue) has none.
                // Normalizing the binder lifetimes positionally makes both sides compare
                // equal.
                //
                // HRTB with type-param binders (`for<T: Foo>`) remains unsupported —
                // keep the sentinel for that case.
                let has_type_binders = generic_params.iter().any(|hp| {
                    matches!(
                        hp.kind,
                        GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. }
                    )
                });
                if has_type_binders {
                    return "<UNSUPPORTED:HRTB>".to_owned();
                }
                // Lifetime binders with `outlives` constraints (e.g. `for<'a: 'b>`) are
                // outside D5 scope: the formatter does not represent the outlives constraint
                // in the fingerprint, so accepting such binders would silently discard the
                // constraint and risk false Blue for `for<'a: 'b> Foo<&'a T>` vs
                // `for<'a> Foo<&'a T>`.  Emit the unsupported sentinel so that any bound
                // containing `for<'a: ...>` never falsely compares equal.
                let has_outlives_binder = generic_params.iter().any(|hp| match &hp.kind {
                    GenericParamDefKind::Lifetime { outlives } => !outlives.is_empty(),
                    _ => false,
                });
                if has_outlives_binder {
                    return "<UNSUPPORTED:HRTB>".to_owned();
                }
                // Collect binder lifetime params in declaration order.
                let binder_lifetimes: Vec<&str> = generic_params
                    .iter()
                    .filter(|hp| matches!(hp.kind, GenericParamDefKind::Lifetime { .. }))
                    .map(|hp| hp.name.as_str())
                    .collect();
                let lt_count = binder_lifetimes.len();
                // Detect whether the trait's args use the Parenthesized (Fn-trait) form
                // and extract the inputs and output.  Both are needed for Fn-desugaring
                // analysis: the output may also carry binder lifetimes (e.g. `Fn(&'a str) ->
                // &'a str`), so counting only inputs would miss shared-use via the output.
                let parenthesized_parts: Option<(&[Type], Option<&Type>)> =
                    match trait_.args.as_deref() {
                        Some(GenericArgs::Parenthesized { inputs, output }) => {
                            // `output` is `&Option<Box<Type>>`; convert to `Option<&Type>`.
                            // Matching on `output` deref-coerces `&Box<Type>` → `&Type`.
                            let out: Option<&Type> = match output {
                                Some(b) => Some(b),
                                None => None,
                            };
                            Some((inputs.as_slice(), out))
                        }
                        _ => None,
                    };
                // Keep the inputs-only slice for the existing `parenthesized_inputs` usage.
                let parenthesized_inputs: Option<&[Type]> =
                    parenthesized_parts.map(|(inputs, _)| inputs);
                let is_parenthesized = parenthesized_inputs.is_some();
                // Fn-desugaring normalization (D5):
                //
                // rustdoc desugars `Fn(&str)` (A-side, no binder) into `for<'_> Fn(&'_ str)`
                // (C-side, 1 binder) and `Fn(&str, &str)` into `for<'a,'b> Fn(&'a str, &'b str)`
                // (C-side, 2 independent binders), etc.
                //
                // A binder lifetime is "desugaring-eligible" in a Parenthesized context when it
                // appears exactly once in the inputs (and output) — it is an independent elided
                // lifetime.  When all binder lifetimes are desugaring-eligible:
                //  - `binder_prefix` is `""` (treat as if no binder, same as A-side).
                //  - `@BR:binder_lt → ""` (drop each binder lifetime so `&'_ str` → `&str`).
                //
                // When any binder lifetime appears more than once (shared-lifetime constraint):
                //  - This is NOT a Fn-desugaring; retain positional labels.
                //  - `binder_prefix` follows the arity rule: `""` for ≤1, `"#L{n}:"` for 2+.
                //
                // For AngleBracketed or no args, always retain positional labels (HRTB present).
                //
                // For all `lt_count >= 1` cases, the sentinel `@BR:` key is set so that
                // `format_type_with_canon` emits concrete non-binder lifetimes (e.g. `'static`)
                // verbatim, preventing false Blue for `for<'a> Fn(&'static str)` vs
                // `for<'a> Fn(&'a str)`.
                let fn_desugar = if is_parenthesized && lt_count >= 1 {
                    // Count per-binder usage across BOTH inputs and output so that
                    // `for<'a> Fn(&'a str) -> &'a str` (shared-use via output) is NOT
                    // treated as desugaring-eligible and retains its positional label.
                    let (inputs, output) = parenthesized_parts.unwrap_or((&[], None));
                    binder_lifetimes.iter().all(|lt| {
                        let in_count = count_binder_lifetime_in_types(inputs, &[lt]);
                        let out_count =
                            output.map_or(0, |o| count_binder_lifetime_in_type(o, &[lt]));
                        in_count + out_count <= 1
                    })
                } else {
                    false
                };
                // Binder-arity prefix: empty when fn_desugar (all-single-use Parenthesized),
                // empty for 0/1-binder non-desugar, "#L{n}:" for 2+-binder non-desugar.
                let binder_prefix = if fn_desugar || lt_count <= 1 {
                    String::new()
                } else {
                    format!("#L{lt_count}:")
                };
                // Build a positional canon map for binder lifetimes.
                //
                // Two key-spaces:
                //  "lt_name"      → "#L{i}"  (for GenericArg::Lifetime in AngleBracketed)
                //  "@BR:lt_name"  → label     (for BorrowedRef::lifetime)
                //
                // GenericArg::Lifetime: always positional `#L{i}` regardless of context.
                //   Keeps `for<'a> Foo<'a>` distinct from `Foo` (no args).
                //
                // BorrowedRef `@BR:`:
                //   fn_desugar → `""` (drop): A/C symmetry for Fn-desugaring elision.
                //   non-desugar → `"#L{i}"` (retain): HRTB presence observable in fingerprint.
                //   Sentinel `@BR:` (value `""`) always set for 1+ binders so concrete lifetimes
                //   (e.g. `'static`) are emitted verbatim and not silently dropped.
                let args_canon: std::borrow::Cow<HashMap<String, String>> = if lt_count >= 1 {
                    let mut merged = canon.clone();
                    for (i, lt_name) in binder_lifetimes.iter().enumerate() {
                        let positional_label = format!("#L{i}");
                        // Binder lifetime names may be stored with the leading `'`
                        // (`"'a"` from A-codec via `type_ref_parser`) or without it
                        // (`"a"` from rustdoc C-side).  Insert both forms so lookups succeed.
                        let apostrophe_name: String;
                        let (bare, apostrophized) = if let Some(b) = lt_name.strip_prefix('\'') {
                            apostrophe_name = (*lt_name).to_owned();
                            (b, apostrophe_name.as_str())
                        } else {
                            apostrophe_name = format!("'{lt_name}");
                            (*lt_name, apostrophe_name.as_str())
                        };
                        // GenericArg::Lifetime lookup: always positional.
                        merged.insert(bare.to_owned(), positional_label.clone());
                        merged.insert(apostrophized.to_owned(), positional_label.clone());
                        // BorrowedRef `@BR:` lookup.
                        let br_label = if fn_desugar {
                            String::new() // Fn-desugaring: drop binder lifetime
                        } else {
                            positional_label.clone() // retain: HRTB observable
                        };
                        merged.insert(format!("@BR:{bare}"), br_label.clone());
                        merged.insert(format!("@BR:{apostrophized}"), br_label);
                    }
                    // Sentinel: signals HRTB context to `format_type_with_canon`'s BorrowedRef arm.
                    merged.insert("@BR:".to_owned(), String::new());
                    std::borrow::Cow::Owned(merged)
                } else {
                    std::borrow::Cow::Borrowed(canon)
                };
                let short = trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                let args_str = trait_
                    .args
                    .as_deref()
                    .map(|a| {
                        let s = format_generic_args_with_canon(a, &args_canon);
                        if s.is_empty() { String::new() } else { format!("<{s}>") }
                    })
                    .unwrap_or_default();
                use rustdoc_types::TraitBoundModifier;
                let modifier_str = match modifier {
                    TraitBoundModifier::None => "",
                    TraitBoundModifier::Maybe => "?",
                    TraitBoundModifier::MaybeConst => "~const ",
                };
                format!("{binder_prefix}{modifier_str}{short}{args_str}")
            }
            // Outlives bounds (e.g. `'static`, `'a`) are formatted verbatim so that
            // `F: 'static + Fn(...)` produces matching fingerprints on both A-codec
            // and C-side (rustdoc) paths.
            GenericBound::Outlives(lt) => lt.clone(),
            // Use is outside D5 scope.
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

    #[test]
    fn test_format_type_with_canon_occ_reordered_constraints_use_same_placeholders() {
        fn trait_bound(name: &str) -> rustdoc_types::GenericBound {
            rustdoc_types::GenericBound::TraitBound {
                trait_: Path { path: name.to_owned(), id: rustdoc_types::Id(0), args: None },
                generic_params: vec![],
                modifier: rustdoc_types::TraitBoundModifier::None,
            }
        }

        fn impl_constraint(name: &str, bound_name: &str) -> AssocItemConstraint {
            AssocItemConstraint {
                name: name.to_owned(),
                args: None,
                binding: AssocItemConstraintKind::Equality(Term::Type(Type::ImplTrait(vec![
                    trait_bound(bound_name),
                ]))),
            }
        }

        fn resolved_with_constraints(constraints: Vec<AssocItemConstraint>) -> Type {
            Type::ResolvedPath(Path {
                path: "Foo".to_owned(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed { args: vec![], constraints })),
            })
        }

        let ordered = resolved_with_constraints(vec![
            impl_constraint("A", "Display"),
            impl_constraint("B", "Debug"),
        ]);
        let reordered = resolved_with_constraints(vec![
            impl_constraint("B", "Debug"),
            impl_constraint("A", "Display"),
        ]);
        let canon = std::collections::HashMap::new();
        let mut ordered_cursor = 0;
        let mut reordered_cursor = 0;

        let ordered_rendered =
            super::format_type_with_canon_occ(&ordered, &canon, &[], true, &mut ordered_cursor);
        let reordered_rendered =
            super::format_type_with_canon_occ(&reordered, &canon, &[], true, &mut reordered_cursor);

        assert_eq!(ordered_cursor, 2, "ordered constraints must consume both impl Trait entries");
        assert_eq!(
            reordered_cursor, 2,
            "reordered constraints must consume both impl Trait entries"
        );
        assert_eq!(
            ordered_rendered, "Foo<A=#0:impl Display, B=#1:impl Debug>",
            "constraints must be rendered in canonical order before placeholders are assigned"
        );
        assert_eq!(
            ordered_rendered, reordered_rendered,
            "constraint source order must not change occurrence placeholder assignment"
        );
    }

    // --- format_generic_bounds_with_canon: HRTB 2-binder arity distinguishes lifetime usage ---

    #[test]
    fn test_hrtb_two_binder_lifetimes_distinct_usage_produces_different_fingerprints() {
        use std::collections::HashMap;

        use rustdoc_types::{
            GenericBound, GenericParamDef, GenericParamDefKind, Id, Path, TraitBoundModifier,
        };

        use super::format_generic_bounds_with_canon;

        fn lt_param(name: &str) -> GenericParamDef {
            GenericParamDef {
                name: name.to_owned(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }
        }

        // for<'a,'b> Fn(&'a str, &'b str)
        let bound_ab = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                        Type::BorrowedRef {
                            lifetime: Some("'b".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                    ],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        // for<'a,'b> Fn(&'a str, &'a str) — same arity but both params use 'a
        let bound_aa = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                    ],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        let canon: HashMap<String, String> = HashMap::new();
        let fp_ab = format_generic_bounds_with_canon(&[bound_ab], &canon);
        let fp_aa = format_generic_bounds_with_canon(&[bound_aa], &canon);
        assert_ne!(
            fp_ab, fp_aa,
            "D5: `for<'a,'b> Fn(&'a str, &'b str)` and \
             `for<'a,'b> Fn(&'a str, &'a str)` must produce distinct fingerprints \
             to avoid false Blue comparisons; got: fp_ab={fp_ab:?} fp_aa={fp_aa:?}"
        );
    }

    #[test]
    fn test_hrtb_one_binder_different_name_same_fingerprint() {
        // `for<'a> Foo<'a>` and `for<'b> Foo<'b>` should produce the same fingerprint
        // because the binder lifetime name is insignificant.
        use std::collections::HashMap;

        use rustdoc_types::{
            GenericArg, GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind, Id, Path,
            TraitBoundModifier,
        };

        use super::format_generic_bounds_with_canon;

        let make_bound = |binder_name: &str, arg_lt: &str| GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_owned(),
                id: Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![GenericArg::Lifetime(arg_lt.to_owned())],
                    constraints: vec![],
                })),
            },
            generic_params: vec![GenericParamDef {
                name: binder_name.to_owned(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };

        let canon: HashMap<String, String> = HashMap::new();
        let fp_a = format_generic_bounds_with_canon(&[make_bound("'a", "'a")], &canon);
        let fp_b = format_generic_bounds_with_canon(&[make_bound("'b", "'b")], &canon);
        assert_eq!(
            fp_a, fp_b,
            "D5: `for<'a> Foo<'a>` and `for<'b> Foo<'b>` must produce the same fingerprint \
             (binder lifetime name is insignificant); got: fp_a={fp_a:?} fp_b={fp_b:?}"
        );
    }

    #[test]
    fn test_hrtb_two_binder_concrete_lifetime_not_equal_to_elided() {
        // `for<'a,'b> Fn(&'static str)` and `for<'a,'b> Fn(&str)` must produce distinct
        // fingerprints because `'static` is a concrete (non-binder) lifetime.
        use std::collections::HashMap;

        use rustdoc_types::{
            GenericBound, GenericParamDef, GenericParamDefKind, Id, Path, TraitBoundModifier,
        };

        use super::format_generic_bounds_with_canon;

        fn lt_param(name: &str) -> GenericParamDef {
            GenericParamDef {
                name: name.to_owned(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }
        }

        // for<'a,'b> Fn(&'static str) — concrete 'static lifetime in arg
        let bound_static = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some("'static".to_owned()),
                        is_mutable: false,
                        type_: Box::new(Type::Primitive("str".to_owned())),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        // for<'a,'b> Fn(&str) — elided (no) lifetime in arg
        let bound_elided = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: None,
                        is_mutable: false,
                        type_: Box::new(Type::Primitive("str".to_owned())),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        let canon: HashMap<String, String> = HashMap::new();
        let fp_static = format_generic_bounds_with_canon(&[bound_static], &canon);
        let fp_elided = format_generic_bounds_with_canon(&[bound_elided], &canon);
        assert_ne!(
            fp_static, fp_elided,
            "D5: `for<'a,'b> Fn(&'static str)` and `for<'a,'b> Fn(&str)` must produce \
             distinct fingerprints (concrete vs elided lifetime in 2+-binder context); \
             got: fp_static={fp_static:?} fp_elided={fp_elided:?}"
        );
    }

    // --- DynTrait bound-order canonicalization stability ---
    //
    // Verifies the "Safety" invariant documented in `format_type_with_canon_occ_inner`'s
    // `Type::DynTrait` arm: because the occurrence cursor is never consumed inside
    // `dyn Trait`'s generic args, sorting the rendered bounds after formatting them does
    // not change which cursor slot any `impl Trait` placeholder is assigned to.  Two
    // `dyn A + B` and `dyn B + A` objects (identical modulo bound order) must produce the
    // same canonicalized string and must leave the cursor unchanged.

    fn make_dyn_trait_type(trait_names: &[&str]) -> Type {
        use rustdoc_types::{DynTrait, Id, Path, PolyTrait};
        Type::DynTrait(DynTrait {
            traits: trait_names
                .iter()
                .map(|name| PolyTrait {
                    trait_: Path { path: name.to_string(), id: Id(0), args: None },
                    generic_params: vec![],
                })
                .collect(),
            lifetime: None,
        })
    }

    #[test]
    fn test_dyn_trait_bound_order_canonicalization_is_stable() {
        // `dyn Display + Debug` and `dyn Debug + Display` must produce the same
        // canonicalized string regardless of the source order of the trait bounds.
        use std::collections::HashMap;

        use super::format_type_with_canon_occ;

        let dyn_ab = make_dyn_trait_type(&["Display", "Debug"]);
        let dyn_ba = make_dyn_trait_type(&["Debug", "Display"]);
        let canon: HashMap<String, String> = HashMap::new();
        let mut cursor_ab = 0usize;
        let mut cursor_ba = 0usize;

        let result_ab = format_type_with_canon_occ(&dyn_ab, &canon, &[], false, &mut cursor_ab);
        let result_ba = format_type_with_canon_occ(&dyn_ba, &canon, &[], false, &mut cursor_ba);

        assert_eq!(
            result_ab, result_ba,
            "dyn Trait bound order must not affect canonicalized string; \
             ab={result_ab:?} ba={result_ba:?}"
        );
        assert_eq!(
            cursor_ab, 0,
            "dyn Trait formatting must not consume the impl Trait occurrence cursor; \
             cursor advanced to {cursor_ab}"
        );
        assert_eq!(
            cursor_ba, 0,
            "dyn Trait formatting must not consume the impl Trait occurrence cursor; \
             cursor advanced to {cursor_ba}"
        );
    }

    #[test]
    fn test_dyn_trait_with_generic_args_does_not_consume_cursor() {
        // `dyn Foo<u8> + Bar<u16>` and `dyn Bar<u16> + Foo<u8>` (same bounds, different source
        // order) must produce the same output and must not consume the cursor.
        use std::collections::HashMap;

        use rustdoc_types::{DynTrait, GenericArg, GenericArgs, Id, Path, PolyTrait};

        use super::format_type_with_canon_occ;

        fn make_poly_trait(name: &str, arg_ty: &str) -> PolyTrait {
            PolyTrait {
                trait_: Path {
                    path: name.to_string(),
                    id: Id(0),
                    args: Some(Box::new(GenericArgs::AngleBracketed {
                        args: vec![GenericArg::Type(Type::Primitive(arg_ty.to_string()))],
                        constraints: vec![],
                    })),
                },
                generic_params: vec![],
            }
        }

        // dyn Foo<u8> + Bar<u16>  (source order: Foo first)
        let dyn_ab = Type::DynTrait(DynTrait {
            traits: vec![make_poly_trait("Foo", "u8"), make_poly_trait("Bar", "u16")],
            lifetime: None,
        });
        // dyn Bar<u16> + Foo<u8>  (source order: Bar first — same set, different order)
        let dyn_ba = Type::DynTrait(DynTrait {
            traits: vec![make_poly_trait("Bar", "u16"), make_poly_trait("Foo", "u8")],
            lifetime: None,
        });

        let canon: HashMap<String, String> = HashMap::new();
        let mut cursor_ab = 0usize;
        let mut cursor_ba = 0usize;

        let result_ab = format_type_with_canon_occ(&dyn_ab, &canon, &[], false, &mut cursor_ab);
        let result_ba = format_type_with_canon_occ(&dyn_ba, &canon, &[], false, &mut cursor_ba);

        assert_eq!(
            result_ab, result_ba,
            "dyn Trait with primitive generic args: bound order must not affect output; \
             ab={result_ab:?} ba={result_ba:?}"
        );
        assert_eq!(
            cursor_ab, 0,
            "dyn Trait with primitive args must not consume cursor; advanced to {cursor_ab}"
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
