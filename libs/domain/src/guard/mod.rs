//! Shell command guard module.
//!
//! Provides deterministic shell command parsing and git operation
//! blocking policy as pure computation (no I/O).

mod parser;
pub mod policy;
mod verdict;

pub use parser::{SimpleCommand, extract_command_substitutions, split_shell, tokenize};
pub use verdict::{Decision, GuardVerdict, ParseError};
