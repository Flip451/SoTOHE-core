//! `CatalogueToExtendedCratePort` — secondary port for Catalogue → ExtendedCrate.
//!
//! This port is declared in the domain layer and implemented in the
//! infrastructure layer by `CatalogueToExtendedCrateCodec` (T005).
//!
//! ## Contract (ADR 2 D8)
//!
//! Converts the selected track catalogue into an `ExtendedCrate` (TypeGraph A).
//! The codec performs:
//! 1. inline → id-reference conversion (`FieldDecl` / `VariantDecl` → separate
//!    `index` items, parent references via `Vec<Id>`).
//! 2. 1 type = 1 Inherent Impl block: all inherent methods are collected into a
//!    single `Impl` block per type.
//! 3. `TypeRef` generics parse via `syn` crate, mapping each identifier to a
//!    `rustdoc_types::Type` variant.
//! 4. `external_crates` auto-build from `TraitImplDeclV2::origin_crate`,
//!    `TypeRef` crate prefixes, and the std prelude allowlist (ADR 2 D5).

use std::collections::BTreeMap;

use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::extended_crate::ExtendedCrate;
use crate::tddd::layer_id::LayerId;
use crate::tddd::new_typegraph_codec_error::NewTypeGraphCodecError;
use crate::tddd::type_signals_doc::RustdocSnapshot;
use rustdoc_types::Crate;

/// Owns the authoritative baseline and current rustdoc graphs for one TDDD layer.
///
/// The context keeps the two graphs together as one value at the catalogue encoding
/// boundary, together with the layer they describe, so callers cannot accidentally
/// associate a layer with graphs from a different layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoritativeRustdocContext {
    layer: LayerId,
    baseline: Crate,
    current: RustdocSnapshot,
}

impl AuthoritativeRustdocContext {
    /// Creates an authoritative rustdoc context for `layer` from a baseline graph
    /// and one identity-bearing current snapshot.
    ///
    /// # Errors
    ///
    /// This constructor is infallible; its inputs are already decoded rustdoc graphs.
    #[must_use]
    pub fn new(layer: LayerId, baseline: Crate, current: RustdocSnapshot) -> Self {
        Self { layer, baseline, current }
    }

    /// Returns the layer described by the authoritative rustdoc graphs.
    #[must_use]
    pub fn layer(&self) -> &LayerId {
        &self.layer
    }

    /// Returns the authoritative baseline rustdoc graph.
    #[must_use]
    pub fn baseline(&self) -> &Crate {
        &self.baseline
    }

    /// Returns the authoritative current rustdoc graph.
    #[must_use]
    pub fn current(&self) -> &Crate {
        self.current.crate_data()
    }

    /// Returns the immutable identity-bearing current rustdoc snapshot.
    #[must_use]
    pub fn current_snapshot(&self) -> &RustdocSnapshot {
        &self.current
    }
}

/// Secondary port: converts a target layer's catalogue into an `ExtendedCrate`.
///
/// Implementors live in the infrastructure layer (see `CatalogueToExtendedCrateCodec`).
/// The domain layer declares only this trait; it does not know about `serde`,
/// file I/O, or `syn` parsing details.
///
/// # Errors
///
/// Returns `NewTypeGraphCodecError` if the selected catalogue contains a
/// `TypeRef` that cannot be parsed as a valid Rust type, or if an identifier is
/// ambiguous in the supplied track-catalogue resolution context.
pub trait CatalogueToExtendedCratePort: Send + Sync {
    /// Encodes the catalogue selected by `target_layer` into an `ExtendedCrate`
    /// (TypeGraph A), using the same-track catalogues and authoritative baseline
    /// and current rustdoc paths for identity resolution.
    ///
    /// `track_catalogues` contains the catalogue for each TDDD-enabled layer in
    /// the active track. It gives the codec the complete declaration context so
    /// add declarations from another layer can be resolved without duplicating
    /// them in the target catalogue.
    ///
    /// `rustdoc_contexts` maps each TDDD-enabled layer to its authoritative
    /// baseline and current rustdoc graphs. Each context's [`layer`](
    /// AuthoritativeRustdocContext::layer) must equal its map key.
    ///
    /// # Errors
    ///
    /// Returns `Err(NewTypeGraphCodecError::InvalidTypeRef)` when a `TypeRef`
    /// string in the selected catalogue fails `syn` parsing or cannot be
    /// reconciled with the authoritative rustdoc paths, or when a rustdoc
    /// context is stored under a key different from its layer.
    ///
    /// Returns `Err(NewTypeGraphCodecError::AmbiguousIdentifier)` when a short
    /// identifier resolves to more than one fully-qualified catalogue entry.
    fn encode(
        &self,
        target_layer: &LayerId,
        track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, NewTypeGraphCodecError>;
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::type_complexity, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::{CrateName, RustdocCratePortError};
    use std::path::PathBuf;

    fn rustdoc_snapshot(crate_data: Crate) -> RustdocSnapshot {
        use crate::tddd::type_signals_doc::{
            CargoProfileName, ExpectedRustdocJsonPath, ResolvedCargoTargetDirectory,
            RustdocExecutionIdentity, construct_rustdoc_snapshot,
        };

        fn decode(bytes: &[u8]) -> Result<Crate, RustdocCratePortError> {
            serde_json::from_slice(bytes).map_err(|error| RustdocCratePortError::ParseFailed {
                crate_name: CrateName::new("test").unwrap(),
                reason: crate::FreeText::new(error.to_string()),
            })
        }

        let bytes = serde_json::to_vec(&crate_data).unwrap();
        let target =
            ResolvedCargoTargetDirectory::try_new(PathBuf::from("/tmp/sotohe-domain-test-target"))
                .unwrap();
        let expected =
            ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/current.json"), &target)
                .unwrap();
        let identity = RustdocExecutionIdentity::new(
            target,
            CrateName::new("domain").unwrap(),
            vec![],
            CargoProfileName::try_new("dev".to_owned()).unwrap(),
            expected,
        )
        .unwrap();
        construct_rustdoc_snapshot(identity, &bytes, decode).unwrap()
    }

    fn rustdoc_crate(crate_version: &str) -> Crate {
        Crate {
            root: rustdoc_types::Id(0),
            crate_version: Some(crate_version.to_owned()),
            includes_private: false,
            index: std::collections::HashMap::new(),
            paths: std::collections::HashMap::new(),
            external_crates: std::collections::HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        }
    }

    #[test]
    fn test_authoritative_rustdoc_context_new_preserves_baseline_and_current_graphs() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = rustdoc_crate("baseline");
        let current = rustdoc_crate("current");
        let context = AuthoritativeRustdocContext::new(
            layer.clone(),
            baseline.clone(),
            rustdoc_snapshot(current.clone()),
        );

        assert_eq!(context.layer(), &layer);
        assert_eq!(context.baseline(), &baseline);
        assert_eq!(context.current(), &current);
        assert_ne!(context.baseline(), context.current());
    }

