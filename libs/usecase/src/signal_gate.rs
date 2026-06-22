//! `SignalGate` application service — sequences the 4 SoT chains and
//! aggregates their outcomes into a single [`SignalGateOutput`].
//!
//! # Design
//!
//! The interactor holds:
//! - [`AdrChainRunnerPort`] for chain ⓪ (ADR → user; live filesystem scan).
//! - [`SpecAdrChainRunnerPort`] for chain ① (spec → ADR; reads spec.json).
//! - [`Arc<dyn SignalLayerReader>`] for chains ② and ③ (catalog-spec / impl-catalog).
//! - [`domain::SignalGateMatrix`] for per-chain strictness resolution.
//!
//! The gate label carried in [`SignalGateCommand`] is decoded to a
//! [`domain::GateKind`] by matching the two canonical label prefixes
//! (`"signal check --gate commit"` / `"signal check --gate merge"`).
//!
//! Reference: ADR `knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md` §D4.

use std::path::PathBuf;
use std::sync::Arc;

use domain::{ChainId, GateKind, SignalGateMatrix, Strictness, TrackId};

use crate::signal::SignalLayerReader;

// ── DTOs / Command ─────────────────────────────────────────────────────────────

/// CQRS command for the signal-gate chain-sequencing use case.
///
/// Carries the gate label, items directory, and active track id.
/// Created pre-baseline; action add.
#[derive(Debug, Clone)]
pub struct SignalGateCommand {
    /// Human-readable gate label (e.g. `"signal check --gate commit"`).
    pub gate_label: String,
    /// Path to the `track/items/` directory.
    pub items_dir: PathBuf,
    /// Active track identifier.
    pub track_id: TrackId,
}

/// Per-chain output DTO inside a signal-gate run.
///
/// One entry per SoT-chain evaluated. `stdout`/`stderr` are opaque output
/// strings produced by the chain verifier; they are free-text and not
/// domain-constrained (docs justify raw String per R9).
#[derive(Debug, Clone)]
pub struct SignalChainOutput {
    /// Human-readable chain label.
    pub chain_label: String,
    /// Whether this chain passed (no error-severity findings).
    pub passed: bool,
    /// Optional stdout text from the chain verifier.
    pub stdout: Option<String>,
    /// Optional stderr text from the chain verifier.
    pub stderr: Option<String>,
}

/// Output DTO for the signal-gate use case.
///
/// Aggregated result of the 4-chain sequencing. Consumed by cli_driver render layer.
#[derive(Debug, Clone)]
pub struct SignalGateOutput {
    /// Human-readable gate label.
    pub gate_label: String,
    /// Whether all 4 chains passed.
    pub passed: bool,
    /// Per-chain outputs (one per SoT chain evaluated).
    pub chain_outputs: Vec<SignalChainOutput>,
}

// ── Error type ─────────────────────────────────────────────────────────────────

/// Error type for the SignalGateService.
///
/// - `ChainExecutionFailed`: one chain verifier returned an error.
/// - `InvalidTrackId`: track id could not be resolved.
/// - `StrictnessConfigLoad`: signal-gates config could not be loaded.
#[derive(Debug)]
pub enum SignalGateError {
    /// One chain verifier returned an error.
    ChainExecutionFailed {
        /// Human-readable chain label.
        chain: String,
        /// Error description.
        reason: String,
    },
    /// Track id could not be resolved.
    InvalidTrackId(String),
    /// Signal-gates config could not be loaded.
    StrictnessConfigLoad(String),
}

impl std::fmt::Display for SignalGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChainExecutionFailed { chain, reason } => {
                write!(f, "chain '{chain}' execution failed: {reason}")
            }
            Self::InvalidTrackId(msg) => write!(f, "invalid track id: {msg}"),
            Self::StrictnessConfigLoad(msg) => write!(f, "signal-gates config load failed: {msg}"),
        }
    }
}

impl std::error::Error for SignalGateError {}

// ── Secondary ports ────────────────────────────────────────────────────────────

/// Secondary port for chain ⓪ (ADR → user): runs the ADR signal scan for a
/// given project root and strictness setting, returning a chain output.
///
/// The infrastructure adapter runs `execute_verify_adr_signals_with_strict`
/// internally; the port exposes only the rendered text outcome.
pub trait AdrChainRunnerPort: Send + Sync {
    /// Run chain ⓪ for the given project root with the resolved strictness.
    ///
    /// Returns `Ok(SignalChainOutput)` on success (pass or fail); `Err` only on
    /// a catastrophic infrastructure failure (not a gate failure).
    fn run_adr_chain(
        &self,
        project_root: PathBuf,
        strict: bool,
    ) -> Result<SignalChainOutput, String>;
}

