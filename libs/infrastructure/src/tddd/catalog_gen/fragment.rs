//! Schema-field decomposition for the catalogue adapter (D6 / IN-08).
//!
//! Each declaration / signature fragment is decomposed into the destination
//! catalogue schema fields. Validation goes through the destination field
//! constructors/parsers (`TypeRef::new`, `FieldName::new`, `ParamName::new`,
//! `SelfReceiver::from_str`, etc.) instead of adding a Rust syntax parser gate.

use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::{
    FieldName, MethodName, ParamName, SelfReceiver, TypeRef, VariantName,
};
use serde_json::{Value, json};
use std::str::FromStr;
use usecase::catalog_gen::CatalogError;

use super::fs_access::schema_error;

/// Build a [`CatalogError::ParseFragment`].
pub(super) fn parse_error(message: impl Into<String>) -> CatalogError {
    CatalogError::ParseFragment { message: FreeText::new(message) }
}

/// Parse a struct-field fragment (`name: Type`) into a `FieldDecl` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment cannot be split as
/// `name: Type`, or [`CatalogError::SchemaInvalid`] when either destination
/// schema field rejects its value.
pub(super) fn parse_field(fragment: &str) -> Result<Value, CatalogError> {
    let (name, ty) = split_binding(fragment)
        .ok_or_else(|| parse_error(format!("field `{fragment}`: expected `name: Type`")))?;
    let name = field_name_value("field name", &name)?;
    let ty = type_ref_value("field type", &ty)?;
    Ok(json!({ "name": name, "ty": ty }))
}

/// Parse a declaration-level generic parameter (`T: Bound + Bound2`) into a
/// `MethodGenericParam` value.
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] when the destination
/// `MethodGenericParam` schema fields reject their values.
pub(super) fn parse_generic(fragment: &str) -> Result<Value, CatalogError> {
    let (name, bounds) = match split_binding(fragment) {
        Some((name, rest)) => (name, type_ref_list("generic bounds", &rest, '+')?),
        None => (fragment.trim().to_owned(), Vec::new()),
    };
    let name = param_name_value("generic name", &name)?;
    Ok(json!({ "name": name, "bounds": bounds }))
}

/// Parse a where-predicate fragment (`T: Bounds` or `T::Item = X`) into a
/// `WherePredicateDecl` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment cannot be split
/// into the schema's `lhs` / `rhs` / `operator` fields, or
/// [`CatalogError::SchemaInvalid`] when a destination field rejects its value.
pub(super) fn parse_where(fragment: &str) -> Result<Value, CatalogError> {
    if let Some((lhs, rhs_part)) = split_once_top_level(fragment, '=') {
        let lhs = type_ref_value("where lhs", &lhs)?;
        let rhs = type_ref_list("where rhs", &rhs_part, '+')?;
        return Ok(json!({ "lhs": lhs, "rhs": rhs, "operator": "Equal" }));
    }
    let (lhs, rhs_part) = split_binding(fragment).ok_or_else(|| {
        parse_error(format!("where predicate `{fragment}`: expected `T: Bounds`"))
    })?;
    let lhs = type_ref_value("where lhs", &lhs)?;
    let rhs = type_ref_list("where rhs", &rhs_part, '+')?;
    Ok(json!({ "lhs": lhs, "rhs": rhs, "operator": "Bound" }))
}

