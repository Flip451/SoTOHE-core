//! `sotp catalog import` (D3 / IN-04 / AC-04): take an existing type's current
//! shape from rustdoc extraction into a layer's catalogue.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::{CatalogEntryName, CatalogImportAction};
use domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity_for_action_in_namespace;
use domain::tddd::catalogue_v2::{
    CatalogueItemNamespace, CrateName, FullyQualifiedItemPath, Identifier, ItemAction, ModulePath,
    TypeRef,
};
use domain::tddd::semantic_verify::CatalogueEntryKey;
use usecase::catalog_gen::{CatalogError, CatalogImportCommand, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, insert_entry, load_bindings, port_error, read_catalogue, scan_entry_holes,
    schema_error, spec_ref_file, track_dir, workspace_root, write_catalogue,
};
use super::import_shape::{
    ImportedShape, build_delete_entry, build_import_entry, parse_type_path, resolve_shape,
};
use super::validate::load_spec_anchors;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::catalogue_document_codec::EXPLICIT_ROOT_MODULE_PATH;
use crate::tddd::catalogue_to_extended_crate_codec::{normalized_paths_for_doc, paths_from_map};

const MAX_RUSTDOC_JSON_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Default)]
struct IdentityResolutionSets {
    baseline: BTreeSet<FullyQualifiedItemPath>,
    current: BTreeSet<FullyQualifiedItemPath>,
}

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
    let identity_sets = load_identity_resolution_sets(&path, items_dir)?;
    import_entry_to_file_with_identity_sets(
        &path,
        items_dir,
        &command,
        &spec_file,
        &spec_anchors,
        &identity_sets,
        || resolve(&root, &command.type_path),
    )
}

/// Insert the resolved import entry into an existing catalogue file.
///
/// The catalogue is read *before* `resolve` runs, so an absent catalogue fails
/// closed with [`CatalogError::FileMissing`] ("run `sotp catalog init` first")
/// before the expensive nightly-rustdoc resolution is attempted. `resolve` is a
/// seam so the pure read → insert → write path stays unit-testable without the
/// nightly toolchain.
#[cfg(test)]
fn import_entry_to_file(
    path: &Path,
    trusted_root: &Path,
    command: &CatalogImportCommand,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
    resolve: impl FnOnce() -> Result<ImportedShape, CatalogError>,
) -> Result<CatalogWriteReport, CatalogError> {
    import_entry_to_file_with_identity_sets(
        path,
        trusted_root,
        command,
        spec_file,
        spec_anchors,
        &IdentityResolutionSets::default(),
        resolve,
    )
}

