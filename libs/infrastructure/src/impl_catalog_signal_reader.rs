//! Filesystem secondary adapter for reading per-layer `<layer>-type-signals.json`.
//!
//! [`FsImplCatalogSignalReader`] implements
//! [`usecase::pre_review_gate::ImplCatalogSignalReaderPort`]. It reads
//! `<items_dir>/<track_id>/<layer>-type-signals.json` and returns a
//! `domain::TypeSignalsDocument`.
//!
//! Errors are mapped to [`usecase::pre_review_gate::ImplCatalogSignalReadError::ReadFailed`]
//! with the layer id and a diagnostic message.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::TypeSignalsDocument;
use domain::tddd::LayerId;
use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::TdddLayerBindingsPort;
use usecase::pre_review_gate::{ImplCatalogSignalReadError, ImplCatalogSignalReaderPort};

use crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use crate::tddd::type_signals_codec;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::path_safety::lexical_normalize;

const MAX_TYPE_SIGNALS_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TYPE_CATALOGUE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TYPE_BASELINE_BYTES: u64 = 64 * 1024 * 1024;

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

fn normalize_optional_baseline_path(
    baseline_path: &Path,
    workspace_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let absolute_path = if baseline_path.is_absolute() {
        baseline_path.to_path_buf()
    } else {
        workspace_root.join(baseline_path)
    };
    let normalized_root = lexical_normalize(workspace_root);
    let normalized_path = lexical_normalize(&absolute_path);
    if !normalized_path.starts_with(&normalized_root) {
        return Err(format!(
            "'{}' resolves outside workspace root '{}'. Only paths under the workspace are allowed",
            baseline_path.display(),
            workspace_root.display()
        ));
    }

    match reject_symlinks_below(&normalized_path, &normalized_root) {
        Ok(true) => Ok(Some(normalized_path)),
        Ok(false) => Ok(None),
        Err(error) => Err(format!("{}: {error}", baseline_path.display())),
    }
}