/// Parse an enum-variant fragment (`Unit`, `Tuple(A, B)`, `Struct { a: T }`)
/// into a `VariantDecl` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment cannot be split
/// into the schema's variant fields, or [`CatalogError::SchemaInvalid`] when a
/// destination field rejects its value.
pub(super) fn parse_variant(fragment: &str) -> Result<Value, CatalogError> {
    let trimmed = fragment.trim();
    if split_once_top_level(trimmed, '=').is_some() {
        return Err(schema_error(format!(
            "variant `{fragment}`: explicit discriminants (`= <expr>`) are unsupported by the \
             catalogue"
        )));
    }

    let tuple_open = find_top_level_char(trimmed, '(');
    let struct_open = find_top_level_char(trimmed, '{');
    match (tuple_open, struct_open) {
        (Some(tuple_idx), None) => parse_tuple_variant(fragment, trimmed, tuple_idx),
        (Some(tuple_idx), Some(struct_idx)) if tuple_idx < struct_idx => {
            parse_tuple_variant(fragment, trimmed, tuple_idx)
        }
        (_, Some(struct_idx)) => parse_struct_variant(fragment, trimmed, struct_idx),
        (None, None) => {
            let name = variant_name_value("variant name", trimmed)?;
            Ok(json!({ "name": name, "payload": { "kind": "unit" } }))
        }
    }
}

fn parse_tuple_variant(
    fragment: &str,
    trimmed: &str,
    tuple_idx: usize,
) -> Result<Value, CatalogError> {
    let name = variant_name_value("variant name", trimmed.get(..tuple_idx).unwrap_or(""))?;
    let inner = extract_delimited(trimmed, '(', ')')
        .ok_or_else(|| parse_error(format!("variant `{fragment}`: unbalanced parentheses")))?;
    let tail = tail_after_delimited(trimmed, '(', ')').unwrap_or_default();
    if !tail.trim().is_empty() {
        return Err(parse_error(format!(
            "variant `{fragment}`: unexpected text after tuple payload"
        )));
    }
    let fields = type_ref_list("tuple variant field", &inner, ',')?;
    Ok(json!({ "name": name, "payload": { "kind": "tuple", "fields": fields } }))
}

fn parse_struct_variant(
    fragment: &str,
    trimmed: &str,
    struct_idx: usize,
) -> Result<Value, CatalogError> {
    let name = variant_name_value("variant name", trimmed.get(..struct_idx).unwrap_or(""))?;
    let inner = extract_delimited(trimmed, '{', '}')
        .ok_or_else(|| parse_error(format!("variant `{fragment}`: unbalanced braces")))?;
    let tail = tail_after_delimited(trimmed, '{', '}').unwrap_or_default();
    if !tail.trim().is_empty() {
        return Err(parse_error(format!(
            "variant `{fragment}`: unexpected text after struct payload"
        )));
    }
    let mut fields = Vec::new();
    for field_frag in split_top_level(&inner, ',') {
        fields.push(parse_field(&field_frag)?);
    }
    Ok(json!({ "name": name, "payload": { "kind": "struct", "fields": fields } }))
}

