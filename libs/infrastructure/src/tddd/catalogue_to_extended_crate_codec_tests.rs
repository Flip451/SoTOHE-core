//! Tests for [`catalogue_to_extended_crate_codec`] (split out to keep the main module under the 200-400 line guideline).

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
use domain::tddd::catalogue_v2::entries::{AssocConstDecl, AssocTypeDecl, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, ParamDeclaration,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction, SelfReceiver};
use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
use domain::tddd::catalogue_v2::{
    AssocConstName, BoundOp, CatalogueDocument, CatalogueEntryKey, CrateName, DeletionRecord,
    FieldName, FunctionName, FunctionPath, MethodName, ModulePath, ParamName, TypeName, TypeRef,
    VariantName, WherePredicateDecl,
};
use domain::tddd::{CatalogueToExtendedCratePort, NewTypeGraphCodecError, SignalEvaluatorPort};
use rustdoc_types::{
    AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Id,
    ItemEnum, ItemKind, ItemSummary, Term, TraitBoundModifier, Type, VariantKind, WherePredicate,
};

use super::*;
use crate::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use crate::tddd::type_ref_parser::{STD_PRELUDE_TYPES, UNRESOLVED_CRATE_ID, std_canonical_path};

fn make_doc(crate_name: &str) -> CatalogueDocument {
    CatalogueDocument::new(
        2,
        CrateName::new(crate_name).unwrap(),
        LayerId::try_new("domain").expect("static valid"),
    )
}

fn insert_empty_enum_type(doc: &mut CatalogueDocument, name: &str) {
    doc.insert_type(
        CatalogueEntryKey::try_new(name.to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
}

fn authoritative_crate_for_doc(doc: &CatalogueDocument) -> rustdoc_types::Crate {
    let mut paths = std::collections::HashMap::new();
    let mut known_paths = std::collections::HashSet::new();
    let mut next_id = 1;
    let mut add_path = |path: Vec<String>, kind: ItemKind| {
        if known_paths.insert((path.clone(), kind)) {
            paths.insert(Id(next_id), ItemSummary { crate_id: 0, path, kind });
            next_id += 1;
        }
    };
    let entry_path = |key: &CatalogueEntryKey, module_path: &ModulePath| {
        let mut path = vec![doc.crate_name().as_str().to_owned()];
        path.extend(module_path.segments().iter().map(|segment| segment.as_str().to_owned()));
        path.push(key.as_str().rsplit("::").next().unwrap_or(key.as_str()).to_owned());
        path
    };

    for (key, entry) in doc.types() {
        add_path(entry_path(key, entry.module_path()), ItemKind::Struct);
    }
    for (key, entry) in doc.traits() {
        add_path(entry_path(key, entry.module_path()), ItemKind::Trait);
    }
    for deletion in doc.deletions() {
        let (name, kind) = match deletion {
            DeletionRecord::Type { name, .. } => (name, ItemKind::Struct),
            DeletionRecord::Trait { name, .. } => (name, ItemKind::Trait),
            DeletionRecord::Function { .. } => continue,
        };
        let mut path = name.as_str().split("::").map(str::to_owned).collect::<Vec<_>>();
        if path.first().map(String::as_str) != Some(doc.crate_name().as_str()) {
            path.insert(0, doc.crate_name().as_str().to_owned());
        }
        add_path(path, kind);
    }

    // The production codec receives these paths from the evaluator's
    // baseline/current rustdoc crates. Keep the fixture explicit about the
    // external items used by the codec tests so that removing a path from this
    // list exercises the same fail-closed behaviour as a missing rustdoc item.
    let prelude_traits = [
        "Default",
        "Clone",
        "Copy",
        "Debug",
        "Display",
        "PartialEq",
        "Eq",
        "Hash",
        "Ord",
        "PartialOrd",
        "Send",
        "Sync",
        "Sized",
        "Unpin",
        "Drop",
        "AsRef",
        "AsMut",
        "Deref",
        "DerefMut",
        "From",
        "Into",
        "TryFrom",
        "TryInto",
        "IntoIterator",
        "DoubleEndedIterator",
        "ExactSizeIterator",
        "FnOnce",
        "FnMut",
        "Fn",
        "ToString",
        "ToOwned",
        "BorrowMut",
        "Borrow",
    ];
    for short_name in STD_PRELUDE_TYPES {
        let path =
            std_canonical_path(short_name).split("::").map(str::to_owned).collect::<Vec<_>>();
        let kind =
            if prelude_traits.contains(short_name) { ItemKind::Trait } else { ItemKind::Struct };
        add_path(path, kind);
    }
    for (path, kind) in [
        (vec!["core", "convert", "From"], ItemKind::Trait),
        (vec!["serde", "Serialize"], ItemKind::Trait),
        (vec!["ext", "Foo"], ItemKind::Struct),
        (vec!["ext", "Trait"], ItemKind::Trait),
    ] {
        add_path(path.into_iter().map(str::to_owned).collect(), kind);
    }

    rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths,
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    }
}

fn encode_doc(doc: CatalogueDocument) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
    let baseline = authoritative_crate_for_doc(&doc);
    let current = baseline.clone();
    CatalogueToExtendedCrateCodec::new().encode(doc, &baseline, &current)
}

fn item_id_for_path(ec: &domain::tddd::ExtendedCrate, path: &[&str]) -> Id {
    let expected: Vec<String> = path.iter().map(|segment| (*segment).to_owned()).collect();
    ec.krate()
        .paths
        .iter()
        .find(|(_, summary)| summary.path == expected)
        .map(|(id, _)| *id)
        .expect("path should be present in encoded crate")
}

#[test]
fn test_authoritative_paths_deduplicates_same_identity_with_independent_ids() {
    let path = vec!["domain".to_owned(), "entries".to_owned(), "TypeEntry".to_owned()];
    let summary =
        |id: u32| (Id(id), ItemSummary { crate_id: 0, path: path.clone(), kind: ItemKind::Struct });
    let baseline = rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths: [summary(1)].into_iter().collect(),
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    };
    let current = rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths: [summary(2)].into_iter().collect(),
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    };

    let paths = authoritative_paths(&baseline, &current);

    assert_eq!(paths.len(), 1);
    assert_eq!(paths.values().next().map(|item| &item.path), Some(&path));
}

#[test]
fn test_authoritative_paths_keeps_same_path_in_distinct_namespaces() {
    let path = vec!["domain".to_owned(), "Thing".to_owned()];
    let summary =
        |id: u32, kind: ItemKind| (Id(id), ItemSummary { crate_id: 0, path: path.clone(), kind });
    let baseline = rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths: [summary(1, ItemKind::Function)].into_iter().collect(),
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    };
    let current = rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths: [summary(2, ItemKind::Struct)].into_iter().collect(),
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    };

    let paths = authoritative_paths(&baseline, &current);

    assert_eq!(paths.len(), 2);
    assert!(paths.values().any(|summary| summary.kind == ItemKind::Function));
    assert!(paths.values().any(|summary| summary.kind == ItemKind::Struct));
}

#[test]
fn test_authoritative_paths_deduplicates_type_kind_changes() {
    let path = vec!["domain".to_owned(), "Thing".to_owned()];
    let make_crate = |id: u32, kind: ItemKind| rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths: [(Id(id), ItemSummary { crate_id: 0, path: path.clone(), kind })]
            .into_iter()
            .collect(),
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    };

    let paths =
        authoritative_paths(&make_crate(1, ItemKind::Struct), &make_crate(2, ItemKind::Enum));

    assert_eq!(paths.len(), 1);
    assert_eq!(paths.values().next().map(|summary| summary.kind), Some(ItemKind::Struct));
}

// -----------------------------------------------------------------------
// Delete tombstones
// -----------------------------------------------------------------------

#[test]
fn test_encode_type_deletion_record_emits_delete_action() {
    let mut doc = make_doc("domain");
    doc.push_deletion(DeletionRecord::Type {
        name: CatalogueEntryKey::try_new("OldType".to_owned()).unwrap(),
        spec_refs: vec![],
        informal_grounds: vec![],
    });

    let ec = encode_doc(doc).unwrap();
    let id = item_id_for_path(&ec, &["domain", "OldType"]);

    assert_eq!(ec.action_for(&id), Some(ItemAction::Delete));
    assert_eq!(ec.krate().paths[&id].kind, ItemKind::Struct);
    assert!(matches!(ec.krate().index[&id].inner, ItemEnum::Struct(_)));
}

#[test]
fn test_encode_trait_deletion_record_emits_delete_action() {
    let mut doc = make_doc("domain");
    doc.push_deletion(DeletionRecord::Trait {
        name: CatalogueEntryKey::try_new("OldPort".to_owned()).unwrap(),
        spec_refs: vec![],
        informal_grounds: vec![],
    });

    let ec = encode_doc(doc).unwrap();
    let id = item_id_for_path(&ec, &["domain", "OldPort"]);

    assert_eq!(ec.action_for(&id), Some(ItemAction::Delete));
    assert_eq!(ec.krate().paths[&id].kind, ItemKind::Trait);
    assert!(matches!(ec.krate().index[&id].inner, ItemEnum::Trait(_)));
}

#[test]
fn test_encode_function_deletion_record_emits_delete_action() {
    let mut doc = make_doc("domain");
    let path = FunctionPath::at_root(
        CrateName::new("domain").unwrap(),
        FunctionName::new("old_fn").unwrap(),
    );
    doc.push_deletion(DeletionRecord::Function {
        path,
        spec_refs: vec![],
        informal_grounds: vec![],
    });

    let ec = encode_doc(doc).unwrap();
    let id = item_id_for_path(&ec, &["domain", "old_fn"]);

    assert_eq!(ec.action_for(&id), Some(ItemAction::Delete));
    assert_eq!(ec.krate().paths[&id].kind, ItemKind::Function);
    assert!(matches!(ec.krate().index[&id].inner, ItemEnum::Function(_)));
}

// -----------------------------------------------------------------------
// Error path: AmbiguousIdentifier
// -----------------------------------------------------------------------

#[test]
fn test_encode_returns_ambiguous_identifier_when_type_and_trait_share_name() {
    // A type named "Foo" and a trait named "Foo" in the same catalogue collide
    // in the short-name index, triggering AmbiguousIdentifier.
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let result = encode_doc(doc);
    assert!(result.is_err(), "expected error due to name collision between type Foo and trait Foo");
    // The domain error reports the ambiguous identifier and its candidates.
    let err = result.unwrap_err();
    assert!(
        matches!(err, domain::tddd::NewTypeGraphCodecError::AmbiguousIdentifier(_, _)),
        "expected AmbiguousIdentifier error, got: {err:?}"
    );
}

