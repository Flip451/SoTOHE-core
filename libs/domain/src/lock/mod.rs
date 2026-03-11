//! File lock domain types, RAII guard, and port trait.

mod error;
mod guard;
mod port;
mod types;

pub use error::LockError;
pub use guard::{FileGuard, ReleaseFn};
pub use port::FileLockManager;
pub use types::{AgentId, FilePath, LockEntry, LockMode};