/// Parse a method / function signature (`fn name<G>(recv, p: T) -> R where W`)
/// into a `MethodDeclaration` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment cannot be split as
/// a method/function signature, or [`CatalogError::SchemaInvalid`] when a
/// destination field rejects its value.
pub(super) fn parse_method(fragment: &str) -> Result<Value, CatalogError> {
    let trimmed = fragment.trim();
    let (is_async, rest) = match strip_keyword(trimmed, "async") {
        Some(rest) => (true, rest),
        None => (false, trimmed),
    };
    let rest = strip_keyword(rest, "fn")
        .ok_or_else(|| parse_error(format!("method `{fragment}`: expected `fn name(...)`")))?;

    let param_open = find_param_open(rest)
        .ok_or_else(|| parse_error(format!("method `{fragment}`: missing parameter list")))?;
    let head = rest.get(..param_open).unwrap_or("").trim();
    let (name, generics) = parse_method_head(head, fragment)?;

    let params_region = rest.get(param_open..).unwrap_or_default();
    let params_inner = extract_delimited(params_region, '(', ')')
        .ok_or_else(|| parse_error(format!("method `{fragment}`: missing parameter list")))?;
    let mut receiver: Option<String> = None;
    let mut params: Vec<Value> = Vec::new();
    for (index, part) in split_top_level(&params_inner, ',').into_iter().enumerate() {
        if index == 0 {
            if let Some(recv) = self_receiver(&part) {
                receiver = Some(recv);
                continue;
            }
            if looks_like_unsupported_receiver(&part) {
                return Err(parse_error(format!(
                    "method `{fragment}`: receiver `{part}` is not representable by SelfReceiver"
                )));
            }
        }
        params.push(parse_param(&part)?);
    }

    let after_params = tail_after_delimited(params_region, '(', ')').unwrap_or_default();
    let (returns, where_body) = split_return_and_where(&after_params);
    let returns = type_ref_value("method return type", &returns)?;
    let mut where_predicates = Vec::new();
    if let Some(body) = where_body {
        for predicate in split_top_level(&body, ',') {
            where_predicates.push(parse_where(&predicate)?);
        }
    }

    let mut method = serde_json::Map::new();
    method.insert("name".to_owned(), json!(name));
    if let Some(recv) = receiver {
        method.insert("receiver".to_owned(), json!(recv));
    }
    method.insert("params".to_owned(), Value::Array(params));
    method.insert("returns".to_owned(), json!(returns));
    method.insert("is_async".to_owned(), json!(is_async));
    method.insert("has_default_impl".to_owned(), json!(false));
    if !generics.is_empty() {
        method.insert("generics".to_owned(), Value::Array(generics));
    }
    if !where_predicates.is_empty() {
        method.insert("where_predicates".to_owned(), Value::Array(where_predicates));
    }
    Ok(Value::Object(method))
}

/// Parse a trait-impl fragment (the trait reference, e.g. `From<X>`) targeting
/// `for_type` into a `TraitImplDecl` value.
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] when the destination `TypeRef` field
/// rejects the trait reference.
pub(super) fn parse_trait_impl(fragment: &str, for_type: &str) -> Result<Value, CatalogError> {
    let trait_ref = type_ref_value("trait impl trait_ref", fragment)?;
    let for_type = type_ref_value("trait impl for_type", for_type)?;
    Ok(json!({ "trait_ref": trait_ref, "for_type": for_type }))
}

// ---------------------------------------------------------------------------
// Bracket-aware substring recovery helpers
// ---------------------------------------------------------------------------

fn type_ref_value(label: &str, value: &str) -> Result<String, CatalogError> {
    let trimmed = value.trim();
    TypeRef::new(trimmed.to_owned())
        .map(|ty| ty.as_str().to_owned())
        .map_err(|err| schema_error(format!("{label} `{trimmed}` is not a valid TypeRef: {err}")))
}

fn type_ref_list(label: &str, input: &str, delimiter: char) -> Result<Vec<String>, CatalogError> {
    let parts = split_top_level_preserving_empty(input, delimiter);
    if parts.is_empty() {
        return Err(schema_error(format!("{label} must contain at least one TypeRef")));
    }
    if parts.iter().any(|part| part.is_empty()) {
        return Err(schema_error(format!("{label} contains an empty TypeRef")));
    }
    parts.into_iter().map(|part| type_ref_value(label, &part)).collect()
}

fn field_name_value(label: &str, value: &str) -> Result<String, CatalogError> {
    let trimmed = value.trim();
    FieldName::new(trimmed.to_owned())
        .map(|name| name.as_str().to_owned())
        .map_err(|err| schema_error(format!("{label} `{trimmed}` is not a valid FieldName: {err}")))
}

fn method_name_value(label: &str, value: &str) -> Result<String, CatalogError> {
    let trimmed = value.trim();
    MethodName::new(trimmed.to_owned()).map(|name| name.as_str().to_owned()).map_err(|err| {
        schema_error(format!("{label} `{trimmed}` is not a valid MethodName: {err}"))
    })
}

fn param_name_value(label: &str, value: &str) -> Result<String, CatalogError> {
    let trimmed = value.trim();
    ParamName::new(trimmed.to_owned())
        .map(|name| name.as_str().to_owned())
        .map_err(|err| schema_error(format!("{label} `{trimmed}` is not a valid ParamName: {err}")))
}

