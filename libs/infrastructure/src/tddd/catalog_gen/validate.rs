//! Validated-input checks for the catalogue adapter (D5): role vocabulary and
//! spec-anchor existence, both fail-closed at input time.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::CatalogEntryKind;
use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::{ContractRole, DataRole, FunctionRole};
use serde_json::{Value, json};
use usecase::catalog_gen::CatalogError;

use super::fs_access::{port_error, schema_error};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::catalogue_spec_refs::read_spec_element_hashes;

/// Validate a role string against the section vocabulary for `kind`.
///
/// # Errors
///
/// Returns [`CatalogError::InvalidRole`] when `role` is outside the section's
/// closed vocabulary.
pub(super) fn validate_role(kind: CatalogEntryKind, role: &str) -> Result<(), CatalogError> {
    let valid = match kind {
        CatalogEntryKind::Struct | CatalogEntryKind::Enum | CatalogEntryKind::TypeAlias => {
            DataRole::from_str(role).is_ok()
        }
        CatalogEntryKind::Trait => ContractRole::from_str(role).is_ok(),
        CatalogEntryKind::Function => FunctionRole::from_str(role).is_ok(),
    };
    if valid { Ok(()) } else { Err(CatalogError::InvalidRole { role: FreeText::new(role) }) }
}

/// Load the set of spec anchors declared in the track's `spec.json`.
///
/// # Errors
///
/// Returns [`CatalogError::Port`] when the spec cannot be read or parsed.
pub(super) fn load_spec_anchors(
    track_dir: &Path,
    items_dir: &Path,
) -> Result<BTreeSet<SpecElementId>, CatalogError> {
    let hashes = read_spec_element_hashes(track_dir, items_dir)
        .map_err(|err| port_error(format!("failed to load spec anchors: {err}")))?;
    Ok(hashes.into_keys().collect())
}

/// Load spec anchors for the read-only `check` path.
///
/// A genuinely absent Phase-0 `spec.json` yields an empty anchor set, but any
/// symlink, read, or parse error remains fail-closed.
///
/// # Errors
///
/// Returns [`CatalogError::Port`] when `spec.json` exists but cannot be safely
/// read or parsed.
pub(super) fn load_spec_anchors_for_check(
    track_dir: &Path,
    items_dir: &Path,
) -> Result<BTreeSet<SpecElementId>, CatalogError> {
    let spec_path = track_dir.join("spec.json");
    match reject_symlinks_below(&spec_path, items_dir)
        .map_err(|err| port_error(format!("symlink guard: {}: {err}", spec_path.display())))?
    {
        false => Ok(BTreeSet::new()),
        true => load_spec_anchors(track_dir, items_dir),
    }
}

/// Validate that `anchor` is well-formed and present in `spec_anchors`.
///
/// # Errors
///
/// Returns [`CatalogError::AnchorNotFound`] for a well-formed but absent anchor,
/// or [`CatalogError::SchemaInvalid`] for a malformed anchor string.
pub(super) fn validate_anchor(
    anchor: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<SpecElementId, CatalogError> {
    let element = SpecElementId::try_new(anchor)
        .map_err(|_| schema_error(format!("invalid spec anchor `{anchor}`")))?;
    if spec_anchors.contains(&element) {
        Ok(element)
    } else {
        Err(CatalogError::AnchorNotFound { anchor: element })
    }
}

/// Build a validated `spec_refs` array (each anchor must resolve).
///
/// # Errors
///
/// Returns [`CatalogError::AnchorNotFound`] / [`CatalogError::SchemaInvalid`]
/// when an anchor does not resolve.
pub(super) fn spec_refs_value(
    anchors: &[String],
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<Value, CatalogError> {
    let mut refs = Vec::new();
    for anchor in anchors {
        let element = validate_anchor(anchor, spec_anchors)?;
        refs.push(json!({ "file": spec_file, "anchor": element.as_ref() }));
    }
    Ok(Value::Array(refs))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn anchor_set() -> BTreeSet<SpecElementId> {
        let mut set = BTreeSet::new();
        set.insert(SpecElementId::try_new("IN-01").unwrap());
        set.insert(SpecElementId::try_new("AC-03").unwrap());
        set
    }

    #[test]
    fn test_validate_role_accepts_section_vocabulary() {
        assert!(validate_role(CatalogEntryKind::Struct, "ValueObject").is_ok());
        assert!(validate_role(CatalogEntryKind::Struct, "SecondaryAdapter").is_ok());
        assert!(validate_role(CatalogEntryKind::Trait, "SecondaryPort").is_ok());
        assert!(validate_role(CatalogEntryKind::Function, "FreeFunction").is_ok());
    }

    #[test]
    fn test_validate_role_rejects_out_of_vocabulary() {
        let err = validate_role(CatalogEntryKind::Struct, "Bogus").unwrap_err();
        assert!(matches!(err, CatalogError::InvalidRole { .. }));
        // A traits-section role is not valid in the types section.
        assert!(validate_role(CatalogEntryKind::Struct, "SecondaryPort").is_err());
    }

    #[test]
    fn test_validate_anchor_accepts_present() {
        let set = anchor_set();
        let element = validate_anchor("IN-01", &set).unwrap();
        assert_eq!(element.as_ref(), "IN-01");
    }

    #[test]
    fn test_validate_anchor_rejects_absent() {
        let set = anchor_set();
        let err = validate_anchor("ZZ-99", &set).unwrap_err();
        assert!(matches!(err, CatalogError::AnchorNotFound { .. }));
    }

    #[test]
    fn test_validate_anchor_rejects_malformed() {
        let set = anchor_set();
        let err = validate_anchor("not-an-anchor", &set).unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
    }
}
