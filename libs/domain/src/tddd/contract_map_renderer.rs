//! `ContractMapRenderer` port trait and `ContractMapRendererError` error type.
//!
//! Placement: domain layer (Decision E-3c / Decision P-4).
//!
//! * [`ContractMapRenderer`] is the secondary port trait that infrastructure
//!   adapters implement. `RenderContractMapInteractor` injects an `R: ContractMapRenderer`
//!   and calls `self.renderer.render(...)` to produce the mermaid content
//!   (Decision P-5 / IN-01 / IN-05 / IN-22 / IN-23).
//!
//! * [`ContractMapRendererError`] covers the two fail-closed error cases for
//!   style config loading (CN-02 / Decision P-4 / IN-22):
//!   - `StyleConfigNotFound` — file is absent (fail-closed, AC-11).
//!   - `StyleConfigInvalid` — file exists but cannot be parsed (fail-closed, AC-11).
//!   - `RenderFailed` — open placeholder for future rendering errors.
//!
//! No serde / TOML dependencies are introduced in this module (CN-03 / Decision P-4).

use std::path::PathBuf;

use crate::tddd::LayerId;
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::contract_map_content::ContractMapContent;
use crate::tddd::contract_map_options::ContractMapRenderOptions;

/// Secondary port for v3 contract-map rendering.
///
/// Infrastructure adapter (`ContractMapRendererAdapter`) implements this port.
/// `RenderContractMapInteractor` injects it via generic `R`.
///
/// # Contract
///
/// * `catalogues` — flat slice of all layer catalogues; each doc carries its
///   own `layer: LayerId` and `crate_name: CrateName` for self-description.
/// * `layer_order` — topologically sorted `LayerId` list (from
///   `CatalogueLoader.load_all`), optionally pre-filtered by the interactor
///   per `RenderContractMapCommand.layer_filter`.
/// * `opts` — render options forwarded verbatim from the command; the adapter
///   may read `opts.layers` as a forward-compatibility stub (not consumed in
///   this track — layer filtering happens in the interactor).
///
/// (Decision E-3c + Decision P-1 + IN-01 + IN-05)
pub trait ContractMapRenderer: Send + Sync {
    /// Render the contract map for the given catalogues.
    ///
    /// # Errors
    ///
    /// Returns [`ContractMapRendererError`] when style configuration is absent
    /// or invalid (fail-closed, CN-02 / AC-11), or when rendering fails.
    fn render(
        &self,
        catalogues: &[CatalogueDocument],
        layer_order: &[LayerId],
        opts: &ContractMapRenderOptions,
    ) -> Result<ContractMapContent, ContractMapRendererError>;
}

/// Error returned by [`ContractMapRenderer::render`].
///
/// Variants cover fail-closed style config loading (CN-02 / AC-11) and
/// rendering failures (open placeholder). (Decision P-4 + IN-22)
#[derive(Debug)]
pub enum ContractMapRendererError {
    /// The style configuration file was not found at the expected path.
    /// Fail-closed: absent config is always an error (CN-02 / AC-11).
    StyleConfigNotFound { path: PathBuf },

    /// The style configuration file exists but could not be parsed as TOML.
    /// Fail-closed: invalid config is always an error (CN-02 / AC-11).
    StyleConfigInvalid { path: PathBuf, reason: String },

    /// Rendering failed for any other reason.
    RenderFailed { reason: String },
}

impl std::fmt::Display for ContractMapRendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StyleConfigNotFound { path } => {
                write!(f, "contract-map style configuration not found: {}", path.display())
            }
            Self::StyleConfigInvalid { path, reason } => {
                write!(
                    f,
                    "contract-map style configuration is invalid at {}: {}",
                    path.display(),
                    reason
                )
            }
            Self::RenderFailed { reason } => {
                write!(f, "contract-map rendering failed: {reason}")
            }
        }
    }
}

impl std::error::Error for ContractMapRendererError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// T001 unit test: each Display variant produces an appropriate message.
    #[test]
    fn test_style_config_not_found_display_contains_path() {
        let path = PathBuf::from("/some/path/contract-map-style.toml");
        let err = ContractMapRendererError::StyleConfigNotFound { path: path.clone() };
        let msg = err.to_string();
        assert!(
            msg.contains("/some/path/contract-map-style.toml"),
            "Display must include path; got: {msg}"
        );
        assert!(msg.contains("not found"), "Display must mention 'not found'; got: {msg}");
    }

    #[test]
    fn test_style_config_invalid_display_contains_path_and_reason() {
        let path = PathBuf::from("/conf/style.toml");
        let err = ContractMapRendererError::StyleConfigInvalid {
            path: path.clone(),
            reason: "unexpected key `foo`".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("/conf/style.toml"), "Display must include path; got: {msg}");
        assert!(msg.contains("unexpected key `foo`"), "Display must include reason; got: {msg}");
    }

    #[test]
    fn test_render_failed_display_contains_reason() {
        let err = ContractMapRendererError::RenderFailed { reason: "graph too large".to_owned() };
        let msg = err.to_string();
        assert!(msg.contains("graph too large"), "Display must include reason; got: {msg}");
        assert!(
            msg.contains("rendering failed"),
            "Display must mention 'rendering failed'; got: {msg}"
        );
    }

    /// Verify that ContractMapRendererError implements std::error::Error.
    #[test]
    fn test_renderer_error_implements_std_error() {
        let err: &dyn std::error::Error =
            &ContractMapRendererError::RenderFailed { reason: "test".to_owned() };
        // source() should return None (no chained cause).
        assert!(err.source().is_none());
    }
}
