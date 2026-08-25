//! Root-shape validation for catalogue-lint TypeRef identity resolution.

use super::{CatalogueLinterError, ExtractedTypeRefPath, TypeRefPathExtractorPort};
use crate::tddd::catalogue_v2::identifiers::{ParamName, TypeRef};
use crate::tddd::catalogue_v2::identity_resolution::CatalogueIdentityResolutionError;

/// Extracts a TypeRef's root path only when the complete reference has path
/// shape. The syntax adapter reports nested paths for wrappers, so accepting
/// its first path without this check would turn `&Thing` or `(Thing,)` into
/// the identity `Thing`.
pub(super) fn root_path_occurrence<E: TypeRefPathExtractorPort>(
    type_ref: &TypeRef,
    extractor: &E,
    type_parameters: &[ParamName],
) -> Result<TypeRef, CatalogueLinterError> {
    let first_path = extractor
        .extract(type_ref, type_parameters, &[], &[])?
        .into_iter()
        .find_map(|occurrence| match occurrence {
            ExtractedTypeRefPath::Path(path) => Some(path),
            ExtractedTypeRefPath::TypeParameter(_)
            | ExtractedTypeRefPath::LifetimeParameter(_)
            | ExtractedTypeRefPath::ConstParameter(_)
            | ExtractedTypeRefPath::AssociatedItemLabel(_) => None,
        })
        .ok_or_else(|| classification_failed(type_ref))?;

    if !root_path_is_anchored(type_ref, &first_path)? {
        return Err(classification_failed(type_ref));
    }
    Ok(first_path)
}

/// Returns whether the extracted path is the root spelling of `type_ref`.
///
/// The extractor has already parsed the complete TypeRef. This is deliberately
/// not a Rust type parser: it only verifies that the root path is followed by
/// one balanced generic argument list, keeping the catalogue-lint identity
/// check separate from syntax extraction and type-grammar interpretation.
fn root_path_is_anchored(
    type_ref: &TypeRef,
    canonical_path: &TypeRef,
) -> Result<bool, CatalogueLinterError> {
    let mut raw = type_ref.as_str();
    let canonical = canonical_path.as_str();

    if canonical.is_empty() {
        return Ok(false);
    }

    // Absolute and relative spellings have the same lookup semantics. The
    // extractor normally preserves this prefix, but normalizing both sides
    // keeps the comparison aligned with the shared resolver.
    let canonical = canonical.strip_prefix("::").unwrap_or(canonical);
    if let Some(remainder) = raw.strip_prefix("::") {
        raw = remainder;
    }
    if canonical.is_empty() {
        return Ok(false);
    }

    let mut segments = canonical.split("::");
    let Some(first_segment) = segments.next() else {
        return Ok(false);
    };
    raw = trim_ignored_prefix(type_ref, raw)?;
    if !consume_path_segment(&mut raw, first_segment) {
        return Ok(false);
    }

    for segment in segments {
        raw = trim_ignored_prefix(type_ref, raw)?;
        let Some(remainder) = raw.strip_prefix("::") else {
            return Ok(false);
        };
        raw = trim_ignored_prefix(type_ref, remainder)?;
        if !consume_path_segment(&mut raw, segment) {
            return Ok(false);
        }
    }

    // A path may stand alone or be followed by one generic argument list. The
    // scanner below is intentionally limited to balancing delimiters and
    // skipping literals; it does not interpret the contents as Rust syntax.
    let suffix = trim_ignored_prefix(type_ref, raw)?;
    if suffix.is_empty() {
        return Ok(true);
    }
    if !suffix.starts_with('<') {
        return Ok(false);
    }
    validate_generic_suffix(type_ref, suffix)
}