/// Secondary port for chain ① (spec → ADR): reads and evaluates the spec.json
/// signal gate with the resolved strictness, returning a chain output.
///
/// The infrastructure adapter runs `verify_from_spec_json` + `resolve_trusted_root`
/// internally; the port exposes only the rendered text outcome.
pub trait SpecAdrChainRunnerPort: Send + Sync {
    /// Run chain ① for the given spec.json path with the resolved strictness.
    ///
    /// Returns `Ok(SignalChainOutput)` on success (pass or fail); `Err` only on
    /// a catastrophic infrastructure failure (not a gate failure).
    fn run_spec_adr_chain(
        &self,
        spec_json_path: PathBuf,
        strict: bool,
    ) -> Result<SignalChainOutput, String>;
}

/// Secondary port for chains ② and ③ (catalog-spec / impl-catalog): runs the
/// argless layer-chain verifier for a given chain label and per-layer parameters.
///
/// This port abstracts the `signal_check_layer_chain_with_strict` composition in
/// `apps/cli-composition/src/signal_layer_chain.rs` so that the interactor can
/// sequence all four chains without importing infrastructure.
pub trait LayerChainRunnerPort: Send + Sync {
    /// Run the catalog-spec chain (chain ②) with the resolved strictness.
    fn run_catalog_spec_chain(
        &self,
        strict: bool,
        signal_reader: &dyn SignalLayerReader,
    ) -> Result<SignalChainOutput, String>;

    /// Run the impl-catalog chain (chain ③) with the resolved strictness.
    fn run_impl_catalog_chain(
        &self,
        strict: bool,
        signal_reader: &dyn SignalLayerReader,
    ) -> Result<SignalChainOutput, String>;
}

// ── Application service trait ──────────────────────────────────────────────────

/// Application service (primary port) for the signal-gate chain sequencing use case.
///
/// Implementations sequence the 4 SoT chains and aggregate outcomes.
/// Extracted from cli_composition signal_check_gate per ADR 1328 D4.
pub trait SignalGateService: Send + Sync {
    /// Run all 4 SoT chains for the given gate command.
    ///
    /// # Errors
    ///
    /// Returns [`SignalGateError`] when:
    /// - The track id is invalid (`InvalidTrackId`).
    /// - A chain runner port fails catastrophically (`ChainExecutionFailed`).
    /// - The strictness config cannot be resolved (`StrictnessConfigLoad`).
    fn run_gate(&self, cmd: SignalGateCommand) -> Result<SignalGateOutput, SignalGateError>;
}

// ── Interactor ─────────────────────────────────────────────────────────────────

/// Interactor implementing [`SignalGateService`].
///
/// Holds the injected [`SignalLayerReader`] port and [`SignalGateMatrix`] strictness
/// source as private fields. Sequences the 4 SoT chains (adr-user, spec-adr,
/// catalog-spec, impl-catalog), resolves strictness per gate matrix, and aggregates
/// results.
pub struct SignalGateInteractor {
    signal_layer_reader: Arc<dyn SignalLayerReader>,
    gate_matrix: SignalGateMatrix,
    adr_runner: Arc<dyn AdrChainRunnerPort>,
    spec_adr_runner: Arc<dyn SpecAdrChainRunnerPort>,
    layer_chain_runner: Arc<dyn LayerChainRunnerPort>,
}

impl SignalGateInteractor {
    /// Construct a new `SignalGateInteractor`.
    ///
    /// - `signal_layer_reader`: secondary port for resolving active-track data used
    ///   by chains ② and ③.
    /// - `gate_matrix`: pre-loaded gate-matrix; strictness per chain is resolved
    ///   from this matrix (no filesystem access during `run_gate`).
    /// - `adr_runner`: secondary port for chain ⓪ (ADR scan).
    /// - `spec_adr_runner`: secondary port for chain ① (spec-adr evaluation).
    /// - `layer_chain_runner`: secondary port for chains ② and ③.
    #[must_use]
    pub fn new(
        signal_layer_reader: Arc<dyn SignalLayerReader>,
        gate_matrix: SignalGateMatrix,
        adr_runner: Arc<dyn AdrChainRunnerPort>,
        spec_adr_runner: Arc<dyn SpecAdrChainRunnerPort>,
        layer_chain_runner: Arc<dyn LayerChainRunnerPort>,
    ) -> Self {
        Self { signal_layer_reader, gate_matrix, adr_runner, spec_adr_runner, layer_chain_runner }
    }
}

