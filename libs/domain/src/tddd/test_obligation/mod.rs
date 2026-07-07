//! Test-obligation gate domain model (SoT chain third-link semantic verification).
//!
//! Groups the value objects, vocabularies, errors, and ports that model the
//! obligation-fulfillment gate: the decision table
//! (`.harness/config/test-obligation-rules.json`) that drives obligation
//! derivation, and the load-time totality guarantee that keeps role coverage
//! fail-closed. See the track spec (IN-02 / IN-04 / IN-17 / CN-05 / CN-10) and
//! ADR `2026-07-02-0359-test-obligation-and-fulfillment-gate`.

pub mod errors;
pub mod ids;
pub mod ports;
pub mod rules;
pub mod vocab;