fn variant_name_value(label: &str, value: &str) -> Result<String, CatalogError> {
    let trimmed = value.trim();
    VariantName::new(trimmed.to_owned()).map(|name| name.as_str().to_owned()).map_err(|err| {
        schema_error(format!("{label} `{trimmed}` is not a valid VariantName: {err}"))
    })
}

fn parse_param(fragment: &str) -> Result<Value, CatalogError> {
    let (name, ty) = split_binding(fragment)
        .ok_or_else(|| parse_error(format!("parameter `{fragment}`: expected `name: Type`")))?;
    let name = param_name_value("parameter name", &name)?;
    let ty = type_ref_value("parameter type", &ty)?;
    Ok(json!({ "name": name, "ty": ty }))
}

fn parse_method_head(head: &str, fragment: &str) -> Result<(String, Vec<Value>), CatalogError> {
    let Some(generic_start) = find_top_level_char(head, '<') else {
        return Ok((method_name_value("method name", head)?, Vec::new()));
    };
    let name = method_name_value("method name", head.get(..generic_start).unwrap_or(""))?;
    let inner = extract_delimited(head, '<', '>')
        .ok_or_else(|| parse_error(format!("method `{fragment}`: unbalanced generics")))?;
    let tail = tail_after_delimited(head, '<', '>').unwrap_or_default();
    if !tail.trim().is_empty() {
        return Err(parse_error(format!("method `{fragment}`: unexpected text after generics")));
    }
    let mut generics = Vec::new();
    for part in split_top_level(&inner, ',') {
        generics.push(parse_generic(&part)?);
    }
    if generics.is_empty() {
        return Err(parse_error(format!("method `{fragment}`: empty generic list")));
    }
    Ok((name, generics))
}

fn strip_keyword<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(keyword)?;
    if rest.chars().next().is_some_and(is_ident_char) {
        return None;
    }
    Some(rest.trim_start())
}

/// Split `input` on `delimiter` at bracket depth 0, trimming each piece and
/// preserving empty pieces. `->` is treated as a literal token so return arrows
/// do not skew depth.
fn split_top_level_preserving_empty(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '-' if chars.peek() == Some(&'>') => {
                chars.next();
                current.push_str("->");
            }
            '<' | '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            '>' | ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            c if c == delimiter && depth <= 0 => {
                parts.push(current.trim().to_owned());
                current.clear();
            }
            other => current.push(other),
        }
    }
    parts.push(current.trim().to_owned());
    parts
}

/// Split `input` on `delimiter` at bracket depth 0, trimming and dropping empty
/// pieces. `->` is treated as a literal token so return arrows do not skew depth.
fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
    split_top_level_preserving_empty(input, delimiter)
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect()
}

/// Split `fragment` at the first single top-level `:` into `(name, rest)`.
fn split_binding(fragment: &str) -> Option<(String, String)> {
    let mut depth: i32 = 0;
    let mut chars = fragment.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => {
                if chars.peek().map(|&(_, next)| next) == Some(':') {
                    chars.next();
                } else {
                    let name = fragment.get(..idx)?.trim().to_owned();
                    let rest = fragment.get(idx + 1..)?.trim().to_owned();
                    if name.is_empty() || rest.is_empty() {
                        return None;
                    }
                    return Some((name, rest));
                }
            }
            _ => {}
        }
    }
    None
}

/// Split `fragment` at the first standalone top-level `delimiter`.
fn split_once_top_level(fragment: &str, delimiter: char) -> Option<(String, String)> {
    let mut depth: i32 = 0;
    let mut prev: Option<char> = None;
    let mut chars = fragment.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            c if c == delimiter && depth == 0 => {
                let next = chars.peek().map(|&(_, n)| n);
                let compound = next == Some('=') || matches!(prev, Some('<' | '>' | '!' | '='));
                if !compound {
                    let before = fragment.get(..idx)?.trim().to_owned();
                    let after = fragment.get(idx + 1..)?.trim().to_owned();
                    return Some((before, after));
                }
            }
            _ => {}
        }
        prev = Some(ch);
    }
    None
}

