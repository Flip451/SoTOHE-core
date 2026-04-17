//! TDDD (Type-Definition-Driven Development) module.
//!
//! Groups type catalogue definitions, signal evaluation, and consistency
//! checking for the per-track / per-layer type catalogue (e.g.
//! `domain-types.json`).
//!
//! Historical note (T001): the catalogue + signal + consistency logic used to
//! live in a single `catalogue.rs` (2088 lines). The TDDD-01 track split it
//! into three modules to meet DM-06's module-size guideline and enable the
//! layer-neutral rename from `DomainType*` to `TypeDefinition*` /
//! `TypeCatalogue*` / `TypeSignal` (ADR 0002 §D3).

pub mod baseline;
pub mod catalogue;
pub mod consistency;
pub mod layer_id;
pub mod signals;

pub use layer_id::LayerId;
