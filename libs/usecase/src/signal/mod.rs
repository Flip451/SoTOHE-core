//! Argless signal orchestration (CN-16 / CN-17 / D8).
//!
//! Exposes the [`SignalLayerReader`] secondary port and four argless usecase
//! orchestrator functions that replace the per-layer `--signals-path` /
//! `--catalog-hash` CLI arguments with a clean port abstraction.
//!
//! All four functions follow the same pattern:
//! 1. Resolve the active-track ID via `reader.active_track_id()`.
//! 2. Enumerate TDDD-enabled layers via `reader.enabled_layers(track_id)`.
//! 3. For each layer, fetch raw catalogue bytes via
//!    `reader.catalogue_bytes(track_id, layer)`.
//! 4. If bytes are present, compute SHA-256 hex and invoke `per_layer_fn`.
//! 5. If bytes are absent, skip the layer (no error).
//!
//! The returned [`domain::verify::VerifyOutcome`] merges all per-layer results.
//! An error from the reader (I/O or unresolved track ID) is surfaced as an
//! error-level finding in the outcome — the function never returns `Err`.
//!
//! The `per_layer_fn` closure captures all infrastructure concerns (signals-file
//! path construction, strictness for check commands) so that these orchestrators
//! remain pure coordination logic with no path or config coupling.

pub mod port;

pub use port::{SignalLayerReader, SignalLayerReaderError};

use domain::tddd::LayerId;
use domain::verify::{VerifyFinding, VerifyOutcome};

fn merge_layer_outcomes(outcomes: Vec<VerifyOutcome>) -> VerifyOutcome {
    let mut result = VerifyOutcome::pass();
    for outcome in outcomes {
        result.merge(outcome);
    }
    result
}

fn run_per_layer<R, F>(reader: &R, per_layer_fn: F) -> VerifyOutcome
where
    R: SignalLayerReader,
    F: Fn(LayerId, &str) -> VerifyOutcome,
{
    let track_id = match reader.active_track_id() {
        Ok(id) => id,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "signal orchestrator: cannot resolve active track ID: {e}"
            ))]);
        }
    };

    let layers = match reader.enabled_layers(track_id.clone()) {
        Ok(ls) => ls,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "signal orchestrator: cannot enumerate enabled layers for track '{}': {e}",
                track_id
            ))]);
        }
    };

    let mut outcomes: Vec<VerifyOutcome> = Vec::new();
    for layer in layers {
        let bytes = match reader.catalogue_bytes(track_id.clone(), layer.clone()) {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(e) => {
                outcomes.push(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "signal orchestrator: cannot read catalogue bytes for layer '{}': {e}",
                    layer
                ))]));
                continue;
            }
        };

        let hash_hex = {
            use sha2::Digest as _;
            let digest = sha2::Sha256::digest(&bytes);
            digest.iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        outcomes.push(per_layer_fn(layer, &hash_hex));
    }

    merge_layer_outcomes(outcomes)
}

/// Argless orchestrator for `signal calc-impl-catalog` (chain ③).
pub fn calc_impl_catalog<R, F>(reader: &R, per_layer_fn: F) -> VerifyOutcome
where
    R: SignalLayerReader,
    F: Fn(LayerId, &str) -> VerifyOutcome,
{
    run_per_layer(reader, per_layer_fn)
}

/// Argless orchestrator for `signal calc-catalog-spec` (chain ②).
pub fn calc_catalog_spec<R, F>(reader: &R, per_layer_fn: F) -> VerifyOutcome
where
    R: SignalLayerReader,
    F: Fn(LayerId, &str) -> VerifyOutcome,
{
    run_per_layer(reader, per_layer_fn)
}

/// Argless orchestrator for `signal check-impl-catalog` (chain ③).
///
/// Strictness is NOT a parameter — the `per_layer_fn` closure captures
/// `strict: bool` from the `SignalGateMatrix` at the call site (D8-5).
pub fn check_impl_catalog<R, F>(reader: &R, per_layer_fn: F) -> VerifyOutcome
where
    R: SignalLayerReader,
    F: Fn(LayerId, &str) -> VerifyOutcome,
{
    run_per_layer(reader, per_layer_fn)
}

