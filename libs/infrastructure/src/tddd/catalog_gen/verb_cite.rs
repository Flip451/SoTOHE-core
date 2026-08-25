//! `sotp catalog cite` (D5 / IN-05 / AC-06): append validated spec anchors to an
//! existing catalogue entry.

use std::collections::BTreeSet;
use std::path::Path;

use domain::plan_ref::SpecElementId;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use serde_json::{Value, json};
use usecase::catalog_gen::{CatalogCiteCommand, CatalogError, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, load_bindings, read_catalogue, resolve_entry_section, scan_entry_holes,
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
    let (section, entry_key) = resolve_requested_entry(&document, &command.entry)?;
    append_spec_refs(&mut document, section, entry_key.as_str(), spec_file, &validated)?;
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, section, entry_key.as_str());
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: entry_key.as_str().to_owned(),
        holes,
    })
}

/// Resolve a cite argument to the exact key stored in the catalogue.
///
/// Qualified arguments use an exact map lookup.  A bare argument remains a
/// supported input spelling for existing command workflows, but may select a
/// qualified key only when that tail is unique across the type and trait
/// sections. Function keys retain their existing exact-match behavior.
/// The write and report always use the exact qualified key.
fn resolve_requested_entry(
    document: &Value,
    requested: &str,
) -> Result<(&'static str, CatalogueEntryKey), CatalogError> {
    let requested_key = CatalogueEntryKey::try_new(requested.to_owned())
        .map_err(|_| schema_error("catalogue entry key must not be empty"))?;
    let exact_sections: Vec<&'static str> = ["types", "traits", "functions"]
        .into_iter()
        .filter(|&section| document.get(section).and_then(|value| value.get(requested)).is_some())
        .collect();
    if !exact_sections.is_empty() {
        return Ok((resolve_entry_section(document, requested)?, requested_key));
    }
    if requested.contains("::") {
        return Err(schema_error(format!("entry `{requested}` not found in catalogue")));
    }

    let mut candidates: Vec<(&'static str, CatalogueEntryKey)> = Vec::new();
    for section in ["types", "traits"] {
        let Some(object) = document.get(section).and_then(Value::as_object) else {
            continue;
        };
        for key in object.keys() {
            if key.rsplit("::").next() == Some(requested) {
                let key = CatalogueEntryKey::try_new(key.clone())
                    .map_err(|_| schema_error("catalogue contains an empty entry key"))?;
                candidates.push((section, key));
            }
        }
    }
    match candidates.as_slice() {
        [(section, key)] => Ok((*section, key.clone())),
        [] => Err(schema_error(format!("entry `{requested}` not found in catalogue"))),
        candidates => {
            let labels = candidates
                .iter()
                .map(|(section, key)| format!("{section}.{}", key.as_str()))
                .collect::<Vec<_>>();
            Err(schema_error(format!(
                "entry `{requested}` is ambiguous across qualified catalogue keys ({})",
                labels.join(", ")
            )))
        }
    }
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

    fn seed_with_delete_tombstone(dir: &Path) -> (std::path::PathBuf, String) {
        let path = dir.join("domain-types.json");
        let value = json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": {
                "Deleted": {
                    "action": "delete",
                    "module_path": "tddd",
                    "spec_refs": [],
                    "informal_grounds": []
                }
            },
            "traits": {},
            "functions": {}
        });
        let seeded = serde_json::to_string_pretty(&value).unwrap();
        std::fs::write(&path, &seeded).unwrap();
        (path, seeded)
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
    fn test_cite_appends_anchor_to_qualified_entry_key() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        let value = json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": {
                "domain::alpha::Foo": {
                    "role": { "ValueObject": {} },
                    "spec_refs": []
                },
                "domain::beta::Foo": {
                    "role": { "ValueObject": {} },
                    "spec_refs": []
                }
            },
            "traits": {},
            "functions": {}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let report = cite_anchors_in_file(
            &path,
            temp.path(),
            &cite_command("domain::alpha::Foo", "IN-01"),
            "spec.json",
            &anchors(),
        )
        .unwrap();

        assert_eq!(report.entry_key, "domain::alpha::Foo");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written["types"]["domain::alpha::Foo"]["spec_refs"][0]["anchor"],
            json!("IN-01")
        );
        assert!(written["types"]["domain::beta::Foo"]["spec_refs"].as_array().unwrap().is_empty());
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

    #[test]
    fn test_cite_appends_anchor_to_delete_tombstone() {
        let temp = tempfile::tempdir().unwrap();
        let (path, _) = seed_with_delete_tombstone(temp.path());
        let report = cite_anchors_in_file(
            &path,
            temp.path(),
            &cite_command("Deleted", "IN-01"),
            "spec.json",
            &anchors(),
        )
        .unwrap();
        assert_eq!(report.entry_key, "Deleted");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["types"]["Deleted"]["spec_refs"][0]["anchor"], json!("IN-01"));
    }

    #[test]
    fn test_cite_rejects_ambiguous_entry() {
        // `Foo` exists as both a type and a trait: cite by bare name cannot pick
        // one, so it must fail closed instead of anchoring the first-match
        // section, and must leave the file untouched.
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        let value = json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": { "Foo": { "role": { "ValueObject": {} }, "spec_refs": [] } },
            "traits": { "Foo": { "role": { "SecondaryPort": {} }, "spec_refs": [] } },
            "functions": {}
        });
        let seeded = serde_json::to_string_pretty(&value).unwrap();
        std::fs::write(&path, &seeded).unwrap();

        let err = cite_anchors_in_file(
            &path,
            temp.path(),
            &cite_command("Foo", "IN-01"),
            "spec.json",
            &anchors(),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), seeded);
    }
}