/// The substring strictly inside the first balanced `open`..`close` pair.
fn extract_delimited(input: &str, open: char, close: char) -> Option<String> {
    let start = input.find(open)?;
    let mut depth: i32 = 0;
    let mut result = String::new();
    let mut chars = input.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if idx < start {
            continue;
        }
        if ch == '-' && matches!(chars.peek(), Some(&(_, '>'))) {
            // Treat `->` as a literal token so a return arrow inside the region
            // (e.g. a `Fn(u32) -> bool` bound) never perturbs `<`/`>` depth.
            chars.next();
            if depth >= 1 {
                result.push_str("->");
            }
            continue;
        }
        if ch == open {
            depth += 1;
            if depth > 1 {
                result.push(ch);
            }
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(result);
            }
            result.push(ch);
        } else if depth >= 1 {
            result.push(ch);
        }
    }
    None
}

/// The substring after the first balanced `open`..`close` pair.
fn tail_after_delimited(input: &str, open: char, close: char) -> Option<String> {
    let start = input.find(open)?;
    let mut depth: i32 = 0;
    let mut chars = input.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if idx < start {
            continue;
        }
        if ch == '-' && matches!(chars.peek(), Some(&(_, '>'))) {
            chars.next();
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                let after = idx + ch.len_utf8();
                return Some(input.get(after..).unwrap_or("").to_owned());
            }
        }
    }
    None
}

/// Locate the byte offset of the parameter-list `(` in a `fn` signature — the
/// first `(` that appears at angle-bracket depth ≤ 0. Scanning past the generic
/// block this way keeps a parenthesised bound (`<F: Fn(u32) -> bool>`) from being
/// mistaken for the parameter list. `->` is consumed as a literal token so a
/// return arrow inside a bound does not perturb `<`/`>` depth tracking.
fn find_param_open(fragment: &str) -> Option<usize> {
    let mut angle_depth: i32 = 0;
    let mut chars = fragment.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '-' if matches!(chars.peek(), Some(&(_, '>'))) => {
                chars.next();
            }
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            '(' if angle_depth <= 0 => return Some(idx),
            _ => {}
        }
    }
    None
}

/// Locate a single character at bracket depth ≤ 0.
fn find_top_level_char(haystack: &str, needle: char) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut chars = haystack.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '-' if matches!(chars.peek(), Some(&(_, '>'))) => {
                chars.next();
            }
            '<' | '(' | '[' | '{' => {
                if depth <= 0 && ch == needle {
                    return Some(idx);
                }
                depth += 1;
            }
            '>' | ')' | ']' | '}' => depth -= 1,
            c if depth <= 0 && c == needle => return Some(idx),
            _ => {}
        }
    }
    None
}

/// Locate `needle` at bracket depth ≤ 0 (tolerating the `->` arrow's `>`).
fn find_top_level(haystack: &str, needle: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (idx, ch) in haystack.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth <= 0 {
            if let Some(rest) = haystack.get(idx..) {
                if rest.starts_with(needle) {
                    return Some(idx);
                }
            }
        }
    }
    None
}

/// Locate a standalone `needle` keyword at bracket depth ≤ 0. Unlike a raw
/// substring search the match must be a whole token — the flanking characters,
/// if any, must not be identifier characters — so a return type such as
/// `somewhere::Thing` does not trip the `where`-clause detector.
fn find_top_level_keyword(haystack: &str, needle: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (idx, ch) in haystack.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth <= 0 {
            if let Some(rest) = haystack.get(idx..) {
                if rest.starts_with(needle) && is_token_boundary(haystack, idx, needle.len()) {
                    return Some(idx);
                }
            }
        }
    }
    None
}

