//! Generic-parameter canonicalization string helpers.
//!
//! Provides `apply_canon_to_str` and the occurrence-key utilities
//! (`format_impl_trait_occurrence_key`, `occurrence_placeholder`).
//!
//! `build_generic_canon_map` lives in `ty_canon` because it depends on
//! `format_type_with_canon`.

use std::collections::HashMap;

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
pub(crate) fn apply_canon_to_str(s: &str, canon: &HashMap<String, String>) -> String {
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

pub(crate) fn format_impl_trait_occurrence_key(placeholder: &str, impl_sig: &str) -> String {
    format!("{placeholder}:{impl_sig}")
}

pub(crate) fn occurrence_placeholder(occurrence_key: &str) -> &str {
    occurrence_key.split_once(':').map_or(occurrence_key, |(placeholder, _)| placeholder)
}
