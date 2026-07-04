//! Filesystem access + path resolution helpers for the catalogue adapter.
//!
//! Centralises the `<layer>-types.json` path convention, the TDDD layer
//! enumeration (from `architecture-rules.json`), catalogue read/write with
//! deterministic formatting, and the shared [`CatalogError`] constructors.

use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::tddd::LayerId;
use domain::tddd::catalog_gen::{CatalogEntryKind, CatalogEntryName, DraftHole};
use serde_json::{Map, Value};
use usecase::catalog_gen::CatalogError;
use usecase::dry_driver_shared::IoFailureDetail;

use crate::tddd::catalogue_document_codec::SCHEMA_VERSION;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::path_safety::lexical_normalize;
use crate::verify::tddd_layers::{TdddLayerBinding, find_binding, load_tddd_layers_from_workspace};

/// Build a [`CatalogError::SchemaInvalid`] with the given diagnostic.
pub(super) fn schema_error(message: impl Into<String>) -> CatalogError {
    CatalogError::SchemaInvalid { message: domain::tddd::catalogue_linter::FreeText::new(message) }
}

/// Build a [`CatalogError::Port`] with the given diagnostic.
pub(super) fn port_error(message: impl Into<String>) -> CatalogError {
    CatalogError::Port { detail: IoFailureDetail::new(message) }
}

/// Derive the workspace root that hosts `architecture-rules.json` from the
/// track items directory (`<ws>/track/items` → `<ws>`; relative `track/items`
/// → the current directory).
pub(super) fn workspace_root(items_dir: &Path) -> PathBuf {
    items_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Resolve the on-disk directory holding a track's artefacts
/// (`<items_dir>/<track_id>`), guarded by `items_dir` as the trusted root.
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] for an invalid track id, or
/// [`CatalogError::Port`] when the resolved directory escapes `items_dir` or
/// contains a symlink boundary.
pub(super) fn track_dir(items_dir: &Path, track_id: &str) -> Result<PathBuf, CatalogError> {
    let valid_track_id = TrackId::try_new(track_id.to_owned())
        .map_err(|err| schema_error(format!("invalid track id `{track_id}`: {err}")))?;
    let trusted_root = lexical_normalize(items_dir);
    let resolved = lexical_normalize(&items_dir.join(valid_track_id.as_ref()));
    if !resolved.starts_with(&trusted_root) {
        return Err(port_error(format!(
            "track path {} resolves outside trusted items root {}",
            resolved.display(),
            trusted_root.display()
        )));
    }
    reject_symlinks_below(&resolved, &trusted_root)
        .map_err(|err| port_error(format!("symlink guard: {}: {err}", resolved.display())))?;
    Ok(resolved)
}

/// Load the enabled TDDD layer bindings from `architecture-rules.json`.
pub(super) fn load_bindings(items_dir: &Path) -> Result<Vec<TdddLayerBinding>, CatalogError> {
    let root = workspace_root(items_dir);
    load_tddd_layers_from_workspace(&root)
        .map_err(|err| port_error(format!("failed to load TDDD layers: {err}")))
}

/// Resolve the catalogue file path for `layer` under `track_dir` from the
/// layer's `catalogue_file` binding.
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] when `layer` is not a TDDD-enabled
/// layer in `architecture-rules.json`. A syntactically valid but unconfigured
/// `--layer` (e.g. the typo `usecaes`) must fail here rather than resolve to an
/// invented `<layer>-types.json`: that file is absent, and the commit gate
/// treats an absent catalogue as `Skipped`, so a silent fallback would let a
/// misspelled layer validate nothing while appearing to succeed.
pub(super) fn catalogue_path(
    track_dir: &Path,
    bindings: &[TdddLayerBinding],
    layer: &LayerId,
) -> Result<PathBuf, CatalogError> {
    let binding = find_binding(bindings, layer.as_ref()).ok_or_else(|| {
        schema_error(format!(
            "layer `{}` is not a TDDD-enabled layer in architecture-rules.json",
            layer.as_ref()
        ))
    })?;
    Ok(track_dir.join(binding.catalogue_file()))
}