/// Whether the `[start, start + len)` slice of `text` sits on identifier word
/// boundaries (the characters flanking it, if present, are not identifier
/// characters).
fn is_token_boundary(text: &str, start: usize, len: usize) -> bool {
    let before_ok = text
        .get(..start)
        .and_then(|before| before.chars().next_back())
        .is_none_or(|ch| !is_ident_char(ch));
    let after_ok = text
        .get(start + len..)
        .and_then(|after| after.chars().next())
        .is_none_or(|ch| !is_ident_char(ch));
    before_ok && after_ok
}

/// Whether `ch` can appear inside a Rust identifier (and so must not be treated
/// as a token boundary).
fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Split a post-parameter tail into a return-type string and optional
/// where-clause body. The `where` match must be a whole token, so a return type
/// such as `somewhere::T` is never mistaken for a where clause.
fn split_return_and_where(after: &str) -> (String, Option<String>) {
    let where_start = find_top_level_keyword(after, "where");
    let (head, where_body) = match where_start {
        Some(idx) => {
            let before = after.get(..idx).unwrap_or("").to_owned();
            let body = after.get(idx + "where".len()..).unwrap_or("").trim().to_owned();
            (before, Some(body))
        }
        None => (after.to_owned(), None),
    };
    let returns = match find_top_level(&head, "->") {
        Some(idx) => head.get(idx + 2..).unwrap_or("").trim().to_owned(),
        None => "()".to_owned(),
    };
    (returns, where_body)
}

/// Recognise a `self` / `&self` / `&mut self` receiver.
fn self_receiver(part: &str) -> Option<String> {
    let compact: String = part.chars().filter(|ch| !ch.is_whitespace()).collect();
    let receiver = match compact.as_str() {
        "self" => "self",
        "&self" => "&self",
        "&mutself" => "&mut self",
        _ => return None,
    };
    SelfReceiver::from_str(receiver).ok().map(|receiver| receiver.to_string())
}

