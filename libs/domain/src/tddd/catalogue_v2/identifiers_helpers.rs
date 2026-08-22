//! Private implementation helpers for catalogue v2 identifiers.

use super::IdentifierError;
use crate::tddd::semantic_verify::CatalogueEntryKey;

// ---------------------------------------------------------------------------
// Identifier validation
// ---------------------------------------------------------------------------

/// Returns `true` if `s` is a syntactically valid Rust identifier fragment:
/// - Non-empty
/// - First character: ASCII alphabetic or underscore
/// - Remaining characters: ASCII alphanumeric or underscore
pub(super) fn is_valid_rust_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
    }
}

/// Splits a catalogue key into its final item name and preceding path segments.
pub(super) fn split_catalogue_key(
    key: &CatalogueEntryKey,
) -> Result<(&str, Vec<&str>), IdentifierError> {
    let segments: Vec<&str> = key.as_str().split("::").collect();
    let (item_name, path_segments) = segments
        .split_last()
        .ok_or_else(|| IdentifierError::InvalidFunctionPath(key.as_str().to_owned()))?;
    if item_name.is_empty() {
        return Err(IdentifierError::InvalidFunctionPath(key.as_str().to_owned()));
    }
    Ok((*item_name, path_segments.to_vec()))
}
