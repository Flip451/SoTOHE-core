//! `SpecRef` with its anchor newtype [`SpecElementId`] and the
//! [`ContentHash`] used to pin a canonical JSON subtree.

use std::fmt;
use std::path::PathBuf;

use crate::ValidationError;

/// Validated newtype for a spec.json element identifier.
///
/// Enforced pattern: two or more ASCII uppercase letters, a single hyphen,
/// and one or more ASCII digits (e.g. `IN-01`, `AC-02`, `CO-03`, `OS-04`).
/// Downstream codec / verify / signal paths receive a [`SpecElementId`] so
/// bare `String` ids never reach them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecElementId(String);

impl SpecElementId {
    /// Validate and wrap `value` as a [`SpecElementId`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidSpecElementId`] when `value` does
    /// not match the `<UPPER>{2,}-<digits>+` pattern.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_spec_element_id(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidSpecElementId(value))
        }
    }
}

impl AsRef<str> for SpecElementId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpecElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_valid_spec_element_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < bytes.len() && bytes.get(i).is_some_and(u8::is_ascii_uppercase) {
        i += 1;
    }
    if i < 2 {
        return false;
    }
    if bytes.get(i) != Some(&b'-') {
        return false;
    }
    i += 1;
    if i >= bytes.len() {
        return false;
    }
    while i < bytes.len() {
        if !bytes.get(i).is_some_and(u8::is_ascii_digit) {
            return false;
        }
        i += 1;
    }
    true
}