/// The portable, repo-relative `spec.json` path recorded in catalogue
/// `spec_refs[].file` entries for `track_id`.
///
/// Always `track/items/<track_id>/spec.json`, independent of the (possibly
/// absolute) `items_dir` the CLI was invoked with, so the written catalogue
/// stays host-independent and every `add` / `import` / `cite` spells the ref
/// identically. Callers pass a `track_id` already validated by [`track_dir`].
pub(super) fn spec_ref_file(track_id: &str) -> String {
    format!("track/items/{track_id}/spec.json")
}

/// The catalogue section (`types` / `traits` / `functions`) an entry kind
/// belongs to.
pub(super) fn section_for_kind(kind: CatalogEntryKind) -> &'static str {
    match kind {
        CatalogEntryKind::Struct | CatalogEntryKind::Enum | CatalogEntryKind::TypeAlias => "types",
        CatalogEntryKind::Trait => "traits",
        CatalogEntryKind::Function => "functions",
    }
}

/// Return whether a guarded catalogue path exists.
///
/// # Errors
///
/// Returns [`CatalogError::Port`] when a symlink boundary is detected or the
/// path cannot be inspected.
pub(super) fn catalogue_present(path: &Path, trusted_root: &Path) -> Result<bool, CatalogError> {
    reject_symlinks_below(path, trusted_root)
        .map_err(|err| port_error(format!("symlink guard: {}: {err}", path.display())))
}

/// Read a catalogue file as a raw JSON tree.
///
/// # Errors
///
/// Returns [`CatalogError::FileMissing`] when the file is absent, or
/// [`CatalogError::SchemaInvalid`] when it does not parse as JSON or uses an
/// unsupported catalogue schema version.
pub(super) fn read_catalogue(path: &Path, trusted_root: &Path) -> Result<Value, CatalogError> {
    if !catalogue_present(path, trusted_root)? {
        return Err(CatalogError::FileMissing { path: path.to_path_buf() });
    }
    let text = std::fs::read_to_string(path)
        .map_err(|err| port_error(format!("failed to read {}: {err}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| schema_error(format!("{}: {err}", path.display())))?;
    reject_duplicate_keys(path, &text)?;
    validate_catalogue_schema_version(path, &value)?;
    Ok(value)
}

/// Reject a catalogue whose raw JSON contains duplicate object keys at any depth.
///
/// `serde_json::Value` silently collapses duplicate keys to last-wins, so the
/// [`Value`] returned by [`read_catalogue`] can no longer expose them. The
/// canonical codec rejects duplicate keys in the `types` / `traits` /
/// `functions` maps via its `StrictMap` deserializer; without this probe an
/// add/cite/check operating on the `Value`-parsed draft would bypass that
/// rejection and let a schema-invalid catalogue through. This walks the raw
/// text and fails closed on the first duplicate.
fn reject_duplicate_keys(path: &Path, text: &str) -> Result<(), CatalogError> {
    struct NoDupKeys;

    impl<'de> serde::Deserialize<'de> for NoDupKeys {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct NoDupVisitor;

            impl<'de> serde::de::Visitor<'de> for NoDupVisitor {
                type Value = NoDupKeys;

                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("a JSON value with unique object keys")
                }

                fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    let mut seen = std::collections::BTreeSet::new();
                    while let Some(key) = access.next_key::<String>()? {
                        access.next_value::<NoDupKeys>()?;
                        if seen.contains(&key) {
                            return Err(serde::de::Error::custom(format!(
                                "duplicate object key `{key}`"
                            )));
                        }
                        seen.insert(key);
                    }
                    Ok(NoDupKeys)
                }

                fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::SeqAccess<'de>,
                {
                    while access.next_element::<NoDupKeys>()?.is_some() {}
                    Ok(NoDupKeys)
                }

                fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_i128<E>(self, _: i128) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_u128<E>(self, _: u128) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }

                fn visit_unit<E>(self) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(NoDupKeys)
                }
            }

            deserializer.deserialize_any(NoDupVisitor)
        }
    }

    serde_json::from_str::<NoDupKeys>(text)
        .map(|_| ())
        .map_err(|err| schema_error(format!("{}: {err}", path.display())))
}

