//! Foundational domain types and traits for the 4-chain SoT signal taxonomy.
//!
//! # Chain taxonomy
//!
//! SoTOHE evaluates track artifacts using four signal chains. Each chain is one
//! link in the SoT Chain (reference chain traversed from downstream to upstream):
//!
//! | `ChainId` variant | UI name       | SoT Chain direction            | Evaluated concern                          |
//! |-------------------|---------------|--------------------------------|--------------------------------------------|
//! | `AdrUser`         | `adr-user`    | ADR → user decision            | ADR decision provenance completeness       |
//! | `SpecAdr`         | `spec-adr`    | spec → ADR                     | Spec requirement grounding completeness    |
//! | `CatalogSpec`     | `catalog-spec`| catalogue → spec               | Type catalogue spec-reference completeness |
//! | `ImplCatalog`     | `impl-catalog`| implementation ↔ catalogue     | Type catalogue ↔ rustdoc API consistency   |
//!
//! # Strictness semantics
//!
//! A [`Strictness`] value governs how `Yellow` (`ConfidenceSignal::Yellow`) signals
//! are treated at gate evaluation time:
//!
//! - [`Strictness::Strict`]: Yellow signals produce `Finding::error` (block).
//! - [`Strictness::Interim`]: Yellow signals produce `Finding::warning` (pass with warning).
//!
//! `Red` signals always produce `Finding::error` regardless of strictness. `Blue` signals
//! always pass. The strictness that applies at any given gate invocation is resolved from
//! a [`SignalGateMatrix`] loaded from `.harness/config/signal-gates.json`.
//!
//! # Trait hierarchy
//!
//! ```text
//! ChainIdentity                      (domain)
//!   ├── SoTChain                     (usecase::chain::traits) ← minimal check contract
//!   │     └── LiveSoTChain           (usecase::chain::traits) ← live calc without persistence
//!   └── PersistedSoTChainGate        (domain) ← pure gate; chains ①②③
//!         └── LoadablePersistedChain (usecase::chain::traits) ← I/O port; chains ①②③
//!               └── PersistedSoTChain (usecase::chain::traits) ← sealed supertrait
//!                     (blanket impl of SoTChain for any T: PersistedSoTChain)
//! ```
//!
//! `SoTChain`, `LiveSoTChain`, `LoadablePersistedChain`, and `PersistedSoTChain` were
//! moved to `usecase::chain::traits` (reviewer Finding #1): those traits cover I/O and
//! dispatch concerns that belong in the usecase layer. `ChainIdentity` and
//! `PersistedSoTChainGate` remain here as pure domain contracts.

use crate::ConfidenceSignal;
use crate::tddd::catalogue_spec_signal::CatalogueSpecSignalsDocument;
use crate::verify::{VerifyFinding, VerifyOutcome};

// ── Value types ──────────────────────────────────────────────────────────────

/// Discriminant identifying one of the four SoT Chain signal evaluation chains.
///
/// Used as the `ID` associated constant of [`ChainIdentity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainId {
    /// Chain ⓪: ADR → user decision (provenance / grounding completeness).
    AdrUser,
    /// Chain ①: spec → ADR (spec requirement grounding completeness).
    SpecAdr,
    /// Chain ②: catalogue → spec (type catalogue spec-reference completeness).
    CatalogSpec,
    /// Chain ③: implementation ↔ catalogue (TDDD catalogue ↔ rustdoc API consistency).
    ImplCatalog,
}

/// Two-valued strictness discriminant for gate evaluation.
///
/// - [`Strictness::Strict`]: Yellow signals are treated as `Finding::error`.
/// - [`Strictness::Interim`]: Yellow signals are treated as `Finding::warning` only.
///
/// `Red` signals are always `Finding::error` regardless of strictness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Yellow signals produce a blocking error in addition to Red.
    Strict,
    /// Yellow signals produce a warning only; only Red signals block.
    Interim,
}

/// Discriminant for the two gate invocation contexts.
///
/// Used as the key axis when resolving a [`Strictness`] value from [`SignalGateMatrix`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateKind {
    /// CI commit gate (evaluated during `cargo make ci`).
    Commit,
    /// PR merge gate (evaluated by `check_strict_merge_gate`).
    Merge,
}

