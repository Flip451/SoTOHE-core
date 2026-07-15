//! Existing rustdoc reuse helpers for the type-signal evaluator.

use std::path::PathBuf;

#[cfg(feature = "test-helpers")]
use std::collections::BTreeMap;

use domain::schema::SchemaExportError;
use domain::tddd::type_signals_doc::{
    CatalogueDeclarationHash, ImplementationInputHash, TypeSignalsDocument,
    TypeSignalsReuseDecision, decide_type_signals_reuse,
};

use super::{MAX_RUSTDOC_JSON_BYTES, read_utf8_file_limited};
use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;

pub(super) trait RustdocJsonPathProvider {
    fn export_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError>;
    fn existing_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError>;
}

pub(super) fn existing_rustdoc_content(
    exporter: &impl RustdocJsonPathProvider,
    target_crate: &str,
) -> Option<String> {
    let path = exporter.existing_rustdoc_json_path(target_crate).ok()?;
    let content = read_utf8_file_limited(&path, MAX_RUSTDOC_JSON_BYTES).ok()?;
    BaselineRustdocCodec::from_json(&content).ok()?;
    Some(content)
}

pub(super) fn reuse_decision_for_recorded_document(
    recorded: Option<&TypeSignalsDocument>,
    current_declaration_hash: &CatalogueDeclarationHash,
    current_implementation_input_hash: Option<&ImplementationInputHash>,
) -> TypeSignalsReuseDecision {
    recorded.map_or(TypeSignalsReuseDecision::ReextractAndEvaluate, |document| {
        decide_type_signals_reuse(
            document.declaration_hash(),
            document.implementation_input_hash(),
            current_declaration_hash,
            current_implementation_input_hash,
        )
    })
}

impl RustdocJsonPathProvider for RustdocSchemaExporter {
    fn export_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        Self::export_rustdoc_json_path(self, crate_name)
    }

    fn existing_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        Self::existing_rustdoc_json_path(self, crate_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{RustdocJsonPathProvider, existing_rustdoc_content};
    use domain::schema::SchemaExportError;
    use std::path::PathBuf;

    struct FixedPathProvider(PathBuf);

    impl RustdocJsonPathProvider for FixedPathProvider {
        fn export_rustdoc_json_path(&self, _: &str) -> Result<PathBuf, SchemaExportError> {
            Err(SchemaExportError::CrateNotFound("fixture".to_owned()))
        }

        fn existing_rustdoc_json_path(&self, _: &str) -> Result<PathBuf, SchemaExportError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn test_existing_rustdoc_content_rejects_malformed_json_for_reuse() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("domain.json");
        std::fs::write(&path, "{ malformed rustdoc json }").unwrap();

        assert_eq!(existing_rustdoc_content(&FixedPathProvider(path), "domain"), None);
    }
}

#[cfg(feature = "test-helpers")]
#[derive(Clone)]
pub struct RustdocLaunchObserver {
    json_paths: BTreeMap<String, PathBuf>,
    fallback_json_path: Option<PathBuf>,
    launches: std::sync::Arc<std::sync::Mutex<BTreeMap<String, usize>>>,
    before_export: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
}

#[cfg(feature = "test-helpers")]
impl std::fmt::Debug for RustdocLaunchObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RustdocLaunchObserver")
            .field("json_paths", &self.json_paths)
            .field("launches", &self.launches())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "test-helpers")]
impl RustdocLaunchObserver {
    #[must_use]
    pub fn using_json_path(json_path: PathBuf) -> Self {
        Self {
            json_paths: BTreeMap::new(),
            fallback_json_path: Some(json_path),
            launches: std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::new())),
            before_export: None,
        }
    }

    /// Creates an observer that returns the matching rustdoc JSON file for
    /// each requested crate. This lets composition tests assert layer-scoped
    /// extraction without changing production rustdoc wiring.
    #[must_use]
    pub fn using_json_paths(json_paths: BTreeMap<String, PathBuf>) -> Self {
        Self {
            json_paths,
            fallback_json_path: None,
            launches: std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::new())),
            before_export: None,
        }
    }

    /// Creates an observer that runs `before_export` immediately before the
    /// simulated rustdoc export. This permits a test to model an input that was
    /// unavailable for the reuse decision but is restored before persistence.
    #[must_use]
    pub fn using_json_path_with_before_export(
        json_path: PathBuf,
        before_export: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            json_paths: BTreeMap::new(),
            fallback_json_path: Some(json_path),
            launches: std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::new())),
            before_export: Some(before_export),
        }
    }

    #[must_use]
    pub fn launches(&self) -> usize {
        self.launches.lock().map_or(0, |launches| launches.values().sum())
    }

    /// Returns the simulated rustdoc launch count for one crate.
    #[must_use]
    pub fn launches_for(&self, crate_name: &str) -> usize {
        self.launches.lock().map_or(0, |launches| launches.get(crate_name).copied().unwrap_or(0))
    }

    fn json_path_for(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        self.json_paths
            .get(crate_name)
            .cloned()
            .or_else(|| self.fallback_json_path.clone())
            .ok_or_else(|| SchemaExportError::CrateNotFound(crate_name.to_owned()))
    }
}

#[cfg(feature = "test-helpers")]
impl RustdocJsonPathProvider for RustdocLaunchObserver {
    fn export_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        if let Some(before_export) = &self.before_export {
            before_export();
        }
        if let Ok(mut launches) = self.launches.lock() {
            *launches.entry(crate_name.to_owned()).or_default() += 1;
        }
        self.json_path_for(crate_name)
    }

    fn existing_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        self.json_path_for(crate_name)
    }
}
