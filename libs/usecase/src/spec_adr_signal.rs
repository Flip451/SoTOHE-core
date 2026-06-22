//! Spec-ADR signal calculation use case.
//!
//! Defines the command DTO, output DTO, error type, secondary port,
//! application service trait, and interactor for computing and persisting
//! chain ① (spec→ADR) signal counts to `spec.json`. The infrastructure
//! adapter (`FsSpecFileWriterAdapter`) lives in `libs/infrastructure` and
//! is injected at composition time.
//!
//! Extracted from `apps/cli-composition/src/signal.rs:167-194`
//! (`signal_calc_spec_adr`) per ADR 2026-06-21-1328 D4.
//!
//! # Port design
//!
//! [`SpecFileWriterPort`] reads and writes [`domain::SpecDocument`] objects —
//! the infrastructure adapter performs JSON codec internally (decode on read,
//! encode on write). This keeps the usecase interactor free of
//! `infrastructure::spec::codec` and `serde_json` codec details, preserving
//! hexagonal purity. The `Decode`/`Encode` error variants on
//! [`SpecAdrSignalError`] are returned by the adapter when codec fails.

use std::path::PathBuf;
use std::sync::Arc;

use domain::SpecDocument;
use thiserror::Error;

// ── Error ──────────────────────────────────────────────────────────────────────

/// Error type for [`SpecFileWriterPort`] and [`SpecAdrSignalService`].
///
/// All variants carry `String` payloads so that the usecase layer remains free
/// of `std::io` and `serde_json` dependencies (hexagonal purity). The adapter
/// converts concrete error types to strings at the infrastructure boundary.
#[derive(Debug, Error)]
pub enum SpecAdrSignalError {
    /// A filesystem I/O failure occurred while reading spec.json. The payload
    /// is the underlying `io::Error` converted to `String` at the adapter boundary.
    #[error("spec-adr signal read error: {0}")]
    Read(String),

    /// The spec.json content could not be decoded. The payload is the
    /// `SpecCodecError` message converted to `String` at the adapter boundary.
    #[error("spec-adr signal decode error: {0}")]
    Decode(String),

    /// The spec document could not be re-encoded to JSON. The payload is the
    /// `SpecCodecError` message converted to `String` at the adapter boundary.
    #[error("spec-adr signal encode error: {0}")]
    Encode(String),

    /// A filesystem I/O failure occurred while writing spec.json. The payload
    /// is the underlying `io::Error` converted to `String` at the adapter boundary.
    #[error("spec-adr signal write error: {0}")]
    Write(String),
}

// ── Command ───────────────────────────────────────────────────────────────────

/// CQRS command for the spec-ADR signal calculation use case.
///
/// Carries the path to the `spec.json` file to be read, evaluated, and
/// re-written with updated signal counts.
#[derive(Debug, Clone)]
pub struct SpecAdrSignalCommand {
    /// Filesystem path of the `spec.json` to update.
    pub spec_json_path: PathBuf,
}

// ── Output DTO ────────────────────────────────────────────────────────────────

/// Output DTO for the spec-ADR signal calculation use case.
///
/// Returns the aggregate blue/yellow/red signal counts after writing the
/// updated `spec.json`. Counts are unsigned integers; they are truly opaque
/// counters with no domain constraints beyond non-negativity.
#[derive(Debug, Clone)]
pub struct SpecAdrSignalOutput {
    /// Number of blue (met) signal requirements.
    pub blue: u32,
    /// Number of yellow (partially met) signal requirements.
    pub yellow: u32,
    /// Number of red (unmet) signal requirements.
    pub red: u32,
}

// ── Secondary port ────────────────────────────────────────────────────────────