/// Per-chain gate configuration row: one [`Strictness`] value per [`GateKind`].
///
/// The two fields directly correspond to the two cells in the `signal-gates.json`
/// per-chain object. This is the parsed, validated domain representation of a single
/// chain row; the [`SignalGateMatrix`] holds four of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainGateEntry {
    /// Strictness to apply at the commit gate.
    pub commit_gate: Strictness,
    /// Strictness to apply at the merge gate.
    pub merge_gate: Strictness,
}

impl ChainGateEntry {
    /// Resolves the [`Strictness`] value for the given [`GateKind`].
    pub fn resolve(&self, gate: GateKind) -> Strictness {
        match gate {
            GateKind::Commit => self.commit_gate,
            GateKind::Merge => self.merge_gate,
        }
    }
}

/// Complete 4-chain × 2-gate strictness matrix.
///
/// Each field holds the per-chain [`ChainGateEntry`] (commit + merge strictness).
/// This is the domain model of the `signal-gates.json` body after schema validation.
/// No implicit defaults: every cell is present and must be provided explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalGateMatrix {
    /// Gate configuration for chain ⓪ (`adr-user`).
    pub adr_user: ChainGateEntry,
    /// Gate configuration for chain ① (`spec-adr`).
    pub spec_adr: ChainGateEntry,
    /// Gate configuration for chain ② (`catalog-spec`).
    pub catalog_spec: ChainGateEntry,
    /// Gate configuration for chain ③ (`impl-catalog`).
    pub impl_catalog: ChainGateEntry,
}

impl SignalGateMatrix {
    /// Resolves the [`Strictness`] for the given `(chain, gate)` cell.
    pub fn resolve(&self, chain: ChainId, gate: GateKind) -> Strictness {
        let entry = match chain {
            ChainId::AdrUser => &self.adr_user,
            ChainId::SpecAdr => &self.spec_adr,
            ChainId::CatalogSpec => &self.catalog_spec,
            ChainId::ImplCatalog => &self.impl_catalog,
        };
        entry.resolve(gate)
    }
}

// ── Pure gate functions ───────────────────────────────────────────────────────

/// Evaluates Chain ② (catalogue → spec) signal gate rules against a
/// [`CatalogueSpecSignalsDocument`].
///
/// Pure function used by both the CI path (`execute_catalogue_spec_signals`)
/// and the merge gate (via `check_chain2_for_layer`). Callers are responsible
/// for all integrity checks — coverage count, positional name match, entry-hash
/// freshness, and `catalogue_declaration_hash` staleness — **before** calling
/// this function. This function only evaluates the Red/Yellow signal gate on the
/// signals already present in `doc`.
///
/// Symmetric with:
/// - `check_spec_doc_signals` (chain ①, `libs/domain/src/spec.rs`)
/// - `check_type_signals` (chain ③, `libs/domain/src/tddd/consistency.rs`)
///
/// # Rules
///
/// - No signals (`doc.signals` is empty) → `VerifyOutcome::pass()` (empty catalogue / no entries).
/// - Any Red signal → `VerifyFinding::error` (unconditional, regardless of `strictness`).
/// - Yellow signal, `Strictness::Strict` → `VerifyFinding::error`.
/// - Yellow signal, `Strictness::Interim` → `VerifyFinding::warning`.
/// - All Blue / no Yellow → `VerifyOutcome::pass()`.
///
/// Taking [`Strictness`] (not `bool`) preserves type safety per
/// `knowledge/conventions/prefer-type-safe-abstractions.md` § Enum-first: callers
/// pass a domain-named discriminant so an inverted conversion cannot silently flip
/// gate behavior.
///
/// Reference: ADR `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` §D2.
#[must_use]
pub fn check_catalogue_spec_signals(
    doc: &CatalogueSpecSignalsDocument,
    strictness: Strictness,
) -> VerifyOutcome {
    let signals = &doc.signals;

    // Empty signals → pass (empty catalogue / no entries).
    if signals.is_empty() {
        return VerifyOutcome::pass();
    }

    // Red check: always an error, regardless of strictness.
    let reds: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal == ConfidenceSignal::Red)
        .map(|s| s.type_name.as_str())
        .collect();
    if !reds.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{} catalogue entry/entries have Red catalogue-spec signal \
             (missing both spec_refs[] and informal_grounds[] — every entry must carry \
             at least one grounding ref): {}",
            reds.len(),
            reds.join(", ")
        ))]);
    }

    // Yellow check: error in Strict mode, warning in Interim mode.
    let yellows: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal == ConfidenceSignal::Yellow)
        .map(|s| s.type_name.as_str())
        .collect();
    if !yellows.is_empty() {
        let message = format!(
            "{} catalogue entry/entries have Yellow catalogue-spec signal \
             — merge gate will block these until upgraded to Blue. Upgrade by promoting \
             informal_grounds[] to spec_refs[] entries with file + anchor, \
             then regenerate catalogue-spec signals: {}",
            yellows.len(),
            yellows.join(", ")
        );
        return match strictness {
            Strictness::Strict => VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]),
            Strictness::Interim => {
                VerifyOutcome::from_findings(vec![VerifyFinding::warning(message)])
            }
        };
    }

    VerifyOutcome::pass()
}

