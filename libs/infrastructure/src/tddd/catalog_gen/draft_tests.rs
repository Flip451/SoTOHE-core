//! Unit tests for the T005 draft layer (`scan_todo_holes` / `try_complete`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use serde_json::json;

use super::{CatalogDraftError, scan_todo_holes, try_complete};

/// A minimal hole-free catalogue document (schema_version 5) as a JSON string.
fn empty_catalogue() -> serde_json::Value {
    json!({
        "schema_version": 5,
        "crate_name": "domain",
        "layer": "domain",
        "types": {},
        "traits": {},
        "functions": {}
    })
}

#[test]
fn test_scan_finds_leaf_hole() {
    let value = json!({ "types": { "Foo": { "docs": { "$todo": "describe the invariant" } } } });
    let holes = scan_todo_holes(&value);
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].path().as_str(), "types.Foo.docs");
    assert_eq!(holes[0].instruction().as_str(), "describe the invariant");
}

#[test]
fn test_scan_finds_object_hole() {
    let value = json!({ "types": { "Foo": { "methods": { "$todo": "design the methods" } } } });
    let holes = scan_todo_holes(&value);
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].path().as_str(), "types.Foo.methods");
}

#[test]
fn test_scan_finds_array_element_hole() {
    let value = json!({ "functions": { "f": { "params": [ { "$todo": "first param" } ] } } });
    let holes = scan_todo_holes(&value);
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].path().as_str(), "functions.f.params[0]");
}

#[test]
fn test_scan_finds_section_hole() {
    let value = json!({ "types": { "$todo": "list all the types" } });
    let holes = scan_todo_holes(&value);
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].path().as_str(), "types");
    assert_eq!(holes[0].instruction().as_str(), "list all the types");
}

#[test]
fn test_scan_empty_for_hole_free_document() {
    let holes = scan_todo_holes(&empty_catalogue());
    assert!(holes.is_empty());
}

#[test]
fn test_scan_collects_multiple_holes() {
    let value = json!({
        "types": {
            "Foo": {
                "docs": { "$todo": "doc" },
                "methods": { "$todo": "methods" }
            }
        }
    });
    let holes = scan_todo_holes(&value);
    assert_eq!(holes.len(), 2);
    let paths: Vec<&str> = holes.iter().map(|hole| hole.path().as_str()).collect();
    assert!(paths.contains(&"types.Foo.docs"));
    assert!(paths.contains(&"types.Foo.methods"));
}

#[test]
fn test_try_complete_returns_document_for_hole_free_draft() {
    let document = try_complete(empty_catalogue()).expect("hole-free draft should decode");
    assert_eq!(document.crate_name.as_str(), "domain");
    assert_eq!(document.layer.as_ref(), "domain");
    assert!(document.types.is_empty());
}

#[test]
fn test_try_complete_reports_incomplete_with_hole_list() {
    let value = json!({
        "schema_version": 5,
        "crate_name": "domain",
        "layer": "domain",
        "types": { "Foo": { "role": { "$todo": "pick the role" } } },
        "traits": {},
        "functions": {}
    });
    match try_complete(value) {
        Err(CatalogDraftError::Incomplete { holes }) => {
            assert_eq!(holes.len(), 1);
            assert_eq!(holes[0].path().as_str(), "types.Foo.role");
        }
        other => panic!("expected Incomplete, got {other:?}"),
    }
}

#[test]
fn test_try_complete_reports_codec_error_on_bad_schema_version() {
    let value = json!({
        "schema_version": 99,
        "crate_name": "domain",
        "layer": "domain",
        "types": {},
        "traits": {},
        "functions": {}
    });
    assert!(matches!(try_complete(value), Err(CatalogDraftError::Codec { .. })));
}
