//! `impl CliApp` delegation shims for the signal command family.
//!
//! Each method forwards to `SignalCompositionRoot::new().method(...)`,
//! preserving `apps/cli` call sites unchanged during the per-context dissolution
//! migration (T012). T013 / T021 will remove `CliApp` entirely.
