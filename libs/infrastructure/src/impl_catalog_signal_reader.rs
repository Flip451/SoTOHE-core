//! Filesystem secondary adapter for reading per-layer `<layer>-type-signals.json`.
//!
//! [`FsImplCatalogSignalReader`] implements
//! [`usecase::pre_review_gate::ImplCatalogSignalReaderPort`]. It reads
//! `<items_dir>/<track_id>/<layer>-type-signals.json` and returns a
//! `domain::TypeSignalsDocument`.
//!
//! Errors are mapped to [`usecase::pre_review_gate::ImplCatalogSignalReadError::ReadFailed`]
//! with the layer id and a diagnostic message.

use std::path::PathBuf;

use domain::TrackId;
use domain::TypeSignalsDocument;
use domain::tddd::LayerId;
use domain::tddd::catalogue_linter::FreeText;
use usecase::pre_review_gate::{ImplCatalogSignalReadError, ImplCatalogSignalReaderPort};

use crate::tddd::type_signals_codec;
use crate::track::symlink_guard::reject_symlinks_below;

const MAX_TYPE_SIGNALS_BYTES: u64 = 16 * 1024 * 1024;

fn read_failed(layer: &LayerId, message: impl Into<String>) -> ImplCatalogSignalReadError {
    ImplCatalogSignalReadError::ReadFailed {
        layer: layer.clone(),
        message: FreeText::new(message.into()),
    }
}

/// Filesystem secondary adapter implementing
/// [`usecase::pre_review_gate::ImplCatalogSignalReaderPort`].
///
/// Reads `<items_dir>/<track_id>/<layer>-type-signals.json`, decodes it as a
/// `domain::TypeSignalsDocument`, and returns it.
///
/// - I/O and decode errors map to [`ImplCatalogSignalReadError::ReadFailed`] with
///   the layer id and a diagnostic message.
///
/// The `items_dir` is injected at construction time.
#[derive(Debug)]
pub struct FsImplCatalogSignalReader {
    items_dir: PathBuf,
}

impl FsImplCatalogSignalReader {
    /// Construct a `FsImplCatalogSignalReader` with the given items directory root.
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }
}

impl ImplCatalogSignalReaderPort for FsImplCatalogSignalReader {
    fn read_optional_signals(
        &self,
        track_id: &TrackId,
        layer: &LayerId,
    ) -> Result<Option<TypeSignalsDocument>, ImplCatalogSignalReadError> {
        let filename = format!("{}-type-signals.json", layer.as_ref());
        let items_dir =
            crate::resolve_items_dir_under_current_repo(&self.items_dir).map_err(|e| {
                read_failed(layer, format!("items_dir rejected before reading type-signals: {e}"))
            })?;
        let path = items_dir.join(track_id.as_ref()).join(&filename);
        match reject_symlinks_below(&path, &items_dir) {
            Ok(true) => self.read_signals(track_id, layer).map(Some),
            Ok(false) => Ok(None),
            Err(e) => {
                Err(read_failed(layer, format!("symlink check failed for {}: {e}", path.display())))
            }
        }
    }

