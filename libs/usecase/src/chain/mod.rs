//! Four-chain SoT signal evaluation structs (usecase layer).
//!
//! Each sub-module provides one chain implementation struct:
//!
//! | Chain | Module | Struct | Traits |
//! |-------|--------|--------|--------|
//! | ⓪ `adr-user`    | [`adr_user`]    | [`AdrUserChain`]    | `ChainIdentity` + `SoTChain` + `LiveSoTChain` |
//! | ① `spec-adr`    | [`spec_adr`]    | [`SpecAdrChain`]    | `ChainIdentity` + `PersistedSoTChain` (→ `SoTChain` blanket) |
//! | ② `catalog-spec`| [`catalog_spec`]| [`CatalogSpecChain`]| `ChainIdentity` + `PersistedSoTChain` (→ `SoTChain` blanket) |
//! | ③ `impl-catalog`| [`impl_catalog`]| [`ImplCatalogChain`]| `ChainIdentity` + `PersistedSoTChain` (→ `SoTChain` blanket) |
//!
//! The blanket impl in `domain::chain` ensures that any type implementing
//! `PersistedSoTChain` automatically satisfies `SoTChain` with the fixed
//! `load → check_freshness → evaluate_gate` pipeline.  Chain ⓪ implements
//! `SoTChain` directly (no persistence file).

pub mod adr_user;
pub mod catalog_spec;
pub mod impl_catalog;
pub mod spec_adr;

pub use adr_user::AdrUserChain;
pub use catalog_spec::{CatalogSpecChain, CatalogSpecInput, CatalogSpecStaleError};
pub use impl_catalog::{ImplCatalogChain, ImplCatalogInput, ImplCatalogStaleError};
pub use spec_adr::{SpecAdrChain, SpecAdrInput, SpecAdrStaleError};

#[cfg(test)]
pub(crate) mod test_support {
    use domain::{ChainIdentity, PersistedSoTChain, SoTChain, verify::VerifyOutcome};

    pub(crate) fn assert_persisted_chain_bounds<T>()
    where
        T: ChainIdentity + PersistedSoTChain + SoTChain,
    {
    }

    pub(crate) fn call_sotchain_check<T>(input: &T::Input<'_>, strict: bool) -> VerifyOutcome
    where
        T: SoTChain,
    {
        T::check(input, strict)
    }
}