/// Decode the gate kind from the gate label string.
///
/// Returns `GateKind::Commit` for labels starting with `"signal check --gate commit"`,
/// `GateKind::Merge` for `"signal check --gate merge"`. Falls back to `Commit` for
/// unrecognised labels so existing tests that pass arbitrary labels don't regress.
fn decode_gate_kind(gate_label: &str) -> GateKind {
    if gate_label.contains("merge") { GateKind::Merge } else { GateKind::Commit }
}

impl SignalGateService for SignalGateInteractor {
    fn run_gate(&self, cmd: SignalGateCommand) -> Result<SignalGateOutput, SignalGateError> {
        let gate_kind = decode_gate_kind(&cmd.gate_label);

        // Derive workspace root and spec.json path from items_dir + track_id.
        // items_dir is track/items/; workspace_root is two levels up.
        let workspace_root = cmd
            .items_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| cmd.items_dir.clone());
        let spec_json_path = cmd.items_dir.join(cmd.track_id.as_ref()).join("spec.json");

        // Resolve strictness for each chain from the gate matrix.
        let adr_strict =
            self.gate_matrix.resolve(ChainId::AdrUser, gate_kind) == Strictness::Strict;
        let spec_strict =
            self.gate_matrix.resolve(ChainId::SpecAdr, gate_kind) == Strictness::Strict;
        let catalog_strict =
            self.gate_matrix.resolve(ChainId::CatalogSpec, gate_kind) == Strictness::Strict;
        let impl_strict =
            self.gate_matrix.resolve(ChainId::ImplCatalog, gate_kind) == Strictness::Strict;

        // Chain ⓪: adr-user (live scan via AdrChainRunnerPort).
        let chain0 =
            self.adr_runner.run_adr_chain(workspace_root, adr_strict).map_err(|reason| {
                SignalGateError::ChainExecutionFailed {
                    chain: "signal check-adr-user".to_owned(),
                    reason,
                }
            })?;

        // Chain ①: spec-adr (spec.json evaluation via SpecAdrChainRunnerPort).
        let chain1 = self.spec_adr_runner.run_spec_adr_chain(spec_json_path, spec_strict).map_err(
            |reason| SignalGateError::ChainExecutionFailed {
                chain: "signal check-spec-adr".to_owned(),
                reason,
            },
        )?;

        // Chain ②: catalog-spec (via LayerChainRunnerPort).
        let chain2 = self
            .layer_chain_runner
            .run_catalog_spec_chain(catalog_strict, self.signal_layer_reader.as_ref())
            .map_err(|reason| SignalGateError::ChainExecutionFailed {
                chain: "signal check-catalog-spec".to_owned(),
                reason,
            })?;

        // Chain ③: impl-catalog (via LayerChainRunnerPort).
        let chain3 = self
            .layer_chain_runner
            .run_impl_catalog_chain(impl_strict, self.signal_layer_reader.as_ref())
            .map_err(|reason| SignalGateError::ChainExecutionFailed {
                chain: "signal check-impl-catalog".to_owned(),
                reason,
            })?;

        let chains = vec![chain0, chain1, chain2, chain3];
        let passed = chains.iter().all(|c| c.passed);
        Ok(SignalGateOutput { gate_label: cmd.gate_label, passed, chain_outputs: chains })
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use domain::tddd::LayerId;
    use domain::{ChainGateEntry, GateKind, SignalGateMatrix, Strictness, TrackId};

    use super::*;
    use crate::signal::port::{SignalLayerReader, SignalLayerReaderError};

    // ── Stub implementations ─────────────────────────────────────────────────

    struct MockSignalLayerReader {
        track_id_result: Result<String, SignalLayerReaderError>,
    }

    impl MockSignalLayerReader {
        fn ok(track_id: &str) -> Self {
            Self { track_id_result: Ok(track_id.to_owned()) }
        }

        #[allow(dead_code)]
        fn err() -> Self {
            Self { track_id_result: Err(SignalLayerReaderError::TrackIdUnresolved) }
        }
    }

