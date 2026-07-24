#![allow(clippy::unwrap_used, clippy::expect_used)]

use domain::TrackId;
use domain::tddd::LayerId;
use domain::tddd::catalog_gen::{CatalogEntryKind, CatalogImportAction};
use usecase::catalog_gen::{
    CatalogAddCommand, CatalogCheckQuery, CatalogCheckVerdict, CatalogCiteCommand, CatalogError,
    CatalogImportCommand, CatalogPort,
};

use super::FsCatalogAdapter;

const TRACK_ID: &str = "adapter-track";

fn setup_items_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("architecture-rules.json"),
        r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#,
    )
    .unwrap();
    let items_dir = workspace.path().join("track/items");
    (workspace, items_dir)
}

fn track_id() -> TrackId {
    TrackId::try_new(TRACK_ID.to_owned()).unwrap()
}

fn invalid_layer() -> LayerId {
    LayerId::try_new("unconfigured").unwrap()
}

fn invalid_layer_add_command() -> CatalogAddCommand {
    CatalogAddCommand {
        layer: invalid_layer(),
        kind: CatalogEntryKind::Struct,
        name: "AdapterEntry".to_owned(),
        role: "ValueObject".to_owned(),
        anchors: vec![],
        fields: vec![],
        methods: vec![],
        variants: vec![],
        trait_impls: vec![],
        inherent_methods: vec![],
        generics: vec![],
        where_predicates: vec![],
        impl_generics: vec![],
        impl_where_predicates: vec![],
        inherent_impl_generics: vec![],
        inherent_impl_where_predicates: vec![],
    }
}

fn add_command() -> CatalogAddCommand {
    CatalogAddCommand {
        layer: LayerId::try_new("domain").unwrap(),
        kind: CatalogEntryKind::Struct,
        name: "AdapterEntry".to_owned(),
        role: "ValueObject".to_owned(),
        anchors: vec!["IN-01".to_owned()],
        fields: vec![],
        methods: vec![],
        variants: vec![],
        trait_impls: vec![],
        inherent_methods: vec![],
        generics: vec![],
        where_predicates: vec![],
        impl_generics: vec![],
        impl_where_predicates: vec![],
        inherent_impl_generics: vec![],
        inherent_impl_where_predicates: vec![],
    }
}

fn write_spec(items_dir: &std::path::Path) {
    let track_dir = items_dir.join(TRACK_ID);
    std::fs::write(
        track_dir.join("spec.json"),
        r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Adapter test",
  "scope": {
    "in_scope": [
      { "id": "IN-01", "text": "adapter behavior" },
      { "id": "AC-01", "text": "adapter acceptance" }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 2, "yellow": 0, "red": 0 }
}"#,
    )
    .unwrap();
}

#[test]
fn test_fs_catalog_adapter_init_and_check_use_validated_track_path() {
    let (_workspace, items_dir) = setup_items_dir();
    let adapter = FsCatalogAdapter::new();
    let track_id = track_id();
    let expected_file = items_dir.join(TRACK_ID).join("domain-types.json");

    let init = CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    assert_eq!(init.created_files, vec![expected_file.display().to_string()]);
    assert!(expected_file.exists());

    let check =
        CatalogPort::check(&adapter, &track_id, &items_dir, CatalogCheckQuery { layer: None })
            .unwrap();
    assert_eq!(check.verdict, CatalogCheckVerdict::Pass);
    assert!(check.findings.is_empty());

    let err = CatalogPort::init(&adapter, &track_id, &items_dir).unwrap_err();
    assert!(matches!(err, CatalogError::FileExists { path } if path == expected_file));
}

#[test]
fn test_fs_catalog_adapter_add_import_and_cite_write_track_catalogue() {
    let (_workspace, items_dir) = setup_items_dir();
    let adapter = FsCatalogAdapter::new();
    let track_id = track_id();
    let expected_file = items_dir.join(TRACK_ID).join("domain-types.json");

    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    write_spec(&items_dir);

    let add = CatalogPort::add(&adapter, &track_id, &items_dir, add_command()).unwrap();
    assert_eq!(add.file_path, expected_file.display().to_string());
    assert_eq!(add.entry_key, "AdapterEntry");

    let cite = CatalogPort::cite(
        &adapter,
        &track_id,
        &items_dir,
        CatalogCiteCommand {
            layer: LayerId::try_new("domain").unwrap(),
            entry: "AdapterEntry".to_owned(),
            anchors: vec!["AC-01".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(cite.file_path, expected_file.display().to_string());
    assert_eq!(cite.entry_key, "AdapterEntry");

    let import = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::RemovedEntry".to_owned(),
            action: CatalogImportAction::Delete,
            anchors: vec!["AC-01".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(import.file_path, expected_file.display().to_string());
    assert_eq!(import.entry_key, "RemovedEntry");
}

#[test]
fn test_fs_catalog_adapter_operations_preserve_invalid_layer_errors() {
    let (_workspace, items_dir) = setup_items_dir();
    let adapter = FsCatalogAdapter::new();
    let track_id = track_id();

    let add =
        CatalogPort::add(&adapter, &track_id, &items_dir, invalid_layer_add_command()).unwrap_err();
    assert!(matches!(add, CatalogError::SchemaInvalid { .. }));

    let import = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: invalid_layer(),
            type_path: "domain::AdapterEntry".to_owned(),
            action: CatalogImportAction::Reference,
            anchors: vec![],
        },
    )
    .unwrap_err();
    assert!(matches!(import, CatalogError::SchemaInvalid { .. }));

    let cite = CatalogPort::cite(
        &adapter,
        &track_id,
        &items_dir,
        CatalogCiteCommand {
            layer: invalid_layer(),
            entry: "AdapterEntry".to_owned(),
            anchors: vec![],
        },
    )
    .unwrap_err();
    assert!(matches!(cite, CatalogError::SchemaInvalid { .. }));

    let check = CatalogPort::check(
        &adapter,
        &track_id,
        &items_dir,
        CatalogCheckQuery { layer: Some(invalid_layer()) },
    )
    .unwrap_err();
    assert!(matches!(check, CatalogError::SchemaInvalid { .. }));
}

#[test]
fn test_fs_catalog_adapter_uses_catalog_port_without_compatibility_delegation() {
    let production_source = include_str!("mod.rs");

    assert!(production_source.contains("impl CatalogPort for FsCatalogAdapter"));
    for direct_port_operation in
        ["verb_init::run", "verb_add::run", "verb_import::run", "verb_cite::run", "verb_check::run"]
    {
        assert!(
            production_source.contains(direct_port_operation),
            "adapter must directly execute {direct_port_operation}"
        );
    }
    for forbidden_runtime_path in ["ServiceImpl", "CompositionRoot", "CatalogInteractor"] {
        assert!(
            !production_source.contains(forbidden_runtime_path),
            "filesystem adapter must not reverse-delegate through {forbidden_runtime_path}"
        );
    }
}