/// Write a JSON tree to a catalogue file with deterministic 2-space formatting
/// and a trailing newline.
///
/// # Errors
///
/// Returns [`CatalogError::Port`] on write failure.
pub(super) fn write_catalogue(
    path: &Path,
    trusted_root: &Path,
    value: &Value,
) -> Result<(), CatalogError> {
    catalogue_present(path, trusted_root)?;
    validate_catalogue_schema_version(path, value)?;
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|err| schema_error(format!("failed to serialize {}: {err}", path.display())))?;
    text.push('\n');
    std::fs::write(path, text)
        .map_err(|err| port_error(format!("failed to write {}: {err}", path.display())))
}

/// Validate the top-level catalogue schema version before raw draft mutation.
///
/// Draft catalogues may still contain `$todo` holes, so add/import/cite cannot
/// full-decode every file through the catalogue document codec. This probe
/// keeps the same fail-closed schema-version boundary before mutating raw JSON.
pub(super) fn validate_catalogue_schema_version(
    path: &Path,
    value: &Value,
) -> Result<(), CatalogError> {
    let version = value.get("schema_version").and_then(Value::as_u64).ok_or_else(|| {
        schema_error(format!("{}: missing or invalid schema_version", path.display()))
    })?;
    if version == u64::from(SCHEMA_VERSION) {
        return Ok(());
    }
    Err(schema_error(format!(
        "{}: unsupported catalogue schema_version: file has {version}, codec expects {SCHEMA_VERSION}",
        path.display()
    )))
}

/// Insert `entry` at `document[section][key]`, rejecting a duplicate key.
///
/// # Errors
///
/// Returns [`CatalogError::DuplicateEntry`] when `key` already exists, or
/// [`CatalogError::SchemaInvalid`] when the document/section shape is wrong.
pub(super) fn insert_entry(
    document: &mut Value,
    section: &str,
    key: &CatalogEntryName,
    entry: Value,
) -> Result<(), CatalogError> {
    let root = document
        .as_object_mut()
        .ok_or_else(|| schema_error("catalogue root is not a JSON object"))?;
    let section_value = root.entry(section.to_owned()).or_insert_with(|| Value::Object(Map::new()));
    let section_obj = section_value.as_object_mut().ok_or_else(|| {
        schema_error(format!("catalogue section `{section}` is not a JSON object"))
    })?;
    if section_obj.contains_key(key.as_str()) {
        return Err(CatalogError::DuplicateEntry { entry_key: key.clone() });
    }
    section_obj.insert(key.as_str().to_owned(), entry);
    Ok(())
}

/// Scan the `$todo` holes within a single entry's subtree.
pub(super) fn scan_entry_holes(document: &Value, section: &str, entry: &str) -> Vec<DraftHole> {
    document
        .get(section)
        .and_then(|section_value| section_value.get(entry))
        .map(super::scan_todo_holes)
        .unwrap_or_default()
}