/// Validated newtype for a SHA-256 content hash.
///
/// Internally stores the canonical 32-byte form. The constructor accepts
/// either a 64-character lowercase hex string via [`try_from_hex`] or a
/// raw 32-byte array via [`from_bytes`]. Upper-case hex is rejected to keep
/// a single canonical string representation.
///
/// [`try_from_hex`]: ContentHash::try_from_hex
/// [`from_bytes`]: ContentHash::from_bytes
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Construct from a raw 32-byte SHA-256 digest.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse a 64-character lowercase hex string into a [`ContentHash`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidContentHash`] when the input is
    /// not exactly 64 characters, contains non-hex characters, or contains
    /// uppercase hex characters.
    pub fn try_from_hex(s: impl AsRef<str>) -> Result<Self, ValidationError> {
        let s = s.as_ref();
        if s.len() != 64 {
            return Err(ValidationError::InvalidContentHash(s.to_string()));
        }
        let mut out = [0u8; 32];
        for (i, pair) in s.as_bytes().chunks_exact(2).enumerate() {
            let Some(&high) = pair.first() else {
                return Err(ValidationError::InvalidContentHash(s.to_string()));
            };
            let Some(&low) = pair.get(1) else {
                return Err(ValidationError::InvalidContentHash(s.to_string()));
            };
            let Some(byte_slot) = out.get_mut(i) else {
                return Err(ValidationError::InvalidContentHash(s.to_string()));
            };
            let high = nibble_from_lowercase_hex(high)
                .ok_or_else(|| ValidationError::InvalidContentHash(s.to_string()))?;
            let low = nibble_from_lowercase_hex(low)
                .ok_or_else(|| ValidationError::InvalidContentHash(s.to_string()))?;
            *byte_slot = (high << 4) | low;
        }
        Ok(Self(out))
    }

    /// Canonical lowercase hex representation (64 characters).
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in &self.0 {
            out.push(nibble_to_lowercase_hex(byte >> 4));
            out.push(nibble_to_lowercase_hex(byte & 0x0f));
        }
        out
    }

    /// Access the underlying 32-byte digest.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

fn nibble_from_lowercase_hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

fn nibble_to_lowercase_hex(n: u8) -> char {
    match n {
        0..=9 => char::from(b'0' + n),
        10..=15 => char::from(b'a' + (n - 10)),
        _ => '?',
    }
}

/// Structured reference to an element within a spec.json document.
///
/// Fields:
///
/// * `file` — path to the spec.json document (always
///   `track/items/<id>/spec.json` in the current repo).
/// * `anchor` — [`SpecElementId`] identifying the target element (an
///   `IN-xx`, `AC-xx`, `CO-xx`, or `OS-xx` entry).
/// * `hash` — canonical JSON-subtree SHA-256 of the target element, used
///   for staleness detection.
///
/// Used in type catalogue entries' `spec_refs[]` to realise SoT Chain ②
/// (spec ← type catalogue).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecRef {
    pub file: PathBuf,
    pub anchor: SpecElementId,
    pub hash: ContentHash,
}

impl SpecRef {
    pub fn new(file: impl Into<PathBuf>, anchor: SpecElementId, hash: ContentHash) -> Self {
        Self { file: file.into(), anchor, hash }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn spec_element_id_accepts_in_prefix() {
        let id = SpecElementId::try_new("IN-01").unwrap();
        assert_eq!(id.as_ref(), "IN-01");
    }

    #[test]
    fn spec_element_id_accepts_three_letter_prefix() {
        let id = SpecElementId::try_new("ABC-123").unwrap();
        assert_eq!(id.as_ref(), "ABC-123");
    }

    #[test]
    fn spec_element_id_rejects_single_letter_prefix() {
        let err = SpecElementId::try_new("A-01").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidSpecElementId(s) if s == "A-01"));
    }

    #[test]
    fn spec_element_id_rejects_empty() {
        let err = SpecElementId::try_new("").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidSpecElementId(s) if s.is_empty()));
    }

    #[test]
    fn spec_element_id_rejects_missing_digits() {
        let err = SpecElementId::try_new("IN-").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidSpecElementId(s) if s == "IN-"));
    }

    #[test]
    fn spec_element_id_rejects_missing_hyphen() {
        let err = SpecElementId::try_new("IN01").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidSpecElementId(s) if s == "IN01"));
    }

    #[test]
    fn spec_element_id_rejects_lowercase_prefix() {
        let err = SpecElementId::try_new("in-01").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidSpecElementId(s) if s == "in-01"));
    }

    #[test]
    fn spec_element_id_rejects_non_digit_suffix() {
        let err = SpecElementId::try_new("IN-01a").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidSpecElementId(s) if s == "IN-01a"));
    }

    #[test]
    fn content_hash_from_bytes_round_trip_hex() {
        let bytes = [0xabu8; 32];
        let h = ContentHash::from_bytes(bytes);
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(hex, "ab".repeat(32));
        let back = ContentHash::try_from_hex(&hex).unwrap();
        assert_eq!(back, h);
        assert_eq!(back.as_bytes(), &bytes);
    }

    #[test]
    fn content_hash_from_hex_accepts_all_digits() {
        let s = "0".repeat(64);
        let h = ContentHash::try_from_hex(&s).unwrap();
        assert_eq!(h.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn content_hash_rejects_wrong_length() {
        let err = ContentHash::try_from_hex("abc").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidContentHash(s) if s == "abc"));
    }

    #[test]
    fn content_hash_rejects_uppercase_hex() {
        let s = "A".repeat(64);
        let err = ContentHash::try_from_hex(&s).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidContentHash(_)));
    }

    #[test]
    fn content_hash_rejects_non_hex_char() {
        let mut s = "a".repeat(63);
        s.push('z');
        let err = ContentHash::try_from_hex(&s).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidContentHash(_)));
    }

    #[test]
    fn content_hash_display_matches_to_hex() {
        let h = ContentHash::from_bytes([0x12u8; 32]);
        assert_eq!(h.to_string(), h.to_hex());
    }

    #[test]
    fn spec_ref_constructs() {
        let anchor = SpecElementId::try_new("IN-01").unwrap();
        let hash = ContentHash::from_bytes([0u8; 32]);
        let r = SpecRef::new("track/items/x/spec.json", anchor.clone(), hash.clone());
        assert_eq!(r.file, PathBuf::from("track/items/x/spec.json"));
        assert_eq!(r.anchor, anchor);
        assert_eq!(r.hash, hash);
    }
}
