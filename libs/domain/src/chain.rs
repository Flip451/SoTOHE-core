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
//! ChainIdentity
//!   ├── SoTChain              ← minimal check contract; all 4 chains satisfy this
//!   │     └── LiveSoTChain   ← live calc without persistence; chain ⓪ (adr-user) only
//!   └── PersistedSoTChain    ← calc+load+freshness+gate; chains ①②③
//!         (blanket impl of SoTChain for any T: PersistedSoTChain)
//! ```
//!
//! The split between [`LiveSoTChain`] and [`PersistedSoTChain`] makes illegal states
//! unrepresentable: chain ⓪ cannot be asked to `calc` into a file or to `check_freshness`
//! against a persisted document — those operations do not exist on the type.

use crate::verify::VerifyOutcome;

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

/// Minimal check contract satisfied by all four chains.
///
/// Provides `check(input, strict) -> VerifyOutcome`. The `strict` flag is resolved
/// by the caller from a [`SignalGateMatrix`] before dispatch.
///
/// For chains ①②③ this is fulfilled via the blanket impl over [`PersistedSoTChain`];
/// for chain ⓪ it is fulfilled directly (because chain ⓪ does not persist state).
/// CLI `check-*` dispatch goes through this trait.
pub trait SoTChain: ChainIdentity {
    /// Evaluate the chain's gate for the given input, returning a [`VerifyOutcome`].
    ///
    /// - `strict = true`: Yellow signals produce `Finding::error`.
    /// - `strict = false`: Yellow signals produce `Finding::warning`.
    ///
    /// Red signals always produce `Finding::error` regardless of `strict`.
    fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome;
}

/// Extension of [`SoTChain`] for chains that compute signals live without persisting.
///
/// Implemented **only** by `AdrUserChain` (chain ⓪). `calc_live` returns the
/// live-computed result without writing any file. Compiling [`PersistedSoTChain::calc`]
/// for chain ⓪ is intentionally impossible — the types do not exist on the chain type.
///
/// # Associated types
///
/// - `LiveCalc`: the computed result type returned by `calc_live`.
/// - `CalcError`: the error type returned when live calculation fails.
pub trait LiveSoTChain: SoTChain {
    /// The live-computed signal result type (e.g. a grounding report for chain ⓪).
    type LiveCalc;
    /// Error produced when `calc_live` cannot complete.
    type CalcError;

    /// Compute the chain's signals live from `input` without writing any persisted file.
    ///
    /// Used by `signal calc-adr-user`. The result is displayed to the user or
    /// consumed by the caller; it is never serialised to disk by this call.
    fn calc_live(input: &Self::Input<'_>) -> Result<Self::LiveCalc, Self::CalcError>;
}

/// Full contract for chains ①②③ that persist their computed signal documents.
///
/// Provides:
/// - [`calc`](PersistedSoTChain::calc): compute signals and write the persisted document.
/// - [`load`](PersistedSoTChain::load): read an already-persisted document from disk.
/// - [`check_freshness`](PersistedSoTChain::check_freshness): detect stale calc results
///   (e.g. hash comparison, self-consistency checks — see ADR D7 §5 for per-chain details).
/// - [`evaluate_gate`](PersistedSoTChain::evaluate_gate): delegate to the domain pure
///   function (`check_spec_doc_signals` / `check_catalogue_spec_signals` /
///   `check_type_signals`) for the signal-gate logic.
/// - [`calc_error`](PersistedSoTChain::calc_error) /
///   [`stale_error`](PersistedSoTChain::stale_error): convert chain-specific error types
///   into a [`VerifyOutcome`] for uniform CLI output.
///
/// Chain ⓪ does **not** implement this trait. The blanket impl `impl<T: PersistedSoTChain>
/// SoTChain for T` ensures that the `SoTChain::check` path for chains ①②③ always runs
/// `load → check_freshness → evaluate_gate`, making it impossible to skip freshness
/// verification in an ad-hoc `check` implementation.
///
/// # Associated types
///
/// - `Persisted`: the persisted document type (e.g. `SpecDocument`, `CatalogueSpecSignalsDocument`,
///   `TypeSignalsDocument`).
/// - `CalcError`: error produced by [`calc`](PersistedSoTChain::calc) or
///   [`load`](PersistedSoTChain::load).
/// - `StaleError`: error produced by [`check_freshness`](PersistedSoTChain::check_freshness)
///   when the persisted document is out of date.
pub trait PersistedSoTChain: ChainIdentity {
    /// The persisted signal document type.
    type Persisted;
    /// Error produced when computing or loading the persisted document fails.
    type CalcError;
    /// Error produced when freshness verification detects a stale document.
    type StaleError;