// ── Traits ───────────────────────────────────────────────────────────────────

/// Marker trait shared by all four SoT Chain types.
///
/// Carries:
/// - `ID: ChainId` — the associated constant that identifies the chain.
/// - `type Input<'a>` — the input data structure the chain reads (GAT). Each chain
///   defines its own input type; for example, chain ⓪ (`AdrUserChain`) uses
///   `type Input<'a> = &'a std::path::Path` (the project root directory), which
///   matches the signature of `execute_verify_adr_signals(project_root: PathBuf)`.
///   Chains ① ② ③ use paths to their respective persisted signal documents.
///
/// # Choice rationale for chain ⓪ `Input<'a>`
///
/// The open question from the impl-planner was whether `AdrUserChain::Input<'a>`
/// should be `&'a Path` or `PathBuf`. The implementation uses `&'a std::path::Path`
/// because:
///
/// 1. `execute_verify_adr_signals` already accepts a `PathBuf` value; callers can pass
///    `path.as_path()` trivially without an additional clone.
/// 2. Borrowed `&Path` is the idiomatic Rust choice for read-only path parameters,
///    consistent with how `check_spec_doc_signals` and similar domain functions take
///    `&SpecDocument` rather than owned types.
/// 3. Later tasks (T005) implementing `AdrUserChain` can always call
///    `.to_path_buf()` internally when an owned value is needed, keeping the public
///    contract zero-copy.
///
/// Implementing this trait declares a new chain to the harness.
pub trait ChainIdentity {
    /// The chain discriminant — used for dispatch in [`SignalGateMatrix::resolve`].
    const ID: ChainId;

    /// The input type the chain reads. Defined as a GAT to allow each chain to
    /// borrow from the caller's stack without requiring heap allocation.
    type Input<'a>;
}

/// Pure gate contract for chains ①②③ that persist their computed signal documents.
///
/// Carries only the pure, value-level concerns that the domain layer owns:
/// - associated types `Persisted`, `CalcError`, `StaleError` (the document and error shapes)
/// - `evaluate_gate`: delegate to the corresponding domain pure function
/// - `calc_error` / `stale_error`: convert chain-specific errors into [`VerifyOutcome`]
///
/// The impure I/O methods (`calc`, `load`, `check_freshness`) live in
/// `usecase::chain::LoadablePersistedChain`, keeping the domain trait free of
/// I/O concerns per hexagonal-architecture port placement rules (CN-05).
///
/// Chain ⓪ does **not** implement this trait.
///
/// # Associated types
///
/// - `Persisted`: the persisted document type (e.g. `SpecDocument`, `CatalogueSpecSignalsDocument`,
///   `TypeSignalsDocument`).
/// - `CalcError`: error produced when computing or loading the persisted document fails.
/// - `StaleError`: error produced when freshness verification detects a stale document.
pub trait PersistedSoTChainGate: ChainIdentity {
    /// The persisted signal document type.
    type Persisted;
    /// Error produced when computing or loading the persisted document fails.
    type CalcError;
    /// Error produced when freshness verification detects a stale document.
    type StaleError;

