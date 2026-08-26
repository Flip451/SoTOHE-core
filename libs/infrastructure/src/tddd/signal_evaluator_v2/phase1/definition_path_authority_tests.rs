//! Regression tests for the Phase 1 definition-path authority.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::composite::{
    StructKind as CatalogueStructKind, StructShape, TypeKindV2,
};
use domain::tddd::catalogue_v2::entries::TypeEntry;
use domain::tddd::catalogue_v2::roles::DataRole;
use domain::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey, CrateName, ItemAction};
use domain::tddd::{CatalogueToExtendedCratePort, ExtendedCrate, LayerId};
use rustdoc_types::{
    Crate, FORMAT_VERSION, Generics, Id, Item, ItemEnum, ItemKind, ItemSummary, Module, Struct,
    StructKind, Target, Visibility,
};

use super::builder::phase1_build_s_and_d_with_rustdoc_root;
use crate::schema_export::bin_target::resolve_rustdoc_root_name;

fn root_item(id: Id, name: &str, children: Vec<Id>) -> Item {
    Item {
        id,
        crate_id: 0,
        name: Some(name.to_owned()),
        span: None,
        visibility: Visibility::Public,
        docs: None,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner: ItemEnum::Module(Module { is_crate: true, items: children, is_stripped: false }),
    }
}

fn struct_item(id: Id, name: &str) -> Item {
    Item {
        id,
        crate_id: 0,
        name: Some(name.to_owned()),
        span: None,
        visibility: Visibility::Public,
        docs: None,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner: ItemEnum::Struct(Struct {
            kind: StructKind::Unit,
            generics: Generics { params: vec![], where_predicates: vec![] },
            impls: vec![],
        }),
    }
}

fn crate_with_struct(root_name: &str, path_root: &str) -> Crate {
    Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: HashMap::from([
            (Id(0), root_item(Id(0), root_name, vec![Id(1)])),
            (Id(1), struct_item(Id(1), "Holder")),
        ]),
        paths: HashMap::from([(
            Id(1),
            ItemSummary {
                crate_id: 0,
                path: vec![path_root.to_owned(), "Holder".to_owned()],
                kind: ItemKind::Struct,
            },
        )]),
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

fn catalogue_graph_with_modified_holder() -> ExtendedCrate {
    ExtendedCrate::new(
        Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::from([
                (Id(0), root_item(Id(0), "cli", vec![Id(1)])),
                (Id(1), struct_item(Id(1), "Holder")),
            ]),
            paths: HashMap::from([(
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["cli".to_owned(), "Holder".to_owned()],
                    kind: ItemKind::Struct,
                },
            )]),
            external_crates: HashMap::new(),
            format_version: FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        },
        BTreeMap::from([(Id(1), ItemAction::Modify)]),
    )
}

fn cli_bin_resolution() -> crate::schema_export::bin_target::RustdocTargetResolution {
    let workspace_root = test_workspace_root();
    resolve_rustdoc_root_name(
        &workspace_root,
        &CrateName::new("cli").expect("cli is a valid package name"),
    )
    .expect("the workspace must expose the cli binary target")
}

fn test_workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist")
}

#[test]
fn test_phase1_definition_path_authority_canonicalizes_bin_root_for_modified_type() {
    let baseline = crate_with_struct("sotp", "sotp");
    let resolution = cli_bin_resolution();

    let (s, _d) = phase1_build_s_and_d_with_rustdoc_root(
        catalogue_graph_with_modified_holder(),
        &baseline,
        Some(&resolution),
    )
    .expect("a package-qualified modify must match the bin rustdoc root");

    let identities = crate::tddd::signal_evaluator_v2::build_type_trait_identity_map(s.krate())
        .expect("Phase 1 must retain the canonical type identity");
    assert!(identities.contains_key("cli::Holder"));
    assert_eq!(
        s.action_for(identities.get("cli::Holder").expect("Holder identity")),
        Some(ItemAction::Modify)
    );
}

fn empty_crate() -> Crate {
    Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: HashMap::new(),
        paths: HashMap::new(),
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

#[test]
fn test_phase1_preserves_unplaced_add_marker_in_s_paths() {
    let baseline = empty_crate();
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").expect("domain is a valid crate name"),
        LayerId::try_new("domain").expect("domain is a valid layer"),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("Future".to_owned()).expect("valid type key"),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(CatalogueStructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            None,
            None,
            vec![],
            vec![],
        ),
    );
    let a = crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec::new()
        .encode(catalogue, &baseline, &baseline)
        .expect("the production codec must emit the unplaced add");

    let (s, _d) = super::builder::phase1_build_s_and_d(a, &baseline)
        .expect("Phase 1 must retain the unplaced add");
    let summary = s
        .krate()
        .paths
        .values()
        .find(|summary| summary.path == ["domain", "Future"])
        .expect("the unplaced add must have an S path summary");

    assert_eq!(summary.crate_id, crate::tddd::canonical_type_identity::SYNTHETIC_UNPLACED_CRATE_ID);
}
