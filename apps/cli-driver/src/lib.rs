// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `cli-driver` — primary adapter layer for the CLI delivery.
//!
//! Owns argument parsing → typed command input, use-case interactor invocation,
//! and `CommandOutcome` rendering / JSON formatting.
//!
//! The `cli-composition` crate stays as the wiring layer (DI / composition root).
//! T014–T020 stage the separation source files; T021 atomically flips the workspace
//! structure (Cargo.toml, dependency graph, deny.toml, architecture-rules.json,
//! apps/cli main.rs wiring) per CN-06.

pub mod arch;
pub mod conventions;
pub mod demo;
pub mod domain;
pub mod file;
pub mod git;
pub mod guard;
pub mod hook;
pub mod ref_verify;
pub mod render;
pub mod signal;
pub mod telemetry;
pub mod verify;

pub use render::CommandOutcome;
