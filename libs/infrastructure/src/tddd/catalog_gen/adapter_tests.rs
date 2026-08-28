#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use domain::TrackId;
use domain::schema::{SchemaExport, TypeInfo, TypeKind};
use domain::tddd::LayerId;
use domain::tddd::catalog_gen::{CatalogEntryKind, CatalogImportAction};
use domain::tddd::catalogue_v2::CrateName;
use serde_json::json;
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
        r#"{"version":2,"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#,
    )
    .unwrap();
    let items_dir = workspace.path().join("track/items");
    (workspace, items_dir)
}

fn bin_root_test_resolver(
    workspace_root: &std::path::Path,
    type_path: &str,
) -> Result<super::import_shape::ImportedShape, CatalogError> {
    let (crate_name, module, name) =
        super::import_shape::parse_type_path(type_path).map_err(|error| {
            super::fs_access::port_error(format!("test type path parsing failed: {error}"))
        })?;
    assert_eq!(crate_name, "cli");
    assert_eq!(module, "commands");
    let package = CrateName::new("cli").unwrap();
    let resolution =
        crate::schema_export::bin_target::resolve_rustdoc_root_name(workspace_root, &package)
            .map_err(|error| {
                super::fs_access::port_error(format!("test target resolution failed: {error}"))
            })?;
    let canonical_type_info = TypeInfo::with_module_path(
        name.clone(),
        TypeKind::Struct,
        None,
        vec![],
        format!("{}::{module}", resolution.rustdoc_root_name().as_str()),
    );
    let package_root_alias = TypeInfo::with_module_path(
        name.clone(),
        TypeKind::Struct,
        None,
        vec![],
        format!("{}::{module}", package.as_str()),
    );
    let canonical_module = format!("{}::{module}", resolution.rustdoc_root_name().as_str());
    let schema = SchemaExport::new(
        "cli".to_owned(),
        vec![package_root_alias, canonical_type_info],
        vec![],
        vec![],
        vec![],
    );
    let selected =
        super::import_shape::select_type_for_resolution(&schema, &resolution, &module, &name)
            .ok_or_else(|| super::fs_access::port_error("bin-root test type was not selected"))?;
    assert_eq!(selected.name(), name);
    assert_eq!(selected.module_path(), Some(canonical_module.as_str()));

    Ok(super::import_shape::ImportedShape {
        module_path: module,
        name: selected.name().to_owned(),
        kind: json!({
            "kind": "struct",
            "shape": { "kind": "unit" }
        }),
        methods: vec![],
    })
}

fn setup_bin_import_fixture(
    library_target: bool,
    track_name: &str,
) -> (tempfile::TempDir, std::path::PathBuf, TrackId) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    let manifest = if library_target {
        r#"[package]
name = "cli"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#
    } else {
        r#"[package]
name = "cli"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "sotp"
path = "src/main.rs"
"#
    };
    std::fs::write(workspace.path().join("Cargo.toml"), manifest).unwrap();
    std::fs::write(
        workspace.path().join("src/lib.rs"),
        "pub mod commands { pub struct VerifyCommand; pub struct LibraryCommand; }\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("src/main.rs"),
        "pub mod commands { pub struct VerifyCommand; }\nfn main() {}\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("architecture-rules.json"),
        r#"{"version":2,"layers":[{"crate":"cli","tddd":{"enabled":true,"catalogue_file":"cli-types.json"}}]}"#,
    )
    .unwrap();

    let items_dir = workspace.path().join("track/items");
    let track_id = TrackId::try_new(track_name.to_owned()).unwrap();
    let track_dir = items_dir.join(track_id.as_ref());
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("spec.json"),
        r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Bin import test",
  "scope": {
    "in_scope": [{ "id": "AC-02", "text": "bin import" }],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#,
    )
    .unwrap();
    (workspace, items_dir, track_id)
}