#[test]
fn test_encode_same_name_type_and_trait_preserves_distinct_evaluator_identities() {
    // One catalogue may contain a type and a trait with the same short name when their
    // declaration modules differ. The encoder must retain both full paths, and the evaluator's
    // identity map must keep them as separate targets rather than collapsing by short name.
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("alpha::SharedName".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["alpha".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("beta::SharedName".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["beta".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    let type_id = item_id_for_path(&encoded, &["domain", "alpha", "SharedName"]);
    let trait_id = item_id_for_path(&encoded, &["domain", "beta", "SharedName"]);
    assert_ne!(type_id, trait_id);

    let identities =
        crate::tddd::signal_evaluator_v2::build_type_trait_identity_map(encoded.krate()).unwrap();
    assert_eq!(identities.get("domain::alpha::SharedName"), Some(&type_id));
    assert_eq!(identities.get("domain::beta::SharedName"), Some(&trait_id));
    assert_eq!(identities.len(), 2, "the evaluator must retain both full-path identities");
}

#[test]
fn test_encode_unique_short_names_resolve_incrate_type_and_trait_refs_to_qualified_paths() {
    // Declarations stay in their normal short-name form. A unique in-crate TypeRef and a
    // unique short trait_ref must resolve to the corresponding module-qualified catalogue
    // entries, with the rustdoc paths map remaining the identity authority.
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("UniqueType".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["alpha".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("UniqueTrait".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["beta".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Implementor".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        TypeRef::new("UniqueType").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("UniqueTrait").unwrap(),
        TypeRef::new("Implementor").unwrap(),
    ));

    let encoded = encode_doc(doc).unwrap();
    let unique_type_id = item_id_for_path(&encoded, &["domain", "alpha", "UniqueType"]);
    let unique_trait_id = item_id_for_path(&encoded, &["domain", "beta", "UniqueTrait"]);
    let holder_id = item_id_for_path(&encoded, &["domain", "Holder"]);

    let ItemEnum::Struct(holder) = &encoded.krate().index[&holder_id].inner else {
        panic!("expected Holder struct");
    };
    let rustdoc_types::StructKind::Plain { fields, .. } = &holder.kind else {
        panic!("expected named Holder fields");
    };
    let ItemEnum::StructField(Type::ResolvedPath(field_path)) =
        &encoded.krate().index[&fields[0]].inner
    else {
        panic!("expected Holder.value to be a resolved local TypeRef");
    };
    assert_eq!(field_path.id, unique_type_id);
    assert_eq!(encoded.krate().paths[&field_path.id].path, ["domain", "alpha", "UniqueType"]);

    let trait_impl = encoded.krate().index.values().find_map(|item| {
        let ItemEnum::Impl(impl_item) = &item.inner else {
            return None;
        };
        impl_item
            .trait_
            .as_ref()
            .filter(|trait_path| trait_path.id == unique_trait_id)
            .map(|trait_path| (trait_path, &impl_item.for_))
    });
    let Some((trait_path, Type::ResolvedPath(for_path))) = trait_impl else {
        panic!("expected impl UniqueTrait for Implementor");
    };
    assert_eq!(trait_path.id, unique_trait_id);
    assert_eq!(encoded.krate().paths[&trait_path.id].path, ["domain", "beta", "UniqueTrait"]);
    assert_eq!(for_path.path, "Implementor");
}

#[test]
fn test_encode_same_short_name_in_different_modules_resolves_qualified_paths() {
    let mut doc = make_doc("domain");
    for (key, module) in [("a::Input", "a"), ("b::Input", "b")] {
        doc.insert_type(
            CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![] },
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
    }
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("left").unwrap(),
                            TypeRef::new("domain::a::Input").unwrap(),
                        ),
                        FieldDecl::new(
                            FieldName::new("right").unwrap(),
                            TypeRef::new("domain::b::Input").unwrap(),
                        ),
                    ],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    assert!(!encoded.krate().external_crates.values().any(|external| external.name == "domain"));
    let left_id = item_id_for_path(&encoded, &["domain", "a", "Input"]);
    let right_id = item_id_for_path(&encoded, &["domain", "b", "Input"]);
    assert_ne!(left_id, right_id);

    let holder_id = item_id_for_path(&encoded, &["domain", "Holder"]);
    let ItemEnum::Struct(holder) = &encoded.krate().index[&holder_id].inner else {
        panic!("expected Holder struct");
    };
    let rustdoc_types::StructKind::Plain { fields, .. } = &holder.kind else {
        panic!("expected named Holder fields");
    };
    let field_types =
        fields.iter().map(|field_id| &encoded.krate().index[field_id].inner).collect::<Vec<_>>();
    assert!(
        matches!(field_types[0], ItemEnum::StructField(Type::ResolvedPath(path)) if path.id == left_id)
    );
    assert!(
        matches!(field_types[1], ItemEnum::StructField(Type::ResolvedPath(path)) if path.id == right_id)
    );
}

#[test]
fn test_encode_duplicate_module_type_and_trait_impl_references_preserve_each_qualified_identity() {
    use domain::tddd::catalogue_v2::entries::InherentImplDeclV2;
    use domain::tddd::catalogue_v2::traits::TraitRefScope;

    let mut doc = make_doc("domain");
    let unit_type = |module: &str| {
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        )
    };
    let port_trait = |module: &str| {
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        )
    };

    doc.insert_type(
        CatalogueEntryKey::try_new("alpha::Input".to_owned()).unwrap(),
        unit_type("alpha"),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("beta::Input".to_owned()).unwrap(),
        unit_type("beta"),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("alpha::Port".to_owned()).unwrap(),
        port_trait("alpha"),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("beta::Port".to_owned()).unwrap(),
        port_trait("beta"),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("alpha_input").unwrap(),
                            TypeRef::new("domain::alpha::Input").unwrap(),
                        ),
                        FieldDecl::new(
                            FieldName::new("beta_input").unwrap(),
                            TypeRef::new("domain::beta::Input").unwrap(),
                        ),
                    ],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("domain::alpha::Port<domain::alpha::Input>").unwrap(),
        TypeRef::new("domain::alpha::Input").unwrap(),
    ));
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("domain::beta::Port<domain::beta::Input>").unwrap(),
        TypeRef::new("domain::beta::Input").unwrap(),
    ));
    doc.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("alpha::Input".to_owned()).unwrap(),
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![],
        vec![],
    ));
    doc.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("beta::Input".to_owned()).unwrap(),
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![],
        vec![],
    ));

    let encoded = encode_doc(doc).expect("duplicate-module references must encode");
    let alpha_id = item_id_for_path(&encoded, &["domain", "alpha", "Input"]);
    let beta_id = item_id_for_path(&encoded, &["domain", "beta", "Input"]);
    let alpha_port_id = item_id_for_path(&encoded, &["domain", "alpha", "Port"]);
    let beta_port_id = item_id_for_path(&encoded, &["domain", "beta", "Port"]);
    assert_ne!(alpha_id, beta_id);
    assert_ne!(alpha_port_id, beta_port_id);
    assert_eq!(encoded.krate().paths[&alpha_id].path, ["domain", "alpha", "Input"]);
    assert_eq!(encoded.krate().paths[&beta_id].path, ["domain", "beta", "Input"]);
    assert_eq!(encoded.krate().paths[&alpha_port_id].path, ["domain", "alpha", "Port"]);
    assert_eq!(encoded.krate().paths[&beta_port_id].path, ["domain", "beta", "Port"]);
    assert_ne!(alpha_id, Id(UNRESOLVED_CRATE_ID));
    assert_ne!(beta_id, Id(UNRESOLVED_CRATE_ID));
    assert_ne!(alpha_port_id, Id(UNRESOLVED_CRATE_ID));
    assert_ne!(beta_port_id, Id(UNRESOLVED_CRATE_ID));

    let identities =
        crate::tddd::signal_evaluator_v2::build_type_trait_identity_map(encoded.krate())
            .expect("type and trait identities must be indexed by fully qualified path");
    assert_eq!(identities.get("domain::alpha::Port"), Some(&alpha_port_id));
    assert_eq!(identities.get("domain::beta::Port"), Some(&beta_port_id));

    let alpha_scope = TraitImplDeclV2::new(
        TypeRef::new("domain::alpha::Port").unwrap(),
        TypeRef::new("domain::alpha::Input").unwrap(),
    )
    .trait_ref_scope();
    let beta_scope = TraitImplDeclV2::new(
        TypeRef::new("domain::beta::Port").unwrap(),
        TypeRef::new("domain::beta::Input").unwrap(),
    )
    .trait_ref_scope();
    let TraitRefScope::Workspace(alpha_scope_key) = alpha_scope else {
        panic!("expected alpha trait reference to retain workspace scope");
    };
    let TraitRefScope::Workspace(beta_scope_key) = beta_scope else {
        panic!("expected beta trait reference to retain workspace scope");
    };
    assert_ne!(alpha_scope_key, beta_scope_key);
    assert_eq!(
        identities.get(alpha_scope_key.as_str()),
        Some(&alpha_port_id),
        "alpha trait scope must resolve to its fully qualified rustdoc identity"
    );
    assert_eq!(
        identities.get(beta_scope_key.as_str()),
        Some(&beta_port_id),
        "beta trait scope must resolve to its fully qualified rustdoc identity"
    );

    let holder_id = item_id_for_path(&encoded, &["domain", "Holder"]);
    let ItemEnum::Struct(holder) = &encoded.krate().index[&holder_id].inner else {
        panic!("expected Holder struct");
    };
    let rustdoc_types::StructKind::Plain { fields, .. } = &holder.kind else {
        panic!("expected Holder fields");
    };
    let field_type_ids = fields
        .iter()
        .map(|field_id| match &encoded.krate().index[field_id].inner {
            ItemEnum::StructField(Type::ResolvedPath(path)) => path.id,
            other => panic!("expected resolved in-crate TypeRef, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(field_type_ids, vec![alpha_id, beta_id]);
    assert!(field_type_ids.iter().all(|id| *id != Id(UNRESOLVED_CRATE_ID)));

    let trait_impl_records = encoded
        .krate()
        .index
        .values()
        .filter_map(|item| {
            let ItemEnum::Impl(impl_item) = &item.inner else {
                return None;
            };
            let trait_path = impl_item.trait_.as_ref()?;
            let GenericArgs::AngleBracketed { args, .. } = trait_path.args.as_deref()? else {
                return None;
            };
            let argument_id = args.iter().find_map(|arg| match arg {
                GenericArg::Type(Type::ResolvedPath(path)) => Some(path.id),
                _ => None,
            })?;
            let Type::ResolvedPath(for_path) = &impl_item.for_ else {
                return None;
            };
            Some((trait_path.path.clone(), trait_path.id, argument_id, for_path.id))
        })
        .collect::<Vec<_>>();
    assert_eq!(trait_impl_records.len(), 2, "both trait_impls must remain distinct");
    assert!(trait_impl_records.contains(&(
        "domain::alpha::Port".to_owned(),
        alpha_port_id,
        alpha_id,
        alpha_id,
    )));
    assert!(trait_impl_records.contains(&(
        "domain::beta::Port".to_owned(),
        beta_port_id,
        beta_id,
        beta_id,
    )));
    for (trait_ref_path, trait_id, argument_id, for_id) in &trait_impl_records {
        assert_ne!(*trait_id, Id(UNRESOLVED_CRATE_ID));
        assert_ne!(*argument_id, Id(UNRESOLVED_CRATE_ID));
        assert_ne!(*for_id, Id(UNRESOLVED_CRATE_ID));
        assert_eq!(
            encoded.krate().paths[trait_id].path,
            trait_ref_path.split("::").map(str::to_owned).collect::<Vec<_>>(),
            "emitted trait_ref ID must resolve to its fully qualified rustdoc path"
        );
    }

    let inherent_impl_for_ids = encoded
        .krate()
        .index
        .values()
        .filter_map(|item| {
            let ItemEnum::Impl(impl_item) = &item.inner else {
                return None;
            };
            if impl_item.trait_.is_some() || impl_item.generics.params.is_empty() {
                return None;
            }
            let Type::ResolvedPath(for_path) = &impl_item.for_ else {
                return None;
            };
            Some(for_path.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(inherent_impl_for_ids.len(), 2);
    assert!(inherent_impl_for_ids.contains(&alpha_id));
    assert!(inherent_impl_for_ids.contains(&beta_id));
    assert!(inherent_impl_for_ids.iter().all(|id| *id != Id(UNRESOLVED_CRATE_ID)));
}

#[test]
fn test_encode_loose_type_and_trait_deletion_names_with_module_paths_preserve_distinct_qualified_identities()
 {
    use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;

    let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "alpha::GoneType": { "action": "delete", "module_path": "alpha" },
    "beta::GoneType": { "action": "delete", "module_path": "beta" }
  },
  "traits": {
    "alpha::GoneTrait": { "action": "delete", "module_path": "alpha" },
    "beta::GoneTrait": { "action": "delete", "module_path": "beta" }
  },
  "functions": {}
}"#;
    let doc = CatalogueDocumentCodec::decode(json, "domain").expect("loose tombstones decode");

    let mut type_deletion_keys = doc
        .deletions()
        .iter()
        .filter_map(|record| match record {
            DeletionRecord::Type { name, .. } => Some(name.as_str().to_owned()),
            DeletionRecord::Trait { .. } | DeletionRecord::Function { .. } => None,
        })
        .collect::<Vec<_>>();
    type_deletion_keys.sort();
    assert_eq!(type_deletion_keys, vec!["alpha::GoneType", "beta::GoneType"]);
    let mut trait_deletion_keys = doc
        .deletions()
        .iter()
        .filter_map(|record| match record {
            DeletionRecord::Trait { name, .. } => Some(name.as_str().to_owned()),
            DeletionRecord::Type { .. } | DeletionRecord::Function { .. } => None,
        })
        .collect::<Vec<_>>();
    trait_deletion_keys.sort();
    assert_eq!(trait_deletion_keys, vec!["alpha::GoneTrait", "beta::GoneTrait"]);

    let encoded = encode_doc(doc).expect("loose tombstones resolve against rustdoc paths");
    let alpha_id = item_id_for_path(&encoded, &["domain", "alpha", "GoneType"]);
    let beta_id = item_id_for_path(&encoded, &["domain", "beta", "GoneType"]);
    let alpha_trait_id = item_id_for_path(&encoded, &["domain", "alpha", "GoneTrait"]);
    let beta_trait_id = item_id_for_path(&encoded, &["domain", "beta", "GoneTrait"]);
    assert_ne!(alpha_id, beta_id);
    assert_ne!(alpha_trait_id, beta_trait_id);
    assert_eq!(encoded.action_for(&alpha_id), Some(ItemAction::Delete));
    assert_eq!(encoded.action_for(&beta_id), Some(ItemAction::Delete));
    assert_eq!(encoded.action_for(&alpha_trait_id), Some(ItemAction::Delete));
    assert_eq!(encoded.action_for(&beta_trait_id), Some(ItemAction::Delete));
    assert_eq!(encoded.krate().paths[&alpha_id].path, ["domain", "alpha", "GoneType"]);
    assert_eq!(encoded.krate().paths[&beta_id].path, ["domain", "beta", "GoneType"]);
    assert_eq!(encoded.krate().paths[&alpha_trait_id].path, ["domain", "alpha", "GoneTrait"]);
    assert_eq!(encoded.krate().paths[&beta_trait_id].path, ["domain", "beta", "GoneTrait"]);
    assert_ne!(alpha_id, Id(UNRESOLVED_CRATE_ID));
    assert_ne!(beta_id, Id(UNRESOLVED_CRATE_ID));
    assert_ne!(alpha_trait_id, Id(UNRESOLVED_CRATE_ID));
    assert_ne!(beta_trait_id, Id(UNRESOLVED_CRATE_ID));
}

#[test]
fn test_encode_delete_tombstone_alias_of_live_entry_returns_collision_error() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("a::Thing".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["a".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_deletion(DeletionRecord::Type {
        name: CatalogueEntryKey::try_new("domain::a::Thing".to_owned()).unwrap(),
        spec_refs: vec![],
        informal_grounds: vec![],
    });

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, NewTypeGraphCodecError::InvalidTypeRef(..)));
    let message = error.to_string();
    assert!(
        message.contains("domain::a::Thing") && message.contains("a::Thing"),
        "collision diagnostic must include both catalogue spellings: {message}"
    );
    assert!(
        message.contains("refusing to overwrite"),
        "collision must fail before the live declaration can be replaced: {message}"
    );
}

#[test]
fn test_encode_rejects_qualified_type_key_with_conflicting_module_path() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("domain::a::Thing".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["b".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("domain::a::Thing"), "diagnostic must name the key: {message}");
    assert!(message.contains("module_path 'a'"), "diagnostic must name the key path: {message}");
    assert!(message.contains("module_path is 'b'"), "diagnostic must name the DTO path: {message}");
}

#[test]
fn test_encode_rejects_qualified_trait_key_with_conflicting_module_path() {
    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("domain::a::Thing".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["b".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("domain::a::Thing"), "diagnostic must name the key: {message}");
    assert!(message.contains("module_path 'a'"), "diagnostic must name the key path: {message}");
    assert!(message.contains("module_path is 'b'"), "diagnostic must name the DTO path: {message}");
}

#[test]
fn test_encode_ambiguous_short_name_returns_all_fully_qualified_candidates() {
    let mut doc = make_doc("domain");
    for (key, module) in [("a::Input", "a"), ("b::Input", "b")] {
        doc.insert_type(
            CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![] },
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
    }
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        TypeRef::new("Input").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    let NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates) = error else {
        panic!("expected AmbiguousIdentifier, got {error:?}");
    };
    assert_eq!(identifier.as_str(), "Input");
    let candidate_paths = candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(candidate_paths, vec!["domain::a::Input", "domain::b::Input"]);
}

// -----------------------------------------------------------------------
// Error path: InvalidTypeRef
// -----------------------------------------------------------------------

#[test]
fn test_encode_returns_invalid_type_ref_for_unparseable_field_type() {
    // A struct field with a TypeRef that syn cannot parse triggers InvalidTypeRef.
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("BadType".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        // "42invalid" is not a valid Rust type expression.
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![MethodDeclaration::new(
                MethodName::new("get_value").unwrap(),
                Some(SelfReceiver::SharedRef),
                vec![],
                // TypeRef::new accepts any non-empty string; the codec rejects it at syn parse time.
                TypeRef::new("42invalid").unwrap(),
                false,
                false,
                vec![],
                vec![],
                vec![],
                ItemAction::Add,
                None,
            )],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let result = encode_doc(doc);
    assert!(result.is_err(), "expected InvalidTypeRef error for unparseable return type");
    let err = result.unwrap_err();
    assert!(
        matches!(err, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)),
        "expected InvalidTypeRef error, got: {err:?}"
    );
}

// -----------------------------------------------------------------------
// AC-05: inline → id-ref conversion — struct fields
// -----------------------------------------------------------------------

#[test]
fn test_encode_struct_fields_are_promoted_to_struct_field_items() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("User".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("email").unwrap(),
                            TypeRef::new("String").unwrap(),
                        ),
                        FieldDecl::new(FieldName::new("id").unwrap(), TypeRef::new("u32").unwrap()),
                    ],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let struct_field_count = ec
        .krate()
        .index
        .values()
        .filter(|item| matches!(item.inner, ItemEnum::StructField(_)))
        .count();
    assert_eq!(struct_field_count, 2);
}

