//! Usecase-layer chain traits: `SoTChain`, `LiveSoTChain`, `LoadablePersistedChain`,
//! and `PersistedSoTChain`.
//!
//! These traits were moved from `domain::chain` to usecase in response to reviewer
//! Finding #1: `LoadablePersistedChain` and `SoTChain` cover I/O and dispatch concerns
//! that belong in the usecase layer. The orphan rule permits defining blanket impls here
//! because `SoTChain`, `LoadablePersistedChain`, and `PersistedSoTChain` are all defined
//! in this crate.
//!
//! `domain` items used here: `ChainIdentity`, `PersistedSoTChainGate`, `Strictness`,
//! `VerifyOutcome` (via `domain::verify`).

use domain::{ChainIdentity, PersistedSoTChainGate, Strictness, verify::VerifyOutcome};

// ── SoTChain ─────────────────────────────────────────────────────────────────

/// Minimal check contract satisfied by all four chains.
///
/// Provides `check(input, strictness) -> VerifyOutcome`. The `strictness` value is
/// resolved by the caller from a `SignalGateMatrix` before dispatch.
///
/// For chains ①②③ this is fulfilled via the blanket impl over [`PersistedSoTChain`];
/// for chain ⓪ it is fulfilled directly (chain ⓪ does not persist state).
/// CLI `check-*` dispatch goes through this trait.
pub trait SoTChain: ChainIdentity {
    /// Evaluate the chain's gate for the given input, returning a [`VerifyOutcome`].
    ///
    /// - [`Strictness::Strict`]: Yellow signals produce `Finding::error`.
    /// - [`Strictness::Interim`]: Yellow signals produce `Finding::warning`.
    ///
    /// Red signals always produce `Finding::error` regardless of `strictness`.
    fn check(input: &Self::Input<'_>, strictness: Strictness) -> VerifyOutcome;
}

// ── LiveSoTChain ─────────────────────────────────────────────────────────────

/// Extension of [`SoTChain`] for chains that compute signals live without persisting.
///
/// Implemented **only** by `AdrUserChain` (chain ⓪). `calc_live` returns the
/// live-computed result without writing any file.
pub trait LiveSoTChain: SoTChain {
    /// The live-computed signal result type.
    type LiveCalc;
    /// Error produced when live calculation fails.
    type CalcError;

    /// Compute the chain's signals live from `input` without writing any persisted file.
    fn calc_live(input: &Self::Input<'_>) -> Result<Self::LiveCalc, Self::CalcError>;
}

// ── LoadablePersistedChain ───────────────────────────────────────────────────

/// I/O port contract for chains ①②③ that persist their computed signal documents.
///
/// Carries the impure operations that require I/O (filesystem reads/writes) and
/// therefore belong in the usecase layer as secondary port implementations:
/// - `calc`: compute signals and write the persisted document to disk.
/// - `load`: read a previously persisted document from disk.
/// - `check_freshness`: compare the persisted document against current sources.
///
/// Implementations live in `usecase::chain::{spec_adr, catalog_spec, impl_catalog}`.
/// The blanket `impl<T: PersistedSoTChain> SoTChain for T` can reference both
/// `LoadablePersistedChain` and `SoTChain` without cross-layer dependency because
/// all three traits are defined in this crate.
pub trait LoadablePersistedChain: PersistedSoTChainGate {
    /// Compute signals from `input` and write the result to the persisted document path.
    fn calc(input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError>;

    /// Load a previously persisted document from the path specified in `input`.
    fn load(input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError>;

    /// Verify that `persisted` is still up-to-date relative to the current `input`.
    fn check_freshness(
        input: &Self::Input<'_>,
        persisted: &Self::Persisted,
    ) -> Result<(), Self::StaleError>;
}

// ── PersistedSoTChain ────────────────────────────────────────────────────────

/// Full contract for chains ①②③: combines the pure gate interface
/// (`PersistedSoTChainGate`) with the I/O port (`LoadablePersistedChain`).
pub trait PersistedSoTChain: PersistedSoTChainGate + LoadablePersistedChain {}

/// Blanket: every `LoadablePersistedChain` implementor automatically satisfies
/// [`PersistedSoTChain`].
impl<T> PersistedSoTChain for T where T: LoadablePersistedChain {}

// ── Blanket impl: PersistedSoTChain → SoTChain ───────────────────────────────

/// Blanket impl: any `T: PersistedSoTChain` automatically satisfies [`SoTChain`].
///
/// The `check` pipeline is fixed as `load → check_freshness → evaluate_gate`.
impl<T> SoTChain for T
where
    T: PersistedSoTChain,
{
    fn check(input: &Self::Input<'_>, strictness: Strictness) -> VerifyOutcome {
        let persisted = match T::load(input) {
            Ok(persisted) => persisted,
            Err(error) => return T::calc_error(error),
        };
        match T::check_freshness(input, &persisted) {
            Ok(()) => T::evaluate_gate(&persisted, strictness),
            Err(error) => T::stale_error(error),
        }
    }
}
