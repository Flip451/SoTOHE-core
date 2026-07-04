//! `sotp catalog cite` (D5 / IN-05 / AC-06): append validated spec anchors to an
//! existing catalogue entry.

use std::collections::BTreeSet;
use std::path::Path;

use domain::plan_ref::SpecElementId;
use serde_json::{Value, json};
use usecase::catalog_gen::{CatalogCiteCommand, CatalogError, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, find_entry_section, load_bindings, read_catalogue, scan_entry_holes,
    schema_error, spec_ref_file, track_dir, write_catalogue,
};
use super::validate::{load_spec_anchors, validate_anchor};

/// Attach validated spec anchors to an existing entry.
///
/// # Errors
///
/// Returns a [`CatalogError`] on missing file, unresolved anchor, absent entry,
/// or filesystem failure.
pub(super) fn run(
    track_id: &str,
    items_dir: &Path,
    command: CatalogCiteCommand,
) -> Result<CatalogWriteReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    let path = catalogue_path(&dir, &bindings, &command.layer)?;
    let spec_file = spec_ref_file(track_id);
    let spec_anchors = load_spec_anchors(&dir, items_dir)?;
    cite_anchors_in_file(&path, items_dir, &command, &spec_file, &spec_anchors)
}

/// Append validated anchors to an entry in an existing catalogue file
/// (testable core).
fn cite_anchors_in_file(
    path: &Path,
    trusted_root: &Path,
    command: &CatalogCiteCommand,
    spec_file: &str,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<CatalogWriteReport, CatalogError> {
    let mut document = read_catalogue(path, trusted_root)?;
    let mut validated = Vec::new();
    for anchor in &command.anchors {
        validated.push(validate_anchor(anchor, spec_anchors)?);
    }
    let section = find_entry_section(&document, &command.entry)
        .ok_or_else(|| schema_error(format!("entry `{}` not found in catalogue", command.entry)))?;
    append_spec_refs(&mut document, section, &command.entry, spec_file, &validated)?;
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, section, &command.entry);
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: command.entry.clone(),
        holes,
    })
}

/// Append `{ file, anchor }` refs to `document[section][entry].spec_refs`,
/// de-duplicating.
fn append_spec_refs(
    document: &mut Value,
    section: &str,
    entry: &str,
    spec_file: &str,
    anchors: &[SpecElementId],
) -> Result<(), CatalogError> {
    let entry_obj = document
        .as_object_mut()
        .and_then(|root| root.get_mut(section))
        .and_then(Value::as_object_mut)
        .and_then(|section_obj| section_obj.get_mut(entry))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| schema_error(format!("entry `{entry}` is not a JSON object")))?;
    let refs = entry_obj.entry("spec_refs".to_owned()).or_insert_with(|| Value::Array(Vec::new()));
    let list =
        refs.as_array_mut().ok_or_else(|| schema_error("`spec_refs` is not a JSON array"))?;
    for anchor in anchors {
        let candidate = json!({ "file": spec_file, "anchor": anchor.as_ref() });
        if !list.contains(&candidate) {
            list.push(candidate);
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use domain::tddd::LayerId;

    use super::*;

    fn anchors() -> BTreeSet<SpecElementId> {
        let mut set = BTreeSet::new();
        set.insert(SpecElementId::try_new("IN-01").unwrap());
        set
    }

    fn seed_with_entry(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("domain-types.json");
        let value = json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": { "Foo": { "role": { "ValueObject": {} }, "spec_refs": [] } },
            "traits": {},
            "functions": {}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        path
    }

    fn cite_command(entry: &str, anchor: &str) -> CatalogCiteCommand {
        CatalogCiteCommand {
            layer: LayerId::try_new("domain").unwrap(),
            entry: entry.to_owned(),
            anchors: vec![anchor.to_owned()],
        }
    }

    #[test]
    fn test_cite_appends_anchor() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_with_entry(temp.path());
        let report = cite_anchors_in_file(
            &path,
            temp.path(),
            &cite_command("Foo", "IN-01"),
            "spec.json",
            &anchors(),
        )
        .unwrap();
        assert_eq!(report.entry_key, "Foo");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["types"]["Foo"]["spec_refs"][0]["anchor"], json!("IN-01"));
    }

    #[test]
    fn test_cite_rejects_unknown_anchor() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_with_entry(temp.path());
        let err = cite_anchors_in_file(
            &path,
            temp.path(),
            &cite_command("Foo", "ZZ-99"),
            "spec.json",
            &anchors(),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::AnchorNotFound { .. }));
    }

    #[test]
    fn test_cite_rejects_unknown_entry() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_with_entry(temp.path());
        let err = cite_anchors_in_file(
            &path,
            temp.path(),
            &cite_command("Missing", "IN-01"),
            "spec.json",
            &anchors(),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
    }
}