// -----------------------------------------------------------------------
// AC-05: inline → id-ref conversion — enum variants
// -----------------------------------------------------------------------

#[test]
fn test_encode_enum_variants_are_promoted_to_variant_items() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("ItemAction".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum {
                variants: vec![
                    VariantDecl::unit(VariantName::new("Add").unwrap()),
                    VariantDecl::tuple(
                        VariantName::new("Error").unwrap(),
                        vec![TypeRef::new("String").unwrap()],
                    ),
                ],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let variant_count =
        ec.krate().index.values().filter(|item| matches!(item.inner, ItemEnum::Variant(_))).count();
    assert_eq!(variant_count, 2);
}

// -----------------------------------------------------------------------
// AC-05: 1 type = 1 Inherent Impl block
// -----------------------------------------------------------------------

#[test]
fn test_encode_type_with_methods_produces_single_inherent_impl_block() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Email".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![
                MethodDeclaration::new(
                    MethodName::new("new").unwrap(),
                    None,
                    vec![],
                    TypeRef::new("Self").unwrap(),
                    false,
                    false,
                    vec![],
                    vec![],
                    vec![],
                    ItemAction::Add,
                    None,
                ),
                MethodDeclaration::new(
                    MethodName::new("as_str").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("str").unwrap(),
                    false,
                    false,
                    vec![],
                    vec![],
                    vec![],
                    ItemAction::Add,
                    None,
                ),
            ],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    // Exactly 1 inherent Impl block.
    let inherent_impl_count = krate
        .index
        .values()
        .filter(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_none()))
        .count();
    assert_eq!(inherent_impl_count, 1, "expected 1 inherent Impl block");

    // 2 Function items for the methods.
    let fn_count =
        krate.index.values().filter(|item| matches!(item.inner, ItemEnum::Function(_))).count();
    assert_eq!(fn_count, 2);
}

// -----------------------------------------------------------------------
// AC-05: Crate.paths — module_path included
// -----------------------------------------------------------------------

#[test]
fn test_encode_paths_includes_module_path_segments() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Draft".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["review".to_string()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let summary = ec
        .krate()
        .paths
        .values()
        .find(|s| s.path.last().map(|n| n == "Draft").unwrap_or(false))
        .expect("Draft not found in paths");
    assert_eq!(summary.path, vec!["domain", "review", "Draft"]);
}

#[test]
fn test_encode_paths_crate_root_type_has_two_segment_path() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("UserId".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let summary = ec
        .krate()
        .paths
        .values()
        .find(|s| s.path.last().map(|n| n == "UserId").unwrap_or(false))
        .expect("UserId not found in paths");
    assert_eq!(summary.path, vec!["domain", "UserId"]);
}

// -----------------------------------------------------------------------
// AC-06: TypeRef generics parse
// -----------------------------------------------------------------------

#[test]
fn test_encode_field_with_generic_type_ref_creates_resolved_path_with_args() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Cart".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("items").unwrap(),
                        TypeRef::new("Vec<String>").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let field_with_args = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::StructField(Type::ResolvedPath(p)) if p.path.contains("Vec") && p.args.is_some())
    });
    assert!(field_with_args.is_some(), "expected Vec<String> field with generic args");
}

// -----------------------------------------------------------------------
// AC-06: std prelude auto-resolution
// -----------------------------------------------------------------------

#[test]
fn test_encode_std_prelude_type_creates_std_external_crate_entry() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("name").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let has_std = ec.krate().external_crates.values().any(|e| e.name == "std");
    assert!(has_std, "expected 'std' in external_crates");
}

#[test]
fn test_encode_bare_prelude_name_with_ambiguous_local_candidates_returns_candidates() {
    let mut doc = make_doc("domain");
    for (key, module) in [("a::String", "a"), ("b::String", "b")] {
        doc.insert_type(
            CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![] },
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
    }
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    let NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates) = error else {
        panic!("expected AmbiguousIdentifier for bare String, got {error:?}");
    };
    assert_eq!(identifier.as_str(), "String");
    let candidate_paths = candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(candidate_paths, vec!["domain::a::String", "domain::b::String"]);
}

#[test]
fn test_encode_generic_bound_with_ambiguous_local_prelude_trait_returns_candidates() {
    let mut doc = make_doc("domain");
    for (key, module) in [("a::Clone", "a"), ("b::Clone", "b")] {
        doc.insert_trait(
            CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
    }
    let crate_name = CrateName::new("domain").unwrap();
    let function_path = FunctionPath::at_root(crate_name, FunctionName::new("bounded").unwrap());
    doc.insert_function(
        function_path,
        domain::tddd::catalogue_v2::entries::FunctionEntry::new(
            ItemAction::Add,
            domain::tddd::catalogue_v2::roles::FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            vec![],
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    let NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates) = error else {
        panic!("expected AmbiguousIdentifier for bound Clone, got {error:?}");
    };
    assert_eq!(identifier.as_str(), "Clone");
    let candidate_paths = candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(candidate_paths, vec!["domain::a::Clone", "domain::b::Clone"]);
}

#[test]
fn test_encode_type_alias_generic_bound_with_ambiguous_local_prelude_trait_returns_candidates() {
    let mut doc = make_doc("domain");
    for (key, module) in [("a::Clone", "a"), ("b::Clone", "b")] {
        doc.insert_trait(
            CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
    }
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![TypeRef::new("Clone").unwrap()],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    let NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates) = error else {
        panic!("expected AmbiguousIdentifier for alias bound Clone, got {error:?}");
    };
    assert_eq!(identifier.as_str(), "Clone");
    let candidate_paths = candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(candidate_paths, vec!["domain::a::Clone", "domain::b::Clone"]);
}

#[test]
fn test_encode_inherent_impl_unique_short_name_resolves_to_qualified_entry() {
    use domain::tddd::catalogue_v2::entries::InherentImplDeclV2;

    let mut doc = make_doc("domain");
    let type_key = CatalogueEntryKey::try_new("a::Thing".to_owned()).unwrap();
    doc.insert_type(
        type_key.clone(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["a".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Thing".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![],
    ));

    let encoded = encode_doc(doc).unwrap();
    let type_id = item_id_for_path(&encoded, &["domain", "a", "Thing"]);
    assert!(
        encoded.krate().index.values().any(|item| {
            matches!(
                &item.inner,
                ItemEnum::Impl(impl_item)
                    if impl_item.trait_.is_none()
                        && matches!(&impl_item.for_, Type::ResolvedPath(path) if path.id == type_id)
            )
        }),
        "the short inherent-impl name must resolve to the only qualified Thing declaration"
    );
}

#[test]
fn test_encode_inherent_impl_ambiguous_short_name_returns_candidates() {
    use domain::tddd::catalogue_v2::entries::InherentImplDeclV2;

    let mut doc = make_doc("domain");
    for (key, module) in [("a::Thing", "a"), ("b::Thing", "b")] {
        doc.insert_type(
            CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
    }
    doc.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Thing".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![],
    ));

    let error = encode_doc(doc).unwrap_err();
    let NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates) = error else {
        panic!("expected AmbiguousIdentifier for inherent Thing, got {error:?}");
    };
    assert_eq!(identifier.as_str(), "Thing");
    let candidate_paths = candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(candidate_paths, vec!["domain::a::Thing", "domain::b::Thing"]);
}

// -----------------------------------------------------------------------
// AC-04: undeclared local types fail closed
// -----------------------------------------------------------------------

#[test]
fn test_encode_undeclared_type_ref_field_returns_unresolved_identifier() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("error").unwrap(),
                        TypeRef::new("DomainError").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(
        error,
        domain::tddd::NewTypeGraphCodecError::UnresolvedIdentifier(ref type_ref)
            if type_ref.as_str() == "DomainError"
    ));
}

#[test]
fn test_encode_external_type_ref_absent_from_authoritative_paths_fails_closed() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("missing").unwrap(),
                        TypeRef::new("ghost::Missing").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(
        error,
        NewTypeGraphCodecError::UnresolvedIdentifier(ref type_ref)
            if type_ref.as_str() == "ghost::Missing"
    ));
}

// -----------------------------------------------------------------------
// AC-05: item_actions populated
// -----------------------------------------------------------------------

#[test]
fn test_encode_item_actions_contains_declared_action() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Email".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Modify,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let has_modify = ec.item_actions().values().any(|a| *a == ItemAction::Modify);
    assert!(has_modify);
}

// -----------------------------------------------------------------------
// AC-05: external_crates from TraitImplDeclV2::origin_crate
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_impl_origin_crate_registered_in_external_crates() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    // ADR `2026-05-20-0048` D1: trait_impls are top-level on CatalogueDocument.
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("serde::Serialize").unwrap(),
        TypeRef::new("Foo").unwrap(),
    ));

    let ec = encode_doc(doc).unwrap();
    let has_serde = ec.krate().external_crates.values().any(|e| e.name == "serde");
    assert!(has_serde, "expected 'serde' in external_crates");
}

// -----------------------------------------------------------------------
// AC-05: trait entry encoding
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_entry_produces_trait_item() {
    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("UserRepository".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let trait_item = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::Trait(_)) && item.name.as_deref() == Some("UserRepository")
    });
    assert!(trait_item.is_some(), "expected Trait item for UserRepository");
}

// -----------------------------------------------------------------------
// Type alias
// -----------------------------------------------------------------------

#[test]
fn test_encode_type_alias_produces_type_alias_item() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("UserResult".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("Result<String, String>").unwrap(),
                generics: vec![],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias_item = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::TypeAlias(_)) && item.name.as_deref() == Some("UserResult")
    });
    assert!(alias_item.is_some(), "expected TypeAlias item for UserResult");
}

// -----------------------------------------------------------------------
// Empty catalogue
// -----------------------------------------------------------------------

#[test]
fn test_encode_empty_catalogue_produces_root_module() {
    let doc = make_doc("domain");
    let ec = encode_doc(doc).unwrap();
    assert!(ec.krate().index.contains_key(&Id(0)), "expected root Id(0)");
}

