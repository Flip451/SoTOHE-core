//! Cache-freshness helpers for the type-signal evaluator.

use std::path::PathBuf;

#[cfg(feature = "test-helpers")]
use std::collections::BTreeMap;

use domain::schema::SchemaExportError;
use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::CrateName;
use domain::tddd::type_signals_doc::{
    TypeSignalsCacheKey, TypeSignalsDocument, TypeSignalsReuseDecision, decide_type_signals_reuse,
};

use crate::schema_export::RustdocSchemaExporter;

pub(super) trait RustdocJsonPathProvider {
    fn export_rustdoc_json_path(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<PathBuf, SchemaExportError>;
}

pub(super) fn reuse_decision_for_recorded_document(
    recorded: Option<&TypeSignalsDocument>,
    current_key: &TypeSignalsCacheKey,
) -> TypeSignalsReuseDecision {
    recorded.map_or(TypeSignalsReuseDecision::ReextractAndEvaluate, |document| {
        decide_type_signals_reuse(document.cache_key(), current_key)
    })
}

impl RustdocJsonPathProvider for RustdocSchemaExporter {
    fn export_rustdoc_json_path(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<PathBuf, SchemaExportError> {
        Self::export_rustdoc_json_path_with_features(self, crate_name, features)
    }
}

#[cfg(feature = "test-helpers")]
#[derive(Clone)]
pub struct RustdocLaunchObserver {
    json_paths: BTreeMap<String, PathBuf>,
    fallback_json_path: Option<PathBuf>,
    launches: std::sync::Arc<std::sync::Mutex<BTreeMap<String, usize>>>,
    feature_selections: std::sync::Arc<std::sync::Mutex<BTreeMap<String, Vec<Vec<String>>>>>,
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
            feature_selections: std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::new())),
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
            feature_selections: std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::new())),
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
            feature_selections: std::sync::Arc::new(std::sync::Mutex::new(BTreeMap::new())),
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

    /// Returns every feature selection supplied to simulated rustdoc for one crate.
    #[must_use]
    pub fn feature_selections_for(&self, crate_name: &str) -> Vec<Vec<String>> {
        self.feature_selections.lock().map_or_else(
            |_| Vec::new(),
            |selections| selections.get(crate_name).cloned().unwrap_or_default(),
        )
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
    fn export_rustdoc_json_path(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<PathBuf, SchemaExportError> {
        if let Some(before_export) = &self.before_export {
            before_export();
        }
        if let Ok(mut launches) = self.launches.lock() {
            *launches.entry(crate_name.as_str().to_owned()).or_default() += 1;
        }
        if let Ok(mut selections) = self.feature_selections.lock() {
            selections
                .entry(crate_name.as_str().to_owned())
                .or_default()
                .push(features.iter().map(|feature| feature.as_str().to_owned()).collect());
        }
        self.json_path_for(crate_name.as_str())
    }
}
