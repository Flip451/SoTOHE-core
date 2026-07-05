//! `sotp catalog import` (D3 / IN-04 / AC-04): take an existing type's current
//! shape from rustdoc extraction into a layer's catalogue.

use std::collections::BTreeSet;
use std::path::Path;

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::{CatalogEntryName, CatalogImportAction};
use usecase::catalog_gen::{CatalogError, CatalogImportCommand, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, insert_entry, load_bindings, read_catalogue, scan_entry_holes, schema_error,
    spec_ref_file, track_dir, workspace_root, write_catalogue,
};
use super::import_shape::{
    ImportedShape, build_delete_entry, build_import_entry, parse_type_path, resolve_shape,
};
use super::validate::load_spec_anchors;

/// Import an existing type into the layer's catalogue.
///
/// # Errors
///
/// Returns a [`CatalogError`] on missing file, rustdoc-resolution failure,
/// duplicate entry, unresolved anchor, or filesystem failure. A missing
/// catalogue is reported before the rustdoc resolution runs (see
/// [`import_entry_to_file`]).
pub(super) fn run(
    track_id: &str,
    items_dir: &Path,
    command: CatalogImportCommand,
) -> Result<CatalogWriteReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    let path = catalogue_path(&dir, &bindings, &command.layer)?;
    let spec_file = spec_ref_file(track_id);
    let spec_anchors = load_spec_anchors(&dir, items_dir)?;
    let root = workspace_root(items_dir);
    import_entry_to_file(&path, items_dir, &command, &spec_file, &spec_anchors, || {
        resolve_shape(&root, &command.type_path)
    })
}

/// Insert the resolved import entry into an existing catalogue file.
///
/// The catalogue is read *before* `resolve` runs, so an absent catalogue fails
/// closed with [`CatalogError::FileMissing`] ("run `sotp catalog init` first")
/// before the expensive nightly-rustdoc resolution is attempted. `resolve` is a
/// seam so the pure read → insert → write path stays unit-testable without the
/// nightly toolchain.
fn import_entry_to_file(
    path: &Path,
    trusted_root: &Path,
    command: &CatalogImportCommand,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
    resolve: impl FnOnce() -> Result<ImportedShape, CatalogError>,
) -> Result<CatalogWriteReport, CatalogError> {
    let mut document = read_catalogue(path, trusted_root)?;
    validate_type_path_crate_matches_catalogue(&document, &command.type_path)?;
    reject_delete_anchors(command)?;
    let (entry_name, entry) = match command.action {
        // A delete import is identity-only (spec IN-04 / GO-03 / AC-04): record the
        // removed type's identity without resolving the (expensive, nightly)
        // rustdoc shape or emitting role/docs `$todo` holes.
        CatalogImportAction::Delete => build_delete_entry(&command.type_path)?,
        CatalogImportAction::Reference | CatalogImportAction::Modify => {
            let shape = resolve()?;
            let entry = build_import_entry(command, &shape, spec_file, spec_anchors)?;
            (shape.name.clone(), entry)
        }
    };
    let name = CatalogEntryName::try_new(entry_name.clone())
        .map_err(|err| schema_error(format!("invalid entry name `{entry_name}`: {err}")))?;
    insert_entry(&mut document, "types", &name, entry)?;
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, "types", name.as_str());
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: name.as_str().to_owned(),
        holes,
    })
}