// -----------------------------------------------------------------------
// generic_args in TraitImplDeclV2 → structured trait_.args (ADR 2026-05-20-0048 D2)
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_impl_with_generic_args_produces_impl_with_structured_trait_args() {
    // Per ADR `2026-05-20-0048` D2, the encoded Impl item's trait path is the canonical
    // BASE path (`"core::convert::From"`) and the generic args are carried structurally
    // in `trait_.args` — NOT re-inlined into the path string.  `build_impl_identity_map`
    // renders the structured args via `format_generic_args` at key-construction time,
    // producing `"RenderContractMapError: core::convert::From<CatalogueLoaderError>"` on
    // both the S-side (this codec) and the C-side (rustdoc).
    let mut doc = make_doc("usecase");
    insert_empty_enum_type(&mut doc, "CatalogueLoaderError");
    insert_empty_enum_type(&mut doc, "ContractMapWriterError");
    doc.insert_type(
        CatalogueEntryKey::try_new("RenderContractMapError".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Modify,
            DataRole::ErrorType,
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    // ADR `2026-05-20-0048` D1/D2: trait_impls are top-level; generic args in trait_ref string.
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("core::convert::From<CatalogueLoaderError>").unwrap(),
        TypeRef::new("RenderContractMapError").unwrap(),
    ));
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("core::convert::From<ContractMapWriterError>").unwrap(),
        TypeRef::new("RenderContractMapError").unwrap(),
    ));

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    // Collect (base_path, structured_args) from all Impl items that have a trait.
    let from_impls: Vec<(String, String)> = krate
        .index
        .values()
        .filter_map(|item| {
            if let ItemEnum::Impl(impl_) = &item.inner {
                let tp = impl_.trait_.as_ref()?;
                let args_joined = match tp.args.as_deref() {
                    Some(rustdoc_types::GenericArgs::AngleBracketed { args, .. }) => args
                        .iter()
                        .filter_map(|a| match a {
                            rustdoc_types::GenericArg::Type(Type::ResolvedPath(p)) => {
                                Some(p.path.clone())
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => String::new(),
                };
                Some((tp.path.clone(), args_joined))
            } else {
                None
            }
        })
        .collect();

    // ADR D2: the trait path is the bare base form — no inline generic args.
    assert!(
        from_impls.iter().all(|(path, _)| !path.contains('<')),
        "trait path must be the bare base form with no inline generic args, got: {from_impls:?}"
    );
    // The generic args are carried structurally in `trait_.args`.
    assert!(
        from_impls
            .iter()
            .any(|(path, args)| path == "core::convert::From"
                && args.contains("CatalogueLoaderError")),
        "expected impl with base path 'core::convert::From' and structured arg 'CatalogueLoaderError', got: {from_impls:?}"
    );
    assert!(
        from_impls
            .iter()
            .any(|(path, args)| path == "core::convert::From"
                && args.contains("ContractMapWriterError")),
        "expected impl with base path 'core::convert::From' and structured arg 'ContractMapWriterError', got: {from_impls:?}"
    );
}

#[test]
fn test_encode_trait_impl_without_generic_args_produces_impl_with_qualified_core_trait_path() {
    // When `generic_args` is None and `origin_crate` is `"core"`, the impl trait path
    // must be the fully-qualified canonical path (`"core::convert::From"` not bare `"From"`).
    // `build_impl_identity_map` uses `krate.paths` to resolve C-side trait paths to
    // their canonical qualified form (e.g. `"core::convert::From"`) so S-side must
    // emit the same form to avoid identity-key mismatches.
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("SomeError".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::ErrorType,
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    // ADR `2026-05-20-0048` D1/D2: trait_impls are top-level; full qualified path in trait_ref.
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("core::convert::From").unwrap(),
        TypeRef::new("SomeError").unwrap(),
    ));

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    let trait_paths: Vec<String> = krate
        .index
        .values()
        .filter_map(|item| {
            if let ItemEnum::Impl(impl_) = &item.inner {
                impl_.trait_.as_ref().map(|tp| tp.path.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        trait_paths.iter().any(|p| p == "core::convert::From"),
        "expected qualified 'core::convert::From' trait path when generic_args is None, got: {trait_paths:?}"
    );
}

// -----------------------------------------------------------------------
// Struct variant with named fields
// -----------------------------------------------------------------------

#[test]
fn test_encode_enum_struct_variant_produces_named_struct_field_items() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("ParseError".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::ErrorType,
            TypeKindV2::Enum {
                variants: vec![VariantDecl::struct_variant(
                    VariantName::new("InvalidToken").unwrap(),
                    vec![FieldDecl::new(
                        FieldName::new("message").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                )],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let struct_variant = ec.krate().index.values().find(|item| {
        if let ItemEnum::Variant(v) = &item.inner {
            matches!(&v.kind, VariantKind::Struct { fields, .. } if !fields.is_empty())
        } else {
            false
        }
    });
    assert!(struct_variant.is_some(), "expected struct Variant with fields");
}

// -----------------------------------------------------------------------
// AC-method-generics: method generic params are encoded as Type::Generic
// -----------------------------------------------------------------------

/// A method with `generics: [{ name: "T", bounds: ["Into<String>"] }]` and
/// a parameter of type `"T"` must encode that parameter as `Type::Generic("T")`,
/// not as a `ResolvedPath`.  Rustdoc emits `Type::Generic` for method-level
/// generic type parameters, so the S-side must match.
#[test]
fn test_encode_method_generic_param_type_emits_type_generic() {
    let mut doc = make_doc("domain");
    let method = MethodDeclaration::new(
        MethodName::new("set_value").unwrap(),
        Some(SelfReceiver::ExclusiveRef),
        vec![ParamDeclaration::new(ParamName::new("value").unwrap(), TypeRef::new("T").unwrap())],
        TypeRef::new("()").unwrap(),
        false,
        false,
        vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Into<String>").unwrap()],
        }],
        vec![],
        vec![],
        ItemAction::Add,
        None,
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("ValueHolder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![method],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();
    // Find the method Function item (set_value).
    let fn_item = krate.index.values().find(|item| {
        item.name.as_deref() == Some("set_value") && matches!(item.inner, ItemEnum::Function(_))
    });
    assert!(fn_item.is_some(), "expected Function item for set_value");
    let ItemEnum::Function(ref f) = fn_item.unwrap().inner else { panic!("expected Function") };
    // The first input is "self" (ExclusiveRef); the second is the "value: T" param.
    let value_param = f.sig.inputs.iter().find(|(name, _)| name == "value");
    assert!(value_param.is_some(), "expected input named 'value'");
    let (_, ty) = value_param.unwrap();
    assert!(
        matches!(ty, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for generic param type, got: {ty:?}"
    );
}

#[test]
fn test_encode_method_nested_generic_type_resolves_as_generic() {
    fn assert_option_of_generic(ty: &Type) {
        let Type::ResolvedPath(path) = ty else {
            panic!("expected Option<T> to be a resolved path, got {ty:?}");
        };
        let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
            panic!("expected Option<T> to carry angle-bracketed arguments: {path:?}");
        };
        assert!(
            args.iter()
                .any(|arg| matches!(arg, GenericArg::Type(Type::Generic(name)) if name == "T")),
            "expected Option<T> to contain Type::Generic(\"T\"), got {args:?}"
        );
    }

    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("GenericHolder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![MethodDeclaration::new(
                MethodName::new("round_trip").unwrap(),
                None,
                vec![ParamDeclaration::new(
                    ParamName::new("value").unwrap(),
                    TypeRef::new("Option<T>").unwrap(),
                )],
                TypeRef::new("Option<T>").unwrap(),
                false,
                false,
                vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
                vec![],
                vec![],
                ItemAction::Add,
                None,
            )],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    let method = encoded
        .krate()
        .index
        .values()
        .find_map(|item| match &item.inner {
            ItemEnum::Function(function) if item.name.as_deref() == Some("round_trip") => {
                Some(function)
            }
            _ => None,
        })
        .expect("expected round_trip method item");
    let (_, parameter_type) = method
        .sig
        .inputs
        .iter()
        .find(|(name, _)| name == "value")
        .expect("expected value parameter");
    assert_option_of_generic(parameter_type);
    assert_option_of_generic(method.sig.output.as_ref().expect("expected return type"));
}

#[test]
fn test_encode_function_nested_generic_type_resolves_as_generic() {
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::roles::FunctionRole;

    let mut doc = make_doc("domain");
    let function_path = FunctionPath::at_root(
        CrateName::new("domain").unwrap(),
        FunctionName::new("round_trip_fn").unwrap(),
    );
    doc.insert_function(
        function_path,
        FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![ParamDeclaration::new(
                ParamName::new("value").unwrap(),
                TypeRef::new("Option<T>").unwrap(),
            )],
            TypeRef::new("Option<T>").unwrap(),
            false,
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    let function = encoded
        .krate()
        .index
        .values()
        .find_map(|item| match &item.inner {
            ItemEnum::Function(function) if item.name.as_deref() == Some("round_trip_fn") => {
                Some(function)
            }
            _ => None,
        })
        .expect("expected free function item");
    for ty in
        [&function.sig.inputs[0].1, function.sig.output.as_ref().expect("expected function output")]
    {
        let Type::ResolvedPath(path) = ty else {
            panic!("expected Option<T> to be a resolved path, got {ty:?}");
        };
        let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
            panic!("expected Option<T> generic args, got {path:?}");
        };
        assert!(
            args.iter()
                .any(|arg| matches!(arg, GenericArg::Type(Type::Generic(name)) if name == "T")),
            "expected Option<T> to contain Type::Generic(\"T\"), got {args:?}"
        );
    }
}

#[test]
fn test_encode_trait_impl_nested_generic_type_resolves_as_generic() {
    fn assert_trait_argument_is_generic(impl_item: &rustdoc_types::Impl) {
        let trait_path = impl_item.trait_.as_ref().expect("expected a trait impl path");
        let Some(GenericArgs::AngleBracketed { args, .. }) = trait_path.args.as_deref() else {
            panic!("expected Trait<T> to carry angle-bracketed arguments: {trait_path:?}");
        };
        assert!(
            args.iter()
                .any(|arg| matches!(arg, GenericArg::Type(Type::Generic(name)) if name == "T")),
            "expected Trait<T> to contain Type::Generic(\"T\"), got {args:?}"
        );
    }

    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Target".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("Trait".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_trait_impl(TraitImplDeclV2::from_parts(
        ItemAction::Add,
        TypeRef::new("Trait<T>").unwrap(),
        TypeRef::new("Target").unwrap(),
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![],
    ));

    let encoded = encode_doc(doc).unwrap();
    let trait_impl = encoded
        .krate()
        .index
        .values()
        .find_map(|item| match &item.inner {
            ItemEnum::Impl(impl_item) if impl_item.trait_.is_some() => Some(impl_item),
            _ => None,
        })
        .expect("expected Trait<T> impl item");
    assert_trait_argument_is_generic(trait_impl);
}

// -----------------------------------------------------------------------
// ADR 2026-07-02-1345 D6: type-declaration-level generics on struct fields,
// enum payloads, and alias targets encode as Type::Generic
// -----------------------------------------------------------------------

/// A plain-struct field whose type is a type-declaration-level generic
/// (`value: T` in `struct Foo<T> { value: T }`) must encode as
/// `Type::Generic("T")`, matching rustdoc's C-side. Without the generic wiring it
/// falls through to an unresolved local-path marker, which surfaces a false
/// non-Blue signal for a structurally-correct generic struct.
#[test]
fn test_encode_struct_field_generic_type_emits_type_generic() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        TypeRef::new("T").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let field = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("value") && matches!(item.inner, ItemEnum::StructField(_))
    });
    assert!(field.is_some(), "expected StructField item for 'value'");
    let ItemEnum::StructField(ref ty) = field.unwrap().inner else {
        panic!("expected StructField")
    };
    assert!(
        matches!(ty, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for generic struct field, got: {ty:?}"
    );
}

/// A tuple-variant payload referencing a type-declaration-level generic
/// (`Some(T)` in `enum Opt<T> { Some(T) }`) must encode as `Type::Generic("T")`.
#[test]
fn test_encode_enum_tuple_variant_payload_generic_emits_type_generic() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Opt".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum {
                variants: vec![VariantDecl::tuple(
                    VariantName::new("Some").unwrap(),
                    vec![TypeRef::new("T").unwrap()],
                )],
            },
            vec![],
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    // The enum encoding produces exactly one StructField: the variant payload.
    let field =
        ec.krate().index.values().find(|item| matches!(item.inner, ItemEnum::StructField(_)));
    assert!(field.is_some(), "expected a StructField item for the variant payload");
    let ItemEnum::StructField(ref ty) = field.unwrap().inner else {
        panic!("expected StructField")
    };
    assert!(
        matches!(ty, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for generic enum payload, got: {ty:?}"
    );
}

/// A type-alias target referencing a type-declaration-level generic
/// (`type Alias<T> = T`) must encode as `Type::Generic("T")`.
#[test]
fn test_encode_type_alias_target_generic_emits_type_generic() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
    });
    assert!(alias.is_some(), "expected TypeAlias item for 'Alias'");
    let ItemEnum::TypeAlias(ref ta) = alias.unwrap().inner else { panic!("expected TypeAlias") };
    assert!(
        matches!(&ta.type_, Type::Generic(g) if g == "T"),
        "expected alias target Type::Generic(\"T\"), got: {:?}",
        ta.type_
    );
}

#[test]
fn test_encode_type_alias_generic_bound_preserves_catalogue_spelling() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![TypeRef::new("Clone").unwrap()],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
    });
    let ItemEnum::TypeAlias(ref ta) = alias.expect("expected TypeAlias").inner else {
        panic!("expected TypeAlias")
    };
    let Some(WherePredicate::BoundPredicate { bounds, .. }) = ta.generics.where_predicates.first()
    else {
        panic!("expected alias bound predicate")
    };
    let Some(GenericBound::TraitBound { trait_, .. }) = bounds.first() else {
        panic!("expected trait bound")
    };
    assert_eq!(trait_.path, "Clone");
    assert_ne!(trait_.id, Id(UNRESOLVED_CRATE_ID));
    assert_eq!(ec.krate().paths[&trait_.id].path, ["std", "clone", "Clone"]);

    // A bare prelude trait in an alias bound must make it through Phase 1 as
    // a known external, rather than being rejected as a local unresolved name.
    // Model the current rustdoc crate's graph-local id for `Clone`: the
    // lexical alias comparison must ignore that id while Phase 1 still
    // requires the catalogue-side bound to be a resolved external.
    let mut c = ec.krate().clone();
    for item in c.index.values_mut() {
        let ItemEnum::TypeAlias(alias) = &mut item.inner else {
            continue;
        };
        for predicate in &mut alias.generics.where_predicates {
            let WherePredicate::BoundPredicate { bounds, .. } = predicate else {
                continue;
            };
            for bound in bounds {
                if let GenericBound::TraitBound { trait_, .. } = bound {
                    if trait_.path == "Clone" {
                        trait_.id = Id(9_000);
                    }
                }
            }
        }
    }
    let empty_baseline = rustdoc_types::Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths: std::collections::HashMap::new(),
        external_crates: std::collections::HashMap::new(),
        format_version: rustdoc_types::FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    };
    let evaluation = SignalEvaluatorV2::new().evaluate(ec, empty_baseline, c);
    assert!(evaluation.is_ok(), "alias bound must survive evaluator Phase 1: {evaluation:?}");
}

#[test]
fn test_encode_type_alias_generic_bound_preserves_unknown_abi_literal_quotes() {
    let mut doc = make_doc("domain");
    insert_empty_enum_type(&mut doc, "Outer");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![TypeRef::new("Outer<extern \"efiapi\" fn()>").unwrap()],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
    });
    let ItemEnum::TypeAlias(ref ta) = alias.expect("expected TypeAlias").inner else {
        panic!("expected TypeAlias")
    };
    let Some(WherePredicate::BoundPredicate { bounds, .. }) = ta.generics.where_predicates.first()
    else {
        panic!("expected alias bound predicate")
    };
    let Some(GenericBound::TraitBound { trait_, .. }) = bounds.first() else {
        panic!("expected trait bound")
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = trait_.args.as_deref() else {
        panic!("expected Outer generic arguments")
    };
    let Some(GenericArg::Type(Type::FunctionPointer(function_pointer))) = args.first() else {
        panic!("expected function pointer generic argument")
    };
    assert_eq!(function_pointer.header.abi, rustdoc_types::Abi::Other("\"efiapi\"".to_owned()));
}

#[test]
fn test_encode_type_alias_generic_bound_accepts_raw_pointer_argument() {
    let mut doc = make_doc("domain");
    insert_empty_enum_type(&mut doc, "Outer");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![TypeRef::new("Outer<*const u8>").unwrap()],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
    });
    let ItemEnum::TypeAlias(ref ta) = alias.expect("expected TypeAlias").inner else {
        panic!("expected TypeAlias")
    };
    let Some(WherePredicate::BoundPredicate { bounds, .. }) = ta.generics.where_predicates.first()
    else {
        panic!("expected alias bound predicate")
    };
    let Some(GenericBound::TraitBound { trait_, .. }) = bounds.first() else {
        panic!("expected trait bound")
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = trait_.args.as_deref() else {
        panic!("expected Outer generic arguments")
    };
    let Some(GenericArg::Type(Type::RawPointer { is_mutable, type_ })) = args.first() else {
        panic!("expected raw pointer generic argument")
    };
    assert!(!is_mutable, "expected `*const` raw pointer");
    assert!(matches!(type_.as_ref(), Type::Primitive(name) if name == "u8"));
}

