//! Artifact loading helpers for the pure-read check interactor.

use std::path::PathBuf;

use domain::SpecDocumentLoaderPort;
use domain::TrackId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::test_obligation::errors::ObligationCheckError;

use super::super::LoadedCatalogueDocument;
use super::super::check_contract::CheckTestObligationsCommand;
use super::super::check_support::{SpecElement, spec_elements_from_document};

/// Loads the catalogue documents named by a check command.
pub(super) fn load_catalogues(
    reader: &(dyn CatalogueDocumentLoaderPort + Send + Sync),
    cmd: &CheckTestObligationsCommand,
) -> Result<Vec<LoadedCatalogueDocument>, ObligationCheckError> {
    let mut catalogues = Vec::with_capacity(cmd.input.catalogue_paths().len());
    for path in cmd.input.catalogue_paths() {
        let document = reader.load(path).map_err(ObligationCheckError::CatalogueLoad)?;
        catalogues.push(LoadedCatalogueDocument::new(path, document));
    }
    Ok(catalogues)
}

/// Reports whether the command names at least one existing catalogue.
pub(super) fn has_catalogue(
    reader: &(dyn CatalogueDocumentLoaderPort + Send + Sync),
    cmd: &CheckTestObligationsCommand,
) -> Result<bool, ObligationCheckError> {
    cmd.input.catalogue_paths().iter().try_fold(false, |exists, path| match reader.load(path) {
        Ok(_) => Ok(true),
        Err(CatalogueDocumentLoaderError::NotFound { .. }) => Ok(exists),
        Err(error) => Err(ObligationCheckError::CatalogueLoad(error)),
    })
}

/// Loads and normalizes the active track's specification elements.
pub(super) fn spec_elements(
    reader: &(dyn SpecDocumentLoaderPort + Send + Sync),
    track_id: &TrackId,
) -> Result<Vec<SpecElement>, ObligationCheckError> {
    let path = PathBuf::from(format!("track/items/{}/spec.json", track_id.as_ref()));
    let spec = reader.load(&path).map_err(ObligationCheckError::SpecLoad)?;
    Ok(spec_elements_from_document(&spec))
}
