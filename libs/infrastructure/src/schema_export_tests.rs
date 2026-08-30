//! Integration tests for `RustdocSchemaExporter`.
//!
//! These tests require nightly toolchain and are marked `#[ignore]` by default.
//! Run with: `cargo test --test '*' -- --ignored` or `cargo nextest run --run-ignored ignored-only`

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeSet;

    use domain::schema::{SchemaExporter, TypeKind};
    use domain::tddd::{CargoFeatureName, catalogue_v2::CrateName};
    use serde_json::Value;

    use crate::schema_export::RustdocSchemaExporter;
    use crate::schema_export_codec;

    fn workspace_root() -> std::path::PathBuf {
        let output = std::process::Command::new("cargo")
            .args(["locate-project", "--workspace", "--message-format", "plain"])
            .output()
            .unwrap();
        let manifest = String::from_utf8_lossy(&output.stdout);
        std::path::PathBuf::from(manifest.trim()).parent().unwrap().to_owned()
    }

    #[test]
    fn test_bin_target_resolution_canonicalizes_rustdoc_root_path_for_catalogue() {
        let resolution = crate::schema_export::bin_target::resolve_rustdoc_root_name(
            &workspace_root(),
            &CrateName::new("cli").unwrap(),
        )
        .unwrap();
        let path = vec![
            resolution.rustdoc_root_name().as_str().to_owned(),
            "commands".to_owned(),
            "run".to_owned(),
        ];

        assert_eq!(
            resolution.canonicalize_rustdoc_path(&path),
            vec!["cli".to_owned(), "commands".to_owned(), "run".to_owned()]
        );
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_domain_crate_contains_known_types() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        assert_eq!(schema.crate_name(), "domain");
        assert!(!schema.types().is_empty(), "expected types to be non-empty");

        // Check for well-known domain types
        let type_names: Vec<&str> = schema.types().iter().map(|t| t.name()).collect();
        assert!(
            type_names.contains(&"TrackStatus"),
            "expected TrackStatus in types, got: {type_names:?}"
        );
        assert!(
            type_names.contains(&"TaskStatus"),
            "expected TaskStatus in types, got: {type_names:?}"
        );

        // TrackStatus should be an enum
        let track_status = schema.types().iter().find(|t| t.name() == "TrackStatus").unwrap();
        assert_eq!(track_status.kind(), &TypeKind::Enum);
        assert!(!track_status.members().is_empty(), "expected TrackStatus to have variants");
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_domain_crate_contains_traits() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        assert!(!schema.traits().is_empty(), "expected traits to be non-empty");

        let trait_names: Vec<&str> = schema.traits().iter().map(|t| t.name()).collect();
        assert!(
            trait_names.contains(&"TrackReader"),
            "expected TrackReader in traits, got: {trait_names:?}"
        );
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_domain_crate_has_impls() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        assert!(!schema.impls().is_empty(), "expected impls to be non-empty");
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn test_export_infrastructure_with_semantic_dup_exposes_catalogued_public_surface() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let crate_name = CrateName::new("infrastructure".to_owned()).unwrap();
        let features = [CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()];

        let json_bytes =
            exporter.export_rustdoc_json_with_features(&crate_name, &features).unwrap();
        let document: Value = serde_json::from_slice(&json_bytes).unwrap();
        let item_names = document["index"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|item| item["name"].as_str())
            .collect::<BTreeSet<_>>();

        for expected in [
            "CodeFragmentExtractorAdapter",
            "ExtractError",
            "FastEmbedAdapter",
            "LanceDbSemanticIndexAdapter",
            "NoopSemanticIndexPort",
            "NullInsertIndexProxy",
            "PersistentIndexLock",
            "PersistentIndexLockError",
            "extract_code_fragments",
            "acquire_persistent_index_lock",
            "persistent_index_lock_path",
        ] {
            assert!(
                item_names.contains(expected),
                "semantic-dup rustdoc surface must include {expected}; found: {item_names:?}"
            );
        }
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_schema_encode_produces_parseable_json() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        let json = schema_export_codec::encode(&schema, false).unwrap();
        assert!(!json.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["crate_name"], "domain");
    }

    #[test]
    fn export_nonexistent_crate_returns_error() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let result = exporter.export("nonexistent-crate-xyz");

        assert!(result.is_err(), "expected error for nonexistent crate");
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_types_are_sorted_by_name() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        let names: Vec<&str> = schema.types().iter().map(|t| t.name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "types should be sorted by name");
    }
}