/// Secondary port for reading and atomically writing `spec.json` files.
///
/// The port operates on [`domain::SpecDocument`] values; the infrastructure
/// adapter performs JSON codec (decode on read, encode + atomic write on
/// write). This keeps the usecase interactor free of `serde_json` and
/// `infrastructure::spec::codec`.
///
/// Error variants carry `String` payloads so that the usecase layer remains
/// free of `std::io` and codec dependencies. The adapter maps concrete errors
/// to string payloads at the infrastructure boundary:
///
/// - Read I/O error → `Read(e.to_string())`
/// - JSON decode error → `Decode(e.to_string())`
/// - JSON encode error → `Encode(e.to_string())`
/// - Write I/O error → `Write(e.to_string())`
pub trait SpecFileWriterPort: Send + Sync {
    /// Read and decode the `spec.json` at `path`, returning a [`SpecDocument`].
    ///
    /// # Errors
    ///
    /// Returns [`SpecAdrSignalError::Read`] on filesystem read failure.
    /// Returns [`SpecAdrSignalError::Decode`] on JSON or domain decode failure.
    fn read_spec_json(&self, path: PathBuf) -> Result<SpecDocument, SpecAdrSignalError>;

    /// Encode `doc` to JSON and atomically write it to the `spec.json` at `path`.
    ///
    /// The written content is newline-terminated, matching the convention
    /// established by the original `signal_calc_spec_adr` implementation.
    ///
    /// # Errors
    ///
    /// Returns [`SpecAdrSignalError::Encode`] on JSON encode failure.
    /// Returns [`SpecAdrSignalError::Write`] on filesystem write failure.
    fn write_spec_json(&self, path: PathBuf, doc: &SpecDocument) -> Result<(), SpecAdrSignalError>;
}

// ── Application service trait ─────────────────────────────────────────────────

/// Application service (primary port) for the spec-ADR signal calculation and
/// persistence use case.
///
/// Runs the read→decode→evaluate→set→write cycle. Extracted from
/// `cli_composition signal_calc_spec_adr` per ADR 1328 D4.
pub trait SpecAdrSignalService: Send + Sync {
    /// Compute and persist chain ① (spec→ADR) signals to `spec.json`.
    ///
    /// # Errors
    ///
    /// - [`SpecAdrSignalError::Read`] — cannot read `spec.json`
    /// - [`SpecAdrSignalError::Decode`] — cannot decode `spec.json` content
    /// - [`SpecAdrSignalError::Encode`] — cannot encode updated `SpecDocument`
    /// - [`SpecAdrSignalError::Write`] — cannot write updated `spec.json`
    fn calc_and_persist(
        &self,
        cmd: SpecAdrSignalCommand,
    ) -> Result<SpecAdrSignalOutput, SpecAdrSignalError>;
}

// ── Interactor ────────────────────────────────────────────────────────────────

/// Interactor implementing [`SpecAdrSignalService`].
///
/// Holds the injected [`SpecFileWriterPort`] as a private field. Executes the
/// read→evaluate→set→write cycle for spec-ADR signal persistence,
/// extracted from `cli_composition` per ADR 1328 D4.
///
/// The port abstraction keeps this interactor free of filesystem I/O and
/// JSON codec details (hexagonal purity).
pub struct SpecAdrSignalInteractor {
    spec_file_writer: Arc<dyn SpecFileWriterPort>,
}

impl SpecAdrSignalInteractor {
    /// Constructs a new interactor with the given file-writer port.
    #[must_use]
    pub fn new(spec_file_writer: Arc<dyn SpecFileWriterPort>) -> Self {
        Self { spec_file_writer }
    }
}

