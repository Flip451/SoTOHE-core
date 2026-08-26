//! `sotp catalog import` (D3 / IN-04 / AC-04): take an existing type's current
//! shape from rustdoc extraction into a layer's catalogue.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::{CatalogEntryName, CatalogImportAction};
use domain::tddd::catalogue_v2::{CrateName, FullyQualifiedItemPath, Identifier, ModulePath};
use domain::tddd::semantic_verify::CatalogueEntryKey;
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
    run_with_resolver(track_id, items_dir, command, resolve_shape)
}

pub(super) fn run_with_resolver(
    track_id: &str,
    items_dir: &Path,
    command: CatalogImportCommand,
    resolve: fn(&Path, &str) -> Result<ImportedShape, CatalogError>,
) -> Result<CatalogWriteReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    let path = catalogue_path(&dir, &bindings, &command.layer)?;
    let spec_file = spec_ref_file(track_id);
    let spec_anchors = load_spec_anchors(&dir, items_dir)?;
    let root = workspace_root(items_dir);
    import_entry_to_file(&path, items_dir, &command, &spec_file, &spec_anchors, || {
        resolve(&root, &command.type_path)
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
    let entry_key = fully_qualified_entry_key(&document, &command.type_path)?;
    let name = CatalogEntryName::try_new(entry_key.as_str().to_owned()).map_err(|err| {
        schema_error(format!("invalid entry name `{}`: {err}", entry_key.as_str()))
    })?;
    reject_duplicate_identity(&document, "types", &entry_key, &name)?;
    let entry = match command.action {
        // A delete import records the removed type's identity plus grounding
        // without resolving the (expensive, nightly) rustdoc shape or emitting
        // role/docs `$todo` holes.
        CatalogImportAction::Delete => {
            build_delete_entry(&command.type_path, &command.anchors, spec_file, spec_anchors)
                .map(|(_, entry)| entry)?
        }
        CatalogImportAction::Reference | CatalogImportAction::Modify => {
            let shape = resolve()?;
            validate_resolved_shape_path(&command.type_path, &shape)?;
            build_import_entry(command, &shape, spec_file, spec_anchors)?
        }
    };
    insert_entry(&mut document, "types", &name, entry)?;
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, "types", name.as_str());
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: name.as_str().to_owned(),
        holes,
    })
}