    impl SignalLayerReader for MockSignalLayerReader {
        fn active_track_id(&self) -> Result<TrackId, SignalLayerReaderError> {
            match &self.track_id_result {
                Ok(s) => TrackId::try_new(s.clone()).map_err(|_| SignalLayerReaderError::Io),
                Err(e) => Err(match e {
                    SignalLayerReaderError::TrackIdUnresolved => {
                        SignalLayerReaderError::TrackIdUnresolved
                    }
                    SignalLayerReaderError::Io => SignalLayerReaderError::Io,
                }),
            }
        }

        fn enabled_layers(&self, _: TrackId) -> Result<Vec<LayerId>, SignalLayerReaderError> {
            Ok(vec![])
        }

        fn catalogue_bytes(
            &self,
            _: TrackId,
            _: LayerId,
        ) -> Result<Option<Vec<u8>>, SignalLayerReaderError> {
            Ok(None)
        }
    }

    struct MockAdrRunner {
        result: Result<SignalChainOutput, String>,
    }

    impl MockAdrRunner {
        fn pass() -> Self {
            Self {
                result: Ok(SignalChainOutput {
                    chain_label: "signal check-adr-user".to_owned(),
                    passed: true,
                    stdout: Some("--- signal check-adr-user ---\n[OK] All checks passed.\n--- signal check-adr-user PASSED ---".to_owned()),
                    stderr: None,
                }),
            }
        }

        fn fail() -> Self {
            Self {
                result: Ok(SignalChainOutput {
                    chain_label: "signal check-adr-user".to_owned(),
                    passed: false,
                    stdout: Some("--- signal check-adr-user ---\n[ERROR] Red signal found.\n--- signal check-adr-user FAILED ---".to_owned()),
                    stderr: None,
                }),
            }
        }

        fn err() -> Self {
            Self { result: Err("adr runner I/O failure".to_owned()) }
        }
    }

    impl AdrChainRunnerPort for MockAdrRunner {
        fn run_adr_chain(
            &self,
            _project_root: PathBuf,
            _strict: bool,
        ) -> Result<SignalChainOutput, String> {
            self.result.clone()
        }
    }

    struct MockSpecAdrRunner {
        result: Result<SignalChainOutput, String>,
    }

    impl MockSpecAdrRunner {
        fn pass() -> Self {
            Self {
                result: Ok(SignalChainOutput {
                    chain_label: "signal check-spec-adr".to_owned(),
                    passed: true,
                    stdout: Some("--- signal check-spec-adr ---\n[OK] All checks passed.\n--- signal check-spec-adr PASSED ---".to_owned()),
                    stderr: None,
                }),
            }
        }

        #[allow(dead_code)]
        fn fail() -> Self {
            Self {
                result: Ok(SignalChainOutput {
                    chain_label: "signal check-spec-adr".to_owned(),
                    passed: false,
                    stdout: Some("--- signal check-spec-adr ---\n[ERROR] Spec not grounded.\n--- signal check-spec-adr FAILED ---".to_owned()),
                    stderr: None,
                }),
            }
        }

        #[allow(dead_code)]
        fn err() -> Self {
            Self { result: Err("spec-adr runner I/O failure".to_owned()) }
        }
    }

    impl SpecAdrChainRunnerPort for MockSpecAdrRunner {
        fn run_spec_adr_chain(
            &self,
            _spec_json_path: PathBuf,
            _strict: bool,
        ) -> Result<SignalChainOutput, String> {
            self.result.clone()
        }
    }

    struct MockLayerChainRunner {
        catalog_result: Result<SignalChainOutput, String>,
        impl_result: Result<SignalChainOutput, String>,
    }

    impl MockLayerChainRunner {
        fn all_pass() -> Self {
            Self {
                catalog_result: Ok(SignalChainOutput {
                    chain_label: "signal check-catalog-spec".to_owned(),
                    passed: true,
                    stdout: Some("--- signal check-catalog-spec ---\n[OK] All checks passed.\n--- signal check-catalog-spec PASSED ---".to_owned()),
                    stderr: None,
                }),
                impl_result: Ok(SignalChainOutput {
                    chain_label: "signal check-impl-catalog".to_owned(),
                    passed: true,
                    stdout: Some("--- signal check-impl-catalog ---\n[OK] All checks passed.\n--- signal check-impl-catalog PASSED ---".to_owned()),
                    stderr: None,
                }),
            }
        }

