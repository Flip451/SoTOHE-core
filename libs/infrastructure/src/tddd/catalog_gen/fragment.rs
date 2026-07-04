//! Rust-fragment parsing for the catalogue adapter (D6 / IN-08).
//!
//! Each declaration / signature fragment is first validated with `syn`
//! (fail-closed — unparseable input is rejected), then the caller's original,
//! human-formatted type strings are recovered with a bracket-aware splitter and
//! stored verbatim into the catalogue's structured DTO shape. No hand-rolled
//! type grammar is introduced; `syn` remains the parser of record and the
//! splitters only slice already-validated input.

use domain::tddd::catalogue_linter::FreeText;
use serde_json::{Value, json};
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
/// Returns [`CatalogError::ParseFragment`] when the fragment does not parse as a
/// named field.
pub(super) fn parse_field(fragment: &str) -> Result<Value, CatalogError> {
    let wrapped = format!("struct __Wrapper {{ {fragment} }}");
    let item: syn::ItemStruct = syn::parse_str(&wrapped)
        .map_err(|err| parse_error(format!("field `{fragment}`: {err}")))?;
    if item.fields.len() != 1 {
        return Err(parse_error(format!("field `{fragment}`: expected exactly one `name: Type`")));
    }
    let (name, ty) = split_binding(fragment)
        .ok_or_else(|| parse_error(format!("field `{fragment}`: expected `name: Type`")))?;
    Ok(json!({ "name": name, "ty": ty }))
}

/// Parse a declaration-level generic parameter (`T: Bound + Bound2`) into a
/// `MethodGenericParam` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment is not a valid
/// generic parameter, or [`CatalogError::SchemaInvalid`] when the fragment is a
/// valid lifetime or const generic parameter — forms the catalogue's
/// `MethodGenericParam` DTO (a `name` + type `bounds`) cannot encode. Rejecting
/// them here keeps `catalog add` fail-closed instead of emitting a `name` such
/// as `'a` or `const N` that the codec later rejects as a non-identifier.
pub(super) fn parse_generic(fragment: &str) -> Result<Value, CatalogError> {
    let param: syn::GenericParam = syn::parse_str(fragment)
        .map_err(|err| parse_error(format!("generic `{fragment}`: {err}")))?;
    match param {
        syn::GenericParam::Type(_) => {}
        syn::GenericParam::Lifetime(_) => {
            return Err(schema_error(format!(
                "generic `{fragment}`: lifetime parameters are unsupported by the catalogue \
                 (only simple type parameters like `T` or `T: Bound` are allowed)"
            )));
        }
        syn::GenericParam::Const(_) => {
            return Err(schema_error(format!(
                "generic `{fragment}`: const generic parameters are unsupported by the catalogue \
                 (only simple type parameters like `T` or `T: Bound` are allowed)"
            )));
        }
    }
    let (name, bounds) = match split_binding(fragment) {
        Some((name, rest)) => (name, split_top_level(&rest, '+')),
        None => (fragment.trim().to_owned(), Vec::new()),
    };
    Ok(json!({ "name": name, "bounds": bounds }))
}

/// Parse a where-predicate fragment (`T: Bounds` or `T::Item = X`) into a
/// `WherePredicateDecl` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment is not a valid
/// where predicate.
pub(super) fn parse_where(fragment: &str) -> Result<Value, CatalogError> {
    let _: syn::WherePredicate = syn::parse_str(fragment)
        .map_err(|err| parse_error(format!("where predicate `{fragment}`: {err}")))?;
    if let Some((lhs, rhs_part)) = split_once_top_level(fragment, '=') {
        let rhs = split_top_level(&rhs_part, '+');
        return Ok(json!({ "lhs": lhs, "rhs": rhs, "operator": "Equal" }));
    }
    let (lhs, rhs_part) = split_binding(fragment).ok_or_else(|| {
        parse_error(format!("where predicate `{fragment}`: expected `T: Bounds`"))
    })?;
    let rhs = split_top_level(&rhs_part, '+');
    Ok(json!({ "lhs": lhs, "rhs": rhs, "operator": "Bound" }))
}