    /// Compute signals from `input` and write the result to the persisted document path.
    ///
    /// Used by `signal calc-<chain>` for chains ①②③.
    fn calc(input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError>;

    /// Load a previously persisted document from the path specified in `input`.
    ///
    /// Returns `Err(CalcError)` if the document cannot be read or deserialized (which
    /// typically means `calc` has never been run or the file was deleted).
    fn load(input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError>;

    /// Verify that `persisted` is still up-to-date relative to the current `input`.
    ///
    /// Returns `Ok(())` if the document is fresh, or `Err(StaleError)` if the
    /// underlying sources have changed since the last `calc` run. Stale detection
    /// is always a hard error — callers must rerun `calc` before proceeding.
    ///
    /// Per ADR §5 each chain has its own freshness mechanism:
    /// - ① self-consistency check (`evaluate_signals()` vs stored `signals()`).
    /// - ② entry hash comparison (`entry_hash` vs current catalogue bytes SHA-256).
    /// - ③ declaration hash comparison (`declaration_hash` vs current catalogue bytes SHA-256).
    fn check_freshness(
        input: &Self::Input<'_>,
        persisted: &Self::Persisted,
    ) -> Result<(), Self::StaleError>;

    /// Apply the signal-gate logic to `persisted` and return the verdict.
    ///
    /// Delegates to the corresponding domain pure function:
    /// - ① `check_spec_doc_signals(&doc, strict)`
    /// - ② `check_catalogue_spec_signals(&doc, strict)` (D2 new function)
    /// - ③ `check_type_signals(&doc, strict)`
    fn evaluate_gate(persisted: &Self::Persisted, strict: bool) -> VerifyOutcome;

    /// Convert a `CalcError` into a [`VerifyOutcome`] for uniform CLI display.
    fn calc_error(error: Self::CalcError) -> VerifyOutcome;

    /// Convert a `StaleError` into a [`VerifyOutcome`] for uniform CLI display.
    fn stale_error(error: Self::StaleError) -> VerifyOutcome;
}

// ── Blanket impl ─────────────────────────────────────────────────────────────

/// Blanket impl: any `T: PersistedSoTChain` automatically satisfies [`SoTChain`].
///
/// The `check` pipeline is fixed as `load → check_freshness → evaluate_gate`, making
/// it impossible to implement a persisted chain's `check` that skips freshness.
///
/// `#[doc(hidden)]` keeps this off the rustdoc-derived TDDD signal surface: catalogue
/// schema v5 has no shape for a generic `impl<T: Bound> Trait for T` (`for_type` must
/// be a concrete TypeRef), so without `doc(hidden)` rustdoc reports this impl as an
/// uncatalogued extra and the chain ③ signal turns Red. The behaviour is unchanged
/// at the language level — every `T: PersistedSoTChain` still gets a `SoTChain` impl.
#[doc(hidden)]
impl<T> SoTChain for T
where
    T: PersistedSoTChain,
{
    fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome {
        let persisted = match T::load(input) {
            Ok(persisted) => persisted,
            Err(error) => return T::calc_error(error),
        };
        match T::check_freshness(input, &persisted) {
            Ok(()) => T::evaluate_gate(&persisted, strict),
            Err(error) => T::stale_error(error),
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
}