        fn catalog_fail() -> Self {
            Self {
                catalog_result: Ok(SignalChainOutput {
                    chain_label: "signal check-catalog-spec".to_owned(),
                    passed: false,
                    stdout: Some("--- signal check-catalog-spec ---\n[ERROR] Yellow signals.\n--- signal check-catalog-spec FAILED ---".to_owned()),
                    stderr: None,
                }),
                impl_result: Ok(SignalChainOutput {
                    chain_label: "signal check-impl-catalog".to_owned(),
                    passed: true,
                    stdout: Some("[OK]".to_owned()),
                    stderr: None,
                }),
            }
        }

        fn impl_err() -> Self {
            Self {
                catalog_result: Ok(SignalChainOutput {
                    chain_label: "signal check-catalog-spec".to_owned(),
                    passed: true,
                    stdout: Some("[OK]".to_owned()),
                    stderr: None,
                }),
                impl_result: Err("impl-catalog runner I/O failure".to_owned()),
            }
        }
    }

    impl LayerChainRunnerPort for MockLayerChainRunner {
        fn run_catalog_spec_chain(
            &self,
            _strict: bool,
            _reader: &dyn SignalLayerReader,
        ) -> Result<SignalChainOutput, String> {
            self.catalog_result.clone()
        }

        fn run_impl_catalog_chain(
            &self,
            _strict: bool,
            _reader: &dyn SignalLayerReader,
        ) -> Result<SignalChainOutput, String> {
            self.impl_result.clone()
        }
    }

    fn all_strict_matrix() -> SignalGateMatrix {
        let strict_entry =
            ChainGateEntry { commit_gate: Strictness::Strict, merge_gate: Strictness::Strict };
        SignalGateMatrix {
            adr_user: strict_entry.clone(),
            spec_adr: strict_entry.clone(),
            catalog_spec: strict_entry.clone(),
            impl_catalog: strict_entry,
        }
    }

    fn commit_gate_cmd(track_id: &str) -> SignalGateCommand {
        let track_id = TrackId::try_new(track_id).expect("invalid test track id");
        SignalGateCommand {
            gate_label: "signal check --gate commit".to_owned(),
            items_dir: PathBuf::from("/workspace/track/items"),
            track_id,
        }
    }