fn validate_generic_suffix(type_ref: &TypeRef, suffix: &str) -> Result<bool, CatalogueLinterError> {
    let Some('<') = suffix.chars().next() else {
        return Ok(false);
    };
    let mut depth = 1_usize;
    let mut delimiters = Vec::new();
    let mut index = '<'.len_utf8();

    while index < suffix.len() {
        let character =
            suffix[index..].chars().next().ok_or_else(|| classification_failed(type_ref))?;
        let width = character.len_utf8();

        if let Some(result) = comment_end(suffix, index) {
            index = result.map_err(|()| classification_failed(type_ref))?;
            continue;
        }

        if depth == 0 && delimiters.is_empty() {
            if character.is_whitespace() {
                index += width;
                continue;
            }
            return Ok(false);
        }

        if let Some(result) = literal_end(suffix, index) {
            index = result.map_err(|()| classification_failed(type_ref))?;
            continue;
        }

        if let Some(expected_close) = delimiters.last().copied() {
            match character {
                '{' => delimiters.push('}'),
                '(' => delimiters.push(')'),
                '[' => delimiters.push(']'),
                '}' | ')' | ']' if character == expected_close => {
                    delimiters.pop();
                }
                '}' | ')' | ']' => return Err(classification_failed(type_ref)),
                _ => {}
            }
            index += width;
            continue;
        }

        match character {
            '{' => delimiters.push('}'),
            '(' => delimiters.push(')'),
            '[' => delimiters.push(']'),
            '}' | ')' | ']' => return Err(classification_failed(type_ref)),
            '<' => {
                depth = depth.checked_add(1).ok_or_else(|| classification_failed(type_ref))?;
            }
            '>' if depth > 0 && !is_return_arrow(suffix, index) => depth -= 1,
            _ => {}
        }
        index += width;
    }

    if depth == 0 && delimiters.is_empty() {
        Ok(true)
    } else {
        Err(classification_failed(type_ref))
    }
}

fn is_return_arrow(source: &str, index: usize) -> bool {
    index > 0 && source.as_bytes().get(index - 1) == Some(&b'-')
}

/// Finds the end of a Rust literal without interpreting its contents.
///
/// The syntax adapter remains authoritative for Rust grammar. This helper only
/// prevents literal delimiters from changing the outer generic-depth check.
fn comment_end(source: &str, index: usize) -> Option<Result<usize, ()>> {
    let remainder = &source[index..];
    if remainder.starts_with("//") {
        let content_start = index + 2;
        let end = source[content_start..]
            .find('\n')
            .map_or(source.len(), |offset| content_start + offset + '\n'.len_utf8());
        return Some(Ok(end));
    }
    if !remainder.starts_with("/*") {
        return None;
    }

    let mut depth = 1_usize;
    let mut cursor = index + 2;
    while cursor < source.len() {
        if source[cursor..].starts_with("/*") {
            let Some(next_depth) = depth.checked_add(1) else {
                return Some(Err(()));
            };
            depth = next_depth;
            cursor += 2;
        } else if source[cursor..].starts_with("*/") {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return Some(Ok(cursor));
            }
        } else {
            let character = source[cursor..].chars().next()?;
            cursor += character.len_utf8();
        }
    }
    Some(Err(()))
}

fn trim_ignored_prefix<'a>(
    type_ref: &TypeRef,
    mut source: &'a str,
) -> Result<&'a str, CatalogueLinterError> {
    loop {
        source = source.trim_start();
        match comment_end(source, 0) {
            Some(Ok(end)) => source = &source[end..],
            Some(Err(())) => return Err(classification_failed(type_ref)),
            None => return Ok(source),
        }
    }
}

fn literal_end(source: &str, index: usize) -> Option<Result<usize, ()>> {
    let remainder = &source[index..];
    if remainder.starts_with("br") || remainder.starts_with('r') {
        if let Some(raw) = raw_string_end(source, index) {
            return Some(raw);
        }
    }

    match remainder.chars().next()? {
        '"' => Some(quoted_literal_end(source, index, '"')),
        '\'' => char_literal_end(source, index),
        'b' if remainder.starts_with("b\"") => {
            Some(quoted_literal_end(source, index + 'b'.len_utf8(), '"'))
        }
        'b' if remainder.starts_with("b'") => char_literal_end(source, index + 'b'.len_utf8()),
        _ => None,
    }
}

