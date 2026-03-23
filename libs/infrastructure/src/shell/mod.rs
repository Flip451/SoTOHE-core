//! Shell parsing infrastructure adapter.
//!
//! Provides [`ConchShellParser`], the conch-parser backed implementation of
//! [`domain::guard::ShellParser`].

mod conch;
mod flatten;

pub use conch::ConchShellParser;
