//! Identity-only deletion records for the catalogue v2 schema.
//!
//! A [`DeletionRecord`] is the tombstone form of a catalogue entry declared with
//! `action: delete`: it records the identity and grounding of a removed item —
//! never its role, shape, methods, or docs. Deletion records live in
//! `CatalogueDocument::deletions`, structurally separated from the live
//! `types` / `traits` / `functions` maps, so live-entry iteration never has to
//! filter out `ItemAction::Delete` entries.
//!
//! See spec IN-04, GO-03, AC-04; ADR `2026-07-02-1345` D3.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`
//! the domain layer is serialization-free; the infrastructure codec dispatches
//! `action: delete` entries into this type.

use crate::plan_ref::{InformalGroundRef, SpecRef};
use crate::tddd::catalogue_v2::identifiers::FunctionPath;
use crate::tddd::semantic_verify::CatalogueEntryKey;

/// A grounded identity record of a catalogue entry being removed.
///
/// Each variant names the top-level map (`types` / `traits` / `functions`) whose
/// entry is deleted and carries the identifier needed to locate it plus the
/// same grounding fields used by live entries. There is no role / kind /
/// methods / docs: a removed item has no live shape to describe, but it still
/// needs an explicit reason to remove it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionRecord {
    /// A removed `types`-map entry.
    Type {
        /// The removed type's name (its `types` map key).
        name: CatalogueEntryKey,
        /// Spec anchors grounding the deletion.
        spec_refs: Vec<SpecRef>,
        /// Informal grounds for deletion work not yet promoted to spec refs.
        informal_grounds: Vec<InformalGroundRef>,
    },
    /// A removed `traits`-map entry.
    Trait {
        /// The removed trait's name (its `traits` map key).
        name: CatalogueEntryKey,
        /// Spec anchors grounding the deletion.
        spec_refs: Vec<SpecRef>,
        /// Informal grounds for deletion work not yet promoted to spec refs.
        informal_grounds: Vec<InformalGroundRef>,
    },
    /// A removed `functions`-map entry.
    Function {
        /// The removed function's crate-qualified path (its `functions` map key).
        path: FunctionPath,
        /// Spec anchors grounding the deletion.
        spec_refs: Vec<SpecRef>,
        /// Informal grounds for deletion work not yet promoted to spec refs.
        informal_grounds: Vec<InformalGroundRef>,
    },
}

impl DeletionRecord {
    /// Spec anchors grounding this deletion.
    #[must_use]
    pub fn spec_refs(&self) -> &[SpecRef] {
        match self {
            Self::Type { spec_refs, .. }
            | Self::Trait { spec_refs, .. }
            | Self::Function { spec_refs, .. } => spec_refs,
        }
    }

    /// Informal grounds associated with this deletion.
    #[must_use]
    pub fn informal_grounds(&self) -> &[InformalGroundRef] {
        match self {
            Self::Type { informal_grounds, .. }
            | Self::Trait { informal_grounds, .. }
            | Self::Function { informal_grounds, .. } => informal_grounds,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::{CrateName, FunctionName};

    #[test]
    fn test_deletion_record_type_variant_carries_full_key() {
        let record = DeletionRecord::Type {
            name: CatalogueEntryKey::try_new("domain::tddd::foo::LayerId".to_owned()).unwrap(),
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        match record {
            DeletionRecord::Type { name, spec_refs, informal_grounds } => {
                assert_eq!(name.as_str(), "domain::tddd::foo::LayerId");
                assert!(spec_refs.is_empty());
                assert!(informal_grounds.is_empty());
            }
            _ => panic!("expected Type variant"),
        }
    }

    #[test]
    fn test_deletion_record_trait_variant_carries_full_key() {
        let record = DeletionRecord::Trait {
            name: CatalogueEntryKey::try_new("domain::CatalogPort".to_owned()).unwrap(),
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(
            record,
            DeletionRecord::Trait {
                name: CatalogueEntryKey::try_new("domain::CatalogPort".to_owned()).unwrap(),
                spec_refs: vec![],
                informal_grounds: vec![],
            }
        );
    }

    #[test]
    fn test_deletion_record_function_variant_carries_path() {
        let crate_name = CrateName::new("domain").unwrap();
        let path = FunctionPath::at_root(crate_name, FunctionName::new("register_user").unwrap());
        let record = DeletionRecord::Function {
            path: path.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        match record {
            DeletionRecord::Function { path: got, spec_refs, informal_grounds } => {
                assert_eq!(got, path);
                assert!(spec_refs.is_empty());
                assert!(informal_grounds.is_empty());
            }
            _ => panic!("expected Function variant"),
        }
    }
}