    /// Apply the signal-gate logic to `persisted` and return the verdict.
    ///
    /// Delegates to the corresponding domain pure function:
    /// - ① `check_spec_doc_signals(&doc, strictness)`
    /// - ② `check_catalogue_spec_signals(&doc, strictness)` (D2 new function)
    /// - ③ `check_type_signals(&doc, strictness)`
    fn evaluate_gate(persisted: &Self::Persisted, strictness: Strictness) -> VerifyOutcome;

    /// Convert a `CalcError` into a [`VerifyOutcome`] for uniform CLI display.
    fn calc_error(error: Self::CalcError) -> VerifyOutcome;

    /// Convert a `StaleError` into a [`VerifyOutcome`] for uniform CLI display.
    fn stale_error(error: Self::StaleError) -> VerifyOutcome;
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ── ChainGateEntry::resolve ───────────────────────────────────────────────

    #[test]
    fn test_chain_gate_entry_resolve_with_commit_gate_returns_commit_strictness() {
        let entry =
            ChainGateEntry { commit_gate: Strictness::Strict, merge_gate: Strictness::Interim };
        assert_eq!(entry.resolve(GateKind::Commit), Strictness::Strict);
    }

    #[test]
    fn test_chain_gate_entry_resolve_with_merge_gate_returns_merge_strictness() {
        let entry =
            ChainGateEntry { commit_gate: Strictness::Strict, merge_gate: Strictness::Interim };
        assert_eq!(entry.resolve(GateKind::Merge), Strictness::Interim);
    }

    #[test]
    fn test_chain_gate_entry_resolve_interim_commit_strict_merge() {
        let entry =
            ChainGateEntry { commit_gate: Strictness::Interim, merge_gate: Strictness::Strict };
        assert_eq!(entry.resolve(GateKind::Commit), Strictness::Interim);
        assert_eq!(entry.resolve(GateKind::Merge), Strictness::Strict);
    }

    // ── SignalGateMatrix::resolve ─────────────────────────────────────────────

    fn recommended_default_matrix() -> SignalGateMatrix {
        // Mirrors the recommended default from ADR D3:
        //   commit_gate: adr_user=interim, spec_adr=strict, catalog_spec=strict, impl_catalog=interim
        //   merge_gate:  all strict
        SignalGateMatrix {
            adr_user: ChainGateEntry {
                commit_gate: Strictness::Interim,
                merge_gate: Strictness::Strict,
            },
            spec_adr: ChainGateEntry {
                commit_gate: Strictness::Strict,
                merge_gate: Strictness::Strict,
            },
            catalog_spec: ChainGateEntry {
                commit_gate: Strictness::Strict,
                merge_gate: Strictness::Strict,
            },
            impl_catalog: ChainGateEntry {
                commit_gate: Strictness::Interim,
                merge_gate: Strictness::Strict,
            },
        }
    }