    fn read_signals(
        &self,
        track_id: &TrackId,
        layer: &LayerId,
    ) -> Result<TypeSignalsDocument, ImplCatalogSignalReadError> {
        let filename = format!("{}-type-signals.json", layer.as_ref());
        let items_dir =
            crate::resolve_items_dir_under_current_repo(&self.items_dir).map_err(|e| {
                read_failed(layer, format!("items_dir rejected before reading type-signals: {e}"))
            })?;
        let path = items_dir.join(track_id.as_ref()).join(&filename);

        match reject_symlinks_below(&path, &items_dir) {
            Ok(true) => {}
            Ok(false) => {
                return Err(read_failed(
                    layer,
                    format!("signal file not found: {}", path.display()),
                ));
            }
            Err(e) => {
                return Err(read_failed(
                    layer,
                    format!("symlink check failed for {}: {e}", path.display()),
                ));
            }
        }

        let metadata = std::fs::symlink_metadata(&path).map_err(|e| {
            read_failed(layer, format!("metadata error reading {}: {e}", path.display()))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(read_failed(
                layer,
                format!("symlink check failed for {}: refused symlink", path.display()),
            ));
        }
        if metadata.len() > MAX_TYPE_SIGNALS_BYTES {
            return Err(read_failed(
                layer,
                format!(
                    "type-signals file exceeds maximum size of {MAX_TYPE_SIGNALS_BYTES} bytes: {} bytes",
                    metadata.len()
                ),
            ));
        }

        let bytes = std::fs::read(&path).map_err(|e| {
            read_failed(layer, format!("I/O error reading {}: {e}", path.display()))
        })?;

        let json = std::str::from_utf8(&bytes)
            .map_err(|e| read_failed(layer, format!("UTF-8 error in {}: {e}", path.display())))?;

        type_signals_codec::decode(json)
            .map_err(|e| read_failed(layer, format!("codec error reading {}: {e}", path.display())))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use domain::TrackId;
    use domain::tddd::LayerId;
    use usecase::pre_review_gate::ImplCatalogSignalReadError;

    use super::*;

    #[test]
    fn fs_impl_catalog_signal_reader_implements_port_without_compatibility_delegation() {
        let source = include_str!("impl_catalog_signal_reader.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();

        assert!(
            production_source
                .contains("impl ImplCatalogSignalReaderPort for FsImplCatalogSignalReader"),
            "reader must implement its declared secondary port"
        );
        for required_method in ["fn read_optional_signals(", "fn read_signals("] {
            assert!(production_source.contains(required_method));
        }
        for forbidden_runtime_path in
            ["ServiceImpl", "CompositionRoot", "PreReviewGateInteractor", "TaskContractDriver"]
        {
            assert!(
                !production_source.contains(forbidden_runtime_path),
                "filesystem adapter must not reverse-delegate through {forbidden_runtime_path}"
            );
        }
    }

    fn layer(s: &str) -> LayerId {
        LayerId::try_new(s.to_owned()).unwrap()
    }

    fn track_id(s: &str) -> TrackId {
        TrackId::try_new(s).unwrap()
    }

    fn temp_items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("impl-catalog-signal-reader-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    const SAMPLE_SIGNALS_JSON: &str = r#"{
  "schema_version": 4,
  "generated_at": "2026-06-27T00:00:00Z",
  "declaration_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "implementation_input_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "baseline_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "signals": [
    {
      "type_name": "MyType",
      "kind_tag": "struct",
      "signal": "blue",
      "found_type": true
    }
  ]
}"#;

    #[test]
    fn read_signals_returns_document_for_existing_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("domain-type-signals.json"), SAMPLE_SIGNALS_JSON).unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let doc = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap();
        assert_eq!(doc.signals().len(), 1);
        let first_signal = doc.signals().first().expect("should have one signal");
        assert_eq!(first_signal.type_name(), "MyType");
    }

    #[test]
    fn read_signals_returns_signal_read_failed_for_missing_file() {
        let dir = temp_items_dir();
        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_optional_signals_returns_none_for_missing_file() {
        let dir = temp_items_dir();
        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let doc = reader.read_optional_signals(&track_id("my-track"), &layer("domain")).unwrap();
        assert!(doc.is_none());
    }

    #[test]
    fn test_read_optional_signals_returns_some_for_existing_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("domain-type-signals.json"), SAMPLE_SIGNALS_JSON).unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let doc = reader.read_optional_signals(&track_id("my-track"), &layer("domain")).unwrap();
        let document = doc.expect("existing signal document must be returned as Some");
        assert_eq!(document.signals().len(), 1);
        assert_eq!(document.signals()[0].type_name(), "MyType");
    }

    #[test]
    fn read_signals_returns_signal_read_failed_for_malformed_json() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("domain-type-signals.json"), b"not json").unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_signals_returns_signal_read_failed_for_oversized_signal_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        let file = fs::File::create(track_dir.join("domain-type-signals.json")).unwrap();
        file.set_len(MAX_TYPE_SIGNALS_BYTES + 1).unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_signals_returns_signal_read_failed_for_symlinked_signal_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        let real = track_dir.join("real-domain-type-signals.json");
        fs::write(&real, SAMPLE_SIGNALS_JSON).unwrap();
        std::os::unix::fs::symlink(&real, track_dir.join("domain-type-signals.json")).unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_signals_returns_signal_read_failed_for_symlinked_track_dir() {
        let dir = temp_items_dir();
        let real_track_dir = dir.path().join("real-track");
        fs::create_dir_all(&real_track_dir).unwrap();
        fs::write(real_track_dir.join("domain-type-signals.json"), SAMPLE_SIGNALS_JSON).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, dir.path().join("my-track")).unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_read_optional_signals_returns_signal_read_failed_for_symlinked_track_dir() {
        let dir = temp_items_dir();
        let real_track_dir = dir.path().join("real-track");
        fs::create_dir_all(&real_track_dir).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, dir.path().join("my-track")).unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err =
            reader.read_optional_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_signals_returns_signal_read_failed_for_symlinked_items_dir() {
        let dir = temp_items_dir();
        let real_items_dir = dir.path().join("real-items");
        let track_dir = real_items_dir.join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("domain-type-signals.json"), SAMPLE_SIGNALS_JSON).unwrap();
        let link_items_dir = dir.path().join("items");
        std::os::unix::fs::symlink(&real_items_dir, &link_items_dir).unwrap();

        let reader = FsImplCatalogSignalReader::new(link_items_dir);
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_signals_returns_signal_read_failed_for_items_dir_outside_current_repo() {
        let dir = tempfile::tempdir().unwrap();
        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err = reader.read_signals(&track_id("my-track"), &layer("domain")).unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }
}
