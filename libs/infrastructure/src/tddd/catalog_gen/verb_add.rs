//! `sotp catalog add` (D2 / D6 / IN-03 / AC-03 / AC-08): append an annotated
//! entry skeleton for a new type to a layer's catalogue.

use std::path::Path;
use std::str::FromStr;

use domain::tddd::catalog_gen::{CatalogEntryKind, CatalogEntryName};
use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::{FunctionPath, Identifier};
use domain::tddd::semantic_verify::CatalogueEntryKey;
use serde_json::Value;
use usecase::catalog_gen::{CatalogAddCommand, CatalogError, CatalogWriteReport};

use super::fs_access::{
    catalogue_path, insert_entry, load_bindings, read_catalogue, scan_entry_holes, schema_error,
    section_for_kind, spec_ref_file, track_dir, write_catalogue,
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
    let path = catalogue_path(&dir, &bindings, &command.layer)?;
    let spec_file = spec_ref_file(track_id);
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
    let crate_name = document
        .get("crate_name")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("catalogue is missing a string `crate_name`"))?
        .to_owned();
    validate_entry_name(command.kind, &command.name, &crate_name)?;
    let name = CatalogEntryName::try_new(command.name.clone())
        .map_err(|err| schema_error(format!("invalid entry name `{}`: {err}", command.name)))?;
    let (entry, trait_impls, inherent_impls) = build_add_entry(command, spec_file, spec_anchors)?;
    let section = section_for_kind(command.kind);
    insert_entry(&mut document, section, &name, entry)?;
    append_top_level_declarations(&mut document, "trait_impls", trait_impls)?;
    append_top_level_declarations(&mut document, "inherent_impls", inherent_impls)?;
    write_catalogue(path, trusted_root, &document)?;
    let holes = scan_entry_holes(&document, section, name.as_str());
    Ok(CatalogWriteReport {
        file_path: path.display().to_string(),
        entry_key: name.as_str().to_owned(),
        holes,
    })
}

/// Append document-level declarations, if any.
///
/// # Errors
///
/// Returns [`CatalogError::SchemaInvalid`] when the draft already carries a
/// top-level declaration list that is not a JSON array (e.g. a `$todo` hole or a
/// hand-edited scalar). `as_array_mut` would return `None` there, and appending
/// into nothing would report success while silently dropping the parsed
/// declarations; the draft must be normalised (e.g. via a codec pass) so the
/// field is an array first.
fn append_top_level_declarations(
    document: &mut Value,
    field: &str,
    declarations: Vec<Value>,
) -> Result<(), CatalogError> {
    if declarations.is_empty() {
        return Ok(());
    }
    let root = document
        .as_object_mut()
        .ok_or_else(|| schema_error("catalogue root is not a JSON object"))?;
    let list = root.entry(field.to_owned()).or_insert_with(|| Value::Array(Vec::new()));
    let array = list.as_array_mut().ok_or_else(|| {
        schema_error(format!(
            "catalogue top-level `{field}` is not a JSON array; normalise the draft \
             (e.g. run it through a codec pass) before adding declarations"
        ))
    })?;
    array.extend(declarations);
    Ok(())
}

/// Fail-closed pre-write validation of `--name` against the entry kind and the
/// target catalogue's crate.
///
/// - Function entries must be crate-qualified (`<crate>::…::<fn_name>`) so the
///   codec's cross-crate function-path guard accepts the key.
/// - Type and trait entries validate through the loose `CatalogueEntryKey`
///   boundary, which accepts an optional fully qualified path.
fn validate_entry_name(
    kind: CatalogEntryKind,
    name: &str,
    crate_name: &str,
) -> Result<(), CatalogError> {
    match kind {
        CatalogEntryKind::Function => validate_function_name(name, crate_name),
        CatalogEntryKind::Struct | CatalogEntryKind::Enum | CatalogEntryKind::TypeAlias => {
            validate_catalogue_entry_key(name)
        }
        CatalogEntryKind::Trait => validate_catalogue_entry_key(name),
    }
}

/// A function entry name must validate through `FunctionPath` and have the
/// leading crate segment equal to the target catalogue crate.
fn validate_function_name(name: &str, crate_name: &str) -> Result<(), CatalogError> {
    let path = FunctionPath::from_str(name).map_err(|err| {
        name_error(format!("function entry name `{name}` must be a valid FunctionPath: {err}"))
    })?;
    if path.crate_name.as_str() == crate_name {
        return Ok(());
    }
    Err(name_error(format!(
        "function entry name `{name}` must be crate-qualified as `{crate_name}::…::<fn_name>`"
    )))
}

/// A type or trait entry key is catalogue notation and may include its
/// crate/module qualification. Structural identity resolution remains outside
/// this draft writer; each path segment must still be a valid catalogue
/// identifier so an invalid key cannot be persisted for later resolution.
fn validate_catalogue_entry_key(name: &str) -> Result<(), CatalogError> {
    let key = CatalogueEntryKey::try_new(name.to_owned()).map_err(|err| {
        name_error(format!("entry name `{name}` must be a valid CatalogueEntryKey: {err}"))
    })?;
    for segment in key.as_str().split("::") {
        Identifier::new(segment.to_owned()).map_err(|err| {
            name_error(format!(
                "entry name `{name}` contains invalid path segment `{segment}`: {err}"
            ))
        })?;
    }
    Ok(())
}