    #[test]
    fn test_signal_gate_matrix_resolve_adr_user_commit_returns_interim() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::AdrUser, GateKind::Commit), Strictness::Interim);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_adr_user_merge_returns_strict() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::AdrUser, GateKind::Merge), Strictness::Strict);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_spec_adr_commit_returns_strict() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::SpecAdr, GateKind::Commit), Strictness::Strict);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_spec_adr_merge_returns_strict() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::SpecAdr, GateKind::Merge), Strictness::Strict);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_catalog_spec_commit_returns_strict() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::CatalogSpec, GateKind::Commit), Strictness::Strict);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_catalog_spec_merge_returns_strict() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::CatalogSpec, GateKind::Merge), Strictness::Strict);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_impl_catalog_commit_returns_interim() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::ImplCatalog, GateKind::Commit), Strictness::Interim);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_impl_catalog_merge_returns_strict() {
        let matrix = recommended_default_matrix();
        assert_eq!(matrix.resolve(ChainId::ImplCatalog, GateKind::Merge), Strictness::Strict);
    }

    #[test]
    fn test_signal_gate_matrix_resolve_all_strict_matrix_returns_strict_for_all_cells() {
        let all_strict = SignalGateMatrix {
            adr_user: ChainGateEntry {
                commit_gate: Strictness::Strict,
                merge_gate: Strictness::Strict,
            },
            spec_adr: ChainGateEntry {
                commit_gate: Strictness::Strict,
                merge_gate: Strictness::Strict,
            },
            catalog_spec: ChainGateEntry {
                commit_gate: Strictness::Strict,
                merge_gate: Strictness::Strict,
            },
            impl_catalog: ChainGateEntry {
                commit_gate: Strictness::Strict,
                merge_gate: Strictness::Strict,
            },
        };

        for chain in
            [ChainId::AdrUser, ChainId::SpecAdr, ChainId::CatalogSpec, ChainId::ImplCatalog]
        {
            for gate in [GateKind::Commit, GateKind::Merge] {
                assert_eq!(
                    all_strict.resolve(chain, gate),
                    Strictness::Strict,
                    "expected Strict for {chain:?} / {gate:?}"
                );
            }
        }
    }

    // ── check_catalogue_spec_signals ─────────────────────────────────────────

    use crate::plan_ref::ContentHash;
    use crate::tddd::catalogue_spec_signal::{CatalogueSpecSignal, CatalogueSpecSignalsDocument};
    use crate::verify::Severity;

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn signal(name: &str, sig: ConfidenceSignal) -> CatalogueSpecSignal {
        CatalogueSpecSignal::new(name, sig, hash(0x00))
    }

    fn doc(signals: Vec<CatalogueSpecSignal>) -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(hash(0xcd), signals)
    }

    #[test]
    fn test_check_catalogue_spec_signals_empty_signals_passes() {
        let outcome = check_catalogue_spec_signals(&doc(vec![]), Strictness::Interim);
        assert!(outcome.findings().is_empty(), "empty signals must pass (no entries): {outcome:?}");
    }

    #[test]
    fn test_check_catalogue_spec_signals_all_blue_passes_in_both_modes() {
        let d = doc(vec![
            signal("TypeA", ConfidenceSignal::Blue),
            signal("TypeB", ConfidenceSignal::Blue),
        ]);

        let outcome_interim = check_catalogue_spec_signals(&d, Strictness::Interim);
        assert!(
            outcome_interim.findings().is_empty(),
            "all-Blue interim must produce zero findings: {outcome_interim:?}"
        );

        let outcome_strict = check_catalogue_spec_signals(&d, Strictness::Strict);
        assert!(
            outcome_strict.findings().is_empty(),
            "all-Blue strict must produce zero findings: {outcome_strict:?}"
        );
    }

    #[test]
    fn test_check_catalogue_spec_signals_red_is_error_regardless_of_strict() {
        let d = doc(vec![
            signal("TypeA", ConfidenceSignal::Blue),
            signal("TypeB", ConfidenceSignal::Red),
        ]);

        let outcome_interim = check_catalogue_spec_signals(&d, Strictness::Interim);
        assert!(
            outcome_interim.has_errors(),
            "red must be an error in interim mode: {outcome_interim:?}"
        );
        let msg = outcome_interim.findings()[0].message();
        assert!(msg.contains("TypeB"), "error must name the offending entry: {msg}");

        let outcome_strict = check_catalogue_spec_signals(&d, Strictness::Strict);
        assert!(
            outcome_strict.has_errors(),
            "red must be an error in strict mode: {outcome_strict:?}"
        );
    }

    #[test]
    fn test_check_catalogue_spec_signals_yellow_is_warning_in_interim_mode() {
        let d = doc(vec![
            signal("TypeA", ConfidenceSignal::Blue),
            signal("TypeB", ConfidenceSignal::Yellow),
        ]);

        let outcome = check_catalogue_spec_signals(&d, Strictness::Interim);
        assert!(!outcome.has_errors(), "yellow in interim mode must not be an error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1, "expected exactly one warning finding");
        assert_eq!(findings[0].severity(), Severity::Warning);
        let msg = findings[0].message();
        assert!(msg.contains("TypeB"), "warning must name the offending entry: {msg}");
        assert!(msg.contains("merge gate will block"), "must warn about merge gate: {msg}");
    }

    #[test]
    fn test_check_catalogue_spec_signals_yellow_is_error_in_strict_mode() {
        let d = doc(vec![
            signal("TypeA", ConfidenceSignal::Blue),
            signal("TypeB", ConfidenceSignal::Yellow),
        ]);

        let outcome = check_catalogue_spec_signals(&d, Strictness::Strict);
        assert!(outcome.has_errors(), "yellow in strict mode must be an error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), Severity::Error);
        let msg = findings[0].message();
        assert!(msg.contains("TypeB"), "error must name the offending entry: {msg}");
    }
}