#[test]
fn test_encode_type_alias_where_subject_preserves_catalogue_spelling() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("Vec<T>").unwrap(),
                rhs: vec![TypeRef::new("Clone").unwrap()],
                operator: BoundOp::Bound,
            }],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
    });
    let ItemEnum::TypeAlias(ref ta) = alias.expect("expected TypeAlias").inner else {
        panic!("expected TypeAlias")
    };
    let Some(WherePredicate::BoundPredicate { type_, .. }) = ta.generics.where_predicates.first()
    else {
        panic!("expected alias where predicate")
    };
    let Type::ResolvedPath(path) = type_ else {
        panic!("expected resolved Vec<T> where subject, got {type_:?}")
    };
    assert_eq!(path.path, "Vec");
    assert!(path.args.is_some(), "expected generic argument T on Vec<T>");
}

#[test]
fn test_encode_type_alias_where_subject_rejects_unsupported_array_lengths() {
    for lhs in ["[u8; LEN]", "[u8; 1 as usize]"] {
        let mut doc = make_doc("domain");
        doc.insert_type(
            CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias {
                    target: TypeRef::new("u8").unwrap(),
                    generics: vec![MethodGenericParam {
                        name: ParamName::new("T").unwrap(),
                        bounds: vec![],
                    }],
                },
                vec![],
                vec![],
                vec![WherePredicateDecl {
                    lhs: TypeRef::new(lhs).unwrap(),
                    rhs: vec![TypeRef::new("Clone").unwrap()],
                    operator: BoundOp::Bound,
                }],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let error = encode_doc(doc).unwrap_err();
        assert!(
            matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)),
            "unsupported lexical array length should be rejected: {lhs}: {error:?}"
        );
    }
}

#[test]
fn test_encode_legacy_type_alias_where_subject_accepts_array_lengths() {
    for lhs in ["[u8; LEN]", "[u8; 1 as usize]"] {
        let mut doc = make_doc("domain");
        doc.insert_type(
            CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias { target: TypeRef::new("u8").unwrap(), generics: vec![] },
                vec![],
                vec![],
                vec![WherePredicateDecl {
                    lhs: TypeRef::new(lhs).unwrap(),
                    rhs: vec![TypeRef::new("Clone").unwrap()],
                    operator: BoundOp::Bound,
                }],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        assert!(
            encode_doc(doc).is_ok(),
            "legacy alias where subject should remain accepted: {lhs}"
        );
    }
}

#[test]
fn test_encode_type_alias_generic_maybe_const_bound_preserves_catalogue_spelling() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![TypeRef::new("~const Clone").unwrap()],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec.krate().index.values().find(|item| {
        item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
    });
    let ItemEnum::TypeAlias(ref ta) = alias.expect("expected TypeAlias").inner else {
        panic!("expected TypeAlias")
    };
    let Some(WherePredicate::BoundPredicate { bounds, .. }) = ta.generics.where_predicates.first()
    else {
        panic!("expected alias bound predicate")
    };
    let Some(GenericBound::TraitBound { trait_, modifier, .. }) = bounds.first() else {
        panic!("expected trait bound")
    };
    assert_eq!(trait_.path, "Clone");
    assert_eq!(*modifier, TraitBoundModifier::MaybeConst);
}

/// A type-alias target using an unqualified associated-type projection rooted
/// at its declared generic (`type Alias<T: Iterator> = T::Item`) must encode
/// as a `QualifiedPath` over `Type::Generic("T")`, not as an external crate
/// path named `T::Item`.
#[test]
fn test_encode_type_alias_target_generic_projection_emits_qualified_path() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T::Item").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![TypeRef::new("Iterator").unwrap()],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let alias = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("Alias") && matches!(item.inner, ItemEnum::TypeAlias(_))
        })
        .expect("expected TypeAlias item for 'Alias'");
    let ItemEnum::TypeAlias(alias) = &alias.inner else { panic!("expected TypeAlias") };
    let Type::QualifiedPath { name, self_type, trait_, .. } = &alias.type_ else {
        panic!("expected alias target T::Item to be a qualified path, got {:?}", alias.type_);
    };
    assert_eq!(name, "Item");
    assert!(trait_.is_none());
    assert_eq!(self_type.as_ref(), &Type::Generic("T".to_owned()));
    assert!(
        !ec.krate().external_crates.values().any(|external| external.name == "T"),
        "generic projection prefix must not be registered as an external crate"
    );
}

/// Keyword-named generic declarations are rejected before lexical comparison;
/// the codec does not recreate Rust grammar to reinterpret `dyn::Item`.
#[test]
fn test_encode_type_alias_target_keyword_generic_projection_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("dyn::Item").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("dyn").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// Raw/keyword generic declarations are rejected; qself parsing is not used to
/// infer a catalogue declaration that the lexical boundary has disallowed.
#[test]
fn test_encode_type_alias_target_qualified_path_keyword_qself_generics_is_rejected() {
    let mut doc = make_doc("domain");
    for (alias_name, generic_name, target) in [
        ("AsAlias", "as", "<as as Trait>::Assoc"),
        ("DynAlias", "dyn", "<dyn as Trait>::Assoc"),
        ("ImplAlias", "impl", "<impl as Trait>::Assoc"),
    ] {
        doc.insert_type(
            CatalogueEntryKey::try_new(alias_name.to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias {
                    target: TypeRef::new(target).unwrap(),
                    generics: vec![MethodGenericParam {
                        name: ParamName::new(generic_name).unwrap(),
                        bounds: vec![],
                    }],
                },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
    }

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// Raw/keyword spellings are rejected at the catalogue boundary instead of
/// being restored by a hand-written Rust grammar classifier.
#[test]
fn test_encode_type_alias_target_rustdoc_normalized_raw_generic_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("Vec<type>").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("type").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// A keyword-named generic is rejected even when it appears next to valid raw
/// pointer syntax; the codec does not restore raw identifiers heuristically.
#[test]
fn test_encode_type_alias_target_const_pointer_keyword_generic_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("*const const").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("const").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// Keyword generic names in both targets and where predicates are rejected;
/// no grammar-aware restoration is attempted for nested expressions.
#[test]
fn test_encode_type_alias_where_predicate_nested_keyword_generic_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("Vec<type>").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("type").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("Vec<type>").unwrap(),
                rhs: vec![TypeRef::new("Clone").unwrap()],
                operator: BoundOp::Bound,
            }],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// A keyword declaration is rejected even if the target expression itself has
/// valid function-pointer syntax.
#[test]
fn test_encode_type_alias_target_with_keyword_syntax_and_keyword_generic_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("for<'a> fn(&'a str)").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("for").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// A `dyn` declaration is rejected rather than being inferred from a `dyn`
/// trait-object target expression.
#[test]
fn test_encode_type_alias_target_with_leading_path_dyn_syntax_and_keyword_generic_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("dyn ::Trait<dyn>").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("dyn").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

/// A keyword declaration is rejected even when the target has a valid `dyn`
/// trait-object lifetime bound.
#[test]
fn test_encode_type_alias_target_with_lifetime_dyn_syntax_and_keyword_generic_is_rejected() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("dyn 'static + Trait<dyn>").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("dyn").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).unwrap_err();
    assert!(matches!(error, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..)));
}

#[test]
fn test_encode_type_alias_with_two_generic_declarations_returns_error() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Alias".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: TypeRef::new("T").unwrap(),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("T").unwrap(),
                    bounds: vec![],
                }],
            },
            vec![],
            vec![MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] }],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let err = encode_doc(doc).unwrap_err();
    assert!(
        err.to_string().contains("both the entry and kind payload"),
        "unexpected error: {err:?}"
    );
}

// -----------------------------------------------------------------------
// ADR 0248 D13: per-method `has_body` from `has_default_impl` (Gap 1)
// -----------------------------------------------------------------------

/// A trait method declared with `has_default_impl: true` (provided default impl)
/// must encode to `rustdoc_types::Function.has_body = true` so that A-side and
/// C-side fingerprints both emit `;body` and `structurally_equal` returns true.
#[test]
fn test_encode_trait_method_with_has_default_impl_true_produces_has_body_true() {
    let mut doc = make_doc("usecase");
    let method = MethodDeclaration::new(
        MethodName::new("describe").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("String").unwrap(),
        false,
        true,
        vec![],
        vec![],
        vec![],
        ItemAction::Add,
        None,
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("Describable".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![method],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("describe") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for describe");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        f.has_body,
        "trait method with has_default_impl=true must encode has_body=true (ADR 0248 D13)"
    );
}

/// A trait method declared with `has_default_impl: false` (required / abstract)
/// must encode to `rustdoc_types::Function.has_body = false` so that A-side and
/// C-side fingerprints both emit `;abstract`.
#[test]
fn test_encode_trait_method_with_has_default_impl_false_produces_has_body_false() {
    let mut doc = make_doc("usecase");
    let method = MethodDeclaration::new(
        MethodName::new("required_op").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        false,
        vec![],
        vec![],
        vec![],
        ItemAction::Add,
        None,
    );
    // has_default_impl is explicitly false via MethodDeclaration::new.
    assert!(!method.has_default_impl());
    doc.insert_trait(
        CatalogueEntryKey::try_new("RequiredOps".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![method],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("required_op")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for required_op");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        !f.has_body,
        "trait method with has_default_impl=false must encode has_body=false (ADR 0248 D13)"
    );
}

/// Inherent method `has_body` is forced to `true` regardless of the
/// `has_default_impl` field (which is not semantically meaningful for inherent
/// methods). This preserves the pre-D13 invariant for struct inherent impls.
#[test]
fn test_encode_inherent_method_always_has_body_true_regardless_of_has_default_impl() {
    let mut doc = make_doc("domain");
    // Even if the catalogue accidentally sets has_default_impl=false on an
    // inherent method, the encoder must still emit has_body=true.
    let method = MethodDeclaration::new(
        MethodName::new("compute").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("u32").unwrap(),
        false,
        false,
        vec![],
        vec![],
        vec![],
        ItemAction::Add,
        None,
    );
    assert!(!method.has_default_impl());
    doc.insert_type(
        CatalogueEntryKey::try_new("Calculator".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![method],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("compute") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for compute");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        f.has_body,
        "inherent method must always encode has_body=true (force_has_body invariant)"
    );
}

// -----------------------------------------------------------------------
// ADR 0248 D14: FunctionEntry.generics → Function.generics (Gap 2)
// -----------------------------------------------------------------------

/// A free function with generic parameters must encode `entry.generics` as
/// `Function.generics`, and any param/return type that names one of those
/// generics must be emitted as `Type::Generic(_)` rather than as an
/// unresolved path. Mirrors `MethodDeclaration.generics` handling.
#[test]
fn test_encode_function_with_generics_emits_type_generic_in_signature() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::roles::FunctionRole;

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("generic_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![ParamDeclaration::new(ParamName::new("value").unwrap(), TypeRef::new("T").unwrap())],
        TypeRef::new("T").unwrap(),
        false,
        vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap()],
        }],
        vec![],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("generic_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for generic_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // generics participates: 1 type-param `T` with bound `Clone`.
    assert_eq!(f.generics.params.len(), 1, "expected 1 generic param, got {:?}", f.generics.params);
    assert_eq!(f.generics.params[0].name, "T");

    // The first input is `value: T` — must be Type::Generic("T").
    let (pname, pty) = &f.sig.inputs[0];
    assert_eq!(pname, "value");
    assert!(
        matches!(pty, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for `value` param, got {pty:?}"
    );

    // Return type is `T` — must be Type::Generic("T").
    let output = f.sig.output.as_ref().expect("expected Some output");
    assert!(
        matches!(output, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for return, got {output:?}"
    );
}

/// A free function with no generics emits `empty_generics()` (no params,
/// no where_predicates). This preserves the pre-D14 baseline for the vast
/// majority of free functions in the workspace.
#[test]
fn test_encode_function_without_generics_emits_empty_generics() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::roles::FunctionRole;

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("simple").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![],
        vec![],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("simple") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for simple");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        f.generics.params.is_empty() && f.generics.where_predicates.is_empty(),
        "function without generics must emit empty Generics"
    );
}

/// A catalogue `WherePredicateDecl.rhs[i]` whose string form starts with `use<`
/// must be accepted at encode time (ADR 2026-05-18-1223 D1 supersedes ADR
/// 2026-05-13-1153 D3).  `validate_supported_bound` and the syntactic pre-check
/// for `use<...>` are both removed; `parse_generic_bound` encodes `use<...>` as a
/// best-effort placeholder `GenericBound::TraitBound` and the encode must succeed.
#[test]
fn test_encode_function_with_use_capture_bound_in_where_predicate_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("ok_use").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![
            MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] },
        ],
        vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("use<U>").unwrap()],
            operator: BoundOp::Bound,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let result = encode_doc(doc);
    assert!(
        result.is_ok(),
        "precise-capture bound `use<U>` must be accepted without error, got: {result:?}"
    );
}

/// Same as the previous test but the precise-capture bound has a space between the
/// `use` keyword and the `<` token (i.e. `use <U>`).  This variant must also be
/// accepted after the syntactic pre-check removal (ADR 2026-05-18-1223 D1).
#[test]
fn test_encode_function_with_use_capture_bound_with_space_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("ok_use_space").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![
            MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] },
        ],
        vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            // Precise-capture with whitespace between `use` and `<`.
            rhs: vec![TypeRef::new("use <U>").unwrap()],
            operator: BoundOp::Bound,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let result = encode_doc(doc);
    assert!(
        result.is_ok(),
        "precise-capture bound `use <U>` (spaced) must be accepted without error, got: {result:?}"
    );
}

/// A `WherePredicateDecl` whose `lhs` is a qualified-path form
/// (`<T as Trait>::Assoc`) must be accepted at encode time under the permissive
/// principle (ADR `2026-05-20-0048`): any syn-parseable input is accepted.
/// The A-codec falls back to an unresolved placeholder for the qualified-path shape
/// it cannot reconstruct exactly — this is acceptable under the permissive principle.
#[test]
fn test_encode_function_with_qualified_path_lhs_in_where_predicate_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("qpath_lhs_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![WherePredicateDecl {
            // Qualified-path LHS: `<T as Iterator>::Item` — syn-parseable, accepted permissively.
            lhs: TypeRef::new("<T as Iterator>::Item").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    // Permissive: encoding must succeed (no shape validation rejection).
    let result = encode_doc(doc);
    assert!(
        result.is_ok(),
        "expected Ok for syn-parseable `<T as Trait>::Assoc` LHS under permissive principle, \
         got: {result:?}"
    );
}

