//! Conversion of three-way evaluator output into persisted type signals.

use std::collections::{BTreeMap, HashMap, HashSet};

use domain::{ConfidenceSignal, TypeSignal};

use crate::tddd::ThreeWaySignal;

#[path = "signal_builder/accumulator.rs"]
mod accumulator;
#[path = "signal_builder/identity_index.rs"]
mod identity_index;

pub(super) use accumulator::{
    AccEntry, AccKey, accumulator_namespaces, impl_owner_namespace, kind_tags_for_accumulator,
    kind_tags_for_entry, record_plain_signal, signal_identity, signal_kind_to_confidence,
    stored_entry_key_for_kind_name, worse_signal,
};
pub(super) use identity_index::{TypeSignalIdentityIndex, build_type_signal_identity_index};
#[cfg(test)]
pub(super) use identity_index::{add_deletion_identity, add_entry_identity};

pub(super) fn build_type_signals_from_report<'a>(
    signals: impl Iterator<Item = &'a ThreeWaySignal>,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
    identity_index: &TypeSignalIdentityIndex,
) -> Vec<TypeSignal> {
    use domain::tddd::signal_evaluator::region::SignalRegion;

    let mut acc: HashMap<AccKey, AccEntry> = HashMap::new();
    let mut order: Vec<AccKey> = Vec::new();
    let mut external_owner_keys: HashSet<AccKey> = HashSet::new();

    for signal in signals {
        let name = signal.item_name();
        let confidence = signal_kind_to_confidence(signal.signal());
        let found_in_c = !matches!(
            signal.region(),
            SignalRegion::SMinusC_Add
                | SignalRegion::SMinusC_Modify
                | SignalRegion::SMinusC_Reference
                | SignalRegion::DMinusC
        );

        if let Some(sep) = name.find(": ") {
            let raw_owner = &name[..sep];
            let trait_part = &name[sep + 2..];
            let candidates = identity_index.owner_candidates(raw_owner);
            let ambiguous = candidates.as_ref().is_some_and(|keys| keys.len() > 1);
            let owners = candidates.map_or_else(|| vec![raw_owner.to_owned()], |keys| keys.clone());

            for owner in owners {
                let namespace = impl_owner_namespace(&owner, kind_tag_map, identity_index);
                let acc_key = (owner, namespace);
                if namespace.is_none() {
                    external_owner_keys.insert(acc_key.clone());
                }
                let entry = acc.entry(acc_key.clone()).or_insert_with(|| {
                    order.push(acc_key.clone());
                    (
                        if ambiguous { ConfidenceSignal::Yellow } else { ConfidenceSignal::Blue },
                        true,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                });
                if ambiguous {
                    entry.0 = worse_signal(entry.0, ConfidenceSignal::Yellow);
                }

                if signal.region() != SignalRegion::DMinusC {
                    entry.0 = worse_signal(entry.0, confidence);
                    match signal.region() {
                        SignalRegion::SIntersectC_Match_Add
                        | SignalRegion::SIntersectC_Match_Modify => {
                            entry.2.push(trait_part.to_owned());
                        }
                        SignalRegion::CMinusSUnionD => {
                            entry.4.push(trait_part.to_owned());
                        }
                        _ => {
                            entry.3.push(trait_part.to_owned());
                        }
                    }
                }
            }
        } else if let Some(namespace) = signal.identity().namespace() {
            let candidates = identity_index
                .declaration_candidates_in_namespace(name, namespace)
                .or_else(|| {
                    kind_tag_map
                        .get(name)
                        .filter(|kind_tags| {
                            kind_tags.iter().any(|kind_tag| {
                                super::signal_tags::kind_tag_namespace(kind_tag) == Some(namespace)
                            })
                        })
                        .map(|_| vec![name.to_owned()])
                })
                .unwrap_or_else(|| vec![name.to_owned()]);
            let ambiguous = candidates.len() > 1;
            for key in candidates {
                // A typed report identity always stays in its own namespace.
                // This also separates it from a same-named function label.
                record_plain_signal(
                    &mut acc,
                    &mut order,
                    (key, Some(namespace)),
                    confidence,
                    found_in_c,
                    ambiguous,
                );
            }
        } else {
            // Namespace-less labels are functions (or other report-only
            // labels). Their report spelling is their identity; catalogue
            // declaration aliases do not apply to them.
            record_plain_signal(
                &mut acc,
                &mut order,
                (name.to_owned(), None),
                confidence,
                found_in_c,
                false,
            );
        }
    }

    // Fill suppressed Reference rows independently for every namespace. A
    // function key is deliberately kept raw even when it aliases a type key.
    for name in kind_tag_map.keys() {
        for namespace in accumulator_namespaces(name, kind_tag_map, identity_index) {
            let entry_name = namespace.map_or_else(
                || name.clone(),
                |namespace| stored_entry_key_for_kind_name(name, namespace, identity_index),
            );
            // An alias without namespace metadata is a report-label join, not
            // enough evidence to manufacture a catalogue identity. Direct
            // catalogue keys and the production index both retain metadata.
            let namespace = if namespace.is_some()
                && !identity_index.has_known_namespace(&entry_name)
                && entry_name != *name
            {
                None
            } else {
                namespace
            };
            let acc_key = (entry_name.clone(), namespace);
            if acc.contains_key(&acc_key) {
                continue;
            }
            order.push(acc_key.clone());
            acc.insert(acc_key, (ConfidenceSignal::Blue, true, Vec::new(), Vec::new(), Vec::new()));
        }
    }

    order
        .into_iter()
        .flat_map(|(name, namespace)| {
            let Some((sig, found_type, found_items, missing_items, extra_items)) =
                acc.remove(&(name.clone(), namespace))
            else {
                return Vec::new();
            };
            let kind_tags = kind_tags_for_accumulator(
                namespace,
                &kind_tags_for_entry(&name, kind_tag_map, identity_index),
                external_owner_keys.contains(&(name.clone(), namespace)),
            );
            if kind_tags.is_empty() {
                return vec![TypeSignal::new(
                    signal_identity(name, namespace),
                    "unknown".to_owned(),
                    sig,
                    found_type,
                    found_items,
                    missing_items,
                    extra_items,
                )];
            }
            let is_collision = kind_tags.len() > 1;
            kind_tags
                .iter()
                .map(|&kind_tag| {
                    let effective_signal = if is_collision {
                        worse_signal(sig, ConfidenceSignal::Yellow)
                    } else {
                        sig
                    };
                    TypeSignal::new(
                        signal_identity(name.clone(), namespace),
                        kind_tag.to_owned(),
                        effective_signal,
                        found_type,
                        found_items.clone(),
                        missing_items.clone(),
                        extra_items.clone(),
                    )
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::useless_vec)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::{
        TypeSignalIdentityIndex, add_deletion_identity, add_entry_identity,
        build_type_signal_identity_index, build_type_signals_from_report,
    };
    use domain::FreeText;
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CatalogueEntryKey, CrateName, DeletionRecord, ModulePath, TypeRef,
    };
    use domain::tddd::signal_evaluator::region::{SignalRegion, ThreeWaySignal};
    use rustdoc_types::{Id, ItemKind, ItemSummary};

    #[test]
    fn test_build_type_signals_keeps_same_key_type_and_trait_rows_independent() {
        use domain::ConfidenceSignal;

        // A catalogue may declare a type and a trait under one key. The
        // evaluator emits one namespace-aware signal each; the persisted rows
        // must keep their own status instead of collapsing into one
        // accumulator and forcing both to Yellow.
        let index = TypeSignalIdentityIndex::default();
        let mut kinds = BTreeMap::new();
        kinds.insert("Shared".to_owned(), vec!["value_object", "secondary_port"]);
        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Trait,
                SignalRegion::SMinusC_Add,
            ),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 2, "{built:?}");
        let type_row = built.iter().find(|row| row.kind_tag() == "value_object").expect("type row");
        let trait_row =
            built.iter().find(|row| row.kind_tag() == "secondary_port").expect("trait row");
        assert_eq!(type_row.identity().namespace(), Some(CatalogueItemNamespace::Type));
        assert_eq!(trait_row.identity().namespace(), Some(CatalogueItemNamespace::Trait));
        assert_eq!(type_row.type_name(), "Shared");
        assert_eq!(trait_row.type_name(), "Shared");
        assert_eq!(type_row.signal(), ConfidenceSignal::Blue);
        assert!(type_row.found_type());
        assert_eq!(trait_row.signal(), ConfidenceSignal::Yellow);
        assert!(!trait_row.found_type());
    }

    #[test]
    fn test_build_type_signals_fills_missing_namespace_row_for_shared_key() {
        use domain::ConfidenceSignal;

        // Only the trait produced a signal (a matched Reference type is
        // suppressed by Phase 2); the type row is still filled as Blue and the
        // trait row keeps its own status.
        let index = TypeSignalIdentityIndex::default();
        let mut kinds = BTreeMap::new();
        kinds.insert("Shared".to_owned(), vec!["value_object", "secondary_port"]);
        let signals = vec![ThreeWaySignal::catalogue_item(
            FreeText::new("Shared"),
            CatalogueItemNamespace::Trait,
            SignalRegion::SIntersectC_Mismatch_Modify,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 2, "{built:?}");
        let type_row = built.iter().find(|row| row.kind_tag() == "value_object").expect("type row");
        let trait_row =
            built.iter().find(|row| row.kind_tag() == "secondary_port").expect("trait row");
        assert_eq!(type_row.identity().namespace(), Some(CatalogueItemNamespace::Type));
        assert_eq!(trait_row.identity().namespace(), Some(CatalogueItemNamespace::Trait));
        assert_eq!(type_row.signal(), ConfidenceSignal::Blue);
        assert_eq!(trait_row.signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_build_type_signals_resolves_catalogue_labels_in_their_namespace() {
        let crate_name = CrateName::new("domain").expect("valid crate name");
        let root = ModulePath::root();
        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let mut index = TypeSignalIdentityIndex::default();
        add_entry_identity(
            &mut index,
            &crate_name,
            "domain::Shared",
            Some(&root),
            CatalogueItemNamespace::Type,
            &rustdoc_paths,
        )
        .expect("type identity resolves");
        add_entry_identity(
            &mut index,
            &crate_name,
            "Shared",
            Some(&root),
            CatalogueItemNamespace::Trait,
            &rustdoc_paths,
        )
        .expect("trait identity resolves");

        let kinds = BTreeMap::from([
            ("domain::Shared".to_owned(), vec!["value_object"]),
            ("Shared".to_owned(), vec!["secondary_port"]),
        ]);
        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Trait,
                SignalRegion::SMinusC_Add,
            ),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let type_row = built
            .iter()
            .find(|signal| signal.type_name() == "domain::Shared")
            .expect("type label resolves to its qualified stored key");
        assert_eq!(type_row.kind_tag(), "value_object");
        assert!(type_row.found_type());
        let trait_row = built
            .iter()
            .find(|signal| signal.type_name() == "Shared")
            .expect("trait label resolves to its stored key");
        assert_eq!(trait_row.kind_tag(), "secondary_port");
        assert!(!trait_row.found_type());
    }

    #[test]
    fn test_build_type_signals_separates_same_key_deletion_namespaces() {
        use domain::ConfidenceSignal;

        let crate_name = CrateName::new("domain").expect("valid crate name");
        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let mut index = TypeSignalIdentityIndex::default();
        add_deletion_identity(
            &mut index,
            &crate_name,
            "Shared",
            CatalogueItemNamespace::Type,
            &rustdoc_paths,
        )
        .expect("type tombstone identity resolves");
        add_deletion_identity(
            &mut index,
            &crate_name,
            "Shared",
            CatalogueItemNamespace::Trait,
            &rustdoc_paths,
        )
        .expect("trait tombstone identity resolves");

        assert_eq!(
            super::accumulator_namespaces("Shared", &BTreeMap::new(), &index),
            vec![Some(CatalogueItemNamespace::Type), Some(CatalogueItemNamespace::Trait)]
        );

        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::DMinusC,
            ),
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Trait,
                SignalRegion::DIntersectC,
            ),
        ];

        let built = build_type_signals_from_report(signals.iter(), &BTreeMap::new(), &index);

        assert_eq!(built.len(), 2, "each tombstone namespace needs its own row: {built:?}");
        let type_row = built
            .iter()
            .find(|row| row.identity().namespace() == Some(CatalogueItemNamespace::Type))
            .expect("type tombstone row");
        assert_eq!(type_row.type_name(), "Shared");
        assert_eq!(type_row.kind_tag(), "unknown");
        assert_eq!(type_row.signal(), ConfidenceSignal::Blue);
        let trait_row = built
            .iter()
            .find(|row| row.identity().namespace() == Some(CatalogueItemNamespace::Trait))
            .expect("trait tombstone row");
        assert_eq!(trait_row.type_name(), "Shared");
        assert_eq!(trait_row.kind_tag(), "unknown");
        assert_eq!(trait_row.signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_build_type_signals_keeps_namespace_less_tag_in_its_own_row() {
        let crate_name = CrateName::new("domain").expect("valid crate name");
        let root = ModulePath::root();
        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let mut index = TypeSignalIdentityIndex::default();
        for namespace in [CatalogueItemNamespace::Type, CatalogueItemNamespace::Trait] {
            add_entry_identity(
                &mut index,
                &crate_name,
                "domain::Shared",
                Some(&root),
                namespace,
                &rustdoc_paths,
            )
            .expect("shared identity resolves in both namespaces");
        }

        let kinds = BTreeMap::from([(
            "domain::Shared".to_owned(),
            vec!["value_object", "secondary_port", "free_function"],
        )]);
        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Trait,
                SignalRegion::SMinusC_Add,
            ),
            ThreeWaySignal::label(
                FreeText::new("domain::Shared"),
                SignalRegion::SIntersectC_Match_Add,
            ),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 3, "each kind tag must have one output row: {built:?}");
        assert!(built.iter().any(|signal| signal.kind_tag() == "value_object"));
        assert!(built.iter().any(|signal| signal.kind_tag() == "secondary_port"));
        assert!(built.iter().any(|signal| signal.kind_tag() == "free_function"));
        for pair in [("value_object", true), ("secondary_port", false), ("free_function", true)] {
            let row = built
                .iter()
                .find(|signal| signal.kind_tag() == pair.0)
                .expect("expected kind tag row");
            assert_eq!(row.found_type(), pair.1, "wrong accumulator for {}", pair.0);
        }
        assert_eq!(
            built
                .iter()
                .find(|signal| signal.kind_tag() == "value_object")
                .expect("type row")
                .signal(),
            domain::ConfidenceSignal::Blue
        );
        assert_eq!(
            built
                .iter()
                .find(|signal| signal.kind_tag() == "secondary_port")
                .expect("trait row")
                .signal(),
            domain::ConfidenceSignal::Yellow
        );
        assert_eq!(
            built
                .iter()
                .find(|signal| signal.kind_tag() == "free_function")
                .expect("function row")
                .identity()
                .namespace(),
            None
        );
        assert_eq!(
            built
                .iter()
                .find(|signal| signal.kind_tag() == "free_function")
                .expect("function row")
                .signal(),
            domain::ConfidenceSignal::Blue
        );
    }

    #[test]
    fn test_build_type_signals_preserves_namespace_less_function_label() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("domain::Shared", "Shared");
        let kinds = BTreeMap::from([("Shared".to_owned(), vec!["value_object"])]);
        let signals = vec![ThreeWaySignal::label(
            FreeText::new("domain::Shared"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let function_row = built
            .iter()
            .find(|signal| signal.type_name() == "domain::Shared")
            .expect("function label must retain its report spelling");
        assert_eq!(function_row.identity().namespace(), None);
        assert_eq!(function_row.kind_tag(), "unknown");
        assert_eq!(function_row.signal(), domain::ConfidenceSignal::Blue);
        assert!(built
            .iter()
            .any(|signal| signal.type_name() == "Shared" && signal.kind_tag() == "value_object"));
    }

    #[test]
    fn test_build_type_signals_preserves_raw_function_label_when_it_matches_catalogue_type() {
        use domain::ConfidenceSignal;

        let mut index = TypeSignalIdentityIndex::default();
        index.add_namespace("Shared", CatalogueItemNamespace::Type);
        let kinds = BTreeMap::from([("Shared".to_owned(), vec!["value_object"])]);
        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::label(FreeText::new("Shared"), SignalRegion::SMinusC_Add),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let function_row = built
            .iter()
            .find(|row| row.kind_tag() == "unknown" && row.identity().namespace().is_none())
            .expect("function label must remain a report-only row");
        assert_eq!(function_row.type_name(), "Shared");
        assert!(matches!(
            function_row.identity(),
            domain::tddd::signal_evaluator::ThreeWaySignalIdentity::Label { label }
                if label.as_str() == "Shared"
        ));
        assert_eq!(function_row.signal(), ConfidenceSignal::Yellow);

        let type_row =
            built.iter().find(|row| row.kind_tag() == "value_object").expect("catalogue type row");
        assert_eq!(type_row.identity().namespace(), Some(CatalogueItemNamespace::Type));
        assert!(type_row.found_type());
    }

    #[test]
    fn test_build_type_signals_namespace_less_label_does_not_join_declared_type() {
        use domain::ConfidenceSignal;

        let mut index = TypeSignalIdentityIndex::default();
        index.add_namespace("Shared", CatalogueItemNamespace::Type);
        let kinds = BTreeMap::from([("Shared".to_owned(), vec!["value_object"])]);
        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::label(FreeText::new("Shared"), SignalRegion::SMinusC_Add),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let declared_type = built
            .iter()
            .find(|signal| signal.kind_tag() == "value_object")
            .expect("declared type row remains present");
        assert_eq!(declared_type.signal(), ConfidenceSignal::Blue);
        assert!(declared_type.found_type());

        let report_label = built
            .iter()
            .find(|signal| signal.kind_tag() == "unknown")
            .expect("namespace-less report label remains present");
        assert_eq!(report_label.type_name(), "Shared");
        assert_eq!(report_label.signal(), ConfidenceSignal::Yellow);
        assert!(!report_label.found_type());
    }

    #[test]
    fn test_build_type_signals_retains_trait_impl_label_on_owner_row() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_namespace("Owner", CatalogueItemNamespace::Type);
        let kinds = BTreeMap::from([("Owner".to_owned(), vec!["value_object"])]);
        let signals = vec![ThreeWaySignal::label(
            FreeText::new("Owner: Trait"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1, "trait impl must join its owner row: {built:?}");
        assert_eq!(built[0].type_name(), "Owner");
        assert_eq!(built[0].kind_tag(), "value_object");
        assert_eq!(built[0].identity().namespace(), Some(CatalogueItemNamespace::Type));
        assert_eq!(built[0].found_items(), &["Trait"]);
    }

    #[test]
    fn test_build_identity_index_assigns_impl_owner_to_type_namespace_key() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        let root = ModulePath::root();
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Shared".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![] },
                vec![],
                vec![],
                vec![],
                Some(root.clone()),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("domain::Shared".to_owned()).expect("valid trait key"),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                Some(root),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("SomeTrait").expect("valid trait ref"),
            TypeRef::new("Shared").expect("valid type ref"),
        ));
        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let index = build_type_signal_identity_index(&catalogue, &rustdoc_paths)
            .expect("type-signal identities resolve in their namespaces");
        let kinds = BTreeMap::from([
            ("Shared".to_owned(), vec!["value_object"]),
            ("domain::Shared".to_owned(), vec!["secondary_port"]),
        ]);
        let signals = vec![ThreeWaySignal::label(
            FreeText::new("domain::Shared: SomeTrait"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let type_row = built
            .iter()
            .find(|signal| signal.type_name() == "Shared")
            .expect("type entry remains the impl owner");
        assert_eq!(type_row.found_items(), &["SomeTrait"]);
        let trait_row = built
            .iter()
            .find(|signal| signal.type_name() == "domain::Shared")
            .expect("trait entry remains separate from the type owner");
        assert!(trait_row.found_items().is_empty());
    }

    #[test]
    fn test_build_type_signals_preserves_namespace_less_function_key_in_fill_pass() {
        let crate_name = CrateName::new("domain").expect("valid crate name");
        let root = ModulePath::root();
        let rustdoc_paths = HashMap::from([(
            Id(1),
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_owned(), "Shared".to_owned()],
                kind: ItemKind::Struct,
            },
        )]);
        let mut index = TypeSignalIdentityIndex::default();
        add_entry_identity(
            &mut index,
            &crate_name,
            "Shared",
            Some(&root),
            CatalogueItemNamespace::Type,
            &rustdoc_paths,
        )
        .expect("type identity resolves");

        let kinds = BTreeMap::from([
            ("Shared".to_owned(), vec!["value_object"]),
            ("domain::Shared".to_owned(), vec!["free_function"]),
        ]);

        let built = build_type_signals_from_report(std::iter::empty(), &kinds, &index);

        assert_eq!(built.len(), 2, "function fill row must not join the type row: {built:?}");
        assert!(
            built.iter().any(|signal| signal.type_name() == "domain::Shared"
                && signal.kind_tag() == "free_function")
        );
        assert!(built
            .iter()
            .any(|signal| signal.type_name() == "Shared" && signal.kind_tag() == "value_object"));
    }

    #[test]
    fn test_build_type_signals_joins_short_impl_owner_to_qualified_entry() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias(
            "CatalogueLinterError",
            "domain::tddd::catalogue_linter::CatalogueLinterError",
        );

        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert(
            "domain::tddd::catalogue_linter::CatalogueLinterError".to_owned(),
            vec!["error_type"],
        );
        let signals = vec![ThreeWaySignal::label(
            FreeText::new("CatalogueLinterError: From<TypeRefPathExtractionError>"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "domain::tddd::catalogue_linter::CatalogueLinterError");
        assert_eq!(built[0].found_items(), &["From<TypeRefPathExtractionError>"]);
    }

    #[test]
    fn test_build_type_signals_joins_external_impl_owner_to_for_type_key() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_impl_alias("Arc", "std::sync::Arc<T>");

        let signals = vec![ThreeWaySignal::label(
            FreeText::new("Arc: TypeRefPathExtractorPort"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(
            signals.iter(),
            &std::collections::BTreeMap::new(),
            &index,
        );

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "std::sync::Arc<T>");
        assert_eq!(built[0].identity().namespace(), None);
    }

    #[test]
    fn test_build_identity_index_joins_generic_impl_owner_to_declaration_identity() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;
        use domain::tddd::catalogue_v2::roles::DataRole;
        use domain::tddd::catalogue_v2::{ParamName, TraitImplDeclV2, TypeRef};
        use rustdoc_types::{Id, ItemKind, ItemSummary};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::Wrapper".to_owned())
                .expect("valid declaration key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(
                    ModulePath::from_segments(vec!["alpha".to_owned()]).expect("valid module path"),
                ),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("domain::ports::Port".to_owned()).expect("valid trait ref"),
            TypeRef::new("domain::alpha::Wrapper<T>".to_owned()).expect("valid impl owner"),
            vec![MethodGenericParam {
                name: ParamName::new("T").expect("valid generic parameter"),
                bounds: vec![],
            }],
            vec![],
        ));

        let rustdoc_paths = HashMap::from([(
            Id(1),
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_owned(), "alpha".to_owned(), "Wrapper".to_owned()],
                kind: ItemKind::Struct,
            },
        )]);
        let index = build_type_signal_identity_index(&catalogue, &rustdoc_paths)
            .expect("generic impl owner identity indexes successfully");

        let signals = vec![ThreeWaySignal::label(
            FreeText::new("domain::alpha::Wrapper<T>: domain::ports::Port"),
            SignalRegion::SIntersectC_Match_Add,
        )];
        let kinds = BTreeMap::from([("domain::alpha::Wrapper".to_owned(), vec!["struct"])]);
        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "domain::alpha::Wrapper");
        assert_eq!(built[0].kind_tag(), "struct");
        assert_eq!(built[0].found_items(), &["domain::ports::Port"]);
    }

    #[test]
    fn test_build_type_signals_duplicate_module_impl_owners_join_independently() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;
        use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};
        use rustdoc_types::{Id, ItemKind, ItemSummary};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        for module in ["alpha", "beta"] {
            let module_path =
                ModulePath::from_segments(vec![module.to_owned()]).expect("valid module path");
            catalogue.insert_type(
                CatalogueEntryKey::try_new(format!("{module}::Input")).expect("valid type key"),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                    vec![],
                    vec![],
                    vec![],
                    Some(module_path.clone()),
                    None,
                    vec![],
                    vec![],
                ),
            );
            catalogue.insert_trait(
                CatalogueEntryKey::try_new(format!("{module}::Port")).expect("valid trait key"),
                TraitEntry::new(
                    ItemAction::Add,
                    ContractRole::SpecificationPort,
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    Some(module_path),
                    None,
                    vec![],
                    vec![],
                ),
            );
            catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
                ItemAction::Add,
                TypeRef::new(format!("domain::{module}::Port<domain::{module}::Input>"))
                    .expect("valid qualified trait ref"),
                TypeRef::new(format!("domain::{module}::Input<T>")).expect("valid generic owner"),
                vec![MethodGenericParam {
                    name: ParamName::new("T").expect("valid generic name"),
                    bounds: vec![],
                }],
                vec![],
            ));
        }

        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "alpha".to_owned(), "Input".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "beta".to_owned(), "Input".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(3),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "alpha".to_owned(), "Port".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
            (
                Id(4),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "beta".to_owned(), "Port".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let identity_index = build_type_signal_identity_index(&catalogue, &rustdoc_paths)
            .expect("duplicate module identities must be indexable");
        let signals = [
            ThreeWaySignal::label(
                FreeText::new("domain::alpha::Input: domain::alpha::Port<domain::alpha::Input>"),
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::label(
                FreeText::new("domain::beta::Input: domain::beta::Port<domain::beta::Input>"),
                SignalRegion::SIntersectC_Match_Add,
            ),
        ];
        let kinds = BTreeMap::from([
            ("alpha::Input".to_owned(), vec!["struct"]),
            ("beta::Input".to_owned(), vec!["struct"]),
        ]);

        let built = build_type_signals_from_report(signals.iter(), &kinds, &identity_index);
        assert_eq!(built.len(), 2, "each qualified owner must produce one signal");
        for (module, other_module) in [("alpha", "beta"), ("beta", "alpha")] {
            let signal = built
                .iter()
                .find(|signal| signal.type_name() == format!("{module}::Input"))
                .expect("qualified owner must remain a separate entry");
            assert_eq!(signal.signal(), domain::ConfidenceSignal::Blue);
            assert_eq!(
                signal.found_items(),
                &[format!("domain::{module}::Port<domain::{module}::Input>")]
            );
            assert!(
                signal
                    .found_items()
                    .iter()
                    .all(|item| !item.contains(&format!("domain::{other_module}::"))),
                "{module} owner must not receive {other_module} impl data: {:?}",
                signal.found_items()
            );
        }
    }

    #[test]
    fn test_build_type_signals_plain_declaration_ignores_external_impl_alias() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("Shared", "domain::alpha::Shared");
        index.add_namespace("domain::alpha::Shared", CatalogueItemNamespace::Type);
        index.add_impl_alias("Shared", "external::Shared");
        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::alpha::Shared".to_owned(), vec!["struct"]);
        let signals = vec![ThreeWaySignal::catalogue_item(
            FreeText::new("Shared"),
            CatalogueItemNamespace::Type,
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "domain::alpha::Shared");
        assert_eq!(built[0].signal(), domain::ConfidenceSignal::Blue);
    }

    #[test]
    fn test_build_type_signals_prefers_exact_impl_owner_over_declaration_alias() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("other::Thing", "domain::other::Thing");
        index.add_impl_alias("other::Thing", "other::Thing");

        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::other::Thing".to_owned(), vec!["struct"]);
        let signals = vec![ThreeWaySignal::label(
            FreeText::new("other::Thing: LocalTrait"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let local = built
            .iter()
            .find(|signal| signal.type_name() == "domain::other::Thing")
            .expect("declared local entry remains in the output");
        assert!(local.found_items().is_empty());
        let external = built
            .iter()
            .find(|signal| signal.type_name() == "other::Thing")
            .expect("exact impl owner receives the impl signal");
        assert_eq!(external.found_items(), ["LocalTrait"]);
    }

    #[test]
    fn test_build_type_signals_marks_ambiguous_short_impl_owner_yellow_for_each_candidate() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("Shared", "domain::alpha::Shared");
        index.add_alias("Shared", "domain::beta::Shared");
        index.add_namespace("domain::alpha::Shared", CatalogueItemNamespace::Type);
        index.add_namespace("domain::beta::Shared", CatalogueItemNamespace::Type);

        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::alpha::Shared".to_owned(), vec!["struct"]);
        kinds.insert("domain::beta::Shared".to_owned(), vec!["struct"]);
        let signals = vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("domain::alpha::Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::catalogue_item(
                FreeText::new("domain::beta::Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::label(
                FreeText::new("Shared: Clone"),
                SignalRegion::SIntersectC_Match_Add,
            ),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 2);
        assert!(built.iter().all(|signal| signal.signal() == domain::ConfidenceSignal::Yellow));
        assert!(built.iter().all(|signal| signal.found_items() == ["Clone"]));
    }

    #[test]
    fn test_build_type_signals_does_not_short_fallback_for_unknown_qualified_owner() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("Shared", "domain::alpha::Shared");
        index.add_alias("Shared", "domain::beta::Shared");

        let signals = vec![ThreeWaySignal::label(
            FreeText::new("domain::gamma::Shared: Clone"),
            SignalRegion::SIntersectC_Match_Add,
        )];
        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::alpha::Shared".to_owned(), vec!["struct"]);
        kinds.insert("domain::beta::Shared".to_owned(), vec!["struct"]);

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let gamma = built
            .iter()
            .find(|signal| signal.type_name() == "domain::gamma::Shared")
            .expect("unknown qualified owner remains a distinct signal");
        assert_eq!(gamma.found_items(), ["Clone"]);
        assert!(
            built
                .iter()
                .filter(|signal| {
                    matches!(signal.type_name(), "domain::alpha::Shared" | "domain::beta::Shared")
                })
                .all(|signal| signal.found_items().is_empty())
        );
    }

    #[test]
    fn test_build_type_signals_nested_supertrait_joins_qualified_kind_alias_to_stored_entry() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("usecase::chain::traits::SoTChain", "SoTChain");
        let kinds = BTreeMap::from([(
            "usecase::chain::traits::SoTChain".to_owned(),
            vec!["secondary_port"],
        )]);
        let signals = vec![ThreeWaySignal::label(
            FreeText::new("usecase::chain::traits::SoTChain: ChainIdentity"),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1, "{built:?}");
        assert_eq!(built[0].type_name(), "SoTChain");
        assert_eq!(built[0].kind_tag(), "secondary_port");
        assert_eq!(built[0].signal(), domain::ConfidenceSignal::Blue);
        assert_eq!(built[0].found_items(), &["ChainIdentity"]);
    }

    #[test]
    fn test_build_identity_index_rejects_add_entry_absent_from_resolution_set() {
        let mut index = TypeSignalIdentityIndex::default();
        let crate_name = CrateName::new("domain").expect("valid crate name");

        let error = add_entry_identity(
            &mut index,
            &crate_name,
            "domain::new::Added",
            None,
            CatalogueItemNamespace::Type,
            &HashMap::new(),
        )
        .expect_err("an add absent from the shared resolution set must fail closed");

        assert!(error.contains("cannot canonicalize catalogue entry"));
    }

    #[test]
    fn test_build_identity_index_resolves_impl_owner_in_type_namespace() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Shared".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![] },
                vec![],
                vec![],
                vec![],
                Some(ModulePath::root()),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("Shared".to_owned()).expect("valid trait key"),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                Some(ModulePath::root()),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("SomeTrait").expect("valid trait reference"),
            TypeRef::new("Shared").expect("valid type reference"),
        ));

        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "Shared".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);

        build_type_signal_identity_index(&catalogue, &rustdoc_paths)
            .expect("for_type must resolve in the type namespace only");
    }

    #[test]
    fn test_build_identity_index_includes_type_and_trait_deletion_aliases() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.push_deletion(DeletionRecord::Type {
            name: CatalogueEntryKey::try_new("RemovedType".to_owned()).expect("valid key"),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        catalogue.push_deletion(DeletionRecord::Trait {
            name: CatalogueEntryKey::try_new("domain::old::RemovedTrait".to_owned())
                .expect("valid key"),
            spec_refs: vec![],
            informal_grounds: vec![],
        });

        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "RemovedType".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "old".to_owned(), "RemovedTrait".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let index = build_type_signal_identity_index(&catalogue, &rustdoc_paths)
            .expect("deletion identities remain indexable");

        assert_eq!(
            index.declaration_candidates("RemovedType"),
            Some(vec!["RemovedType".to_owned()])
        );
        assert_eq!(
            index.declaration_candidates("domain::old::RemovedTrait"),
            Some(vec!["domain::old::RemovedTrait".to_owned()])
        );
    }

    #[test]
    fn test_add_deletion_identity_uses_authoritative_catalogue_key() {
        let mut index = TypeSignalIdentityIndex::default();
        let crate_name = CrateName::new("infrastructure").expect("valid crate name");

        let rustdoc_paths = HashMap::from([(
            Id(1),
            ItemSummary {
                crate_id: 0,
                path: vec![
                    "infrastructure".to_owned(),
                    "CatalogueToExtendedCrateCodecError".to_owned(),
                ],
                kind: ItemKind::Struct,
            },
        )]);
        add_deletion_identity(
            &mut index,
            &crate_name,
            "CatalogueToExtendedCrateCodecError",
            CatalogueItemNamespace::Type,
            &rustdoc_paths,
        )
        .expect("deletion key resolves through the shared resolution set");

        assert_eq!(
            index.declaration_candidates("CatalogueToExtendedCrateCodecError"),
            Some(vec!["CatalogueToExtendedCrateCodecError".to_owned()])
        );
    }
}
