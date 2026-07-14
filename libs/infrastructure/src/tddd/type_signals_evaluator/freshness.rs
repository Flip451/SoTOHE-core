//! Snapshot verification and reuse-decision helpers for the type-signal evaluator.

use super::*;

use domain::tddd::type_signals_doc::{LiveRustdocSnapshotStatus, decide_type_signals_reuse};

pub(super) fn snapshot_status_and_content(
    exporter: &impl RustdocJsonPathProvider,
    target_crate: &str,
) -> (LiveRustdocSnapshotStatus, Option<String>) {
    let path = match exporter.existing_rustdoc_json_path(target_crate) {
        Ok(path) => path,
        Err(_) => return (LiveRustdocSnapshotStatus::ReadFailed, None),
    };
    let content = match read_utf8_file_limited(&path, MAX_RUSTDOC_SNAPSHOT_BYTES) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (LiveRustdocSnapshotStatus::Missing, None);
        }
        Err(_) => return (LiveRustdocSnapshotStatus::ReadFailed, None),
    };
    if BaselineRustdocCodec::from_json(&content).is_err() {
        return (LiveRustdocSnapshotStatus::ParseFailed, None);
    }
    match digest_identity(content.as_bytes(), LiveRustdocSnapshotHash::new) {
        Ok(hash) => (LiveRustdocSnapshotStatus::Verified(hash), Some(content)),
        Err(_) => (LiveRustdocSnapshotStatus::HashMismatch, None),
    }
}

/// Internal boundary for rustdoc JSON lookup and launch. Keeping it private
/// makes launch behavior observable in tests without expanding the
/// catalogue-governed public API.
pub(super) trait RustdocJsonPathProvider {
    fn export_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError>;

    fn existing_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError>;
}

/// Feature-gated rustdoc launch observer for composition-level tests.
///
/// The observer supplies a pre-built rustdoc JSON snapshot, so tests exercise
/// the real type-signals adapter without invoking a toolchain subprocess.
/// It is excluded from normal builds and rustdoc output.
#[cfg(feature = "test-helpers")]
#[derive(Clone, Debug)]
pub struct RustdocLaunchObserver {
    snapshot_path: PathBuf,
    launches: Arc<AtomicUsize>,
}

#[cfg(feature = "test-helpers")]
impl RustdocLaunchObserver {
    /// Create an observer that serves `snapshot_path` for lookup and export.
    #[must_use]
    pub fn using_snapshot(snapshot_path: PathBuf) -> Self {
        Self { snapshot_path, launches: Arc::new(AtomicUsize::new(0)) }
    }

    /// Number of rustdoc exports requested through this observer.
    #[must_use]
    pub fn launches(&self) -> usize {
        self.launches.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "test-helpers")]
impl RustdocJsonPathProvider for RustdocLaunchObserver {
    fn export_rustdoc_json_path(&self, _crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        Ok(self.snapshot_path.clone())
    }

    fn existing_rustdoc_json_path(&self, _crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        Ok(self.snapshot_path.clone())
    }
}

impl RustdocJsonPathProvider for RustdocSchemaExporter {
    fn export_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        Self::export_rustdoc_json_path(self, crate_name)
    }

    fn existing_rustdoc_json_path(&self, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
        Self::existing_rustdoc_json_path(self, crate_name)
    }
}

pub(super) fn reuse_decision_for_recorded_document(
    recorded: Option<&TypeSignalsDocument>,
    current: &TypeSignalsCurrentInputs,
    snapshot_status: LiveRustdocSnapshotStatus,
) -> TypeSignalsReuseDecision {
    recorded.map_or(TypeSignalsReuseDecision::ReextractAndEvaluate, |document| {
        decide_type_signals_reuse(document.freshness(), current, snapshot_status)
    })
}