/// Parse an enum-variant fragment (`Unit`, `Tuple(A, B)`, `Struct { a: T }`)
/// into a `VariantDecl` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment is not a valid
/// variant, or [`CatalogError::SchemaInvalid`] when the variant carries an
/// explicit discriminant (`Name = <expr>`) — a form the catalogue's
/// `VariantDecl` DTO cannot encode. Rejecting it here keeps `catalog add`
/// fail-closed instead of storing `Name = <expr>` as the variant `name`, which
/// the codec later rejects as a non-identifier.
pub(super) fn parse_variant(fragment: &str) -> Result<Value, CatalogError> {
    let variant: syn::Variant = syn::parse_str(fragment)
        .map_err(|err| parse_error(format!("variant `{fragment}`: {err}")))?;
    if variant.discriminant.is_some() {
        return Err(schema_error(format!(
            "variant `{fragment}`: explicit discriminants (`= <expr>`) are unsupported by the \
             catalogue"
        )));
    }
    let trimmed = fragment.trim();
    if trimmed.contains('(') {
        let name = leading_name(trimmed, '(');
        let inner = extract_delimited(trimmed, '(', ')')
            .ok_or_else(|| parse_error(format!("variant `{fragment}`: unbalanced parentheses")))?;
        let fields = split_top_level(&inner, ',');
        return Ok(json!({ "name": name, "payload": { "kind": "tuple", "fields": fields } }));
    }
    if trimmed.contains('{') {
        let name = leading_name(trimmed, '{');
        let inner = extract_delimited(trimmed, '{', '}')
            .ok_or_else(|| parse_error(format!("variant `{fragment}`: unbalanced braces")))?;
        let mut fields = Vec::new();
        for field_frag in split_top_level(&inner, ',') {
            fields.push(parse_field(&field_frag)?);
        }
        return Ok(json!({ "name": name, "payload": { "kind": "struct", "fields": fields } }));
    }
    Ok(json!({ "name": trimmed, "payload": { "kind": "unit" } }))
}