// -----------------------------------------------------------------------
// ADR 2026-05-13-1153 D1: explicit WherePredicateDecl → where_predicates
// -----------------------------------------------------------------------

/// A `FunctionEntry` with an explicit `WherePredicateDecl` (`where T: Clone`)
/// must emit a `WherePredicate::BoundPredicate` in `Function.generics.where_predicates`
/// with `type_ = Type::Generic("T")`, and the `GenericParamDef.bounds` for that
/// parameter must be empty (ADR D1 — all bounds lifted to where form).
#[test]
fn test_encode_function_with_explicit_where_predicate_emits_bound_predicate() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("where_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        // generic param `T` with no inline bounds
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // explicit where predicate: `where T: Clone`
        vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("where_fn") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for where_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // One type param `T` with empty inline bounds (all bounds lifted to where form).
    assert_eq!(f.generics.params.len(), 1, "expected 1 generic param");
    let param = &f.generics.params[0];
    assert_eq!(param.name, "T");
    let GenericParamDefKind::Type { bounds, .. } = &param.kind else {
        panic!("expected Type kind for param T");
    };
    assert!(
        bounds.is_empty(),
        "GenericParamDef.bounds must be empty (D1: bounds lifted to where form)"
    );

    // One BoundPredicate for `T: Clone` in where_predicates.
    assert_eq!(
        f.generics.where_predicates.len(),
        1,
        "expected 1 where predicate, got {:?}",
        f.generics.where_predicates
    );
    let WherePredicate::BoundPredicate { type_, bounds, .. } = &f.generics.where_predicates[0]
    else {
        panic!("expected BoundPredicate, got {:?}", f.generics.where_predicates[0]);
    };
    assert!(
        matches!(type_, Type::Generic(g) if g == "T"),
        "BoundPredicate LHS must be Type::Generic(\"T\"), got {type_:?}"
    );
    assert!(!bounds.is_empty(), "BoundPredicate bounds must be non-empty");
}

/// A `FunctionEntry` with a non-trivial LHS in a `WherePredicateDecl`
/// (`where Vec<T>: Clone`) must emit a `WherePredicate::BoundPredicate` whose
/// `type_` is NOT `Type::Generic` (it is a resolved-path or generic array).
/// Verifies the non-bare-generic-name branch of `build_where_form_generics`.
#[test]
fn test_encode_function_with_non_trivial_lhs_where_predicate_emits_bound_predicate() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("vec_where_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // explicit where predicate: `where Vec<T>: Clone` — non-trivial LHS
        vec![WherePredicateDecl {
            lhs: TypeRef::new("Vec<T>").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("vec_where_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for vec_where_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // Must have exactly one where predicate (the Vec<T>: Clone entry).
    assert_eq!(
        f.generics.where_predicates.len(),
        1,
        "expected 1 where predicate for `where Vec<T>: Clone`"
    );
    let WherePredicate::BoundPredicate { type_, bounds, .. } = &f.generics.where_predicates[0]
    else {
        panic!("expected BoundPredicate, got {:?}", f.generics.where_predicates[0]);
    };
    // LHS must not be a bare generic; it should be some compound type.
    assert!(
        !matches!(type_, Type::Generic(g) if g == "T"),
        "LHS for `Vec<T>: Clone` must not be Type::Generic(\"T\")"
    );
    assert!(!bounds.is_empty(), "BoundPredicate bounds must be non-empty for Clone");
}

/// A `FunctionEntry` with a `WherePredicateDecl` using `BoundOp::Equal` must produce
/// a `WherePredicate::EqPredicate` (not `BoundPredicate`) in the extended-crate output.
/// Verifies the `Equal` branch of `build_where_form_generics`.
#[test]
fn test_encode_function_with_equal_where_predicate_emits_eq_predicate() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};
    use rustdoc_types::{Term, WherePredicate};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("eq_where_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // explicit where predicate: `where T::Assoc = u32` (Equal operator)
        vec![WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap()],
            operator: BoundOp::Equal,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let ec = encode_doc(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("eq_where_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for eq_where_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    assert_eq!(
        f.generics.where_predicates.len(),
        1,
        "expected 1 where predicate for Equal predicate"
    );
    // Must be EqPredicate, not BoundPredicate.
    assert!(
        matches!(f.generics.where_predicates[0], WherePredicate::EqPredicate { .. }),
        "Equal operator must emit WherePredicate::EqPredicate, got {:?}",
        f.generics.where_predicates[0]
    );
    let WherePredicate::EqPredicate { ref rhs, .. } = f.generics.where_predicates[0] else {
        panic!("expected EqPredicate");
    };
    // RHS must be Term::Type (not Term::Const).
    assert!(matches!(rhs, Term::Type(_)), "EqPredicate rhs must be Term::Type, got {rhs:?}");
}

/// A `FunctionEntry` with a `BoundOp::Equal` predicate and multiple RHS entries
/// must be rejected by `CatalogueToExtendedCrateCodec::encode` with an error.
/// Verifies the defensive rhs.len() == 1 check in `build_where_form_generics`.
#[test]
fn test_encode_function_with_equal_predicate_multiple_rhs_returns_error() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_eq_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // Invalid: Equal with two RHS entries.
        vec![WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap(), TypeRef::new("String").unwrap()],
            operator: BoundOp::Equal,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let result = encode_doc(doc);
    assert!(
        result.is_err(),
        "Equal predicate with multiple rhs must return an error, got: {result:?}"
    );
}

/// A `FunctionEntry` with a `BoundOp::Equal` predicate whose LHS is a bare type
/// parameter (no `::`) must be accepted by `CatalogueToExtendedCrateCodec::encode`
/// (permissive principle, ADR `2026-05-20-0048`).  The JSON codec no longer enforces
/// the `::` invariant on Equal-predicate LHS values, so the encoder must match.
/// The resulting `WherePredicate::EqPredicate` carries a `Type::Generic("T")` LHS.
#[test]
fn test_encode_function_with_equal_predicate_bare_type_param_lhs_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};
    use rustdoc_types::{Term, Type, WherePredicate};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bare_lhs_eq_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // Permissive: bare type parameter as Equal-predicate LHS (`where T = u32`).
        vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap()],
            operator: BoundOp::Equal,
        }],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let ec = encode_doc(doc).expect("bare type param LHS must succeed under permissive principle");
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("bare_lhs_eq_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for bare_lhs_eq_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert_eq!(f.generics.where_predicates.len(), 1);
    // LHS must be Type::Generic("T"); RHS must be Term::Type.
    let WherePredicate::EqPredicate { ref lhs, ref rhs } = f.generics.where_predicates[0] else {
        panic!("expected EqPredicate, got {:?}", f.generics.where_predicates[0]);
    };
    assert!(
        matches!(lhs, Type::Generic(n) if n == "T"),
        "expected Type::Generic(\"T\"), got {lhs:?}"
    );
    assert!(matches!(rhs, Term::Type(_)), "expected Term::Type, got {rhs:?}");
}

// -----------------------------------------------------------------------
// ADR 2026-05-18-1223 D1: validate_supported_bound 撤廃 — lifetime / HRTB /
// precise-capture bounds must be accepted (AC-02)
// -----------------------------------------------------------------------

/// A `FunctionEntry` with a lifetime bound (`'static`) on an inline
/// `MethodGenericParam.bounds` entry must be accepted without error
/// (ADR 2026-05-18-1223 D1 — `validate_supported_bound` abolished).
/// The `GenericBound::Outlives("static")` produced by `parse_generic_bound`
/// must appear in the encoded `BoundPredicate.bounds` for that parameter.
#[test]
fn test_encode_function_with_lifetime_bound_static_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("lifetime_bound_fn").unwrap());
    // `<F: Fn() + Send + Sync + 'static>` — inline bounds include a lifetime bound.
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        vec![MethodGenericParam {
            name: ParamName::new("F").unwrap(),
            bounds: vec![
                TypeRef::new("Fn()").unwrap(),
                TypeRef::new("Send").unwrap(),
                TypeRef::new("Sync").unwrap(),
                // Lifetime bound: must be accepted after validate_supported_bound removal.
                TypeRef::new("'static").unwrap(),
            ],
        }],
        vec![],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let result = encode_doc(doc);
    assert!(
        result.is_ok(),
        "lifetime bound `'static` must be accepted without error, got: {result:?}"
    );

    // Verify the encoded BoundPredicate contains a GenericBound::Outlives("static") entry.
    let ec = result.unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("lifetime_bound_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for lifetime_bound_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // All bounds are lifted to where form (ADR 2026-05-13-1153 D1).
    // The BoundPredicate for `F` must contain a GenericBound::Outlives("'static").
    // The apostrophe is included so that A-codec Outlives strings match the C-side
    // rustdoc representation (rustdoc stores `"'static"` not `"static"`).
    let has_static_outlives = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "F")
                && bounds.iter().any(|b| matches!(b, GenericBound::Outlives(lt) if lt == "'static"))
        } else {
            false
        }
    });
    assert!(
        has_static_outlives,
        "encoded where_predicates must contain Outlives(\"'static\") for `F: 'static`, \
         got: {:?}",
        f.generics.where_predicates
    );
}

/// A `FunctionEntry` with a named lifetime bound (`'a`) on an inline
/// `MethodGenericParam.bounds` entry must be accepted without error
/// (ADR 2026-05-18-1223 D1).
#[test]
fn test_encode_function_with_lifetime_bound_named_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path =
        FunctionPath::at_root(crate_n, FunctionName::new("named_lifetime_bound_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        // `<T: Clone + 'a>` — named lifetime bound.
        vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap(), TypeRef::new("'a").unwrap()],
        }],
        vec![],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let result = encode_doc(doc);
    assert!(
        result.is_ok(),
        "named lifetime bound `'a` must be accepted without error, got: {result:?}"
    );

    // The BoundPredicate for `T` must contain a GenericBound::Outlives("'a").
    // The apostrophe is included so that A-codec Outlives strings match the C-side
    // rustdoc representation (rustdoc stores `"'a"` not `"a"`).
    let ec = result.unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("named_lifetime_bound_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for named_lifetime_bound_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    let has_named_outlives = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "T")
                && bounds.iter().any(|b| matches!(b, GenericBound::Outlives(lt) if lt == "'a"))
        } else {
            false
        }
    });
    assert!(
        has_named_outlives,
        "encoded where_predicates must contain Outlives(\"'a\") for `T: 'a`, \
         got: {:?}",
        f.generics.where_predicates
    );
}

/// A `FunctionEntry` with an HRTB trait bound (`for<'a> Fn(&'a ())`) on an inline
/// `MethodGenericParam.bounds` entry must be accepted without error
/// (ADR 2026-05-18-1223 D1).  The encoded `GenericBound::TraitBound` must carry
/// the `generic_params` field populated with the HRTB binder lifetime.
#[test]
fn test_encode_function_with_hrtb_trait_bound_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("hrtb_bound_fn").unwrap());
    let entry = FunctionEntry::new(
        ItemAction::Add,
        FunctionRole::FreeFunction,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        // `<F: for<'a> Fn(&'a ())>` — HRTB on inline bound.
        vec![MethodGenericParam {
            name: ParamName::new("F").unwrap(),
            bounds: vec![TypeRef::new("for<'a> Fn(&'a ())").unwrap()],
        }],
        vec![],
        None,
        vec![],
        vec![],
    );
    doc.insert_function(fn_path, entry);

    let result = encode_doc(doc);
    assert!(
        result.is_ok(),
        "HRTB trait bound `for<'a> Fn(&'a ())` must be accepted without error, got: {result:?}"
    );

    // The BoundPredicate for `F` must contain a TraitBound with non-empty generic_params
    // (the HRTB binder).
    let ec = result.unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("hrtb_bound_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for hrtb_bound_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    let has_hrtb_trait_bound = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "F")
                && bounds.iter().any(|b| {
                    matches!(b, GenericBound::TraitBound { generic_params, .. }
                        if !generic_params.is_empty())
                })
        } else {
            false
        }
    });
    assert!(
        has_hrtb_trait_bound,
        "encoded where_predicates must contain a TraitBound with non-empty generic_params \
         for `for<'a> Fn(&'a ())`, got: {:?}",
        f.generics.where_predicates
    );
}

#[test]
fn test_encode_hrtb_external_trait_absent_from_authoritative_paths_fails_closed() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let function_path = FunctionPath::at_root(
        CrateName::new("domain").unwrap(),
        FunctionName::new("missing_hrtb_trait").unwrap(),
    );
    doc.insert_function(
        function_path,
        FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![MethodGenericParam {
                name: ParamName::new("F").unwrap(),
                bounds: vec![TypeRef::new("for<'a> ghost::Trait").unwrap()],
            }],
            vec![],
            None,
            vec![],
            vec![],
        ),
    );

    let error = encode_doc(doc).expect_err("missing HRTB trait must fail closed");
    assert!(matches!(error, NewTypeGraphCodecError::UnresolvedIdentifier(_)));
}

// -----------------------------------------------------------------------
// T007 / AC-08: A-codec — impl-block-level generics encoding
// (IN-09: InherentImplDeclV2.impl_generics, TraitImplDeclV2.impl_generics,
//  TraitEntry.generics)
// -----------------------------------------------------------------------

