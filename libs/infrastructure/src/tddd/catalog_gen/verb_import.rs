//! `sotp catalog import` (D3 / IN-04 / AC-04): take an existing type's current
//! shape from rustdoc extraction into a layer's catalogue.

use std::collections::BTreeSet;
use std::path::Path;

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::CatalogEntryName;
use usecase::catalog_gen::{CatalogError, CatalogImportCommand, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, insert_entry, load_bindings, read_catalogue, scan_entry_holes, schema_error,
    track_dir, workspace_root, write_catalogue,
};
use super::import_shape::{ImportedShape, build_import_entry, resolve_shape};
use super::validate::load_spec_anchors;

/// Import an existing type into the layer's catalogue.
///
/// # Errors
///
/// Returns a [`CatalogError`] on missing file, rustdoc-resolution failure,
/// duplicate entry, unresolved anchor, or filesystem failure.
pub(super) fn run(
    track_id: &str,
    items_dir: &Path,
    command: CatalogImportCommand,
) -> Result<CatalogWriteReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    let path = catalogue_path(&dir, &bindings, &command.layer);
    let spec_file = format!("{}/spec.json", dir.display());
    let spec_anchors = load_spec_anchors(&dir, items_dir)?;
    let root = workspace_root(items_dir);
    let shape = resolve_shape(&root, &command.type_path)?;
    import_entry_to_file(&path, items_dir, &command, &shape, &spec_file, &spec_anchors)
}

/// Insert the resolved import entry into an existing catalogue file
/// (testable core; the rustdoc resolution is separated into
/// [`resolve_shape`]).
fn import_entry_to_file(
    path: &Path,
    trusted_root: &Path,
    command: &CatalogImportCommand,
    shape: &ImportedShape,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<CatalogWriteReport, CatalogError> {
    let mut document = read_catalogue(path, trusted_root)?;
    let name = CatalogEntryName::try_new(shape.name.clone())
        .map_err(|err| schema_error(format!("invalid entry name `{}`: {err}", shape.name)))?;
    let entry = build_import_entry(command, shape, spec_file, spec_anchors)?;
    insert_entry(&mut document, "types", &name, entry)?;
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, "types", name.as_str());
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: name.as_str().to_owned(),
        holes,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use domain::tddd::LayerId;
    use domain::tddd::catalog_gen::CatalogImportAction;
    use serde_json::{Value, json};

    use super::*;

    fn seed_catalogue(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("domain-types.json");
        let value = json!({
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

    fn sample_shape() -> ImportedShape {
        ImportedShape {
            module_path: "tddd".to_owned(),
            name: "LayerId".to_owned(),
            kind: json!({
                "kind": "struct",
                "shape": { "kind": "plain", "fields": [{ "name": "value", "ty": "String" }], "has_stripped_fields": false }
            }),
            methods: vec![],
        }
    }

    fn import_command(action: CatalogImportAction) -> CatalogImportCommand {
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::tddd::LayerId".to_owned(),
            action,
            anchors: vec![],
        }
    }

    #[test]
    fn test_import_reference_writes_entry() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let report = import_entry_to_file(
            &path,
            temp.path(),
            &import_command(CatalogImportAction::Reference),
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(report.entry_key, "LayerId");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["types"]["LayerId"]["action"], json!("reference"));
        assert_eq!(written["types"]["LayerId"]["kind"]["kind"], json!("struct"));
    }

    #[test]
    fn test_import_delete_writes_entry() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        import_entry_to_file(
            &path,
            temp.path(),
            &import_command(CatalogImportAction::Delete),
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap();
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["types"]["LayerId"]["action"], json!("delete"));
    }

    #[test]
    fn test_import_duplicate_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let command = import_command(CatalogImportAction::Reference);
        import_entry_to_file(
            &path,
            temp.path(),
            &command,
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap();
        let err = import_entry_to_file(
            &path,
            temp.path(),
            &command,
            &sample_shape(),
            "spec.json",
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::DuplicateEntry { .. }));
    }
}