/// Parse a method / function signature (`fn name<G>(recv, p: T) -> R where W`)
/// into a `MethodDeclaration` value.
///
/// # Errors
///
/// Returns [`CatalogError::ParseFragment`] when the fragment is not a valid
/// signature.
pub(super) fn parse_method(fragment: &str) -> Result<Value, CatalogError> {
    let signature: syn::Signature = syn::parse_str(fragment)
        .map_err(|err| parse_error(format!("method `{fragment}`: {err}")))?;
    let name = signature.ident.to_string();
    let is_async = signature.asyncness.is_some();

    let params_inner = extract_delimited(fragment, '(', ')')
        .ok_or_else(|| parse_error(format!("method `{fragment}`: missing parameter list")))?;
    let mut receiver: Option<String> = None;
    let mut params: Vec<Value> = Vec::new();
    for (index, part) in split_top_level(&params_inner, ',').into_iter().enumerate() {
        if index == 0 {
            if let Some(recv) = self_receiver(&part) {
                receiver = Some(recv);
                continue;
            }
        }
        params.push(parse_field(&part)?);
    }

    let after_params = tail_after_delimited(fragment, '(', ')').unwrap_or_default();
    let has_where = signature.generics.where_clause.is_some();
    let (returns, where_body) = split_return_and_where(&after_params, has_where);
    let generics = extract_method_generics(fragment)?;
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
/// Returns [`CatalogError::ParseFragment`] when the trait reference is invalid.
pub(super) fn parse_trait_impl(fragment: &str, for_type: &str) -> Result<Value, CatalogError> {
    let _: syn::TypeParamBound = syn::parse_str(fragment)
        .map_err(|err| parse_error(format!("trait impl `{fragment}`: {err}")))?;
    Ok(json!({ "trait_ref": fragment.trim(), "for_type": for_type }))
}

// ---------------------------------------------------------------------------
// Bracket-aware substring recovery helpers
// ---------------------------------------------------------------------------

/// Split `input` on `delimiter` at bracket depth 0, trimming and dropping empty
/// pieces. `->` is treated as a literal token so return arrows do not skew depth.
fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
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
    parts.into_iter().filter(|part| !part.is_empty()).collect()
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

/// The identifier before the first `open` delimiter.
fn leading_name(fragment: &str, open: char) -> String {
    match fragment.find(open) {
        Some(idx) => fragment.get(..idx).unwrap_or("").trim().to_owned(),
        None => fragment.trim().to_owned(),
    }
}

/// The substring strictly inside the first balanced `open`..`close` pair.
fn extract_delimited(input: &str, open: char, close: char) -> Option<String> {
    let start = input.find(open)?;
    let mut depth: i32 = 0;
    let mut result = String::new();
    for (idx, ch) in input.char_indices() {
        if idx < start {
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
    for (idx, ch) in input.char_indices() {
        if idx < start {
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
/// where-clause body. `has_where` is the syn-derived signal for whether the
/// signature actually carries a where clause, so a return type that merely
/// contains the substring `where` (e.g. `somewhere::T`) is never mistaken for
/// one.
fn split_return_and_where(after: &str, has_where: bool) -> (String, Option<String>) {
    let where_start = if has_where { find_top_level_keyword(after, "where") } else { None };
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

/// Extract declaration generics from `fn name<...>(` into a vec of values.
fn extract_method_generics(fragment: &str) -> Result<Vec<Value>, CatalogError> {
    let paren = fragment.find('(').unwrap_or(fragment.len());
    let head = fragment.get(..paren).unwrap_or("");
    if !head.contains('<') {
        return Ok(Vec::new());
    }
    let inner = extract_delimited(head, '<', '>')
        .ok_or_else(|| parse_error(format!("method `{fragment}`: unbalanced generics")))?;
    let mut generics = Vec::new();
    for part in split_top_level(&inner, ',') {
        generics.push(parse_generic(&part)?);
    }
    Ok(generics)
}

/// Recognise a `self` / `&self` / `&mut self` receiver.
fn self_receiver(part: &str) -> Option<String> {
    let compact: String = part.chars().filter(|ch| !ch.is_whitespace()).collect();
    match compact.as_str() {
        "self" => Some("self".to_owned()),
        "&self" => Some("&self".to_owned()),
        "&mutself" => Some("&mut self".to_owned()),
        _ => None,
    }
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
    fn test_parse_variant_unit_and_tuple() {
        let unit = parse_variant("Idle").unwrap();
        assert_eq!(unit["name"], json!("Idle"));
        assert_eq!(unit["payload"]["kind"], json!("unit"));

        let tuple = parse_variant("Pair(i32, u8)").unwrap();
        assert_eq!(tuple["name"], json!("Pair"));
        assert_eq!(tuple["payload"]["kind"], json!("tuple"));
        assert_eq!(tuple["payload"]["fields"], json!(["i32", "u8"]));
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

    // Finding F1: lifetime and const generic parameters parse as valid Rust
    // (`syn::GenericParam`) but the catalogue's `MethodGenericParam` DTO cannot
    // encode them, so they must be rejected at the fragment boundary rather than
    // emitting a `name` like `'a` / `const N` that the codec later rejects.
    // A simple type parameter (with or without bounds) is still accepted.
    #[test]
    fn test_parse_generic_rejects_lifetime_and_const_but_accepts_type_param() {
        assert!(matches!(parse_generic("'a"), Err(CatalogError::SchemaInvalid { .. })));
        assert!(matches!(parse_generic("const N: usize"), Err(CatalogError::SchemaInvalid { .. })));
        assert!(parse_generic("T").is_ok());
        assert!(parse_generic("T: Clone + Send").is_ok());
    }

    // Finding F1: a lifetime generic on a method signature routes through
    // `extract_method_generics` → `parse_generic`, so the whole signature is
    // rejected instead of writing an invalid `'a` generic-param name.
    #[test]
    fn test_parse_method_rejects_lifetime_generic() {
        assert!(matches!(
            parse_method("fn make<'a>() -> &'a str"),
            Err(CatalogError::SchemaInvalid { .. })
        ));
    }

    // Finding F2: a fieldless variant with an explicit discriminant parses as a
    // valid `syn::Variant` but the catalogue's `VariantDecl` DTO cannot encode
    // the discriminant, so it must be rejected instead of storing `Error = 1` as
    // the variant name. A plain fieldless variant is still accepted.
    #[test]
    fn test_parse_variant_rejects_discriminant_but_accepts_fieldless() {
        assert!(matches!(parse_variant("Error = 1"), Err(CatalogError::SchemaInvalid { .. })));
        let ok = parse_variant("Error").unwrap();
        assert_eq!(ok["name"], json!("Error"));
        assert_eq!(ok["payload"]["kind"], json!("unit"));
    }
}