impl SpecAdrSignalService for SpecAdrSignalInteractor {
    fn calc_and_persist(
        &self,
        cmd: SpecAdrSignalCommand,
    ) -> Result<SpecAdrSignalOutput, SpecAdrSignalError> {
        // Step 1: Read and decode spec.json via the secondary port.
        let mut doc = self.spec_file_writer.read_spec_json(cmd.spec_json_path.clone())?;

        // Step 2: Evaluate signal counts (pure domain computation).
        let counts = doc.evaluate_signals();

        // Step 3: Set the evaluated counts on the document.
        doc.set_signals(counts);

        // Step 4: Encode and write back to spec.json via the secondary port.
        self.spec_file_writer.write_spec_json(cmd.spec_json_path, &doc)?;

        // Step 5: Return the computed signal counts as the output DTO.
        Ok(SpecAdrSignalOutput { blue: counts.blue(), yellow: counts.yellow(), red: counts.red() })
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use domain::{SignalCounts, SpecDocument, SpecScope};

    use super::{
        SpecAdrSignalCommand, SpecAdrSignalError, SpecAdrSignalInteractor, SpecAdrSignalOutput,
        SpecAdrSignalService, SpecFileWriterPort,
    };

    // ── Minimal valid SpecDocument fixture ───────────────────────────────────

    /// Returns a minimal valid `SpecDocument` with no requirements.
    ///
    /// `evaluate_signals()` on this document returns `(0, 0, 0)`.
    fn minimal_doc() -> SpecDocument {
        SpecDocument::new(
            "Test spec",
            "1.0.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap()
    }

    // ── Mock port ─────────────────────────────────────────────────────────────

    struct MockPort {
        /// The document to return from `read_spec_json`.
        read_doc: SpecDocument,
        /// Record of (path, doc_blue, doc_yellow, doc_red) tuples from writes.
        writes: Mutex<Vec<(PathBuf, u32, u32, u32)>>,
        /// If Some, `read_spec_json` returns this error.
        read_error: Option<String>,
        /// If Some, `write_spec_json` returns this error.
        write_error: Option<String>,
    }

    impl MockPort {
        fn new(read_doc: SpecDocument) -> Self {
            Self { read_doc, writes: Mutex::new(Vec::new()), read_error: None, write_error: None }
        }

        fn with_read_error(error_msg: String) -> Self {
            Self {
                read_doc: minimal_doc(),
                writes: Mutex::new(Vec::new()),
                read_error: Some(error_msg),
                write_error: None,
            }
        }

        fn with_write_error(read_doc: SpecDocument, error_msg: String) -> Self {
            Self {
                read_doc,
                writes: Mutex::new(Vec::new()),
                read_error: None,
                write_error: Some(error_msg),
            }
        }

        fn recorded_write_paths(&self) -> Vec<PathBuf> {
            self.writes.lock().unwrap().iter().map(|(p, _, _, _)| p.clone()).collect()
        }

        fn write_count(&self) -> usize {
            self.writes.lock().unwrap().len()
        }
    }

    impl SpecFileWriterPort for MockPort {
        fn read_spec_json(&self, _path: PathBuf) -> Result<SpecDocument, SpecAdrSignalError> {
            if let Some(ref msg) = self.read_error {
                return Err(SpecAdrSignalError::Read(msg.clone()));
            }
            Ok(self.read_doc.clone())
        }

        fn write_spec_json(
            &self,
            path: PathBuf,
            doc: &SpecDocument,
        ) -> Result<(), SpecAdrSignalError> {
            if let Some(ref msg) = self.write_error {
                return Err(SpecAdrSignalError::Write(msg.clone()));
            }
            let signals = doc.signals().copied().unwrap_or(SignalCounts::new(0, 0, 0));
            self.writes.lock().unwrap().push((
                path,
                signals.blue(),
                signals.yellow(),
                signals.red(),
            ));
            Ok(())
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn interactor_calls_read_and_write_with_spec_json_path() {
        let mock = Arc::new(MockPort::new(minimal_doc()));
        let interactor =
            SpecAdrSignalInteractor::new(Arc::clone(&mock) as Arc<dyn SpecFileWriterPort>);

        let path = PathBuf::from("track/items/test/spec.json");
        let cmd = SpecAdrSignalCommand { spec_json_path: path.clone() };
        interactor.calc_and_persist(cmd).expect("calc_and_persist must succeed");

        let paths = mock.recorded_write_paths();
        assert_eq!(paths.len(), 1, "exactly one write must be recorded");
        assert_eq!(paths[0], path, "write path must match command spec_json_path");
    }

    #[test]
    fn interactor_returns_output_with_signal_counts() {
        let mock = Arc::new(MockPort::new(minimal_doc()));
        let interactor =
            SpecAdrSignalInteractor::new(Arc::clone(&mock) as Arc<dyn SpecFileWriterPort>);

        let cmd =
            SpecAdrSignalCommand { spec_json_path: PathBuf::from("track/items/test/spec.json") };
        let output = interactor.calc_and_persist(cmd).expect("calc_and_persist must succeed");

        // Output fields must be accessible and are non-negative u32 values.
        let SpecAdrSignalOutput { blue, yellow, red } = output;
        // For minimal_doc (no requirements), evaluate_signals → (0, 0, 0).
        assert_eq!(blue, 0);
        assert_eq!(yellow, 0);
        assert_eq!(red, 0);
    }

    #[test]
    fn interactor_sets_signals_on_document_before_write() {
        // A doc with stale signals (0,0,0) but no requirements → evaluate → still (0,0,0).
        // To verify set_signals is called, use a doc with pre-set signals and check
        // the mock records the post-evaluate values.
        let mut doc = minimal_doc();
        // Pre-set signals to non-zero to detect if they get overwritten.
        doc.set_signals(SignalCounts::new(99, 88, 77));

        let mock = Arc::new(MockPort::new(doc));
        let interactor =
            SpecAdrSignalInteractor::new(Arc::clone(&mock) as Arc<dyn SpecFileWriterPort>);

        let cmd =
            SpecAdrSignalCommand { spec_json_path: PathBuf::from("track/items/test/spec.json") };
        interactor.calc_and_persist(cmd).expect("calc_and_persist must succeed");

        let writes = mock.writes.lock().unwrap();
        // After evaluate_signals() on empty doc, all counts become 0.
        assert_eq!(writes[0].1, 0, "blue must be reset to evaluated value");
        assert_eq!(writes[0].2, 0, "yellow must be reset to evaluated value");
        assert_eq!(writes[0].3, 0, "red must be reset to evaluated value");
    }

    #[test]
    fn read_error_propagates_as_read_variant() {
        let mock = Arc::new(MockPort::with_read_error("permission denied".to_string()));
        let interactor =
            SpecAdrSignalInteractor::new(Arc::clone(&mock) as Arc<dyn SpecFileWriterPort>);

        let cmd =
            SpecAdrSignalCommand { spec_json_path: PathBuf::from("track/items/test/spec.json") };
        let result = interactor.calc_and_persist(cmd);

        assert!(result.is_err(), "read error must propagate");
        assert!(
            matches!(result, Err(SpecAdrSignalError::Read(_))),
            "error must be the Read variant"
        );
    }

    #[test]
    fn write_error_propagates_as_write_variant() {
        let mock = Arc::new(MockPort::with_write_error(minimal_doc(), "disk full".to_string()));
        let interactor =
            SpecAdrSignalInteractor::new(Arc::clone(&mock) as Arc<dyn SpecFileWriterPort>);

        let cmd =
            SpecAdrSignalCommand { spec_json_path: PathBuf::from("track/items/test/spec.json") };
        let result = interactor.calc_and_persist(cmd);

        assert!(result.is_err(), "write error must propagate");
        assert!(
            matches!(result, Err(SpecAdrSignalError::Write(_))),
            "error must be the Write variant"
        );
    }

    #[test]
    fn no_write_when_read_fails() {
        let mock = Arc::new(MockPort::with_read_error("no such file".to_string()));
        let interactor =
            SpecAdrSignalInteractor::new(Arc::clone(&mock) as Arc<dyn SpecFileWriterPort>);

        let cmd =
            SpecAdrSignalCommand { spec_json_path: PathBuf::from("track/items/test/spec.json") };
        let _ = interactor.calc_and_persist(cmd);

        assert_eq!(mock.write_count(), 0, "no write must be recorded when read fails");
    }
}