/// T007 AC-08 (a): `TraitEntry` with `generics: [T]` encodes the trait-level
/// generic as a `GenericParamDef` in the Trait item's `Generics`.
///
/// The codec must call `build_where_form_generics` for trait-level generics so that
/// `trait Foo<T>` produces a `Trait` item with one type param in its `generics.params`,
/// not `empty_generics()`.
#[test]
fn test_trait_decl_generics_encoded_correctly() {
    use domain::tddd::catalogue_v2::WherePredicateDecl;
    use rustdoc_types::{GenericParamDefKind, WherePredicate};

    let mut doc = make_doc("domain");
    let method_generic = MethodGenericParam {
        name: ParamName::new("T").unwrap(),
        bounds: vec![TypeRef::new("Clone").unwrap()],
    };
    let where_pred = WherePredicateDecl {
        lhs: TypeRef::new("T").unwrap(),
        operator: domain::tddd::catalogue_v2::BoundOp::Bound,
        rhs: vec![TypeRef::new("Send").unwrap()],
    };
    doc.insert_trait(
        CatalogueEntryKey::try_new("MyTrait".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![method_generic],
            vec![where_pred],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    // Find the Trait item for "MyTrait".
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("MyTrait") && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("MyTrait trait item must be present");
    let ItemEnum::Trait(ref trait_inner) = trait_item.inner else {
        panic!("expected Trait inner");
    };

    // generics.params must have one type param "T".
    assert_eq!(
        trait_inner.generics.params.len(),
        1,
        "TraitEntry with generics:[T] must produce 1 GenericParamDef, got: {:?}",
        trait_inner.generics.params
    );
    assert_eq!(trait_inner.generics.params[0].name, "T");
    assert!(
        matches!(trait_inner.generics.params[0].kind, GenericParamDefKind::Type { .. }),
        "type param T must be GenericParamDefKind::Type"
    );

    // In where-form encoding: bounds from `generics[T].bounds` and `where_predicates`
    // are both emitted as WherePredicate::BoundPredicate entries.
    // The `T: Clone` inline bound and `T: Send` where_predicate should both appear.
    let wp_lhs_strings: Vec<String> = trait_inner
        .generics
        .where_predicates
        .iter()
        .filter_map(|wp| {
            if let WherePredicate::BoundPredicate {
                type_: rustdoc_types::Type::Generic(n), ..
            } = wp
            {
                Some(n.clone())
            } else {
                None
            }
        })
        .collect();
    // Both "T: Clone" (from generics.bounds) and "T: Send" (from where_predicates) land
    // in where_predicates as BoundPredicate entries. They may be merged into one or kept
    // separate depending on build_where_form_generics merging strategy.
    // At minimum, "T" must appear as the LHS of at least one where predicate.
    assert!(
        wp_lhs_strings.contains(&"T".to_string()),
        "T must appear as LHS of at least one WherePredicate::BoundPredicate, got: {:?}",
        trait_inner.generics.where_predicates
    );
}

/// T007 AC-08 (b): `TraitImplDeclV2` with `impl_generics: [T]` encodes the
/// impl-block-level generic as a `GenericParamDef` in the Impl item's `Generics`.
///
/// The codec must use `build_where_form_generics` for impl-block generics so that
/// `impl<T: Send> Trait for Foo` produces an Impl item with `generics.params = [T]`,
/// not `empty_generics()`.
#[test]
fn test_trait_impl_block_generics_encoded_correctly() {
    use rustdoc_types::{GenericParamDefKind, WherePredicate};

    let mut doc = make_doc("domain");
    // ADR `2026-05-20-0048` D1/D2: top-level trait_impls; new API: (trait_ref, for_type).
    let trait_impl = TraitImplDeclV2::from_parts(
        domain::tddd::catalogue_v2::ItemAction::Add,
        TypeRef::new("std::marker::Send").unwrap(),
        TypeRef::new("Foo").unwrap(),
        vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap()],
        }],
        // impl_where_predicates: T: Send (explicit where predicate)
        vec![domain::tddd::catalogue_v2::WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            operator: domain::tddd::catalogue_v2::BoundOp::Bound,
            rhs: vec![TypeRef::new("Send").unwrap()],
        }],
    );

    doc.insert_type(
        CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_trait_impl(trait_impl);

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    // Find the trait Impl item (trait_ is Some).
    let trait_impl_item = krate
        .index
        .values()
        .find(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_some()))
        .expect("must find a trait Impl item");
    let ItemEnum::Impl(ref impl_inner) = trait_impl_item.inner else {
        panic!("expected Impl inner");
    };

    // generics.params must have one type param "T".
    assert_eq!(
        impl_inner.generics.params.len(),
        1,
        "TraitImplDeclV2 with impl_generics:[T] must produce 1 GenericParamDef, got: {:?}",
        impl_inner.generics.params
    );
    assert_eq!(impl_inner.generics.params[0].name, "T");
    assert!(
        matches!(impl_inner.generics.params[0].kind, GenericParamDefKind::Type { .. }),
        "impl generic T must be GenericParamDefKind::Type"
    );

    // where_predicates must contain bound predicates for T (from both impl_generics.bounds
    // and impl_where_predicates).
    let has_t_predicate = impl_inner.generics.where_predicates.iter().any(|wp| {
        matches!(wp, WherePredicate::BoundPredicate { type_: rustdoc_types::Type::Generic(n), .. } if n == "T")
    });
    assert!(
        has_t_predicate,
        "T must appear as LHS of at least one WherePredicate::BoundPredicate in the impl block \
         generics, got: {:?}",
        impl_inner.generics.where_predicates
    );
}

#[test]
fn test_trait_impl_for_type_generic_shadows_same_named_local_type() {
    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("T".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("Port".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let trait_impl = TraitImplDeclV2::from_parts(
        domain::tddd::catalogue_v2::ItemAction::Add,
        TypeRef::new("Port").unwrap(),
        TypeRef::new("T").unwrap(),
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![],
    );
    doc.push_trait_impl(trait_impl);

    let ec = encode_doc(doc).unwrap();
    let trait_impl_item = ec
        .krate()
        .index
        .values()
        .find(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_some()))
        .expect("must find a trait Impl item");
    let ItemEnum::Impl(ref impl_inner) = trait_impl_item.inner else {
        panic!("expected Impl inner");
    };

    assert_eq!(
        impl_inner.for_,
        Type::Generic("T".to_string()),
        "impl<T> Port for T must encode the impl target as generic T, not the local type named T"
    );
}

/// T007 AC-08 (c): `InherentImplDeclV2` entries in `CatalogueDocument::inherent_impls`
/// with `impl_generics: [L, R, W]` are encoded as separate Impl items whose
/// `generics.params` contains L, R, W.
///
/// When an `InherentImplDeclV2` is present, the codec must create a *separate*
/// inherent Impl item (in addition to the type's own TypeEntry-driven impl block,
/// if any). The new Impl item must carry the impl-block-level generics.
#[test]
fn test_inherent_impl_block_generics_encoded_correctly() {
    use domain::tddd::catalogue_v2::entries::InherentImplDeclV2;
    use rustdoc_types::GenericParamDefKind;

    let mut doc = make_doc("domain");

    // Register the type "Bar" so the impl block can reference it.
    doc.insert_type(
        CatalogueEntryKey::try_new("Bar".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    // InherentImplDeclV2 with impl_generics: [L, R, W].
    doc.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Bar".to_owned()).unwrap(),
        vec![
            MethodGenericParam {
                name: ParamName::new("L").unwrap(),
                bounds: vec![TypeRef::new("Send").unwrap()],
            },
            MethodGenericParam { name: ParamName::new("R").unwrap(), bounds: vec![] },
            MethodGenericParam {
                name: ParamName::new("W").unwrap(),
                bounds: vec![TypeRef::new("Sync").unwrap()],
            },
        ],
        vec![],
        vec![],
    ));

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    // Find the inherent Impl item that was produced from the InherentImplDeclV2
    // (trait_ is None, generics.params is non-empty with [L, R, W]).
    let generic_inherent_impl = krate.index.values().find(|item| {
        if let ItemEnum::Impl(i) = &item.inner {
            i.trait_.is_none() && !i.generics.params.is_empty()
        } else {
            false
        }
    });

    let impl_item = generic_inherent_impl
        .expect("must find an inherent Impl item with non-empty generics from InherentImplDeclV2");
    let ItemEnum::Impl(ref impl_inner) = impl_item.inner else {
        panic!("expected Impl inner");
    };

    assert_eq!(
        impl_inner.generics.params.len(),
        3,
        "InherentImplDeclV2 with impl_generics:[L, R, W] must produce 3 GenericParamDefs, got: {:?}",
        impl_inner.generics.params
    );
    let names: Vec<&str> = impl_inner.generics.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["L", "R", "W"], "generic param names must be L, R, W in order");
    for p in &impl_inner.generics.params {
        assert!(
            matches!(p.kind, GenericParamDefKind::Type { .. }),
            "each impl generic must be GenericParamDefKind::Type, got: {:?}",
            p.kind
        );
    }

    // where_predicates coverage: L has bounds:[Send] and W has bounds:[Sync], so two
    // WherePredicate::BoundPredicate entries must be emitted. R has no bounds and must
    // produce a GenericParamDef but no WherePredicate.
    // (build_where_form_generics emits bounds inline on params only if non-empty)
    use rustdoc_types::WherePredicate;
    let wp_names: Vec<String> = impl_inner
        .generics
        .where_predicates
        .iter()
        .filter_map(|wp| {
            if let WherePredicate::BoundPredicate {
                type_: rustdoc_types::Type::Generic(n), ..
            } = wp
            {
                Some(n.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        wp_names.contains(&"L".to_string()),
        "L (bounds:[Send]) must appear as WherePredicate LHS, got: {:?}",
        impl_inner.generics.where_predicates
    );
    assert!(
        wp_names.contains(&"W".to_string()),
        "W (bounds:[Sync]) must appear as WherePredicate LHS, got: {:?}",
        impl_inner.generics.where_predicates
    );
    assert!(
        !wp_names.contains(&"R".to_string()),
        "R (no bounds) must NOT appear as WherePredicate LHS, got: {:?}",
        impl_inner.generics.where_predicates
    );

    // Critical Phase-1 linkage check: the inherent impl's Id must appear in the
    // owning type ("Bar")'s `Struct.impls` list. Without this linkage the impl
    // item exists in the index but the type does not point to it, which means
    // downstream signal evaluation never compares it.
    let impl_id = impl_item.id;
    let bar_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("Bar") && matches!(item.inner, ItemEnum::Struct(_))
        })
        .expect("Bar struct item must be present");
    let ItemEnum::Struct(ref bar_struct) = bar_item.inner else {
        panic!("expected Struct inner for Bar");
    };
    assert!(
        bar_struct.impls.contains(&impl_id),
        "inherent impl Id {:?} must be linked in Bar's Struct.impls, got: {:?}",
        impl_id,
        bar_struct.impls
    );
}

/// T007 AC-08 (regression): a catalogue without `TraitEntry.generics` / `impl_generics`
/// (legacy empty-Vec fields) must encode to an item with `empty_generics()` for trait
/// and impl blocks, preserving the existing (pre-T007) signal evaluation behaviour.
#[test]
fn test_existing_catalogue_no_change_in_signal_for_trait_no_generics() {
    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![], // generics empty = old catalogue
            vec![], // where_predicates empty = old catalogue
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let ec = encode_doc(doc).unwrap();
    let krate = ec.krate();

    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("MyPort") && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("MyPort must be present");
    let ItemEnum::Trait(ref t) = trait_item.inner else { panic!("expected Trait") };

    // No generics declared → empty_generics().
    assert!(
        t.generics.params.is_empty(),
        "trait with no generics must encode to empty params, got: {:?}",
        t.generics.params
    );
    assert!(
        t.generics.where_predicates.is_empty(),
        "trait with no generics must encode to empty where_predicates, got: {:?}",
        t.generics.where_predicates
    );
}

#[test]
fn test_trait_assoc_items_encode_trait_generic_projection_types() {
    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("ProjectionPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![AssocTypeDecl {
                name: TypeName::new("Output").unwrap(),
                bounds: vec![],
                default: Some(TypeRef::new("Vec<T::Item>").unwrap()),
            }],
            vec![AssocConstDecl {
                name: AssocConstName::new("ID").unwrap(),
                ty: TypeRef::new("<T as Iterator>::Item").unwrap(),
                default_value: None,
            }],
            vec![],
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    let krate = encoded.krate();
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("ProjectionPort")
                && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("ProjectionPort trait must be encoded");
    let ItemEnum::Trait(trait_inner) = &trait_item.inner else { panic!("expected Trait") };

    let assoc_type = trait_inner
        .items
        .iter()
        .filter_map(|id| krate.index.get(id))
        .find(|item| item.name.as_deref() == Some("Output"))
        .expect("Output associated type must be linked from Trait.items");
    let ItemEnum::AssocType { type_: Some(Type::ResolvedPath(vec_path)), .. } = &assoc_type.inner
    else {
        panic!("expected assoc type default Vec<T::Item>, got {:?}", assoc_type.inner);
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = vec_path.args.as_deref() else {
        panic!("Vec default must carry generic args: {vec_path:?}");
    };
    let Some(GenericArg::Type(Type::QualifiedPath { name, self_type, trait_, .. })) = args.first()
    else {
        panic!("Vec<T::Item> arg must encode as QualifiedPath, got {args:?}");
    };
    assert_eq!(name, "Item");
    assert!(trait_.is_none(), "T::Item projection must have no explicit trait path");
    assert_eq!(self_type.as_ref(), &Type::Generic("T".to_string()));

    let assoc_const = trait_inner
        .items
        .iter()
        .filter_map(|id| krate.index.get(id))
        .find(|item| item.name.as_deref() == Some("ID"))
        .expect("ID associated const must be linked from Trait.items");
    let ItemEnum::AssocConst { type_: Type::QualifiedPath { name, self_type, trait_, .. }, .. } =
        &assoc_const.inner
    else {
        panic!("expected assoc const type <T as Iterator>::Item, got {:?}", assoc_const.inner);
    };
    assert_eq!(name, "Item");
    assert_eq!(self_type.as_ref(), &Type::Generic("T".to_string()));
    assert!(
        trait_.as_ref().is_some_and(|path| path.path.ends_with("Iterator")),
        "expected Iterator trait path, got {trait_:?}"
    );
}

#[test]
fn test_trait_assoc_items_reject_invalid_trait_generic_projection_name() {
    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("InvalidProjectionPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![AssocTypeDecl {
                name: TypeName::new("Output").unwrap(),
                bounds: vec![],
                default: Some(TypeRef::new("T::Item-foo").unwrap()),
            }],
            vec![],
            vec![],
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let result = encode_doc(doc);
    assert!(
        matches!(result, Err(domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(..))),
        "invalid associated projection names must fall through to parser validation, got {result:?}"
    );
}