fn import_entry_to_file_with_identity_sets(
    path: &Path,
    trusted_root: &Path,
    command: &CatalogImportCommand,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
    identity_sets: &IdentityResolutionSets,
    resolve: impl FnOnce() -> Result<ImportedShape, CatalogError>,
) -> Result<CatalogWriteReport, CatalogError> {
    let mut document = read_catalogue(path, trusted_root)?;
    let entry_key = fully_qualified_entry_key(&document, &command.type_path)?;
    let name = CatalogEntryName::try_new(entry_key.as_str().to_owned()).map_err(|err| {
        schema_error(format!("invalid entry name `{}`: {err}", entry_key.as_str()))
    })?;
    reject_duplicate_identity(
        &document,
        "types",
        &entry_key,
        &name,
        &identity_sets.baseline,
        &identity_sets.current,
    )?;
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
    baseline: &BTreeSet<FullyQualifiedItemPath>,
    current: &BTreeSet<FullyQualifiedItemPath>,
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
        let action = existing_item_action(raw_key, entry)?;
        let existing_key = CatalogueEntryKey::try_new(raw_key.to_owned()).map_err(|error| {
            schema_error(format!("invalid existing catalogue entry key `{raw_key}`: {error}"))
        })?;
        let module_path = if raw_key.contains("::") {
            Some(ModulePath::root())
        } else {
            existing_module_path(raw_key, entry)?
        };
        // D3: an omitted placement is unresolved, not the crate root. Resolve
        // the existing declaration with its own action against the shared
        // baseline/current identity sets before deciding whether it collides
        // with this qualified import.
        let Some(module_path) = module_path else {
            // An existing bare Add is declaration-first and has no established
            // identity to compare. Keep the duplicate guard conservative rather
            // than allowing a stale or absent current snapshot to manufacture a
            // distinct identity that a later fresh rustdoc run could collide
            // with.
            if action == ItemAction::Add {
                return Err(CatalogError::DuplicateEntry { entry_key: new_name.clone() });
            }
            let reference = TypeRef::new(raw_key.to_owned()).map_err(|error| {
                schema_error(format!("invalid existing entry reference `{raw_key}`: {error}"))
            })?;
            let existing_identity = resolve_catalogue_identity_for_action_in_namespace(
                &reference,
                &crate_name,
                action,
                baseline,
                current,
                CatalogueItemNamespace::Type,
            )
            .map_err(|_| CatalogError::DuplicateEntry { entry_key: new_name.clone() })?;
            if existing_identity == new_identity {
                return Err(CatalogError::DuplicateEntry { entry_key: new_name.clone() });
            }
            continue;
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

fn existing_item_action(
    raw_key: &str,
    entry: &serde_json::Value,
) -> Result<ItemAction, CatalogError> {
    let action = match entry.get("action") {
        None => "add",
        Some(value) => value.as_str().ok_or_else(|| {
            schema_error(format!("existing entry `{raw_key}` has a non-string `action`"))
        })?,
    };
    ItemAction::from_str(action).map_err(|error| {
        schema_error(format!("existing entry `{raw_key}` has invalid action `{action}`: {error}"))
    })
}

fn load_identity_resolution_sets(
    catalogue_path: &Path,
    trusted_items_root: &Path,
) -> Result<IdentityResolutionSets, CatalogError> {
    let document = read_catalogue(catalogue_path, trusted_items_root)?;
    let crate_name_text = document
        .get("crate_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| schema_error("catalogue is missing a string `crate_name`"))?;
    let crate_name = CrateName::new(crate_name_text.to_owned())
        .map_err(|error| schema_error(format!("invalid catalogue crate_name: {error}")))?;

    let baseline_path = baseline_path(catalogue_path)?;
    let baseline = load_optional_rustdoc(&baseline_path, trusted_items_root)?;
    // Bare Add entries are rejected conservatively, while the action-aware
    // resolver uses only the baseline for the remaining existing-item actions.
    // There is therefore no reason for this duplicate check to inspect the
    // shared, mutable current rustdoc cache.

    Ok(IdentityResolutionSets {
        baseline: baseline
            .as_ref()
            .map(|krate| rustdoc_identities(krate, &crate_name))
            .unwrap_or_default(),
        current: BTreeSet::new(),
    })
}

fn baseline_path(catalogue_path: &Path) -> Result<PathBuf, CatalogError> {
    let stem = catalogue_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| schema_error("catalogue path has no valid file stem"))?;
    Ok(catalogue_path.with_file_name(format!("{stem}-baseline.json")))
}

fn load_optional_rustdoc(
    path: &Path,
    trusted_root: &Path,
) -> Result<Option<rustdoc_types::Crate>, CatalogError> {
    let Some(content) =
        crate::trusted_file::read_bounded_regular_file(path, trusted_root, MAX_RUSTDOC_JSON_BYTES)
            .map_err(|error| {
                port_error(format!(
                    "failed to read rustdoc identity snapshot {}: {error}",
                    path.display()
                ))
            })?
    else {
        return Ok(None);
    };
    BaselineRustdocCodec::from_json(&content).map(Some).map_err(|error| {
        port_error(format!(
            "failed to decode rustdoc identity snapshot {}: {error}",
            path.display()
        ))
    })
}

fn rustdoc_identities(
    krate: &rustdoc_types::Crate,
    crate_name: &CrateName,
) -> BTreeSet<FullyQualifiedItemPath> {
    let normalized = normalized_paths_for_doc(krate, crate_name);
    paths_from_map(&normalized)
}

fn terminal_entry_name(key: &str) -> &str {
    key.rsplit("::").next().unwrap_or(key)
}

/// Decodes an existing bare entry's placement exactly as `CatalogueDocumentCodec`
/// does: an omitted or empty `module_path` is an unresolved placement (`None`),
/// the explicit root marker is the crate root, and any other value is parsed.
fn existing_module_path(
    raw_key: &str,
    entry: &serde_json::Value,
) -> Result<Option<ModulePath>, CatalogError> {
    let Some(module_value) = entry.get("module_path") else {
        return Ok(None);
    };
    let module = module_value.as_str().ok_or_else(|| {
        schema_error(format!("existing entry `{raw_key}` has an invalid `module_path`"))
    })?;
    if module.is_empty() {
        return Ok(None);
    }
    if module == EXPLICIT_ROOT_MODULE_PATH {
        return Ok(Some(ModulePath::root()));
    }
    ModulePath::from_str(module).map(Some).map_err(|error| {
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

    fn sample_shape_at(module_path: &str, name: &str) -> ImportedShape {
        ImportedShape {
            module_path: module_path.to_owned(),
            name: name.to_owned(),
            kind: json!({
                "kind": "struct",
                "shape": { "kind": "plain", "fields": [{ "name": "value", "ty": "String" }], "has_stripped_fields": false }
            }),
            methods: vec![],
        }
    }

    fn sample_shape() -> ImportedShape {
        sample_shape_at("tddd", "LayerId")
    }

    fn import_command(action: CatalogImportAction) -> CatalogImportCommand {
        import_command_for(action, "domain::tddd::LayerId")
    }

    fn import_command_for(action: CatalogImportAction, type_path: &str) -> CatalogImportCommand {
        CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: type_path.to_owned(),
            action,
            anchors: vec![],
        }
    }

    fn write_existing_bare_type(path: &Path, action: &str) {
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        document["types"] = json!({ "Thing": { "action": action } });
        std::fs::write(path, serde_json::to_string_pretty(&document).unwrap()).unwrap();
    }

    fn rustdoc_snapshot(entries: &[(&str, &str)]) -> String {
        let paths = entries
            .iter()
            .enumerate()
            .map(|(index, (module_path, name))| {
                let mut path = vec!["domain".to_owned()];
                path.extend(
                    module_path
                        .split("::")
                        .filter(|segment| !segment.is_empty())
                        .map(str::to_owned),
                );
                path.push((*name).to_owned());
                (
                    rustdoc_types::Id((index + 1) as u32),
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path,
                        kind: rustdoc_types::ItemKind::Struct,
                    },
                )
            })
            .collect();
        let krate = rustdoc_types::Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: std::collections::HashMap::new(),
            paths,
            external_crates: std::collections::HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        };
        serde_json::to_string(&krate).unwrap()
    }

    fn production_import_resolver(
        _workspace_root: &Path,
        type_path: &str,
    ) -> Result<ImportedShape, CatalogError> {
        let (_, module, name) = parse_type_path(type_path)
            .map_err(|error| port_error(format!("test type path parsing failed: {error}")))?;
        Ok(sample_shape_at(&module, &name))
    }

    fn resolver_must_not_run(
        _workspace_root: &Path,
        _type_path: &str,
    ) -> Result<ImportedShape, CatalogError> {
        Err(port_error("the rustdoc shape resolver must not run"))
    }

    fn setup_production_identity_import_fixture(
        existing_action: &str,
        baseline_entries: &[(&str, &str)],
    ) -> (tempfile::TempDir, std::path::PathBuf, String, std::path::PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("src")).unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            r#"[package]
name = "domain"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
        )
        .unwrap();
        std::fs::write(workspace.path().join("src/lib.rs"), "pub struct Thing;\n").unwrap();
        std::fs::write(
            workspace.path().join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#,
        )
        .unwrap();

        let items_dir = workspace.path().join("track/items");
        let track_id = "production-import-track".to_owned();
        let track_dir = items_dir.join(&track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("spec.json"),
            r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Production identity import test",
  "scope": {
    "in_scope": [{ "id": "AC-02", "text": "identity import" }],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#,
        )
        .unwrap();
        let path = seed_catalogue(&track_dir);
        write_existing_bare_type(&path, existing_action);
        std::fs::write(baseline_path(&path).unwrap(), rustdoc_snapshot(baseline_entries)).unwrap();
        (workspace, items_dir, track_id, path)
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
    fn test_import_accepts_distinct_identity_beside_explicit_root_entry() {
        // `CatalogueDocumentCodec` encodes `Some(ModulePath::root())` as ".".
        // A same-named entry placed at the crate root is a different identity
        // from `domain::tddd::LayerId`, so the import must proceed instead of
        // rejecting "." as an invalid module path.
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["types"] = json!({
            "LayerId": {"module_path": EXPLICIT_ROOT_MODULE_PATH}
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
        .expect("an explicit-root entry with the same name is a distinct identity");

        assert_eq!(report.entry_key, "domain::tddd::LayerId");
    }

    #[test]
    fn test_import_duplicate_identity_rejected_for_explicit_root_entry() {
        // The explicit root marker resolves to `ModulePath::root()`, so importing
        // the crate-root `domain::LayerId` beside a bare `LayerId` placed at "."
        // is the same identity and must be rejected before resolution.
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["types"] = json!({
            "LayerId": {"module_path": EXPLICIT_ROOT_MODULE_PATH}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();
        let command = CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "domain::LayerId".to_owned(),
            action: CatalogImportAction::Reference,
            anchors: vec![],
        };
        let resolver_called = std::cell::Cell::new(false);

        let error = import_entry_to_file(
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

        assert!(matches!(error, CatalogError::DuplicateEntry { .. }));
        assert!(!resolver_called.get(), "identity duplicate must fail before resolution");
    }

    #[test]
    fn test_import_unplaced_reference_and_modify_compare_resolved_identity() {
        // A bare existing entry must be compared by the identity selected from
        // its own action. A baseline `alpha::Thing` does not block an import of
        // the distinct qualified `beta::Thing`. The fixture goes through
        // `run_with_resolver`, so the identity sets come from the production
        // snapshot construction rather than a hand-built test universe.
        for (existing_action, import_action) in
            [("reference", CatalogImportAction::Reference), ("modify", CatalogImportAction::Modify)]
        {
            let (_workspace, items_dir, track_id, _path) =
                setup_production_identity_import_fixture(existing_action, &[("alpha", "Thing")]);
            let report = run_with_resolver(
                &track_id,
                &items_dir,
                import_command_for(import_action, "domain::beta::Thing"),
                production_import_resolver,
            )
            .expect("a distinct resolved identity must not be rejected");

            assert_eq!(report.entry_key, "domain::beta::Thing");
        }
    }

    #[test]
    fn test_import_unplaced_reference_and_modify_reject_same_resolved_identity() {
        for existing_action in ["reference", "modify"] {
            let (_workspace, items_dir, track_id, _path) =
                setup_production_identity_import_fixture(existing_action, &[("alpha", "Thing")]);
            let error = run_with_resolver(
                &track_id,
                &items_dir,
                import_command_for(CatalogImportAction::Reference, "domain::alpha::Thing"),
                production_import_resolver,
            )
            .expect_err("the resolved same identity must be rejected");

            assert!(matches!(error, CatalogError::DuplicateEntry { .. }), "{error:?}");
        }
    }

    #[test]
    fn test_import_unplaced_entry_resolution_fails_closed_when_unresolved_or_ambiguous() {
        for baseline in [&[][..], &[("alpha", "Thing"), ("beta", "Thing")][..]] {
            let (_workspace, items_dir, track_id, _path) =
                setup_production_identity_import_fixture("reference", baseline);
            let error = run_with_resolver(
                &track_id,
                &items_dir,
                import_command_for(CatalogImportAction::Reference, "domain::gamma::Thing"),
                production_import_resolver,
            )
            .expect_err("unresolved and ambiguous existing identities must fail closed");

            assert!(matches!(error, CatalogError::DuplicateEntry { .. }), "{error:?}");
        }
    }

    #[test]
    fn test_import_existing_bare_add_fails_closed_without_current_snapshot_lookup() {
        let (_workspace, items_dir, track_id, _path) =
            setup_production_identity_import_fixture("add", &[]);

        let error = run_with_resolver(
            &track_id,
            &items_dir,
            import_command_for(CatalogImportAction::Reference, "domain::beta::Thing"),
            resolver_must_not_run,
        )
        .expect_err("an existing bare add must be rejected conservatively");

        assert!(matches!(error, CatalogError::DuplicateEntry { .. }), "{error:?}");
    }

    #[test]
    fn test_import_rejects_non_string_existing_action_through_production_path() {
        let (_workspace, items_dir, track_id, path) =
            setup_production_identity_import_fixture("add", &[]);
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["types"]["Thing"]["action"] = json!(7);
        std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();

        let error = run_with_resolver(
            &track_id,
            &items_dir,
            import_command_for(CatalogImportAction::Reference, "domain::beta::Thing"),
            resolver_must_not_run,
        )
        .expect_err("a non-string action must be schema-invalid");

        assert!(matches!(error, CatalogError::SchemaInvalid { .. }), "{error:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_load_optional_rustdoc_rejects_oversized_snapshot_before_decode() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("snapshot.json");
        std::fs::File::create(&path).unwrap().set_len(MAX_RUSTDOC_JSON_BYTES + 1).unwrap();

        let error = load_optional_rustdoc(&path, temp.path())
            .expect_err("an oversized rustdoc snapshot must fail closed");

        assert!(matches!(error, CatalogError::Port { .. }), "{error:?}");
    }

    #[test]
    fn test_import_skips_unrelated_draft_entry_without_module_identity() {
        let (_workspace, items_dir, track_id, path) =
            setup_production_identity_import_fixture("add", &[]);
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["types"] = json!({
            "Other": {"module_path": {"$todo": "assign the module path"}}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();

        let report = run_with_resolver(
            &track_id,
            &items_dir,
            import_command(CatalogImportAction::Reference),
            production_import_resolver,
        )
        .unwrap();

        assert_eq!(report.entry_key, "domain::tddd::LayerId");
    }
}
