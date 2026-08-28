use domain::tddd::{ExtendedCrate, Phase1Error};
use rustdoc_types::Crate;

/// Test-only convenience entry point without a rustdoc-root translation.
pub(crate) fn phase1_build_s_and_d(
    a: ExtendedCrate,
    b: &Crate,
) -> Result<(ExtendedCrate, Crate), Phase1Error> {
    super::phase1_build_s_and_d_with_rustdoc_root(a, b, None)
}

use super::merge_definition_path_maps;
use rustdoc_types::{Id, ItemKind, ItemSummary};
use std::collections::HashMap;

#[test]
fn test_merge_definition_paths_reserves_catalogue_ids_before_remapping() {
    let summary = |name: &str| ItemSummary {
        crate_id: 0,
        path: vec!["domain".to_owned(), name.to_owned()],
        kind: ItemKind::Struct,
    };
    let baseline = HashMap::from([(Id(5), summary("BaselineA")), (Id(10), summary("BaselineB"))]);
    let catalogue = HashMap::from([
        (Id(5), summary("CatalogueAtBaselineId")),
        (Id(11), summary("CatalogueOnly")),
    ]);

    let merged = merge_definition_path_maps(&baseline, &catalogue)
        .expect("the merged definition paths must retain every summary");

    assert_eq!(merged.len(), 4);
    for expected in [
        ["domain", "BaselineA"],
        ["domain", "BaselineB"],
        ["domain", "CatalogueAtBaselineId"],
        ["domain", "CatalogueOnly"],
    ] {
        assert!(
            merged.values().any(|summary| summary.path == expected),
            "merged paths must retain {expected:?}"
        );
    }
}
