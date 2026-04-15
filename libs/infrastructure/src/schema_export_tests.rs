//! Integration tests for `RustdocSchemaExporter`.
//!
//! These tests require nightly toolchain and are marked `#[ignore]` by default.
//! Run with: `cargo test --test '*' -- --ignored` or `cargo nextest run --run-ignored ignored-only`

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use domain::schema::{SchemaExporter, TypeKind};

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
