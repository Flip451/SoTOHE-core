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

pub mod adr_baseline;
pub mod arch;
pub mod batch_plan;
pub mod capability;
pub mod catalog_gen;
pub mod codex_runtime;
pub mod contract_map;
pub mod conventions;
pub mod conventions_resolve;
pub mod demo;
pub mod domain;
pub mod dry;
pub mod file;
pub mod git;
pub mod guard;
pub mod hook;
pub mod maintenance;
pub mod phase_command;
pub mod pr;
pub mod ref_verify;
pub mod render;
pub mod review;
pub mod semantic_dup;
pub mod signal;
pub mod signal_report;
pub mod task_contract;
pub mod telemetry;
pub mod template_conventions;
pub mod template_export;
pub mod test_obligation;
pub mod track;
mod track_clear_override;
mod track_contract_map;
mod track_lint;
mod track_next_task;
pub mod track_resolution;
mod track_set_commit_hash;
mod track_set_override;
mod track_spec_element_hash;
mod track_switch_base;
pub mod track_tddd;
mod track_transition;
mod track_type_signals;
pub mod verify;

pub use render::CommandOutcome;
