//! Hook subdomain: types, verdicts, and errors for security-critical hook dispatch.

mod error;
mod types;
mod verdict;

pub use error::HookError;
pub use types::{HookContext, HookInput, HookName};
pub use verdict::HookVerdict;