/// Build a [`CatalogError::ParseFragment`] for a rejected entry name.
fn name_error(message: String) -> CatalogError {
    CatalogError::ParseFragment { message: FreeText::new(message) }
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
            inherent_methods: vec![],
            generics: vec![],
            where_predicates: vec![],
            impl_generics: vec![],
            impl_where_predicates: vec![],
            inherent_impl_generics: vec![],
            inherent_impl_where_predicates: vec![],
        }
    }

    fn function_command(name: &str) -> CatalogAddCommand {
        CatalogAddCommand {
            layer: LayerId::try_new("domain").unwrap(),
            kind: CatalogEntryKind::Function,
            name: name.to_owned(),
            role: "FreeFunction".to_owned(),
            anchors: vec!["IN-01".to_owned()],
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

    // Finding 4: a bare function name is rejected — the codec requires the key
    // to be crate-qualified.
    #[test]
    fn test_add_entry_function_requires_crate_qualified_name() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let err = add_entry_to_file(
            &path,
            temp.path(),
            &function_command("run"),
            "spec.json",
            &anchors(),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    // Finding 4: a crate-qualified function name whose crate matches the
    // catalogue is accepted and keyed verbatim.
    #[test]
    fn test_add_entry_function_accepts_crate_qualified_name() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let report = add_entry_to_file(
            &path,
            temp.path(),
            &function_command("domain::tddd::run"),
            "spec.json",
            &anchors(),
        )
        .unwrap();
        assert_eq!(report.entry_key, "domain::tddd::run");
    }

    // Finding 4: a function name qualified with a different crate is rejected.
    #[test]
    fn test_add_entry_function_rejects_wrong_crate() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let err = add_entry_to_file(
            &path,
            temp.path(),
            &function_command("other::run"),
            "spec.json",
            &anchors(),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ParseFragment { .. }));
    }

    // Fully qualified type keys are accepted so same-named declarations can
    // coexist in one catalogue.
    #[test]
    fn test_add_entry_type_accepts_qualified_name() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let report = add_entry_to_file(
            &path,
            temp.path(),
            &struct_command("foo::Bar"),
            "spec.json",
            &anchors(),
        )
        .unwrap();
        assert_eq!(report.entry_key, "foo::Bar");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(written["types"].get("foo::Bar").is_some());
    }

    #[test]
    fn test_add_entry_qualified_name_keeps_inherent_impl_target_qualified() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut command = struct_command("domain::models::Bar");
        command.inherent_methods = vec!["fn value(&self) -> u32".to_owned()];

        add_entry_to_file(&path, temp.path(), &command, "spec.json", &anchors()).unwrap();

        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written["inherent_impls"][0]["type_name"],
            serde_json::json!("domain::models::Bar")
        );
    }

    #[test]
    fn test_add_entry_rejects_invalid_qualified_name_segments() {
        for invalid_name in ["domain::::Port", "domain::bad-name", "Bad Name"] {
            let temp = tempfile::tempdir().unwrap();
            let path = seed_catalogue(temp.path());
            let err = add_entry_to_file(
                &path,
                temp.path(),
                &struct_command(invalid_name),
                "spec.json",
                &anchors(),
            )
            .unwrap_err();
            assert!(matches!(err, CatalogError::ParseFragment { .. }));
        }
    }

    // A draft whose top-level `trait_impls` is a `$todo` hole (not an array)
    // must reject an `add` that carries trait impls, rather than reporting
    // success while silently dropping them. The file must be left untouched.
    #[test]
    fn test_add_entry_rejects_malformed_trait_impls() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        let seeded = serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": {},
            "traits": {},
            "functions": {},
            "trait_impls": { "$todo": "list the trait impls declared in this layer" }
        }))
        .unwrap();
        std::fs::write(&path, &seeded).unwrap();

        let mut command = struct_command("Foo");
        command.fields = vec!["message: String".to_owned()];
        command.trait_impls = vec!["From<CodecError>".to_owned()];

        let err =
            add_entry_to_file(&path, temp.path(), &command, "spec.json", &anchors()).unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
        // No silent drop: the malformed draft is neither mutated nor rewritten.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), seeded);
    }

    #[test]
    fn test_add_entry_appends_inherent_impls() {
        let temp = tempfile::tempdir().unwrap();
        let path = seed_catalogue(temp.path());
        let mut command = struct_command("Foo");
        command.inherent_methods = vec!["fn value(&self) -> u32".to_owned()];

        add_entry_to_file(&path, temp.path(), &command, "spec.json", &anchors()).unwrap();

        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["inherent_impls"][0]["type_name"], serde_json::json!("Foo"));
        assert_eq!(written["inherent_impls"][0]["methods"][0]["name"], serde_json::json!("value"));
    }

    #[test]
    fn test_add_entry_rejects_malformed_inherent_impls() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("domain-types.json");
        let seeded = serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 5,
            "crate_name": "domain",
            "layer": "domain",
            "types": {},
            "traits": {},
            "functions": {},
            "inherent_impls": { "$todo": "list the inherent impls declared in this layer" }
        }))
        .unwrap();
        std::fs::write(&path, &seeded).unwrap();

        let mut command = struct_command("Foo");
        command.inherent_methods = vec!["fn value(&self) -> u32".to_owned()];

        let err =
            add_entry_to_file(&path, temp.path(), &command, "spec.json", &anchors()).unwrap_err();
        assert!(matches!(err, CatalogError::SchemaInvalid { .. }));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), seeded);
    }
}