fn validate_type_path_crate_matches_catalogue(
    document: &serde_json::Value,
    type_path: &str,
) -> Result<(), CatalogError> {
    let (crate_name, _, _) = parse_type_path(type_path)?;
    let catalogue_crate = document
        .get("crate_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| schema_error("catalogue is missing a string `crate_name`"))?;
    if crate_name == catalogue_crate {
        return Ok(());
    }
    Err(schema_error(format!(
        "type path `{type_path}` targets crate `{crate_name}`, but catalogue crate_name is `{catalogue_crate}`"
    )))
}

fn reject_delete_anchors(command: &CatalogImportCommand) -> Result<(), CatalogError> {
    if command.action != CatalogImportAction::Delete || command.anchors.is_empty() {
        return Ok(());
    }
    Err(schema_error(
        "delete imports do not accept --anchor because deletion tombstones do not persist spec_refs",
    ))
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
            "spec.json",
            &BTreeSet::new(),
            || Ok(sample_shape()),
        )
        .unwrap();
        assert_eq!(report.entry_key, "LayerId");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["types"]["LayerId"]["action"], json!("reference"));
        assert_eq!(written["types"]["LayerId"]["kind"]["kind"], json!("struct"));
    }

    #[test]
    fn test_import_delete_writes_identity_only_tombstone() {
        // A delete import is identity-only (spec IN-04 / GO-03 / AC-04): it records
        // the removed type's identity with no role/docs `$todo` holes, and never
        // resolves the (expensive, nightly) rustdoc shape.
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let resolver_called = std::cell::Cell::new(false);
        let report = import_entry_to_file(
            &path,
            temp.path(),
            &import_command(CatalogImportAction::Delete),
            "spec.json",
            &BTreeSet::new(),
            || {
                resolver_called.set(true);
                Ok(sample_shape())
            },
        )
        .unwrap();
        assert!(!resolver_called.get(), "delete import must not resolve the rustdoc shape");
        assert!(report.holes.is_empty(), "delete tombstone must have no $todo holes");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let entry = &written["types"]["LayerId"];
        assert_eq!(entry["action"], json!("delete"));
        assert_eq!(entry["module_path"], json!("tddd"));
        assert!(entry.get("role").is_none(), "delete tombstone must not carry a role");
        assert!(entry.get("docs").is_none(), "delete tombstone must not carry docs");
        assert!(entry.get("kind").is_none(), "delete tombstone must not carry a kind");
    }

    #[test]
    fn test_import_delete_rejects_anchors() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut command = import_command(CatalogImportAction::Delete);
        command.anchors = vec!["IN-01".to_owned()];
        let resolver_called = std::cell::Cell::new(false);
        let err = import_entry_to_file(
            &path,
            temp.path(),
            &command,
            "spec.json",
            &BTreeSet::new(),
            || {
                resolver_called.set(true);
                Ok(sample_shape())
            },
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
        assert!(!resolver_called.get(), "delete import anchor rejection must not resolve rustdoc");
    }

    #[test]
    fn test_import_rejects_type_path_crate_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut command = import_command(CatalogImportAction::Reference);
        command.type_path = "usecase::tddd::LayerId".to_owned();
        let resolver_called = std::cell::Cell::new(false);
        let err = import_entry_to_file(
            &path,
            temp.path(),
            &command,
            "spec.json",
            &BTreeSet::new(),
            || {
                resolver_called.set(true);
                Ok(sample_shape())
            },
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
        assert!(!resolver_called.get(), "crate mismatch must fail before rustdoc resolution");
    }

    #[test]
    fn test_import_missing_catalogue_reports_before_rustdoc() {
        // The catalogue file is never created. `import` must fail with
        // `FileMissing` ("run init first") *before* the rustdoc resolver runs,
        // rather than surfacing a rustdoc/type-resolution error first (which on a
        // machine without the nightly toolchain would be reported instead of the
        // actionable missing-catalogue error).
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        let resolver_called = std::cell::Cell::new(false);
        let err = import_entry_to_file(
            &path,
            temp.path(),
            &import_command(CatalogImportAction::Reference),
            "spec.json",
            &BTreeSet::new(),
            || {
                resolver_called.set(true);
                Ok(sample_shape())
            },
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::FileMissing { .. }));
        assert!(
            !resolver_called.get(),
            "rustdoc resolution must not run when the catalogue is missing"
        );
    }

    #[test]
    fn test_import_duplicate_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let command = import_command(CatalogImportAction::Reference);
        import_entry_to_file(&path, temp.path(), &command, "spec.json", &BTreeSet::new(), || {
            Ok(sample_shape())
        })
        .unwrap();
        let err = import_entry_to_file(
            &path,
            temp.path(),
            &command,
            "spec.json",
            &BTreeSet::new(),
            || Ok(sample_shape()),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::DuplicateEntry { .. }));
    }
}