#[test]
fn test_trait_assoc_items_resolve_external_ids_inside_explicit_qualified_paths() {
    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("ExternalProjectionPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![AssocConstDecl {
                name: AssocConstName::new("EXTERNAL").unwrap(),
                ty: TypeRef::new("<ext::Foo as ext::Trait>::Assoc").unwrap(),
                default_value: None,
            }],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    let krate = encoded.krate();
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("ExternalProjectionPort")
                && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("ExternalProjectionPort trait must be encoded");
    let ItemEnum::Trait(trait_inner) = &trait_item.inner else { panic!("expected Trait") };

    let assoc_const = trait_inner
        .items
        .iter()
        .filter_map(|id| krate.index.get(id))
        .find(|item| item.name.as_deref() == Some("EXTERNAL"))
        .expect("EXTERNAL associated const must be linked from Trait.items");
    let ItemEnum::AssocConst { type_: Type::QualifiedPath { self_type, trait_, .. }, .. } =
        &assoc_const.inner
    else {
        panic!("expected explicit qualified path type, got {:?}", assoc_const.inner);
    };
    let Type::ResolvedPath(self_path) = self_type.as_ref() else {
        panic!("expected external self type path, got {self_type:?}");
    };
    assert_eq!(self_path.path, "ext::Foo");
    assert_ne!(
        self_path.id,
        Id(UNRESOLVED_CRATE_ID),
        "qualified-path self_type external id must be resolved"
    );
    assert!(
        krate.paths.contains_key(&self_path.id),
        "resolved external self_type id must have a path summary"
    );

    let trait_path = trait_.as_ref().expect("qualified path must keep the explicit trait path");
    assert_eq!(trait_path.path, "ext::Trait");
    assert_ne!(
        trait_path.id,
        Id(UNRESOLVED_CRATE_ID),
        "qualified-path trait external id must be resolved"
    );
    assert!(
        krate.paths.contains_key(&trait_path.id),
        "resolved external trait id must have a path summary"
    );
}

#[test]
fn test_trait_assoc_items_rewrite_nested_trait_generic_projections() {
    fn assert_t_item_projection(ty: &Type, context: &str) {
        let Type::QualifiedPath { name, self_type, trait_, .. } = ty else {
            panic!("{context}: expected T::Item qualified projection, got {ty:?}");
        };
        assert_eq!(name, "Item", "{context}: associated item name");
        assert!(trait_.is_none(), "{context}: T::Item must have no explicit trait path");
        assert_eq!(
            self_type.as_ref(),
            &Type::Generic("T".to_string()),
            "{context}: projection self type"
        );
    }

    fn iterator_item_constraint_type<'a>(path: &'a rustdoc_types::Path, context: &str) -> &'a Type {
        let Some(GenericArgs::AngleBracketed { constraints, .. }) = path.args.as_deref() else {
            panic!("{context}: expected Iterator associated-item constraint args, got {path:?}");
        };
        let item_constraint = constraints
            .iter()
            .find(|constraint| constraint.name == "Item")
            .expect("Iterator<Item = ...> constraint must be present");
        let AssocItemConstraintKind::Equality(Term::Type(ty)) = &item_constraint.binding else {
            panic!("{context}: expected Item equality type, got {:?}", item_constraint.binding);
        };
        ty
    }

    fn assert_iterator_constraint_projects_t_item(path: &rustdoc_types::Path, context: &str) {
        let ty = iterator_item_constraint_type(path, context);
        assert_t_item_projection(ty, context);
    }

    let mut doc = make_doc("domain");
    doc.insert_trait(
        CatalogueEntryKey::try_new("NestedProjectionPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![
                AssocTypeDecl {
                    name: TypeName::new("FnOutput").unwrap(),
                    bounds: vec![],
                    default: Some(TypeRef::new("fn() -> T::Item").unwrap()),
                },
                AssocTypeDecl {
                    name: TypeName::new("ImplOutput").unwrap(),
                    bounds: vec![],
                    default: Some(TypeRef::new("impl Iterator<Item = T::Item>").unwrap()),
                },
                AssocTypeDecl {
                    name: TypeName::new("DynOutput").unwrap(),
                    bounds: vec![],
                    default: Some(TypeRef::new("dyn Iterator<Item = T::Item>").unwrap()),
                },
                AssocTypeDecl {
                    name: TypeName::new("BoundedOutput").unwrap(),
                    bounds: vec![TypeRef::new("Iterator<Item = T::Item>").unwrap()],
                    default: None,
                },
                AssocTypeDecl {
                    name: TypeName::new("ShadowedOutput").unwrap(),
                    bounds: vec![TypeRef::new("Iterator<Item = From>").unwrap()],
                    default: None,
                },
            ],
            vec![],
            vec![],
            vec![
                MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
                MethodGenericParam { name: ParamName::new("From").unwrap(), bounds: vec![] },
            ],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let encoded = encode_doc(doc).unwrap();
    let krate = encoded.krate();
    assert!(
        !krate.external_crates.values().any(|krate| krate.name == "T"),
        "trait generic projection T::Item must not register T as an external crate: {:?}",
        krate.external_crates
    );
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("NestedProjectionPort")
                && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("NestedProjectionPort trait must be encoded");
    let ItemEnum::Trait(trait_inner) = &trait_item.inner else { panic!("expected Trait") };

    let find_assoc_type = |assoc_name: &str| {
        trait_inner
            .items
            .iter()
            .filter_map(|id| krate.index.get(id))
            .find(|item| item.name.as_deref() == Some(assoc_name))
            .unwrap_or_else(|| {
                panic!("{assoc_name} associated type must be linked from Trait.items")
            })
    };

    let ItemEnum::AssocType { type_: Some(Type::FunctionPointer(fn_ptr)), .. } =
        &find_assoc_type("FnOutput").inner
    else {
        panic!("FnOutput must encode as a function pointer");
    };
    let fn_output = fn_ptr.sig.output.as_ref().expect("function pointer must have output type");
    assert_t_item_projection(fn_output, "function pointer output");

    let ItemEnum::AssocType { type_: Some(Type::ImplTrait(bounds)), .. } =
        &find_assoc_type("ImplOutput").inner
    else {
        panic!("ImplOutput must encode as impl Trait");
    };
    let Some(GenericBound::TraitBound { trait_: impl_trait_path, .. }) = bounds.first() else {
        panic!("ImplOutput must carry an Iterator trait bound, got {bounds:?}");
    };
    assert_iterator_constraint_projects_t_item(impl_trait_path, "impl trait constraint");

    let ItemEnum::AssocType { type_: Some(Type::DynTrait(dyn_trait)), .. } =
        &find_assoc_type("DynOutput").inner
    else {
        panic!("DynOutput must encode as dyn Trait");
    };
    let Some(poly_trait) = dyn_trait.traits.first() else {
        panic!("DynOutput must carry an Iterator trait");
    };
    assert_iterator_constraint_projects_t_item(&poly_trait.trait_, "dyn trait constraint");

    let ItemEnum::AssocType { bounds, .. } = &find_assoc_type("BoundedOutput").inner else {
        panic!("BoundedOutput must encode as an associated type");
    };
    let Some(GenericBound::TraitBound { trait_: bounded_trait_path, .. }) = bounds.first() else {
        panic!("BoundedOutput must carry an Iterator trait bound, got {bounds:?}");
    };
    assert_iterator_constraint_projects_t_item(bounded_trait_path, "assoc type bound constraint");

    let ItemEnum::AssocType { bounds, .. } = &find_assoc_type("ShadowedOutput").inner else {
        panic!("ShadowedOutput must encode as an associated type");
    };
    let Some(GenericBound::TraitBound { trait_: shadowed_trait_path, .. }) = bounds.first() else {
        panic!("ShadowedOutput must carry an Iterator trait bound, got {bounds:?}");
    };
    let shadowed_item_type =
        iterator_item_constraint_type(shadowed_trait_path, "shadowed generic bound constraint");
    assert_eq!(
        shadowed_item_type,
        &Type::Generic("From".to_string()),
        "trait generic `From` must shadow the std prelude trait in assoc type bounds"
    );
}

/// ADR `2026-07-02-1345` D6: a `TypeEntry`'s declared `generics` / `where_predicates`
/// must be encoded into the A-side rustdoc `Struct.generics` so the signal comparator
/// observes the declared values. They must NOT be silently dropped (the old
/// `empty_generics()` behaviour) — that is exactly the "silent drop / substitution to
/// another declaration level" D6 forbids.
#[test]
fn test_encode_struct_declared_generics_reach_rustdoc_generics() {
    use domain::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};

    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Container".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("T").unwrap(),
                rhs: vec![TypeRef::new("Send").unwrap()],
                operator: BoundOp::Bound,
            }],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );

    let a = encode_doc(doc).unwrap();
    let krate = a.krate();
    let struct_inner = krate
        .index
        .values()
        .find_map(|item| match &item.inner {
            ItemEnum::Struct(s) => Some(s),
            _ => None,
        })
        .expect("encoded Container struct must be present in the index");

    // Declared generic param `T` reaches the rustdoc struct generics (not empty).
    assert_eq!(
        struct_inner.generics.params.len(),
        1,
        "declared struct generic param must be encoded, not dropped"
    );
    assert_eq!(struct_inner.generics.params[0].name, "T");
    // Declared bounds land in the maximally-desugared where form (symmetric with the
    // trait / function encoders' `build_where_form_generics`).
    assert!(
        !struct_inner.generics.where_predicates.is_empty(),
        "declared struct where-predicate / bound must be encoded, not dropped"
    );
}

#[test]
fn test_encode_public_std_reexports_preserve_adapter_spelling_and_resolve_definition_paths() {
    let mut doc = make_doc("domain");
    insert_empty_enum_type(&mut doc, "Widget");
    for trait_ref in ["std::iter::Iterator", "std::ops::Deref", "std::ops::FnOnce"] {
        doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new(trait_ref.to_owned()).unwrap(),
            TypeRef::new("Widget".to_owned()).unwrap(),
        ));
    }

    // Model the rustdoc authority at the adapter boundary: the public std paths
    // are absent, while rustdoc exposes the defining core paths. The codec keeps
    // the adapter's source spelling, while the canonical identity assertion below
    // verifies that the shared resolver supplies the re-export semantics.
    let mut baseline = authoritative_crate_for_doc(&doc);
    let public_paths = [
        vec!["std".to_owned(), "iter".to_owned(), "Iterator".to_owned()],
        vec!["std".to_owned(), "ops".to_owned(), "Deref".to_owned()],
        vec!["std".to_owned(), "ops".to_owned(), "FnOnce".to_owned()],
    ];
    baseline.paths.retain(|_, summary| !public_paths.contains(&summary.path));
    for (id, path) in [
        (
            Id(10_000),
            vec![
                "core".to_owned(),
                "iter".to_owned(),
                "traits".to_owned(),
                "iterator".to_owned(),
                "Iterator".to_owned(),
            ],
        ),
        (
            Id(10_001),
            vec!["core".to_owned(), "ops".to_owned(), "deref".to_owned(), "Deref".to_owned()],
        ),
        (
            Id(10_002),
            vec!["core".to_owned(), "ops".to_owned(), "function".to_owned(), "FnOnce".to_owned()],
        ),
    ] {
        baseline.paths.insert(id, ItemSummary { crate_id: 0, path, kind: ItemKind::Trait });
    }
    let current = baseline.clone();

    let encoded = CatalogueToExtendedCrateCodec::new()
        .encode(doc, &baseline, &current)
        .expect("public std re-exports resolve through authoritative core paths");

    for (source, expected_identity) in [
        ("std::iter::Iterator", "core::iter::traits::iterator::Iterator"),
        ("std::ops::Deref", "core::ops::deref::Deref"),
        ("std::ops::FnOnce", "core::ops::function::FnOnce"),
    ] {
        let trait_path = encoded
            .krate()
            .index
            .values()
            .find_map(|item| match &item.inner {
                ItemEnum::Impl(impl_item) => {
                    impl_item.trait_.as_ref().filter(|path| path.path == source).cloned()
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected adapter-preserved trait path {source}"));
        assert_eq!(
            encoded.krate().paths[&trait_path.id].path.join("::"),
            source,
            "the adapter retains the source spelling on the emitted rustdoc path"
        );

        let source_ref = TypeRef::new(source.to_owned()).unwrap();
        let identity = crate::tddd::canonical_type_identity::canonicalize_catalogue_type_ref(
            &source_ref,
            &CrateName::new("domain").unwrap(),
            &baseline.paths,
            &[],
        )
        .expect("the shared resolver normalizes the public re-export");
        assert_eq!(identity.as_str(), expected_identity);
    }
}

#[test]
fn test_encode_short_declaration_keys_resolve_through_module_paths_for_impl_and_generics() {
    use domain::tddd::catalogue_v2::entries::InherentImplDeclV2;

    let mut doc = make_doc("domain");
    doc.insert_type(
        CatalogueEntryKey::try_new("Node".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum { variants: vec![] },
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["alpha".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("Port".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["beta".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        TypeRef::new("Option<Node>").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("Port".to_owned()).unwrap(),
        TypeRef::new("Node".to_owned()).unwrap(),
    ));
    doc.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Node".to_owned()).unwrap(),
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![],
        vec![],
    ));

    let encoded = encode_doc(doc).expect("short declaration notation remains resolvable");
    let node_id = item_id_for_path(&encoded, &["domain", "alpha", "Node"]);
    let port_id = item_id_for_path(&encoded, &["domain", "beta", "Port"]);
    let holder_id = item_id_for_path(&encoded, &["domain", "Holder"]);

    let ItemEnum::Struct(holder) = &encoded.krate().index[&holder_id].inner else {
        panic!("expected Holder struct");
    };
    let rustdoc_types::StructKind::Plain { fields, .. } = &holder.kind else {
        panic!("expected named Holder fields");
    };
    let ItemEnum::StructField(Type::ResolvedPath(option_path)) =
        &encoded.krate().index[&fields[0]].inner
    else {
        panic!("expected Option<Node> to encode as a resolved generic path");
    };
    assert_eq!(option_path.path, "std::option::Option");
    let Some(GenericArgs::AngleBracketed { args, .. }) = option_path.args.as_deref() else {
        panic!("expected Option<Node> generic arguments");
    };
    assert!(matches!(
        args.first(),
        Some(GenericArg::Type(Type::ResolvedPath(path))) if path.id == node_id
    ));

    assert!(encoded.krate().index.values().any(|item| {
        matches!(
            &item.inner,
            ItemEnum::Impl(impl_item)
                if impl_item
                    .trait_
                    .as_ref()
                    .is_some_and(|path| path.id == port_id)
                    && matches!(&impl_item.for_, Type::ResolvedPath(path) if path.id == node_id)
        )
    }));
    assert!(encoded.krate().index.values().any(|item| {
        matches!(
            &item.inner,
            ItemEnum::Impl(impl_item)
                if impl_item.trait_.is_none()
                    && impl_item.generics.params.iter().any(|param| param.name == "T")
                    && matches!(&impl_item.for_, Type::ResolvedPath(path) if path.id == node_id)
        )
    }));
}