fn unrelated_root_test_resolver(
    workspace_root: &std::path::Path,
    type_path: &str,
) -> Result<super::import_shape::ImportedShape, CatalogError> {
    let (crate_name, module, name) =
        super::import_shape::parse_type_path(type_path).map_err(|error| {
            super::fs_access::port_error(format!("test type path parsing failed: {error}"))
        })?;
    let package = CrateName::new(crate_name.clone()).unwrap();
    let resolution =
        crate::schema_export::bin_target::resolve_rustdoc_root_name(workspace_root, &package)
            .map_err(|error| {
                super::fs_access::port_error(format!("test target resolution failed: {error}"))
            })?;
    let unrelated_root = "unrelated".to_owned();
    let unrelated_path = vec![unrelated_root.clone(), module.clone(), name.clone()];
    let canonical = crate::tddd::canonical_type_identity::canonicalize_rustdoc_root_path(
        &unrelated_path,
        &package,
        Some(resolution.rustdoc_root_name()),
    );
    let expected = vec![package.as_str().to_owned(), module.clone(), name.clone()];
    assert_eq!(
        canonical, unrelated_path,
        "the shared canonicalizer must not rewrite an unknown root"
    );
    assert_ne!(canonical, expected, "an unrelated root must remain outside the package identity");

    let type_info = TypeInfo::with_module_path(
        name.clone(),
        TypeKind::Struct,
        None,
        vec![],
        format!("{}::{module}::{name}", unrelated_root),
    );
    let schema = SchemaExport::new(crate_name, vec![type_info], vec![], vec![], vec![]);
    assert!(
        super::import_shape::select_type_for_resolution(&schema, &resolution, &module, &name)
            .is_none(),
        "the import selector must fail closed for an unrelated root"
    );
    Err(super::fs_access::schema_error(
        "unrelated rustdoc root does not match the shared package canonicalization",
    ))
}

#[test]
fn test_catalog_import_matches_bin_root_through_shared_identity_canonicalization() {
    let (_fixture_workspace, items_dir) = setup_items_dir();
    let adapter = FsCatalogAdapter::new();
    let track_id = track_id();
    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    write_spec(&items_dir);
    let import = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::commands::VerifyCommand".to_owned(),
            action: CatalogImportAction::Delete,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(import.entry_key, "domain::commands::VerifyCommand");

    let workspace_root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let resolution = crate::schema_export::bin_target::resolve_rustdoc_root_name(
        &workspace_root,
        &CrateName::new("cli").unwrap(),
    )
    .unwrap();
    let type_info = TypeInfo::with_module_path(
        "VerifyCommand".to_owned(),
        TypeKind::Struct,
        None,
        vec![],
        format!("{}::commands::verify", resolution.rustdoc_root_name().as_str()),
    );
    let schema = SchemaExport::new("cli".to_owned(), vec![type_info], vec![], vec![], vec![]);

    let selected = super::import_shape::select_type_for_resolution(
        &schema,
        &resolution,
        "commands::verify",
        "VerifyCommand",
    );
    assert!(selected.is_some(), "catalog import must compare canonical package-root paths");
}

#[test]
fn test_catalog_import_all_actions_and_function_resolve_bin_root_through_adapter() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("Cargo.toml"),
        r#"[package]
name = "cli"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "sotp"
path = "src/main.rs"
"#,
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("src/main.rs"),
        "pub mod commands { pub struct VerifyCommand; }\nfn main() {}\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("architecture-rules.json"),
        r#"{"version":2,"layers":[{"crate":"cli","tddd":{"enabled":true,"catalogue_file":"cli-types.json"}}]}"#,
    )
    .unwrap();

    let items_dir = workspace.path().join("track/items");
    let track_id = TrackId::try_new("bin-import-track".to_owned()).unwrap();
    let track_dir = items_dir.join(track_id.as_ref());
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("spec.json"),
        r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Bin import test",
  "scope": {
    "in_scope": [{ "id": "AC-02", "text": "bin import" }],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#,
    )
    .unwrap();

    let adapter = FsCatalogAdapter::with_import_resolver(bin_root_test_resolver);
    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    let added_type = CatalogPort::add(
        &adapter,
        &track_id,
        &items_dir,
        CatalogAddCommand {
            layer: LayerId::try_new("cli").unwrap(),
            kind: CatalogEntryKind::Struct,
            name: "cli::commands::AddedCommand".to_owned(),
            role: "ValueObject".to_owned(),
            anchors: vec!["AC-02".to_owned()],
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
        },
    )
    .unwrap();
    assert_eq!(added_type.entry_key, "cli::commands::AddedCommand");

    let added_function = CatalogPort::add(
        &adapter,
        &track_id,
        &items_dir,
        CatalogAddCommand {
            layer: LayerId::try_new("cli").unwrap(),
            kind: CatalogEntryKind::Function,
            name: "cli::commands::run".to_owned(),
            role: "FreeFunction".to_owned(),
            anchors: vec!["AC-02".to_owned()],
            fields: vec![],
            methods: vec!["fn run(input: u32) -> bool".to_owned()],
            variants: vec![],
            trait_impls: vec![],
            inherent_methods: vec![],
            generics: vec![],
            where_predicates: vec![],
            impl_generics: vec![],
            impl_where_predicates: vec![],
            inherent_impl_generics: vec![],
            inherent_impl_where_predicates: vec![],
        },
    )
    .unwrap();
    assert_eq!(added_function.entry_key, "cli::commands::run");

    let modified = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("cli").unwrap(),
            type_path: "cli::commands::VerifyCommand".to_owned(),
            action: CatalogImportAction::Modify,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(modified.entry_key, "cli::commands::VerifyCommand");

    let referenced = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("cli").unwrap(),
            type_path: "cli::commands::ReferenceCommand".to_owned(),
            action: CatalogImportAction::Reference,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(referenced.entry_key, "cli::commands::ReferenceCommand");

    let catalogue: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(track_dir.join("cli-types.json")).unwrap())
            .unwrap();
    assert_eq!(catalogue["types"]["cli::commands::AddedCommand"]["action"], "add");
    for (entry_key, action) in [
        ("cli::commands::VerifyCommand", "modify"),
        ("cli::commands::ReferenceCommand", "reference"),
    ] {
        let entry = &catalogue["types"][entry_key];
        assert_eq!(entry["action"], action);
        assert_eq!(entry["module_path"], "commands");
        assert_eq!(entry["spec_refs"][0]["anchor"], "AC-02");
    }
    assert_eq!(catalogue["functions"]["cli::commands::run"]["action"], "add");
}