fn read_catalogue_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("metadata error reading {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("symlink check failed for {}: refused symlink", path.display()));
    }
    if !metadata.file_type().is_file() {
        return Err(format!("catalogue is not a regular file: {}", path.display()));
    }
    if metadata.len() > MAX_TYPE_CATALOGUE_BYTES {
        return Err(format!(
            "catalogue file exceeds maximum size of {MAX_TYPE_CATALOGUE_BYTES} bytes: {} bytes",
            metadata.len()
        ));
    }
    let file = std::fs::File::open(path)
        .map_err(|error| format!("I/O error reading {}: {error}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| format!("metadata error reading {}: {error}", path.display()))?;
    if !opened_metadata.file_type().is_file() {
        return Err(format!("catalogue is not a regular file: {}", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_TYPE_CATALOGUE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("I/O error reading {}: {error}", path.display()))?;
    if bytes.len() > MAX_TYPE_CATALOGUE_BYTES as usize {
        return Err(format!(
            "catalogue file exceeds maximum size of {MAX_TYPE_CATALOGUE_BYTES} bytes: {} bytes",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn read_baseline_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("metadata error reading {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("symlink check failed for {}: refused symlink", path.display()));
    }
    if !metadata.file_type().is_file() {
        return Err(format!("baseline is not a regular file: {}", path.display()));
    }
    if metadata.len() > MAX_TYPE_BASELINE_BYTES {
        return Err(format!(
            "baseline file exceeds maximum size of {MAX_TYPE_BASELINE_BYTES} bytes: {} bytes",
            metadata.len()
        ));
    }
    let file = std::fs::File::open(path)
        .map_err(|error| format!("I/O error reading {}: {error}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| format!("metadata error reading {}: {error}", path.display()))?;
    if !opened_metadata.file_type().is_file() {
        return Err(format!("baseline is not a regular file: {}", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_TYPE_BASELINE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("I/O error reading {}: {error}", path.display()))?;
    if bytes.len() > MAX_TYPE_BASELINE_BYTES as usize {
        return Err(format!(
            "baseline file exceeds maximum size of {MAX_TYPE_BASELINE_BYTES} bytes: {} bytes",
            bytes.len()
        ));
    }
    Ok(bytes)
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
        if !metadata.file_type().is_file() {
            return Err(read_failed(
                layer,
                format!("type-signals path is not a regular file: {}", path.display()),
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

        let file = std::fs::File::open(&path).map_err(|e| {
            read_failed(layer, format!("I/O error reading {}: {e}", path.display()))
        })?;
        let opened_metadata = file.metadata().map_err(|e| {
            read_failed(layer, format!("metadata error reading {}: {e}", path.display()))
        })?;
        if !opened_metadata.file_type().is_file() {
            return Err(read_failed(
                layer,
                format!("type-signals path is not a regular file: {}", path.display()),
            ));
        }
        let mut bytes = Vec::new();
        file.take(MAX_TYPE_SIGNALS_BYTES.saturating_add(1)).read_to_end(&mut bytes).map_err(
            |e| read_failed(layer, format!("I/O error reading {}: {e}", path.display())),
        )?;
        if bytes.len() > MAX_TYPE_SIGNALS_BYTES as usize {
            return Err(read_failed(
                layer,
                format!(
                    "type-signals file exceeds maximum size of {MAX_TYPE_SIGNALS_BYTES} bytes: {} bytes",
                    bytes.len()
                ),
            ));
        }

        let json = std::str::from_utf8(&bytes)
            .map_err(|e| read_failed(layer, format!("UTF-8 error in {}: {e}", path.display())))?;

        let document = type_signals_codec::decode(json).map_err(|e| {
            read_failed(layer, format!("codec error reading {}: {e}", path.display()))
        })?;

        let repository = crate::git_cli::SystemGitRepo::discover_from(&items_dir).map_err(|e| {
            read_failed(
                layer,
                format!("cannot discover repository for type-signals freshness: {e}"),
            )
        })?;
        let workspace_root = repository.root().canonicalize().map_err(|e| {
            read_failed(
                layer,
                format!("cannot resolve repository root for type-signals freshness: {e}"),
            )
        })?;
        let bindings =
            FsTdddLayerBindingsAdapter::new().load(&workspace_root, Some(layer.as_ref())).map_err(
                |e| read_failed(layer, format!("cannot load layer binding for freshness: {e}")),
            )?;
        let binding = bindings.first().ok_or_else(|| {
            read_failed(layer, "layer binding for type-signals freshness was not returned")
        })?;
        let track_dir = path.parent().ok_or_else(|| {
            read_failed(layer, "type-signals path has no track directory for freshness")
        })?;
        let catalogue_path = track_dir.join(&binding.catalogue_file);
        let catalogue_bytes =
            read_catalogue_bytes(&catalogue_path).map_err(|error| read_failed(layer, error))?;
        let current_declaration_hash = type_signals_codec::declaration_hash(&catalogue_bytes);
        if *document.cache_key().declaration_hash() != current_declaration_hash {
            return Err(read_failed(
                layer,
                format!(
                    "{}: declaration_hash mismatch (recorded={}, current={}) — re-run `sotp signal calc-impl-catalog` to refresh the evaluation result",
                    path.display(),
                    document.cache_key().declaration_hash().as_digest().as_str(),
                    current_declaration_hash.as_digest().as_str()
                ),
            ));
        }

        let baseline_path = track_dir.join(&binding.baseline_file);
        let normalized_baseline = normalize_optional_baseline_path(&baseline_path, &workspace_root)
            .map_err(|error| read_failed(layer, error))?;
        if let Some(baseline_path) = normalized_baseline {
            let baseline_bytes = read_baseline_bytes(&baseline_path)
                .map_err(|error| read_failed(layer, format!("cannot read baseline: {error}")))?;
            let current_baseline_hash = type_signals_codec::baseline_hash(&baseline_bytes);
            if *document.cache_key().baseline_hash() != current_baseline_hash {
                return Err(read_failed(
                    layer,
                    format!(
                        "{}: baseline_hash mismatch (recorded={}, current={}) — re-run `sotp signal calc-impl-catalog` to refresh the evaluation result",
                        path.display(),
                        document.cache_key().baseline_hash().as_digest().as_str(),
                        current_baseline_hash.as_digest().as_str()
                    ),
                ));
            }
        }

        Ok(document)
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

    #[cfg(unix)]
    fn with_structurally_absent_nightly<T>(action: impl FnOnce() -> T) -> T {
        use std::os::unix::fs::PermissionsExt;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let fake_bin = tempfile::tempdir().unwrap();
            let fake_rustup = fake_bin.path().join("rustup");
            fs::write(
                &fake_rustup,
                "#!/bin/sh\nif [ \"$1\" = \"toolchain\" ] && [ \"$2\" = \"list\" ]; then\nprintf 'stable-x86_64-unknown-linux-gnu (default)\\n'\nexit 0\nfi\nexit 1\n",
            )
            .unwrap();
            fs::set_permissions(&fake_rustup, fs::Permissions::from_mode(0o755)).unwrap();

            let mut path_entries = vec![fake_bin.path().to_path_buf()];
            if let Some(path) = std::env::var_os("PATH") {
                path_entries.extend(std::env::split_paths(&path));
            }
            let path = std::env::join_paths(path_entries).unwrap();
            temp_env::with_var("PATH", Some(path.as_os_str()), action)
        })
    }

    #[cfg(not(unix))]
    fn with_structurally_absent_nightly<T>(action: impl FnOnce() -> T) -> T {
        action()
    }

    fn read_signals_without_local_nightly(
        reader: &FsImplCatalogSignalReader,
        track_id: &TrackId,
        layer: &LayerId,
    ) -> Result<TypeSignalsDocument, ImplCatalogSignalReadError> {
        with_structurally_absent_nightly(|| reader.read_signals(track_id, layer))
    }

    fn read_optional_signals_without_local_nightly(
        reader: &FsImplCatalogSignalReader,
        track_id: &TrackId,
        layer: &LayerId,
    ) -> Result<Option<TypeSignalsDocument>, ImplCatalogSignalReadError> {
        with_structurally_absent_nightly(|| reader.read_optional_signals(track_id, layer))
    }

    const SAMPLE_CATALOGUE_JSON: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#;

    fn signal_json(baseline_hash: &str) -> String {
        let declaration_hash =
            type_signals_codec::declaration_hash(SAMPLE_CATALOGUE_JSON.as_bytes());
        format!(
            r#"{{
  "schema_version": 4,
  "generated_at": "2026-06-27T00:00:00Z",
  "declaration_hash": "{}",
  "head_commit": "{}",
  "baseline_hash": "{baseline_hash}",
  "signals": [
    {{
      "type_name": "MyType",
      "kind_tag": "struct",
      "signal": "blue",
      "found_type": true
    }}
  ]
}}"#,
            declaration_hash.as_digest().as_str(),
            "a".repeat(40),
        )
    }

    fn write_signal_fixture(
        track_dir: &std::path::Path,
        baseline_bytes: Option<&[u8]>,
        recorded_baseline_bytes: &[u8],
    ) {
        fs::write(track_dir.join("domain-types.json"), SAMPLE_CATALOGUE_JSON).unwrap();
        if let Some(baseline_bytes) = baseline_bytes {
            fs::write(track_dir.join("domain-types-baseline.json"), baseline_bytes).unwrap();
        }
        let recorded_baseline_hash = type_signals_codec::baseline_hash(recorded_baseline_bytes)
            .as_digest()
            .as_str()
            .to_owned();
        fs::write(track_dir.join("domain-type-signals.json"), signal_json(&recorded_baseline_hash))
            .unwrap();
    }

    const SAMPLE_SIGNALS_JSON: &str = r#"{
  "schema_version": 4,
  "generated_at": "2026-06-27T00:00:00Z",
  "declaration_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
        write_signal_fixture(&track_dir, None, b"fixture-baseline");

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let doc =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap();
        assert_eq!(doc.signals().len(), 1);
        let first_signal = doc.signals().first().expect("should have one signal");
        assert_eq!(first_signal.type_name(), "MyType");
    }

    #[test]
    fn read_signals_returns_signal_read_failed_for_missing_file() {
        let dir = temp_items_dir();
        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_optional_signals_returns_none_for_missing_file() {
        let dir = temp_items_dir();
        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let doc = read_optional_signals_without_local_nightly(
            &reader,
            &track_id("my-track"),
            &layer("domain"),
        )
        .unwrap();
        assert!(doc.is_none());
    }

    #[test]
    fn test_read_optional_signals_returns_some_for_existing_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        write_signal_fixture(&track_dir, None, b"fixture-baseline");

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let doc = read_optional_signals_without_local_nightly(
            &reader,
            &track_id("my-track"),
            &layer("domain"),
        )
        .unwrap();
        let document = doc.expect("existing signal document must be returned as Some");
        assert_eq!(document.signals().len(), 1);
        assert_eq!(document.signals()[0].type_name(), "MyType");
    }

    #[test]
    fn read_signals_rejects_stale_present_baseline_before_pre_review_evaluation() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        write_signal_fixture(&track_dir, Some(b"recaptured-baseline"), b"original-baseline");

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let error =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();

        assert!(
            matches!(error, ImplCatalogSignalReadError::ReadFailed { .. }),
            "a stale local baseline must block signal reads: {error}"
        );
        assert!(
            error.to_string().contains("baseline_hash mismatch"),
            "the freshness failure must identify the stale baseline: {error}"
        );
    }

    #[test]
    fn read_signals_accepts_matching_present_baseline_before_pre_review_evaluation() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        write_signal_fixture(&track_dir, Some(b"matching-baseline"), b"matching-baseline");

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let document =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap();

        assert_eq!(document.signals().len(), 1);
        assert_eq!(document.signals()[0].type_name(), "MyType");
    }

    #[test]
    fn read_signals_preserves_declaration_path_when_baseline_and_nightly_are_absent() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        write_signal_fixture(&track_dir, None, b"missing-baseline");

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let document =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap();

        assert_eq!(document.signals()[0].type_name(), "MyType");
        assert_eq!(
            document.cache_key().declaration_hash(),
            &type_signals_codec::declaration_hash(SAMPLE_CATALOGUE_JSON.as_bytes())
        );
    }

    #[test]
    fn read_signals_returns_signal_read_failed_for_malformed_json() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("domain-type-signals.json"), b"not json").unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
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
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_signals_returns_signal_read_failed_for_oversized_baseline_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        write_signal_fixture(&track_dir, None, b"fixture-baseline");
        fs::File::create(track_dir.join("domain-types-baseline.json"))
            .unwrap()
            .set_len(MAX_TYPE_BASELINE_BYTES + 1)
            .unwrap();

        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
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
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
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
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
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
        let err = read_optional_signals_without_local_nightly(
            &reader,
            &track_id("my-track"),
            &layer("domain"),
        )
        .unwrap_err();
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
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }

    #[test]
    fn test_read_signals_returns_signal_read_failed_for_items_dir_outside_current_repo() {
        let dir = tempfile::tempdir().unwrap();
        let reader = FsImplCatalogSignalReader::new(dir.path().to_path_buf());
        let err =
            read_signals_without_local_nightly(&reader, &track_id("my-track"), &layer("domain"))
                .unwrap_err();
        assert!(
            matches!(err, ImplCatalogSignalReadError::ReadFailed { .. }),
            "expected SignalReadFailed, got: {err}"
        );
    }
}
