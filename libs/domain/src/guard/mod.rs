//! Shell command guard module.
//!
//! Provides deterministic shell command policy as pure computation (no I/O).
//! Shell parsing is abstracted behind the [`ShellParser`] port trait;
//! the conch-parser backed implementation lives in the infrastructure layer.

pub mod policy;
mod port;
mod text;
mod types;
mod verdict;

pub use port::ShellParser;
pub use text::{extract_command_substitutions, tokenize};
pub use types::SimpleCommand;
pub use verdict::{GuardVerdict, ParseError};
