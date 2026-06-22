//! Infrastructure adapters for the PR review polling service (D4 / T008).
//!
//! Provides:
//! - [`SystemSleepAdapter`]: implements [`usecase::pr_review_polling::SleepPort`] via
//!   `std::thread::sleep`.

use std::time::Duration;

use usecase::pr_review_polling::SleepPort;

// ── SystemSleepAdapter ────────────────────────────────────────────────────────

/// Infrastructure adapter implementing [`SleepPort`] via `std::thread::sleep`.
///
/// The sole implementation used in production; tests use a mock/recording
/// implementation injected at construction time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSleepAdapter;

impl SleepPort for SystemSleepAdapter {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
