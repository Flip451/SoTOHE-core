//! `BaselineDocument` — wrapper that groups a `rustdoc_types::Crate` with its layer
//! identity and crate name.
//!
//! Symmetric to `CatalogueDocument` in the Contract Map pipeline. Placing layer
//! identity inside the document lets callers (e.g., `RenderBaselineGraphInteractor`)
//! group documents by layer without any additional context. This follows
//! Decision A-r3 of ADR `2026-05-22-1507-baseline-graph-renderer-rustdoc-adaptation.md`.
//!
//! ## Design
//!
//! - `layer`: `LayerId` — identifies the architectural layer (e.g. `"domain"`).
//! - `crate_name`: `CrateName` — the Rust crate (e.g. `"domain"`).
//! - `krate`: `rustdoc_types::Crate` — the raw rustdoc JSON payload; no extension
//!   added in the domain layer (ADR `2026-05-08-0258` D2).
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec layer handles
//! JSON deserialization of `rustdoc_types::Crate`.

use crate::tddd::catalogue_v2::identifiers::CrateName;
use crate::tddd::layer_id::LayerId;

// ---------------------------------------------------------------------------
// BaselineDocument
// ---------------------------------------------------------------------------

/// Wrapper struct grouping a `rustdoc_types::Crate` with its layer identity
/// (`LayerId`) and crate name (`CrateName`).
///
/// Contract Map `CatalogueDocument`-symmetric: the renderer can group documents
/// by layer autonomously without caller-provided context. Internal `krate` field
/// holds `rustdoc_types::Crate` pure — no extension (ADR 2026-05-08-0258 D2).
///
/// (Decision A-r3 / IN-01 / AC-01)
#[derive(Debug, Clone)]
pub struct BaselineDocument {
    /// Architectural layer identifier (e.g. `"domain"`, `"usecase"`, `"infrastructure"`).
    pub layer: LayerId,
    /// Rust crate name (e.g. `"domain"`, `"usecase"`, `"infrastructure"`).
    pub crate_name: CrateName,
    /// Raw rustdoc JSON payload; no domain extension.
    pub krate: rustdoc_types::Crate,
}

impl BaselineDocument {
    /// Creates a new `BaselineDocument`.
    ///
    /// # Errors
    ///
    /// This constructor is infallible; validation of `layer` and `crate_name` was
    /// already performed by their respective constructors.
    #[must_use]
    pub fn new(layer: LayerId, crate_name: CrateName, krate: rustdoc_types::Crate) -> Self {
        Self { layer, crate_name, krate }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Build a minimal `rustdoc_types::Crate` for test purposes.
    ///
    /// Uses `serde_json` deserialization to construct the struct in a forward-compatible
    /// way — this avoids depending on the exact field list of `rustdoc_types::Crate`
    /// in test code, which would break whenever `rustdoc-types` adds a new required field.
    fn minimal_crate() -> rustdoc_types::Crate {
        let json = format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {},
                "target": {{"triple": "", "target_features": []}}
            }}"#,
            rustdoc_types::FORMAT_VERSION
        );
        serde_json::from_str(&json).expect("minimal_crate JSON must be valid")
    }

    #[test]
    fn test_baseline_document_new_stores_layer() {
        let layer = LayerId::try_new("domain").unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let doc = BaselineDocument::new(layer.clone(), crate_name, minimal_crate());
        assert_eq!(doc.layer, layer);
    }

    #[test]
    fn test_baseline_document_new_stores_crate_name() {
        let layer = LayerId::try_new("domain").unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let doc = BaselineDocument::new(layer, crate_name.clone(), minimal_crate());
        assert_eq!(doc.crate_name, crate_name);
    }

    #[test]
    fn test_baseline_document_new_stores_krate_root() {
        let layer = LayerId::try_new("domain").unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let krate = minimal_crate();
        let root_id = krate.root;
        let doc = BaselineDocument::new(layer, crate_name, krate);
        assert_eq!(doc.krate.root, root_id);
    }

    #[test]
    fn test_baseline_document_clone_produces_independent_copy() {
        let layer = LayerId::try_new("usecase").unwrap();
        let crate_name = CrateName::new("usecase").unwrap();
        let doc = BaselineDocument::new(layer, crate_name, minimal_crate());
        let cloned = doc.clone();
        assert_eq!(doc.layer, cloned.layer);
        assert_eq!(doc.crate_name, cloned.crate_name);
    }

    #[test]
    fn test_baseline_document_debug_does_not_panic() {
        let layer = LayerId::try_new("infrastructure").unwrap();
        let crate_name = CrateName::new("infrastructure").unwrap();
        let doc = BaselineDocument::new(layer, crate_name, minimal_crate());
        let _ = format!("{doc:?}");
    }

    #[test]
    fn test_baseline_document_with_different_layers() {
        // Verify that distinct layers are stored independently.
        let doc_domain = BaselineDocument::new(
            LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            minimal_crate(),
        );
        let doc_usecase = BaselineDocument::new(
            LayerId::try_new("usecase").unwrap(),
            CrateName::new("usecase").unwrap(),
            minimal_crate(),
        );
        assert_ne!(doc_domain.layer, doc_usecase.layer);
        assert_ne!(doc_domain.crate_name, doc_usecase.crate_name);
    }
}