fn reject_duplicate_identity(
    document: &serde_json::Value,
    section: &str,
    new_key: &CatalogueEntryKey,
    new_name: &CatalogEntryName,
) -> Result<(), CatalogError> {
    let crate_name_text = document
        .get("crate_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| schema_error("catalogue is missing a string `crate_name`"))?;
    let crate_name = CrateName::new(crate_name_text.to_owned())
        .map_err(|error| schema_error(format!("invalid catalogue crate_name: {error}")))?;
    let new_identity =
        FullyQualifiedItemPath::from_catalogue_entry_key(&crate_name, new_key, &ModulePath::root())
            .map_err(|error| {
                schema_error(format!("invalid import identity `{}`: {error}", new_key.as_str()))
            })?;
    let requested_name = terminal_entry_name(new_key.as_str());
    let Some(section_value) = document.get(section) else {
        return Ok(());
    };
    let entries = section_value.as_object().ok_or_else(|| {
        schema_error(format!("catalogue section `{section}` is not a JSON object"))
    })?;

    for (raw_key, entry) in entries {
        if raw_key == "$todo" {
            continue;
        }
        if terminal_entry_name(raw_key) != requested_name {
            continue;
        }
        let existing_key = CatalogueEntryKey::try_new(raw_key.to_owned()).map_err(|error| {
            schema_error(format!("invalid existing catalogue entry key `{raw_key}`: {error}"))
        })?;
        let module_path = if raw_key.contains("::") {
            ModulePath::root()
        } else {
            existing_module_path(raw_key, entry)?
        };
        let existing_identity = FullyQualifiedItemPath::from_catalogue_entry_key(
            &crate_name,
            &existing_key,
            &module_path,
        )
        .map_err(|error| {
            schema_error(format!("invalid existing entry identity `{raw_key}`: {error}"))
        })?;
        if existing_identity == new_identity {
            return Err(CatalogError::DuplicateEntry { entry_key: new_name.clone() });
        }
    }
    Ok(())
}

fn terminal_entry_name(key: &str) -> &str {
    key.rsplit("::").next().unwrap_or(key)
}

fn existing_module_path(
    raw_key: &str,
    entry: &serde_json::Value,
) -> Result<ModulePath, CatalogError> {
    let Some(module_value) = entry.get("module_path") else {
        return Ok(ModulePath::root());
    };
    let module = module_value.as_str().ok_or_else(|| {
        schema_error(format!("existing entry `{raw_key}` has an invalid `module_path`"))
    })?;
    if module.is_empty() {
        return Ok(ModulePath::root());
    }
    ModulePath::from_str(module).map_err(|error| {
        schema_error(format!("existing entry `{raw_key}` has invalid module_path: {error}"))
    })
}

/// Build the catalogue key from the exact crate/module/type path supplied to
/// `catalog import --type`.  The catalogue entry keeps its module path in the
/// entry body, while the map key retains the complete identity so two
/// same-named types can be written side by side.
fn fully_qualified_entry_key(
    document: &serde_json::Value,
    type_path: &str,
) -> Result<CatalogueEntryKey, CatalogError> {
    validate_type_path_segments(type_path)?;
    let (crate_name, module, name) = parse_type_path(type_path)?;
    let catalogue_crate = document
        .get("crate_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| schema_error("catalogue is missing a string `crate_name`"))?;
    if crate_name != catalogue_crate {
        return Err(schema_error(format!(
            "type path `{type_path}` targets crate `{crate_name}`, but catalogue crate_name is `{catalogue_crate}`"
        )));
    }
    let raw_key = if module.is_empty() {
        format!("{crate_name}::{name}")
    } else {
        format!("{crate_name}::{module}::{name}")
    };
    CatalogueEntryKey::try_new(raw_key)
        .map_err(|_| schema_error(format!("invalid catalogue entry key for `{type_path}`")))
}

fn validate_type_path_segments(type_path: &str) -> Result<(), CatalogError> {
    for segment in type_path.split("::") {
        if segment.is_empty() {
            return Err(schema_error(format!(
                "type path `{type_path}` contains an empty path segment"
            )));
        }
        Identifier::new(segment.to_owned()).map_err(|error| {
            schema_error(format!(
                "type path `{type_path}` contains invalid path segment `{segment}`: {error}"
            ))
        })?;
    }
    Ok(())
}

fn validate_resolved_shape_path(
    type_path: &str,
    shape: &ImportedShape,
) -> Result<(), CatalogError> {
    let (_, module, name) = parse_type_path(type_path)?;
    if shape.module_path == module && shape.name == name {
        return Ok(());
    }
    Err(schema_error(format!("resolved type shape does not match requested path `{type_path}`")))
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

    fn spec_anchors(ids: &[&str]) -> BTreeSet<domain::plan_ref::SpecElementId> {
        ids.iter().map(|id| domain::plan_ref::SpecElementId::try_new(*id).unwrap()).collect()
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
        assert_eq!(report.entry_key, "domain::tddd::LayerId");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["types"]["domain::tddd::LayerId"]["action"], json!("reference"));
        assert_eq!(written["types"]["domain::tddd::LayerId"]["kind"]["kind"], json!("struct"));
    }

    #[test]
    fn test_import_delete_writes_grounded_tombstone() {
        // A delete import records the removed type's identity and spec grounding
        // with no role/docs `$todo` holes, and never resolves the (expensive,
        // nightly) rustdoc shape.
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut command = import_command(CatalogImportAction::Delete);
        command.anchors = vec!["IN-01".to_owned()];
        let resolver_called = std::cell::Cell::new(false);
        let report = import_entry_to_file(
            &path,
            temp.path(),
            &command,
            "spec.json",
            &spec_anchors(&["IN-01"]),
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
        let entry = &written["types"]["domain::tddd::LayerId"];
        assert_eq!(entry["action"], json!("delete"));
        assert_eq!(entry["module_path"], json!("tddd"));
        assert_eq!(entry["spec_refs"][0]["anchor"], json!("IN-01"));
        assert_eq!(entry["informal_grounds"], json!([]));
        assert!(entry.get("role").is_none(), "delete tombstone must not carry a role");
        assert!(entry.get("docs").is_none(), "delete tombstone must not carry docs");
        assert!(entry.get("kind").is_none(), "delete tombstone must not carry a kind");
    }

    #[test]
    fn test_import_delete_requires_anchor() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let command = import_command(CatalogImportAction::Delete);
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
        assert!(
            !resolver_called.get(),
            "delete import anchor requirement must not resolve rustdoc"
        );
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
    fn test_import_rejects_empty_type_path_segment_before_parse() {
        let document = serde_json::json!({"crate_name": "domain"});
        let error = fully_qualified_entry_key(&document, "domain::::LayerId").unwrap_err();
        assert!(matches!(error, CatalogError::SchemaInvalid { .. }));
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

    #[test]
    fn test_import_duplicate_identity_rejected_for_loose_existing_key() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["types"] = json!({
            "LayerId": {"module_path": "tddd"}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();
        let resolver_called = std::cell::Cell::new(false);

        let error = import_entry_to_file(
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

        assert!(matches!(error, CatalogError::DuplicateEntry { .. }));
        assert!(!resolver_called.get(), "identity duplicate must fail before resolution");
    }

    #[test]
    fn test_import_skips_unrelated_draft_entry_without_module_identity() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["types"] = json!({
            "Other": {"module_path": {"$todo": "assign the module path"}}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();

        let report = import_entry_to_file(
            &path,
            temp.path(),
            &import_command(CatalogImportAction::Reference),
            "spec.json",
            &BTreeSet::new(),
            || Ok(sample_shape()),
        )
        .unwrap();

        assert_eq!(report.entry_key, "domain::tddd::LayerId");
    }
}
