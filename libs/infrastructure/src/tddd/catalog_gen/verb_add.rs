//! `sotp catalog add` (D2 / D6 / IN-03 / AC-03 / AC-08): append an annotated
//! entry skeleton for a new type to a layer's catalogue.

use std::path::Path;

use domain::tddd::catalog_gen::CatalogEntryName;
use serde_json::Value;
use usecase::catalog_gen::{CatalogAddCommand, CatalogError, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, insert_entry, load_bindings, read_catalogue, scan_entry_holes, schema_error,
    section_for_kind, track_dir, write_catalogue,
};
use super::json_build::build_add_entry;
use super::validate::load_spec_anchors;

/// Add a new-type entry skeleton to the layer's catalogue.
///
/// # Errors
///
/// Returns a [`CatalogError`] on missing file, invalid input, duplicate entry,
/// or filesystem failure.
pub(super) fn run(
    track_id: &str,
    items_dir: &Path,
    command: CatalogAddCommand,
) -> Result<CatalogWriteReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    let path = catalogue_path(&dir, &bindings, &command.layer);
    let spec_file = format!("{}/spec.json", dir.display());
    let spec_anchors = load_spec_anchors(&dir, items_dir)?;
    add_entry_to_file(&path, items_dir, &command, &spec_file, &spec_anchors)
}

/// Insert the built entry into an existing catalogue file (testable core).
fn add_entry_to_file(
    path: &Path,
    trusted_root: &Path,
    command: &CatalogAddCommand,
    spec_file: &str,
    spec_anchors: &std::collections::BTreeSet<domain::plan_ref::SpecElementId>,
) -> Result<CatalogWriteReport, CatalogError> {
    let mut document = read_catalogue(path, trusted_root)?;
    let name = CatalogEntryName::try_new(command.name.clone())
        .map_err(|err| schema_error(format!("invalid entry name `{}`: {err}", command.name)))?;
    let (entry, trait_impls) = build_add_entry(command, spec_file, spec_anchors)?;
    let section = section_for_kind(command.kind);
    insert_entry(&mut document, section, &name, entry)?;
    append_trait_impls(&mut document, trait_impls);
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, section, name.as_str());
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: name.as_str().to_owned(),
        holes,
    })
}

/// Append document-level trait-impl declarations, if any.
fn append_trait_impls(document: &mut Value, trait_impls: Vec<Value>) {
    if trait_impls.is_empty() {
        return;
    }
    if let Some(root) = document.as_object_mut() {
        let list = root.entry("trait_impls".to_owned()).or_insert_with(|| Value::Array(Vec::new()));
        if let Some(array) = list.as_array_mut() {
            array.extend(trait_impls);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::collections::BTreeSet;

    use domain::plan_ref::SpecElementId;
    use domain::tddd::LayerId;
    use domain::tddd::catalog_gen::CatalogEntryKind;

    use super::*;
    use crate::tddd::catalog_gen::scan_todo_holes;

    fn anchors() -> BTreeSet<SpecElementId> {
        let mut set = BTreeSet::new();
        set.insert(SpecElementId::try_new("IN-01").unwrap());
        set
    }

    fn seed_catalogue(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("domain-types.json");
        let value = serde_json::json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": {},
            "traits": {},
            "functions": {}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        path
    }

    fn struct_command(name: &str) -> CatalogAddCommand {
        CatalogAddCommand {
            layer: LayerId::try_new("domain").unwrap(),
            kind: CatalogEntryKind::Struct,
            name: name.to_owned(),
            role: "ValueObject".to_owned(),
            anchors: vec!["IN-01".to_owned()],
            fields: vec!["count: u32".to_owned()],
            methods: vec![],
            variants: vec![],
            trait_impls: vec![],
            generics: vec![],
            where_predicates: vec![],
            impl_generics: vec![],
            impl_where_predicates: vec![],
        }
    }

    #[test]
    fn test_add_entry_happy_path() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let report =
            add_entry_to_file(&path, temp.path(), &struct_command("Foo"), "spec.json", &anchors())
                .unwrap();
        assert_eq!(report.entry_key, "Foo");
        assert!(!report.holes.is_empty());

        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(written["types"].get("Foo").is_some());
        let holes = scan_todo_holes(&written);
        assert!(holes.iter().any(|hole| hole.path().as_str().contains("Foo")));
    }

    #[test]
    fn test_add_entry_duplicate_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        add_entry_to_file(&path, temp.path(), &struct_command("Foo"), "spec.json", &anchors())
            .unwrap();
        let err =
            add_entry_to_file(&path, temp.path(), &struct_command("Foo"), "spec.json", &anchors())
                .unwrap_err();
        assert!(matches!(err, CatalogError::DuplicateEntry { .. }));
    }

    #[test]
    fn test_add_entry_missing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("absent-types.json");
        let err =
            add_entry_to_file(&path, temp.path(), &struct_command("Foo"), "spec.json", &anchors())
                .unwrap_err();
        assert!(matches!(err, CatalogError::FileMissing { .. }));
    }
}