/// Argless orchestrator for `signal check-catalog-spec` (chain ②).
///
/// Strictness is NOT a parameter — the `per_layer_fn` closure captures
/// `strict: bool` from the `SignalGateMatrix` at the call site (D8-5).
pub fn check_catalog_spec<R, F>(reader: &R, per_layer_fn: F) -> VerifyOutcome
where
    R: SignalLayerReader,
    F: Fn(LayerId, &str) -> VerifyOutcome,
{
    run_per_layer(reader, per_layer_fn)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::cell::RefCell;

    use domain::TrackId;
    use domain::tddd::LayerId;
    use domain::verify::VerifyOutcome;

    use super::port::{SignalLayerReader, SignalLayerReaderError};
    use super::*;

    struct MockReader {
        track_id: Result<String, SignalLayerReaderError>,
        layers_error: bool,
        layers: Vec<String>,
        bytes_errors: Vec<String>,
        bytes_map: Vec<(String, Option<Vec<u8>>)>,
    }

    impl MockReader {
        fn new(track_id: &str, layers: &[&str]) -> Self {
            Self {
                track_id: Ok(track_id.to_owned()),
                layers_error: false,
                layers: layers.iter().map(|s| (*s).to_owned()).collect(),
                bytes_errors: Vec::new(),
                bytes_map: Vec::new(),
            }
        }

        fn with_bytes(mut self, layer: &str, bytes: &[u8]) -> Self {
            self.bytes_map.push((layer.to_owned(), Some(bytes.to_vec())));
            self
        }

        fn with_absent(mut self, layer: &str) -> Self {
            self.bytes_map.push((layer.to_owned(), None));
            self
        }

        fn with_layers_error(mut self) -> Self {
            self.layers_error = true;
            self
        }

        fn with_bytes_error(mut self, layer: &str) -> Self {
            self.bytes_errors.push(layer.to_owned());
            self
        }

        fn with_track_id_error(mut self) -> Self {
            self.track_id = Err(SignalLayerReaderError::TrackIdUnresolved);
            self
        }
    }

    impl SignalLayerReader for MockReader {
        fn active_track_id(&self) -> Result<TrackId, SignalLayerReaderError> {
            match &self.track_id {
                Ok(s) => Ok(TrackId::try_new(s.clone()).expect("invalid mock track id")),
                Err(SignalLayerReaderError::TrackIdUnresolved) => {
                    Err(SignalLayerReaderError::TrackIdUnresolved)
                }
                Err(SignalLayerReaderError::Io) => Err(SignalLayerReaderError::Io),
            }
        }

        fn enabled_layers(
            &self,
            _track_id: TrackId,
        ) -> Result<Vec<LayerId>, SignalLayerReaderError> {
            if self.layers_error {
                return Err(SignalLayerReaderError::Io);
            }
            Ok(self
                .layers
                .iter()
                .map(|s| LayerId::try_new(s.clone()).expect("invalid mock layer id"))
                .collect())
        }

        fn catalogue_bytes(
            &self,
            _track_id: TrackId,
            layer: LayerId,
        ) -> Result<Option<Vec<u8>>, SignalLayerReaderError> {
            let key = layer.as_ref();
            if self.bytes_errors.iter().any(|error_layer| error_layer == key) {
                return Err(SignalLayerReaderError::Io);
            }
            for (k, v) in &self.bytes_map {
                if k == key {
                    return Ok(v.clone());
                }
            }
            Ok(None)
        }
    }

    fn spy_fn(
        calls: &RefCell<Vec<(String, String)>>,
    ) -> impl Fn(LayerId, &str) -> VerifyOutcome + '_ {
        move |layer, hash| {
            calls.borrow_mut().push((layer.to_string(), hash.to_owned()));
            VerifyOutcome::pass()
        }
    }

    #[test]
    fn test_calc_impl_catalog_calls_per_layer_fn_with_sha256() {
        let bytes = b"hello world";
        let expected_hash = {
            use sha2::Digest as _;
            let digest = sha2::Sha256::digest(bytes);
            digest.iter().map(|b| format!("{b:02x}")).collect::<String>()
        };

        let reader =
            MockReader::new("my-track-2026-01-01", &["domain"]).with_bytes("domain", bytes);
        let calls = RefCell::new(Vec::new());
        let outcome = calc_impl_catalog(&reader, spy_fn(&calls));

        assert!(outcome.is_ok(), "expected pass, got {outcome:?}");
        let captured = calls.borrow();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].0, "domain");
        assert_eq!(captured[0].1, expected_hash);
    }

    #[test]
    fn test_calc_impl_catalog_skips_absent_layers() {
        let reader = MockReader::new("my-track-2026-01-01", &["domain", "usecase"])
            .with_bytes("domain", b"data")
            .with_absent("usecase");
        let calls = RefCell::new(Vec::new());
        calc_impl_catalog(&reader, spy_fn(&calls));

        let captured = calls.borrow();
        assert_eq!(captured.len(), 1, "absent layer must not invoke per_layer_fn");
        assert_eq!(captured[0].0, "domain");
    }

    #[test]
    fn test_calc_impl_catalog_returns_error_when_track_id_unresolved() {
        let reader = MockReader::new("t", &[]).with_track_id_error();
        let calls = RefCell::new(Vec::new());
        let outcome = calc_impl_catalog(&reader, spy_fn(&calls));

        assert!(!outcome.is_ok(), "expected failure outcome");
        assert!(calls.borrow().is_empty(), "per_layer_fn must not be called on reader error");
    }

    #[test]
    fn test_calc_impl_catalog_enabled_layers_error_returns_failure() {
        let reader = MockReader::new("my-track-2026-01-01", &[]).with_layers_error();
        let calls = RefCell::new(Vec::new());
        let outcome = calc_impl_catalog(&reader, spy_fn(&calls));

        assert!(!outcome.is_ok(), "expected enabled_layers failure outcome");
        assert!(calls.borrow().is_empty(), "per_layer_fn must not be called on reader error");
        assert!(
            outcome
                .findings()
                .iter()
                .any(|finding| finding.message().contains("cannot enumerate enabled layers")),
            "expected enabled_layers error finding, got {outcome:?}"
        );
    }

    #[test]
    fn test_calc_impl_catalog_catalogue_bytes_error_continues_other_layers() {
        let reader =
            MockReader::new("my-track-2026-01-01", &["domain", "usecase", "infrastructure"])
                .with_bytes("domain", b"d")
                .with_bytes_error("usecase")
                .with_bytes("infrastructure", b"i");
        let calls = RefCell::new(Vec::new());
        let outcome = calc_impl_catalog(&reader, spy_fn(&calls));

        assert!(!outcome.is_ok(), "expected catalogue_bytes failure outcome");
        let captured = calls.borrow();
        assert_eq!(captured.len(), 2, "read error must not stop later layers");
        assert_eq!(captured[0].0, "domain");
        assert_eq!(captured[1].0, "infrastructure");
        assert!(
            outcome.findings().iter().any(|finding| {
                finding.message().contains("cannot read catalogue bytes for layer 'usecase'")
            }),
            "expected catalogue_bytes error finding, got {outcome:?}"
        );
    }

    #[test]
    fn test_calc_impl_catalog_multi_layer_merges_outcomes() {
        let reader = MockReader::new("my-track-2026-01-01", &["domain", "usecase"])
            .with_bytes("domain", b"d")
            .with_bytes("usecase", b"u");
        let calls = RefCell::new(Vec::new());
        let outcome = calc_impl_catalog(&reader, spy_fn(&calls));

        assert!(outcome.is_ok());
        assert_eq!(calls.borrow().len(), 2);
    }

    #[test]
    fn test_calc_catalog_spec_calls_per_layer_fn() {
        let reader =
            MockReader::new("my-track-2026-01-01", &["domain"]).with_bytes("domain", b"spec data");
        let calls = RefCell::new(Vec::new());
        let outcome = calc_catalog_spec(&reader, spy_fn(&calls));

        assert!(outcome.is_ok());
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn test_check_impl_catalog_calls_per_layer_fn() {
        let reader =
            MockReader::new("my-track-2026-01-01", &["domain"]).with_bytes("domain", b"check data");
        let calls = RefCell::new(Vec::new());
        let outcome = check_impl_catalog(&reader, spy_fn(&calls));

        assert!(outcome.is_ok());
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn test_check_impl_catalog_captures_strict_in_closure() {
        let reader =
            MockReader::new("my-track-2026-01-01", &["domain"]).with_bytes("domain", b"data");
        let outcome = check_impl_catalog(&reader, |_layer, _hash| {
            VerifyOutcome::from_findings(vec![VerifyFinding::error("strict violation")])
        });

        assert!(!outcome.is_ok(), "failing closure must produce a failing outcome");
    }

    #[test]
    fn test_check_catalog_spec_calls_per_layer_fn() {
        let reader = MockReader::new("my-track-2026-01-01", &["domain"])
            .with_bytes("domain", b"catalog bytes");
        let calls = RefCell::new(Vec::new());
        let outcome = check_catalog_spec(&reader, spy_fn(&calls));

        assert!(outcome.is_ok());
        assert_eq!(calls.borrow().len(), 1);
    }
}