fn looks_like_unsupported_receiver(part: &str) -> bool {
    let compact: String = part.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.starts_with("self:") || compact.starts_with("mutself:")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_field() {
        let value = parse_field("count: u32").unwrap();
        assert_eq!(value["name"], json!("count"));
        assert_eq!(value["ty"], json!("u32"));
    }

    #[test]
    fn test_parse_field_generic_type_preserved() {
        let value = parse_field("items: Vec<String>").unwrap();
        assert_eq!(value["ty"], json!("Vec<String>"));
    }

    #[test]
    fn test_parse_field_uses_typeref_newtype_not_rust_syntax_gate() {
        let value = parse_field("items: Vec<").unwrap();
        assert_eq!(value["name"], json!("items"));
        assert_eq!(value["ty"], json!("Vec<"));
    }

    #[test]
    fn test_parse_field_rejects_garbage() {
        assert!(matches!(
            parse_field("this is not a field"),
            Err(CatalogError::ParseFragment { .. })
        ));
    }

    #[test]
    fn test_parse_generic_with_bounds() {
        let value = parse_generic("T: Clone + Send").unwrap();
        assert_eq!(value["name"], json!("T"));
        assert_eq!(value["bounds"], json!(["Clone", "Send"]));
    }

    #[test]
    fn test_parse_generic_rejects_empty_bound_fragment() {
        assert!(matches!(parse_generic("T: Clone +"), Err(CatalogError::SchemaInvalid { .. })));
        assert!(matches!(
            parse_generic("T: Clone ++ Send"),
            Err(CatalogError::SchemaInvalid { .. })
        ));
    }

    #[test]
    fn test_parse_generic_without_bounds() {
        let value = parse_generic("T").unwrap();
        assert_eq!(value["name"], json!("T"));
        assert_eq!(value["bounds"], json!([]));
    }

    #[test]
    fn test_parse_where_bound() {
        let value = parse_where("T: Clone").unwrap();
        assert_eq!(value["lhs"], json!("T"));
        assert_eq!(value["rhs"], json!(["Clone"]));
        assert_eq!(value["operator"], json!("Bound"));
    }

    #[test]
    fn test_parse_where_rejects_empty_rhs_fragment() {
        assert!(matches!(parse_where("T: Clone +"), Err(CatalogError::SchemaInvalid { .. })));
        assert!(matches!(
            parse_where("T::Item = "),
            Err(CatalogError::ParseFragment { .. } | CatalogError::SchemaInvalid { .. })
        ));
    }

    #[test]
    fn test_parse_variant_unit_and_tuple() {
        let unit = parse_variant("Idle").unwrap();
        assert_eq!(unit["name"], json!("Idle"));
        assert_eq!(unit["payload"]["kind"], json!("unit"));

        let tuple = parse_variant("Pair(i32, u8)").unwrap();
        assert_eq!(tuple["name"], json!("Pair"));
        assert_eq!(tuple["payload"]["kind"], json!("tuple"));
        assert_eq!(tuple["payload"]["fields"], json!(["i32", "u8"]));
    }

    // A struct variant whose field type contains parentheses (`(u8, u8)`) must be
    // recognised as a struct variant, not misread as a tuple variant that stores
    // `Point { coords:` as the name.
    #[test]
    fn test_parse_variant_named_with_paren_in_field_type() {
        let value = parse_variant("Point { coords: (u8, u8) }").unwrap();
        assert_eq!(value["name"], json!("Point"));
        assert_eq!(value["payload"]["kind"], json!("struct"));
        assert_eq!(value["payload"]["fields"], json!([{ "name": "coords", "ty": "(u8, u8)" }]));
    }

    // A struct variant with an `fn`-pointer field type (also parenthesised) is
    // likewise recognised as a struct variant.
    #[test]
    fn test_parse_variant_named_with_fn_field_type() {
        let value = parse_variant("Callback { f: fn(u8) }").unwrap();
        assert_eq!(value["name"], json!("Callback"));
        assert_eq!(value["payload"]["kind"], json!("struct"));
        assert_eq!(value["payload"]["fields"], json!([{ "name": "f", "ty": "fn(u8)" }]));
    }

    #[test]
    fn test_parse_method_with_receiver_and_return() {
        let value = parse_method("fn run(&self, id: u32) -> bool").unwrap();
        assert_eq!(value["name"], json!("run"));
        assert_eq!(value["receiver"], json!("&self"));
        assert_eq!(value["params"], json!([{ "name": "id", "ty": "u32" }]));
        assert_eq!(value["returns"], json!("bool"));
        assert_eq!(value["is_async"], json!(false));
    }

    #[test]
    fn test_parse_method_associated_no_return() {
        let value = parse_method("fn make()").unwrap();
        assert_eq!(value["name"], json!("make"));
        assert_eq!(value.get("receiver"), None);
        assert_eq!(value["returns"], json!("()"));
    }

    #[test]
    fn test_parse_method_with_generics_and_where() {
        let value = parse_method("fn parse<T: Clone>(input: T) -> T where T: Send").unwrap();
        assert_eq!(value["name"], json!("parse"));
        assert_eq!(value["generics"], json!([{ "name": "T", "bounds": ["Clone"] }]));
        assert_eq!(value["returns"], json!("T"));
        assert_eq!(
            value["where_predicates"],
            json!([{ "lhs": "T", "rhs": ["Send"], "operator": "Bound" }])
        );
    }

    #[test]
    fn test_parse_method_rejects_non_signature() {
        assert!(matches!(parse_method("not a function"), Err(CatalogError::ParseFragment { .. })));
    }

    // Finding F1 (PR #182 round 10): a parenthesised bound in the generic list
    // (`Fn(u32) -> bool`) contains `(...)` that precedes the real parameter list.
    // The signature is valid Rust, so it must parse — the parameter list is the
    // `(...)` following the closed generic block, not the bound's parentheses.
    #[test]
    fn test_parse_method_accepts_fn_bound_in_generics() {
        let value = parse_method("fn run<F: Fn(u32) -> bool>(f: F) -> bool").unwrap();
        assert_eq!(value["name"], json!("run"));
        assert_eq!(value["params"], json!([{ "name": "f", "ty": "F" }]));
        assert_eq!(value["returns"], json!("bool"));
    }

    // Finding 3: a return type whose path contains the substring `where`
    // (e.g. `somewhere::Thing`) must not be mistaken for a where clause.
    #[test]
    fn test_parse_method_return_type_containing_where_substring() {
        let value = parse_method("fn locate() -> somewhere::Thing").unwrap();
        assert_eq!(value["returns"], json!("somewhere::Thing"));
        assert_eq!(value.get("where_predicates"), None);
    }

    // Finding 3: a genuine where clause is still split off even when the return
    // type also contains the substring `where`.
    #[test]
    fn test_parse_method_where_substring_return_with_real_where_clause() {
        let value = parse_method("fn locate<T>() -> somewhere::Thing where T: Send").unwrap();
        assert_eq!(value["returns"], json!("somewhere::Thing"));
        assert_eq!(
            value["where_predicates"],
            json!([{ "lhs": "T", "rhs": ["Send"], "operator": "Bound" }])
        );
    }

    #[test]
    fn test_parse_trait_impl() {
        let value = parse_trait_impl("From<CodecError>", "MyType").unwrap();
        assert_eq!(value["trait_ref"], json!("From<CodecError>"));
        assert_eq!(value["for_type"], json!("MyType"));
    }

    #[test]
    fn test_parse_trait_impl_uses_typeref_newtype_not_path_gate() {
        let sized = parse_trait_impl("?Sized", "MyType").unwrap();
        assert_eq!(sized["trait_ref"], json!("?Sized"));

        let lifetime = parse_trait_impl("'a", "MyType").unwrap();
        assert_eq!(lifetime["trait_ref"], json!("'a"));
    }

    // The real `MethodGenericParam` schema stores `name: ParamName`, so lifetime
    // / const generic forms are rejected by `ParamName::new`, not by a Rust
    // syntax parser. Bounds remain `TypeRef` values.
    #[test]
    fn test_parse_generic_rejects_lifetime_and_const_but_accepts_type_param() {
        assert!(matches!(parse_generic("'a"), Err(CatalogError::SchemaInvalid { .. })));
        assert!(matches!(parse_generic("const N: usize"), Err(CatalogError::SchemaInvalid { .. })));
        assert!(parse_generic("T").is_ok());
        assert!(parse_generic("T: Clone + Send").is_ok());
        assert!(parse_generic("T: ?Sized + 'static").is_ok());
    }

    // A lifetime generic on a method signature routes through the actual
    // `MethodGenericParam.name: ParamName` schema type, so the whole signature is
    // rejected instead of writing an invalid `'a` generic-param name.
    #[test]
    fn test_parse_method_rejects_lifetime_generic() {
        assert!(matches!(
            parse_method("fn make<'a>() -> &'a str"),
            Err(CatalogError::SchemaInvalid { .. })
        ));
    }

    // A fieldless variant with an explicit discriminant carries information
    // that the catalogue's `VariantDecl` DTO cannot encode, so it must be
    // rejected instead of storing `Error = 1` as the variant name.
    #[test]
    fn test_parse_variant_rejects_discriminant_but_accepts_fieldless() {
        assert!(matches!(parse_variant("Error = 1"), Err(CatalogError::SchemaInvalid { .. })));
        let ok = parse_variant("Error").unwrap();
        assert_eq!(ok["name"], json!("Error"));
        assert_eq!(ok["payload"]["kind"], json!("unit"));
    }
}
