//! Application-owned port for loading attested TDDD catalogue documents.
//!
//! Filesystem access, JSON decoding, and source-byte hashing are application
//! orchestration concerns. This port keeps them behind the usecase boundary
//! while returning domain-owned attestation and error values.

use std::path::Path;

use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    AttestedCatalogueDocument, CatalogueDocumentLoaderError,
};

/// Secondary port for loading an attested catalogue document from a path.
pub trait AttestedCatalogueDocumentLoaderPort: Send + Sync {
    /// Loads the catalogue document and the declaration hash of the exact
    /// source bytes from which it was decoded.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the file is absent, cannot be read, or does
    /// not decode as a valid catalogue document.
    fn load(&self, path: &Path) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError>;
}