    #[test]
    fn encode_contract_includes_target_layer_and_track_catalogues() {
        use crate::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use crate::tddd::catalogue_v2::entries::TypeEntry;
        use crate::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use crate::tddd::catalogue_v2::{CatalogueEntryKey, CrateName, ModulePath};

        struct RecordingPort {
            observed: std::sync::Mutex<
                Option<(
                    LayerId,
                    BTreeMap<LayerId, CatalogueDocument>,
                    BTreeMap<LayerId, AuthoritativeRustdocContext>,
                )>,
            >,
        }

        impl CatalogueToExtendedCratePort for RecordingPort {
            fn encode(
                &self,
                target_layer: &LayerId,
                track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
                rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
            ) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
                *self.observed.lock().expect("lock") = Some((
                    target_layer.clone(),
                    track_catalogues.clone(),
                    rustdoc_contexts.clone(),
                ));
                Ok(ExtendedCrate::new(
                    Crate {
                        root: rustdoc_types::Id(0),
                        crate_version: None,
                        includes_private: false,
                        index: std::collections::HashMap::new(),
                        paths: std::collections::HashMap::new(),
                        external_crates: std::collections::HashMap::new(),
                        format_version: rustdoc_types::FORMAT_VERSION,
                        target: rustdoc_types::Target {
                            triple: String::new(),
                            target_features: vec![],
                        },
                    },
                    BTreeMap::new(),
                ))
            }
        }

        let mut declaring = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        declaring.insert_type(
            CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(ModulePath::from_segments(vec!["model".to_owned()]).unwrap()),
                None,
                vec![],
                vec![],
            ),
        );
        let target = CatalogueDocument::new(
            5,
            CrateName::new("usecase").unwrap(),
            LayerId::try_new("usecase").unwrap(),
        );
        let target_layer = LayerId::try_new("usecase").unwrap();
        let track_catalogues = BTreeMap::from([
            (LayerId::try_new("domain").unwrap(), declaring.clone()),
            (target_layer.clone(), target),
        ]);
        let mut baseline = Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: std::collections::HashMap::new(),
            paths: std::collections::HashMap::new(),
            external_crates: std::collections::HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        };
        baseline.crate_version = Some("baseline".to_owned());
        let mut current = baseline.clone();
        current.crate_version = Some("current".to_owned());
        let rustdoc_contexts = BTreeMap::from([(
            target_layer.clone(),
            AuthoritativeRustdocContext::new(
                target_layer.clone(),
                baseline.clone(),
                rustdoc_snapshot(current.clone()),
            ),
        )]);
        let port = RecordingPort { observed: std::sync::Mutex::new(None) };
        port.encode(&target_layer, &track_catalogues, &rustdoc_contexts).unwrap();
        let (observed_layer, observed_catalogues, observed_contexts) =
            port.observed.lock().expect("lock").clone().expect("encode must observe inputs");
        assert_eq!(observed_layer, target_layer);
        assert_eq!(
            observed_catalogues
                .get(&LayerId::try_new("domain").unwrap())
                .and_then(|doc| doc
                    .types()
                    .get(&CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap()))
                .map(|entry| entry.action()),
            Some(ItemAction::Add)
        );
        assert!(
            observed_catalogues
                .get(&target_layer)
                .is_some_and(|doc| !doc.types().keys().any(|key| key.as_str().contains("UserId"))),
            "the referencing catalogue must omit a duplicate declaring-layer add"
        );
        let observed_context = observed_contexts
            .get(&target_layer)
            .expect("encode must observe the target layer's rustdoc context");
        assert_eq!(observed_context.baseline(), &baseline);
        assert_eq!(observed_context.current(), &current);
    }
}