/// Find which section (`types` / `traits` / `functions`) holds `entry`.
pub(super) fn find_entry_section(document: &Value, entry: &str) -> Option<&'static str> {
    ["types", "traits", "functions"]
        .into_iter()
        .find(|&section| document.get(section).and_then(|value| value.get(entry)).is_some())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_root_from_relative_items_dir() {
        assert_eq!(workspace_root(Path::new("track/items")), PathBuf::from("."));
    }

    #[test]
    fn test_workspace_root_from_absolute_items_dir() {
        assert_eq!(workspace_root(Path::new("/ws/track/items")), PathBuf::from("/ws"));
    }

    #[test]
    fn test_track_dir_joins_valid_track_id() {
        assert_eq!(
            track_dir(Path::new("track/items"), "my-track").unwrap(),
            PathBuf::from("track/items/my-track")
        );
    }

    #[test]
    fn test_track_dir_rejects_invalid_track_id() {
        let err = track_dir(Path::new("track/items"), "../escape").unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_track_dir_rejects_symlinked_track_dir() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("my-track")).unwrap();

        let err = track_dir(temp.path(), "my-track").unwrap_err();
        assert!(matches!(err, CatalogError::Port { .. }));
    }

    #[test]
    fn test_section_for_kind() {
        assert_eq!(section_for_kind(CatalogEntryKind::Struct), "types");
        assert_eq!(section_for_kind(CatalogEntryKind::Enum), "types");
        assert_eq!(section_for_kind(CatalogEntryKind::TypeAlias), "types");
        assert_eq!(section_for_kind(CatalogEntryKind::Trait), "traits");
        assert_eq!(section_for_kind(CatalogEntryKind::Function), "functions");
    }

    #[test]
    fn test_read_catalogue_missing_file() {
        let err = read_catalogue(Path::new("/nonexistent/does-not-exist.json"), Path::new("/"))
            .unwrap_err();
        assert!(matches!(err, CatalogError::FileMissing { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_catalogue_rejects_symlink_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("real-types.json");
        let link = temp.path().join("domain-types.json");
        std::fs::write(&target, "{}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let err = read_catalogue(&link, temp.path()).unwrap_err();
        assert!(matches!(err, CatalogError::Port { .. }));
    }

    #[test]
    fn test_read_catalogue_rejects_bad_schema_version_before_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        std::fs::write(
            &path,
            r#"{"schema_version":99,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#,
        )
        .unwrap();

        let err = read_catalogue(&path, temp.path()).unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
    }

    #[test]
    fn test_read_catalogue_rejects_duplicate_object_key() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        // Two `Foo` entries under `types`: `serde_json::Value` would collapse
        // these to last-wins, hiding the duplicate from the codec's StrictMap
        // rejection. read_catalogue must fail closed on the raw duplicate.
        std::fs::write(
            &path,
            r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":"a"},"Foo":{"role":"b"}},"traits":{},"functions":{}}"#,
        )
        .unwrap();

        let err = read_catalogue(&path, temp.path()).unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
    }

    #[test]
    fn test_read_catalogue_accepts_unique_keys() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        std::fs::write(
            &path,
            r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":"a"},"Bar":{"role":"b"}},"traits":{},"functions":{}}"#,
        )
        .unwrap();

        let value = read_catalogue(&path, temp.path()).unwrap();
        assert!(value.get("types").and_then(|types| types.get("Foo")).is_some());
        assert!(value.get("types").and_then(|types| types.get("Bar")).is_some());
    }

    #[test]
    fn test_insert_entry_rejects_duplicate() {
        let mut document =
            serde_json::json!({ "types": { "Foo": {} }, "traits": {}, "functions": {} });
        let key = CatalogEntryName::try_new("Foo".to_owned()).unwrap();
        let err = insert_entry(&mut document, "types", &key, serde_json::json!({})).unwrap_err();
        assert!(matches!(err, CatalogError::DuplicateEntry { .. }));
    }

    #[test]
    fn test_insert_entry_adds_new_key() {
        let mut document = serde_json::json!({ "types": {}, "traits": {}, "functions": {} });
        let key = CatalogEntryName::try_new("Bar".to_owned()).unwrap();
        insert_entry(&mut document, "types", &key, serde_json::json!({ "role": "x" })).unwrap();
        assert!(document["types"].get("Bar").is_some());
    }

    fn single_domain_binding() -> Vec<TdddLayerBinding> {
        crate::verify::tddd_layers::parse_tddd_layers(
            r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn test_catalogue_path_resolves_configured_layer() {
        let bindings = single_domain_binding();
        let layer = LayerId::try_new("domain").unwrap();
        let path = catalogue_path(Path::new("track/items/t"), &bindings, &layer).unwrap();
        assert_eq!(path, PathBuf::from("track/items/t/domain-types.json"));
    }

    #[test]
    fn test_catalogue_path_rejects_unconfigured_layer() {
        // `usecaes` is a syntactically valid LayerId (a typo of `usecase`) that
        // is not a configured TDDD layer. It must fail closed rather than invent
        // `usecaes-types.json`, which the commit gate would silently skip.
        let bindings = single_domain_binding();
        let layer = LayerId::try_new("usecaes").unwrap();
        let err = catalogue_path(Path::new("track/items/t"), &bindings, &layer).unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
    }

    #[test]
    fn test_spec_ref_file_is_repo_relative() {
        // The recorded spec ref is a pure function of the track id, so it can
        // never embed the (possibly absolute) items_dir.
        assert_eq!(spec_ref_file("my-track-2026"), "track/items/my-track-2026/spec.json");
    }
}