fn quoted_literal_end(source: &str, index: usize, quote: char) -> Result<usize, ()> {
    let mut escaped = false;
    let start = index + quote.len_utf8();
    for (offset, character) in source[start..].char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == quote {
            return Ok(start + offset + character.len_utf8());
        }
    }
    Err(())
}

fn raw_string_end(source: &str, index: usize) -> Option<Result<usize, ()>> {
    let remainder = &source[index..];
    let prefix_len = if remainder.starts_with("br") {
        2
    } else if remainder.starts_with('r') {
        1
    } else {
        return None;
    };

    let mut quote_index = index + prefix_len;
    while source[quote_index..].starts_with('#') {
        quote_index += '#'.len_utf8();
    }
    if !source[quote_index..].starts_with('"') {
        return None;
    }

    let hash_count = quote_index - (index + prefix_len);
    let content_start = quote_index + '"'.len_utf8();
    let bytes = source.as_bytes();
    let mut cursor = content_start;
    while cursor < source.len() {
        if bytes.get(cursor) == Some(&b'"') {
            let suffix_start = cursor + '"'.len_utf8();
            let suffix_end = suffix_start + hash_count;
            if bytes
                .get(suffix_start..suffix_end)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
            {
                return Some(Ok(suffix_end));
            }
        }
        let character = source[cursor..].chars().next()?;
        cursor += character.len_utf8();
    }
    Some(Err(()))
}

fn char_literal_end(source: &str, index: usize) -> Option<Result<usize, ()>> {
    let remainder = &source[index..];
    if !remainder.starts_with('\'') {
        return None;
    }

    let content_start = index + '\''.len_utf8();
    let first = source[content_start..].chars().next()?;
    let closing_quote = if first == '\\' {
        let escape_start = content_start + first.len_utf8();
        let escaped = source[escape_start..].chars().next()?;
        if escaped == 'u' && source[escape_start + escaped.len_utf8()..].starts_with('{') {
            let mut cursor = escape_start + escaped.len_utf8() + '{'.len_utf8();
            while cursor < source.len() {
                let character = source[cursor..].chars().next()?;
                cursor += character.len_utf8();
                if character == '}' {
                    break;
                }
            }
            cursor
        } else if escaped == 'x' {
            let mut cursor = escape_start + escaped.len_utf8();
            for _ in 0..2 {
                let digit = source[cursor..].chars().next()?;
                if !digit.is_ascii_hexdigit() {
                    return Some(Err(()));
                }
                cursor += digit.len_utf8();
            }
            cursor
        } else {
            escape_start + escaped.len_utf8()
        }
    } else {
        content_start + first.len_utf8()
    };

    if source[closing_quote..].starts_with('\'') {
        Some(Ok(closing_quote + '\''.len_utf8()))
    } else if first == '\\' {
        Some(Err(()))
    } else {
        // A lifetime such as `'static` is not a character literal.
        None
    }
}

fn consume_path_segment(raw: &mut &str, canonical_segment: &str) -> bool {
    let canonical_segment = canonical_segment.strip_prefix("r#").unwrap_or(canonical_segment);
    if canonical_segment.is_empty() {
        return false;
    }
    let raw_segment = raw.strip_prefix("r#").unwrap_or(*raw);
    let Some(remainder) = raw_segment.strip_prefix(canonical_segment) else {
        return false;
    };
    *raw = remainder;
    true
}

fn classification_failed(type_ref: &TypeRef) -> CatalogueLinterError {
    CatalogueLinterError::IdentityResolutionFailed(
        CatalogueIdentityResolutionError::ClassificationFailed { location: type_ref.clone() },
    )
}