#[test]
fn test_catalog_import_key_equals_shared_root_canonicalization_byte_for_byte() {
    let (workspace, items_dir, track_id) = setup_bin_import_fixture(false, "bin-equality-track");
    let adapter = FsCatalogAdapter::with_import_resolver(bin_root_test_resolver);
    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();

    let package = CrateName::new("cli").unwrap();
    let resolution =
        crate::schema_export::bin_target::resolve_rustdoc_root_name(workspace.path(), &package)
            .unwrap();
    let rustdoc_path = vec![
        resolution.rustdoc_root_name().as_str().to_owned(),
        "commands".to_owned(),
        "VerifyCommand".to_owned(),
    ];
    let canonical_key = crate::tddd::canonical_type_identity::canonicalize_rustdoc_root_path(
        &rustdoc_path,
        &package,
        Some(resolution.rustdoc_root_name()),
    )
    .join("::");
    let report = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("cli").unwrap(),
            type_path: "cli::commands::VerifyCommand".to_owned(),
            action: CatalogImportAction::Modify,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();

    assert_eq!(report.entry_key.as_bytes(), canonical_key.as_bytes());
}

#[test]
fn test_catalog_import_root_alias_is_single_application_and_library_root_is_idempotent() {
    let (_workspace, items_dir, track_id) = setup_bin_import_fixture(false, "bin-idempotent-track");
    let adapter = FsCatalogAdapter::with_import_resolver(bin_root_test_resolver);
    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    let bin_report = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("cli").unwrap(),
            type_path: "cli::commands::VerifyCommand".to_owned(),
            action: CatalogImportAction::Reference,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(bin_report.entry_key, "cli::commands::VerifyCommand");
    assert!(!bin_report.entry_key.contains("cli::cli::"));
    assert!(!bin_report.entry_key.starts_with("sotp::"));

    let (library_workspace, library_items, library_track) =
        setup_bin_import_fixture(true, "library-idempotent-track");
    let library_adapter = FsCatalogAdapter::with_import_resolver(bin_root_test_resolver);
    CatalogPort::init(&library_adapter, &library_track, &library_items).unwrap();
    let library_report = CatalogPort::import(
        &library_adapter,
        &library_track,
        &library_items,
        CatalogImportCommand {
            layer: LayerId::try_new("cli").unwrap(),
            type_path: "cli::commands::LibraryCommand".to_owned(),
            action: CatalogImportAction::Reference,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    let package = CrateName::new("cli").unwrap();
    let resolution = crate::schema_export::bin_target::resolve_rustdoc_root_name(
        library_workspace.path(),
        &package,
    )
    .unwrap();
    let library_path = vec![
        resolution.rustdoc_root_name().as_str().to_owned(),
        "commands".to_owned(),
        "LibraryCommand".to_owned(),
    ];
    let expected_library_key =
        crate::tddd::canonical_type_identity::canonicalize_rustdoc_root_path(
            &library_path,
            &package,
            Some(resolution.rustdoc_root_name()),
        )
        .join("::");
    assert_eq!(library_report.entry_key, expected_library_key);
    assert_eq!(library_report.entry_key, "cli::commands::LibraryCommand");
}

#[test]
fn test_catalog_import_unrelated_root_fails_with_shared_canonicalization_error() {
    let (_workspace, items_dir, track_id) = setup_bin_import_fixture(false, "unrelated-root-track");
    let adapter = FsCatalogAdapter::with_import_resolver(unrelated_root_test_resolver);
    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    let error = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("cli").unwrap(),
            type_path: "cli::commands::VerifyCommand".to_owned(),
            action: CatalogImportAction::Modify,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .expect_err("an unrelated rustdoc root must fail closed");
    assert!(matches!(error, CatalogError::SchemaInvalid { .. }));
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
        name: "domain::alpha::Shared".to_owned(),
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
      { "id": "AC-01", "text": "adapter acceptance" },
      { "id": "AC-02", "text": "second adapter acceptance" }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 3, "yellow": 0, "red": 0 }
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
    assert_eq!(add.entry_key, "domain::alpha::Shared");

    let cite = CatalogPort::cite(
        &adapter,
        &track_id,
        &items_dir,
        CatalogCiteCommand {
            layer: LayerId::try_new("domain").unwrap(),
            entry: "domain::alpha::Shared".to_owned(),
            anchors: vec!["AC-01".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(cite.file_path, expected_file.display().to_string());
    assert_eq!(cite.entry_key, "domain::alpha::Shared");

    let import = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::beta::Shared".to_owned(),
            action: CatalogImportAction::Delete,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(import.file_path, expected_file.display().to_string());
    assert_eq!(import.entry_key, "domain::beta::Shared");

    let catalogue: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&expected_file).unwrap()).unwrap();
    let types = catalogue
        .get("types")
        .and_then(serde_json::Value::as_object)
        .expect("catalogue must contain a types object");
    assert!(types.contains_key("domain::alpha::Shared"));
    assert!(types.contains_key("domain::beta::Shared"));

    let anchors_for = |entry_key: &str| {
        types
            .get(entry_key)
            .and_then(|entry| entry.get("spec_refs"))
            .and_then(serde_json::Value::as_array)
            .expect("catalogue entry must contain spec_refs")
            .iter()
            .filter_map(|reference| reference.get("anchor"))
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
    };
    let alpha_anchors = anchors_for("domain::alpha::Shared");
    let beta_anchors = anchors_for("domain::beta::Shared");
    assert!(alpha_anchors.contains(&"AC-01"));
    assert!(!alpha_anchors.contains(&"AC-02"));
    assert!(beta_anchors.contains(&"AC-02"));
    assert!(!beta_anchors.contains(&"AC-01"));
}

#[test]
fn test_fs_catalog_adapter_keeps_duplicate_names_distinct_across_writes() {
    let (_workspace, items_dir) = setup_items_dir();
    let adapter = FsCatalogAdapter::new();
    let track_id = track_id();
    let expected_file = items_dir.join(TRACK_ID).join("domain-types.json");

    CatalogPort::init(&adapter, &track_id, &items_dir).unwrap();
    write_spec(&items_dir);

    let first = CatalogPort::add(&adapter, &track_id, &items_dir, add_command()).unwrap();
    let mut second_command = add_command();
    second_command.name = "domain::beta::Shared".to_owned();
    let second = CatalogPort::add(&adapter, &track_id, &items_dir, second_command).unwrap();
    assert_eq!(first.entry_key, "domain::alpha::Shared");
    assert_eq!(second.entry_key, "domain::beta::Shared");

    let cite = CatalogPort::cite(
        &adapter,
        &track_id,
        &items_dir,
        CatalogCiteCommand {
            layer: LayerId::try_new("domain").unwrap(),
            entry: "domain::beta::Shared".to_owned(),
            anchors: vec!["AC-01".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(cite.entry_key, "domain::beta::Shared");

    let import = CatalogPort::import(
        &adapter,
        &track_id,
        &items_dir,
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::gamma::Shared".to_owned(),
            action: CatalogImportAction::Delete,
            anchors: vec!["AC-02".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(import.entry_key, "domain::gamma::Shared");

    let catalogue: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&expected_file).unwrap()).unwrap();
    let types = catalogue
        .get("types")
        .and_then(serde_json::Value::as_object)
        .expect("catalogue must contain types");
    assert_eq!(types.len(), 3);
    for entry_key in ["domain::alpha::Shared", "domain::beta::Shared", "domain::gamma::Shared"] {
        assert!(types.contains_key(entry_key), "missing qualified entry {entry_key}");
    }

    let anchors_for = |entry_key: &str| {
        types
            .get(entry_key)
            .and_then(|entry| entry.get("spec_refs"))
            .and_then(serde_json::Value::as_array)
            .expect("entry must contain spec_refs")
            .iter()
            .filter_map(|reference| reference.get("anchor"))
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
    };
    assert!(anchors_for("domain::alpha::Shared").contains(&"IN-01"));
    assert!(!anchors_for("domain::alpha::Shared").contains(&"AC-01"));
    assert!(anchors_for("domain::beta::Shared").contains(&"AC-01"));
    assert!(!anchors_for("domain::beta::Shared").contains(&"AC-02"));
    assert!(anchors_for("domain::gamma::Shared").contains(&"AC-02"));
    assert!(!anchors_for("domain::gamma::Shared").contains(&"AC-01"));
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