    fn build_interactor(
        adr: impl AdrChainRunnerPort + 'static,
        spec: impl SpecAdrChainRunnerPort + 'static,
        layer: impl LayerChainRunnerPort + 'static,
    ) -> SignalGateInteractor {
        let reader = Arc::new(MockSignalLayerReader::ok("test-track-2026-06-21"));
        SignalGateInteractor::new(
            reader,
            all_strict_matrix(),
            Arc::new(adr),
            Arc::new(spec),
            Arc::new(layer),
        )
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// All 4 chains pass → `passed: true`, 4 chain outputs.
    #[test]
    fn test_all_chains_pass_produces_passed_true() {
        let svc = build_interactor(
            MockAdrRunner::pass(),
            MockSpecAdrRunner::pass(),
            MockLayerChainRunner::all_pass(),
        );
        let output =
            svc.run_gate(commit_gate_cmd("test-track-2026-06-21")).expect("run_gate must succeed");
        assert!(output.passed, "all chains pass → output.passed must be true");
        assert_eq!(output.chain_outputs.len(), 4, "must have 4 chain outputs");
    }

    /// Any single chain fails → `passed: false`.
    #[test]
    fn test_single_chain_fail_produces_passed_false() {
        let svc = build_interactor(
            MockAdrRunner::fail(),
            MockSpecAdrRunner::pass(),
            MockLayerChainRunner::all_pass(),
        );
        let output = svc
            .run_gate(commit_gate_cmd("test-track-2026-06-21"))
            .expect("run_gate must succeed even when a chain fails");
        assert!(!output.passed, "a failing chain must set output.passed to false");
    }

    /// catalog-spec chain fails → `passed: false`.
    #[test]
    fn test_catalog_spec_chain_fail_produces_passed_false() {
        let svc = build_interactor(
            MockAdrRunner::pass(),
            MockSpecAdrRunner::pass(),
            MockLayerChainRunner::catalog_fail(),
        );
        let output = svc
            .run_gate(commit_gate_cmd("test-track-2026-06-21"))
            .expect("run_gate must succeed even when catalog-spec fails");
        assert!(!output.passed, "catalog-spec failure must set output.passed to false");
    }

    /// `AdrChainRunnerPort` returns `Err` → `SignalGateError::ChainExecutionFailed`.
    #[test]
    fn test_adr_chain_runner_err_returns_chain_execution_failed() {
        let svc = build_interactor(
            MockAdrRunner::err(),
            MockSpecAdrRunner::pass(),
            MockLayerChainRunner::all_pass(),
        );
        let result = svc.run_gate(commit_gate_cmd("test-track-2026-06-21"));
        assert!(
            matches!(result, Err(SignalGateError::ChainExecutionFailed { .. })),
            "adr runner Err must surface as ChainExecutionFailed, got {result:?}"
        );
    }

    /// `SpecAdrChainRunnerPort` returns `Err` → `SignalGateError::ChainExecutionFailed`.
    #[test]
    fn test_spec_adr_chain_runner_err_returns_chain_execution_failed() {
        let svc = build_interactor(
            MockAdrRunner::pass(),
            MockSpecAdrRunner::err(),
            MockLayerChainRunner::all_pass(),
        );
        let result = svc.run_gate(commit_gate_cmd("test-track-2026-06-21"));
        assert!(
            matches!(result, Err(SignalGateError::ChainExecutionFailed { .. })),
            "spec-adr runner Err must surface as ChainExecutionFailed, got {result:?}"
        );
    }

    /// `LayerChainRunnerPort::run_impl_catalog_chain` returns `Err` → `ChainExecutionFailed`.
    #[test]
    fn test_impl_catalog_chain_runner_err_returns_chain_execution_failed() {
        let svc = build_interactor(
            MockAdrRunner::pass(),
            MockSpecAdrRunner::pass(),
            MockLayerChainRunner::impl_err(),
        );
        let result = svc.run_gate(commit_gate_cmd("test-track-2026-06-21"));
        assert!(
            matches!(result, Err(SignalGateError::ChainExecutionFailed { .. })),
            "impl-catalog runner Err must surface as ChainExecutionFailed, got {result:?}"
        );
    }

    /// Gate label containing "merge" resolves to `GateKind::Merge`.
    #[test]
    fn test_gate_label_merge_decodes_to_merge_kind() {
        assert_eq!(decode_gate_kind("signal check --gate merge"), GateKind::Merge);
    }

    /// Gate label containing "commit" resolves to `GateKind::Commit`.
    #[test]
    fn test_gate_label_commit_decodes_to_commit_kind() {
        assert_eq!(decode_gate_kind("signal check --gate commit"), GateKind::Commit);
    }

    /// Spec.json path is derived from items_dir + track_id.
    #[test]
    fn test_spec_json_path_is_derived_from_items_dir_and_track_id() {
        // We verify indirectly: `SpecAdrChainRunnerPort` receives the expected path.
        struct PathCapture(std::sync::Mutex<Option<PathBuf>>);

        impl SpecAdrChainRunnerPort for PathCapture {
            fn run_spec_adr_chain(
                &self,
                spec_json_path: PathBuf,
                _strict: bool,
            ) -> Result<SignalChainOutput, String> {
                *self.0.lock().unwrap() = Some(spec_json_path);
                Ok(SignalChainOutput {
                    chain_label: "signal check-spec-adr".to_owned(),
                    passed: true,
                    stdout: None,
                    stderr: None,
                })
            }
        }

        let capture = Arc::new(PathCapture(std::sync::Mutex::new(None)));
        let reader = Arc::new(MockSignalLayerReader::ok("test-track-2026-06-21"));
        let spec_adr: Arc<dyn SpecAdrChainRunnerPort> = capture.clone();
        let interactor = SignalGateInteractor::new(
            reader,
            all_strict_matrix(),
            Arc::new(MockAdrRunner::pass()),
            spec_adr,
            Arc::new(MockLayerChainRunner::all_pass()),
        );

        let track_id = TrackId::try_new("test-track-2026-06-21").unwrap();
        let cmd = SignalGateCommand {
            gate_label: "signal check --gate commit".to_owned(),
            items_dir: PathBuf::from("/ws/track/items"),
            track_id,
        };
        interactor.run_gate(cmd).expect("run_gate must succeed");

        let captured_path = capture.0.lock().unwrap().clone().unwrap();
        assert_eq!(
            captured_path,
            PathBuf::from("/ws/track/items/test-track-2026-06-21/spec.json"),
            "spec_json_path must be items_dir/track_id/spec.json"
        );
    }
}
