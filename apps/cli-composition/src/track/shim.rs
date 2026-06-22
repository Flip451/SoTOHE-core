//! `CliApp` compatibility shims for the `track` command family.
//!
//! All methods here delegate to the corresponding `TrackCompositionRoot`
//! method.  They exist solely so that existing `apps/cli` call-sites
//! (`CliApp::new().track_*()`) continue to compile without change.
//!
//! No business logic lives here — every body is a one-line delegation.
