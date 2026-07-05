//! Identity-only deletion records for the catalogue v2 schema.
//!
//! A [`DeletionRecord`] is the tombstone form of a catalogue entry declared with
//! `action: delete`: it records only the identity of a removed item — never its
//! role, shape, methods, or docs. Deletion records live in
//! `CatalogueDocument::deletions`, structurally separated from the live
//! `types` / `traits` / `functions` maps, so live-entry iteration never has to
//! filter out `ItemAction::Delete` entries.
//!
//! See spec IN-04, GO-03, AC-04; ADR `2026-07-02-1345` D3.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`
//! the domain layer is serialization-free; the infrastructure codec dispatches
//! `action: delete` entries into this type.

use crate::tddd::catalogue_v2::identifiers::{FunctionPath, ModulePath, TraitName, TypeName};

/// An identity-only record of a catalogue entry being removed.
///
/// Each variant names the top-level map (`types` / `traits` / `functions`) whose
/// entry is deleted and carries only the identifier needed to locate it. There
/// is no role / kind / methods / docs: a removed item has no live shape to
/// describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionRecord {
    /// A removed `types`-map entry.
    Type {
        /// The removed type's name (its `types` map key).
        name: TypeName,
        /// The removed type's crate-relative module path.
        module_path: ModulePath,
    },
    /// A removed `traits`-map entry.
    Trait {
        /// The removed trait's name (its `traits` map key).
        name: TraitName,
        /// The removed trait's crate-relative module path.
        module_path: ModulePath,
    },
    /// A removed `functions`-map entry.
    Function {
        /// The removed function's crate-qualified path (its `functions` map key).
        path: FunctionPath,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::tddd::catalogue_v2::identifiers::{CrateName, FunctionName};

    #[test]
    fn test_deletion_record_type_variant_carries_name_and_module() {
        let record = DeletionRecord::Type {
            name: TypeName::new("LayerId").unwrap(),
            module_path: ModulePath::from_str("tddd::foo").unwrap(),
        };
        match record {
            DeletionRecord::Type { name, module_path } => {
                assert_eq!(name.as_str(), "LayerId");
                assert_eq!(module_path.to_string(), "tddd::foo");
            }
            _ => panic!("expected Type variant"),
        }
    }

    #[test]
    fn test_deletion_record_trait_variant_carries_name_and_module() {
        let record = DeletionRecord::Trait {
            name: TraitName::new("CatalogPort").unwrap(),
            module_path: ModulePath::root(),
        };
        assert_eq!(
            record,
            DeletionRecord::Trait {
                name: TraitName::new("CatalogPort").unwrap(),
                module_path: ModulePath::root(),
            }
        );
    }

    #[test]
    fn test_deletion_record_function_variant_carries_path() {
        let crate_name = CrateName::new("domain").unwrap();
        let path = FunctionPath::at_root(crate_name, FunctionName::new("register_user").unwrap());
        let record = DeletionRecord::Function { path: path.clone() };
        match record {
            DeletionRecord::Function { path: got } => assert_eq!(got, path),
            _ => panic!("expected Function variant"),
        }
    }
}
