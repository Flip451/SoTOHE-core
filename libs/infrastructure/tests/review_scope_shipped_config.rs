//! Regression test pinning the shipped `.harness/config/review-scope.json`
//! LAYER groups to `architecture-rules.json`
//! (track `template-extraction-boundary-2026-07-06`, T012; spec IN-09 / AC-09 /
//! CN-08; ADR D5-b).
//!
//! The v2 loader ([`infrastructure::review_v2::load_v2_scope_config`]) cross-
//! checks the review-scope LAYER groups against `architecture-rules.json` and
//! fails closed on drift (a missing layer group, or a group whose patterns are
//! not exactly `["<layer-path>/**"]`). This test drives that guard over the
//! *real* repo files so a future `architecture-rules.json` edit that forgets to
//! update `review-scope.json` (or vice versa) fails CI here, rather than only in
//! a live `sotp review` run.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::review_v2::{FilePath, MainScopeName, ScopeName};
use infrastructure::review_v2::load_v2_scope_config;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Layer crate names enumerated from the real `architecture-rules.json` at test
/// run time (not hard-coded), in file declaration order.
fn arch_layer_crates() -> Vec<String> {
    let path = repo_root().join("architecture-rules.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    value["layers"]
        .as_array()
        .unwrap_or_else(|| panic!("{} must have a 'layers' array", path.display()))
        .iter()
        .map(|layer| {
            layer["crate"]
                .as_str()
                .unwrap_or_else(|| panic!("layer entry missing 'crate' string: {layer}"))
                .to_owned()
        })
        .collect()
}

/// The shipped review-scope.json must load without an `InvalidField` drift
/// error, proving every `architecture-rules.json` layer has a matching group
/// with the expected `["<layer-path>/**"]` pattern.
#[test]
fn test_shipped_review_scope_layer_groups_match_arch_rules() {
    let root = repo_root();
    let scope_path = root.join(".harness/config/review-scope.json");
    let track_id = TrackId::try_new("template-extraction-boundary-2026-07-06").unwrap();

    let config = load_v2_scope_config(&scope_path, &track_id, &root).unwrap_or_else(|e| {
        panic!(
            "shipped review-scope.json failed the arch-rules layer-group guard: {e}\n\
             (a layer in architecture-rules.json has no matching review-scope group, \
             or a group's patterns drifted from [\"<layer-path>/**\"])"
        )
    });

    // Every arch-rules layer crate must be a configured review scope.
    for crate_name in arch_layer_crates() {
        let scope = ScopeName::Main(MainScopeName::new(crate_name.as_str()).unwrap());
        assert!(
            config.contains_scope(&scope),
            "architecture-rules.json layer '{crate_name}' has no matching review-scope group"
        );
    }
}

/// The derived obligations artifact is operational, but the author-written
/// test bindings remain in content review.
#[test]
fn test_shipped_review_scope_classifies_obligations_as_operational_and_test_bindings_as_content() {
    let root = repo_root();
    let scope_path = root.join(".harness/config/review-scope.json");
    let fixture_track_id = "review-scope-regression-fixture";
    let track_id = TrackId::try_new(fixture_track_id).unwrap();
    let config = load_v2_scope_config(&scope_path, &track_id, &root).unwrap();

    let obligations =
        FilePath::new(format!("track/items/{fixture_track_id}/obligations.json")).unwrap();
    let test_bindings =
        FilePath::new(format!("track/items/{fixture_track_id}/test-bindings.json")).unwrap();

    let classified = config.classify(&[obligations.clone(), test_bindings.clone()]);

    assert!(
        !classified.values().any(|files| files.contains(&obligations)),
        "derived obligations.json must be excluded as review-operational"
    );
    assert_eq!(
        classified.get(&ScopeName::Other).map(Vec::as_slice),
        Some([test_bindings].as_slice()),
        "author-written test-bindings.json must remain in content review"
    );
}
