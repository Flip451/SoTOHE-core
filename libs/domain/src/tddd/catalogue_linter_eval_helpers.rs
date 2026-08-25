//! Signature-attribution helpers for `evaluate_catalogue_lint`.
//!
//! Identity-sensitive rules use the shared domain resolver in the parent
//! module. These helpers remain for the existing `NoRoleInMethodSignature`
//! rule, whose layer attribution intentionally retains its established
//! short-name semantics.

use std::collections::BTreeMap;

use super::super::RoleKind;
use super::super::helpers::{entry_role_kind, identifier_name_in_str};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::layer_id::LayerId;

/// Returns `true` if a path rooted at `layer_id` and ending with `bare_name`
/// appears in `sig_type` as a boundary-delimited occurrence.
pub(super) fn layer_qualified_name_in_sig(sig_type: &str, layer_id: &str, bare_name: &str) -> bool {
    let prefix = format!("{layer_id}::");
    let mut haystack = sig_type;
    let mut offset = 0usize;
    while let Some(pos) = haystack.find(&prefix) {
        let abs_pos = offset + pos;
        let boundary_before = if abs_pos == 0 {
            true
        } else {
            sig_type[..abs_pos]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_alphanumeric() && c != '_')
        };

        if boundary_before {
            let mut rest = &haystack[pos + prefix.len()..];
            loop {
                if let Some(after_name) = rest.strip_prefix(bare_name) {
                    let terminated =
                        after_name.chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_');
                    if terminated {
                        return true;
                    }
                }
                let seg_end = rest
                    .char_indices()
                    .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                if seg_end == 0 {
                    break;
                }
                match rest.get(seg_end..) {
                    Some(after_seg) if after_seg.starts_with("::") => rest = &after_seg[2..],
                    _ => break,
                }
            }
        }

        let advance = pos + prefix.len();
        offset += advance;
        haystack = &haystack[advance..];
    }
    false
}

/// Returns `true` when `bare_name` appears without a path separator directly
/// before it.
pub(super) fn bare_name_unqualified_in_sig(sig_type: &str, bare_name: &str) -> bool {
    identifier_name_in_str(sig_type, bare_name, |before| !before.ends_with("::"))
}

/// Attributes an unqualified signature occurrence to the catalogue entry that
/// owns it, while preserving the existing target-layer priority behavior.
pub(in crate::tddd::catalogue_linter) fn sig_type_contains_entry(
    sig_type: &str,
    bare_name: &str,
    cat_layer_id: &LayerId,
    target_layer_id: &LayerId,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> bool {
    if layer_qualified_name_in_sig(sig_type, cat_layer_id.as_ref(), bare_name) {
        return true;
    }

    let has_other_qualified = all_catalogues.keys().any(|other_layer| {
        other_layer != cat_layer_id
            && layer_qualified_name_in_sig(sig_type, other_layer.as_ref(), bare_name)
    });
    if has_other_qualified && !bare_name_unqualified_in_sig(sig_type, bare_name) {
        return false;
    }
    if !bare_name_unqualified_in_sig(sig_type, bare_name) {
        return false;
    }

    let target_owns = all_catalogues
        .get(target_layer_id)
        .and_then(|cat| find_in_catalogue(cat, bare_name))
        .is_some();
    if target_owns {
        cat_layer_id == target_layer_id
    } else {
        all_catalogues.get(cat_layer_id).and_then(|cat| find_in_catalogue(cat, bare_name)).is_some()
    }
}

/// Finds the role of a short entry name for signature attribution.
fn find_in_catalogue(catalogue: &CatalogueDocument, bare_name: &str) -> Option<RoleKind> {
    if let Some((_, entry)) = catalogue
        .types()
        .iter()
        .find(|(key, entry)| key.as_str() == bare_name && entry.action() != ItemAction::Delete)
    {
        return Some(entry_role_kind(entry));
    }
    catalogue
        .traits()
        .iter()
        .find(|(key, entry)| key.as_str() == bare_name && entry.action() != ItemAction::Delete)
        .map(|(_, entry)| RoleKind::from_contract_role(entry.role()))
}
