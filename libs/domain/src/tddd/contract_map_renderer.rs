//! Secondary port for rendering the contract map from catalogue v3 documents.
//!
//! `ContractMapRenderer` defines the rendering boundary between the domain
//! layer and the infrastructure adapter that performs the actual mermaid
//! flowchart generation (ADR 2026-05-13-0000 Decision E-3c). The adapter
//! lives in `libs/infrastructure/src/tddd/contract_map_renderer_adapter.rs`
//! and is injected into the usecase layer via this port.
//!
//! `ContractMapRendererError` is the canonical error type returned by
//! renderer implementations. It covers the three failure modes identified
//! in the catalogue declaration for this track: a missing style config,
//! an unparseable style config, and a generic rendering failure.

use crate::tddd::LayerId;
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::contract_map_content::ContractMapContent;
use crate::tddd::contract_map_options::ContractMapRenderOptions;

/// Secondary port for rendering the contract map from catalogue v3 documents
/// (ADR 2026-05-13-0000 Decision E-3c).
///
/// The adapter in the infrastructure layer uses `syn` to parse `TypeRef`
/// strings and emits a mermaid flowchart. The port accepts
/// `&[CatalogueDocument]` (self-descriptive: each document carries
/// `crate_name` + `layer`) and `layer_order` for stable left-to-right
/// subgraph layout. Style settings are opaque to the port — the adapter
/// loads them internally from `.harness/config/contract-map-style.toml`
/// fail-closed (ADR Decision C + CN-03).
pub trait ContractMapRenderer: Send + Sync {
    /// Render the contract map from the given catalogues and layer order.
    ///
    /// # Errors
    ///
    /// Returns [`ContractMapRendererError::StyleConfigNotFound`] if the style
    /// configuration file is absent (fail-closed).
    ///
    /// Returns [`ContractMapRendererError::StyleConfigParse`] if the style
    /// configuration file cannot be parsed.
    ///
    /// Returns [`ContractMapRendererError::RenderFailed`] if any other
    /// rendering failure occurs (e.g. a malformed `TypeRef` string).
    fn render(
        &self,
        catalogues: &[CatalogueDocument],
        layer_order: &[LayerId],
        opts: &ContractMapRenderOptions,
    ) -> Result<ContractMapContent, ContractMapRendererError>;
}

/// Error variants returned by [`ContractMapRenderer`] implementations.
///
/// Variant inventory matches the `domain-types.json` declaration for the
/// `contract-map-v3-2026-05-15` track.
///
/// `StyleConfigNotFound`: the style TOML is absent (fail-closed per CN-03).
/// `StyleConfigParse`: the TOML could not be parsed.
/// `RenderFailed`: a generic rendering failure.
#[derive(Debug, thiserror::Error)]
pub enum ContractMapRendererError {
    /// The style configuration file is absent at the expected path.
    /// The renderer fails closed rather than falling back to a default.
    #[error("style config not found at {}", .path.display())]
    StyleConfigNotFound { path: std::path::PathBuf },

    /// The style configuration file exists but could not be parsed.
    #[error("style config parse error at {}: {reason}", .path.display())]
    StyleConfigParse { path: std::path::PathBuf, reason: String },

    /// A generic rendering failure occurred (e.g. malformed `TypeRef`).
    #[error("render failed: {reason}")]
    RenderFailed { reason: String },
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::*;

    #[test]
    fn test_contract_map_renderer_error_style_config_not_found_formats_path() {
        let err = ContractMapRendererError::StyleConfigNotFound {
            path: std::path::PathBuf::from("/some/path/contract-map-style.toml"),
        };
        let msg = err.to_string();
        assert!(msg.contains("style config not found"));
        assert!(msg.contains("contract-map-style.toml"));
        // Error chain: no underlying source — the domain port expresses failure as a
        // human-readable reason string rather than wrapping an infra error type.
        assert!(err.source().is_none());
    }

    #[test]
    fn test_contract_map_renderer_error_style_config_parse_includes_reason() {
        let err = ContractMapRendererError::StyleConfigParse {
            path: std::path::PathBuf::from("/harness/config/contract-map-style.toml"),
            reason: "missing field `fill`".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("style config parse error"));
        assert!(msg.contains("missing field `fill`"));
        // The parse reason is encoded as a String field (see domain-types.json catalogue
        // declaration). The adapter stringifies the underlying toml error before crossing
        // the domain boundary, so `source()` is intentionally None here.
        assert!(err.source().is_none());
    }

    #[test]
    fn test_contract_map_renderer_error_render_failed_includes_reason() {
        let err = ContractMapRendererError::RenderFailed {
            reason: "unexpected end of input".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("render failed"));
        assert!(msg.contains("unexpected end of input"));
        // The render failure reason is a String (catalogue contract). The adapter
        // converts the infra-level error to a string, so `source()` returns None.
        assert!(err.source().is_none());
    }
}
